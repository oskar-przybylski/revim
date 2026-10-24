//! Vim motions and commands.

use revim_api::{Mode, Module, Motion, Registrar};

/// Vim-style editing: `h` `j` `k` `l` and the arrow keys move the cursor.
///
/// Commands: `vim.move.left`, `vim.move.right`, `vim.move.up`, `vim.move.down`.
pub struct Vim;

impl Module for Vim {
    fn id(&self) -> &'static str {
        "vim"
    }
    fn register(&mut self, r: &mut Registrar) {
        r.command("vim.move.left", "Move cursor left", |cx| {
            cx.move_cursor(Motion::Left)
        });
        r.command("vim.move.right", "Move cursor right", |cx| {
            cx.move_cursor(Motion::Right)
        });
        r.command("vim.move.up", "Move cursor up", |cx| {
            cx.move_cursor(Motion::Up)
        });
        r.command("vim.move.down", "Move cursor down", |cx| {
            cx.move_cursor(Motion::Down)
        });
        r.keymap(Mode::Normal, "h", "vim.move.left");
        r.keymap(Mode::Normal, "l", "vim.move.right");
        r.keymap(Mode::Normal, "k", "vim.move.up");
        r.keymap(Mode::Normal, "j", "vim.move.down");

        r.keymap(Mode::Normal, "<Left>", "vim.move.left");
        r.keymap(Mode::Normal, "<Right>", "vim.move.right");
        r.keymap(Mode::Normal, "<Up>", "vim.move.up");
        r.keymap(Mode::Normal, "<Down>", "vim.move.down");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use revim_api::testing::{Editor, editor, press};

    fn cursor(ed: &Editor) -> usize {
        ed.state().focused_window().cursor()
    }

    #[test]
    fn hjkl_move_the_cursor() {
        // a0 b1 \n2 | c3 d4 \n5
        let mut ed = editor("ab\ncd\n", vec![Box::new(Vim)]);
        press(&mut ed, "l");
        assert_eq!(cursor(&ed), 1);
        press(&mut ed, "j");
        assert_eq!(cursor(&ed), 4);
        press(&mut ed, "h");
        assert_eq!(cursor(&ed), 3);
        press(&mut ed, "k");
        assert_eq!(cursor(&ed), 0);
    }

    #[test]
    fn arrows_work_like_hjkl() {
        let mut ed = editor("ab\ncd\n", vec![Box::new(Vim)]);
        for (key, expected) in [("<Right>", 1), ("<Down>", 4), ("<Left>", 3), ("<Up>", 0)] {
            assert!(press(&mut ed, key), "{key}");
            assert_eq!(cursor(&ed), expected, "{key}");
        }
    }

    #[test]
    fn sticky_column_through_a_short_line() {
        // a0..f5 \n6 | a7 b8 \n9 | a10 b11 c12 d13 e14 f15
        let mut ed = editor("abcdef\nab\nabcdef\n", vec![Box::new(Vim)]);
        for _ in 0..4 {
            press(&mut ed, "l");
        }
        press(&mut ed, "j");
        press(&mut ed, "j");
        assert_eq!(cursor(&ed), 14);
    }
}
