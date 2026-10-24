//! API for writing Revim modules. A module depends on this crate only.
//!
//! # Modules
//!
//! Everything the editor can do — moving the cursor, quitting, and later the statusline,
//! git or LSP — is provided by a [`Module`]. A module:
//!
//! - registers **commands** — named actions such as `vim.move.down` — and **key bindings**
//!   that run them, in [`Module::register`] through the [`Registrar`],
//! - changes the editor from command handlers through [`Ctx`]: read the state with
//!   [`Ctx::state`], move the cursor with [`Ctx::move_cursor`], run another command with
//!   [`Ctx::exec`], quit with [`Ctx::quit`],
//! - optionally reacts to [`Event`]s in [`Module::on_event`].
//!
//! Modules are listed in `src/conf.rs` of the editor; removing a line disables a module.
//!
//! # Conventions
//!
//! - **Command names** are `module.object.action`, e.g. `vim.move.down`, and must be unique.
//! - **Keys** are written in Vim notation: `"j"`, `"J"`, `"<C-q>"`, `"<Up>"`, `"<CR>"`,
//!   `"<Esc>"`. Sequences (`"gg"`) are not supported yet.
//! - Registration problems (duplicate commands or keys, invalid key notation, keys bound to
//!   commands nobody registered) are collected and reported at startup as
//!   [`RegisterError`]s naming the module — the editor does not start with them.
//! - Changes to the state happen immediately; [`Ctx::exec`] and events are queued and run
//!   after the current handler returns.
//! - [`Event`] gets new variants over time: always match it with a `_` arm.
//!
//! # Example
//!
//! A module that moves the cursor two lines down with `J`, tested with [`testing`]:
//!
//! ```
//! use revim_api::testing::{editor, press};
//! use revim_api::{Mode, Module, Motion, Registrar};
//!
//! /// Moves the cursor two lines down.
//! struct Jump;
//!
//! impl Module for Jump {
//!     fn id(&self) -> &'static str {
//!         "jump"
//!     }
//!
//!     fn register(&mut self, r: &mut Registrar) {
//!         r.command("jump.down2", "Move cursor two lines down", |cx| {
//!             cx.move_cursor(Motion::Down);
//!             cx.move_cursor(Motion::Down);
//!         });
//!         r.keymap(Mode::Normal, "J", "jump.down2");
//!     }
//! }
//!
//! let mut ed = editor("a\nb\nc\n", vec![Box::new(Jump)]);
//! assert!(press(&mut ed, "J"));
//! assert_eq!(ed.state().focused_window().cursor(), 4); // 'c'
//! ```
//!
//! Reacting to events — read the current state through the id carried by the event:
//!
//! ```
//! use revim_api::{Ctx, Event, Module, Registrar};
//!
//! /// Remembers the last cursor position of any window.
//! #[derive(Default)]
//! struct LastCursor(Option<usize>);
//!
//! impl Module for LastCursor {
//!     fn id(&self) -> &'static str {
//!         "last-cursor"
//!     }
//!
//!     fn register(&mut self, _r: &mut Registrar) {}
//!
//!     fn on_event(&mut self, event: &Event, cx: &mut Ctx) {
//!         match event {
//!             Event::CursorMoved { window } => {
//!                 self.0 = cx.state().window(*window).map(|w| w.cursor());
//!             }
//!             _ => {}
//!         }
//!     }
//! }
//! ```

pub use revim_core::movement::Motion;
pub use revim_kernel::{
    ctx::Ctx,
    document::{Document, Uri},
    event::Event,
    key::{Key, KeyParseError},
    keymap::Mode,
    module::{Module, RegisterError, RegisterErrorKind, Registrar},
    state::{DocId, EditorState},
    window::{Window, WindowId},
};

/// Helpers for testing modules without a terminal: build an editor with your module and
/// press keys.
pub mod testing {
    pub use revim_kernel::{editor::Editor, key::parse_key};

    use crate::{Document, EditorState, Module, Registrar};

    /// Provides `editor.quit`, which the kernel requires, so that a module can be tested alone.
    struct QuitStub;

    impl Module for QuitStub {
        fn id(&self) -> &'static str {
            "test"
        }
        fn register(&mut self, r: &mut Registrar) {
            r.command("editor.quit", "Quit the editor (test stub)", |cx| cx.quit());
        }
    }

    /// An editor on `text` with `modules`, plus a stub `editor.quit` command.
    ///
    /// Don't pass a module that registers `editor.quit` itself — build it with
    /// [`Editor::new`] instead.
    ///
    /// # Panics
    /// If registration fails; the message lists every problem.
    pub fn editor(text: &str, mut modules: Vec<Box<dyn Module>>) -> Editor {
        modules.insert(0, Box::new(QuitStub));
        let state = EditorState::with_document(Document::from_text(None, text));
        match Editor::new(state, modules) {
            Ok(editor) => editor,
            Err(errors) => {
                let list: Vec<String> = errors.iter().map(ToString::to_string).collect();
                panic!("module registration failed:\n{}", list.join("\n"));
            }
        }
    }

    /// Presses `key` written in Vim notation (`"j"`, `"<C-q>"`); returns whether it was bound.
    ///
    /// # Panics
    /// If the notation is invalid.
    pub fn press(editor: &mut Editor, key: &str) -> bool {
        let parsed = parse_key(key).unwrap_or_else(|e| panic!("invalid key `{key}`: {e}"));
        editor.handle_key(parsed)
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn editor_without_modules_starts() {
            let ed = editor("a\n", vec![]);
            assert_eq!(ed.state().focused_doc().text().to_string(), "a\n");
        }

        #[test]
        fn press_reports_unbound_keys() {
            let mut ed = editor("a\n", vec![]);
            assert!(!press(&mut ed, "j"));
        }

        #[test]
        #[should_panic(expected = "invalid key")]
        fn press_panics_on_invalid_notation() {
            let mut ed = editor("a\n", vec![]);
            press(&mut ed, "<Foo>");
        }
    }
}
