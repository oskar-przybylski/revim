//! Cursor movements in Normal mode.
//!
//! Pure functions: they take the text and the current cursor position (a char index in the
//! whole text) and return the new position. They never change the text.
//!
//! Lines are split on `\n` only; a trailing `\r` is treated as part of a `\r\n` line ending.
//! In Normal mode the cursor stands *on* a char, so it never lands on a line ending.
//! Columns are counted in chars.
//!
//! ```
//! use revim_core::movement::{column, down, right};
//! use ropey::Rope;
//!
//! let text = Rope::from_str("fn main() {\n    run();\n}\n");
//! let pos = right(&text, 0, false);            // 'n'
//! let col = column(&text, pos);         // 1
//! assert_eq!(down(&text, pos, col, false), 13); // second line, column 1
//! ```

use ropey::{Rope, RopeSlice};

/// A cursor movement, independent of the key that triggered it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Motion {
    /// One char left (`h`), see [`left`].
    Left,
    /// One char right (`l`), see [`right`].
    Right,
    /// One line up (`k`), see [`up`].
    Up,
    /// One line down (`j`), see [`down`].
    Down,
}
/// `h`: moves one char left; does not wrap to the previous line.
///
/// ```
/// use revim_core::movement::left;
/// use ropey::Rope;
///
/// let text = Rope::from_str("abc\nde");
/// assert_eq!(left(&text, 5), 4); // 'e' → 'd'
/// assert_eq!(left(&text, 4), 4); // 'd' is the line start → stays
/// ```
pub fn left(text: &Rope, pos: usize) -> usize {
    if pos > 0
        && let Some(char_before) = text.get_char(pos - 1)
        && char_before != '\n'
    {
        pos - 1
    } else {
        pos
    }
}

/// `l`: moves one char right, never onto the line ending (`\n` or `\r\n`).
///
/// Without `past_end` (Normal) it stops on the last char of the line; with `past_end`
/// (Insert) it may go one further, after the last char, where typed text is appended. On an
/// empty line the position does not change in Normal.
///
/// ```
/// use revim_core::movement::right;
/// use ropey::Rope;
///
/// let text = Rope::from_str("abc\nde");
/// assert_eq!(right(&text, 0, false), 1);
/// assert_eq!(right(&text, 2, false), 2); // Normal: 'c' is the last char of the line → stays
/// assert_eq!(right(&text, 2, true), 3); // Insert: after 'c'
/// assert_eq!(right(&text, 3, true), 3); // and no further
/// ```
pub fn right(text: &Rope, pos: usize, past_end: bool) -> usize {
    let line = text.char_to_line(pos);
    let last = text.line_to_char(line) + last_col(line_content_len(text.line(line)), past_end);
    if pos < last { pos + 1 } else { pos }
}

/// `j`: moves one line down to column `col`, or to the last valid column if the line is
/// shorter: its last char without `past_end` (Normal), after its last char with `past_end`
/// (Insert).
///
/// `col` is the column to aim for (Vim's "sticky" column): pass the column the vertical
/// movement started from, so that moving through short lines does not lose it.
/// On the last line the position does not change. A trailing line ending does not start
/// a new line (`"abc\n"` has one line, as in Vim).
///
/// ```
/// use revim_core::movement::down;
/// use ropey::Rope;
///
/// let text = Rope::from_str("abc\nde\nfghij\n");
/// assert_eq!(down(&text, 2, 2, false), 5);  // "de" is shorter → its last char 'e'
/// assert_eq!(down(&text, 5, 2, false), 9);  // back to column 2 → 'h'
/// assert_eq!(down(&text, 9, 2, false), 9);  // last line → stays
/// assert_eq!(down(&text, 3, 3, true), 6);   // Insert: after "de"
/// ```
pub fn down(text: &Rope, pos: usize, col: usize, past_end: bool) -> usize {
    let line_idx = text.char_to_line(pos);
    let next_idx = line_idx + 1;
    if next_idx >= line_count(text) {
        return pos;
    }
    pos_in_line(text, next_idx, col, past_end)
}

/// `k`: moves one line up to column `col`, or to the last valid column if the line is
/// shorter (see [`down`] for `col` and `past_end`). On the first line the position does not
/// change.
///
/// ```
/// use revim_core::movement::up;
/// use ropey::Rope;
///
/// let text = Rope::from_str("ab\nabcdef");
/// assert_eq!(up(&text, 8, 5, false), 1); // "ab" is shorter → its last char 'b'
/// assert_eq!(up(&text, 1, 1, false), 1); // first line → stays
/// assert_eq!(up(&text, 8, 5, true), 2);  // Insert: after "ab"
/// ```
pub fn up(text: &Rope, pos: usize, col: usize, past_end: bool) -> usize {
    let line_idx = text.char_to_line(pos);
    let Some(prev_idx) = line_idx.checked_sub(1) else {
        return pos;
    };
    pos_in_line(text, prev_idx, col, past_end)
}

/// Number of chars in `line` without its line ending (`\n` or `\r\n`).
fn line_content_len(line: RopeSlice) -> usize {
    let mut n = line.len_chars();
    if n > 0 && line.char(n - 1) == '\n' {
        n -= 1;
        if n > 0 && line.char(n - 1) == '\r' {
            n -= 1;
        }
    }
    n
}

/// Last column the cursor may stand on in a line of `len` chars (without the line ending):
/// after the last char with `past_end` (Insert), on it without (Normal; 0 on an empty line).
fn last_col(len: usize, past_end: bool) -> usize {
    if past_end { len } else { len.saturating_sub(1) }
}

/// Position in line `line_idx` at column `col`, clamped to the last valid column
/// ([`last_col`]).
fn pos_in_line(text: &Rope, line_idx: usize, col: usize, past_end: bool) -> usize {
    let start = text.line_to_char(line_idx);
    let len = line_content_len(text.line(line_idx));
    start + col.min(last_col(len, past_end))
}

/// Number of lines as Vim counts them: a trailing line ending does not start a new line.
///
/// ```
/// use revim_core::movement::line_count;
/// use ropey::Rope;
///
/// assert_eq!(line_count(&Rope::from_str("a\nb\n")), 2);
/// assert_eq!(line_count(&Rope::from_str("a\nb")), 2);
/// assert_eq!(line_count(&Rope::from_str("")), 1); // an empty document has one empty line
/// ```
pub fn line_count(text: &Rope) -> usize {
    let n = text.len_lines();
    if n > 1 && text.char(text.len_chars() - 1) == '\n' {
        n - 1
    } else {
        n
    }
}

/// Column of `pos` within its line, counted in chars from the line start.
///
/// ```
/// use revim_core::movement::column;
/// use ropey::Rope;
///
/// let text = Rope::from_str("abc\nde");
/// assert_eq!(column(&text, 5), 1); // 'e'
/// ```
pub fn column(text: &Rope, pos: usize) -> usize {
    let line_idx = text.char_to_line(pos);
    pos - text.line_to_char(line_idx)
}

/// Nearest valid cursor position to `pos`, on the same line when possible.
///
/// With `past_end` (Insert) the cursor may stand after the last char of a line; without it
/// (Normal) it stands on a char, except on an empty line. Never on the line ending (`\n` or
/// `\r\n`) and never on the line after a trailing line ending, which is not a line (see
/// [`line_count`]). A `pos` past the end of the text is treated as the end of the text.
///
/// ```
/// use revim_core::movement::clamp_cursor;
/// use ropey::Rope;
///
/// let text = Rope::from_str("abc\n");
/// assert_eq!(clamp_cursor(&text, 3, false), 2); // Normal: from '\n' onto 'c'
/// assert_eq!(clamp_cursor(&text, 3, true), 3); // Insert: after 'c' is fine
/// assert_eq!(clamp_cursor(&text, 4, true), 3); // not on the line after the trailing '\n'
/// ```
pub fn clamp_cursor(text: &Rope, pos: usize, past_end: bool) -> usize {
    let pos = pos.min(text.len_chars());
    let line = text.char_to_line(pos).min(line_count(text) - 1);
    let start = text.line_to_char(line);
    let len = line_content_len(text.line(line));
    start + (pos - start).min(last_col(len, past_end))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn text() -> Rope {
        Rope::from_str("abc\nde\n\nfghij")
    }

    #[test]
    fn left_stops_at_line_start() {
        assert_eq!(left(&text(), 5), 4);
        assert_eq!(left(&text(), 4), 4);
        assert_eq!(left(&text(), 0), 0);
    }

    #[test]
    fn right_stops_on_last_char_not_newline() {
        assert_eq!(right(&text(), 1, false), 2);
        assert_eq!(right(&text(), 2, false), 2);
        assert_eq!(right(&text(), 7, false), 7);
        assert_eq!(right(&text(), 12, false), 12);
    }

    #[test]
    fn down_clamps_to_shorter_lines_and_keeps_column() {
        assert_eq!(down(&text(), 2, 2, false), 5);
        assert_eq!(down(&text(), 5, 2, false), 7);
        assert_eq!(down(&text(), 7, 2, false), 10);
        assert_eq!(down(&text(), 10, 2, false), 10);
    }

    #[test]
    fn up_mirrors_down() {
        assert_eq!(up(&text(), 10, 2, false), 7);
        assert_eq!(up(&text(), 1, 1, false), 1);
    }

    #[test]
    fn up_clamps_to_shorter_line() {
        let r = Rope::from_str("ab\nabcdef\n");
        let pos = r.line_to_char(1) + 5;
        assert_eq!(up(&r, pos, 5, false), 1);
    }

    #[test]
    fn up_on_first_line_stays() {
        let r = Rope::from_str("abc\n");
        assert_eq!(up(&r, 2, 2, false), 2);
    }

    #[test]
    fn up_to_empty_line() {
        let r = Rope::from_str("\nabc\n");
        assert_eq!(up(&r, 3, 2, false), 0);
    }

    #[test]
    fn up_handles_crlf() {
        let r = Rope::from_str("ab\r\nabcd\r\n");
        let pos = r.line_to_char(1) + 3;
        assert_eq!(up(&r, pos, 3, false), 1);
    }

    #[test]
    fn column_counts_from_line_start() {
        assert_eq!(column(&text(), 5), 1);
        assert_eq!(column(&text(), 10), 2);
    }

    #[test]
    fn right_moves_from_line_start() {
        assert_eq!(right(&text(), 0, false), 1);
        assert_eq!(right(&text(), 4, false), 5);
    }

    #[test]
    fn right_does_not_step_onto_carriage_return() {
        // a0 b1 \r2 \n3 | c4 d5 \r6 \n7
        let r = Rope::from_str("ab\r\ncd\r\n");
        assert_eq!(right(&r, 0, false), 1);
        assert_eq!(right(&r, 1, false), 1);
    }

    #[test]
    fn right_on_empty_crlf_line_stays() {
        // a0 \r1 \n2 | \r3 \n4 | b5
        let r = Rope::from_str("a\r\n\r\nb");
        assert_eq!(right(&r, 3, false), 3);
    }

    #[test]
    fn down_does_not_enter_line_after_trailing_newline() {
        let r = Rope::from_str("abc\n");
        assert_eq!(down(&r, 1, 1, false), 1);
    }

    #[test]
    fn down_enters_trailing_blank_line() {
        // "abc\n\n" is two lines in Vim: "abc" and an empty one
        let r = Rope::from_str("abc\n\n");
        assert_eq!(down(&r, 1, 1, false), 4);
    }

    #[test]
    fn single_newline_is_one_empty_line() {
        let r = Rope::from_str("\n");
        assert_eq!(
            (
                left(&r, 0),
                right(&r, 0, false),
                up(&r, 0, 0, false),
                down(&r, 0, 0, false)
            ),
            (0, 0, 0, 0)
        );
    }

    #[test]
    fn down_and_up_skip_carriage_return_when_clamping() {
        // a0 b1 c2 d3 \r4 \n5 | a6 b7 \r8 \n9 | a10 b11 c12 d13 \r14 \n15
        let r = Rope::from_str("abcd\r\nab\r\nabcd\r\n");
        assert_eq!(down(&r, 3, 3, false), 7); // 'b', not '\r'
        assert_eq!(down(&r, 7, 3, false), 13); // back to column 3
        assert_eq!(up(&r, 13, 3, false), 7);
        assert_eq!(column(&r, 13), 3);
    }

    #[test]
    fn down_from_empty_crlf_line() {
        // a0 \r1 \n2 | \r3 \n4 | b5
        let r = Rope::from_str("a\r\n\r\nb");
        assert_eq!(down(&r, 0, 0, false), 3);
        assert_eq!(down(&r, 3, 0, false), 5);
    }

    #[test]
    fn movement_counts_chars_not_bytes() {
        // α0 β1 γ2 δ3 \n4 | a5 b6 — each Greek letter is 2 bytes in UTF-8
        let r = Rope::from_str("αβγδ\nab");
        assert_eq!(right(&r, 0, false), 1);
        assert_eq!(right(&r, 3, false), 3);
        assert_eq!(left(&r, 5), 5);
        assert_eq!(down(&r, 3, 3, false), 6);
        assert_eq!(column(&r, 6), 1);
    }

    // Vim (fileformat=unix) splits lines only on '\n'. Other chars must stay inside the line.
    #[test]
    fn only_newline_breaks_lines() {
        // a0 \x0C1 b2 \n3 | c4  — form feed is not a line break
        assert_eq!(down(&Rope::from_str("a\x0Cb\nc"), 0, 0, false), 4);
        // a0 \r1 b2 \n3 | c4  — a lone '\r' is not a line break
        assert_eq!(down(&Rope::from_str("a\rb\nc"), 0, 0, false), 4);
    }

    #[test]
    fn empty_text_stays_at_zero() {
        let t = Rope::from_str("");
        assert_eq!(
            (
                left(&t, 0),
                right(&t, 0, false),
                up(&t, 0, 0, false),
                down(&t, 0, 0, false)
            ),
            (0, 0, 0, 0)
        );
    }

    #[test]
    fn clamp_keeps_valid_positions() {
        // a0 b1 c2 \n3 | d4 e5 \n6 | \n7 | f8 g9 h10 i11 j12
        for pos in [0, 2, 4, 5, 7, 8, 12] {
            assert_eq!(clamp_cursor(&text(), pos, false), pos, "{pos}");
            assert_eq!(clamp_cursor(&text(), pos, true), pos, "{pos}");
        }
    }

    #[test]
    fn clamp_moves_off_the_line_ending_in_normal() {
        assert_eq!(clamp_cursor(&text(), 3, false), 2);
        assert_eq!(clamp_cursor(&text(), 6, false), 5);
    }

    #[test]
    fn clamp_allows_after_the_last_char_in_insert() {
        assert_eq!(clamp_cursor(&text(), 3, true), 3);
        assert_eq!(clamp_cursor(&text(), 6, true), 6);
        assert_eq!(clamp_cursor(&text(), 13, true), 13); // end of text without '\n'
    }

    #[test]
    fn clamp_past_the_end_of_text() {
        assert_eq!(clamp_cursor(&text(), 100, false), 12);
        assert_eq!(clamp_cursor(&text(), 100, true), 13);
    }

    #[test]
    fn clamp_on_empty_line_stays() {
        assert_eq!(clamp_cursor(&text(), 7, false), 7);
        assert_eq!(clamp_cursor(&text(), 7, true), 7);
    }

    #[test]
    fn clamp_never_on_the_line_after_trailing_newline() {
        let text = Rope::from_str("ab\n");
        assert_eq!(clamp_cursor(&text, 3, false), 1);
        assert_eq!(clamp_cursor(&text, 3, true), 2);
    }

    #[test]
    fn clamp_skips_carriage_return() {
        // a0 b1 \r2 \n3
        let text = Rope::from_str("ab\r\n");
        assert_eq!(clamp_cursor(&text, 2, false), 1);
        assert_eq!(clamp_cursor(&text, 3, false), 1);
        assert_eq!(clamp_cursor(&text, 3, true), 2);
    }

    #[test]
    fn clamp_in_empty_text() {
        let text = Rope::new();
        assert_eq!(clamp_cursor(&text, 0, false), 0);
        assert_eq!(clamp_cursor(&text, 5, true), 0);
    }

    #[test]
    fn right_in_insert_goes_after_the_last_char() {
        // a0 b1 c2 \n3 | d4 e5 \n6 | \n7 | f8 g9 h10 i11 j12
        assert_eq!(right(&text(), 2, true), 3);
        assert_eq!(right(&text(), 3, true), 3); // not onto the next line
        assert_eq!(right(&text(), 12, true), 13); // last line without '\n'
        assert_eq!(right(&text(), 13, true), 13);
    }

    #[test]
    fn right_in_insert_on_empty_line_stays() {
        assert_eq!(right(&text(), 7, true), 7);
    }

    #[test]
    fn right_in_insert_does_not_step_between_carriage_return_and_newline() {
        // a0 b1 \r2 \n3
        let r = Rope::from_str("ab\r\n");
        assert_eq!(right(&r, 1, true), 2);
        assert_eq!(right(&r, 2, true), 2);
    }

    #[test]
    fn down_and_up_in_insert_keep_the_column_after_the_last_char() {
        // a0 b1 c2 d3 \n4 | e5 f6 \n7 | g8 h9 i10 j11 \n12
        let r = Rope::from_str("abcd\nef\nghij\n");
        assert_eq!(down(&r, 4, 4, true), 7); // after "ef"
        assert_eq!(down(&r, 7, 4, true), 12); // back to column 4: after "ghij"
        assert_eq!(up(&r, 12, 4, true), 7);
        assert_eq!(down(&r, 4, 4, false), 6); // Normal: on 'f'
    }

    #[test]
    fn down_in_insert_does_not_enter_line_after_trailing_newline() {
        let r = Rope::from_str("ab\n");
        assert_eq!(down(&r, 2, 2, true), 2);
    }
}
