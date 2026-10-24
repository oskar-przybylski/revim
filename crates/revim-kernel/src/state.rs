//! Editor state: open documents and what is focused.

use slotmap::{SlotMap, new_key_type};

use crate::document::Document;

new_key_type! {
    /// Stable identifier of a document inside [`EditorState`].
    ///
    /// Cheap to copy and safe to keep around (in windows, modules, background tasks) instead
    /// of a reference. Once its document is removed, the id does not point to any other document.
    pub struct DocId;
}

/// The whole state of the editor, owned by the kernel.
///
/// Currently holds the open documents and which one is focused; windows, layout,
/// registers and mode will be added here.
///
/// An `EditorState` always has a focused document.
pub struct EditorState {
    docs: SlotMap<DocId, Document>,
    focused: DocId,
}

impl EditorState {
    /// Creates the state with one document, which becomes the focused one.
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
        let focused = docs.insert(doc);
        Self { docs, focused }
    }
    /// The document that is currently focused (shown and edited).
    pub fn focused_doc(&self) -> &Document {
        &self.docs[self.focused]
    }
}
