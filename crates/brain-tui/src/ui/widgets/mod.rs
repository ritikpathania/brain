/// Chat widget showing history message viewport.
pub mod chat;
/// Knowledge Compiler subsystem inspection panel widget.
pub mod compiler_panel;
/// Header widget displaying app title and status.
pub mod header;
/// Composable Home dashboard widgets.
pub mod home;
/// Bounded welcome component with integrated title and 2-column split.
pub mod home_welcome;
/// Right-aligned ambient status line widget.
pub mod ambient_status;
/// Interactive knowledge inspector widget.
pub mod inspector;
/// Markdown block cache enforcing zero-flicker block immutability.
pub mod markdown_cache;
/// Static Raven/Owl mascot identity widget.
pub mod mascot;
/// Memory stewardship list widget.
pub mod memory_list;
/// Prompt input buffer bar widget.
pub mod prompt;
/// Diagnostic reasoning plan execution DAG widget.
pub mod reasoning_plan;
/// Reasoning progress visualization widget and observer state machine.
pub mod reasoning_progress;
/// Reflection subsystem inspection panel widget.
pub mod reflection_panel;
/// ScrollAnchor state machine.
pub mod scroll_anchor;
/// Sidebar widget listing session threads.
pub mod sidebar;

/// Confidence badge widget.
pub mod confidence_badge;
/// Generic Document Inspector modal widget.
pub mod document_inspector;
/// Scannable bordered EvidenceCard widget.
pub mod evidence_card;

/// Pure Canvas widget rendering PositionedGraph.
pub mod graph_canvas;
/// Node Inspector drawer widget.
pub mod node_inspector;

/// Side-by-side contradiction card widget.
pub mod contradiction_card;
/// Reflection dashboard overview widget.
pub mod reflection_dashboard;
/// Stewardship list widget.
pub mod stewardship_list;

/// Evolution overview list widget.
pub mod evolution_overview;
/// Semantic graph diff viewer widget.
pub mod proposal_diff;

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
/// Causal concept explainability timeline screen widget.
pub mod explainability;
/// Interactive Reflection review and proposal action screen widget.
pub mod interactive_reflection;
/// Knowledge Automation orchestration screen widget.
pub mod knowledge_automation;
/// Knowledge Evolution governance policy & planning screen widget.
pub mod knowledge_evolution;
/// Knowledge Graph Explorer read-only inspection screen widget.
pub mod knowledge_explorer;
/// Command Palette overlay widget.
pub mod palette;
/// Modal pinned context overlay widget.
pub mod pinned_overlay;
/// Runtime Dashboard operational control panel widget.
pub mod runtime_dashboard;
/// Shared ScreenState interface trait.
pub mod screen_state;

pub use explainability::{
    draw_explainability_screen, ExplainabilityIntent, ExplainabilityState, ExplanationNavigator,
};
pub use interactive_reflection::{
    draw_interactive_reflection_screen, InteractiveReflectionIntent, InteractiveReflectionState,
    ProposalDispatchState, ReflectionProposalNavigator,
};
pub use knowledge_automation::{
    draw_knowledge_automation_screen, KnowledgeAutomationIntent, KnowledgeAutomationNavigator,
    KnowledgeAutomationState,
};
pub use knowledge_evolution::{
    draw_knowledge_evolution_screen, KnowledgeEvolutionIntent, KnowledgeEvolutionNavigator,
    KnowledgeEvolutionState,
};
pub use knowledge_explorer::{draw_knowledge_explorer, ExplorerIntent, KnowledgeExplorerState};
pub use runtime_dashboard::{draw_runtime_dashboard, RuntimeDashboardState};
pub use screen_state::ScreenState;
