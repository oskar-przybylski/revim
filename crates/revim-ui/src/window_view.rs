//! Rendering of a window: the visible part of its document and the cursor.

use ratatui::{
    buffer::Buffer,
    layout::{Position, Rect},
    style::{Color, Modifier, Style},
    widgets::Widget,
};
use revim_kernel::{
    document::Document,
    gutter::{GutterColumn, Highlight},
    window::Window,
};

/// A view of a [`Window`] that can be drawn with ratatui.
///
/// Borrows the window, its document and the gutter columns for one frame. Draws the gutter
/// columns on the left, one space, then the lines starting at the window's scroll position,
/// one text line per screen row, without wrapping. Rows past the end of the document show a
/// dimmed `~` in the first cell, like in Vim. [`cursor_position`] tells where to place the
/// terminal cursor.
///
/// [`cursor_position`]: WindowView::cursor_position
///
/// ```
/// use ratatui::{buffer::Buffer, layout::{Position, Rect}, widgets::Widget};
/// use revim_kernel::{document::Document, state::EditorState};
/// use revim_ui::window_view::WindowView;
///
/// let state = EditorState::with_document(Document::from_text(None, "hello\nworld\n"));
/// let view = WindowView::new(state.focused_window(), state.focused_doc(), &[]);
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
    columns: &'a [GutterColumn],
}

impl<'a> WindowView<'a> {
    /// Creates a view of `window` showing `doc` (the window's document) with gutter `columns`
    /// (see `Editor::gutter_columns`); pass `&[]` for no gutter.
    pub fn new(window: &'a Window, doc: &'a Document, columns: &'a [GutterColumn]) -> Self {
        Self {
            window,
            doc,
            columns,
        }
    }

    /// Screen position of the cursor when the view is drawn in `area`, or `None` if the
    /// cursor is outside of the text area (scrolled out vertically or past the right edge).
    pub fn cursor_position(&self, area: Rect) -> Option<Position> {
        let (_, text_area) = self.split(area);
        let text = self.doc.text();
        let cursor = self.window.cursor();
        let line = text.char_to_line(cursor);
        let col = cursor - text.line_to_char(line);

        let row = line.checked_sub(self.window.scroll())?;
        if row >= text_area.height as usize || col >= text_area.width as usize {
            return None;
        }
        Some(Position::new(
            text_area.x + col as u16,
            text_area.y + row as u16,
        ))
    }

    /// Splits `area` into the gutter (all columns plus one space after them, if they take any
    /// room) and the text.
    /// The single place that decides the layout, so drawing and the cursor always agree.
    fn split(&self, area: Rect) -> (Rect, Rect) {
        let columns = self
            .columns
            .iter()
            .fold(0u16, |sum, column| sum.saturating_add(column.width));
        let separator = u16::from(columns > 0);
        let gutter_width = columns.saturating_add(separator).min(area.width);

        let gutter = Rect {
            width: gutter_width,
            ..area
        };
        let text = Rect {
            x: area.x + gutter_width,
            width: area.width - gutter_width,
            ..area
        };
        (gutter, text)
    }
}

impl Widget for &WindowView<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let (gutter, text_area) = self.split(area);

        // gutter columns, left to right, clipped to the gutter area
        let mut x = gutter.x;
        for column in self.columns {
            let room = (gutter.right() - x).min(column.width);
            if room == 0 {
                continue; // zero width, or the gutter is full
            }
            for (row, cell) in column.cells.iter().take(gutter.height as usize).enumerate() {
                buf.set_stringn(
                    x,
                    gutter.y + row as u16,
                    &cell.text,
                    room as usize,
                    style(cell.highlight),
                );
            }
            x += room;
        }

        // lines as Vim counts them, so the empty line after a trailing `\n` is not drawn
        let scroll = self.window.scroll();
        let shown = self
            .doc
            .line_count()
            .saturating_sub(scroll)
            .min(area.height as usize);

        let lines = self.doc.text().lines().skip(scroll).take(shown);
        for (row, line) in lines.enumerate() {
            let content = displayable(&line.to_string());
            buf.set_stringn(
                text_area.x,
                text_area.y + row as u16,
                content,
                text_area.width as usize,
                Style::default(),
            );
        }

        // past the end of the document: `~` where the gutter would be
        for row in shown..area.height as usize {
            buf.set_stringn(
                area.x,
                area.y + row as u16,
                "~",
                area.width as usize,
                END_OF_FILE,
            );
        }
    }
}

/// Style of the `~` marking rows past the end of the document.
const END_OF_FILE: Style = Style::new().fg(Color::DarkGray);

/// Screen style of a gutter highlight. Temporary fixed colors until themes exist.
fn style(highlight: Highlight) -> Style {
    match highlight {
        Highlight::LineNumber => Style::new().fg(Color::White),
        Highlight::CurrentLineNumber => Style::new().fg(Color::Yellow).add_modifier(Modifier::BOLD),
        Highlight::Dimmed => Style::new().fg(Color::DarkGray),
        _ => Style::default(),
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
    use revim_kernel::{gutter::GutterCell, state::EditorState};

    fn render(state: &EditorState, area: Rect) -> (Buffer, Option<Position>) {
        let view = WindowView::new(state.focused_window(), state.focused_doc(), &[]);
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

    // ----- gutter -----

    fn column(width: u16, cells: &[(&str, Highlight)]) -> GutterColumn {
        GutterColumn {
            width,
            cells: cells
                .iter()
                .map(|(text, highlight)| GutterCell {
                    text: text.to_string(),
                    highlight: *highlight,
                })
                .collect(),
        }
    }

    fn render_with(
        state: &EditorState,
        columns: &[GutterColumn],
        area: Rect,
    ) -> (Buffer, Option<Position>) {
        let view = WindowView::new(state.focused_window(), state.focused_doc(), columns);
        let mut buf = Buffer::empty(area);
        (&view).render(area, &mut buf);
        (buf, view.cursor_position(area))
    }

    /// The text of the buffer without styles, one string per row.
    fn text_of(buf: &Buffer) -> Vec<String> {
        let area = buf.area;
        (area.top()..area.bottom())
            .map(|y| {
                (area.left()..area.right())
                    .map(|x| buf[(x, y)].symbol())
                    .collect()
            })
            .collect()
    }

    #[test]
    fn gutter_is_drawn_left_of_the_text_with_a_space() {
        let numbers = column(
            2,
            &[
                (" 1", Highlight::CurrentLineNumber),
                (" 2", Highlight::LineNumber),
            ],
        );
        let (buf, _) = render_with(&state("ab\ncd\n"), &[numbers], Rect::new(0, 0, 6, 2));
        assert_eq!(text_of(&buf), [" 1 ab ", " 2 cd "]);
    }

    #[test]
    fn gutter_cells_use_their_highlight() {
        let cells = column(
            1,
            &[
                ("1", Highlight::CurrentLineNumber),
                ("2", Highlight::LineNumber),
                ("3", Highlight::Dimmed),
                ("4", Highlight::Normal),
            ],
        );
        let (buf, _) = render_with(&state("a\nb\nc\nd\n"), &[cells], Rect::new(0, 0, 4, 4));
        assert_eq!(buf[(0, 0)].fg, Color::Yellow);
        assert!(buf[(0, 0)].modifier.contains(Modifier::BOLD));
        assert_eq!(buf[(0, 1)].fg, Color::White);
        assert_eq!(buf[(0, 2)].fg, Color::DarkGray);
        assert_eq!(buf[(0, 3)].fg, Color::Reset); // `Normal` keeps the default style
        assert_eq!(buf[(2, 0)].fg, Color::Reset); // so does the text
    }

    #[test]
    fn several_columns_are_drawn_in_order() {
        let signs = column(1, &[("+", Highlight::Normal)]);
        let numbers = column(2, &[(" 1", Highlight::LineNumber)]);
        let (buf, _) = render_with(&state("x\n"), &[signs, numbers], Rect::new(0, 0, 6, 1));
        assert_eq!(text_of(&buf), ["+ 1 x "]);
    }

    #[test]
    fn cursor_is_shifted_by_the_gutter() {
        let mut state = state("abc\n");
        state.move_cursor(Motion::Right);
        let numbers = column(3, &[("  1", Highlight::CurrentLineNumber)]);
        let (_, cursor) = render_with(&state, &[numbers], Rect::new(0, 0, 10, 1));
        assert_eq!(cursor, Some(Position::new(5, 0))); // 3 gutter + 1 space + column 1
    }

    #[test]
    fn cursor_past_the_text_area_is_hidden() {
        // area 6 wide: gutter 2 + space 1 → text area 3 wide (columns 0..=2)
        let mut state = state("abcdef\n");
        for _ in 0..3 {
            state.move_cursor(Motion::Right);
        }
        let numbers = column(2, &[(" 1", Highlight::LineNumber)]);
        let (_, cursor) = render_with(&state, &[numbers], Rect::new(0, 0, 6, 1));
        assert_eq!(cursor, None);
    }

    #[test]
    fn gutter_wider_than_the_area_does_not_panic() {
        let wide = column(50, &[("1", Highlight::LineNumber)]);
        let (buf, cursor) = render_with(&state("a\n"), &[wide], Rect::new(0, 0, 4, 1));
        assert_eq!(text_of(&buf), ["1   "]);
        assert_eq!(cursor, None); // no room left for the text
    }

    #[test]
    fn zero_width_column_takes_no_room() {
        let hidden = column(0, &[("x", Highlight::Normal)]);
        let numbers = column(1, &[("1", Highlight::LineNumber)]);
        let (buf, _) = render_with(&state("a\n"), &[hidden, numbers], Rect::new(0, 0, 4, 1));
        assert_eq!(text_of(&buf), ["1 a "]);

        let (buf, cursor) = render_with(
            &state("a\n"),
            &[column(0, &[("x", Highlight::Normal)])],
            Rect::new(0, 0, 2, 1),
        );
        assert_eq!(text_of(&buf), ["a "]); // no separator either
        assert_eq!(cursor, Some(Position::new(0, 0)));
    }

    // ----- end of file -----

    #[test]
    fn rows_past_the_end_show_a_tilde() {
        let (buf, _) = render(&state("a\n"), Rect::new(0, 0, 3, 3));
        assert_eq!(text_of(&buf), ["a  ", "~  ", "~  "]);
        assert_eq!(buf[(0, 1)].fg, Color::DarkGray);
    }

    #[test]
    fn tilde_is_drawn_in_place_of_the_gutter() {
        let numbers = column(2, &[(" 1", Highlight::CurrentLineNumber)]);
        let (buf, _) = render_with(&state("a\n"), &[numbers], Rect::new(0, 0, 5, 2));
        assert_eq!(text_of(&buf), [" 1 a ", "~    "]);
    }

    #[test]
    fn empty_document_has_one_line_then_tildes() {
        let (buf, _) = render(&state(""), Rect::new(0, 0, 2, 3));
        assert_eq!(text_of(&buf), ["  ", "~ ", "~ "]);
    }

    #[test]
    fn last_line_without_line_ending_is_not_a_tilde() {
        let (buf, _) = render(&state("a\nb"), Rect::new(0, 0, 2, 3));
        assert_eq!(text_of(&buf), ["a ", "b ", "~ "]);
    }
}
