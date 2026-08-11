//! Composable Home dashboard widgets.

pub mod checklist;
pub mod memory_context;
pub mod quick_actions;
pub mod recent_sessions;
pub mod system_status;
pub mod welcome;

pub use checklist::{ChecklistItem, ChecklistWidget};
pub use memory_context::MemoryContextWidget;
pub use quick_actions::{QuickActionItem, QuickActionsWidget};
pub use recent_sessions::RecentSessionsWidget;
pub use system_status::SystemStatusWidget;
pub use welcome::WelcomeWidget;
