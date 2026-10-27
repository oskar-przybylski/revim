//! Core text model: changes to text, cursor movements and selections.
//!
//! Pure logic with no I/O, async or UI.

pub mod change;
pub mod movement;
pub mod selection;
pub mod transaction;
