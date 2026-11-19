//! Cursors and selections.

use crate::change::{Assoc, ChangeSet};

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
    /// Moves every range through `change_set`, so that it stays on the same text after the
    /// change is applied. A position at an insertion ends up after the inserted text; a
    /// position inside deleted text ends up where the deletion was.
    ///
    /// ```
    /// use revim_core::change::ChangeSet;
    /// use revim_core::selection::Selection;
    ///
    /// let mut sel = Selection::point(1); // on 'b' in "ab"
    /// sel.map(&ChangeSet::replace(2, 0..0, "xy")); // "xyab"
    /// assert_eq!(sel, Selection::point(3)); // still on 'b'
    /// ```
    pub fn map(&mut self, change_set: &ChangeSet) {
        self.ranges.iter_mut().for_each(|r| {
            r.head = change_set.map_pos(r.head, Assoc::After);
            r.anchor = change_set.map_pos(r.anchor, Assoc::After)
        });
    }
    /// Whether every range lies within a text of `len` chars (a position may equal `len`: the
    /// end of the text).
    pub fn fits(&self, len: usize) -> bool {
        self.ranges.iter().all(|r| r.head <= len && r.anchor <= len)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn map_moves_anchor_and_head() {
        let mut sel = Selection {
            ranges: vec![Range { anchor: 1, head: 3 }],
            primary: 0,
        };
        sel.map(&ChangeSet::replace(4, 0..0, "x"));
        assert_eq!(sel.primary(), Range { anchor: 2, head: 4 });
    }

    #[test]
    fn map_inside_deleted_text_falls_to_its_start() {
        let mut sel = Selection::point(3);
        sel.map(&ChangeSet::replace(5, 1..4, ""));
        assert_eq!(sel, Selection::point(1));
    }

    #[test]
    fn map_at_insertion_goes_after_it() {
        let mut sel = Selection::point(1);
        sel.map(&ChangeSet::replace(2, 1..1, "zż"));
        assert_eq!(sel, Selection::point(3));
    }

    #[test]
    fn map_moves_every_range() {
        let mut sel = Selection {
            ranges: vec![Range { anchor: 0, head: 0 }, Range { anchor: 2, head: 2 }],
            primary: 1,
        };
        sel.map(&ChangeSet::replace(3, 1..1, "x"));
        assert_eq!(
            sel.ranges,
            [Range { anchor: 0, head: 0 }, Range { anchor: 3, head: 3 }]
        );
        assert_eq!(sel.primary(), Range { anchor: 3, head: 3 });
    }

    #[test]
    fn fits_allows_the_end_of_text() {
        assert!(Selection::point(3).fits(3));
        assert!(!Selection::point(4).fits(3));
        let sel = Selection {
            ranges: vec![Range { anchor: 0, head: 1 }, Range { anchor: 5, head: 2 }],
            primary: 0,
        };
        assert!(!sel.fits(4));
    }
}
