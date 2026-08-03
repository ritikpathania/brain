//! Theme tokens representing semantic UI states and areas.

/// Semantic tokens used to style UI components consistently.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ThemeToken {
    /// Primary color for highlight text, borders, and cursor.
    Primary,
    /// Secondary color for accessory elements.
    Secondary,
    /// Accent color for active sections.
    Accent,
    /// Muted color for passive, inactive, or disabled elements.
    Muted,
    /// Status success color.
    Success,
    /// Status warning color.
    Warning,
    /// Status danger/error color.
    Danger,
    /// Status informational color.
    Info,

    /// Main body/primary text color.
    TextPrimary,
    /// Secondary text color.
    TextSecondary,
    /// Muted text color.
    TextMuted,

    /// Main application title/header style.
    HeaderPrimary,
    /// Section/panel header style.
    HeaderSecondary,

    /// Active item selection style.
    Selection,
    /// Focus outline/border style.
    Focus,

    /// Default panel border style.
    Border,
    /// Active focused border style.
    BorderActive,
    /// Subtle divider/separator border style.
    BorderSubtle,

    /// Text cursor style.
    Cursor,

    /// Inline code style.
    CodeInline,
    /// Fenced code block style.
    CodeBlock,
    /// Hyperlink style.
    Link,
    /// Tag/badge style.
    Tag,

    /// Active thinking state.
    Thinking,
    /// Active text streaming state.
    Streaming,
    /// Style for user messages.
    User,
    /// Style for assistant messages.
    Assistant,
    /// Style for tool messages.
    Tool,
    /// Style for system logs and notifications.
    System,
    /// Background terminal color.
    Background,
    /// Surface panel background color.
    Surface,
}
