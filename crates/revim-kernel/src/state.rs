//! Editor state: open documents, the windows showing them and which window is focused.

use revim_core::{movement::Motion, transaction::Transaction};
use slotmap::{SlotMap, new_key_type};

use crate::{
    document::Document,
    keymap::Mode,
    window::{Window, WindowId},
};

new_key_type! {
    /// Stable identifier of a document inside [`EditorState`].
    ///
    /// Cheap to copy and safe to keep around (in windows, modules, background tasks) instead
    /// of a reference. Once its document is removed, the id does not point to any other document.
    pub struct DocId;
}

/// The whole state of the editor, owned by the kernel.
///
/// Holds the open documents, the windows showing them and which window is focused.
///
/// An `EditorState` always has a focused window, so there is always a focused document.
/// Windows are changed only through methods of `EditorState`, which pass them the text of
/// their document.
pub struct EditorState {
    docs: SlotMap<DocId, Document>,
    windows: SlotMap<WindowId, Window>,
    mode: Mode,
    focused: WindowId,
    quit_requested: bool,
}

impl EditorState {
    /// Creates the state with one document shown in one window, which becomes focused.
    ///
    /// ```
    /// use revim_kernel::document::Document;
    /// use revim_kernel::state::EditorState;
    ///
    /// let state = EditorState::with_document(Document::from_text(None, "hello"));
    /// assert_eq!(state.focused_doc().text().to_string(), "hello");
    /// ```
    pub fn with_document(doc: Document) -> Self {
        let mut docs = SlotMap::<DocId, Document>::with_key();
        let mut windows = SlotMap::<WindowId, Window>::with_key();
        let focused = windows.insert(Window::new(docs.insert(doc)));
        Self {
            docs,
            windows,
            focused,
            mode: Mode::Normal,
            quit_requested: false,
        }
    }
    /// The document of the focused window.
    pub fn focused_doc(&self) -> &Document {
        &self.docs[self.windows[self.focused].doc()]
    }
    /// Id of the document of the focused window.
    pub fn focused_doc_id(&self) -> DocId {
        self.focused_window().doc()
    }
    /// The window that is currently focused (shown and receiving input).
    pub fn focused_window(&self) -> &Window {
        &self.windows[self.focused]
    }
    /// Id of the focused window.
    pub fn focused_window_id(&self) -> WindowId {
        self.focused
    }
    /// The window with this id, or `None` if it does not exist (e.g. was closed).
    pub fn window(&self, id: WindowId) -> Option<&Window> {
        self.windows.get(id)
    }
    /// The document with this id, or `None` if it does not exist (e.g. was closed).
    pub fn document(&self, id: DocId) -> Option<&Document> {
        self.docs.get(id)
    }
    /// Sets how many lines the focused window shows. Call it whenever the screen size is
    /// known (e.g. before drawing each frame), so that scrolling keeps the cursor visible.
    pub fn set_view_height(&mut self, height: usize) {
        self.windows[self.focused].set_height(height);
    }

    /// Switches to `mode` and moves the cursors of the focused document to a valid position
    /// for it.
    pub(crate) fn set_mode(&mut self, mode: Mode) {
        self.mode = mode;
        self.clamp_cursors(self.focused_doc_id())
    }

    /// Whether quitting was requested (see [`Ctx::quit`](crate::ctx::Ctx::quit)).
    pub fn should_quit(&self) -> bool {
        self.quit_requested
    }

    pub(crate) fn request_quit(&mut self) {
        self.quit_requested = true;
    }

    /// The current editing mode (the same for all windows).
    pub fn mode(&self) -> Mode {
        self.mode
    }

    /// Moves the cursor of the focused window, as the current mode allows: in Insert it may go
    /// after the last char of a line ([`Mode::cursor_past_end`]).
    ///
    /// ```
    /// use revim_core::movement::Motion;
    /// use revim_kernel::{document::Document, state::EditorState};
    ///
    /// let mut state = EditorState::with_document(Document::from_text(None, "ab\ncd\n"));
    /// state.move_cursor(Motion::Down);
    /// assert_eq!(state.focused_window().cursor(), 3); // 'c'
    /// ```
    pub fn move_cursor(&mut self, motion: Motion) {
        let win = &mut self.windows[self.focused];
        let doc = &self.docs[win.doc()];
        win.move_cursor(doc.text(), motion, self.mode.cursor_past_end());
    }

    /// Puts the cursor of every window showing `doc` on a valid position for the current mode.
    fn clamp_cursors(&mut self, doc: DocId) {
        let text = self.docs[doc].text();
        let past_end = self.mode.cursor_past_end();
        for (_, win) in self.windows.iter_mut().filter(|(_, w)| w.doc() == doc) {
            win.clamp_cursor(text, past_end);
        }
    }

    /// Applies `tx` to the document of the focused window: changes the text, moves the cursors
    /// of all windows showing it and puts the focused window's cursor at `tx.selection`, if any.
    /// Then moves every cursor of the document to a valid position for the current mode.
    ///
    /// # Panics
    /// If `tx` does not fit the document, or its selection is outside of the new text.
    /// Checked before anything changes.
    pub(crate) fn apply(&mut self, tx: Transaction) {
        let doc_id = self.focused_doc_id();
        let doc = &mut self.docs[doc_id];
        assert_eq!(
            tx.changes.len_before(),
            doc.text().len_chars(),
            "transaction does not fit the document"
        );
        if let Some(sel) = &tx.selection {
            assert!(
                sel.fits(tx.changes.len_after()),
                "selection outside of the new text"
            );
        }

        doc.apply(&tx.changes);
        let text = doc.text();
        for (_, win) in self.windows.iter_mut().filter(|(_, w)| w.doc() == doc_id) {
            win.map_selection(text, &tx.changes);
        }
        if let Some(sel) = tx.selection {
            self.windows[self.focused].set_selection(text, sel);
        }
        self.clamp_cursors(doc_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use revim_core::{change::ChangeSet, selection::Selection};

    fn state(text: &str) -> EditorState {
        EditorState::with_document(Document::from_text(None, text))
    }

    #[test]
    fn with_document_focuses_a_window_on_it() {
        let state = state("hello\n");
        let win = state.focused_window();
        assert_eq!(state.docs[win.doc()].text().to_string(), "hello\n");
        assert_eq!((win.cursor(), win.scroll()), (0, 0));
    }

    #[test]
    fn move_cursor_moves_the_focused_window() {
        let mut state = state("ab\ncd\n");
        state.move_cursor(Motion::Down);
        state.move_cursor(Motion::Right);
        assert_eq!(state.focused_window().cursor(), 4); // 'd'
    }

    #[test]
    fn move_cursor_in_insert_goes_after_the_last_char() {
        let mut state = state("ab\ncd\n");
        state.move_cursor(Motion::Right);
        state.move_cursor(Motion::Right);
        assert_eq!(state.focused_window().cursor(), 1); // Normal: stays on 'b'
        state.set_mode(Mode::Insert);
        state.move_cursor(Motion::Right);
        assert_eq!(state.focused_window().cursor(), 2); // Insert: after 'b'
        state.move_cursor(Motion::Down);
        assert_eq!(state.focused_window().cursor(), 5); // after 'd'
    }

    #[test]
    fn window_and_document_by_id() {
        let state = state("hello\n");
        let win = state.window(state.focused_window_id()).unwrap();
        assert_eq!(win.cursor(), 0);
        let doc = state.document(state.focused_doc_id()).unwrap();
        assert_eq!(doc.text().to_string(), "hello\n");
    }

    #[test]
    fn quit_is_not_requested_initially() {
        let mut state = state("");
        assert!(!state.should_quit());
        state.request_quit();
        assert!(state.should_quit());
    }

    /// Opens a second window on the focused document.
    fn second_window(state: &mut EditorState) -> WindowId {
        let doc = state.focused_doc_id();
        state.windows.insert(Window::new(doc))
    }

    fn insert(state: &EditorState, pos: usize, text: &str) -> Transaction {
        let len = state.focused_doc().text().len_chars();
        Transaction::new(ChangeSet::replace(len, pos..pos, text))
    }

    #[test]
    fn starts_in_normal_mode() {
        assert_eq!(state("").mode(), Mode::Normal);
    }

    #[test]
    fn apply_changes_the_text_and_the_version() {
        let mut state = state("ab\n");
        let tx = insert(&state, 1, "X");
        state.apply(tx);
        assert_eq!(state.focused_doc().text().to_string(), "aXb\n");
        assert_eq!(state.focused_doc().version(), 1);
    }

    #[test]
    fn apply_sets_the_selection_of_the_focused_window() {
        let mut state = state("ab\n");
        let tx = insert(&state, 0, "XY").with_selection(Selection::point(1));
        state.apply(tx);
        assert_eq!(state.focused_window().cursor(), 1);
    }

    #[test]
    fn apply_without_selection_maps_the_cursor() {
        let mut state = state("ab\n");
        state.move_cursor(Motion::Right); // on 'b'
        let tx = insert(&state, 0, "XY");
        state.apply(tx);
        assert_eq!(state.focused_window().cursor(), 3); // still on 'b'
    }

    #[test]
    fn apply_maps_cursors_of_other_windows() {
        let mut state = state("ab\ncd\n");
        let other = second_window(&mut state);
        let doc = state.focused_doc_id();
        state.windows[other].set_selection(state.docs[doc].text(), Selection::point(4)); // 'd'
        let tx = insert(&state, 0, "XY").with_selection(Selection::point(2));
        state.apply(tx);
        assert_eq!(state.focused_window().cursor(), 2);
        assert_eq!(state.windows[other].cursor(), 6); // still on 'd'
    }

    #[test]
    fn apply_in_normal_moves_the_cursor_off_the_line_end() {
        // `x` on the last char: "abc" → "ab", the mapped cursor (2) would be on '\n'
        let mut state = state("abc\n");
        state.move_cursor(Motion::Right);
        state.move_cursor(Motion::Right);
        let len = state.focused_doc().text().len_chars();
        state.apply(Transaction::new(ChangeSet::replace(len, 2..3, "")));
        assert_eq!(state.focused_window().cursor(), 1); // 'b'
    }

    #[test]
    fn apply_clamps_cursors_of_other_windows() {
        let mut state = state("abc\n");
        let other = second_window(&mut state);
        let doc = state.focused_doc_id();
        state.windows[other].set_selection(state.docs[doc].text(), Selection::point(2)); // 'c'
        let len = state.focused_doc().text().len_chars();
        state.apply(Transaction::new(ChangeSet::replace(len, 1..3, ""))); // "a\n"
        assert_eq!(state.windows[other].cursor(), 0);
    }

    #[test]
    fn apply_in_insert_keeps_the_cursor_after_the_last_char() {
        let mut state = state("ab\n");
        state.set_mode(Mode::Insert);
        let tx = insert(&state, 2, "c").with_selection(Selection::point(3));
        state.apply(tx);
        assert_eq!(state.focused_doc().text().to_string(), "abc\n");
        assert_eq!(state.focused_window().cursor(), 3);
    }

    #[test]
    #[should_panic(expected = "does not fit")]
    fn apply_panics_on_a_change_for_another_length() {
        let mut state = state("ab\n");
        state.apply(Transaction::new(ChangeSet::replace(1, 0..0, "x")));
    }

    #[test]
    fn set_mode_normal_moves_the_cursor_off_the_line_end() {
        let mut state = state("ab\n");
        state.set_mode(Mode::Insert);
        let tx = insert(&state, 2, "c").with_selection(Selection::point(3));
        state.apply(tx);
        state.set_mode(Mode::Normal);
        assert_eq!(state.mode(), Mode::Normal);
        assert_eq!(state.focused_window().cursor(), 2); // 'c'
    }

    #[test]
    fn set_view_height_enables_scrolling() {
        let mut state = state("a\nb\nc\nd\ne\n");
        state.set_view_height(2);
        for _ in 0..3 {
            state.move_cursor(Motion::Down);
        }
        assert_eq!(state.focused_window().scroll(), 2); // line 3 is the last visible one
    }
}
