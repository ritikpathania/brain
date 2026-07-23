//! Immutable data-driven view models for stateless widgets.

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
    /// Ratatui color for status.
    pub color: ratatui::style::Color,
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
    /// Status ratatui color.
    pub color: ratatui::style::Color,
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
    /// Priority ratatui color.
    pub priority_color: ratatui::style::Color,
    /// Execution status ("Succeeded", "Failed", "Running").
    pub status: String,
    /// Status ratatui color.
    pub status_color: ratatui::style::Color,
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
        use ratatui::style::Color;

        let (health_text, health_color) = match report.health.to_lowercase().as_str() {
            "healthy" => ("HEALTHY".to_string(), Color::Green),
            "degraded" => ("DEGRADED".to_string(), Color::Yellow),
            "unhealthy" => ("UNHEALTHY".to_string(), Color::Red),
            _ => ("UNKNOWN".to_string(), Color::Gray),
        };

        let health = HealthViewModel {
            status_text: health_text,
            reason: report.health_reason.clone(),
            color: health_color,
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
                let (status_text, color) = if p.lag_sequence_count == 0 {
                    ("UP TO DATE".to_string(), Color::Green)
                } else {
                    (format!("LAGGING ({})", p.lag_sequence_count), Color::Yellow)
                };
                ProjectionLagItemViewModel {
                    name: truncate_str(&p.projection_id, 16),
                    last_processed: p.last_processed_sequence.to_string(),
                    max_sequence: p.max_event_sequence.to_string(),
                    lag_count: p.lag_sequence_count.to_string(),
                    status: status_text,
                    color,
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
                let prio_color = match t.priority.to_lowercase().as_str() {
                    "critical" => Color::LightRed,
                    "high" => Color::LightYellow,
                    "normal" => Color::Cyan,
                    _ => Color::DarkGray,
                };
                let status_color = if t.status.contains("Succeeded") {
                    Color::Green
                } else if t.status.contains("Failed") {
                    Color::Red
                } else {
                    Color::Yellow
                };

                TaskHistoryItemViewModel {
                    id: truncate_str(&t.id, 8),
                    kind: truncate_str(&t.kind, 12),
                    priority: t.priority.clone(),
                    priority_color: prio_color,
                    status: truncate_str(&t.status, 14),
                    status_color,
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
    /// Ratatui style color for direction.
    pub direction_color: ratatui::style::Color,
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
        use ratatui::style::Color;

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
                let dir_color = if is_out { Color::Cyan } else { Color::Green };

                sorted_relations.push(RelationItemViewModel {
                    target_id: r.target_id,
                    target_label: truncate_str(&r.target_label, 18),
                    target_type: truncate_str(&r.target_type, 12),
                    relation: r.relation,
                    direction: dir_str.to_string(),
                    direction_color: dir_color,
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
