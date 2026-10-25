//! Gutter columns: line numbers and a line marking the extent of the document.

use revim_api::{GutterCell, GutterContext, GutterProvider, Highlight, Module, Registrar};

/// Adds two gutter columns: line numbers, then a line from the first to the last line of
/// the document (`┓`, `┃`, `┛`; hidden when the document has one line).
///
/// Numbers are 1-based and right-aligned in a column at least 3 cells wide, wider when the
/// largest number needs it; the cursor's line is highlighted.
pub struct Gutter;

impl Module for Gutter {
    fn id(&self) -> &'static str {
        "gutter"
    }
    fn register(&mut self, r: &mut Registrar) {
        r.gutter(LineNumberGutter);
        r.gutter(PipeGutter);
    }
}

/// Line numbers, right-aligned; the cursor's line uses [`Highlight::CurrentLineNumber`].
struct LineNumberGutter;
impl GutterProvider for LineNumberGutter {
    fn width(&self, cx: &GutterContext) -> u16 {
        ((cx.doc.line_count().ilog10() + 1) as u16).max(3)
    }
    fn cell(&self, cx: &GutterContext, line: usize) -> GutterCell {
        let w = self.width(cx) as usize;
        let text = format!("{:>w$}", line + 1);

        let current_line = cx.doc.text().char_to_line(cx.window.cursor());
        let highlight = if current_line == line {
            Highlight::CurrentLineNumber
        } else {
            Highlight::LineNumber
        };

        GutterCell { text, highlight }
    }
}

/// A line next to the document: `┓` on the first line, `┃` in between, `┛` on the last.
struct PipeGutter;
impl GutterProvider for PipeGutter {
    fn width(&self, cx: &GutterContext) -> u16 {
        if cx.doc.line_count() > 1 { 1 } else { 0 }
    }
    fn cell(&self, cx: &GutterContext, line: usize) -> GutterCell {
        let is_last = cx.doc.line_count() - 1 == line;
        let is_first = line == 0;
        let text = if is_first {
            "┓"
        } else if is_last {
            "┛"
        } else {
            "┃"
        }
        .to_string();

        GutterCell {
            text,
            highlight: Highlight::Dimmed,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use revim_api::Motion;
    use revim_api::testing::{Editor, editor};

    /// `n` lines: "1\n2\n...".
    fn lines(n: usize) -> String {
        (1..=n).map(|i| format!("{i}\n")).collect()
    }

    /// Texts of the number column and the pipe column, for a window `height` rows high.
    fn gutter(ed: &mut Editor, height: usize) -> (Vec<String>, Vec<String>) {
        let columns = ed.gutter_columns(height);
        assert_eq!(columns.len(), 2);
        let text = |i: usize| columns[i].cells.iter().map(|c| c.text.clone()).collect();
        (text(0), text(1))
    }

    #[test]
    fn numbers_are_one_based_and_right_aligned() {
        let mut ed = editor(&lines(10), vec![Box::new(Gutter)]);
        let (numbers, _) = gutter(&mut ed, 10);
        assert_eq!(numbers[0], "  1");
        assert_eq!(numbers[8], "  9");
        assert_eq!(numbers[9], " 10");
        assert_eq!(ed.gutter_columns(10)[0].width, 3);
    }

    #[test]
    fn number_width_follows_the_line_count() {
        // at least 3 wide, then one cell per digit
        for (text, width) in [(String::new(), 3), (lines(999), 3), (lines(1000), 4)] {
            let mut ed = editor(&text, vec![Box::new(Gutter)]);
            assert_eq!(
                ed.gutter_columns(1)[0].width,
                width,
                "{} lines",
                text.lines().count()
            );
        }
    }

    #[test]
    fn cursor_line_is_highlighted() {
        let mut ed = editor(&lines(3), vec![Box::new(Gutter)]);
        ed.state_mut().move_cursor(Motion::Down);
        let highlights: Vec<Highlight> = ed.gutter_columns(3)[0]
            .cells
            .iter()
            .map(|c| c.highlight)
            .collect();
        assert_eq!(
            highlights,
            [
                Highlight::LineNumber,
                Highlight::CurrentLineNumber,
                Highlight::LineNumber
            ]
        );
    }

    #[test]
    fn numbers_follow_the_scroll_position() {
        let mut ed = editor(&lines(5), vec![Box::new(Gutter)]);
        ed.state_mut().set_view_height(2);
        for _ in 0..4 {
            ed.state_mut().move_cursor(Motion::Down); // last line, scrolled by 3
        }
        let (numbers, _) = gutter(&mut ed, 2);
        assert_eq!(numbers, ["  4", "  5"]);
    }

    #[test]
    fn pipe_marks_first_middle_and_last_line() {
        let mut ed = editor(&lines(3), vec![Box::new(Gutter)]);
        let (_, pipe) = gutter(&mut ed, 3);
        assert_eq!(pipe, ["┓", "┃", "┛"]);
    }

    #[test]
    fn pipe_is_dimmed() {
        let mut ed = editor(&lines(3), vec![Box::new(Gutter)]);
        let columns = ed.gutter_columns(3);
        assert!(
            columns[1]
                .cells
                .iter()
                .all(|c| c.highlight == Highlight::Dimmed)
        );
    }

    #[test]
    fn numbers_are_never_cut_off() {
        // 7 digits: wider than any fixed cap would allow, so the column grows with them
        let mut ed = editor(&lines(1_000_000), vec![Box::new(Gutter)]);
        ed.state_mut().set_view_height(1);
        let column = &ed.gutter_columns(1)[0];
        assert_eq!(column.width, 7);
        assert_eq!(column.cells[0].text, "      1");
    }

    #[test]
    fn pipe_is_hidden_on_a_single_line_document() {
        let mut ed = editor("only\n", vec![Box::new(Gutter)]);
        let columns = ed.gutter_columns(3);
        assert_eq!(columns.len(), 1); // only the numbers
        assert_eq!(columns[0].cells[0].text, "  1");
    }
}
