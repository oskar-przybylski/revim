//! Events sent to modules and the deferred work queued by handlers.

use crate::{state::DocId, window::WindowId};

/// Something that happened in the editor, delivered to every module's
/// [`Module::on_event`](crate::module::Module::on_event).
///
/// Events carry ids, not references: they are queued and delivered after the change, so a
/// module reads the current state through the id (e.g. [`EditorState::window`]).
/// New variants will be added, so match with a `_` arm.
///
/// [`EditorState::window`]: crate::state::EditorState::window
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Event {
    /// A document was opened (also sent for the initial document at startup).
    DocumentOpened { doc: DocId },
    /// The cursor of a window moved to a different position.
    CursorMoved { window: WindowId },
}

/// Work deferred until the current handler returns.
pub(crate) enum Effect {
    /// Run the command with this name.
    Exec(String),
    /// Deliver the event to all modules.
    Emit(Event),
}
