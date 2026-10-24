//! The running editor: state, modules, registries and the effect queue.

use std::collections::VecDeque;

use crate::{
    command::CommandRegistry,
    ctx::Ctx,
    event::{Effect, Event},
    key::Key,
    keymap::{Keymap, Mode},
    module::{Module, RegisterError, RegisterErrorKind, Registrar},
    state::EditorState,
};

/// Upper bound of effects run for one input; protects against commands or modules that keep
/// triggering each other. The rest of the queue is dropped when it is reached.
const MAX_EFFECTS: usize = 10_000;

/// The editor kernel at runtime: state, modules, registries and the effect queue.
///
/// Input is handled run-to-completion: a key runs its command, then every queued command and
/// event, before the next key is handled.
pub struct Editor {
    state: EditorState,
    modules: Vec<Box<dyn Module>>,
    commands: CommandRegistry,
    keymap: Keymap,
    effects: VecDeque<Effect>,
    mode: Mode,
}

impl Editor {
    /// Registers all modules, then emits `DocumentOpened` for the initial document.
    ///
    /// # Errors
    /// Every problem found, each tagged with the module it comes from: duplicate commands
    /// or keys, invalid key notation, keys bound to commands nobody registered and a missing
    /// `editor.quit` (see [`RegisterErrorKind`]).
    pub fn new(
        state: EditorState,
        mut modules: Vec<Box<dyn Module>>,
    ) -> Result<Self, Vec<RegisterError>> {
        let mut commands = CommandRegistry::default();
        let mut keymap = Keymap::default();
        let mut errors: Vec<RegisterError> = Vec::new();
        modules.iter_mut().for_each(|m| {
            m.register(&mut Registrar::new(
                m.id(),
                &mut commands,
                &mut keymap,
                &mut errors,
            ))
        });

        // Checked after all modules registered: a key may point to a command of a module
        // that comes later in the list.
        let mut unknown = Vec::new();
        for (&(mode, key), binding) in keymap.bindings() {
            if !commands.contains(&binding.command) {
                unknown.push(RegisterError {
                    module: binding.module,
                    kind: RegisterErrorKind::UnknownCommand {
                        mode,
                        key,
                        command: binding.command.clone(),
                    },
                });
            }
        }
        // HashMap order is random; sort so that the report is the same on every run.
        unknown.sort_by_key(ToString::to_string);
        errors.extend(unknown);

        if !commands.contains("editor.quit") {
            errors.push(RegisterError {
                module: "editor",
                kind: RegisterErrorKind::MissingQuitCommand,
            });
        }

        if !errors.is_empty() {
            return Err(errors);
        }

        let mut effects: VecDeque<Effect> = VecDeque::new();
        effects.push_back(Effect::Emit(Event::DocumentOpened {
            doc: state.focused_doc_id(),
        }));

        let mut editor = Self {
            state,
            modules,
            commands,
            keymap,
            effects,
            mode: Mode::Normal,
        };
        editor.drain();
        Ok(editor)
    }

    /// Read-only access to the editor state (e.g. for drawing).
    pub fn state(&self) -> &EditorState {
        &self.state
    }
    /// Mutable access to the editor state for the runtime (e.g. the view height).
    pub fn state_mut(&mut self) -> &mut EditorState {
        &mut self.state
    }

    /// Runs the command bound to `key` in the current mode, then everything it queued.
    ///
    /// Returns `false` if the key is not bound (nothing happens).
    pub fn handle_key(&mut self, key: Key) -> bool {
        if let Some(bind) = self.keymap.lookup(self.mode, key) {
            self.effects
                .push_back(Effect::Exec(bind.command.to_string()));
            self.drain();
            true
        } else {
            false
        }
    }

    /// Runs queued effects until the queue is empty or [`MAX_EFFECTS`] is reached.
    fn drain(&mut self) {
        let mut iteration: usize = 0;
        while let Some(effect) = self.effects.pop_front() {
            iteration += 1;
            if iteration > MAX_EFFECTS {
                self.effects = VecDeque::new();
                break;
            }

            let mut ctx = Ctx::new(&mut self.state, &mut self.effects);
            match effect {
                Effect::Exec(name) => {
                    if let Some(cmd) = self.commands.get(&name) {
                        (cmd.handler)(&mut ctx);
                    }
                }
                Effect::Emit(event) => {
                    self.modules
                        .iter_mut()
                        .for_each(|m| m.on_event(&event, &mut ctx));
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{cell::Cell, rc::Rc};

    use super::*;
    use crate::{document::Document, key::KeyParseError};
    use revim_core::movement::Motion;

    /// A module whose registration is a plain function.
    struct TestModule(&'static str, fn(&mut Registrar));
    impl Module for TestModule {
        fn id(&self) -> &'static str {
            self.0
        }
        fn register(&mut self, r: &mut Registrar) {
            (self.1)(r)
        }
    }

    fn register(modules: Vec<Box<dyn Module>>) -> Vec<RegisterError> {
        match Editor::new(EditorState::with_document(Document::scratch()), modules) {
            Ok(_) => Vec::new(),
            Err(errors) => errors,
        }
    }

    fn quit(r: &mut Registrar) {
        r.command("editor.quit", "Quit the editor", |cx| cx.quit());
        r.keymap(Mode::Normal, "<C-q>", "editor.quit");
    }

    #[test]
    fn valid_modules_register_without_errors() {
        assert_eq!(register(vec![Box::new(TestModule("editor", quit))]), []);
    }

    #[test]
    fn reports_every_error_with_its_module() {
        let modules: Vec<Box<dyn Module>> = vec![
            Box::new(TestModule("a", |r| {
                r.command("a.x", "", |_| {});
                r.command("a.x", "", |_| {});
                r.keymap(Mode::Normal, "<Foo>", "a.x");
                r.keymap(Mode::Normal, "j", "a.missing");
            })),
            Box::new(TestModule("b", |r| r.keymap(Mode::Normal, "j", "a.x"))),
        ];
        let error = |module, kind| RegisterError { module, kind };
        assert_eq!(
            register(modules),
            [
                error(
                    "a",
                    RegisterErrorKind::DuplicateCommand {
                        command: "a.x".into()
                    }
                ),
                error(
                    "a",
                    RegisterErrorKind::InvalidKey {
                        key: "<Foo>".into(),
                        reason: KeyParseError::UnknownKey("<Foo>".into()),
                    }
                ),
                error(
                    "b",
                    RegisterErrorKind::DuplicateKey {
                        mode: Mode::Normal,
                        key: Key::Char('j'),
                        bound_to: "a.missing".into(),
                    }
                ),
                error(
                    "a",
                    RegisterErrorKind::UnknownCommand {
                        mode: Mode::Normal,
                        key: Key::Char('j'),
                        command: "a.missing".into(),
                    }
                ),
                error("editor", RegisterErrorKind::MissingQuitCommand),
            ]
        );
    }

    #[test]
    fn unknown_commands_are_reported_in_a_stable_order() {
        let modules: Vec<Box<dyn Module>> = vec![
            Box::new(TestModule("editor", quit)),
            Box::new(TestModule("a", |r| {
                for key in ["a", "b", "c", "d", "e", "f"] {
                    r.keymap(Mode::Normal, key, "a.missing");
                }
            })),
        ];
        let keys: Vec<String> = register(modules)
            .iter()
            .map(|e| match &e.kind {
                RegisterErrorKind::UnknownCommand { key, .. } => key.to_string(),
                other => panic!("unexpected {other:?}"),
            })
            .collect();
        assert_eq!(keys, ["a", "b", "c", "d", "e", "f"]);
    }

    /// Counts events and offers commands for testing the effect queue.
    struct Probe {
        moved: Rc<Cell<usize>>,
        opened: Rc<Cell<usize>>,
    }

    impl Module for Probe {
        fn id(&self) -> &'static str {
            "probe"
        }
        fn register(&mut self, r: &mut Registrar) {
            quit(r);
            r.command("probe.down", "", |cx| cx.move_cursor(Motion::Down));
            r.command("probe.loop", "", |cx| cx.exec("probe.loop"));
            r.command("probe.deferred", "", |cx| {
                let before = cx.state().focused_window().cursor();
                cx.exec("probe.down");
                // `exec` is deferred: the cursor has not moved yet
                assert_eq!(cx.state().focused_window().cursor(), before);
            });
            r.keymap(Mode::Normal, "j", "probe.down");
            r.keymap(Mode::Normal, "L", "probe.loop");
            r.keymap(Mode::Normal, "d", "probe.deferred");
        }
        fn on_event(&mut self, event: &Event, _cx: &mut Ctx) {
            if matches!(event, Event::CursorMoved { .. }) {
                self.moved.set(self.moved.get() + 1);
            }
            if matches!(event, Event::DocumentOpened { .. }) {
                self.opened.set(self.opened.get() + 1);
            }
        }
    }

    struct Counters {
        moved: Rc<Cell<usize>>,
        opened: Rc<Cell<usize>>,
    }

    fn editor_with_probe(text: &str) -> (Editor, Counters) {
        let moved = Rc::new(Cell::new(0));
        let opened = Rc::new(Cell::new(0));
        let probe = Probe {
            moved: moved.clone(),
            opened: opened.clone(),
        };
        let state = EditorState::with_document(Document::from_text(None, text));
        let editor = Editor::new(state, vec![Box::new(probe)]).unwrap();
        (editor, Counters { moved, opened })
    }

    #[test]
    fn bound_key_runs_its_command() {
        let (mut editor, _) = editor_with_probe("a\nb\n");
        assert!(editor.handle_key(Key::Char('j')));
        assert_eq!(editor.state().focused_window().cursor(), 2);
    }

    #[test]
    fn unbound_key_is_not_handled() {
        let (mut editor, _) = editor_with_probe("a\nb\n");
        assert!(!editor.handle_key(Key::Char('x')));
        assert_eq!(editor.state().focused_window().cursor(), 0);
    }

    #[test]
    fn document_opened_is_sent_at_startup() {
        let (_, counters) = editor_with_probe("a\n");
        assert_eq!(counters.opened.get(), 1);
    }

    #[test]
    fn cursor_moved_reaches_modules() {
        let (mut editor, counters) = editor_with_probe("a\nb\n");
        editor.handle_key(Key::Char('j'));
        assert_eq!(counters.moved.get(), 1);
    }

    #[test]
    fn no_cursor_moved_when_cursor_stays() {
        let (mut editor, counters) = editor_with_probe("a\n");
        editor.handle_key(Key::Char('j')); // a single line: nowhere to go
        assert_eq!(counters.moved.get(), 0);
    }

    #[test]
    fn exec_runs_after_the_current_handler() {
        let (mut editor, _) = editor_with_probe("a\nb\n");
        editor.handle_key(Key::Char('d')); // asserts inside the handler
        assert_eq!(editor.state().focused_window().cursor(), 2);
    }

    #[test]
    fn command_calling_itself_stops_at_the_limit() {
        let (mut editor, _) = editor_with_probe("a\nb\n");
        editor.handle_key(Key::Char('L')); // would loop forever without MAX_EFFECTS
        assert!(editor.effects.is_empty());
        assert!(editor.handle_key(Key::Char('j'))); // the editor still works
        assert_eq!(editor.state().focused_window().cursor(), 2);
    }

    #[test]
    fn quit_command_sets_the_flag() {
        let (mut editor, _) = editor_with_probe("a\n");
        editor.handle_key(Key::Ctrl('q'));
        assert!(editor.state().should_quit());
    }
}
