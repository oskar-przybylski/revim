//! Vim motions and commands.

use revim_api::{ChangeSet, Ctx, Mode, Module, Motion, Registrar, Selection, Transaction};

/// Vim-style editing: `h` `j` `k` `l` and the arrow keys move the cursor (arrows also in
/// Insert), `i` enters Insert, `<Esc>` goes back to Normal.
///
/// Commands: `vim.move.left`, `vim.move.right`, `vim.move.up`, `vim.move.down`,
/// `vim.mode.insert`, `vim.mode.normal`. Typing and deleting text in Insert is done by the
/// `editor.*` commands of `mod-builtin`.
pub struct Vim;

impl Module for Vim {
    fn id(&self) -> &'static str {
        "vim"
    }
    fn register(&mut self, r: &mut Registrar) {
        r.command("vim.move.left", "Move cursor left", |cx, _| {
            cx.move_cursor(Motion::Left)
        });
        r.command("vim.move.right", "Move cursor right", |cx, _| {
            cx.move_cursor(Motion::Right)
        });
        r.command("vim.move.up", "Move cursor up", |cx, _| {
            cx.move_cursor(Motion::Up)
        });
        r.command("vim.move.down", "Move cursor down", |cx, _| {
            cx.move_cursor(Motion::Down)
        });

        r.command("vim.mode.normal", "Back to Normal mode", Vim::normal);
        r.command("vim.mode.append", "Append after the cursor", Vim::append);
        r.command("vim.mode.insert", "Insert before the cursor", |cx, _| {
            cx.set_mode(Mode::Insert)
        });

        r.command("vim.move.delete", "Delete below the cursor", Vim::delete);

        r.keymap(Mode::Normal, "h", "vim.move.left", "");
        r.keymap(Mode::Normal, "l", "vim.move.right", "");
        r.keymap(Mode::Normal, "k", "vim.move.up", "");
        r.keymap(Mode::Normal, "j", "vim.move.down", "");

        r.keymap(Mode::Normal, "<Left>", "vim.move.left", "");
        r.keymap(Mode::Normal, "<Right>", "vim.move.right", "");
        r.keymap(Mode::Normal, "<Up>", "vim.move.up", "");
        r.keymap(Mode::Normal, "<Down>", "vim.move.down", "");

        r.keymap(Mode::Insert, "<Left>", "vim.move.left", "");
        r.keymap(Mode::Insert, "<Right>", "vim.move.right", "");
        r.keymap(Mode::Insert, "<Up>", "vim.move.up", "");
        r.keymap(Mode::Insert, "<Down>", "vim.move.down", "");

        r.keymap(Mode::Normal, "x", "vim.move.delete", "");

        r.keymap(Mode::Normal, "i", "vim.mode.insert", "");
        r.keymap(Mode::Normal, "a", "vim.mode.append", "");
        r.keymap(Mode::Insert, "<Esc>", "vim.mode.normal", "");
    }
}

impl Vim {
    /// `<Esc>`: like in Vim the cursor goes one char left (not past the line start), so that
    /// it stands on the last typed char.
    fn normal(cx: &mut Ctx, _args: &str) {
        if cx.state().mode() == Mode::Insert {
            // still in Insert: the cursor may be after the last char, `Left` puts it on it
            cx.move_cursor(Motion::Left);
        }
        cx.set_mode(Mode::Normal);
    }
    fn delete(cx: &mut Ctx, n: &str) {
        let n = match n {
            "" => 1,
            n => match n.parse::<usize>() {
                Ok(n) => n,
                Err(_) => return,
            },
        };
        let pos = cx.state().focused_window().cursor();
        let n = n.min(pos);
        if n == 0 {
            return;
        }
        let len = cx.state().focused_doc().text().len_chars();
        let tx = Transaction::new(ChangeSet::replace(len, pos..pos + n, ""))
            .with_selection(Selection::point(pos));
        cx.apply(tx);
    }
    fn append(cx: &mut Ctx, _args: &str) {
        cx.set_mode(Mode::Insert);
        cx.move_cursor(Motion::Right);
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

    #[test]
    fn i_enters_insert_and_esc_leaves_it() {
        let mut ed = editor("ab\n", vec![Box::new(Vim)]);
        assert!(press(&mut ed, "i"));
        assert_eq!(ed.state().mode(), Mode::Insert);
        assert!(press(&mut ed, "<Esc>"));
        assert_eq!(ed.state().mode(), Mode::Normal);
    }

    #[test]
    fn esc_moves_the_cursor_one_char_left() {
        let mut ed = editor("abc\n", vec![Box::new(Vim)]);
        press(&mut ed, "l");
        press(&mut ed, "l"); // on 'c'
        press(&mut ed, "i");
        press(&mut ed, "<Esc>");
        assert_eq!(cursor(&ed), 1); // on 'b'
    }

    #[test]
    fn esc_at_the_line_start_stays() {
        // a0 \n1 | b2
        let mut ed = editor("a\nb\n", vec![Box::new(Vim)]);
        press(&mut ed, "j");
        press(&mut ed, "i");
        press(&mut ed, "<Esc>");
        assert_eq!(cursor(&ed), 2);
    }

    #[test]
    fn arrows_move_in_insert() {
        let mut ed = editor("ab\ncd\n", vec![Box::new(Vim)]);
        press(&mut ed, "i");
        for (key, expected) in [("<Right>", 1), ("<Down>", 4), ("<Left>", 3), ("<Up>", 0)] {
            assert!(press(&mut ed, key), "{key}");
            assert_eq!(cursor(&ed), expected, "{key}");
        }
        assert_eq!(ed.state().mode(), Mode::Insert);
    }

    #[test]
    fn right_arrow_in_insert_goes_after_the_last_char() {
        let mut ed = editor("ab\n", vec![Box::new(Vim)]);
        press(&mut ed, "i");
        press(&mut ed, "<Right>");
        press(&mut ed, "<Right>");
        press(&mut ed, "<Right>");
        assert_eq!(cursor(&ed), 2); // after 'b', not on the next line
        press(&mut ed, "<Esc>");
        assert_eq!(cursor(&ed), 1); // back on 'b'
    }

    #[test]
    fn l_in_normal_stops_on_the_last_char() {
        let mut ed = editor("ab\n", vec![Box::new(Vim)]);
        press(&mut ed, "l");
        press(&mut ed, "l");
        assert_eq!(cursor(&ed), 1);
    }

    #[test]
    fn hjkl_do_not_move_in_insert() {
        let mut ed = editor("ab\ncd\n", vec![Box::new(Vim)]);
        press(&mut ed, "i");
        assert!(!press(&mut ed, "l")); // typed text, handled by mod-builtin's fallback
        assert_eq!(cursor(&ed), 0);
    }
}
