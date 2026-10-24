use std::{io, path::PathBuf};

use crossterm::event::{Event, EventStream, KeyCode, KeyEventKind, KeyModifiers};
use futures::StreamExt;
use ratatui::{
    DefaultTerminal,
    text::{Line, Text},
    widgets::{Block, Paragraph},
};
use revim_kernel::{
    document::{Document, Uri},
    state::EditorState,
};

pub async fn run(path: Option<PathBuf>) -> io::Result<()> {
    let doc = load(path).await?;
    let mut state = EditorState::with_document(doc);

    let mut terminal = ratatui::init();
    let result = event_loop(&mut terminal, &mut state).await;
    ratatui::restore();
    result
}

async fn event_loop(terminal: &mut DefaultTerminal, state: &mut EditorState) -> io::Result<()> {
    let mut events = EventStream::new();
    loop {
        terminal.draw(|f| {
            let area = f.area();
            let text = state.focused_doc().text();
            let lines: Vec<Line> = text
                .lines()
                .take(area.height as usize)
                .map(|line| {
                    let mut s = line.to_string();
                    let len = s.trim_end_matches(['\n', '\r']).len();
                    s.truncate(len);
                    Line::from(s)
                })
                .collect();

            let title = match state.focused_doc().uri() {
                Some(Uri::File(fname)) => fname.display().to_string(),
                _ => String::from("Scratchpad"),
            };
            f.render_widget(
                Paragraph::new(Text::from(lines)).block(Block::bordered().title(title)),
                area,
            );
        })?;

        tokio::select! {
            maybe_event = events.next() => match maybe_event {
                Some(Ok(Event::Key(key)))
                    if key.kind == KeyEventKind::Press
                        && key.code == KeyCode::Char('q')
                        && key.modifiers.contains(KeyModifiers::CONTROL) => return Ok(()),
                Some(Ok(_)) => {}
                Some(Err(e)) => return Err(e),
                None => return Ok(()),
            }
        }
    }
}

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
