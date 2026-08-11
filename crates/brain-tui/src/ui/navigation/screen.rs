//! Top-level screen enum navigation targets.

/// First-class screen views within Brain TUI.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Screen {
    /// Landing screen / Daily Home dashboard.
    #[default]
    Home,
    /// Active conversation view.
    Conversation,
    /// Primary Relational Knowledge workspace.
    Workspace,
    /// Interactive Knowledge Graph Explorer.
    GraphExplorer,
    /// Engine Reflection telemetry and log inspector.
    Reflection,
    /// Knowledge Evolution plan reviewer.
    Evolution,
    /// Engine Settings and Preferences screen.
    Settings,
}

impl Screen {
    /// Returns the human-readable panel title.
    pub fn title(self) -> &'static str {
        match self {
            Screen::Home => "Home",
            Screen::Conversation => "Conversation",
            Screen::Workspace => "Knowledge Workspace",
            Screen::GraphExplorer => "Graph Explorer",
            Screen::Reflection => "Reflection Logs",
            Screen::Evolution => "Knowledge Evolution",
            Screen::Settings => "Settings",
        }
    }
}
