//! Editor kernel: the single owner of editor state.
//!
//! Holds documents ([`document::Document`]) and the editor state that ties them together
//! ([`state::EditorState`]). Modules and the UI read and change state only through this crate.

pub mod document;
pub mod state;
pub mod window;
