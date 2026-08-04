//! Interaction module managing buffers, scroll state, and command dispatchers.

/// Abstract syntax tree definitions for markdown blocks.
pub mod ast;
pub mod chat;
pub mod dispatcher;
pub mod editor;
/// Width-aware layout tree compiler.
pub mod layout_tree;
/// Language syntax highlighting lexer framework.
pub mod lexer;
pub mod markdown;
/// Hierarchical document cursor navigation.
pub mod navigation;
/// Document parser mapping markdown to semantic blocks.
pub mod parser;
pub mod scroll;
/// Session list scroll-and-selection navigator.
pub mod session_navigator;
/// Sidebar navigation component interaction.
pub mod sidebar;
/// Unified event timeline models.
pub mod timeline;

pub use chat::{ChatMessage, ChatState, GenerationState, MessageId, MessageRole};
pub use dispatcher::{DispatchResult, Dispatcher, InteractionContext, UiEvent};
pub use editor::{Cursor, Editor, TextBuffer};
pub use markdown::{MarkdownDocument, MarkdownRenderState};
pub use scroll::{AutoFollowPolicy, ScrollState};
pub use session_navigator::{SessionListItem, SessionNavigator};
pub use sidebar::{
    BrowseState, ParsedQuery, RenameState, SearchState, SessionFilter, SessionLookup, SidebarEvent,
    SidebarInteraction, SidebarMode,
};
pub use timeline::{EventOrdinal, TimelineItem};
