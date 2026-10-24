//! Terminal UI: ratatui widgets that draw the editor state.
//!
//! All drawing lives here, so the kernel and core do not depend on ratatui. Widgets only read
//! the state and never change it.

pub mod window_view;
