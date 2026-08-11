/// Layout and constraints drawing coordinator.
pub mod renderer;

/// Status footer widget.
pub mod status_footer;

/// Semantic style definitions.
pub mod theme;

/// Stateless layout widgets and ViewModels.
pub mod widgets;

/// Encapsulated presentation view models.
pub mod view_models;

/// Stateless rendering helpers.
pub mod render;

/// Stateless UI primitives.
pub mod primitives;

/// Layout and geometry calculation module.
pub mod layout;

/// Read-only reasoning trace diagnostic visualizer.
pub mod reasoning_trace_widget;
pub use reasoning_trace_widget::*;

/// Navigation, modal, and shortcut routing.
pub mod navigation;

/// Screen composition trait.
pub mod screen;

/// Full-viewport screen components.
pub mod screens;

/// Focus manager.
pub mod focus;

/// Screen router.
pub mod router;

/// Input event router.
pub mod input;

/// Interaction module.
pub mod interaction;

/// Protocol definitions.
pub mod protocol;

/// Unified application state.
pub mod state;

/// Scheduler module.
pub mod scheduler;

/// Application orchestrator service.
pub mod application;

/// Command Palette and Slash Commands pipeline.
pub mod command;

/// Unified Global Search omnibox and providers.
pub mod search;
