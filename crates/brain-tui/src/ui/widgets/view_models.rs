//! Immutable data-driven view models for stateless widgets.

use crate::ui::theme::ThemeToken;

/// Layout limits for stateless widgets.
pub const MAX_SHORTCUTS: usize = 8;
/// Maximum tab entries supported by the toolbar.
pub const MAX_TABS: usize = 8;
/// Maximum action buttons supported by confirmation dialogs.
pub const MAX_DIALOG_BUTTONS: usize = 4;
/// Maximum visible rows supported by the vertical list.
pub const MAX_VISIBLE_LIST_ROWS: usize = 32;
/// Maximum visible lines supported by the ScrollView viewport.
pub const MAX_VISIBLE_SCROLL_ROWS: usize = 64;

/// Classification of StatusBar states.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusKind {
    /// Idle/passive state.
    Idle,
    /// System is processing/thinking.
    Working,
    /// Streaming content.
    Streaming,
    /// Error status.
    Error,
    /// Offline state.
    Offline,
}

/// View model for the StatusBar widget.
pub struct StatusBarView<'a> {
    /// Active session title.
    pub title: &'a str,
    /// Semantic status classifier.
    pub kind: StatusKind,
    /// Detailed status message.
    pub message: &'a str,
}

/// A single shortcut hint.
#[derive(Debug, Clone, Copy)]
pub struct ShortcutHint<'a> {
    /// Shortcut hotkey trigger label.
    pub key: &'a str,
    /// User action description.
    pub description: &'a str,
}

/// View model for the Footer widget.
pub struct FooterView<'a> {
    /// List of registered shortcut keys and actions.
    pub shortcuts: &'a [ShortcutHint<'a>],
}

/// Classification of active focused state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusState {
    /// Panel has input focus.
    Focused,
    /// Panel is visible but inactive.
    Inactive,
    /// Panel is disabled.
    Disabled,
}

/// View model for the Panel container widget.
pub struct PanelView<'a> {
    /// Container title label.
    pub title: &'a str,
    /// Panel focus state.
    pub focus: FocusState,
}

/// Classification of Dialog button kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ButtonKind {
    /// Primary action confirmation button.
    Primary,
    /// Dismiss action button.
    Secondary,
    /// High-risk action button.
    Danger,
}

/// Dialog action button view structure.
#[derive(Debug, Clone, Copy)]
pub struct DialogButton<'a> {
    /// Action label text.
    pub label: &'a str,
    /// Selection category.
    pub kind: ButtonKind,
    /// Clickable status.
    pub enabled: bool,
}

/// View model for Dialog prompt widgets.
pub struct DialogView<'a> {
    /// Modal header title.
    pub title: &'a str,
    /// Prompt description text.
    pub message: &'a str,
    /// Action choices.
    pub buttons: &'a [DialogButton<'a>],
    /// Highlighted choice index.
    pub selected_index: usize,
}

/// View model for Section dividers.
pub struct SectionView<'a> {
    /// Section name/header label.
    pub title: &'a str,
    /// Expanded status.
    pub collapsed: bool,
}

/// View model structure for a single toolbar tab entry.
#[derive(Debug, Clone, Copy)]
pub struct TabView<'a> {
    /// Tab label text.
    pub title: &'a str,
    /// Active highlight state.
    pub active: bool,
}

/// View model for tab Toolbar headers.
pub struct ToolbarView<'a> {
    /// Available tab view details.
    pub tabs: &'a [TabView<'a>],
}

/// Representation model for individual select list items.
#[derive(Debug, Clone, Copy)]
pub struct ListItem<'a> {
    /// Label text.
    pub label: &'a str,
    /// Selection status.
    pub selected: bool,
    /// Enabled/disabled status.
    pub disabled: bool,
}

/// View model for Lists.
pub struct ListView<'a> {
    /// Collection of list items.
    pub items: &'a [ListItem<'a>],
}

/// ScrollView representation model.
pub struct ScrollViewModel<'a> {
    /// Content lines to show inside viewport.
    pub lines: &'a [&'a str],
    /// Scroll offset.
    pub scroll_offset: usize,
}

/// View model for CommandHint helper popup.
pub struct CommandHintView<'a> {
    /// Suggestion query snippet.
    pub command: &'a str,
    /// Detailed usage information parameter template.
    pub usage: &'a str,
}

/// View model structure for Empty state containers.
pub struct EmptyStateView<'a> {
    /// Error/information title header.
    pub title: &'a str,
    /// Description message.
    pub description: &'a str,
    /// Icon indicator symbol template.
    pub icon: &'static str,
}

/// Target panels that can receive key focus.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusTarget {
    /// Active message stream scroll area.
    Conversation,
    /// Prompt input bar.
    Prompt,
    /// Session list sidebar.
    Sidebar,
    /// Command Palette overlay.
    CommandPalette,
    /// Modal dialog overlay.
    Dialog,
}

/// Connection states mapped to connectivity icons.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionState {
    /// Normal connected state.
    Connected,
    /// Reconnecting state.
    Connecting,
    /// Disconnected state.
    Offline,
    /// Connection failure.
    Error,
}

/// Semantic view model representing the chat screen state.
pub struct ChatScreenView<'a> {
    /// Active session thread title.
    pub session_title: &'a str,
    /// Connection status classification.
    pub connection: ConnectionState,
    /// Whether the background daemon is processing commands.
    pub is_working: bool,
    /// Number of message entries.
    pub message_count: usize,
    /// Text current input buffer.
    pub input_buffer: &'a str,
    /// Focused panel category.
    pub focus: FocusTarget,
}

/// Helper function to safely truncate strings with trailing ellipsis.
pub fn truncate_str(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else if max_len > 3 {
        format!("{}...", &s[..max_len - 3])
    } else {
        s[..max_len].to_string()
    }
}

/// Health status view model for HealthWidget.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HealthViewModel {
    /// Formatted status string ("HEALTHY", "DEGRADED", "UNHEALTHY").
    pub status_text: String,
    /// Detailed health reason string if non-healthy.
    pub reason: Option<String>,
    /// Theme token for status.
    pub status_token: ThemeToken,
    /// System uptime formatted string (e.g. "1h 24m 12s").
    pub uptime_text: String,
    /// Active event subscribers count string.
    pub subscribers_text: String,
    /// Storage backend identifier string (e.g. "SQLite WAL").
    pub storage_backend: String,
}

/// Orchestrator status view model for OrchestratorWidget.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrchestratorViewModel {
    /// Pending tasks count string.
    pub pending_count_text: String,
    /// Tasks completed count string.
    pub completed_count_text: String,
    /// Tasks failed count string.
    pub failed_count_text: String,
    /// Tasks dropped count string.
    pub dropped_count_text: String,
    /// Last wait latency formatted string (e.g. "2ms").
    pub last_wait_text: String,
    /// Last exec latency formatted string (e.g. "45ms").
    pub last_exec_text: String,
    /// Currently running task details string, if any.
    pub current_running_task_text: String,
}

/// Projection lag item view model for ProjectionLagWidget.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectionLagItemViewModel {
    /// Truncated projection name.
    pub name: String,
    /// Last processed sequence string.
    pub last_processed: String,
    /// Max event sequence string.
    pub max_sequence: String,
    /// Sequence lag count string.
    pub lag_count: String,
    /// Status indicator string ("UP TO DATE", "LAGGING (14)").
    pub status: String,
    /// Status theme token.
    pub status_token: ThemeToken,
}

/// Projection lag view model for ProjectionLagWidget.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectionLagViewModel {
    /// List of projection lag items.
    pub items: Vec<ProjectionLagItemViewModel>,
}

/// Reflection metrics view model for ReflectionWidget.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReflectionViewModel {
    /// Cycles executed count string.
    pub cycles_text: String,
    /// Total findings count string.
    pub findings_text: String,
    /// Total commands executed count string.
    pub commands_executed_text: String,
    /// Total skipped findings count string.
    pub commands_skipped_text: String,
    /// Last cycle duration formatted string (e.g. "120ms" or "None").
    pub last_duration_text: String,
}

/// Task history trace item view model for TaskHistoryWidget.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskHistoryItemViewModel {
    /// Truncated task ID (e.g., "4a2f8b12").
    pub id: String,
    /// Task category kind ("compile", "project", "reflect", "maintain").
    pub kind: String,
    /// Priority level ("critical", "high", "normal", "low").
    pub priority: String,
    /// Priority theme token.
    pub priority_token: ThemeToken,
    /// Execution status ("Succeeded", "Failed", "Running").
    pub status: String,
    /// Status theme token.
    pub status_token: ThemeToken,
    /// Wait duration formatted string (e.g. "1ms").
    pub wait_duration_text: String,
    /// Exec duration formatted string (e.g. "42ms").
    pub exec_duration_text: String,
}

/// Task history view model for TaskHistoryWidget.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskHistoryViewModel {
    /// List of task trace items.
    pub items: Vec<TaskHistoryItemViewModel>,
    /// Selected index cursor position.
    pub selected_index: Option<usize>,
}

/// Composite view model for RuntimeDashboard.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeDashboardViewModel {
    /// Sequence counter string.
    pub sequence_text: String,
    /// Snapshot timestamp string.
    pub timestamp_text: String,
    /// Health widget view model.
    pub health: HealthViewModel,
    /// Orchestrator widget view model.
    pub orchestrator: OrchestratorViewModel,
    /// Projection lag widget view model.
    pub projections: ProjectionLagViewModel,
    /// Reflection widget view model.
    pub reflection: ReflectionViewModel,
    /// Task history trace widget view model.
    pub task_history: TaskHistoryViewModel,
}

impl RuntimeDashboardViewModel {
    /// Creates a presentation-layer `RuntimeDashboardViewModel` from a versioned `RuntimeDiagnosticsReport`.
    pub fn from_report(
        report: &brain_integrations::dto::v1::RuntimeDiagnosticsReport,
        selected_history_idx: Option<usize>,
    ) -> Self {
        let (health_text, health_token) = match report.health.to_lowercase().as_str() {
            "healthy" => ("HEALTHY".to_string(), ThemeToken::Success),
            "degraded" => ("DEGRADED".to_string(), ThemeToken::Warning),
            "unhealthy" => ("UNHEALTHY".to_string(), ThemeToken::Danger),
            _ => ("UNKNOWN".to_string(), ThemeToken::TextMuted),
        };

        let health = HealthViewModel {
            status_text: health_text,
            reason: report.health_reason.clone(),
            status_token: health_token,
            uptime_text: "Active".to_string(),
            subscribers_text: "1".to_string(),
            storage_backend: "SQLite WAL".to_string(),
        };

        let orch = &report.orchestrator;
        let current_task_str = match &orch.current_running_task {
            Some(t) => format!(
                "Running {} [{}] (prio: {})",
                truncate_str(&t.kind, 12),
                truncate_str(&t.id, 8),
                t.priority
            ),
            None => "Idle (waiting for tasks)".to_string(),
        };

        let orchestrator = OrchestratorViewModel {
            pending_count_text: orch.pending_tasks_count.to_string(),
            completed_count_text: orch.tasks_completed.to_string(),
            failed_count_text: orch.tasks_failed.to_string(),
            dropped_count_text: orch.tasks_dropped.to_string(),
            last_wait_text: format!("{}ms", orch.last_task_wait_ms),
            last_exec_text: format!("{}ms", orch.last_task_exec_ms),
            current_running_task_text: current_task_str,
        };

        let projection_items = report
            .projection_lags
            .iter()
            .map(|p| {
                let (status_text, token) = if p.lag_sequence_count == 0 {
                    ("UP TO DATE".to_string(), ThemeToken::Success)
                } else {
                    (
                        format!("LAGGING ({})", p.lag_sequence_count),
                        ThemeToken::Warning,
                    )
                };
                ProjectionLagItemViewModel {
                    name: truncate_str(&p.projection_id, 16),
                    last_processed: p.last_processed_sequence.to_string(),
                    max_sequence: p.max_event_sequence.to_string(),
                    lag_count: p.lag_sequence_count.to_string(),
                    status: status_text,
                    status_token: token,
                }
            })
            .collect();

        let projections = ProjectionLagViewModel {
            items: projection_items,
        };

        let ref_dto = &report.reflection;
        let last_dur_str = match ref_dto.last_reflection_duration_ms {
            Some(ms) => format!("{}ms", ms),
            None => "None".to_string(),
        };

        let reflection = ReflectionViewModel {
            cycles_text: ref_dto.reflections_executed.to_string(),
            findings_text: ref_dto.reflection_findings_count.to_string(),
            commands_executed_text: ref_dto.reflection_commands_executed.to_string(),
            commands_skipped_text: ref_dto.reflection_commands_skipped.to_string(),
            last_duration_text: last_dur_str,
        };

        let history_items = orch
            .task_history
            .iter()
            .map(|t| {
                let prio_token = match t.priority.to_lowercase().as_str() {
                    "critical" => ThemeToken::Danger,
                    "high" => ThemeToken::Warning,
                    "normal" => ThemeToken::Info,
                    _ => ThemeToken::TextMuted,
                };
                let status_token = if t.status.contains("Succeeded") {
                    ThemeToken::Success
                } else if t.status.contains("Failed") {
                    ThemeToken::Danger
                } else {
                    ThemeToken::Warning
                };

                TaskHistoryItemViewModel {
                    id: truncate_str(&t.id, 8),
                    kind: truncate_str(&t.kind, 12),
                    priority: t.priority.clone(),
                    priority_token: prio_token,
                    status: truncate_str(&t.status, 14),
                    status_token,
                    wait_duration_text: format!("{}ms", t.wait_duration_ms),
                    exec_duration_text: format!("{}ms", t.exec_duration_ms),
                }
            })
            .collect();

        let task_history = TaskHistoryViewModel {
            items: history_items,
            selected_index: selected_history_idx,
        };

        Self {
            sequence_text: report.snapshot_sequence.to_string(),
            timestamp_text: report.snapshot_timestamp_ms.to_string(),
            health,
            orchestrator,
            projections,
            reflection,
            task_history,
        }
    }
}

/// Concept item view model for ConceptListWidget.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConceptItemViewModel {
    /// Truncated concept ID.
    pub id: String,
    /// Truncated display label.
    pub label: String,
    /// Concept node type.
    pub node_type: String,
    /// Total relationships count text.
    pub relationships_count_text: String,
}

/// Concept catalog view model for ConceptListWidget.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConceptListViewModel {
    /// Catalog of concept items.
    pub items: Vec<ConceptItemViewModel>,
    /// Selected index inside list.
    pub selected_index: Option<usize>,
}

/// Core concept detail view model for ConceptDetailsWidget.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConceptDetailsViewModel {
    /// Concept identifier string.
    pub id: String,
    /// Canonical display label.
    pub label: String,
    /// Node classification type.
    pub node_type: String,
}

/// Relationship edge item view model for RelationsWidget.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelationItemViewModel {
    /// Target concept ID string.
    pub target_id: String,
    /// Target concept display label.
    pub target_label: String,
    /// Target concept type.
    pub target_type: String,
    /// Relation classification (e.g., "works_on").
    pub relation: String,
    /// Edge direction ("OUTGOING" or "INCOMING").
    pub direction: String,
    /// Theme token for direction.
    pub direction_token: ThemeToken,
    /// Formatted confidence weight string (e.g. "0.95").
    pub weight_text: String,
}

/// Relations list view model for RelationsWidget.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelationsViewModel {
    /// Deterministically sorted list of relations.
    pub items: Vec<RelationItemViewModel>,
    /// Selected relation cursor index.
    pub selected_index: Option<usize>,
}

/// Property attribute view model for PropertiesWidget.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PropertyItemViewModel {
    /// Group classification ("System", "Canonical", "User", "Metadata").
    pub group: String,
    /// Attribute key name.
    pub key: String,
    /// Attribute string value.
    pub value: String,
}

/// Properties list view model for PropertiesWidget.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PropertiesViewModel {
    /// Deterministically ordered key-value properties.
    pub items: Vec<PropertyItemViewModel>,
}

/// Provenance metadata view model for ProvenanceWidget.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProvenanceViewModel {
    /// Source origin classification (e.g. "Ingested", "Inferred").
    pub source: String,
    /// Originating compiler pass, if available.
    pub compiler_pass: String,
    /// Physical location reference string.
    pub location: String,
    /// Ingestion timestamp formatted string.
    pub timestamp_text: String,
    /// Formatted key-value annotations text list.
    pub extra_info: Vec<(String, String)>,
}

/// Composite view model for KnowledgeExplorer screen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KnowledgeExplorerViewModel {
    /// Concept list view model.
    pub concept_list: ConceptListViewModel,
    /// Currently focused concept details (if loaded).
    pub details: Option<ConceptDetailsViewModel>,
    /// Relationship edges view model.
    pub relations: RelationsViewModel,
    /// Properties map view model.
    pub properties: PropertiesViewModel,
    /// Provenance history view model.
    pub provenance: Option<ProvenanceViewModel>,
}

impl KnowledgeExplorerViewModel {
    /// Creates a presentation-layer `KnowledgeExplorerViewModel` with deterministic sorting rules.
    pub fn from_report(
        concepts: &[brain_integrations::dto::v1::ConceptSummaryDto],
        detail: Option<&brain_integrations::dto::v1::ConceptDetailReport>,
        selected_concept_idx: Option<usize>,
        selected_relation_idx: Option<usize>,
    ) -> Self {
        let concept_items = concepts
            .iter()
            .map(|c| ConceptItemViewModel {
                id: truncate_str(&c.id, 8),
                label: truncate_str(&c.label, 20),
                node_type: truncate_str(&c.node_type, 12),
                relationships_count_text: c.relationships_count.to_string(),
            })
            .collect();

        let concept_list = ConceptListViewModel {
            items: concept_items,
            selected_index: selected_concept_idx,
        };

        let details = detail.map(|d| ConceptDetailsViewModel {
            id: d.id.clone(),
            label: d.label.clone(),
            node_type: d.node_type.clone(),
        });

        // Deterministic Relation Sorting:
        // Outgoing -> Incoming -> Relation -> Target label
        let mut sorted_relations = Vec::new();
        if let Some(d) = detail {
            let mut rel_dtos = d.relations.clone();
            rel_dtos.sort_by(|a, b| {
                let dir_a = if a.direction.to_lowercase().contains("out") {
                    0
                } else {
                    1
                };
                let dir_b = if b.direction.to_lowercase().contains("out") {
                    0
                } else {
                    1
                };
                dir_a
                    .cmp(&dir_b)
                    .then_with(|| a.relation.cmp(&b.relation))
                    .then_with(|| a.target_label.cmp(&b.target_label))
            });

            for r in rel_dtos {
                let is_out = r.direction.to_lowercase().contains("out");
                let dir_str = if is_out { "OUTGOING" } else { "INCOMING" };
                let dir_token = if is_out {
                    ThemeToken::Info
                } else {
                    ThemeToken::Success
                };

                sorted_relations.push(RelationItemViewModel {
                    target_id: r.target_id,
                    target_label: truncate_str(&r.target_label, 18),
                    target_type: truncate_str(&r.target_type, 12),
                    relation: r.relation,
                    direction: dir_str.to_string(),
                    direction_token: dir_token,
                    weight_text: format!("{:.2}", r.weight),
                });
            }
        }

        let relations = RelationsViewModel {
            items: sorted_relations,
            selected_index: selected_relation_idx,
        };

        // Deterministic Property Grouping:
        // System -> Canonical -> User -> Metadata
        let mut prop_items = Vec::new();
        if let Some(d) = detail {
            for (k, v) in &d.properties {
                let group = if k == "id" || k.starts_with("sys_") {
                    "System".to_string()
                } else if k == "label" || k == "node_type" {
                    "Canonical".to_string()
                } else if k.starts_with("user_") {
                    "User".to_string()
                } else {
                    "Metadata".to_string()
                };

                prop_items.push(PropertyItemViewModel {
                    group,
                    key: k.clone(),
                    value: truncate_str(v, 30),
                });
            }
            // Sort deterministically by Group order then Key name
            prop_items.sort_by(|a, b| {
                let g_rank = |g: &str| match g {
                    "System" => 0,
                    "Canonical" => 1,
                    "User" => 2,
                    _ => 3,
                };
                g_rank(&a.group)
                    .cmp(&g_rank(&b.group))
                    .then_with(|| a.key.cmp(&b.key))
            });
        }

        let properties = PropertiesViewModel { items: prop_items };

        let provenance = detail.map(|d| ProvenanceViewModel {
            source: d.provenance.source.clone(),
            compiler_pass: d
                .provenance
                .compiler_pass
                .clone()
                .unwrap_or_else(|| "None".to_string()),
            location: d.provenance.location.clone(),
            timestamp_text: d.provenance.timestamp_ms.to_string(),
            extra_info: d
                .provenance
                .extra_info
                .iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect(),
        });

        Self {
            concept_list,
            details,
            relations,
            properties,
            provenance,
        }
    }
}

/// Step item view model for ExplanationTimelineWidget.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExplanationStepItemViewModel {
    /// Unique deterministic step identifier string.
    pub step_id: String,
    /// Monotonic sequence number in timeline chain.
    pub step_sequence: u64,
    /// Optional parent step ID establishing explicit causal origin.
    pub parent_step_id: Option<String>,
    /// Formatted timestamp string.
    pub time_text: String,
    /// Formatted stage text (e.g. "[OBSERVATION]").
    pub stage_text: String,
    /// Visual status badge string ("✓", "⚠", "✖", "ℹ").
    pub status_badge: String,
    /// Theme token for status badge.
    pub status_token: ThemeToken,
    /// Display title string.
    pub title: String,
    /// Stage narrative description string.
    pub description: String,
}

/// Concept summary view model for ExplanationSummaryWidget.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExplanationSummaryViewModel {
    /// Concept identifier string.
    pub concept_id: String,
    /// Canonical display label.
    pub concept_label: String,
    /// Node classification type.
    pub node_type: String,
    /// Ingestion timestamp string.
    pub created_at_text: String,
    /// Total causal steps count text.
    pub total_steps_text: String,
}

/// Timeline view model for ExplanationTimelineWidget.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExplanationTimelineViewModel {
    /// Timeline step items list.
    pub items: Vec<ExplanationStepItemViewModel>,
    /// Cursor selection index.
    pub selected_index: Option<usize>,
}

/// Stage execution detail view model for ExplanationDetailWidget.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExplanationDetailPaneViewModel {
    /// Step ID string.
    pub step_id: String,
    /// Step sequence.
    pub step_sequence: u64,
    /// Parent step ID formatted text ("None", "step_001", or "Parent step unavailable").
    pub parent_step_id_text: String,
    /// Stage text.
    pub stage_text: String,
    /// Status text.
    pub status_text: String,
    /// Title string.
    pub title: String,
    /// Detailed description.
    pub description: String,
    /// Key-value metadata annotations pairs list.
    pub metadata_items: Vec<(String, String)>,
}

/// Composite view model for ExplainabilityScreen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExplanationViewModel {
    /// Target concept explanation summary.
    pub summary: Option<ExplanationSummaryViewModel>,
    /// Timeline steps list.
    pub timeline: ExplanationTimelineViewModel,
    /// Focused step execution details.
    pub detail_pane: Option<ExplanationDetailPaneViewModel>,
}

impl ExplanationViewModel {
    /// Creates a presentation-layer `ExplanationViewModel` from a `v1::ExplanationReport`.
    pub fn from_report(
        report: Option<&brain_integrations::dto::v1::ExplanationReport>,
        selected_step_idx: Option<usize>,
    ) -> Self {
        use brain_integrations::dto::v1::{ExplanationStage, ExplanationStatus};

        let summary = report.map(|r| ExplanationSummaryViewModel {
            concept_id: r.concept_id.clone(),
            concept_label: r.concept_label.clone(),
            node_type: r.node_type.clone(),
            created_at_text: r.created_at_ms.to_string(),
            total_steps_text: r.steps.len().to_string(),
        });

        let mut step_items = Vec::new();
        let mut step_id_set = std::collections::HashSet::new();

        if let Some(r) = report {
            for step in &r.steps {
                step_id_set.insert(step.step_id.clone());
            }

            for step in &r.steps {
                let stage_str = match step.stage {
                    ExplanationStage::Observation => "[OBSERVATION]",
                    ExplanationStage::Compiler => "[COMPILER]",
                    ExplanationStage::Knowledge => "[KNOWLEDGE]",
                    ExplanationStage::Projection => "[PROJECTION]",
                    ExplanationStage::Reflection => "[REFLECTION]",
                    ExplanationStage::Recommendation => "[RECOMMENDATION]",
                };

                let (badge, token) = match step.status {
                    ExplanationStatus::Success => ("✓", ThemeToken::Success),
                    ExplanationStatus::Warning => ("⚠", ThemeToken::Warning),
                    ExplanationStatus::Error => ("✖", ThemeToken::Danger),
                    ExplanationStatus::Info => ("ℹ", ThemeToken::Info),
                };

                step_items.push(ExplanationStepItemViewModel {
                    step_id: step.step_id.clone(),
                    step_sequence: step.step_sequence,
                    parent_step_id: step.parent_step_id.clone(),
                    time_text: step.timestamp_ms.to_string(),
                    stage_text: stage_str.to_string(),
                    status_badge: badge.to_string(),
                    status_token: token,
                    title: step.title.clone(),
                    description: step.description.clone(),
                });
            }
        }

        let timeline = ExplanationTimelineViewModel {
            items: step_items,
            selected_index: selected_step_idx,
        };

        let detail_pane = if let (Some(r), Some(idx)) = (report, selected_step_idx) {
            r.steps.get(idx).map(|step| {
                let parent_text = match &step.parent_step_id {
                    None => "None".to_string(),
                    Some(parent_id) => {
                        if step_id_set.contains(parent_id) {
                            parent_id.clone()
                        } else {
                            format!("{} (Parent step unavailable / compacted)", parent_id)
                        }
                    }
                };

                let stage_str = match step.stage {
                    ExplanationStage::Observation => "Observation Ingestion",
                    ExplanationStage::Compiler => "Compiler Normalization",
                    ExplanationStage::Knowledge => "Canonical Record Established",
                    ExplanationStage::Projection => "Projection Index Update",
                    ExplanationStage::Reflection => "Reflection Finding Cycle",
                    ExplanationStage::Recommendation => "Recommendation Resolution",
                };

                let status_str = match step.status {
                    ExplanationStatus::Success => "✓ Success",
                    ExplanationStatus::Warning => "⚠ Warning / Finding",
                    ExplanationStatus::Error => "✖ Diagnostic Error",
                    ExplanationStatus::Info => "ℹ Informational Telemetry",
                };

                let meta_list: Vec<(String, String)> = step
                    .metadata
                    .iter()
                    .map(|(k, v)| (k.clone(), v.clone()))
                    .collect();

                ExplanationDetailPaneViewModel {
                    step_id: step.step_id.clone(),
                    step_sequence: step.step_sequence,
                    parent_step_id_text: parent_text,
                    stage_text: stage_str.to_string(),
                    status_text: status_str.to_string(),
                    title: step.title.clone(),
                    description: step.description.clone(),
                    metadata_items: meta_list,
                }
            })
        } else {
            None
        };

        Self {
            summary,
            timeline,
            detail_pane,
        }
    }
}

/// Proposal item view model for InteractiveReflection proposal list table.
#[derive(Debug, Clone, PartialEq)]
pub struct ReflectionProposalItemViewModel {
    /// Proposal ID string (e.g. "prop_94a2b18c").
    pub proposal_id: String,
    /// Status badge string ("[PENDING]", "[ACCEPTED]", "[REJECTED]", "[DEFERRED]").
    pub status_badge: String,
    /// Status badge theme token.
    pub status_token: ThemeToken,
    /// Typed action badge string ("[MERGE]", "[STRENGTHEN]", "[PRUNE]", "[INFER]").
    pub action_badge: String,
    /// Action badge theme token.
    pub action_token: ThemeToken,
    /// Primary source concept ID.
    pub source_concept_id: String,
    /// Target concept ID, if any.
    pub target_concept_id_text: String,
    /// Formatted confidence percentage text (e.g. "94%").
    pub confidence_text: String,
    /// Summary sentence string.
    pub explanation_summary: String,
}

/// Detailed view model for focused proposal breakdown pane.
#[derive(Debug, Clone, PartialEq)]
pub struct ReflectionProposalDetailViewModel {
    /// Proposal ID string.
    pub proposal_id: String,
    /// Finding kind description string.
    pub finding_kind: String,
    /// Source concept ID.
    pub source_concept_id: String,
    /// Target concept ID text.
    pub target_concept_id_text: String,
    /// Confidence text.
    pub confidence_text: String,
    /// Action type text.
    pub action_type_text: String,
    /// Status text.
    pub status_text: String,
    /// Full explanation summary description.
    pub explanation_summary: String,
    /// Formatted creation timestamp string.
    pub created_at_text: String,
    /// Formatted resolution timestamp string, if resolved.
    pub resolved_at_text: String,
}

/// Composite view model for InteractiveReflection screen.
#[derive(Debug, Clone, PartialEq)]
pub struct InteractiveReflectionViewModel {
    /// Proposals list items.
    pub items: Vec<ReflectionProposalItemViewModel>,
    /// Selected index.
    pub selected_index: Option<usize>,
    /// Focused proposal detail view model.
    pub detail_pane: Option<ReflectionProposalDetailViewModel>,
    /// Pending proposals count.
    pub pending_count: usize,
    /// Accepted proposals count.
    pub accepted_count: usize,
    /// Rejected proposals count.
    pub rejected_count: usize,
    /// Deferred proposals count.
    pub deferred_count: usize,
}

impl InteractiveReflectionViewModel {
    /// Creates an `InteractiveReflectionViewModel` from proposal DTOs.
    pub fn from_proposals(
        proposals: &[brain_integrations::dto::v1::ReflectionProposalDto],
        selected_idx: Option<usize>,
        filter: Option<brain_integrations::dto::v1::ReflectionProposalStatus>,
    ) -> Self {
        use brain_integrations::dto::v1::{ReflectionActionType, ReflectionProposalStatus};

        let mut pending_count = 0;
        let mut accepted_count = 0;
        let mut rejected_count = 0;
        let mut deferred_count = 0;

        for p in proposals {
            match p.status {
                ReflectionProposalStatus::Pending => pending_count += 1,
                ReflectionProposalStatus::Accepted => accepted_count += 1,
                ReflectionProposalStatus::Rejected => rejected_count += 1,
                ReflectionProposalStatus::Deferred => deferred_count += 1,
            }
        }

        let filtered: Vec<&brain_integrations::dto::v1::ReflectionProposalDto> = proposals
            .iter()
            .filter(|p| filter.is_none_or(|f| p.status == f))
            .collect();

        let mut items = Vec::new();
        for p in &filtered {
            let (status_badge, status_token) = match p.status {
                ReflectionProposalStatus::Pending => ("[PENDING]", ThemeToken::Warning),
                ReflectionProposalStatus::Accepted => ("[ACCEPTED]", ThemeToken::Success),
                ReflectionProposalStatus::Rejected => ("[REJECTED]", ThemeToken::Danger),
                ReflectionProposalStatus::Deferred => ("[DEFERRED]", ThemeToken::TextMuted),
            };

            let (action_badge, action_token) = match p.action_type {
                ReflectionActionType::MergeEntities => ("[MERGE]", ThemeToken::Secondary),
                ReflectionActionType::StrengthenEdge => ("[STRENGTHEN]", ThemeToken::Info),
                ReflectionActionType::PruneFact => ("[PRUNE]", ThemeToken::Danger),
                ReflectionActionType::InferRelation => ("[INFER]", ThemeToken::Success),
            };

            items.push(ReflectionProposalItemViewModel {
                proposal_id: p.proposal_id.clone(),
                status_badge: status_badge.to_string(),
                status_token,
                action_badge: action_badge.to_string(),
                action_token,
                source_concept_id: p.source_concept_id.clone(),
                target_concept_id_text: p
                    .target_concept_id
                    .clone()
                    .unwrap_or_else(|| "None".to_string()),
                confidence_text: format!("{:.0}%", p.confidence * 100.0),
                explanation_summary: p.explanation_summary.clone(),
            });
        }

        let detail_pane = if let (Some(idx), false) = (selected_idx, filtered.is_empty()) {
            filtered.get(idx).map(|p| {
                let status_text = match p.status {
                    ReflectionProposalStatus::Pending => "Pending Review",
                    ReflectionProposalStatus::Accepted => "Accepted & Executed",
                    ReflectionProposalStatus::Rejected => "Rejected by User",
                    ReflectionProposalStatus::Deferred => "Deferred for Future Review",
                };

                let action_text = match p.action_type {
                    ReflectionActionType::MergeEntities => "Merge Duplicate Entities",
                    ReflectionActionType::StrengthenEdge => "Strengthen Adjacency Edge",
                    ReflectionActionType::PruneFact => "Prune Superseded Fact",
                    ReflectionActionType::InferRelation => "Infer Relationship Edge",
                };

                ReflectionProposalDetailViewModel {
                    proposal_id: p.proposal_id.clone(),
                    finding_kind: p.finding_kind.clone(),
                    source_concept_id: p.source_concept_id.clone(),
                    target_concept_id_text: p
                        .target_concept_id
                        .clone()
                        .unwrap_or_else(|| "None".to_string()),
                    confidence_text: format!("{:.1}%", p.confidence * 100.0),
                    action_type_text: action_text.to_string(),
                    status_text: status_text.to_string(),
                    explanation_summary: p.explanation_summary.clone(),
                    created_at_text: p.created_at_ms.to_string(),
                    resolved_at_text: p
                        .resolved_at_ms
                        .map_or_else(|| "Unresolved".to_string(), |t| t.to_string()),
                }
            })
        } else {
            None
        };

        Self {
            items,
            selected_index: selected_idx,
            detail_pane,
            pending_count,
            accepted_count,
            rejected_count,
            deferred_count,
        }
    }
}

/// Presentation model for a governance policy row item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvolutionPolicyItemViewModel {
    /// Policy ID.
    pub policy_id: String,
    /// Priority badge index string (e.g. "[P10]").
    pub priority_badge: String,
    /// Human readable name.
    pub name: String,
    /// Trigger classification string.
    pub trigger_badge: String,
    /// Action classification string.
    pub action_badge: String,
    /// Auto-apply indicator text ("AUTO" / "MANUAL").
    pub auto_apply_text: String,
}

/// Presentation model for an evolution plan summary & steps breakdown.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvolutionPlanViewModel {
    /// Plan ID.
    pub plan_id: String,
    /// Target graph version text.
    pub target_version_text: String,
    /// Governing policy ID.
    pub policy_id: String,
    /// Status badge string (e.g. "[DRAFT]", "[EXECUTED]").
    pub status_badge: String,
    /// Status badge theme token.
    pub status_token: ThemeToken,
    /// Total step count string.
    pub steps_count_text: String,
    /// Step rationale descriptions.
    pub step_descriptions: Vec<String>,
}

/// Presentation model for a separate evolution simulation report.
#[derive(Debug, Clone, PartialEq)]
pub struct EvolutionSimulationViewModel {
    /// Plan ID analyzed.
    pub plan_id: String,
    /// Entities affected text.
    pub entities_affected_text: String,
    /// Facts retired text.
    pub facts_retired_text: String,
    /// Edges strengthened text.
    pub edges_strengthened_text: String,
    /// Confidence delta text (e.g. "+12.0%").
    pub confidence_delta_text: String,
    /// Risk level badge (e.g. "[LOW RISK]").
    pub risk_badge: String,
    /// Risk badge theme token.
    pub risk_token: ThemeToken,
}

/// Presentation model for an evolution audit record history item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvolutionAuditItemViewModel {
    /// Audit ID.
    pub audit_id: String,
    /// Graph version after execution text.
    pub graph_version_text: String,
    /// Plan ID executed.
    pub plan_id: String,
    /// Governing policy name.
    pub policy_name: String,
    /// Outcome badge (e.g. "[APPLIED]", "[CONFLICT]").
    pub outcome_badge: String,
    /// Outcome badge theme token.
    pub outcome_token: ThemeToken,
    /// Summary sentence.
    pub summary: String,
}

/// Top-level ViewModel composing Knowledge Evolution screen panes.
#[derive(Debug, Clone, PartialEq)]
pub struct KnowledgeEvolutionViewModel {
    /// Active governance policy item models.
    pub policies: Vec<EvolutionPolicyItemViewModel>,
    /// Active selected policy index.
    pub selected_policy_index: Option<usize>,
    /// Currently focused or generated evolution plan view model.
    pub active_plan: Option<EvolutionPlanViewModel>,
    /// Currently active simulation report view model.
    pub simulation_report: Option<EvolutionSimulationViewModel>,
    /// Historical audit record view models.
    pub audit_history: Vec<EvolutionAuditItemViewModel>,
}

impl KnowledgeEvolutionViewModel {
    /// Constructs `KnowledgeEvolutionViewModel` from DTO lists.
    pub fn from_data(
        policies: &[brain_integrations::dto::v1::EvolutionPolicyDto],
        selected_policy_idx: Option<usize>,
        plan: Option<&brain_integrations::dto::v1::EvolutionPlanDto>,
        sim_report: Option<&brain_integrations::dto::v1::EvolutionSimulationReport>,
        audit_history: &[brain_integrations::dto::v1::EvolutionAuditRecordDto],
    ) -> Self {
        use brain_integrations::dto::v1::{EvolutionExecutionOutcome, EvolutionPlanStatus};

        let policy_vms = policies
            .iter()
            .map(|p| EvolutionPolicyItemViewModel {
                policy_id: p.policy_id.clone(),
                priority_badge: format!("[P{}]", p.priority),
                name: p.name.clone(),
                trigger_badge: format!("{:?}", p.trigger_kind),
                action_badge: format!("{:?}", p.action_kind),
                auto_apply_text: if p.auto_apply {
                    "AUTO".to_string()
                } else {
                    "MANUAL".to_string()
                },
            })
            .collect();

        let plan_vm = plan.map(|p| {
            let (status_badge, status_token) = match p.status {
                EvolutionPlanStatus::Draft => ("[DRAFT]", ThemeToken::Warning),
                EvolutionPlanStatus::Approved => ("[APPROVED]", ThemeToken::Success),
                EvolutionPlanStatus::Executed => ("[EXECUTED]", ThemeToken::Info),
                EvolutionPlanStatus::RolledBack => ("[ROLLED BACK]", ThemeToken::Danger),
            };

            let step_descriptions = p
                .steps
                .iter()
                .map(|s| format!("{}. {}", s.sequence, s.description))
                .collect();

            EvolutionPlanViewModel {
                plan_id: p.plan_id.clone(),
                target_version_text: format!("Target Graph Version: v{}", p.target_graph_version),
                policy_id: p.policy_id.clone(),
                status_badge: status_badge.to_string(),
                status_token,
                steps_count_text: format!("{} steps", p.steps.len()),
                step_descriptions,
            }
        });

        let sim_vm = sim_report.map(|s| {
            let (risk_badge, risk_token) = match s.risk_level.as_str() {
                "LOW" => ("[LOW RISK]", ThemeToken::Success),
                "MEDIUM" => ("[MEDIUM RISK]", ThemeToken::Warning),
                _ => ("[HIGH RISK]", ThemeToken::Danger),
            };

            EvolutionSimulationViewModel {
                plan_id: s.plan_id.clone(),
                entities_affected_text: format!("Entities Affected: {}", s.entities_affected_count),
                facts_retired_text: format!("Facts Retired: {}", s.facts_retired_count),
                edges_strengthened_text: format!(
                    "Edges Strengthened: {}",
                    s.edges_strengthened_count
                ),
                confidence_delta_text: format!(
                    "Confidence Delta: {:+.1}%",
                    s.confidence_delta * 100.0
                ),
                risk_badge: risk_badge.to_string(),
                risk_token,
            }
        });

        let audit_vms = audit_history
            .iter()
            .map(|a| {
                let (outcome_badge, outcome_token) = match a.outcome {
                    EvolutionExecutionOutcome::Applied => ("[APPLIED]", ThemeToken::Success),
                    EvolutionExecutionOutcome::PlanConflict => ("[CONFLICT]", ThemeToken::Danger),
                    EvolutionExecutionOutcome::AlreadyExecuted => {
                        ("[ALREADY EXECUTED]", ThemeToken::Warning)
                    }
                    EvolutionExecutionOutcome::NotFound => ("[NOT FOUND]", ThemeToken::TextMuted),
                };

                EvolutionAuditItemViewModel {
                    audit_id: a.audit_id.clone(),
                    graph_version_text: format!("v{}", a.graph_version),
                    plan_id: a.plan_id.clone(),
                    policy_name: a.policy_name.clone(),
                    outcome_badge: outcome_badge.to_string(),
                    outcome_token,
                    summary: a.summary.clone(),
                }
            })
            .collect();

        Self {
            policies: policy_vms,
            selected_policy_index: selected_policy_idx,
            active_plan: plan_vm,
            simulation_report: sim_vm,
            audit_history: audit_vms,
        }
    }
}

/// Presentation model for an automation rule item row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AutomationRuleItemViewModel {
    /// Rule ID.
    pub rule_id: String,
    /// Human-readable name.
    pub name: String,
    /// Trigger classification string.
    pub trigger_badge: String,
    /// Action classification string.
    pub action_badge: String,
    /// Active status badge ("ACTIVE" / "PAUSED").
    pub status_badge: String,
    /// Active status theme token.
    pub status_token: ThemeToken,
    /// Target policy ID.
    pub target_policy_id: String,
}

/// Presentation model for a scheduled queue item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AutomationQueueItemViewModel {
    /// Queue ID.
    pub queue_id: String,
    /// Automation execution ID traceability string.
    pub automation_execution_id: String,
    /// Rule ID.
    pub rule_id: String,
    /// Queue status badge.
    pub status_badge: String,
    /// Status theme token.
    pub status_token: ThemeToken,
    /// Retry attempt counter text.
    pub retry_count_text: String,
}

/// Presentation model for an automation execution log history item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AutomationLogItemViewModel {
    /// Log ID.
    pub log_id: String,
    /// Automation execution ID traceability string.
    pub automation_execution_id: String,
    /// Rule ID executed.
    pub rule_id: String,
    /// Evolution plan ID text.
    pub plan_id_text: String,
    /// Graph version text.
    pub graph_version_text: String,
    /// Summary sentence.
    pub summary: String,
}

/// Top-level ViewModel composing Knowledge Automation screen panes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KnowledgeAutomationViewModel {
    /// Automation rules models.
    pub rules: Vec<AutomationRuleItemViewModel>,
    /// Selected rule index.
    pub selected_rule_index: Option<usize>,
    /// Scheduled execution queue models.
    pub queue: Vec<AutomationQueueItemViewModel>,
    /// Execution history log models.
    pub logs: Vec<AutomationLogItemViewModel>,
}

impl KnowledgeAutomationViewModel {
    /// Constructs `KnowledgeAutomationViewModel` from DTO lists.
    pub fn from_data(
        rules: &[brain_integrations::dto::v1::AutomationRuleDto],
        selected_rule_idx: Option<usize>,
        queue: &[brain_integrations::dto::v1::AutomationQueueItemDto],
        logs: &[brain_integrations::dto::v1::AutomationExecutionLogDto],
    ) -> Self {
        use brain_integrations::dto::v1::AutomationQueueStatus;

        let rule_vms = rules
            .iter()
            .map(|r| {
                let (status_badge, status_token) = if r.is_active {
                    ("[ACTIVE]", ThemeToken::Success)
                } else {
                    ("[PAUSED]", ThemeToken::TextMuted)
                };

                AutomationRuleItemViewModel {
                    rule_id: r.rule_id.clone(),
                    name: r.name.clone(),
                    trigger_badge: format!("{:?}", r.trigger_kind),
                    action_badge: format!("{:?}", r.action_kind),
                    status_badge: status_badge.to_string(),
                    status_token,
                    target_policy_id: r.target_policy_id.clone(),
                }
            })
            .collect();

        let queue_vms = queue
            .iter()
            .map(|q| {
                let (status_badge, status_token) = match q.status {
                    AutomationQueueStatus::Queued => ("[QUEUED]", ThemeToken::Warning),
                    AutomationQueueStatus::Running => ("[RUNNING]", ThemeToken::Info),
                    AutomationQueueStatus::Completed => ("[COMPLETED]", ThemeToken::Success),
                    AutomationQueueStatus::Failed => ("[FAILED]", ThemeToken::Danger),
                    AutomationQueueStatus::Cancelled => ("[CANCELLED]", ThemeToken::TextMuted),
                };

                AutomationQueueItemViewModel {
                    queue_id: q.queue_id.clone(),
                    automation_execution_id: q.automation_execution_id.clone(),
                    rule_id: q.rule_id.clone(),
                    status_badge: status_badge.to_string(),
                    status_token,
                    retry_count_text: format!("retries: {}", q.retry_count),
                }
            })
            .collect();

        let log_vms = logs
            .iter()
            .map(|l| AutomationLogItemViewModel {
                log_id: l.log_id.clone(),
                automation_execution_id: l.automation_execution_id.clone(),
                rule_id: l.rule_id.clone(),
                plan_id_text: l.plan_id.clone().unwrap_or_else(|| "none".to_string()),
                graph_version_text: format!("v{}", l.graph_version),
                summary: l.outcome_summary.clone(),
            })
            .collect();

        Self {
            rules: rule_vms,
            selected_rule_index: selected_rule_idx,
            queue: queue_vms,
            logs: log_vms,
        }
    }
}
