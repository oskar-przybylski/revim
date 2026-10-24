//! Windows: views of documents.
use revim_core::selection::Selection;
use slotmap::new_key_type;

use crate::state::DocId;

new_key_type! {
    /// Stable identifier of a window inside the editor state.
    pub struct WindowId;
}

/// A view of one document: which document is shown, where the cursor is and how it is scrolled.
///
/// Several windows may show the same document, each with its own cursor and scroll.
#[expect(dead_code)]
pub struct Window {
    /// The shown document.
    doc: DocId,
    /// Cursor(s) of this window.
    selection: Selection,
    /// Index of the first visible line.
    scroll: usize,
    /// Column that vertical movements (`j`, `k`) aim for; `None` until the first one.
    desired_col: Option<usize>,
}
