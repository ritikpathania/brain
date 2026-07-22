/// Chat widget showing history message viewport.
pub mod chat;
/// Header widget displaying app title and status.
pub mod header;
/// Interactive knowledge inspector widget.
pub mod inspector;
/// Prompt input buffer bar widget.
pub mod prompt;
/// Reflection subsystem inspection panel widget.
pub mod reflection_panel;
/// Sidebar widget listing session threads.
pub mod sidebar;
/// Status footer widget carrying shortcuts list.
pub mod status;

/// Widget rendering trait.
pub mod brain_widget;
/// Decoupled, immutable data structures for widgets.
pub mod view_models;

/// ChatScreen composer widget.
pub mod chat_screen;
/// CommandHint widget primitives.
pub mod command_hint;
/// Dialog widget primitives.
pub mod dialog;
/// EmptyState widget primitives.
pub mod empty_state;
/// Footer widget primitives.
pub mod footer;
/// List widget primitives.
pub mod list;
/// Panel widget primitives.
pub mod panel;
/// ScrollView widget primitives.
pub mod scroll_view;
/// Section widget primitives.
pub mod section;
/// StatusBar widget primitives.
pub mod status_bar;
/// Toolbar widget primitives.
pub mod toolbar;

pub use chat_screen::ChatScreen;
pub use command_hint::CommandHint;
pub use dialog::Dialog;
pub use empty_state::EmptyState;
pub use footer::Footer;
pub use list::List;
pub use panel::Panel;
pub use scroll_view::ScrollViewWidget;
pub use section::Section;
pub use status_bar::StatusBar;
pub use toolbar::Toolbar;

/// Autocomplete suggestions overlay widget.
pub mod completion;
/// Command Palette overlay widget.
pub mod palette;
/// Modal pinned context overlay widget.
pub mod pinned_overlay;
/// Runtime Dashboard operational control panel widget.
pub mod runtime_dashboard;

pub use runtime_dashboard::{draw_runtime_dashboard, RuntimeDashboardState};
