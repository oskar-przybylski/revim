//! Editor kernel: the single owner of editor state.
//!
//! Holds documents ([`document::Document`]), windows showing them ([`window::Window`]) and the
//! editor state that owns both ([`state::EditorState`]). Modules and the UI read and change
//! state only through this crate.

pub mod document;
pub mod state;
pub mod window;
