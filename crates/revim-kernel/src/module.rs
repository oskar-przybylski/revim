//! Modules, their registration and registration errors.

use std::fmt;

use crate::{
    command::{Command, CommandRegistry},
    ctx::Ctx,
    event::Event,
    gutter::{GutterProvider, GutterProviderRegistry},
    key::{Key, KeyParseError, parse_key},
    keymap::{Keymap, Mode},
};

/// A unit of functionality: registers what it adds and reacts to events.
pub trait Module: 'static {
    /// Short unique id, used as the prefix of the module's command names and in error messages.
    fn id(&self) -> &'static str;
    /// Called once at startup: register commands, key bindings and gutter columns through `r`.
    fn register(&mut self, r: &mut Registrar);
    /// Called for every [`Event`]. The default does nothing.
    fn on_event(&mut self, _event: &Event, _cx: &mut Ctx) {}
}

/// A module that panicked and was disabled.
///
/// `Display` gives `[module] panicked and was disabled: message`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModuleFailure {
    /// Id of the module that panicked.
    pub module: &'static str,
    /// The panic message.
    pub message: String,
}

/// Collects what a module adds while it registers. Errors are collected, not returned,
/// so all problems of all modules are reported at once.
pub struct Registrar<'a> {
    module: &'static str,
    commands: &'a mut CommandRegistry,
    keymap: &'a mut Keymap,
    gutters: &'a mut GutterProviderRegistry,
    errors: &'a mut Vec<RegisterError>,
}
impl<'a> Registrar<'a> {
    /// Used by `Editor::new`: one registrar per module.
    pub(crate) fn new(
        module: &'static str,
        commands: &'a mut CommandRegistry,
        keymap: &'a mut Keymap,
        gutters: &'a mut GutterProviderRegistry,
        errors: &'a mut Vec<RegisterError>,
    ) -> Self {
        Self {
            module,
            commands,
            keymap,
            gutters,
            errors,
        }
    }

    /// Registers a command. `name` must be unique (by convention `module.object.action`),
    /// `desc` is a short description for key hints and the command palette.
    pub fn command(&mut self, name: &str, desc: &str, handler: impl Fn(&mut Ctx) + 'static) {
        let cmd = Command {
            name: name.to_string(),
            desc: desc.to_string(),
            module: self.module,
            handler: Box::new(handler),
        };
        if !self.commands.add(cmd) {
            self.error(RegisterErrorKind::DuplicateCommand {
                command: name.to_string(),
            });
            return;
        }
        log::info!(
            "Module [{}] registered command: {} ({})",
            self.module,
            name,
            desc
        );
    }

    /// Binds `key` (Vim notation: `"j"`, `"<C-q>"`, `"<Up>"`) in `mode` to `command`.
    ///
    /// The command may belong to any module; whether it exists is checked after all modules
    /// have registered.
    pub fn keymap(&mut self, mode: Mode, key: &str, command: &str) {
        match parse_key(key) {
            Ok(parsed) => match self.keymap.bind(mode, parsed, command, self.module) {
                Ok(()) => log::info!(
                    "Module [{}] registered keymap: {key} for mode {mode} with command {command}",
                    self.module,
                ),
                Err(bound_to) => self.error(RegisterErrorKind::DuplicateKey {
                    mode,
                    key: parsed,
                    bound_to,
                }),
            },
            Err(reason) => {
                self.error(RegisterErrorKind::InvalidKey {
                    key: key.to_string(),
                    reason,
                });
            }
        }
    }

    /// Adds a gutter column drawn by `provider`. Columns are drawn left to right in the order
    /// they are registered, across all modules.
    pub fn gutter(&mut self, provider: impl GutterProvider) {
        self.gutters.push(self.module, Box::new(provider));
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
    DuplicateCommand {
        /// Name of the command.
        command: String,
    },
    /// The key is already bound in this mode; the first binding is kept.
    DuplicateKey {
        /// Mode of the binding.
        mode: Mode,
        /// The key.
        key: Key,
        /// Command the key is already bound to.
        bound_to: String,
    },
    /// The key notation could not be parsed.
    InvalidKey {
        /// The notation as written by the module.
        key: String,
        /// Why it could not be parsed.
        reason: KeyParseError,
    },
    /// The key is bound to a command that no module registered.
    UnknownCommand {
        /// Mode of the binding.
        mode: Mode,
        /// The key.
        key: Key,
        /// Name of the missing command.
        command: String,
    },
    /// No module registered `editor.quit`, so there would be no way to quit.
    MissingQuitCommand,
    /// The module panicked in [`Module::register`].
    ModulePanicked {
        /// The panic message.
        message: String,
    },
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
            RegisterErrorKind::ModulePanicked { message } => {
                write!(f, "panicked while registering: {message}")
            }
        }
    }
}

impl fmt::Display for ModuleFailure {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "[{}] panicked and was disabled: {}",
            self.module, self.message
        )
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

    #[test]
    fn display_of_every_error_kind() {
        let cases = [
            (
                RegisterErrorKind::DuplicateCommand {
                    command: "vim.move.down".into(),
                },
                "[vim] duplicate command `vim.move.down`",
            ),
            (
                RegisterErrorKind::DuplicateKey {
                    mode: Mode::Normal,
                    key: Key::Char('j'),
                    bound_to: "vim.move.down".into(),
                },
                "[vim] key j (Normal) is already bound to `vim.move.down`",
            ),
            (
                RegisterErrorKind::InvalidKey {
                    key: "<Foo>".into(),
                    reason: KeyParseError::UnknownKey("<Foo>".into()),
                },
                "[vim] invalid key `<Foo>`: unknown key name",
            ),
            (
                RegisterErrorKind::MissingQuitCommand,
                "[vim] command `editor.quit` is not registered, the editor could not be closed",
            ),
            (
                RegisterErrorKind::ModulePanicked {
                    message: "boom".into(),
                },
                "[vim] panicked while registering: boom",
            ),
        ];
        for (kind, expected) in cases {
            let error = RegisterError {
                module: "vim",
                kind,
            };
            assert_eq!(error.to_string(), expected);
        }
    }

    #[test]
    fn module_failure_display_says_the_module_was_disabled() {
        let failure = ModuleFailure {
            module: "gutter",
            message: "index out of bounds".into(),
        };
        assert_eq!(
            failure.to_string(),
            "[gutter] panicked and was disabled: index out of bounds"
        );
    }
}
