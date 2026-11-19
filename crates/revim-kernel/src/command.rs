//! Commands: named actions and their registry.

use std::collections::HashMap;

use crate::ctx::Ctx;

/// The function run when a command is executed.
pub type CommandFn = Box<dyn Fn(&mut Ctx, &str)>;

/// A named action that keys, other commands and modules can run.
pub struct Command {
    /// Unique name, by convention `module.object.action` (e.g. `vim.move.down`).
    pub name: String,
    /// Short description shown in key hints and the command palette.
    pub desc: String,
    /// Id of the module that registered the command (disabled together with it).
    pub(crate) module: &'static str,
    pub(crate) handler: CommandFn,
}

/// All registered commands, by name.
#[derive(Default)]
pub struct CommandRegistry {
    commands: HashMap<String, Command>,
}
impl CommandRegistry {
    /// Adds `cmd`. Returns `false` and keeps the existing command if one with the same
    /// name is already registered.
    pub(crate) fn add(&mut self, cmd: Command) -> bool {
        if self.commands.contains_key(&cmd.name) {
            return false;
        }
        self.commands.insert(cmd.name.clone(), cmd);
        true
    }
    /// The command with this name.
    pub(crate) fn get(&self, name: &str) -> Option<&Command> {
        self.commands.get(name)
    }
    /// Whether a command with this name is registered.
    pub(crate) fn contains(&self, name: &str) -> bool {
        self.commands.contains_key(name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cmd(name: &str, desc: &str) -> Command {
        Command {
            name: name.into(),
            desc: desc.into(),
            module: "test",
            handler: Box::new(|_, _| {}),
        }
    }

    #[test]
    fn add_and_get() {
        let mut reg = CommandRegistry::default();
        assert!(reg.add(cmd("a.x", "first")));
        assert!(reg.contains("a.x"));
        assert_eq!(reg.get("a.x").map(|c| c.desc.as_str()), Some("first"));
        assert!(reg.get("a.y").is_none());
    }

    #[test]
    fn duplicate_keeps_the_first_command() {
        let mut reg = CommandRegistry::default();
        reg.add(cmd("a.x", "first"));
        assert!(!reg.add(cmd("a.x", "second")));
        assert_eq!(reg.get("a.x").map(|c| c.desc.as_str()), Some("first"));
    }
}
