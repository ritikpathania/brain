//! Immutable data-driven view models for stateless widgets.

/// Layout limits for stateless widgets.
pub const MAX_SHORTCUTS: usize = 8;
/// Maximum tab entries supported by the toolbar.
pub const MAX_TABS: usize = 8;
/// Maximum action buttons supported by confirmation dialogs.
pub const MAX_DIALOG_BUTTONS: usize = 4;
/// Maximum visible rows supported by the vertical list.
pub const MAX_VISIBLE_LIST_ROWS: usize = 32;
/// Maximum visible lines supported by the ScrollView viewport.
pub const MAX_VISIBLE_SCROLL_ROWS: usize = 64;

/// Classification of StatusBar states.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusKind {
    /// Idle/passive state.
    Idle,
    /// System is processing/thinking.
    Working,
    /// Streaming content.
    Streaming,
    /// Error status.
    Error,
    /// Offline state.
    Offline,
}

/// View model for the StatusBar widget.
pub struct StatusBarView<'a> {
    /// Active session title.
    pub title: &'a str,
    /// Semantic status classifier.
    pub kind: StatusKind,
    /// Detailed status message.
    pub message: &'a str,
}

/// A single shortcut hint.
#[derive(Debug, Clone, Copy)]
pub struct ShortcutHint<'a> {
    /// Shortcut hotkey trigger label.
    pub key: &'a str,
    /// User action description.
    pub description: &'a str,
}

/// View model for the Footer widget.
pub struct FooterView<'a> {
    /// List of registered shortcut keys and actions.
    pub shortcuts: &'a [ShortcutHint<'a>],
}

/// Classification of active focused state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusState {
    /// Panel has input focus.
    Focused,
    /// Panel is visible but inactive.
    Inactive,
    /// Panel is disabled.
    Disabled,
}

/// View model for the Panel container widget.
pub struct PanelView<'a> {
    /// Container title label.
    pub title: &'a str,
    /// Panel focus state.
    pub focus: FocusState,
}

/// Classification of Dialog button kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ButtonKind {
    /// Primary action confirmation button.
    Primary,
    /// Dismiss action button.
    Secondary,
    /// High-risk action button.
    Danger,
}

/// Dialog action button view structure.
#[derive(Debug, Clone, Copy)]
pub struct DialogButton<'a> {
    /// Action label text.
    pub label: &'a str,
    /// Selection category.
    pub kind: ButtonKind,
    /// Clickable status.
    pub enabled: bool,
}

/// View model for Dialog prompt widgets.
pub struct DialogView<'a> {
    /// Modal header title.
    pub title: &'a str,
    /// Prompt description text.
    pub message: &'a str,
    /// Action choices.
    pub buttons: &'a [DialogButton<'a>],
    /// Highlighted choice index.
    pub selected_index: usize,
}

/// View model for Section dividers.
pub struct SectionView<'a> {
    /// Section name/header label.
    pub title: &'a str,
    /// Expanded status.
    pub collapsed: bool,
}

/// View model structure for a single toolbar tab entry.
#[derive(Debug, Clone, Copy)]
pub struct TabView<'a> {
    /// Tab label text.
    pub title: &'a str,
    /// Active highlight state.
    pub active: bool,
}

/// View model for tab Toolbar headers.
pub struct ToolbarView<'a> {
    /// Available tab view details.
    pub tabs: &'a [TabView<'a>],
}

/// Representation model for individual select list items.
#[derive(Debug, Clone, Copy)]
pub struct ListItem<'a> {
    /// Label text.
    pub label: &'a str,
    /// Selection status.
    pub selected: bool,
    /// Enabled/disabled status.
    pub disabled: bool,
}

/// View model for Lists.
pub struct ListView<'a> {
    /// Collection of list items.
    pub items: &'a [ListItem<'a>],
}

/// ScrollView representation model.
pub struct ScrollViewModel<'a> {
    /// Content lines to show inside viewport.
    pub lines: &'a [&'a str],
    /// Scroll offset.
    pub scroll_offset: usize,
}

/// View model for CommandHint helper popup.
pub struct CommandHintView<'a> {
    /// Suggestion query snippet.
    pub command: &'a str,
    /// Detailed usage information parameter template.
    pub usage: &'a str,
}

/// View model structure for Empty state containers.
pub struct EmptyStateView<'a> {
    /// Error/information title header.
    pub title: &'a str,
    /// Description message.
    pub description: &'a str,
    /// Icon indicator symbol template.
    pub icon: &'static str,
}

/// Target panels that can receive key focus.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusTarget {
    /// Active message stream scroll area.
    Conversation,
    /// Prompt input bar.
    Prompt,
    /// Session list sidebar.
    Sidebar,
    /// Command Palette overlay.
    CommandPalette,
    /// Modal dialog overlay.
    Dialog,
}

/// Connection states mapped to connectivity icons.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionState {
    /// Normal connected state.
    Connected,
    /// Reconnecting state.
    Connecting,
    /// Disconnected state.
    Offline,
    /// Connection failure.
    Error,
}

/// Semantic view model representing the chat screen state.
pub struct ChatScreenView<'a> {
    /// Active session thread title.
    pub session_title: &'a str,
    /// Connection status classification.
    pub connection: ConnectionState,
    /// Whether the background daemon is processing commands.
    pub is_working: bool,
    /// Number of message entries.
    pub message_count: usize,
    /// Text current input buffer.
    pub input_buffer: &'a str,
    /// Focused panel category.
    pub focus: FocusTarget,
}
