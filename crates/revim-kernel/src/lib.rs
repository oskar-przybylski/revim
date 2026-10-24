//! Editor kernel: the single owner of editor state.
//!
//! Holds documents ([`document::Document`]), windows showing them ([`window::Window`]) and the
//! editor state that owns both ([`state::EditorState`]), plus the module system: modules
//! ([`module::Module`]) register commands and key bindings, and [`editor::Editor`] runs them.
//! Modules use this crate through `revim-api`; the UI reads the state from it.

pub mod command;
pub mod ctx;
pub mod document;
pub mod editor;
pub mod event;
pub mod key;
pub mod keymap;
pub mod module;
pub mod state;
pub mod window;
