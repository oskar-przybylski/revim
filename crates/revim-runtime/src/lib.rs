//! Editor runtime: terminal setup, the main event loop and dispatching input to the kernel.

use std::{io, path::PathBuf};

use crossterm::event::{Event, EventStream, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use futures::StreamExt;
use ratatui::DefaultTerminal;
use revim_core::movement::Motion;
use revim_kernel::{
    document::{Document, Uri},
    state::EditorState,
};
use revim_ui::window_view::WindowView;

/// Runs the editor until the user quits (`Ctrl-q`).
///
/// Opens `path` (or an empty scratch buffer when `None`; a missing file opens as an empty
/// document with that path), takes over the terminal, runs the event loop and restores the
/// terminal afterwards — also when the loop fails.
///
/// # Errors
/// If the file cannot be read (other than not existing), or on a terminal I/O error.
pub async fn run(path: Option<PathBuf>) -> io::Result<()> {
    let doc = load(path).await?;
    let mut state = EditorState::with_document(doc);

    let mut terminal = ratatui::init();
    let result = event_loop(&mut terminal, &mut state).await;
    ratatui::restore();
    result
}

/// Draws a frame, waits for the next event, handles it — until quit or a terminal error.
async fn event_loop(terminal: &mut DefaultTerminal, state: &mut EditorState) -> io::Result<()> {
    let mut events = EventStream::new();
    loop {
        terminal.draw(|f| {
            let area = f.area();
            state.set_view_height(area.height as usize);
            let view = WindowView::new(state.focused_window(), state.focused_doc());
            f.render_widget(&view, area);
            if let Some(pos) = view.cursor_position(area) {
                f.set_cursor_position(pos);
            }
        })?;

        tokio::select! {
            maybe_event = events.next() => match maybe_event {
                Some(Ok(Event::Key(key)))
                    if key.kind == KeyEventKind::Press
                        && key.code == KeyCode::Char('q')
                        && key.modifiers.contains(KeyModifiers::CONTROL) => return Ok(()),
                Some(Ok(Event::Key(key))) if key.kind == KeyEventKind::Press => {
                    if let Some(m) = key_to_motion(&key) { state.move_cursor(m) }
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

/// Translates a key press into a cursor motion (`h` `j` `k` `l`); other keys give `None`.
/// Temporary: to be replaced by the keymap.
fn key_to_motion(key: &KeyEvent) -> Option<Motion> {
    if key.kind == KeyEventKind::Press {
        match key.code {
            KeyCode::Char('h') => Some(Motion::Left),
            KeyCode::Char('j') => Some(Motion::Down),
            KeyCode::Char('k') => Some(Motion::Up),
            KeyCode::Char('l') => Some(Motion::Right),
            _ => None,
        }
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn press(c: char) -> KeyEvent {
        KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE)
    }

    #[test]
    fn hjkl_map_to_motions() {
        assert_eq!(key_to_motion(&press('h')), Some(Motion::Left));
        assert_eq!(key_to_motion(&press('j')), Some(Motion::Down));
        assert_eq!(key_to_motion(&press('k')), Some(Motion::Up));
        assert_eq!(key_to_motion(&press('l')), Some(Motion::Right));
    }

    #[test]
    fn other_keys_are_not_motions() {
        assert_eq!(key_to_motion(&press('x')), None);
        assert_eq!(
            key_to_motion(&KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
            None
        );
    }

    #[test]
    fn key_release_is_ignored() {
        let release = KeyEvent::new_with_kind(
            KeyCode::Char('j'),
            KeyModifiers::NONE,
            KeyEventKind::Release,
        );
        assert_eq!(key_to_motion(&release), None);
    }
}
