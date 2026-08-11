//! Command palette state machine, interaction data structures, and observational telemetry.

use crate::ui::command::provider::{PaletteItem, PaletteProvider, PaletteSection};
use crate::ui::command::registry::CommandMetadata;
use crate::ui::command::{
    CommandDescriptor, CommandId, ModelId, ParameterDescriptor, ParameterId, ThemeId,
};
use crate::ui::interaction::Editor;
use brain_domain::SessionId;

/// Lightweight observational telemetry recorded during palette interactions.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PaletteTelemetry {
    /// Query string length when command was accepted or query changed.
    pub query_length: usize,
    /// Number of candidate matches available for the query.
    pub candidate_count: usize,
    /// Selected index when command was accepted.
    pub selected_index: usize,
    /// Accepted command identifier string (if executed).
    pub accepted_command: Option<String>,
}

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
    /// Index of the selected command/option in flat list.
    pub selected_index: usize,
    /// Current workflow stage.
    pub stage: PaletteStage,
    /// Observational telemetry recorded during the current palette session.
    pub telemetry: PaletteTelemetry,
    /// Pluggable search controller.
    pub search_controller: Option<crate::ui::search::controller::SearchController>,
    /// Pluggable search aggregator.
    pub search_aggregator: Option<crate::ui::search::aggregator::SearchAggregator>,
    /// Precompiled view state snapshot.
    pub view_state: crate::ui::search::types::SearchViewState,
}

impl Default for CommandPaletteState {
    fn default() -> Self {
        Self::new()
    }
}

impl CommandPaletteState {
    /// Constructs a clean `CommandPaletteState`.
    pub fn new() -> Self {
        Self {
            open: false,
            editor: Editor::new(),
            selected_index: 0,
            stage: PaletteStage::Search,
            telemetry: PaletteTelemetry::default(),
            search_controller: None,
            search_aggregator: None,
            view_state: crate::ui::search::types::SearchViewState::default(),
        }
    }

    /// Backward-compatible initialization helper.
    pub fn initialize<T: Send + Sync + 'static + ?Sized, S: Send + Sync + 'static + ?Sized>(
        &mut self,
        _client: std::sync::Arc<T>,
        _sink: std::sync::Arc<S>,
    ) {
    }

    /// Resets the palette state to closed/empty.
    pub fn reset(&mut self) {
        self.close();
    }

    /// Opens the command palette overlay with empty query or optional initial query.
    pub fn open_with_query(&mut self, initial_query: Option<&str>) {
        self.open = true;
        self.editor = Editor::new();
        if let Some(q) = initial_query {
            for ch in q.chars() {
                self.editor.insert_char(ch);
            }
        }
        self.selected_index = 0;
        self.stage = PaletteStage::Search;
        self.telemetry = PaletteTelemetry {
            query_length: self.editor.text().len(),
            candidate_count: 0,
            selected_index: 0,
            accepted_command: None,
        };
    }

    /// Closes the command palette overlay.
    pub fn close(&mut self) {
        self.open = false;
        self.editor.clear();
        self.selected_index = 0;
        self.stage = PaletteStage::Search;
    }

    /// Toggles command palette visibility.
    pub fn toggle(&mut self) {
        if self.open {
            self.close();
        } else {
            self.open_with_query(None);
        }
    }

    /// Triggers search processing for a query text string.
    pub fn trigger_search<S: AsRef<str>>(
        &mut self,
        query: S,
        _context: &crate::ui::search::types::SearchContext,
    ) {
        self.editor.clear();
        for ch in query.as_ref().chars() {
            self.editor.insert_char(ch);
        }
    }

    /// Returns search results view state.
    pub fn results(&self) -> Vec<crate::ui::search::types::SearchResult> {
        self.view_state.results().to_vec()
    }

    /// Returns matching command metadata for backward compatibility.
    pub fn matches(&self) -> Vec<CommandMetadata> {
        let registry = crate::ui::command::registry::CommandRegistry::new();
        let index = crate::ui::command::index::CommandIndex::build(&registry);
        let provider = crate::ui::command::provider::CommandProvider::new(&index);
        let (_, flat) = self.query_provider(&provider);
        flat.into_iter()
            .filter_map(|item| registry.get_by_id(item.id).cloned())
            .collect()
    }

    /// Queries a `PaletteProvider` and flattens matching sections for rendering.
    pub fn query_provider(
        &self,
        provider: &dyn PaletteProvider,
    ) -> (Vec<PaletteSection>, Vec<PaletteItem>) {
        let sections = provider.query(self.editor.text());
        let mut flat_items = Vec::new();
        for sec in &sections {
            for item in &sec.items {
                flat_items.push(item.clone());
            }
        }
        (sections, flat_items)
    }

    /// Moves palette selection down with wrapped boundary bounds.
    pub fn move_selection_down(&mut self) {
        let count = self.matches().len();
        if count > 0 {
            self.selected_index = (self.selected_index + 1) % count;
        }
    }

    /// Moves palette selection up with wrapped boundary bounds.
    pub fn move_selection_up(&mut self) {
        let count = self.matches().len();
        if count > 0 {
            if self.selected_index == 0 {
                self.selected_index = count - 1;
            } else {
                self.selected_index -= 1;
            }
        }
    }

    /// Returns active selected command title string.
    pub fn selected_command_title(&self) -> String {
        self.matches()
            .get(self.selected_index)
            .map(|c| c.title.to_string())
            .unwrap_or_default()
    }

    /// Returns active selected command identifier string.
    pub fn selected_command_id(&self) -> String {
        self.matches()
            .get(self.selected_index)
            .map(|c| c.id.to_string())
            .unwrap_or_default()
    }
}
