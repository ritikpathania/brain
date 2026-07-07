//! Interaction module managing buffers, scroll state, and command dispatchers.

pub mod editor;
pub mod scroll;
pub mod dispatcher;
pub mod chat;
pub mod markdown;
/// Sidebar navigation component interaction.
pub mod sidebar;
/// Abstract syntax tree definitions for markdown blocks.
pub mod ast;
/// Document parser mapping markdown to semantic blocks.
pub mod parser;
/// Language syntax highlighting lexer framework.
pub mod lexer;
/// Width-aware layout tree compiler.
pub mod layout_tree;

pub use editor::{TextBuffer, Cursor, Editor};
pub use scroll::{ScrollState, AutoFollowPolicy};
pub use dispatcher::{InteractionContext, DispatchResult, Dispatcher, UiEvent};
pub use chat::{MessageId, MessageRole, GenerationState, ChatMessage, ChatState};
pub use markdown::{MarkdownDocument, MarkdownRenderState};
pub use sidebar::{SessionFilter, SidebarMode, BrowseState, ParsedQuery, SearchState, RenameState, SidebarInteraction, SidebarEvent, SessionLookup};
