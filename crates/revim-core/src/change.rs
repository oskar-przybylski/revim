// change.rs
// Author: oskar
// Created: 23.10.2026
//
// Heavily inspired by Helix text editor
// https://github.com/helix-editor/helix/blob/master/helix-core/src/transaction.rs
// props for creators of this idea

use std::cmp::Ordering;

use ropey::Rope;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Op {
    Retain(usize),
    Delete(usize),
    Insert(String),
}

impl Op {
    fn push_into(self, ops: &mut Vec<Op>) {
        match (ops.last_mut(), self) {
            (_, Op::Retain(0) | Op::Delete(0)) => {}
            (_, Op::Insert(s)) if s.is_empty() => {}
            (Some(Op::Retain(last)), Op::Retain(n)) => *last += n,
            (Some(Op::Delete(last)), Op::Delete(n)) => *last += n,
            (Some(Op::Insert(last)), Op::Insert(s)) => last.push_str(&s),
            (_, op) => ops.push(op),
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// Defines what happens if inserted string is exactly on pos in map_pos
pub enum Assoc {
    Before, // stay before inserted text
    After,  // stay after inserted text
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ChangeSet {
    ops: Vec<Op>,
    len_before: usize, // text len before apply, Retain + Delete
    len_after: usize,  // text len after apply, Retain + Insert len
}

impl ChangeSet {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn from_ops(ops: Vec<Op>) -> Self {
        let (len_before, len_after) = ops.iter().fold((0, 0), |(b, a), op| match op {
            Op::Retain(n) => (b + n, a + n),
            Op::Delete(n) => (b + n, a),
            Op::Insert(s) => (b, a + s.chars().count()),
        });

        Self {
            ops,
            len_before,
            len_after,
        }
    }

    pub fn retain(&mut self, n: usize) {
        self.len_before += n;
        self.len_after += n;
        Op::Retain(n).push_into(&mut self.ops);
    }
    pub fn delete(&mut self, n: usize) {
        self.len_before += n;
        Op::Delete(n).push_into(&mut self.ops);
    }
    pub fn insert(&mut self, s: &str) {
        self.len_after += s.chars().count();
        Op::Insert(s.to_string()).push_into(&mut self.ops);
    }

    pub fn len_before(&self) -> usize {
        self.len_before
    }
    pub fn len_after(&self) -> usize {
        self.len_after
    }

    pub fn apply(&self, text: &mut Rope) {
        assert_eq!(
            text.len_chars(),
            self.len_before,
            "apply: lengths don't match"
        );

        let mut pos = 0;
        for op in &self.ops {
            match op {
                Op::Retain(n) => pos += n,
                Op::Delete(n) => text.remove(pos..pos + n),
                Op::Insert(s) => {
                    text.insert(pos, s);
                    pos += s.chars().count();
                }
            }
        }
    }

    pub fn map_pos(&self, pos: usize, assoc: Assoc) -> usize {
        let mut old = 0;
        let mut shift: isize = 0;

        for op in &self.ops {
            if old > pos {
                break;
            }
            match op {
                Op::Retain(n) => old += n,
                Op::Delete(n) => {
                    // delete before position, left shift
                    if old + n <= pos {
                        shift -= *n as isize;
                    // pos inside delete
                    } else if old <= pos {
                        return (old as isize + shift) as usize;
                    }
                    old += n;
                }
                Op::Insert(s) => {
                    let len = s.chars().count() as isize;
                    if old < pos || (old == pos && assoc == Assoc::After) {
                        shift += len; // insert before pos, right shift
                    }
                }
            }
        }
        (pos as isize + shift) as usize
    }

    pub fn invert(&self, original: &Rope) -> ChangeSet {
        assert_eq!(
            original.len_chars(),
            self.len_before,
            "invert: lengths don't match"
        );

        let mut old = 0;
        let mut ops: Vec<Op> = Vec::with_capacity(self.ops.len());

        for op in &self.ops {
            match op {
                Op::Retain(n) => {
                    ops.push(Op::Retain(*n));
                    old += n;
                }
                Op::Delete(n) => {
                    ops.push(Op::Insert(original.slice(old..old + n).to_string()));
                    old += n;
                }
                Op::Insert(s) => ops.push(Op::Delete(s.chars().count())),
            }
        }

        Self::from_ops(ops)
    }

    pub fn compose(self, other: Self) -> Self {
        use Op::*;
        assert_eq!(
            self.len_after, other.len_before,
            "compose: lengths don't match"
        );

        let mut a_iter = self.ops.into_iter();
        let mut b_iter = other.ops.into_iter();
        let mut a = a_iter.next();
        let mut b = b_iter.next();

        let mut out = Self::new();

        loop {
            match (a.take(), b.take()) {
                (None, None) => break,
                // deletion in A
                (Some(Delete(n)), b_op) => {
                    out.delete(n);
                    a = a_iter.next();
                    b = b_op;
                }
                // insertion in B
                (a_op, Some(Insert(s))) => {
                    out.insert(&s);
                    a = a_op;
                    b = b_iter.next();
                }
                (Some(Retain(i)), Some(Retain(j))) => match i.cmp(&j) {
                    Ordering::Less => {
                        out.retain(i);
                        a = a_iter.next();
                        b = Some(Op::Retain(j - i));
                    }
                    Ordering::Equal => {
                        out.retain(i);
                        a = a_iter.next();
                        b = b_iter.next();
                    }
                    Ordering::Greater => {
                        out.retain(j);
                        a = Some(Op::Retain(i - j));
                        b = b_iter.next();
                    }
                },
                (Some(Insert(mut s)), Some(Delete(j))) => {
                    let len = s.chars().count();
                    match len.cmp(&j) {
                        Ordering::Less => {
                            a = a_iter.next();
                            b = Some(Delete(j - len));
                        }
                        Ordering::Equal => {
                            a = a_iter.next();
                            b = b_iter.next();
                        }
                        Ordering::Greater => {
                            let (pos, _) = s.char_indices().nth(j).unwrap();
                            s.replace_range(0..pos, "");
                            a = Some(Insert(s));
                            b = b_iter.next();
                        }
                    }
                }
                (Some(Insert(s)), Some(Retain(j))) => {
                    let len = s.chars().count();
                    match len.cmp(&j) {
                        Ordering::Less => {
                            out.insert(&s);
                            a = a_iter.next();
                            b = Some(Retain(j - len));
                        }
                        Ordering::Equal => {
                            out.insert(&s);
                            a = a_iter.next();
                            b = b_iter.next();
                        }
                        Ordering::Greater => {
                            let (pos, _) = s.char_indices().nth(j).unwrap();
                            let mut before = s;
                            let after = before.split_off(pos);

                            out.insert(&before);
                            a = Some(Insert(after));
                            b = b_iter.next();
                        }
                    }
                }
                (Some(Retain(i)), Some(Delete(j))) => match i.cmp(&j) {
                    Ordering::Less => {
                        out.delete(i);
                        a = a_iter.next();
                        b = Some(Delete(j - i));
                    }
                    Ordering::Equal => {
                        out.delete(i);
                        a = a_iter.next();
                        b = b_iter.next();
                    }
                    Ordering::Greater => {
                        out.delete(j);
                        a = Some(Retain(i - j));
                        b = b_iter.next();
                    }
                },
                _ => unreachable!("lengths checked by assert above"),
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use Assoc::*;
    use Op::*;

    #[test]
    fn new_computes_lenghts() {
        let cs = ChangeSet::from_ops(vec![Retain(6), Delete(5), Insert("Revim".into())]);
        assert_eq!(cs.len_before(), 11);
        assert_eq!(cs.len_after(), 11);
    }

    #[test]
    fn apply_replaces_word() {
        let mut text = Rope::from_str("hello world");
        ChangeSet::from_ops(vec![Retain(6), Delete(5), Insert("Revim".into())]).apply(&mut text);
        assert_eq!(text.to_string(), "hello Revim");
    }

    #[test]
    fn apply_inserts_in_many_places() {
        let mut text = Rope::from_str("a\nb\nc\n");
        let cs = ChangeSet::from_ops(vec![
            Insert("// ".into()),
            Retain(2),
            Insert("// ".into()),
            Retain(2),
            Insert("// ".into()),
            Retain(2),
        ]);
        cs.apply(&mut text);
        assert_eq!(text.to_string(), "// a\n// b\n// c\n");
    }

    #[test]
    fn apply_counts_chars_not_bytes() {
        let mut text = Rope::from_str("zażółć");
        ChangeSet::from_ops(vec![Retain(2), Delete(4), Insert("b".into())]).apply(&mut text);
        assert_eq!(text.to_string(), "zab");
    }

    #[test]
    #[should_panic]
    fn apply_panics_on_wrong_length() {
        let mut text = Rope::from_str("abc");
        ChangeSet::from_ops(vec![Retain(5)]).apply(&mut text);
    }

    fn replace_world() -> ChangeSet {
        // "hello world" → "hello Revim"
        ChangeSet::from_ops(vec![Retain(6), Delete(5), Insert("Revim".into())])
    }

    #[test]
    fn map_pos_before_change_is_unchanged() {
        assert_eq!(replace_world().map_pos(0, After), 0);
        assert_eq!(replace_world().map_pos(5, After), 5);
    }

    #[test]
    fn map_pos_inside_deleted_falls_to_start() {
        assert_eq!(replace_world().map_pos(6, After), 6);
        assert_eq!(replace_world().map_pos(8, After), 6);
    }

    #[test]
    fn map_pos_after_change_shifts() {
        let cs = ChangeSet::from_ops(vec![Retain(4), Delete(3), Retain(4)]);
        assert_eq!(cs.map_pos(9, After), 6);
        assert_eq!(cs.map_pos(5, After), 4);
        assert_eq!(cs.map_pos(11, After), 8);
    }

    #[test]
    fn map_pos_at_insert_respects_assoc() {
        let cs = ChangeSet::from_ops(vec![Retain(1), Insert("X".into()), Retain(1)]);
        assert_eq!(cs.map_pos(1, Before), 1);
        assert_eq!(cs.map_pos(1, After), 2);
    }

    #[test]
    fn map_pos_insert_at_start_counts_chars() {
        let cs = ChangeSet::from_ops(vec![Insert("żółw ".into()), Retain(2)]);
        assert_eq!(cs.map_pos(0, Before), 0);
        assert_eq!(cs.map_pos(0, After), 5);
        assert_eq!(cs.map_pos(1, After), 6);
    }

    #[test]
    fn invert_swaps_insert_and_delete() {
        let original = Rope::from_str("hello world");
        let inv = replace_world().invert(&original);
        assert_eq!(
            inv,
            ChangeSet::from_ops(vec![Retain(6), Insert("world".into()), Delete(5)])
        );
    }

    #[test]
    fn invert_restores_original_text() {
        let original = Rope::from_str("foo bar baz");
        let cs = ChangeSet::from_ops(vec![Retain(4), Delete(3), Retain(4)]); // diw
        let inv = cs.invert(&original); // PRZED apply

        let mut text = original.clone();
        cs.apply(&mut text);
        assert_eq!(text.to_string(), "foo  baz");
        inv.apply(&mut text);
        assert_eq!(text.to_string(), "foo bar baz");
    }

    #[test]
    fn invert_handles_multibyte_chars() {
        let original = Rope::from_str("zażółć");
        let cs = ChangeSet::from_ops(vec![Retain(2), Delete(4), Insert("b".into())]);
        let inv = cs.invert(&original);
        assert_eq!(
            inv,
            ChangeSet::from_ops(vec![Retain(2), Insert("żółć".into()), Delete(1)])
        );
    }

    #[test]
    fn invert_lengths_are_swapped() {
        let original = Rope::from_str("hello world");
        let cs = ChangeSet::from_ops(vec![Retain(6), Delete(5), Insert("Rust".into())]);
        let inv = cs.invert(&original);
        assert_eq!(inv.len_before(), cs.len_after());
        assert_eq!(inv.len_after(), cs.len_before());
    }

    #[test]
    fn compose_typing_merges_into_one_insert() {
        let a = ChangeSet::from_ops(vec![Retain(3), Insert("a".into()), Retain(4)]); // "foo bar" → "fooa bar"
        let b = ChangeSet::from_ops(vec![Retain(4), Insert("b".into()), Retain(4)]);
        let c = ChangeSet::from_ops(vec![Retain(5), Insert("c".into()), Retain(4)]);
        assert_eq!(
            a.compose(b).compose(c),
            ChangeSet::from_ops(vec![Retain(3), Insert("abc".into()), Retain(4)])
        );
    }

    #[test]
    fn compose_delete_then_insert_equals_replace() {
        let a = ChangeSet::from_ops(vec![Retain(6), Delete(5)]); // "hello world" → "hello "
        let b = ChangeSet::from_ops(vec![Retain(6), Insert("Revim".into())]); // → "hello Revim"
        assert_eq!(a.compose(b), replace_world());
    }

    #[test]
    fn compose_insert_then_delete_cancels() {
        let a = ChangeSet::from_ops(vec![Retain(2), Insert("xyz".into()), Retain(2)]); // "abcd" → "abxyzcd"
        let b = ChangeSet::from_ops(vec![Retain(2), Delete(3), Retain(2)]); // → "abcd"
        assert_eq!(a.compose(b), ChangeSet::from_ops(vec![Retain(4)]));
    }

    #[test]
    fn compose_equals_sequential_apply() {
        let t0 = Rope::from_str("zażółć gęślą jaźń");
        let a = ChangeSet::from_ops(vec![Retain(2), Delete(4), Insert("XY".into()), Retain(11)]);
        let b = ChangeSet::from_ops(vec![
            Retain(1),
            Delete(2),
            Retain(3),
            Insert("!".into()),
            Retain(9),
        ]);

        let mut seq = t0.clone();
        a.apply(&mut seq);
        b.apply(&mut seq);

        let mut once = t0.clone();
        a.compose(b).apply(&mut once);
        assert_eq!(once.to_string(), seq.to_string());
    }

    #[test]
    #[should_panic]
    fn compose_panics_on_length_mismatch() {
        let a = ChangeSet::from_ops(vec![Retain(3)]);
        let b = ChangeSet::from_ops(vec![Retain(5)]);
        a.compose(b);
    }
}
