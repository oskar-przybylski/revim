//! Key bindings: which command a key runs in each mode.

use std::{collections::HashMap, fmt};

use crate::key::Key;

/// Editing mode; key bindings are per mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Mode {
    /// Moving around and running commands.
    Normal,
    /// Typing text: the cursor may stand after the last char of a line.
    Insert,
}
impl Mode {
    /// Whether the cursor may stand after the last char of a line (Insert) or must
    /// stand on a char (Normal).
    pub fn cursor_past_end(self) -> bool {
        matches!(self, Mode::Insert)
    }
}
impl fmt::Display for Mode {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(match self {
            Mode::Normal => "Normal",
            Mode::Insert => "Insert",
        })
    }
}
/// What a key is bound to and who bound it.
pub(crate) struct Binding {
    /// Name of the command the key runs.
    pub command: String,
    /// Argument passed to the command; empty means none. Always empty for a fallback (the
    /// typed char is passed instead).
    pub args: String,
    /// Id of the module that added the binding (for error messages).
    pub module: &'static str,
}
/// Maps a key in a mode to the command it runs, plus an optional fallback per mode for chars
/// that have no binding.
#[derive(Default)]
pub struct Keymap {
    bindings: HashMap<(Mode, Key), Binding>,
    fallbacks: HashMap<Mode, Binding>,
}
impl Keymap {
    /// Binds `key` in `mode` to `command` with the argument `args`, remembering the `module`
    /// that added the binding.
    ///
    /// If the key is already bound, keeps the existing binding and returns the command it
    /// points to.
    pub(crate) fn bind(
        &mut self,
        mode: Mode,
        key: Key,
        command: &str,
        args: &str,
        module: &'static str,
    ) -> Result<(), String> {
        if let Some(existing) = self.bindings.get(&(mode, key)) {
            return Err(existing.command.clone());
        }
        self.bindings.insert(
            (mode, key),
            Binding {
                command: command.to_string(),
                args: args.to_string(),
                module,
            },
        );
        Ok(())
    }

    /// Sets the command run for a [`Key::Char`] that has no binding in `mode`; the typed char
    /// becomes its argument. One fallback per mode.
    ///
    /// If `mode` already has a fallback, keeps it and returns the command it points to.
    pub(crate) fn set_fallback(
        &mut self,
        mode: Mode,
        command: &str,
        module: &'static str,
    ) -> Result<(), String> {
        if let Some(existing) = self.fallbacks.get(&mode) {
            return Err(existing.command.clone());
        }
        self.fallbacks.insert(
            mode,
            Binding {
                command: command.to_string(),
                args: String::new(),
                module,
            },
        );
        Ok(())
    }

    /// The fallback of `mode`, if any.
    pub(crate) fn fallback(&self, mode: Mode) -> Option<&Binding> {
        self.fallbacks.get(&mode)
    }
    /// All fallbacks, in no particular order.
    pub(crate) fn fallbacks(&self) -> impl Iterator<Item = (&Mode, &Binding)> {
        self.fallbacks.iter()
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
            keymap.bind(Mode::Normal, Key::Char('j'), "a.down", "", "a"),
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
            .bind(Mode::Normal, Key::Char('j'), "a.down", "", "a")
            .unwrap();
        assert_eq!(
            keymap.bind(Mode::Normal, Key::Char('j'), "b.other", "", "b"),
            Err("a.down".to_string())
        );
        let binding = keymap.lookup(Mode::Normal, Key::Char('j')).unwrap();
        assert_eq!((binding.command.as_str(), binding.module), ("a.down", "a"));
    }

    #[test]
    fn bind_keeps_the_argument() {
        let mut keymap = Keymap::default();
        keymap
            .bind(Mode::Insert, Key::Enter, "a.insert", "\n", "a")
            .unwrap();
        let binding = keymap.lookup(Mode::Insert, Key::Enter).unwrap();
        assert_eq!(binding.args, "\n");
    }

    #[test]
    fn same_key_in_different_modes_are_different_bindings() {
        let mut keymap = Keymap::default();
        keymap
            .bind(Mode::Normal, Key::Char('x'), "a.delete", "", "a")
            .unwrap();
        keymap
            .bind(Mode::Insert, Key::Char('x'), "a.other", "", "a")
            .unwrap();
        let normal = keymap.lookup(Mode::Normal, Key::Char('x')).unwrap();
        let insert = keymap.lookup(Mode::Insert, Key::Char('x')).unwrap();
        assert_eq!(
            (normal.command.as_str(), insert.command.as_str()),
            ("a.delete", "a.other")
        );
    }

    #[test]
    fn fallback_set_and_lookup() {
        let mut keymap = Keymap::default();
        assert_eq!(keymap.set_fallback(Mode::Insert, "a.insert", "a"), Ok(()));
        let fallback = keymap.fallback(Mode::Insert).unwrap();
        assert_eq!(
            (fallback.command.as_str(), fallback.module),
            ("a.insert", "a")
        );
        assert_eq!(fallback.args, "");
    }

    #[test]
    fn fallback_is_per_mode() {
        let mut keymap = Keymap::default();
        keymap.set_fallback(Mode::Insert, "a.insert", "a").unwrap();
        assert!(keymap.fallback(Mode::Normal).is_none());
    }

    #[test]
    fn duplicate_fallback_keeps_the_first_and_returns_it() {
        let mut keymap = Keymap::default();
        keymap.set_fallback(Mode::Insert, "a.insert", "a").unwrap();
        assert_eq!(
            keymap.set_fallback(Mode::Insert, "b.insert", "b"),
            Err("a.insert".to_string())
        );
        assert_eq!(keymap.fallback(Mode::Insert).unwrap().module, "a");
    }

    #[test]
    fn mode_display() {
        assert_eq!(Mode::Normal.to_string(), "Normal");
        assert_eq!(Mode::Insert.to_string(), "Insert");
    }

    #[test]
    fn only_insert_allows_the_cursor_past_the_line_end() {
        assert!(Mode::Insert.cursor_past_end());
        assert!(!Mode::Normal.cursor_past_end());
    }
}
