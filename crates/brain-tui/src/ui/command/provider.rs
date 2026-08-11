//! Pluggable PaletteProvider trait and CommandProvider implementation.

use crate::ui::command::index::CommandIndex;
use crate::ui::command::matcher::FuzzyMatcher;
use crate::ui::command::ranker::CommandRanker;
use crate::ui::command::registry::{CommandCategory, CommandIcon};

/// Structured item rendered in palette list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaletteItem {
    /// Command identifier (e.g. "session.new").
    pub id: &'static str,
    /// Slash command string (e.g. "/session new").
    pub name: &'static str,
    /// Display title label.
    pub title: &'static str,
    /// Brief description.
    pub description: &'static str,
    /// Category grouping.
    pub category: CommandCategory,
    /// Typed icon glyph.
    pub icon: CommandIcon,
    /// Optional shortcut hint (e.g. "Ctrl+N").
    pub shortcut: Option<&'static str>,
}

/// Category-grouped section of palette items.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaletteSection {
    /// Header title label for category grouping.
    pub title: &'static str,
    /// Ordered list of palette items inside category.
    pub items: Vec<PaletteItem>,
}

/// Generic provider trait powering the Command Palette.
pub trait PaletteProvider {
    /// Queries provider and yields category-grouped sections.
    fn query(&self, query: &str) -> Vec<PaletteSection>;
}

/// Core built-in command provider conforming to PaletteProvider.
pub struct CommandProvider<'a> {
    index: &'a CommandIndex,
}

impl<'a> CommandProvider<'a> {
    /// Creates a new CommandProvider using the supplied CommandIndex.
    pub fn new(index: &'a CommandIndex) -> Self {
        Self { index }
    }
}

impl<'a> PaletteProvider for CommandProvider<'a> {
    fn query(&self, query: &str) -> Vec<PaletteSection> {
        let mut matches = FuzzyMatcher::match_query(self.index, query);
        CommandRanker::rank(&mut matches);

        let mut sections: Vec<PaletteSection> = Vec::new();

        for candidate in matches {
            let item = PaletteItem {
                id: candidate.metadata.id,
                name: candidate.metadata.name,
                title: candidate.metadata.title,
                description: candidate.metadata.description,
                category: candidate.metadata.category,
                icon: candidate.metadata.icon,
                shortcut: candidate.metadata.shortcut,
            };

            let cat_label = candidate.metadata.category.label();
            if let Some(sec) = sections.iter_mut().find(|s| s.title == cat_label) {
                sec.items.push(item);
            } else {
                sections.push(PaletteSection {
                    title: cat_label,
                    items: vec![item],
                });
            }
        }

        sections
    }
}
