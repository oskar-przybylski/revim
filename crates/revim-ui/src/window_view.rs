//! Rendering of a window: the visible part of its document and the cursor.

use ratatui::{
    buffer::Buffer,
    layout::{Position, Rect},
    style::Style,
    widgets::Widget,
};
use revim_kernel::{document::Document, window::Window};

/// A view of a [`Window`] that can be drawn with ratatui.
///
/// Borrows the window and its document for one frame. Draws lines starting at the window's
/// scroll position, one text line per screen row, without wrapping; [`cursor_position`]
/// tells where to place the terminal cursor.
///
/// [`cursor_position`]: WindowView::cursor_position
///
/// ```
/// use ratatui::{buffer::Buffer, layout::{Position, Rect}, widgets::Widget};
/// use revim_kernel::{document::Document, state::EditorState};
/// use revim_ui::window_view::WindowView;
///
/// let state = EditorState::with_document(Document::from_text(None, "hello\nworld\n"));
/// let view = WindowView::new(state.focused_window(), state.focused_doc());
///
/// let area = Rect::new(0, 0, 5, 2);
/// let mut buf = Buffer::empty(area);
/// (&view).render(area, &mut buf);
///
/// assert_eq!(buf, Buffer::with_lines(["hello", "world"]));
/// assert_eq!(view.cursor_position(area), Some(Position::new(0, 0)));
/// ```
pub struct WindowView<'a> {
    window: &'a Window,
    doc: &'a Document,
}

impl<'a> WindowView<'a> {
    /// Creates a view of `window` showing `doc` (the window's document).
    pub fn new(window: &'a Window, doc: &'a Document) -> Self {
        Self { window, doc }
    }

    /// Screen position of the cursor when the view is drawn in `area`, or `None` if the
    /// cursor is outside of it (scrolled out vertically or past the right edge).
    pub fn cursor_position(&self, area: Rect) -> Option<Position> {
        let text = self.doc.text();
        let cursor = self.window.cursor();
        let line = text.char_to_line(cursor);
        let col = cursor - text.line_to_char(line);

        let row = line.checked_sub(self.window.scroll())?;
        if row >= area.height as usize || col >= area.width as usize {
            return None;
        }
        Some(Position::new(area.x + col as u16, area.y + row as u16))
    }
}

impl Widget for &WindowView<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let lines = self
            .doc
            .text()
            .lines()
            .skip(self.window.scroll())
            .take(area.height as usize);

        for (row, line) in lines.enumerate() {
            let content = displayable(&line.to_string());
            buf.set_stringn(
                area.x,
                area.y + row as u16,
                content,
                area.width as usize,
                Style::default(),
            );
        }
    }
}

/// Prepares one line of text for the screen: drops the line ending and replaces tabs with a
/// single space, so that one char always takes one cell (matching the cursor column).
fn displayable(line: &str) -> String {
    line.trim_end_matches(['\n', '\r']).replace('\t', " ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use revim_core::movement::Motion;
    use revim_kernel::state::EditorState;

    fn render(state: &EditorState, area: Rect) -> (Buffer, Option<Position>) {
        let view = WindowView::new(state.focused_window(), state.focused_doc());
        let mut buf = Buffer::empty(area);
        (&view).render(area, &mut buf);
        (buf, view.cursor_position(area))
    }

    fn state(text: &str) -> EditorState {
        EditorState::with_document(Document::from_text(None, text))
    }

    #[test]
    fn renders_visible_lines_from_scroll() {
        let mut state = state("one\ntwo\nthree\n");
        state.set_view_height(2);
        state.move_cursor(Motion::Down);
        state.move_cursor(Motion::Down); // cursor on "three", view scrolled by one line

        let (buf, cursor) = render(&state, Rect::new(0, 0, 10, 2));
        assert_eq!(buf, Buffer::with_lines(["two       ", "three     "]));
        assert_eq!(cursor, Some(Position::new(0, 1)));
    }

    #[test]
    fn strips_crlf_line_endings() {
        let (buf, _) = render(&state("a\r\nb\r\n"), Rect::new(0, 0, 3, 2));
        assert_eq!(buf, Buffer::with_lines(["a  ", "b  "]));
    }

    #[test]
    fn truncates_long_lines_to_width() {
        let (buf, _) = render(&state("abcdef\n"), Rect::new(0, 0, 3, 1));
        assert_eq!(buf, Buffer::with_lines(["abc"]));
    }

    #[test]
    fn replaces_tabs_with_one_cell() {
        let (buf, _) = render(&state("\tx\n"), Rect::new(0, 0, 3, 1));
        assert_eq!(buf, Buffer::with_lines([" x "]));
    }

    #[test]
    fn cursor_follows_column() {
        let mut state = state("abc\n");
        state.move_cursor(Motion::Right);
        state.move_cursor(Motion::Right);
        let (_, cursor) = render(&state, Rect::new(0, 0, 10, 1));
        assert_eq!(cursor, Some(Position::new(2, 0)));
    }

    #[test]
    fn cursor_hidden_past_right_edge() {
        let mut state = state("abcdef\n");
        for _ in 0..5 {
            state.move_cursor(Motion::Right);
        }
        let (_, cursor) = render(&state, Rect::new(0, 0, 3, 1));
        assert_eq!(cursor, None);
    }

    #[test]
    fn cursor_is_offset_by_area_origin() {
        let (_, cursor) = render(&state("abc\n"), Rect::new(4, 2, 10, 3));
        assert_eq!(cursor, Some(Position::new(4, 2)));
    }

    #[test]
    fn empty_document_has_cursor_at_origin() {
        let (buf, cursor) = render(&state(""), Rect::new(0, 0, 2, 1));
        assert_eq!(buf, Buffer::with_lines(["  "]));
        assert_eq!(cursor, Some(Position::new(0, 0)));
    }
}
