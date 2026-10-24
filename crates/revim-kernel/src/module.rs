//! Modules, their registration and registration errors.

use std::fmt;

use crate::{
    command::{Command, CommandRegistry},
    ctx::Ctx,
    event::Event,
    key::{Key, KeyParseError, parse_key},
    keymap::{Keymap, Mode},
};

/// A unit of functionality: registers what it adds and reacts to events.
pub trait Module: 'static {
    /// Short unique id, used as the prefix of the module's command names and in error messages.
    fn id(&self) -> &'static str;
    /// Called once at startup: register commands and key bindings through `r`.
    fn register(&mut self, r: &mut Registrar);
    /// Called for every [`Event`]. The default does nothing.
    fn on_event(&mut self, _event: &Event, _cx: &mut Ctx) {}
}

/// Collects what a module adds while it registers. Errors are collected, not returned,
/// so all problems of all modules are reported at once.
pub struct Registrar<'a> {
    module: &'static str,
    commands: &'a mut CommandRegistry,
    keymap: &'a mut Keymap,
    errors: &'a mut Vec<RegisterError>,
}
impl<'a> Registrar<'a> {
    /// Used by `Editor::new`: one registrar per module.
    pub(crate) fn new(
        module: &'static str,
        commands: &'a mut CommandRegistry,
        keymap: &'a mut Keymap,
        errors: &'a mut Vec<RegisterError>,
    ) -> Self {
        Self {
            module,
            commands,
            keymap,
            errors,
        }
    }

    /// Registers a command. `name` must be unique (by convention `module.object.action`),
    /// `desc` is a short description for key hints and the command palette.
    pub fn command(&mut self, name: &str, desc: &str, handler: impl Fn(&mut Ctx) + 'static) {
        let cmd = Command {
            name: name.to_string(),
            desc: desc.to_string(),
            handler: Box::new(handler),
        };
        if !self.commands.add(cmd) {
            self.error(RegisterErrorKind::DuplicateCommand {
                command: name.to_string(),
            });
        }
    }

    /// Binds `key` (Vim notation: `"j"`, `"<C-q>"`, `"<Up>"`) in `mode` to `command`.
    ///
    /// The command may belong to any module; whether it exists is checked after all modules
    /// have registered.
    pub fn keymap(&mut self, mode: Mode, key: &str, command: &str) {
        match parse_key(key) {
            Ok(parsed) => {
                if let Err(bound_to) = self.keymap.bind(mode, parsed, command, self.module) {
                    self.error(RegisterErrorKind::DuplicateKey {
                        mode,
                        key: parsed,
                        bound_to,
                    });
                }
            }
            Err(reason) => {
                self.error(RegisterErrorKind::InvalidKey {
                    key: key.to_string(),
                    reason,
                });
            }
        }
    }

    fn error(&mut self, kind: RegisterErrorKind) {
        self.errors.push(RegisterError {
            module: self.module,
            kind,
        });
    }
}

/// A problem found while registering modules, tagged with the module it comes from.
///
/// `Display` gives a one-line message, e.g.
/// ``[vim] key j (Normal) is already bound to `vim.move.down` ``.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegisterError {
    /// Id of the module that caused the problem.
    pub module: &'static str,
    /// What went wrong.
    pub kind: RegisterErrorKind,
}

/// What went wrong while registering modules.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum RegisterErrorKind {
    /// A command with this name is already registered; the first one is kept.
    DuplicateCommand { command: String },
    /// The key is already bound in this mode; the first binding is kept.
    DuplicateKey {
        mode: Mode,
        key: Key,
        /// Command the key is already bound to.
        bound_to: String,
    },
    /// The key notation could not be parsed.
    InvalidKey {
        /// The notation as written by the module.
        key: String,
        reason: KeyParseError,
    },
    /// The key is bound to a command that no module registered.
    UnknownCommand {
        mode: Mode,
        key: Key,
        command: String,
    },
    /// No module registered `editor.quit`, so there would be no way to quit.
    MissingQuitCommand,
}

impl fmt::Display for RegisterError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "[{}] ", self.module)?;
        match &self.kind {
            RegisterErrorKind::DuplicateCommand { command } => {
                write!(f, "duplicate command `{command}`")
            }
            RegisterErrorKind::DuplicateKey {
                mode,
                key,
                bound_to,
            } => {
                write!(f, "key {key} ({mode}) is already bound to `{bound_to}`")
            }
            RegisterErrorKind::InvalidKey { key, reason } => {
                write!(f, "invalid key `{key}`: {reason}")
            }
            RegisterErrorKind::UnknownCommand { mode, key, command } => {
                write!(
                    f,
                    "key {key} ({mode}) is bound to unknown command `{command}`"
                )
            }
            RegisterErrorKind::MissingQuitCommand => f.write_str(
                "command `editor.quit` is not registered, the editor could not be closed",
            ),
        }
    }
}

impl std::error::Error for RegisterError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_names_the_module_and_the_key_in_vim_notation() {
        let error = RegisterError {
            module: "vim",
            kind: RegisterErrorKind::UnknownCommand {
                mode: Mode::Normal,
                key: Key::Ctrl('q'),
                command: "editor.quitt".into(),
            },
        };
        assert_eq!(
            error.to_string(),
            "[vim] key <C-q> (Normal) is bound to unknown command `editor.quitt`"
        );
    }
}
