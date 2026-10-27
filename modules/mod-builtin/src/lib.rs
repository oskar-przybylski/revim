//! Built-in editor commands that every configuration needs.

use revim_api::{ChangeSet, Ctx, Mode, Module, Registrar, Selection, Transaction};

/// Core editor commands, independent of the key set (Vim or other):
///
/// | Command | Argument | Keys |
/// |---|---|---|
/// | `editor.quit` | — | `<C-q>` (Normal) |
/// | `editor.change_mode` | `Normal` or `Insert`; anything else does nothing | — |
/// | `editor.insert` | text to insert at the cursor; empty does nothing | every unbound char in Insert, `<CR>` (`"\n"`), `<Tab>` (`"\t"`) |
/// | `editor.delete_backward` | how many chars to delete before the cursor; empty = 1 | `<BS>` (Insert) |
///
/// Entering and leaving modes with keys (`i`, `<Esc>`) is left to the module that defines
/// the key set, e.g. `mod-vim`.
pub struct Builtin;

impl Module for Builtin {
    fn id(&self) -> &'static str {
        "editor"
    }
    fn register(&mut self, r: &mut Registrar) {
        r.command("editor.quit", "Quit the editor", |cx, _| cx.quit());
        r.command(
            "editor.change_mode",
            "Changes mode of editor",
            Builtin::change_mode,
        );
        r.command(
            "editor.insert",
            "Insert text at the cursor",
            Builtin::insert,
        );
        r.command(
            "editor.delete_backward",
            "Delete chars before the cursor",
            Builtin::delete_backward,
        );

        r.keymap(Mode::Normal, "<C-q>", "editor.quit", "");

        r.keymap(Mode::Insert, "<CR>", "editor.insert", "\n");
        r.keymap(Mode::Insert, "<Tab>", "editor.insert", "\t");
        r.keymap(Mode::Insert, "<BS>", "editor.delete_backward", "");
        r.keymap_fallback(Mode::Insert, "editor.insert");
    }
}

impl Builtin {
    /// `editor.change_mode`: switches to the mode named by `mode` (as `Mode` displays it).
    fn change_mode(cx: &mut Ctx, mode: &str) {
        match mode {
            "Insert" => cx.set_mode(Mode::Insert),
            "Normal" => cx.set_mode(Mode::Normal),
            _ => {}
        }
    }
    /// `editor.insert`: inserts `text` at the cursor and puts the cursor after it.
    fn insert(cx: &mut Ctx, text: &str) {
        if text.is_empty() {
            return;
        }
        let pos = cx.state().focused_window().cursor();
        let len = cx.state().focused_doc().text().len_chars();
        let tx = Transaction::new(ChangeSet::replace(len, pos..pos, text))
            .with_selection(Selection::point(pos + text.chars().count()));
        cx.apply(tx);
    }
    /// `editor.delete_backward`: deletes `n` chars before the cursor (1 if `n` is empty), at
    /// most up to the start of the text; a line ending counts as a char, so at a line start
    /// the line is joined with the previous one. Does nothing if `n` is not a number.
    fn delete_backward(cx: &mut Ctx, n: &str) {
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
        let tx = Transaction::new(ChangeSet::replace(len, pos - n..pos, ""))
            .with_selection(Selection::point(pos - n));
        cx.apply(tx);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use revim_api::testing::{Editor, press};
    use revim_api::{Document, EditorState};

    /// Test-only keys: `i` / `<Esc>` switch modes, `v` asks for an unknown mode, `D` deletes
    /// two chars back, `X` passes an invalid count.
    struct Keys;
    impl Module for Keys {
        fn id(&self) -> &'static str {
            "keys"
        }
        fn register(&mut self, r: &mut Registrar) {
            r.keymap(Mode::Normal, "i", "editor.change_mode", "Insert");
            r.keymap(Mode::Normal, "v", "editor.change_mode", "Visual");
            r.keymap(Mode::Insert, "<Esc>", "editor.change_mode", "Normal");
            r.keymap(Mode::Insert, "<C-d>", "editor.delete_backward", "2");
            r.keymap(Mode::Insert, "<C-x>", "editor.delete_backward", "x");
        }
    }

    fn editor_on(text: &str) -> Editor {
        // `testing::editor` adds its own `editor.quit`, so build the editor directly.
        let state = EditorState::with_document(Document::from_text(None, text));
        Editor::new(state, vec![Box::new(Builtin), Box::new(Keys)]).unwrap()
    }

    fn editor() -> Editor {
        editor_on("")
    }

    fn text(ed: &Editor) -> String {
        ed.state().focused_doc().text().to_string()
    }

    fn cursor(ed: &Editor) -> usize {
        ed.state().focused_window().cursor()
    }

    fn type_keys(ed: &mut Editor, keys: &[&str]) {
        for key in keys {
            press(ed, key);
        }
    }

    #[test]
    fn ctrl_q_quits() {
        let mut ed = editor();
        assert!(!ed.state().should_quit());
        assert!(press(&mut ed, "<C-q>"));
        assert!(ed.state().should_quit());
    }

    #[test]
    fn change_mode_switches_modes() {
        let mut ed = editor();
        press(&mut ed, "i");
        assert_eq!(ed.state().mode(), Mode::Insert);
        press(&mut ed, "<Esc>");
        assert_eq!(ed.state().mode(), Mode::Normal);
    }

    #[test]
    fn change_mode_ignores_unknown_modes() {
        let mut ed = editor();
        assert!(press(&mut ed, "v"));
        assert_eq!(ed.state().mode(), Mode::Normal);
    }

    #[test]
    fn typing_in_insert_inserts_chars() {
        let mut ed = editor();
        type_keys(&mut ed, &["i", "a", "ż", "<Space>"]);
        assert_eq!(text(&ed), "aż ");
        assert_eq!(cursor(&ed), 3);
    }

    #[test]
    fn typing_inserts_at_the_cursor() {
        let mut ed = editor_on("ac\n");
        type_keys(&mut ed, &["i", "b"]); // cursor on 'a': inserted before it
        assert_eq!(text(&ed), "bac\n");
        assert_eq!(cursor(&ed), 1);
    }

    #[test]
    fn typing_in_normal_inserts_nothing() {
        let mut ed = editor();
        assert!(!press(&mut ed, "a"));
        assert_eq!(text(&ed), "");
    }

    #[test]
    fn enter_and_tab_insert_line_break_and_tab() {
        let mut ed = editor_on("\n");
        type_keys(&mut ed, &["i", "a", "<CR>", "<Tab>", "b"]);
        assert_eq!(text(&ed), "a\n\tb\n");
        assert_eq!(cursor(&ed), 4);
    }

    #[test]
    fn backspace_deletes_the_char_before_the_cursor() {
        let mut ed = editor();
        type_keys(&mut ed, &["i", "a", "b", "<BS>"]);
        assert_eq!(text(&ed), "a");
        assert_eq!(cursor(&ed), 1);
    }

    #[test]
    fn backspace_at_the_start_does_nothing() {
        let mut ed = editor_on("ab\n");
        type_keys(&mut ed, &["i", "<BS>"]);
        assert_eq!(text(&ed), "ab\n");
        assert_eq!(cursor(&ed), 0);
    }

    #[test]
    fn backspace_at_the_line_start_joins_lines() {
        let mut ed = editor_on("\n");
        type_keys(&mut ed, &["i", "a", "<CR>", "b", "<BS>", "<BS>"]);
        assert_eq!(text(&ed), "a\n");
        assert_eq!(cursor(&ed), 1);
    }

    #[test]
    fn delete_backward_takes_a_count() {
        let mut ed = editor();
        type_keys(&mut ed, &["i", "a", "b", "c", "<C-d>"]);
        assert_eq!(text(&ed), "a");
        type_keys(&mut ed, &["<C-d>"]); // only one char before the cursor
        assert_eq!(text(&ed), "");
    }

    #[test]
    fn delete_backward_ignores_an_invalid_count() {
        let mut ed = editor();
        type_keys(&mut ed, &["i", "a", "<C-x>"]);
        assert_eq!(text(&ed), "a");
        assert!(ed.failures().is_empty());
    }

    #[test]
    fn leaving_insert_puts_the_cursor_on_the_last_char() {
        let mut ed = editor_on("\n");
        type_keys(&mut ed, &["i", "a", "b", "<Esc>"]);
        assert_eq!(cursor(&ed), 1); // on 'b', not after it
    }
}
