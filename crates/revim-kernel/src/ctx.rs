//! The context passed to command handlers and event handlers.

use std::collections::VecDeque;

use revim_core::movement::Motion;

use crate::{
    event::{self, Effect, Event},
    state::EditorState,
};

/// What a module can do while handling a command or an event.
///
/// Changes to the state happen immediately; [`exec`](Self::exec) and the events emitted by
/// changes are queued and run after the current handler returns, so a handler is never
/// re-entered while it runs.
pub struct Ctx<'a> {
    state: &'a mut EditorState,
    effects: &'a mut VecDeque<Effect>,
}
impl<'a> Ctx<'a> {
    pub(crate) fn new(state: &'a mut EditorState, effects: &'a mut VecDeque<Effect>) -> Self {
        Self { state, effects }
    }

    /// Read-only access to the editor state.
    pub fn state(&self) -> &EditorState {
        self.state
    }
    /// Moves the cursor of the focused window; emits [`Event::CursorMoved`] if it actually moved.
    pub fn move_cursor(&mut self, motion: Motion) {
        let window = self.state.focused_window_id();
        let before = self.state.focused_window().cursor();
        self.state.move_cursor(motion);
        if self.state.focused_window().cursor() != before {
            event::emit(self.effects, Event::CursorMoved { window });
        }
    }
    /// Runs the command `command` after the current handler returns. An unknown name is ignored.
    pub fn exec(&mut self, command: &str) {
        self.effects.push_back(Effect::Exec(command.to_string()))
    }
    /// Asks the editor to quit after the current input is handled.
    pub fn quit(&mut self) {
        self.state.request_quit();
    }
}
