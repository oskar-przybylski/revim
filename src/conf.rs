use revim_api::Module;

/// Modules loaded at startup, in order. Remove a line to disable a module;
pub fn modules() -> Vec<Box<dyn Module>> {
    vec![Box::new(mod_builtin::Builtin), Box::new(mod_vim::Vim)]
}

#[cfg(test)]
mod tests {
    use super::*;
    use revim_api::testing::Editor;
    use revim_api::{Document, EditorState};

    #[test]
    fn default_modules_register_without_errors() {
        let state = EditorState::with_document(Document::scratch());
        if let Err(errors) = Editor::new(state, modules()) {
            let list: Vec<String> = errors.iter().map(ToString::to_string).collect();
            panic!("module registration failed:\n{}", list.join("\n"));
        }
    }
}
