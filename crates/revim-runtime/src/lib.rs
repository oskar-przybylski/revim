//! Editor runtime: terminal setup, the main event loop and dispatching input to the kernel.

use std::{io, path::PathBuf};

use crossterm::event::{Event, EventStream, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use futures::StreamExt;
use ratatui::DefaultTerminal;
use revim_kernel::{
    document::{Document, Uri},
    editor::Editor,
    key::Key,
    module::Module,
    panic::in_guarded_mode,
    state::EditorState,
};
use revim_ui::window_view::WindowView;

/// Runs the editor with `modules` until a command asks to quit (`editor.quit`).
///
/// Opens `path` (or an empty scratch buffer when `None`; a missing file opens as an empty
/// document with that path) and registers the modules — both before taking over the
/// terminal, so errors are printed normally. Then runs the event loop and restores the
/// terminal afterwards, also when the loop fails.
///
/// A panic in module code does not touch the terminal: the module is disabled and the editor
/// keeps running; disabled modules are listed after the terminal is restored. Any other panic
/// restores the terminal and ends the editor.
///
/// # Errors
/// If the file cannot be read (other than not existing), if module registration fails
/// (the message lists every problem), or on a terminal I/O error.
pub async fn run(path: Option<PathBuf>, modules: Vec<Box<dyn Module>>) -> io::Result<()> {
    let doc = load(path).await?;
    let state = EditorState::with_document(doc);
    let mut editor = Editor::new(state, modules).map_err(|errors| {
        let list: Vec<String> = errors.iter().map(ToString::to_string).collect();
        io::Error::other(format!("module registration failed:\n{}", list.join("\n")))
    })?;

    let mut terminal = ratatui::init();
    // Wraps ratatui's hook (installed by `init`), which restores the terminal. Module panics
    // are caught by the kernel, so the terminal must stay as it is for them.
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        if !in_guarded_mode() {
            previous(info)
        }
    }));
    let result = event_loop(&mut terminal, &mut editor).await;
    ratatui::restore();
    editor.failures().iter().for_each(|f| eprintln!("{f}"));
    result
}

/// Draws a frame, waits for the next event, handles it — until quit or a terminal error.
async fn event_loop(terminal: &mut DefaultTerminal, editor: &mut Editor) -> io::Result<()> {
    let mut events = EventStream::new();
    loop {
        terminal.draw(|f| {
            let area = f.area();
            editor.state_mut().set_view_height(area.height as usize);
            let state = editor.state();
            let view = WindowView::new(state.focused_window(), state.focused_doc());
            f.render_widget(&view, area);
            if let Some(pos) = view.cursor_position(area) {
                f.set_cursor_position(pos);
            }
        })?;

        tokio::select! {
            maybe_event = events.next() => match maybe_event {
                Some(Ok(Event::Key(key))) if key.kind == KeyEventKind::Press => {
                   if let Some(key) = to_key(&key) { editor.handle_key(key); }
                   if editor.state().should_quit() { return Ok(()) }
                }
                Some(Ok(_)) => {}
                Some(Err(e)) => return Err(e),
                None => return Ok(()),
            }
        }
    }
}

/// Reads the document to edit; a missing file gives an empty document with that path.
async fn load(path: Option<PathBuf>) -> io::Result<Document> {
    let doc = match path {
        Some(filepath) => {
            let text = match tokio::fs::read_to_string(&filepath).await {
                Ok(file) => file,
                Err(e) if e.kind() == io::ErrorKind::NotFound => String::new(),
                Err(e) => return Err(e),
            };

            Document::from_text(Some(Uri::File(filepath)), &text)
        }
        None => Document::scratch(),
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
}
