//! Built-in editor commands that every configuration needs.

use revim_api::{Mode, Module, Registrar};

/// Core editor commands: `editor.quit` (`<C-q>`).
pub struct Builtin;

impl Module for Builtin {
    fn id(&self) -> &'static str {
        "editor"
    }
    fn register(&mut self, r: &mut Registrar) {
        r.command("editor.quit", "Quit the editor", |cx| cx.quit());
        r.keymap(Mode::Normal, "<C-q>", "editor.quit");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use revim_api::testing::{Editor, press};
    use revim_api::{Document, EditorState};

    fn editor() -> Editor {
        // `testing::editor` adds its own `editor.quit`, so build the editor directly.
        let state = EditorState::with_document(Document::scratch());
        Editor::new(state, vec![Box::new(Builtin)]).unwrap()
    }

    #[test]
    fn ctrl_q_quits() {
        let mut ed = editor();
        assert!(!ed.state().should_quit());
        assert!(press(&mut ed, "<C-q>"));
        assert!(ed.state().should_quit());
    }
}
