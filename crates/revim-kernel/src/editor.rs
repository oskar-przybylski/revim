//! The running editor: state, modules, registries and the effect queue.

use std::collections::{HashSet, VecDeque};

use crate::{
    command::CommandRegistry,
    ctx::Ctx,
    event::{self, Effect, Event},
    gutter::{GutterColumn, GutterContext, GutterProviderRegistry},
    key::Key,
    keymap::{Keymap, Mode},
    module::{Module, ModuleFailure, RegisterError, RegisterErrorKind, Registrar},
    panic::run_guarded,
    state::EditorState,
};

/// Upper bound of effects run for one input; protects against commands or modules that keep
/// triggering each other. The rest of the queue is dropped when it is reached.
const MAX_EFFECTS: usize = 10_000;

/// The editor kernel at runtime: state, modules, registries and the effect queue.
///
/// Input is handled run-to-completion: a key runs its command, then every queued command and
/// event, before the next key is handled.
///
/// Module code (registration, command handlers, event handlers) runs guarded: a module that
/// panics is disabled — its commands and event handler are not called again — and recorded in
/// [`failures`](Self::failures); the editor keeps running.
pub struct Editor {
    state: EditorState,
    modules: Vec<Box<dyn Module>>,
    /// Ids of modules that panicked; their code is skipped.
    disabled_modules: HashSet<&'static str>,
    /// Why each disabled module was disabled, in order.
    failures: Vec<ModuleFailure>,
    commands: CommandRegistry,
    keymap: Keymap,
    gutters: GutterProviderRegistry,
    effects: VecDeque<Effect>,
    mode: Mode,
}

impl Editor {
    /// Registers all modules, then emits `DocumentOpened` for the initial document.
    ///
    /// # Errors
    /// Every problem found, each tagged with the module it comes from: duplicate commands
    /// or keys, invalid key notation, keys bound to commands nobody registered, a missing
    /// `editor.quit` and modules that panicked while registering (see [`RegisterErrorKind`]).
    pub fn new(
        state: EditorState,
        mut modules: Vec<Box<dyn Module>>,
    ) -> Result<Self, Vec<RegisterError>> {
        let mut commands = CommandRegistry::default();
        let mut keymap = Keymap::default();
        let mut gutters = GutterProviderRegistry::default();
        let mut errors: Vec<RegisterError> = Vec::new();
        log::info!("Starting module registration");
        modules.iter_mut().for_each(|m| {
            log::info!("Registering module [{}]", m.id());
            if let Err(message) = run_guarded(|| {
                m.register(&mut Registrar::new(
                    m.id(),
                    &mut commands,
                    &mut keymap,
                    &mut gutters,
                    &mut errors,
                ))
            }) {
                errors.push(RegisterError {
                    module: m.id(),
                    kind: RegisterErrorKind::ModulePanicked { message },
                });
            }
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
        event::emit(
            &mut effects,
            Event::DocumentOpened {
                doc: state.focused_doc_id(),
            },
        );

        let mut editor = Self {
            state,
            modules,
            disabled_modules: HashSet::new(),
            failures: Vec::new(),
            commands,
            keymap,
            gutters,
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
    /// Returns `false` if the key is not bound (nothing happens). A key bound to a command of
    /// a disabled module returns `true` and does nothing.
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

    /// Runs queued effects until the queue is empty or [`MAX_EFFECTS`] is reached; then the rest
    /// of the queue is dropped with a warning.
    /// Isolates module functions so panic doesn't break editor loop. Disables modules that panic.
    fn drain(&mut self) {
        let mut iteration: usize = 0;
        while let Some(effect) = self.effects.pop_front() {
            iteration += 1;
            if iteration > MAX_EFFECTS {
                // `effect` was taken from the queue but not run: dropped too
                log::warn!(
                    "more than {MAX_EFFECTS} effects for one input, dropping {} queued effects \
                     (commands or modules may be triggering each other)",
                    self.effects.len() + 1
                );
                self.effects.clear();
                break;
            }

            let mut ctx = Ctx::new(&mut self.state, &mut self.effects);
            match effect {
                Effect::Exec(name) => {
                    let Some(cmd) = self.commands.get(&name) else {
                        continue;
                    };
                    if self.disabled_modules.contains(cmd.module) {
                        continue;
                    }
                    if let Err(message) = run_guarded(|| (cmd.handler)(&mut ctx)) {
                        record_failure(
                            &mut self.disabled_modules,
                            &mut self.failures,
                            cmd.module,
                            message,
                        );
                    }
                }
                Effect::Emit(event) => {
                    for module in &mut self.modules {
                        let id = module.id();
                        if self.disabled_modules.contains(id) {
                            continue;
                        }
                        if let Err(message) = run_guarded(|| module.on_event(&event, &mut ctx)) {
                            record_failure(
                                &mut self.disabled_modules,
                                &mut self.failures,
                                id,
                                message,
                            );
                        }
                    }
                }
            }
        }
    }

    /// Modules that panicked and were disabled, in the order it happened.
    pub fn failures(&self) -> &[ModuleFailure] {
        &self.failures
    }

    /// Asks every gutter provider for its column for the focused window drawn `height` rows
    /// high, in registration order.
    ///
    /// Each column has one cell per visible line, starting at the window's scroll position and
    /// ending at the last line of the document — rows past the end get no cells. Columns of
    /// width 0 are left out. Providers of disabled modules are skipped; a provider that panics
    /// disables its module and its column is left out.
    pub fn gutter_columns(&mut self, height: usize) -> Vec<GutterColumn> {
        let doc = self.state.focused_doc();
        let window = self.state.focused_window();
        let cx = GutterContext { doc, window };

        // only lines that exist: no numbers below the end of the file
        let first = window.scroll();
        let last = (first + height).min(doc.line_count());

        let mut columns = Vec::new();
        for (module, provider) in self.gutters.providers() {
            if self.disabled_modules.contains(module) {
                continue;
            }
            let column = run_guarded(|| {
                let width = provider.width(&cx);
                // width 0 hides the column: no cells to ask for
                (width > 0).then(|| GutterColumn {
                    width,
                    cells: (first..last).map(|line| provider.cell(&cx, line)).collect(),
                })
            });
            match column {
                Ok(Some(column)) => columns.push(column),
                Ok(None) => {}
                Err(message) => record_failure(
                    &mut self.disabled_modules,
                    &mut self.failures,
                    module,
                    message,
                ),
            }
        }
        columns
    }
}

/// Disables `module` and remembers why.
fn record_failure(
    disabled: &mut HashSet<&'static str>,
    failures: &mut Vec<ModuleFailure>,
    module: &'static str,
    message: String,
) {
    disabled.insert(module);
    failures.push(ModuleFailure { module, message });
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

    // ----- panic isolation -----

    /// Counts how often its code runs; its command always panics, `on_event` optionally.
    struct Boom {
        handler_calls: Rc<Cell<usize>>,
        event_calls: Rc<Cell<usize>>,
        panic_in_event: bool,
    }

    impl Module for Boom {
        fn id(&self) -> &'static str {
            "boom"
        }
        fn register(&mut self, r: &mut Registrar) {
            let calls = self.handler_calls.clone();
            r.command("boom.now", "", move |_| {
                calls.set(calls.get() + 1);
                panic!("handler failed");
            });
            r.keymap(Mode::Normal, "b", "boom.now");
        }
        fn on_event(&mut self, _event: &Event, _cx: &mut Ctx) {
            self.event_calls.set(self.event_calls.get() + 1);
            if self.panic_in_event {
                panic!("event failed");
            }
        }
    }

    struct BoomCounters {
        handler_calls: Rc<Cell<usize>>,
        event_calls: Rc<Cell<usize>>,
    }

    /// `Boom` first, then `Probe` (which provides `editor.quit` and `j`).
    fn editor_with_boom(panic_in_event: bool) -> (Editor, BoomCounters, Counters) {
        let boom = BoomCounters {
            handler_calls: Rc::new(Cell::new(0)),
            event_calls: Rc::new(Cell::new(0)),
        };
        let probe = Counters {
            moved: Rc::new(Cell::new(0)),
            opened: Rc::new(Cell::new(0)),
        };
        let modules: Vec<Box<dyn Module>> = vec![
            Box::new(Boom {
                handler_calls: boom.handler_calls.clone(),
                event_calls: boom.event_calls.clone(),
                panic_in_event,
            }),
            Box::new(Probe {
                moved: probe.moved.clone(),
                opened: probe.opened.clone(),
            }),
        ];
        let state = EditorState::with_document(Document::from_text(None, "a\nb\n"));
        (Editor::new(state, modules).unwrap(), boom, probe)
    }

    fn failure(module: &'static str, message: &str) -> ModuleFailure {
        ModuleFailure {
            module,
            message: message.into(),
        }
    }

    #[test]
    fn panic_in_register_is_a_registration_error() {
        let errors = register(vec![
            Box::new(TestModule("editor", quit)),
            Box::new(TestModule("boom", |_| panic!("register failed"))),
            Box::new(TestModule("bad", |r| {
                r.keymap(Mode::Normal, "<Foo>", "editor.quit")
            })),
        ]);
        assert_eq!(errors.len(), 2, "{errors:?}");
        assert_eq!(
            errors[0],
            RegisterError {
                module: "boom",
                kind: RegisterErrorKind::ModulePanicked {
                    message: "register failed".into()
                },
            }
        );
        // registration goes on after a panic: the next module's error is reported too
        assert_eq!(errors[1].module, "bad");
    }

    #[test]
    fn panicking_handler_disables_its_module() {
        let (mut editor, boom, _) = editor_with_boom(false);
        assert!(editor.handle_key(Key::Char('b')));
        assert_eq!(boom.handler_calls.get(), 1);
        assert_eq!(editor.failures, [failure("boom", "handler failed")]);
        assert!(editor.disabled_modules.contains("boom"));
    }

    #[test]
    fn editor_keeps_working_after_a_module_panics() {
        let (mut editor, _, probe) = editor_with_boom(false);
        editor.handle_key(Key::Char('b'));
        assert!(editor.handle_key(Key::Char('j'))); // Probe's command still runs
        assert_eq!(editor.state().focused_window().cursor(), 2);
        assert_eq!(probe.moved.get(), 1);
        editor.handle_key(Key::Ctrl('q'));
        assert!(editor.state().should_quit());
    }

    #[test]
    fn disabled_module_command_is_not_called_again() {
        let (mut editor, boom, _) = editor_with_boom(false);
        editor.handle_key(Key::Char('b'));
        assert!(editor.handle_key(Key::Char('b'))); // still bound, but skipped
        assert_eq!(boom.handler_calls.get(), 1);
        assert_eq!(editor.failures.len(), 1);
    }

    #[test]
    fn disabled_module_gets_no_more_events() {
        let (mut editor, boom, probe) = editor_with_boom(false);
        assert_eq!(boom.event_calls.get(), 1); // DocumentOpened at startup
        editor.handle_key(Key::Char('b')); // panics in the handler → disabled
        editor.handle_key(Key::Char('j')); // CursorMoved
        assert_eq!(boom.event_calls.get(), 1);
        assert_eq!(probe.moved.get(), 1);
    }

    #[test]
    fn panicking_event_handler_disables_module_and_others_still_get_the_event() {
        // Boom panics on DocumentOpened while the editor starts
        let (mut editor, boom, probe) = editor_with_boom(true);
        assert_eq!(editor.failures, [failure("boom", "event failed")]);
        assert_eq!(probe.opened.get(), 1); // Probe comes after Boom and still got it

        editor.handle_key(Key::Char('j'));
        assert_eq!(boom.event_calls.get(), 1); // no more events for Boom
        assert_eq!(probe.moved.get(), 1);

        editor.handle_key(Key::Char('b'));
        assert_eq!(boom.handler_calls.get(), 0); // nor its commands
    }

    // ----- gutter -----

    use crate::gutter::{GutterCell, GutterProvider, Highlight};

    /// Shows the 0-based line index, prefixed with its own tag.
    struct LineIndex(&'static str);
    impl GutterProvider for LineIndex {
        fn width(&self, _cx: &GutterContext) -> u16 {
            2
        }
        fn cell(&self, _cx: &GutterContext, line: usize) -> GutterCell {
            GutterCell {
                text: format!("{}{line}", self.0),
                highlight: Highlight::Normal,
            }
        }
    }

    /// Panics on the first cell.
    struct Panics;
    impl GutterProvider for Panics {
        fn width(&self, _cx: &GutterContext) -> u16 {
            1
        }
        fn cell(&self, _cx: &GutterContext, _line: usize) -> GutterCell {
            panic!("gutter failed")
        }
    }

    fn gutter_editor(text: &str, modules: Vec<Box<dyn Module>>) -> Editor {
        let mut all: Vec<Box<dyn Module>> = vec![Box::new(TestModule("editor", quit))];
        all.extend(modules);
        let state = EditorState::with_document(Document::from_text(None, text));
        Editor::new(state, all).unwrap()
    }

    /// The texts of each column's cells.
    fn texts(columns: &[GutterColumn]) -> Vec<Vec<&str>> {
        columns
            .iter()
            .map(|c| c.cells.iter().map(|cell| cell.text.as_str()).collect())
            .collect()
    }

    #[test]
    fn gutter_columns_come_in_registration_order() {
        let mut editor = gutter_editor(
            "a\nb\n",
            vec![
                Box::new(TestModule("first", |r| r.gutter(LineIndex("a")))),
                Box::new(TestModule("second", |r| {
                    r.gutter(LineIndex("b"));
                    r.gutter(LineIndex("c"));
                })),
            ],
        );
        let columns = editor.gutter_columns(2);
        assert_eq!(texts(&columns), [["a0", "a1"], ["b0", "b1"], ["c0", "c1"]]);
        assert!(columns.iter().all(|c| c.width == 2));
    }

    #[test]
    fn gutter_cells_only_for_lines_that_exist() {
        let mut editor = gutter_editor(
            "a\nb\n",
            vec![Box::new(TestModule("g", |r| r.gutter(LineIndex(""))))],
        );
        // 2 lines in a 5 rows high window; no cell for the empty line after the last `\n`
        assert_eq!(texts(&editor.gutter_columns(5)), [["0", "1"]]);
        assert_eq!(texts(&editor.gutter_columns(1)), [["0"]]);
    }

    #[test]
    fn gutter_cells_start_at_the_scroll_position() {
        let mut editor = gutter_editor(
            "a\nb\nc\nd\n",
            vec![Box::new(TestModule("g", |r| r.gutter(LineIndex(""))))],
        );
        editor.state_mut().set_view_height(2);
        for _ in 0..3 {
            editor.state_mut().move_cursor(Motion::Down); // cursor on "d", scrolled by 2
        }
        assert_eq!(texts(&editor.gutter_columns(2)), [["2", "3"]]);
    }

    /// Hidden (width 0); fails the test if asked for a cell.
    struct Hidden;
    impl GutterProvider for Hidden {
        fn width(&self, _cx: &GutterContext) -> u16 {
            0
        }
        fn cell(&self, _cx: &GutterContext, _line: usize) -> GutterCell {
            panic!("cell of a hidden column")
        }
    }

    #[test]
    fn zero_width_column_is_left_out() {
        let mut editor = gutter_editor(
            "a\n",
            vec![Box::new(TestModule("g", |r| {
                r.gutter(Hidden);
                r.gutter(LineIndex(""));
            }))],
        );
        assert_eq!(texts(&editor.gutter_columns(1)), [["0"]]);
        assert!(editor.failures().is_empty()); // `cell` was not called
    }

    #[test]
    fn panicking_gutter_provider_disables_its_module() {
        let mut editor = gutter_editor(
            "a\n",
            vec![
                Box::new(TestModule("boom", |r| r.gutter(Panics))),
                Box::new(TestModule("g", |r| r.gutter(LineIndex("")))),
            ],
        );
        // the other module's column is still there
        assert_eq!(texts(&editor.gutter_columns(1)), [["0"]]);
        assert_eq!(editor.failures(), [failure("boom", "gutter failed")]);

        // not called again: no second failure
        assert_eq!(texts(&editor.gutter_columns(1)), [["0"]]);
        assert_eq!(editor.failures().len(), 1);
    }

    #[test]
    fn module_disabled_elsewhere_loses_its_gutter_column() {
        let mut editor = gutter_editor(
            "a\n",
            vec![Box::new(TestModule("boom", |r| {
                r.command("boom.now", "", |_| panic!("handler failed"));
                r.keymap(Mode::Normal, "b", "boom.now");
                r.gutter(LineIndex(""));
            }))],
        );
        assert_eq!(editor.gutter_columns(1).len(), 1);
        editor.handle_key(Key::Char('b')); // the command panics → module disabled
        assert!(editor.gutter_columns(1).is_empty());
    }
}
