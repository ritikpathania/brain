//! Command palette state machine and interaction data structures.

use crate::ui::command::{
    CommandDescriptor, CommandId, ModelId, ParameterDescriptor, ParameterId, ThemeId,
};
use crate::ui::interaction::Editor;
use brain_domain::SessionId;

/// A single collected parameter with its identifier and typed value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CollectedParameter {
    /// Opaque identifier of the parameter.
    pub id: ParameterId,
    /// Collected value of the parameter.
    pub value: ParameterValue,
}

/// Collection state representing values accumulated so far.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParameterCollectionState {
    /// Opaque command identifier being populated.
    pub command_id: CommandId,
    /// Vector of collected parameters.
    pub collected: Vec<CollectedParameter>,
}

impl ParameterCollectionState {
    /// Creates a new ParameterCollectionState.
    pub fn new(command_id: CommandId) -> Self {
        Self {
            command_id,
            collected: Vec::new(),
        }
    }

    /// Resolves the descriptor of the parameter currently being collected.
    pub fn current_parameter<'a>(
        &self,
        descriptor: &'a CommandDescriptor,
    ) -> Option<&'a ParameterDescriptor> {
        descriptor.parameters.get(self.collected.len())
    }
}

/// Stages of the multi-step command palette workflow.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PaletteStage {
    /// Filtering the static list of commands.
    Search,
    /// Collecting parameter inputs.
    CollectParameter(ParameterCollectionState),
    /// Confirming execution (e.g. for destructive actions) before building the invocation.
    Confirm {
        /// The command being confirmed.
        command_id: CommandId,
        /// The arguments collected so far.
        arguments: ParameterCollectionState,
    },
}

/// Strongly-typed parameter values.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParameterValue {
    /// Text value.
    String(String),
    /// Boolean toggle.
    Boolean(bool),
    /// Theme selection.
    Theme(ThemeId),
    /// Session selection.
    Session(SessionId),
    /// Model selection.
    Model(ModelId),
}

/// Represents the interaction and rendering state of the command palette overlay.
pub struct CommandPaletteState {
    /// Whether the overlay is currently open and visible.
    pub open: bool,
    /// Input editor for command query or parameter collection.
    pub editor: Editor,
    /// Index of the selected command/option in listing.
    pub selected_index: usize,
    /// Current workflow stage.
    pub stage: PaletteStage,
    /// Pluggable search controller.
    pub search_controller: Option<crate::ui::search::controller::SearchController>,
    /// Pluggable search aggregator.
    pub search_aggregator: Option<crate::ui::search::aggregator::SearchAggregator>,
    /// Precompiled view state snapshot.
    pub view_state: crate::ui::search::types::SearchViewState,
}

impl CommandPaletteState {
    /// Instantiates a new CommandPaletteState in the closed search stage.
    pub fn new() -> Self {
        Self {
            open: false,
            editor: Editor::new(),
            selected_index: 0,
            stage: PaletteStage::Search,
            search_controller: None,
            search_aggregator: None,
            view_state: crate::ui::search::types::SearchViewState::default(),
        }
    }

    /// Initializes pluggable search providers and controllers.
    pub fn initialize(
        &mut self,
        client: std::sync::Arc<dyn crate::client::ExecutionClient>,
        sink: std::sync::Arc<dyn crate::ui::search::types::SearchEventSink>,
    ) {
        let immediate_providers: Vec<std::sync::Arc<dyn crate::ui::search::types::SearchProvider>> = vec![
            std::sync::Arc::new(crate::ui::search::providers::CommandsProvider),
            std::sync::Arc::new(crate::ui::search::providers::SessionsProvider),
            std::sync::Arc::new(crate::ui::search::providers::LocalMessagesProvider),
        ];
        let async_providers: Vec<std::sync::Arc<dyn crate::ui::search::types::SearchProvider>> =
            vec![std::sync::Arc::new(
                crate::ui::search::providers::RemoteMessagesProvider::new(client),
            )];
        let expected_providers = vec![
            crate::ui::search::types::PROVIDER_COMMANDS,
            crate::ui::search::types::PROVIDER_SESSIONS,
            crate::ui::search::types::PROVIDER_LOCAL_MESSAGES,
            crate::ui::search::types::PROVIDER_REMOTE_MESSAGES,
        ];

        self.search_controller = Some(crate::ui::search::controller::SearchController::new(
            immediate_providers,
            async_providers,
            sink,
        ));
        self.search_aggregator = Some(crate::ui::search::aggregator::SearchAggregator::new(
            expected_providers,
        ));
        self.view_state = crate::ui::search::types::SearchViewState::default();
    }

    /// Triggers a new search query execution.
    pub fn trigger_search(
        &mut self,
        text: String,
        context: &crate::ui::search::types::SearchContext,
    ) {
        if let Some(ref mut agg) = self.search_aggregator {
            agg.set_query(text.clone());
        }
        if let Some(ref mut controller) = self.search_controller {
            controller.search(text, context);
        }
        if let Some(ref agg) = self.search_aggregator {
            self.view_state = agg.view_state();
        }
    }

    /// Resets the palette state to closed and back to search stage.
    pub fn reset(&mut self) {
        self.open = false;
        self.editor = Editor::new();
        self.selected_index = 0;
        self.stage = PaletteStage::Search;
        if let Some(ref mut controller) = self.search_controller {
            controller.cancel();
        }
        if let Some(ref mut agg) = self.search_aggregator {
            agg.reset();
        }
        self.view_state = crate::ui::search::types::SearchViewState::default();
    }

    /// Return the ranked, active search results.
    pub fn results(&self) -> &[crate::ui::search::types::SearchResult] {
        self.view_state.results()
    }

    /// Filter COMMANDS matching the current search term.
    pub fn matches(&self) -> impl Iterator<Item = &'static crate::ui::command::CommandDescriptor> {
        let term = self.editor.text().to_lowercase();
        crate::ui::command::COMMANDS.iter().filter(move |cmd| {
            cmd.visibility != crate::ui::command::CommandVisibility::SlashOnly
                && (cmd.title.to_lowercase().contains(&term)
                    || cmd
                        .aliases
                        .iter()
                        .any(|alias| alias.to_lowercase().contains(&term))
                    || cmd
                        .keywords
                        .iter()
                        .any(|kw| kw.to_lowercase().contains(&term)))
        })
    }
}

impl crate::ui::layout::Overlay for CommandPaletteState {
    fn is_visible(&self) -> bool {
        self.open
    }

    fn geometry(&self, screen_area: ratatui::layout::Rect) -> ratatui::layout::Rect {
        crate::ui::layout::CommandPaletteGeometry::compute(screen_area)
    }
}
