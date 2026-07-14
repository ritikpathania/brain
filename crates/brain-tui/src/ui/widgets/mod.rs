/// Header widget displaying app title and status.
pub mod header;
/// Chat widget showing history message viewport.
pub mod chat;
/// Prompt input buffer bar widget.
pub mod prompt;
/// Status footer widget carrying shortcuts list.
pub mod status;
/// Sidebar widget listing session threads.
pub mod sidebar;
/// Interactive knowledge inspector widget.
pub mod inspector;

/// Decoupled, immutable data structures for widgets.
pub mod view_models;
/// Widget rendering trait.
pub mod brain_widget;

/// StatusBar widget primitives.
pub mod status_bar;
/// Footer widget primitives.
pub mod footer;
/// Panel widget primitives.
pub mod panel;
/// Dialog widget primitives.
pub mod dialog;
/// Section widget primitives.
pub mod section;
/// Toolbar widget primitives.
pub mod toolbar;
/// List widget primitives.
pub mod list;
/// ScrollView widget primitives.
pub mod scroll_view;
/// CommandHint widget primitives.
pub mod command_hint;
/// EmptyState widget primitives.
pub mod empty_state;
/// ChatScreen composer widget.
pub mod chat_screen;

pub use status_bar::StatusBar;
pub use footer::Footer;
pub use panel::Panel;
pub use dialog::Dialog;
pub use section::Section;
pub use toolbar::Toolbar;
pub use list::List;
pub use scroll_view::ScrollViewWidget;
pub use command_hint::CommandHint;
pub use empty_state::EmptyState;
pub use chat_screen::ChatScreen;

/// Autocomplete suggestions overlay widget.
pub mod completion;
/// Command Palette overlay widget.
pub mod palette;
/// Modal pinned context overlay widget.
pub mod pinned_overlay;

