//! Editor state: open documents, the windows showing them and which window is focused.

use revim_core::movement::Motion;
use slotmap::{SlotMap, new_key_type};

use crate::{
    document::Document,
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
    focused: WindowId,
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
        }
    }
    /// The document of the focused window.
    pub fn focused_doc(&self) -> &Document {
        &self.docs[self.windows[self.focused].doc()]
    }
    /// The window that is currently focused (shown and receiving input).
    pub fn focused_window(&self) -> &Window {
        &self.windows[self.focused]
    }

    /// Sets how many lines the focused window shows. Call it whenever the screen size is
    /// known (e.g. before drawing each frame), so that scrolling keeps the cursor visible.
    pub fn set_view_height(&mut self, height: usize) {
        self.windows[self.focused].set_height(height);
    }

    /// Moves the cursor of the focused window.
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
        win.move_cursor(doc.text(), motion);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn set_view_height_enables_scrolling() {
        let mut state = state("a\nb\nc\nd\ne\n");
        state.set_view_height(2);
        for _ in 0..3 {
            state.move_cursor(Motion::Down);
        }
        assert_eq!(state.focused_window().scroll(), 2); // line 3 is the last visible one
    }
}
