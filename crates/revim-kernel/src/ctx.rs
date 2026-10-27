//! The context passed to command handlers and event handlers.

use std::collections::VecDeque;

use revim_core::{movement::Motion, transaction::Transaction};

use crate::{
    event::{Effect, Event},
    keymap::Mode,
    state::EditorState,
};

/// What a module can do while handling a command or an event.
///
/// Changes to the state happen immediately; [`exec`](Self::exec) and the events emitted by
/// changes are queued and run after the current handler returns, so a handler is never
/// re-entered while it runs.
pub struct Ctx<'a> {
    state: &'a mut EditorState,
    effects: &'a mut VecDeque<Effect>,
}
impl<'a> Ctx<'a> {
    pub(crate) fn new(state: &'a mut EditorState, effects: &'a mut VecDeque<Effect>) -> Self {
        Self { state, effects }
    }

    /// Read-only access to the editor state.
    pub fn state(&self) -> &EditorState {
        self.state
    }

    /// Switches the editor to `mode`. The cursors of the focused document are moved to a
    /// valid position for the new mode (in Normal off the end of the line, see
    /// [`Mode::cursor_past_end`]).
    ///
    /// Emits [`Event::CursorMoved`] if the cursor of the focused window had to move, then
    /// [`Event::ModeChanged`]. Does nothing if the editor already is in `mode`.
    pub fn set_mode(&mut self, mode: Mode) {
        let before = self.state.focused_window().cursor();
        if self.state().mode() == mode {
            return;
        }
        self.state.set_mode(mode);
        if self.state.focused_window().cursor() != before {
            self.emit_event(Event::CursorMoved {
                window: self.state.focused_window_id(),
            });
        }
        self.emit_event(Event::ModeChanged);
    }

    /// Applies `tx` to the document of the focused window: changes its text, moves the cursors
    /// of every window showing it through the change and puts the focused window's cursor at
    /// `tx.selection`, if given. Then every cursor is moved to a valid position for the current
    /// mode. Emits [`Event::DocChanged`].
    ///
    /// Build the change for the current text: [`ChangeSet::len_before`] must equal the length
    /// of the document, and positions in `tx.selection` refer to the text after the change.
    ///
    /// ```
    /// use revim_kernel::ctx::Ctx;
    /// use revim_core::{change::ChangeSet, selection::Selection, transaction::Transaction};
    ///
    /// /// Inserts "hi" at the cursor and puts the cursor after it.
    /// fn insert_hi(cx: &mut Ctx, _args: &str) {
    ///     let pos = cx.state().focused_window().cursor();
    ///     let len = cx.state().focused_doc().text().len_chars();
    ///     let tx = Transaction::new(ChangeSet::replace(len, pos..pos, "hi"))
    ///         .with_selection(Selection::point(pos + 2));
    ///     cx.apply(tx);
    /// }
    /// ```
    ///
    /// # Panics
    /// If `tx` does not fit the document or its selection is outside of the new text; the
    /// module is then disabled. Nothing is changed in that case.
    ///
    /// [`ChangeSet::len_before`]: revim_core::change::ChangeSet::len_before
    pub fn apply(&mut self, tx: Transaction) {
        let changes = tx.changes.clone();
        self.state.apply(tx);
        self.emit_event(Event::DocChanged {
            doc: self.state.focused_doc_id(),
            changes,
            version: self.state.focused_doc().version(),
        });
    }

    /// Moves the cursor of the focused window; emits [`Event::CursorMoved`] if it actually moved.
    pub fn move_cursor(&mut self, motion: Motion) {
        let window = self.state.focused_window_id();
        let before = self.state.focused_window().cursor();
        self.state.move_cursor(motion);
        if self.state.focused_window().cursor() != before {
            self.emit_event(Event::CursorMoved { window });
        }
    }
    /// Runs the command `command` with the argument `args` (`""` for none) after the current
    /// handler returns. An unknown name is ignored.
    pub fn exec(&mut self, command: &str, args: &str) {
        log::debug!("queued command execution {command} {args}");
        self.effects.push_back(Effect::Exec {
            command: command.to_string(),
            args: args.to_string(),
        })
    }

    /// Queues `event` for every module, after the current handler returns.
    fn emit_event(&mut self, event: Event) {
        log::debug!("queued event {event:?}");
        self.effects.push_back(Effect::Emit(event));
    }

    /// Asks the editor to quit after the current input is handled.
    pub fn quit(&mut self) {
        log::debug!("queued exit");
        self.state.request_quit();
    }
}
