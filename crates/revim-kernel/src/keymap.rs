//! Key bindings: which command a key runs in each mode.

use std::{collections::HashMap, fmt};

use crate::key::Key;

/// Editing mode; key bindings are per mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Mode {
    /// Moving around and running commands.
    Normal,
}
impl fmt::Display for Mode {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(match self {
            Mode::Normal => "Normal",
        })
    }
}
/// What a key is bound to and who bound it.
pub(crate) struct Binding {
    /// Name of the command the key runs.
    pub command: String,
    /// Id of the module that added the binding (for error messages).
    pub module: &'static str,
}
/// Maps a key in a mode to the command it runs.
#[derive(Default)]
pub struct Keymap {
    bindings: HashMap<(Mode, Key), Binding>,
}
impl Keymap {
    /// Binds `key` in `mode` to `command`, remembering the `module` that added the binding.
    ///
    /// If the key is already bound, keeps the existing binding and returns the command it
    /// points to.
    pub(crate) fn bind(
        &mut self,
        mode: Mode,
        key: Key,
        command: &str,
        module: &'static str,
    ) -> Result<(), String> {
        if let Some(existing) = self.bindings.get(&(mode, key)) {
            return Err(existing.command.clone());
        }
        self.bindings.insert(
            (mode, key),
            Binding {
                command: command.to_string(),
                module,
            },
        );
        Ok(())
    }
    /// The binding of `key` in `mode`, if any.
    pub(crate) fn lookup(&self, mode: Mode, key: Key) -> Option<&Binding> {
        self.bindings.get(&(mode, key))
    }
    /// All bindings, in no particular order.
    pub(crate) fn bindings(&self) -> impl Iterator<Item = (&(Mode, Key), &Binding)> {
        self.bindings.iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bind_and_lookup() {
        let mut keymap = Keymap::default();
        assert_eq!(
            keymap.bind(Mode::Normal, Key::Char('j'), "a.down", "a"),
            Ok(())
        );
        let binding = keymap.lookup(Mode::Normal, Key::Char('j')).unwrap();
        assert_eq!((binding.command.as_str(), binding.module), ("a.down", "a"));
        assert!(keymap.lookup(Mode::Normal, Key::Char('k')).is_none());
    }

    #[test]
    fn duplicate_bind_keeps_the_first_and_returns_it() {
        let mut keymap = Keymap::default();
        keymap
            .bind(Mode::Normal, Key::Char('j'), "a.down", "a")
            .unwrap();
        assert_eq!(
            keymap.bind(Mode::Normal, Key::Char('j'), "b.other", "b"),
            Err("a.down".to_string())
        );
        let binding = keymap.lookup(Mode::Normal, Key::Char('j')).unwrap();
        assert_eq!((binding.command.as_str(), binding.module), ("a.down", "a"));
    }

    #[test]
    fn mode_display() {
        assert_eq!(Mode::Normal.to_string(), "Normal");
    }
}
