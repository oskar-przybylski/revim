//! Transactions: one atomic edit of a document.

use crate::{change::ChangeSet, selection::Selection};

/// One atomic edit of a document: the change to its text and, optionally, the selection of
/// the window that made the edit.
///
/// Without a selection the cursor of that window is moved through [`changes`](Self::changes)
/// like in every other window showing the document.
///
/// ```
/// use revim_core::change::ChangeSet;
/// use revim_core::selection::Selection;
/// use revim_core::transaction::Transaction;
///
/// // "ab" → "aXb", cursor on 'b' after the inserted "X"
/// let mut cs = ChangeSet::new();
/// cs.retain(1);
/// cs.insert("X");
/// cs.retain(1);
///
/// let tx = Transaction::new(cs).with_selection(Selection::point(2));
/// assert_eq!(tx.changes.len_after(), 3);
/// assert_eq!(tx.selection, Some(Selection::point(2)));
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Transaction {
    /// The change to the text.
    pub changes: ChangeSet,
    /// Selection of the editing window after the change; `None` maps its current selection
    /// through [`changes`](Self::changes).
    pub selection: Option<Selection>,
}

impl Transaction {
    /// A transaction with `changes` and no explicit selection.
    ///
    /// ```
    /// use revim_core::change::ChangeSet;
    /// use revim_core::transaction::Transaction;
    ///
    /// let tx = Transaction::new(ChangeSet::new());
    /// assert!(tx.selection.is_none());
    /// ```
    pub fn new(changes: ChangeSet) -> Self {
        Self {
            changes,
            selection: None,
        }
    }

    /// Sets the selection of the editing window after the change, replacing any previous one.
    ///
    /// Positions refer to the text **after** the change.
    #[must_use]
    pub fn with_selection(mut self, selection: Selection) -> Self {
        self.selection = Some(selection);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn with_selection_replaces_the_previous_one() {
        let tx = Transaction::new(ChangeSet::new())
            .with_selection(Selection::point(1))
            .with_selection(Selection::point(0));
        assert_eq!(tx.selection, Some(Selection::point(0)));
    }
}
