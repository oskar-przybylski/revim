use std::io;

use crossterm::event::{Event, EventStream, KeyCode, KeyEventKind, KeyModifiers};
use futures::StreamExt;
use ratatui::{
    DefaultTerminal,
    widgets::{Block, Paragraph},
};

pub async fn run() -> io::Result<()> {
    let mut terminal = ratatui::init();
    let result = event_loop(&mut terminal).await;
    ratatui::restore();
    result
}

async fn event_loop(terminal: &mut DefaultTerminal) -> io::Result<()> {
    let mut events = EventStream::new();
    loop {
        terminal.draw(|f| {
            f.render_widget(
                Paragraph::new("revim").centered().block(Block::bordered()),
                f.area(),
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
