//! Events sent to modules and the deferred work queued by handlers.

use revim_core::change::ChangeSet;

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
    DocumentOpened {
        /// The opened document.
        doc: DocId,
    },
    /// The cursor of a window moved to a different position.
    CursorMoved {
        /// The window whose cursor moved.
        window: WindowId,
    },
    /// The text of a document was changed by a transaction ([`Ctx::apply`]). The cursors of
    /// all windows showing it are already moved.
    ///
    /// [`Ctx::apply`]: crate::ctx::Ctx::apply
    DocChanged {
        /// The changed document.
        doc: DocId,
        /// The change, from the text before to the text after it; use it to move positions
        /// a module keeps ([`ChangeSet::map_pos`]).
        changes: ChangeSet,
        /// Version of the document after the change.
        version: u64,
    },
    /// The editing mode changed; read the new one with [`EditorState::mode`].
    ///
    /// [`EditorState::mode`]: crate::state::EditorState::mode
    ModeChanged,
}

/// Work deferred until the current handler returns.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Effect {
    /// Run a command.
    Exec {
        /// Name of the command.
        command: String,
        /// Argument passed to the handler; empty means none.
        args: String,
    },
    /// Deliver the event to all modules.
    Emit(Event),
}
