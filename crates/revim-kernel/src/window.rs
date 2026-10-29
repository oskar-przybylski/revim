//! Windows: views of documents.

use revim_core::{
    change::ChangeSet,
    movement::{Motion, clamp_cursor, column, down, left, right, up},
    selection::Selection,
};
use ropey::Rope;
use slotmap::new_key_type;

use crate::state::DocId;

new_key_type! {
    /// Stable identifier of a window inside the editor state.
    pub struct WindowId;
}

/// A view of one document: which document is shown, where the cursor is and how it is scrolled.
///
/// Several windows may show the same document, each with its own cursor and scroll.
pub struct Window {
    /// The shown document.
    doc: DocId,
    /// Cursor(s) of this window.
    selection: Selection,
    /// Index of the first visible line.
    scroll: usize,
    /// Number of visible lines, set by the UI; 0 until the first frame (no scrolling then).
    height: usize,
    /// Column that vertical movements (`j`, `k`) aim for; `None` until the first one.
    desired_col: Option<usize>,
}

impl Window {
    /// Creates a window showing `doc`, with the cursor at the start and no scroll.
    pub fn new(doc: DocId) -> Self {
        Self {
            doc,
            selection: Selection::point(0),
            scroll: 0,
            height: 0,
            desired_col: None,
        }
    }

    /// The document shown in this window.
    pub fn doc(&self) -> DocId {
        self.doc
    }
    /// Cursor position: char index of the head of the primary range.
    pub fn cursor(&self) -> usize {
        self.selection.primary().head
    }
    /// Index of the first visible line.
    pub fn scroll(&self) -> usize {
        self.scroll
    }

    /// Places a single cursor at `pos` and sets the sticky column.
    fn set_cursor(&mut self, pos: usize, desired_col: Option<usize>) {
        self.selection = Selection::point(pos);
        self.desired_col = desired_col;
    }

    /// Replaces the selection, e.g. with the one a transaction asked for. Resets the sticky
    /// column and scrolls so that the cursor stays visible.
    pub(crate) fn set_selection(&mut self, text: &Rope, selection: Selection) {
        self.selection = selection;
        self.desired_col = None;
        self.scroll_to_line(text.char_to_line(self.cursor()));
    }

    /// Moves the selection through `changes`, so that it stays on the same text after an edit.
    pub(crate) fn map_selection(&mut self, text: &Rope, changes: &ChangeSet) {
        self.selection.map(changes);
        self.scroll_to_line(text.char_to_line(self.cursor()));
    }

    /// Sets the number of visible lines, used to keep the cursor in view when it moves.
    pub fn set_height(&mut self, height: usize) {
        self.height = height;
    }
    /// Moves the cursor by `motion` within `text` (the text of this window's document) and
    /// scrolls so that the cursor stays visible.
    ///
    /// With `past_end` (Insert, see `Mode::cursor_past_end`) the cursor may go after the last
    /// char of a line; without it (Normal) it stays on a char.
    ///
    /// Vertical motions keep the sticky column: moving through a shorter line and back
    /// returns to the original column. Horizontal motions reset it.
    pub fn move_cursor(&mut self, text: &Rope, motion: Motion, past_end: bool) {
        match motion {
            Motion::Left => self.set_cursor(left(text, self.cursor()), None),
            Motion::Right => self.set_cursor(right(text, self.cursor(), past_end), None),
            Motion::Down => {
                let col = self.desired_col.unwrap_or(column(text, self.cursor()));
                let new_pos = down(text, self.cursor(), col, past_end);
                self.set_cursor(new_pos, Some(col));
                self.scroll_to_line(text.char_to_line(new_pos));
            }
            Motion::Up => {
                let col = self.desired_col.unwrap_or(column(text, self.cursor()));
                let new_pos = up(text, self.cursor(), col, past_end);
                self.set_cursor(new_pos, Some(col));
                self.scroll_to_line(text.char_to_line(new_pos));
            }
        }
    }
    /// Scrolls the minimum needed for line `cursor_line` to be visible. Does nothing while
    /// the height is unknown (0).
    fn scroll_to_line(&mut self, cursor_line: usize) {
        if self.height == 0 {
            return;
        }
        if cursor_line < self.scroll {
            self.scroll = cursor_line;
        } else if cursor_line >= self.scroll + self.height {
            self.scroll = cursor_line + 1 - self.height;
        }
    }

    /// Moves the cursor to the nearest valid position in `text` ([`clamp_cursor`]); with
    /// `past_end` it may stand after the last char of a line (Insert). Keeps the sticky column.
    /// Collapses the selection to a single cursor.
    pub(crate) fn clamp_cursor(&mut self, text: &Rope, past_end: bool) {
        self.set_cursor(
            clamp_cursor(text, self.cursor(), past_end),
            self.desired_col,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use slotmap::KeyData;

    fn win() -> Window {
        Window::new(DocId::from(KeyData::from_ffi(1)))
    }

    #[test]
    fn moving_down_past_view_scrolls() {
        let text = Rope::from_str(&"line\n".repeat(50));
        let mut w = win();
        w.set_height(10);
        for _ in 0..30 {
            w.move_cursor(&text, Motion::Down, false);
        }
        assert_eq!(w.scroll(), 21);
    }

    #[test]
    fn sticky_column_survives_short_line() {
        let text = Rope::from_str("abcdef\nab\nabcdef\n");
        let mut w = win();
        w.set_height(10);
        for _ in 0..4 {
            w.move_cursor(&text, Motion::Right, false);
        } // column 4
        w.move_cursor(&text, Motion::Down, false); // "ab" → column 1
        w.move_cursor(&text, Motion::Down, false); // back to column 4
        assert_eq!(w.cursor(), 14);
    }

    #[test]
    fn new_window_starts_at_top() {
        let w = win();
        assert_eq!((w.cursor(), w.scroll(), w.desired_col), (0, 0, None));
    }

    #[test]
    fn scroll_down_when_cursor_below_view() {
        let mut w = win();
        w.set_height(10);
        w.scroll_to_line(30);
        assert_eq!(w.scroll(), 21); // line 30 is the last visible one: 21..=30
    }

    #[test]
    fn scroll_up_when_cursor_above_view() {
        let mut w = win();
        w.set_height(10);
        w.scroll_to_line(30);
        w.scroll_to_line(5);
        assert_eq!(w.scroll(), 5);
    }

    #[test]
    fn no_scroll_when_cursor_visible() {
        let mut w = win();
        w.set_height(10);
        w.scroll_to_line(30);
        w.scroll_to_line(25);
        assert_eq!(w.scroll(), 21);
    }

    #[test]
    fn zero_height_does_not_panic() {
        let mut w = win();
        w.set_height(0);
        w.scroll_to_line(3); // e.g. a tiny terminal
    }

    #[test]
    fn scrolling_uses_lines_not_char_positions() {
        // 50 lines of 80 chars: char positions grow 81 per line
        let text = Rope::from_str(&format!("{}\n", "a".repeat(80)).repeat(50));
        let mut w = win();
        w.set_height(10);
        for _ in 0..15 {
            w.move_cursor(&text, Motion::Down, false);
        }
        assert_eq!(w.scroll(), 6); // line 15 is the last visible one: 6..=15
    }

    #[test]
    fn moving_up_above_view_scrolls_back() {
        let text = Rope::from_str(&"line\n".repeat(50));
        let mut w = win();
        w.set_height(10);
        for _ in 0..30 {
            w.move_cursor(&text, Motion::Down, false);
        }
        for _ in 0..30 {
            w.move_cursor(&text, Motion::Up, false);
        }
        assert_eq!((w.cursor(), w.scroll()), (0, 0));
    }

    #[test]
    fn horizontal_move_resets_sticky_column() {
        // a0..f5 \n6 | a7 b8 \n9 | a10 ..
        let text = Rope::from_str("abcdef\nab\nabcdef\n");
        let mut w = win();
        w.set_height(10);
        for _ in 0..4 {
            w.move_cursor(&text, Motion::Right, false);
        }
        w.move_cursor(&text, Motion::Down, false); // 'b' in "ab", sticky column 4
        w.move_cursor(&text, Motion::Left, false); // 'a', sticky column reset
        assert_eq!(w.desired_col, None);
        w.move_cursor(&text, Motion::Down, false); // column 0, not 4
        assert_eq!(w.cursor(), 10);
    }

    #[test]
    fn unknown_height_does_not_scroll() {
        let text = Rope::from_str(&"line\n".repeat(20));
        let mut w = win();
        for _ in 0..15 {
            w.move_cursor(&text, Motion::Down, false);
        }
        assert_eq!(w.scroll(), 0);
    }

    #[test]
    fn set_selection_resets_the_sticky_column_and_scrolls() {
        let text = Rope::from_str(&"line\n".repeat(50));
        let mut w = win();
        w.set_height(10);
        w.move_cursor(&text, Motion::Down, false); // sticky column set
        w.set_selection(&text, Selection::point(5 * 30)); // line 30
        assert_eq!((w.cursor(), w.desired_col, w.scroll()), (150, None, 21));
    }

    #[test]
    fn map_selection_follows_the_text() {
        let mut text = Rope::from_str("ab\n");
        let mut w = win();
        w.move_cursor(&text, Motion::Right, false); // 'b'
        let changes = ChangeSet::replace(3, 0..0, "xy");
        changes.apply(&mut text);
        w.map_selection(&text, &changes);
        assert_eq!(w.cursor(), 3); // still 'b'
    }

    #[test]
    fn clamp_cursor_depends_on_past_end() {
        let text = Rope::from_str("ab\n");
        let mut w = win();
        w.set_selection(&text, Selection::point(2)); // after 'b'
        w.clamp_cursor(&text, true);
        assert_eq!(w.cursor(), 2);
        w.clamp_cursor(&text, false);
        assert_eq!(w.cursor(), 1);
    }

    #[test]
    fn past_end_lets_the_cursor_go_after_the_last_char() {
        // a0 b1 \n2 | c3 d4 e5 \n6
        let text = Rope::from_str("ab\ncde\n");
        let mut w = win();
        w.move_cursor(&text, Motion::Right, true);
        w.move_cursor(&text, Motion::Right, true);
        assert_eq!(w.cursor(), 2); // after 'b'
        w.move_cursor(&text, Motion::Down, true);
        assert_eq!(w.cursor(), 5); // column 2: on 'e'
        w.move_cursor(&text, Motion::Right, true);
        assert_eq!(w.cursor(), 6); // after 'e'
        w.move_cursor(&text, Motion::Up, true);
        assert_eq!(w.cursor(), 2); // column 3 does not fit "ab": after 'b'
    }
}
