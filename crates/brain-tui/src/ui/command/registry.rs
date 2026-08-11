//! Data-driven CommandRegistry with keyword alias matching, typed icons, and availability rules.

/// Availability rules governing command eligibility.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum CommandAvailability {
    /// Always available in any screen or modal state.
    #[default]
    Always,
    /// Requires an active workspace context.
    RequiresWorkspace,
    /// Requires an active memory session.
    RequiresSession,
    /// Requires an active daemon IPC connection.
    RequiresDaemon,
}

/// Category grouping for command palette entries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum CommandCategory {
    /// Session commands (new, list, rename).
    Session,
    /// Memory commands (stewardship, graph, retrieval).
    Memory,
    /// Knowledge commands (concepts, entities).
    Knowledge,
    /// Workspace commands (settings, file tree).
    Workspace,
    /// Appearance commands (themes, modes).
    Appearance,
    /// Diagnostics commands (health, stats, telemetry).
    Diagnostics,
    /// General system commands (help, about).
    System,
}

impl CommandCategory {
    /// Returns static string label for the category.
    pub fn label(self) -> &'static str {
        match self {
            CommandCategory::Session => "Session",
            CommandCategory::Memory => "Memory",
            CommandCategory::Knowledge => "Knowledge",
            CommandCategory::Workspace => "Workspace",
            CommandCategory::Appearance => "Appearance",
            CommandCategory::Diagnostics => "Diagnostics",
            CommandCategory::System => "System",
        }
    }
}

/// Strongly-typed icon indicator enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CommandIcon {
    /// Session action icon.
    Session,
    /// Search retrieval icon.
    Search,
    /// Memory stewardship icon.
    Memory,
    /// Knowledge graph icon.
    Knowledge,
    /// Theme / appearance icon.
    Theme,
    /// Workspace settings icon.
    Settings,
    /// System diagnostics icon.
    Diagnostics,
}

impl CommandIcon {
    /// Resolves theme-aware glyph or Unicode icon.
    pub fn glyph(self) -> &'static str {
        match self {
            CommandIcon::Session => "▶",
            CommandIcon::Search => "🔍",
            CommandIcon::Memory => "🧠",
            CommandIcon::Knowledge => "🕸",
            CommandIcon::Theme => "🎨",
            CommandIcon::Settings => "⚙",
            CommandIcon::Diagnostics => "📊",
        }
    }
}

/// Rich metadata declaration for a command palette entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandMetadata {
    /// Unique command identifier string (e.g. "session.new").
    pub id: &'static str,
    /// Slash command route (e.g. "/session new").
    pub name: &'static str,
    /// Display title shown in palette list.
    pub title: &'static str,
    /// Detailed description shown in command drawer card.
    pub description: &'static str,
    /// Category grouping.
    pub category: CommandCategory,
    /// Typed icon indicator.
    pub icon: CommandIcon,
    /// Keyword aliases for fuzzy search matching.
    pub keywords: Vec<&'static str>,
    /// Secondary command aliases (e.g. "new", "create").
    pub aliases: Vec<&'static str>,
    /// Optional primary keyboard shortcut hint.
    pub shortcut: Option<&'static str>,
    /// Availability constraint rule.
    pub availability: CommandAvailability,
    /// Static author-controlled priority weight (higher = ranks earlier).
    pub priority: u16,
}

/// Central registry owning all registered application commands.
pub struct CommandRegistry {
    commands: Vec<CommandMetadata>,
}

impl Default for CommandRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl CommandRegistry {
    /// Initializes CommandRegistry with core built-in commands.
    pub fn new() -> Self {
        let core_commands = vec![
            CommandMetadata {
                id: "session.new",
                name: "/session new",
                title: "New Session",
                description: "Start a new reasoning session in active workspace.",
                category: CommandCategory::Session,
                icon: CommandIcon::Session,
                keywords: vec!["session", "new", "create", "start", "workspace"],
                aliases: vec!["new", "create"],
                shortcut: Some("Ctrl+N"),
                availability: CommandAvailability::Always,
                priority: 100,
            },
            CommandMetadata {
                id: "search.memory",
                name: "/search",
                title: "Search Memory",
                description: "Search relational memory and knowledge graph with hybrid retrieval.",
                category: CommandCategory::Memory,
                icon: CommandIcon::Search,
                keywords: vec!["search", "memory", "knowledge", "find", "vector", "fts"],
                aliases: vec!["find", "query", "memories"],
                shortcut: Some("Ctrl+F"),
                availability: CommandAvailability::Always,
                priority: 90,
            },
            CommandMetadata {
                id: "memory.inspect",
                name: "/memory",
                title: "Inspect Graph",
                description: "Inspect & manage long-term knowledge entities.",
                category: CommandCategory::Knowledge,
                icon: CommandIcon::Knowledge,
                keywords: vec!["memory", "graph", "inspect", "entities", "stewardship"],
                aliases: vec!["graph", "entities"],
                shortcut: Some("Ctrl+M"),
                availability: CommandAvailability::Always,
                priority: 85,
            },
            CommandMetadata {
                id: "theme.change",
                name: "/theme",
                title: "Change Theme",
                description: "Open Theme Selector to switch appearance palette.",
                category: CommandCategory::Appearance,
                icon: CommandIcon::Theme,
                keywords: vec!["theme", "appearance", "color", "dark", "light", "mode"],
                aliases: vec!["appearance", "color"],
                shortcut: Some("Ctrl+T"),
                availability: CommandAvailability::Always,
                priority: 70,
            },
            CommandMetadata {
                id: "system.help",
                name: "/help",
                title: "Shortcuts & Help",
                description: "Show available slash commands & keybindings.",
                category: CommandCategory::System,
                icon: CommandIcon::Diagnostics,
                keywords: vec!["help", "shortcuts", "commands", "docs", "keybindings"],
                aliases: vec!["shortcuts", "keys"],
                shortcut: Some("F1"),
                availability: CommandAvailability::Always,
                priority: 80,
            },
            CommandMetadata {
                id: "chat.clear",
                name: "/clear",
                title: "Clear Chat",
                description: "Clear current conversation history.",
                category: CommandCategory::Session,
                icon: CommandIcon::Session,
                keywords: vec!["clear", "reset", "clean", "chat"],
                aliases: vec!["clear", "reset"],
                shortcut: Some("Ctrl+L"),
                availability: CommandAvailability::Always,
                priority: 95,
            },
        ];

        Self {
            commands: core_commands,
        }
    }

    /// Registers a new command into the registry.
    pub fn register(&mut self, metadata: CommandMetadata) {
        self.commands.push(metadata);
    }

    /// Returns a slice of all registered command metadata.
    pub fn commands(&self) -> &[CommandMetadata] {
        &self.commands
    }

    /// Looks up command metadata by unique command identifier.
    pub fn get_by_id(&self, id: &str) -> Option<&CommandMetadata> {
        self.commands.iter().find(|cmd| cmd.id == id)
    }

    /// Looks up command metadata by slash name route.
    pub fn get_by_name(&self, name: &str) -> Option<&CommandMetadata> {
        self.commands.iter().find(|cmd| cmd.name == name)
    }
}
