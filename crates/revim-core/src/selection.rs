//! Cursors and selections.

/// A range of text between two char positions.
///
/// `anchor` is where the range started and stays fixed while extending; `head` is the end
/// that moves (where the cursor is). In Normal mode the cursor is a range with
/// `anchor == head` and stands on the char at `head`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Range {
    /// Fixed end of the range.
    pub anchor: usize,
    /// Moving end of the range — the cursor position.
    pub head: usize,
}

/// The selection of one window: one or more [`Range`]s, one of which is primary.
///
/// A selection always has at least one range. The primary range is the "main cursor":
/// the view follows it and commands that need a single position use it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Selection {
    ranges: Vec<Range>,
    primary: usize,
}

impl Selection {
    /// A selection with a single cursor at `pos`.
    ///
    /// ```
    /// use revim_core::selection::{Range, Selection};
    ///
    /// let sel = Selection::point(3);
    /// assert_eq!(sel.primary(), Range { anchor: 3, head: 3 });
    /// ```
    pub fn point(pos: usize) -> Self {
        Selection {
            ranges: vec![Range {
                anchor: pos,
                head: pos,
            }],
            primary: 0,
        }
    }
    /// The primary range.
    pub fn primary(&self) -> Range {
        self.ranges[self.primary]
    }
}
