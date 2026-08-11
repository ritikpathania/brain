//! Fast indexed lookup structure over CommandMetadata entries.

use crate::ui::command::registry::{CommandMetadata, CommandRegistry};
use std::collections::HashMap;

/// Precompiled index structure for fast fuzzy matching across slash routes, titles, aliases, and keywords.
#[derive(Debug, Clone)]
pub struct CommandIndex {
    entries: Vec<CommandMetadata>,
    id_to_index: HashMap<&'static str, usize>,
}

impl CommandIndex {
    /// Builds a new CommandIndex from a CommandRegistry.
    pub fn build(registry: &CommandRegistry) -> Self {
        let entries = registry.commands().to_vec();
        let mut id_to_index = HashMap::with_capacity(entries.len());
        for (idx, cmd) in entries.iter().enumerate() {
            id_to_index.insert(cmd.id, idx);
        }
        Self {
            entries,
            id_to_index,
        }
    }

    /// Returns all indexed command entries.
    pub fn entries(&self) -> &[CommandMetadata] {
        &self.entries
    }

    /// Looks up entry by command ID.
    pub fn get_by_id(&self, id: &str) -> Option<&CommandMetadata> {
        self.id_to_index.get(id).map(|&idx| &self.entries[idx])
    }
}
