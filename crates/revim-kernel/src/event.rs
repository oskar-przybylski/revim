//! Events sent to modules and the deferred work queued by handlers.

use std::collections::VecDeque;

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
}

/// Work deferred until the current handler returns.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Effect {
    /// Run the command with this name.
    Exec(String),
    /// Deliver the event to all modules.
    Emit(Event),
}

/// Queues `event` for delivery to every module after the current handler returns.
pub(crate) fn emit(effects: &mut VecDeque<Effect>, event: Event) {
    log::debug!("queued event {event:?}");
    effects.push_back(Effect::Emit(event));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{document::Document, state::EditorState};

    #[test]
    fn emit_queues_events_at_the_back_in_order() {
        let state = EditorState::with_document(Document::scratch());
        let (doc, window) = (state.focused_doc_id(), state.focused_window_id());
        let mut effects = VecDeque::from([Effect::Exec("first".into())]);

        emit(&mut effects, Event::DocumentOpened { doc });
        emit(&mut effects, Event::CursorMoved { window });

        assert_eq!(
            effects,
            [
                Effect::Exec("first".into()),
                Effect::Emit(Event::DocumentOpened { doc }),
                Effect::Emit(Event::CursorMoved { window }),
            ]
        );
    }
}
