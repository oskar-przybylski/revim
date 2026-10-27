//! Editor runtime: terminal setup, the main event loop and dispatching input to the kernel.

use std::{io, path::PathBuf};

use crossterm::{
    cursor::SetCursorStyle,
    event::{Event, EventStream, KeyCode, KeyEvent, KeyEventKind, KeyModifiers},
};
use futures::StreamExt;
use ratatui::DefaultTerminal;
use revim_kernel::{
    document::{Document, Uri},
    editor::Editor,
    key::Key,
    keymap::Mode,
    module::Module,
    panic::in_guarded_mode,
    state::EditorState,
};
use revim_ui::window_view::WindowView;

/// Runs the editor with `modules` until a command asks to quit (`editor.quit`).
///
/// Opens `path` (or an empty scratch buffer when `None`; a missing file opens as an empty
/// document with that path) and registers the modules — both before taking over the
/// terminal, so a failure ends the editor before the UI starts. Then runs the event loop and
/// restores the terminal afterwards, also when the loop fails.
///
/// A panic in module code does not touch the terminal: the panic and the disabling of the
/// module are logged and the editor keeps running. Any other panic restores the terminal and
/// ends the editor.
///
/// # Errors
/// If the file cannot be read (other than not existing; the message names the file), if module
/// registration fails (every problem is logged, the error only counts them), or on a terminal
/// I/O error.
pub async fn run(path: Option<PathBuf>, modules: Vec<Box<dyn Module>>) -> io::Result<()> {
    let doc = load(path).await?;
    let state = EditorState::with_document(doc);
    let mut editor = Editor::new(state, modules).map_err(|errors| {
        errors.iter().for_each(|e| log::error!("{e}"));
        io::Error::other(format!(
            "module registration failed ({} errors)",
            errors.len()
        ))
    })?;

    let mut terminal = ratatui::init();
    // Wraps ratatui's hook (installed by `init`), which restores the terminal. Module panics
    // are caught by the kernel, so the terminal must stay as it is for them; they are only
    // logged here, because only the hook knows where the panic happened.
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        if in_guarded_mode() {
            log::error!("module {info}");
        } else {
            previous(info)
        }
    }));
    let result = event_loop(&mut terminal, &mut editor).await;
    // Give the shell back its own cursor shape. Ignoring a failure here is deliberate: it must
    // not hide the result of the event loop, and the terminal is restored next anyway.
    let _ = crossterm::execute!(io::stdout(), SetCursorStyle::DefaultUserShape);
    ratatui::restore();
    result
}

/// Draws a frame, waits for the next event, handles it — until quit or a terminal error.
async fn event_loop(terminal: &mut DefaultTerminal, editor: &mut Editor) -> io::Result<()> {
    let mut events = EventStream::new();
    let mut logged = 0; // module failures already logged
    let mut shown_mode: Option<Mode> = None;
    loop {
        terminal.draw(|f| {
            let area = f.area();
            let h = area.height as usize;
            editor.state_mut().set_view_height(h);
            let gutters = editor.gutter_columns(h);
            let state = editor.state();
            let view = WindowView::new(state.focused_window(), state.focused_doc(), &gutters);
            f.render_widget(&view, area);
            if let Some(pos) = view.cursor_position(area) {
                f.set_cursor_position(pos);
            }
        })?;
        let mode = editor.state().mode();
        if shown_mode != Some(mode) {
            crossterm::execute!(terminal.backend_mut(), cursor_style(mode))?;
            shown_mode = Some(mode);
        }
        // gutter providers run while drawing and may disable their module
        log_new_failures(editor, &mut logged);

        tokio::select! {
            maybe_event = events.next() => match maybe_event {
                Some(Ok(Event::Key(key))) if key.kind == KeyEventKind::Press => {
                   if let Some(key) = to_key(&key) { editor.handle_key(key); }
                   log_new_failures(editor, &mut logged);
                   if editor.state().should_quit() { return Ok(()) }
                }
                Some(Ok(_)) => {}
                Some(Err(e)) => return Err(e),
                None => return Ok(()),
            }
        }
    }
}

/// Terminal cursor shape for `mode`: a bar in Insert (the cursor is between chars), a block
/// otherwise (it stands on a char).
fn cursor_style(mode: Mode) -> SetCursorStyle {
    match mode {
        Mode::Insert => SetCursorStyle::SteadyBar,
        _ => SetCursorStyle::SteadyBlock,
    }
}

/// Logs the module failures recorded since the previous call; `logged` counts the ones
/// already logged.
fn log_new_failures(editor: &Editor, logged: &mut usize) {
    for failure in &editor.failures()[*logged..] {
        log::error!("{failure}");
    }
    *logged = editor.failures().len();
}

/// Reads the document to edit; a missing file gives an empty document with that path.
async fn load(path: Option<PathBuf>) -> io::Result<Document> {
    let doc = match path {
        Some(filepath) => {
            let text = match tokio::fs::read_to_string(&filepath).await {
                Ok(file) => {
                    log::info!(
                        "Loaded document: {}, size: {}B",
                        filepath.display(),
                        file.len()
                    );
                    file
                }
                Err(e) if e.kind() == io::ErrorKind::NotFound => {
                    log::info!("Created new document: {}", filepath.display());
                    String::new()
                }
                Err(e) => {
                    return Err(io::Error::new(
                        e.kind(),
                        format!("cannot open {}: {e}", filepath.display()),
                    ));
                }
            };

            Document::from_text(Some(Uri::File(filepath)), &text)
        }
        None => {
            log::info!("No path supplied in argument, creating scratchpad");
            Document::scratch()
        }
    };
    Ok(doc)
}

/// Translates a terminal key press into a kernel [`Key`].
///
/// Ctrl + character becomes [`Key::Ctrl`] with a lowercase letter; key releases and keys the
/// kernel does not know (function keys, …) give `None`. Other modifiers are ignored.
fn to_key(event: &KeyEvent) -> Option<Key> {
    if event.kind != KeyEventKind::Press {
        return None;
    }

    let key = match event.code {
        KeyCode::Char(c) if event.modifiers.contains(KeyModifiers::CONTROL) => {
            Key::Ctrl(c.to_ascii_lowercase())
        }
        KeyCode::Char(c) => Key::Char(c),
        KeyCode::Up => Key::Up,
        KeyCode::Down => Key::Down,
        KeyCode::Left => Key::Left,
        KeyCode::Right => Key::Right,
        KeyCode::Esc => Key::Esc,
        KeyCode::Enter => Key::Enter,
        KeyCode::Backspace => Key::Backspace,
        KeyCode::Tab => Key::Tab,
        _ => return None,
    };
    Some(key)
}

#[cfg(test)]
mod tests {
    use super::*;
    use revim_kernel::module::Registrar;

    fn press(code: KeyCode, modifiers: KeyModifiers) -> Option<Key> {
        to_key(&KeyEvent::new(code, modifiers))
    }

    #[test]
    fn characters_keep_their_case() {
        assert_eq!(
            press(KeyCode::Char('j'), KeyModifiers::NONE),
            Some(Key::Char('j'))
        );
        assert_eq!(
            press(KeyCode::Char('J'), KeyModifiers::SHIFT),
            Some(Key::Char('J'))
        );
    }

    #[test]
    fn ctrl_characters_are_lowercase() {
        assert_eq!(
            press(KeyCode::Char('q'), KeyModifiers::CONTROL),
            Some(Key::Ctrl('q'))
        );
        assert_eq!(
            press(KeyCode::Char('Q'), KeyModifiers::CONTROL),
            Some(Key::Ctrl('q'))
        );
    }

    #[test]
    fn named_keys() {
        assert_eq!(press(KeyCode::Up, KeyModifiers::NONE), Some(Key::Up));
        assert_eq!(press(KeyCode::Enter, KeyModifiers::NONE), Some(Key::Enter));
        assert_eq!(press(KeyCode::Esc, KeyModifiers::NONE), Some(Key::Esc));
        assert_eq!(
            press(KeyCode::Backspace, KeyModifiers::NONE),
            Some(Key::Backspace)
        );
        assert_eq!(press(KeyCode::Tab, KeyModifiers::NONE), Some(Key::Tab));
    }

    #[test]
    fn unsupported_keys_and_releases_are_ignored() {
        assert_eq!(press(KeyCode::F(5), KeyModifiers::NONE), None);
        let release = KeyEvent::new_with_kind(
            KeyCode::Char('j'),
            KeyModifiers::NONE,
            KeyEventKind::Release,
        );
        assert_eq!(to_key(&release), None);
    }

    /// Registers `editor.quit` and `boom.now`, a command bound to `b` that always panics.
    struct Boom;
    impl Module for Boom {
        fn id(&self) -> &'static str {
            "boom"
        }
        fn register(&mut self, r: &mut Registrar) {
            r.command("editor.quit", "Quit the editor", |cx, _| cx.quit());
            r.command("boom.now", "Panic", |_, _| panic!("boom"));
            r.keymap(Mode::Normal, "b", "boom.now", "");
        }
    }

    #[test]
    fn each_module_failure_is_logged_once() {
        let state = EditorState::with_document(Document::scratch());
        let mut editor = Editor::new(state, vec![Box::new(Boom)]).expect("modules register");
        let mut logged = 0;

        log_new_failures(&editor, &mut logged);
        assert_eq!(logged, 0);

        editor.handle_key(Key::Char('b'));
        log_new_failures(&editor, &mut logged);
        assert_eq!(logged, 1);

        log_new_failures(&editor, &mut logged); // nothing new since the last call
        assert_eq!(logged, 1);
    }

    #[test]
    fn cursor_is_a_bar_only_in_insert() {
        assert!(matches!(
            cursor_style(Mode::Insert),
            SetCursorStyle::SteadyBar
        ));
        assert!(matches!(
            cursor_style(Mode::Normal),
            SetCursorStyle::SteadyBlock
        ));
    }
}
