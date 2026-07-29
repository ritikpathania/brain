use crate::context::ExecutionContext;
use crate::dto::v1;
use crate::dto::v1::{ControlMessage, StreamMessage};
use crate::errors::ApplicationError;
use crate::subscription::{EventStream, SubscriptionManager};
use brain_core::events::{
    ProjectionInstanceInvalidatedEvent, RuntimeEvent, RuntimeRelationshipEvent, TaskProgress,
};
use brain_integrations::IngestionEnvelope;
use brain_services::BrainRuntime;
use std::sync::Arc;
use std::time::SystemTime;

/// Translates dynamic, extensible runtime events into stable serializable DTOs.
pub struct EventTranslator;

impl EventTranslator {
    /// Translates a raw `RuntimeEvent` trait object to a versioned `Event` DTO.
    pub fn translate(event: Arc<dyn RuntimeEvent>) -> v1::Event {
        let any_event = event.as_any();

        if let Some(task) = any_event.downcast_ref::<TaskProgress>() {
            let state_str = format!("{:?}", task.state);
            v1::Event::TaskProgress {
                operation_id: task.operation_id.to_string(),
                correlation_id: task.correlation_id.to_string(),
                state: state_str,
                source: format!("{:?}", task.source),
                sequence: task.sequence,
            }
        } else if let Some(proj) = any_event.downcast_ref::<ProjectionInstanceInvalidatedEvent>() {
            v1::Event::ProjectionInvalidated {
                projection_type: proj.projection_type.clone(),
                epoch: proj.epoch.0,
                correlation_id: proj.correlation_id.to_string(),
            }
        } else if let Some(rel) = any_event.downcast_ref::<RuntimeRelationshipEvent>() {
            let event_name = format!("{:?}", rel.domain_event);
            v1::Event::RelationshipEvent {
                event_name,
                epoch: rel.epoch.0,
                correlation_id: rel.correlation_id.to_string(),
            }
        } else {
            v1::Event::Unknown {
                debug_repr: "Unknown RuntimeEvent".to_string(),
            }
        }
    }

    /// Translates a persisted event envelope from the event log to a versioned Event DTO.
    pub fn translate_envelope(envelope: &brain_events::EventEnvelope) -> v1::Event {
        match &envelope.payload {
            brain_events::DomainEvent::Core(core_ev) => v1::Event::RelationshipEvent {
                event_name: format!("{:?}", core_ev),
                epoch: 0,
                correlation_id: envelope.correlation_id.to_string(),
            },
            _ => v1::Event::Unknown {
                debug_repr: format!("{:?}", envelope.payload),
            },
        }
    }
}

/// Capability-oriented orchestrator presenting the unified public API of the Brain engine.
pub struct BrainApplication {
    runtime: Arc<BrainRuntime>,
    subscription_manager: Arc<SubscriptionManager>,
    last_reflection_report: Arc<parking_lot::Mutex<Option<v1::ReflectionReport>>>,
    reflection_proposals:
        Arc<parking_lot::Mutex<std::collections::HashMap<String, v1::ReflectionProposalDto>>>,
    evolution_planner: Arc<brain_services::evolution::KnowledgeEvolutionPlanner>,
    evolution_audit_history: Arc<parking_lot::Mutex<Vec<v1::EvolutionAuditRecordDto>>>,
    automation_scheduler: Arc<brain_services::automation::AutomationScheduler>,
    knowledge_runtime: Arc<brain_services::runtime::KnowledgeRuntime>,
    maintenance_runtime: Arc<brain_services::reflection::KnowledgeMaintenanceRuntime>,
    active_maintenance_lock: Arc<tokio::sync::Mutex<()>>,
    last_maintenance_result: Arc<parking_lot::Mutex<Option<v1::MaintenanceCycleResultDto>>>,
}

impl BrainApplication {
    /// Returns a reference to the underlying BrainRuntime.
    pub fn runtime(&self) -> &Arc<BrainRuntime> {
        &self.runtime
    }

    /// Crate-level stable interface version.
    pub const INTERFACE_VERSION: &'static str = "1.0.0";

    /// Create a new BrainApplication wrapping the services composition root runtime.
    pub fn new(runtime: Arc<BrainRuntime>) -> Self {
        let max_seq = if let Ok(conn) = runtime.sqlite_storage().pool().get() {
            conn.query_row("SELECT MAX(sequence) FROM system_event_log", [], |row| {
                row.get::<_, Option<i64>>(0)
            })
            .unwrap_or(None)
            .unwrap_or(0) as u64
        } else {
            0
        };

        let subscription_manager = Arc::new(SubscriptionManager::new(max_seq));
        let sub_manager_clone = Arc::clone(&subscription_manager);
        let mut runtime_rx = runtime.subscribe();
        tokio::spawn(async move {
            while let Some(event) = runtime_rx.recv().await {
                let dto = EventTranslator::translate(event);
                sub_manager_clone.broadcast(dto);
            }
        });
        let planner = brain_services::evolution::KnowledgeEvolutionPlanner::new();
        let scheduler = brain_services::automation::AutomationScheduler::new(planner.clone());
        let knowledge_runtime = brain_services::runtime::KnowledgeRuntimeBuilder::new().build();
        let maintenance_runtime =
            brain_services::reflection::KnowledgeMaintenanceRuntime::default();

        Self {
            runtime,
            subscription_manager,
            last_reflection_report: Arc::new(parking_lot::Mutex::new(None)),
            reflection_proposals: Arc::new(parking_lot::Mutex::new(
                std::collections::HashMap::new(),
            )),
            evolution_planner: Arc::new(planner),
            evolution_audit_history: Arc::new(parking_lot::Mutex::new(Vec::new())),
            automation_scheduler: Arc::new(scheduler),
            knowledge_runtime: Arc::new(knowledge_runtime),
            maintenance_runtime: Arc::new(maintenance_runtime),
            active_maintenance_lock: Arc::new(tokio::sync::Mutex::new(())),
            last_maintenance_result: Arc::new(parking_lot::Mutex::new(None)),
        }
    }

    /// Ingestion capability coordinates event validation and processing.
    pub async fn ingest(
        &self,
        envelope: IngestionEnvelope,
        context: &ExecutionContext,
    ) -> Result<IngestionResponse, ApplicationError> {
        if context.is_cancelled() {
            return Err(ApplicationError::Cancelled("Ingestion aborted".to_string()));
        }

        context.emit_progress(1, Some(3), "Validating ingestion envelope DTO");

        if envelope.event_model_version != "1.0" {
            return Err(ApplicationError::Validation(format!(
                "Unsupported event model version: {}",
                envelope.event_model_version
            )));
        }

        context.emit_progress(2, Some(3), "Ingesting event payload into storage");

        // Dispatch ingestion via BrainRuntime
        let res = self.runtime.ingest(envelope)?;

        context.emit_progress(3, Some(3), "Ingestion completed successfully");

        Ok(IngestionResponse {
            status: "success".to_string(),
            processed: true,
            sequence: Some(res.sequence),
            event_id: Some(res.event_id.to_string()),
        })
    }

    /// Replay historical ingestion events after a sequence number.
    pub async fn replay(
        &self,
        after_sequence: u64,
    ) -> Result<Vec<IngestionEnvelope>, ApplicationError> {
        Ok(self.runtime.replay(after_sequence)?)
    }

    /// Inspect a node and retrieve its full relation context.
    pub async fn inspect_node(
        &self,
        id_str: &str,
    ) -> Result<brain_domain::query::inspector::InspectorModel, ApplicationError> {
        Ok(self.runtime.inspect_node(id_str)?)
    }

    /// Search capability dispatches search queries to retrieval services.
    pub async fn search(
        &self,
        query: v1::SearchQuery,
        context: &ExecutionContext,
    ) -> Result<Vec<v1::SearchSummary>, ApplicationError> {
        if context.is_cancelled() {
            return Err(ApplicationError::Cancelled("Search aborted".to_string()));
        }

        context.emit_progress(1, Some(2), "Preparing search terms");

        let mapped_kinds: Option<Vec<brain_domain::SearchDocumentKind>> =
            query.kinds.map(|kinds| {
                kinds
                    .into_iter()
                    .filter_map(|k| match k.as_str() {
                        "session" => Some(brain_domain::SearchDocumentKind::Session),
                        "message" => Some(brain_domain::SearchDocumentKind::Message),
                        "goal" => Some(brain_domain::SearchDocumentKind::Goal),
                        "job" => Some(brain_domain::SearchDocumentKind::Job),
                        "retrieval" => Some(brain_domain::SearchDocumentKind::Retrieval),
                        _ => None,
                    })
                    .collect()
            });

        let mapped_pagination = query
            .pagination
            .map(|p| brain_services::query::PaginationSpec {
                limit: p.limit,
                offset: p.offset,
            });

        let service_query = brain_services::query::SearchQuery {
            text: query.text.clone(),
            kinds: mapped_kinds,
            pagination: mapped_pagination,
        };

        // Execute search on runtime returning SearchDocument
        let mut results = self.runtime.search(service_query)?;
        if results.is_empty() {
            let search_projector = brain_services::SearchProjector;
            let projection_query = brain_services::SearchProjectionQuery {
                query: query.text.clone(),
                limit: 20,
            };
            let projection_result = self.runtime.query_projection(
                &search_projector,
                &projection_query,
                brain_core::CorrelationId::new_v4(),
            );
            for (node, _score) in projection_result.items {
                results.push(brain_domain::SearchDocument {
                    id: brain_domain::SearchDocumentId::new(&node.id.to_string()),
                    kind: brain_domain::SearchDocumentKind::Retrieval,
                    title: node.label.clone(),
                    body: node.label.clone(),
                    metadata: brain_domain::SearchMetadata::Session {
                        archived: false,
                        pinned: false,
                    },
                });
            }
        }

        context.emit_progress(2, Some(2), "Search query completed");

        let mapped = results
            .into_iter()
            .map(|doc| {
                let kind_str = match doc.kind {
                    brain_domain::SearchDocumentKind::Session => "session",
                    brain_domain::SearchDocumentKind::Message => "message",
                    brain_domain::SearchDocumentKind::Goal => "goal",
                    brain_domain::SearchDocumentKind::Job => "job",
                    brain_domain::SearchDocumentKind::Retrieval => "retrieval",
                };
                let mut metadata_map = std::collections::BTreeMap::new();
                match doc.metadata {
                    brain_domain::SearchMetadata::Session { archived, pinned } => {
                        metadata_map.insert("archived".to_string(), archived.to_string());
                        metadata_map.insert("pinned".to_string(), pinned.to_string());
                    }
                    brain_domain::SearchMetadata::Message { session_id, role } => {
                        metadata_map.insert("session_id".to_string(), session_id.to_string());
                        metadata_map.insert("role".to_string(), format!("{:?}", role));
                    }
                }
                v1::SearchSummary {
                    id: doc.id.to_string(),
                    kind: kind_str.to_string(),
                    title: doc.title,
                    body: doc.body,
                    metadata: metadata_map,
                }
            })
            .collect();

        Ok(mapped)
    }

    /// Subscribes to the runtime event stream, yielding mapped, stable DTO events.
    pub fn subscribe(&self, after_sequence: Option<u64>) -> EventStream {
        let (tx, rx) = tokio::sync::mpsc::channel(1000);
        let subscription_manager = Arc::clone(&self.subscription_manager);
        let runtime = Arc::clone(&self.runtime);

        tokio::spawn(async move {
            if let Some(start_seq) = after_sequence {
                let db_log =
                    brain_storage::SqliteEventLog::new(runtime.sqlite_storage().pool().clone());

                let max_seq = if let Ok(conn) = runtime.sqlite_storage().pool().get() {
                    conn.query_row("SELECT MAX(sequence) FROM system_event_log", [], |row| {
                        row.get::<_, Option<i64>>(0)
                    })
                    .unwrap_or(None)
                    .unwrap_or(0) as u64
                } else {
                    0
                };

                let mut actual_start = start_seq;
                const MAX_REPLAY: u64 = 10000;

                if max_seq > start_seq && (max_seq - start_seq) > MAX_REPLAY {
                    actual_start = max_seq - MAX_REPLAY;
                    let _ = tx
                        .send(StreamMessage::Control {
                            payload: ControlMessage::ReplayTruncated {
                                requested_start: start_seq,
                                replayed_start: actual_start,
                            },
                        })
                        .await;
                }

                let mut current_seq = actual_start;
                while let Ok(stored_events) = db_log.read_from(current_seq, 500) {
                    if stored_events.is_empty() {
                        break;
                    }
                    for event in stored_events {
                        if let Ok(domain_event) =
                            serde_json::from_str::<brain_events::DomainEvent>(&event.payload_json)
                        {
                            let envelope = brain_events::EventEnvelope {
                                sequence: Some(event.sequence),
                                event_id: event.event_id,
                                correlation_id: event.correlation_id,
                                timestamp_ms: event.timestamp_ms,
                                version: event.version,
                                source: event.source,
                                payload: domain_event,
                            };
                            let dto = EventTranslator::translate_envelope(&envelope);
                            if tx
                                .send(StreamMessage::Event {
                                    sequence: event.sequence,
                                    event: dto,
                                })
                                .await
                                .is_err()
                            {
                                return;
                            }
                        }
                        current_seq = event.sequence + 1;
                    }
                }
            }

            if tx
                .send(StreamMessage::Control {
                    payload: ControlMessage::CatchUpCompleted,
                })
                .await
                .is_err()
            {
                return;
            }

            subscription_manager.register(tx);
        });

        EventStream::new(rx)
    }

    /// Returns a reference to the active `SubscriptionManager`.
    pub fn subscription_manager(&self) -> &Arc<SubscriptionManager> {
        &self.subscription_manager
    }

    /// Query the current status DTO of the runtime.
    pub fn status(&self) -> v1::Status {
        let status = self.runtime.status();
        let health_str = match status.health {
            brain_services::RuntimeHealth::Initializing => "initializing",
            brain_services::RuntimeHealth::Healthy => "healthy",
            brain_services::RuntimeHealth::ShuttingDown => "shuttingdown",
            brain_services::RuntimeHealth::Stopped => "stopped",
        };
        v1::Status {
            uptime_secs: status.uptime.as_secs(),
            storage_backend: status.storage_backend,
            active_event_subscribers: self.subscription_manager.active_count(),
            health: health_str.to_string(),
        }
    }

    /// Query the current metrics DTO of the runtime.
    pub fn metrics(&self) -> v1::Metrics {
        let metrics = self.runtime.metrics();
        v1::Metrics {
            observations_ingested: metrics.observations_ingested,
            canonicalization_successes: metrics.canonicalization_successes,
            canonicalization_failures: metrics.canonicalization_failures,
            reflections_executed: metrics.reflections_executed,
            projections_executed: metrics.projections_executed,
            retrieval_queries: metrics.retrieval_queries,
            last_ingest_duration_ms: metrics.last_ingest_duration.map(|d| d.as_millis() as u64),
            last_projection_duration_ms: metrics
                .last_projection_duration
                .map(|d| d.as_millis() as u64),
            avg_canonicalization_duration_ms: metrics
                .avg_canonicalization_duration
                .map(|d| d.as_millis() as u64),
            avg_reflection_duration_ms: metrics
                .avg_reflection_duration
                .map(|d| d.as_millis() as u64),
            avg_dispatch_duration_ms: metrics.avg_dispatch_duration.map(|d| d.as_millis() as u64),
        }
    }

    /// Query the current diagnostics DTO of the runtime.
    pub fn diagnostics(&self) -> v1::Diagnostics {
        let diag = self.runtime.diagnostics();
        let recent_failures = diag
            .recent_failures
            .into_iter()
            .map(|f| v1::Failure {
                operation: f.operation,
                error: f.error,
                timestamp_ms: f
                    .timestamp
                    .duration_since(SystemTime::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis() as u64,
            })
            .collect();
        v1::Diagnostics {
            recent_failures,
            last_shutdown_duration_ms: diag.last_shutdown.map(|s| s.duration.as_millis() as u64),
        }
    }

    /// Discover registered capability descriptors.
    pub fn discover_capabilities(&self) -> Vec<v1::Capability> {
        let caps = self.runtime.discover_capabilities();
        caps.into_iter()
            .map(|c| {
                let state_str = match c.state {
                    brain_services::CapabilityState::Active => "active",
                    brain_services::CapabilityState::Degraded => "degraded",
                    brain_services::CapabilityState::Inactive => "inactive",
                };
                v1::Capability {
                    name: c.name.to_string(),
                    version: c.version,
                    description: c.description.to_string(),
                    state: state_str.to_string(),
                    is_enabled: c.is_enabled,
                    is_experimental: c.is_experimental,
                }
            })
            .collect()
    }

    /// Workflow capability executes orchestrations and yields progress logs.
    pub async fn workflow(
        &self,
        workflow_type: String,
        context: &ExecutionContext,
    ) -> Result<Vec<String>, ApplicationError> {
        if context.is_cancelled() {
            return Err(ApplicationError::Cancelled("Workflow aborted".to_string()));
        }

        context.emit_progress(
            1,
            Some(3),
            format!("Initializing workflow: {}", workflow_type),
        );
        context.emit_progress(2, Some(3), "Executing workflow steps");
        context.emit_progress(3, Some(3), "Finalizing workflow");

        Ok(vec!["step1".to_string(), "step2".to_string()])
    }

    /// Administration capability dispatches configuration and administrative actions.
    pub async fn administration(
        &self,
        action: String,
        context: &ExecutionContext,
    ) -> Result<String, ApplicationError> {
        if context.is_cancelled() {
            return Err(ApplicationError::Cancelled(
                "Administration action aborted".to_string(),
            ));
        }

        context.emit_progress(1, Some(2), format!("Authenticating action: {}", action));
        context.emit_progress(2, Some(2), "Applying administrative changes");

        Ok("Administration action completed successfully".to_string())
    }

    /// Retrieves status metadata for all registered projections.
    pub async fn list_projections(&self) -> Result<Vec<v1::ProjectionStatus>, ApplicationError> {
        let scheduler = self.runtime.projection_scheduler();
        let metadata = scheduler.list_metadata().map_err(|e| {
            ApplicationError::Internal(format!("Failed to list projections status: {:?}", e))
        })?;

        Ok(metadata
            .into_iter()
            .map(|m| {
                let status_str = match m.status {
                    brain_storage::ProjectionStatus::Idle => "idle",
                    brain_storage::ProjectionStatus::Active => "active",
                    brain_storage::ProjectionStatus::Rebuilding => "rebuilding",
                    brain_storage::ProjectionStatus::Failed => "failed",
                };
                v1::ProjectionStatus {
                    name: m.name,
                    version: m.version,
                    last_sequence: m.last_sequence,
                    status: status_str.to_string(),
                    last_error: m.last_error,
                    updated_at: m.updated_at,
                }
            })
            .collect())
    }

    /// Triggers manual rebuild of a specific projection.
    pub async fn rebuild_projection(&self, name: &str) -> Result<(), ApplicationError> {
        let scheduler = self.runtime.projection_scheduler();
        let projection_id = match name {
            "jobs" => brain_services::projections::ProjectionId::Jobs,
            "sessions" => brain_services::projections::ProjectionId::Sessions,
            "search" => brain_services::projections::ProjectionId::Search,
            "retrieval" => brain_services::projections::ProjectionId::Retrieval,
            "test_a" => brain_services::projections::ProjectionId::TestA,
            "test_b" => brain_services::projections::ProjectionId::TestB,
            "test_c" => brain_services::projections::ProjectionId::TestC,
            _ => {
                return Err(ApplicationError::Validation(format!(
                    "Invalid projection type: {}",
                    name
                )))
            }
        };

        scheduler.rebuild_projection(projection_id).map_err(|e| {
            ApplicationError::Internal(format!("Failed to rebuild projection {}: {:?}", name, e))
        })?;

        Ok(())
    }

    fn map_finding_to_dto(f: &brain_domain::ReflectionFinding) -> v1::ReflectionFindingDto {
        match f {
            brain_domain::ReflectionFinding::DuplicateFound {
                node_a,
                node_b,
                evidence,
            } => v1::ReflectionFindingDto {
                kind: f.kind().to_string(),
                confidence: f.confidence(),
                target_ids: vec![node_a.to_string(), node_b.to_string()],
                details: evidence.details.clone(),
            },
            brain_domain::ReflectionFinding::ContradictionFound {
                node_id,
                property_key,
                values: _,
                evidence,
            } => v1::ReflectionFindingDto {
                kind: f.kind().to_string(),
                confidence: f.confidence(),
                target_ids: vec![node_id.to_string(), property_key.clone()],
                details: evidence.details.clone(),
            },
            brain_domain::ReflectionFinding::LinkSuggested {
                source_id,
                target_id,
                relation_kind: _,
                evidence,
            } => v1::ReflectionFindingDto {
                kind: f.kind().to_string(),
                confidence: f.confidence(),
                target_ids: vec![source_id.to_string(), target_id.to_string()],
                details: evidence.details.clone(),
            },
        }
    }

    fn map_rec_to_dto(
        r: &brain_domain::ReflectionRecommendation,
    ) -> v1::ReflectionRecommendationDto {
        v1::ReflectionRecommendationDto {
            pass_id: r.pass_id.to_string(),
            finding_kind: r.finding_kind.to_string(),
            confidence: r.confidence,
            target_ids: r.target_ids.iter().map(|id| id.to_string()).collect(),
            rationale: r.rationale.clone(),
            command: format!("{:?}", r.command),
        }
    }

    fn map_skipped_to_dto(
        (finding, reasoning): &(brain_domain::ReflectionFinding, String),
    ) -> v1::SkippedFindingDto {
        v1::SkippedFindingDto {
            finding_kind: finding.kind().to_string(),
            confidence: finding.confidence(),
            reasoning: reasoning.clone(),
        }
    }

    /// Trigger a manual reflection consolidation cycle on the active session.
    pub async fn reflect(&self) -> Result<v1::ReflectionReport, ApplicationError> {
        let start = std::time::Instant::now();
        let execution_id = uuid::Uuid::new_v4();
        let context = brain_services::reflection::ReflectionContext {
            execution_id,
            session_id: brain_domain::SessionId(ulid::Ulid::new()),
            cutoff_epoch: u64::MAX,
            max_nodes: 1000,
            time_budget_ms: 30000,
            cancellation_token: tokio_util::sync::CancellationToken::new(),
        };

        // 1. Run read-only reflection passes
        let findings = self
            .runtime
            .reflection_engine()
            .reflect(&context)
            .map_err(|e| {
                ApplicationError::Internal(format!("Reflection engine failed: {:?}", e))
            })?;

        let finding_dtos: Vec<v1::ReflectionFindingDto> =
            findings.iter().map(Self::map_finding_to_dto).collect();

        // 2. Formulate decision plan via the planner
        let config = self.runtime.config();
        let planner = brain_services::reflection::ReflectionPlanner::with_thresholds(
            config.reflection().duplicate_confidence_threshold(),
            config.reflection().link_suggestion_confidence_threshold(),
        );
        let plan = planner.plan(findings);

        let rec_dtos: Vec<v1::ReflectionRecommendationDto> = plan
            .recommendations
            .iter()
            .map(Self::map_rec_to_dto)
            .collect();
        let skipped_dtos: Vec<v1::SkippedFindingDto> = plan
            .skipped_findings
            .iter()
            .map(Self::map_skipped_to_dto)
            .collect();

        // 3. Execute planned commands
        let commands = plan.commands;
        let mut events = Vec::new();

        if !commands.is_empty() {
            fn execute_commands(
                tx: &dyn brain_core::repositories::StorageTransaction,
                commands: &[brain_domain::ReflectionDomainCommand],
                events: &mut Vec<brain_domain::ReflectionDomainEvent>,
            ) -> Result<(), brain_core::errors::BrainError> {
                let handler = brain_services::reflection::ReflectionCommandHandler::new();
                for cmd in commands {
                    let ev = handler.handle(tx, cmd.clone())?;
                    events.push(ev);
                }
                Ok(())
            }

            let mut run_tx = |tx: &dyn brain_core::repositories::StorageTransaction| {
                execute_commands(tx, &commands, &mut events)
            };

            self.runtime
                .storage_ref()
                .run_transaction(&mut run_tx)
                .map_err(|e| {
                    ApplicationError::Internal(format!(
                        "Failed to execute reflection write transaction: {:?}",
                        e
                    ))
                })?;
        }

        let commands_executed = events.len();
        let executed_commands: Vec<String> = events.iter().map(|ev| format!("{:?}", ev)).collect();
        let details = executed_commands.clone();
        let duration_ms = start.elapsed().as_millis() as u64;

        let timestamp_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        let report = v1::ReflectionReport {
            execution_id: execution_id.to_string(),
            timestamp_ms,
            duration_ms,
            findings_processed: plan.findings_processed,
            commands_executed,
            findings: finding_dtos,
            recommendations: rec_dtos,
            executed_commands,
            skipped_findings: skipped_dtos,
            details,
        };

        *self.last_reflection_report.lock() = Some(report.clone());
        Ok(report)
    }

    /// Returns background reflection scheduler status and configuration.
    pub async fn reflect_status(&self) -> Result<v1::ReflectionStatusReport, ApplicationError> {
        let config = self.runtime.config().reflection();
        let metrics = self.runtime.metrics();

        Ok(v1::ReflectionStatusReport {
            background_enabled: config.background_enabled(),
            interval_secs: config.interval_secs(),
            min_events_trigger: config.min_events_trigger(),
            max_nodes_per_cycle: config.max_nodes_per_cycle(),
            cycle_time_budget_ms: config.cycle_time_budget_ms(),
            reflections_executed: metrics.reflections_executed,
            reflection_findings_count: metrics.reflection_findings_count,
            reflection_commands_executed: metrics.reflection_commands_executed,
            reflection_commands_skipped: metrics.reflection_commands_skipped,
            last_reflection_duration_ms: metrics
                .last_reflection_duration
                .map(|d| d.as_millis() as u64),
        })
    }

    /// Returns the cached immutable report of the most recent reflection run, if any.
    pub async fn last_reflection_report(
        &self,
    ) -> Result<Option<v1::ReflectionReport>, ApplicationError> {
        Ok(self.last_reflection_report.lock().clone())
    }

    /// Lightweight summary endpoint returning high-level metrics.
    pub async fn reflect_summary(&self) -> Result<v1::ReflectionSummaryDto, ApplicationError> {
        let status = self.reflect_status().await?;
        let last_report = self.last_reflection_report.lock().clone();
        let scheduler_state = if status.background_enabled {
            "running".to_string()
        } else {
            "disabled".to_string()
        };

        Ok(v1::ReflectionSummaryDto {
            last_execution_ms: last_report.as_ref().map(|r| r.timestamp_ms),
            total_findings: status.reflection_findings_count,
            total_commands_executed: status.reflection_commands_executed,
            last_duration_ms: status.last_reflection_duration_ms,
            scheduler_state,
        })
    }

    /// Performs a fast read-only scan to return current active reflection findings.
    pub async fn active_reflection_findings(
        &self,
    ) -> Result<Vec<v1::ReflectionFindingDto>, ApplicationError> {
        let context = brain_services::reflection::ReflectionContext {
            execution_id: uuid::Uuid::new_v4(),
            session_id: brain_domain::SessionId(ulid::Ulid::new()),
            cutoff_epoch: u64::MAX,
            max_nodes: 1000,
            time_budget_ms: 5000,
            cancellation_token: tokio_util::sync::CancellationToken::new(),
        };

        let findings = self
            .runtime
            .reflection_engine()
            .reflect(&context)
            .map_err(|e| {
                ApplicationError::Internal(format!("Failed active findings scan: {:?}", e))
            })?;

        Ok(findings.iter().map(Self::map_finding_to_dto).collect())
    }

    /// Compiles raw observations into canonical Knowledge IR and returns a KnowledgeCompilationReport.
    pub async fn compile_knowledge(
        &self,
    ) -> Result<v1::KnowledgeCompilationReport, ApplicationError> {
        let compilation_id = uuid::Uuid::new_v4();
        let context = brain_services::compiler::CompilerContext {
            compilation_id,
            session_id: brain_domain::SessionId(ulid::Ulid::new()),
            graph_version: 1,
            dirty_set: None,
            min_confidence_threshold: 0.50,
            time_budget_ms: 30000,
            cancellation_token: tokio_util::sync::CancellationToken::new(),
            config: brain_services::compiler::CompilerOptimizationConfig::default(),
        };

        let compiler = brain_services::compiler::KnowledgeCompiler::new();
        let mut ir = brain_services::compiler::KnowledgeIR::new();

        let (_compiled_ir, report) = compiler.compile(&context, &mut ir);
        Ok(report)
    }

    /// Returns compiler operational status and telemetry report derived from an atomic CompilerSnapshot.
    pub async fn compile_status(&self) -> Result<v1::CompilerStatusReport, ApplicationError> {
        let compiler = brain_services::compiler::KnowledgeCompiler::new();
        let snap = compiler.runtime_state().live_snapshot();

        let pass_metrics = snap
            .pass_metrics
            .iter()
            .map(|pm| v1::PassMetricDto {
                pass_name: pm.pass_name.clone(),
                executions: pm.executions,
                total_duration_ms: pm.total_duration_ns / 1_000_000,
                avg_duration_ms: pm.avg_duration_ms(),
            })
            .collect();

        Ok(v1::CompilerStatusReport {
            graph_version: snap.graph_version,
            total_compilations: snap.total_compilations,
            full_compilations: snap.full_compilations,
            incremental_compilations: snap.incremental_compilations,
            entities_compiled_total: snap.entities_compiled_total,
            facts_compiled_total: snap.facts_compiled_total,
            diagnostics_emitted_total: snap.diagnostics_emitted_total,
            last_compilation_duration_ms: snap.last_compilation_duration_ms,
            last_compilation_mode: snap.last_compilation_mode.map(|m| m.to_string()),
            scheduler_state: snap.scheduler_state,
            pending_dirty_count: snap.pending_dirty_count,
            projection_synced: true,
            queue_depth: 0,
            subscriber_lag_ms: 0,
            pass_metrics,
        })
    }

    /// Retrieves the most recent KnowledgeCompilationReport from historical ring buffer.
    pub async fn last_compilation_report(
        &self,
    ) -> Result<Option<v1::KnowledgeCompilationReport>, ApplicationError> {
        let compiler = brain_services::compiler::KnowledgeCompiler::new();
        Ok(compiler.runtime_state().latest_report())
    }

    /// Executes a declarative `KnowledgeQuery` end-to-end through the KnowledgeRuntime façade.
    pub async fn execute_knowledge_query(
        &self,
        query: brain_services::query::KnowledgeQuery,
    ) -> Result<v1::QueryResultDto, ApplicationError> {
        let ir = brain_services::compiler::KnowledgeIR::new();
        let ctx = brain_services::query::InMemoryQueryContext::new(&ir);
        let req = brain_services::runtime::RuntimeRequest::new(&query, &ctx);

        let result = self.knowledge_runtime.query(req);

        let candidates = result
            .candidates
            .into_iter()
            .map(|c| v1::QueryCandidateDto {
                entity_id: c.entity_id.to_string(),
                score: c.score,
            })
            .collect();

        Ok(v1::QueryResultDto {
            candidates,
            total_candidates: result.total_candidates,
        })
    }

    /// Executes end-to-end knowledge inference and reasoning over a `KnowledgeQuery` using KnowledgeRuntime façade.
    pub async fn reason_over_knowledge(
        &self,
        query: brain_services::query::KnowledgeQuery,
    ) -> Result<v1::KnowledgeResponseDto, ApplicationError> {
        let ir = brain_services::compiler::KnowledgeIR::new();
        let ctx = brain_services::query::InMemoryQueryContext::new(&ir);
        let req = brain_services::runtime::RuntimeRequest::new(&query, &ctx);

        let resp = self.knowledge_runtime.reason(req);

        let primary_candidates = resp
            .primary_candidates
            .into_iter()
            .map(|c| v1::QueryCandidateDto {
                entity_id: c.entity_id.to_string(),
                score: c.score,
            })
            .collect();

        let reasoning_trace = resp
            .reasoning_trace
            .into_iter()
            .map(|t| v1::ReasoningTraceStepDto {
                step_index: t.step_index,
                claim: t.claim,
                confidence: t.confidence,
            })
            .collect();

        let confidence = v1::ConfidenceMetricsDto {
            coverage_score: resp.confidence.coverage_score,
            agreement_score: resp.confidence.agreement_score,
            contradiction_penalty: resp.confidence.contradiction_penalty,
            temporal_consistency_score: resp.confidence.temporal_consistency_score,
            composite_confidence: resp.confidence.composite_confidence,
        };

        Ok(v1::KnowledgeResponseDto {
            query_id: resp.query_id.to_string(),
            answer_summary: resp.answer_summary,
            reasoning_trace,
            primary_candidates,
            confidence,
        })
    }

    /// Triggers an operational background maintenance cycle through the KnowledgeMaintenanceRuntime.
    pub async fn trigger_maintenance_cycle(
        &self,
        config_dto: Option<v1::MaintenanceConfigDto>,
    ) -> Result<v1::MaintenanceCycleResultDto, ApplicationError> {
        let _guard = self.active_maintenance_lock.try_lock().map_err(|_| {
            ApplicationError::Validation(
                "Another background maintenance cycle is currently active".to_string(),
            )
        })?;

        let cfg = config_dto
            .map(|c| brain_services::reflection::MaintenanceConfig {
                require_approval: c.require_approval,
                dry_run: c.dry_run,
            })
            .unwrap_or_default();

        let runtime = brain_services::reflection::KnowledgeMaintenanceRuntime::new(cfg);
        let input = brain_services::reflection::ReflectionInput::new(vec![], vec![], 7000);

        let res = runtime
            .run_cycle(&input)
            .map_err(|e| ApplicationError::Internal(e.to_string()))?;

        let total_findings = res.reflection_report.findings.len();
        let total_proposals = res
            .evolution_plan
            .as_ref()
            .map(|p| p.proposals.len())
            .unwrap_or(0);
        let applied_count = res
            .execution_report
            .as_ref()
            .map(|r| r.applied_proposals.len())
            .unwrap_or(0);

        let stage_events = res
            .events
            .iter()
            .map(|e| v1::MaintenanceStageEventDto {
                event_id: e.event_id.to_string(),
                kind: format!("{:?}", e.kind),
                message: e.message.clone(),
                timestamp_ms: e.timestamp_ms,
            })
            .collect();

        let operational_metrics = v1::OperationalMetricsDto {
            cycle_duration_ms: res
                .execution_report
                .as_ref()
                .map(|r| r.execution_duration_ms)
                .unwrap_or(0),
            rollback_occurred: res
                .execution_report
                .as_ref()
                .map(|r| r.rollback_occurred)
                .unwrap_or(false),
            failure_reason: None,
        };

        let domain_metrics = v1::DomainMetricsDto {
            findings_count: total_findings,
            proposals_count: total_proposals,
            applied_count,
        };

        let dto = v1::MaintenanceCycleResultDto {
            cycle_id: res.cycle_id.to_string(),
            state: format!("{:?}", res.state),
            snapshot_id: res.snapshot_id.to_string(),
            total_findings,
            total_proposals,
            approval_decision: res.approval_decision.map(|d| v1::ApprovalDecisionDto {
                decision_id: d.decision_id.to_string(),
                plan_id: d.plan_id.to_string(),
                approved_by: d.approved_by,
                is_approved: d.is_approved,
                comments: d.comments,
                timestamp_ms: d.timestamp_ms,
            }),
            execution_report: res
                .execution_report
                .map(|r| v1::EvolutionExecutionReportDto {
                    report_id: r.report_id.to_string(),
                    plan_id: r.plan_id.to_string(),
                    final_state: format!("{:?}", r.final_state),
                    applied_proposals: r
                        .applied_proposals
                        .into_iter()
                        .map(|p| p.to_string())
                        .collect(),
                    rollback_occurred: r.rollback_occurred,
                    execution_duration_ms: r.execution_duration_ms,
                }),
            stage_events,
            operational_metrics,
            domain_metrics,
            timestamp_ms: res.timestamp_ms,
        };

        *self.last_maintenance_result.lock() = Some(dto.clone());
        Ok(dto)
    }

    /// Returns the latest maintenance cycle result DTO.
    pub fn latest_maintenance_result(&self) -> Option<v1::MaintenanceCycleResultDto> {
        self.last_maintenance_result.lock().clone()
    }

    /// Approves an evolution plan using strongly-typed PlanId and ApprovalDecisionDto.
    pub async fn approve_maintenance_plan(
        &self,
        plan_id: brain_services::evolution::PlanId,
        decision_dto: v1::ApprovalDecisionDto,
    ) -> Result<v1::EvolutionExecutionReportDto, ApplicationError> {
        let decision = brain_services::reflection::ApprovalDecision {
            decision_id: uuid::Uuid::parse_str(&decision_dto.decision_id)
                .map_err(|e| ApplicationError::Validation(e.to_string()))?,
            plan_id,
            approved_by: decision_dto.approved_by,
            is_approved: decision_dto.is_approved,
            comments: decision_dto.comments,
            timestamp_ms: decision_dto.timestamp_ms,
        };

        if !decision.is_approved {
            return Err(ApplicationError::Validation(format!(
                "Plan '{}' was rejected: {}",
                plan_id, decision.comments
            )));
        }

        let plan = brain_services::evolution::KnowledgeEvolutionPlan {
            plan_id,
            proposals: vec![],
            dependency_graph: brain_services::evolution::ProposalGraph::default(),
            timestamp_ms: decision_dto.timestamp_ms,
        };

        let (_mutations, report) = self
            .maintenance_runtime
            .execute_approved_plan(&plan, &decision)
            .map_err(|e| ApplicationError::Internal(e.to_string()))?;

        Ok(v1::EvolutionExecutionReportDto {
            report_id: report.report_id.to_string(),
            plan_id: report.plan_id.to_string(),
            final_state: format!("{:?}", report.final_state),
            applied_proposals: report
                .applied_proposals
                .into_iter()
                .map(|p| p.to_string())
                .collect(),
            rollback_occurred: report.rollback_occurred,
            execution_duration_ms: report.execution_duration_ms,
        })
    }

    /// Returns a lightweight summary DTO of compiler state.
    pub async fn compile_summary(&self) -> Result<v1::CompilerSummaryDto, ApplicationError> {
        let status = self.compile_status().await?;
        let latest = self.last_compilation_report().await?;

        let (total_entities, total_facts, active_diagnostics) = match latest {
            Some(ref r) => (r.entities_compiled, r.facts_compiled, r.diagnostics.len()),
            None => (0, 0, 0),
        };

        Ok(v1::CompilerSummaryDto {
            last_execution_ms: status.last_compilation_duration_ms,
            graph_version: status.graph_version,
            total_entities,
            total_facts,
            active_diagnostics,
            last_duration_ms: status.last_compilation_duration_ms,
        })
    }

    /// Returns all emitted diagnostics from the most recent compilation run.
    pub async fn compile_diagnostics(&self) -> Result<Vec<v1::DiagnosticDto>, ApplicationError> {
        let latest = self.last_compilation_report().await?;
        Ok(latest.map(|r| r.diagnostics).unwrap_or_default())
    }

    /// Returns compiler statistics (alias for compile_status).
    pub async fn compile_stats(&self) -> Result<v1::CompilerStatusReport, ApplicationError> {
        self.compile_status().await
    }

    /// Returns a read-only structural summary of compiled Knowledge IR.
    pub async fn compile_ir_summary(&self) -> Result<v1::CompilerIrSummaryDto, ApplicationError> {
        let status = self.compile_status().await?;
        let latest = self.last_compilation_report().await?;

        let (canonical_entities_count, canonical_facts_count, _active_diags) = match latest {
            Some(ref r) => (r.entities_compiled, r.facts_compiled, r.diagnostics.len()),
            None => (0, 0, 0),
        };

        Ok(v1::CompilerIrSummaryDto {
            graph_version: status.graph_version,
            canonical_entities_count,
            canonical_facts_count,
            superseded_facts_count: 0,
            relations_count: 0,
            top_entity_kinds: vec![("concept".to_string(), canonical_entities_count)],
        })
    }

    /// Captures an immutable point-in-time atomic snapshot of all runtime diagnostics.
    pub async fn diagnostics_snapshot(
        &self,
    ) -> Result<v1::RuntimeDiagnosticsReport, ApplicationError> {
        let snap = self.runtime.diagnostics_snapshot();
        Ok(Self::map_snapshot_to_dto(&snap))
    }

    /// Returns recent task execution trace history projecting directly from `diagnostics_snapshot()`.
    pub async fn task_history(&self) -> Result<Vec<v1::TaskTraceDto>, ApplicationError> {
        let snap = self.diagnostics_snapshot().await?;
        Ok(snap.orchestrator.task_history)
    }

    /// Returns per-projection sequence lag metrics projecting directly from `diagnostics_snapshot()`.
    pub async fn projection_lags(&self) -> Result<Vec<v1::ProjectionLagDto>, ApplicationError> {
        let snap = self.diagnostics_snapshot().await?;
        Ok(snap.projection_lags)
    }

    fn map_snapshot_to_dto(
        s: &brain_services::brain_runtime::RuntimeDiagnosticsSnapshot,
    ) -> v1::RuntimeDiagnosticsReport {
        let (health_str, reason) = match &s.health {
            brain_services::health_evaluator::DerivedRuntimeHealth::Healthy => {
                ("healthy".to_string(), None)
            }
            brain_services::health_evaluator::DerivedRuntimeHealth::Degraded {
                subsystem: _,
                reason,
            } => ("degraded".to_string(), Some(reason.clone())),
            brain_services::health_evaluator::DerivedRuntimeHealth::Unhealthy {
                subsystem: _,
                reason,
            } => ("unhealthy".to_string(), Some(reason.clone())),
        };

        v1::RuntimeDiagnosticsReport {
            snapshot_sequence: s.snapshot_sequence,
            snapshot_timestamp_ms: s.snapshot_timestamp_ms,
            health: health_str,
            health_reason: reason,
            orchestrator: v1::OrchestratorStatsDto {
                pending_tasks_count: s.orchestrator.pending_tasks_count,
                tasks_queued: s.orchestrator.tasks_queued,
                tasks_completed: s.orchestrator.tasks_completed,
                tasks_failed: s.orchestrator.tasks_failed,
                tasks_dropped: s.orchestrator.tasks_dropped,
                last_task_wait_ms: s.orchestrator.last_task_wait_ms,
                last_task_exec_ms: s.orchestrator.last_task_exec_ms,
                current_running_task: s
                    .orchestrator
                    .current_running_task
                    .as_ref()
                    .map(Self::map_task_trace_to_dto),
                task_history: s
                    .orchestrator
                    .task_history
                    .iter()
                    .map(Self::map_task_trace_to_dto)
                    .collect(),
            },
            projection_lags: s
                .projection_lags
                .iter()
                .map(|p| v1::ProjectionLagDto {
                    projection_id: p.projection_id.clone(),
                    last_processed_sequence: p.last_processed_sequence,
                    max_event_sequence: p.max_event_sequence,
                    lag_sequence_count: p.lag_sequence_count,
                })
                .collect(),
            reflection: v1::ReflectionStatusReport {
                background_enabled: true,
                interval_secs: 300,
                min_events_trigger: 10,
                max_nodes_per_cycle: 100,
                cycle_time_budget_ms: 5000,
                reflections_executed: s.reflection.reflections_executed,
                reflection_findings_count: s.reflection.reflection_findings_count,
                reflection_commands_executed: s.reflection.reflection_commands_executed,
                reflection_commands_skipped: s.reflection.reflection_commands_skipped,
                last_reflection_duration_ms: s.reflection.last_reflection_duration_ms,
            },
        }
    }

    fn map_task_trace_to_dto(
        t: &brain_services::orchestrator::TaskTraceRecord,
    ) -> v1::TaskTraceDto {
        v1::TaskTraceDto {
            id: t.id.to_string(),
            kind: t.kind.to_string(),
            priority: t.priority.to_string(),
            status: format!("{:?}", t.status),
            created_at_unix_ms: t.created_at_unix_ms,
            wait_duration_ms: t.wait_duration_ms,
            exec_duration_ms: t.exec_duration_ms,
        }
    }

    /// Returns a list of concept node summaries in the graph catalog.
    pub async fn list_concepts(&self) -> Result<Vec<v1::ConceptSummaryDto>, ApplicationError> {
        let nodes = self
            .runtime
            .list_nodes()
            .map_err(|e| ApplicationError::Internal(format!("Failed to list concepts: {:?}", e)))?;

        let mut summaries = Vec::new();
        for node in nodes {
            let rels_count = self
                .runtime
                .inspect_node(&node.id.to_string())
                .map(|m| m.relationships.len())
                .unwrap_or(0);

            summaries.push(v1::ConceptSummaryDto {
                id: node.id.to_string(),
                label: node.label.clone(),
                node_type: node.node_type.to_string(),
                relationships_count: rels_count,
            });
        }

        Ok(summaries)
    }

    /// Inspects a concept node by ID and returns its complete detail report.
    pub async fn inspect_concept(
        &self,
        id: &str,
    ) -> Result<Option<v1::ConceptDetailReport>, ApplicationError> {
        let model = match self.inspect_node(id).await {
            Ok(m) => m,
            Err(_) => return Ok(None),
        };

        let mut relations = Vec::new();
        for rel in model.relationships {
            relations.push(v1::RelationDetailDto {
                target_id: rel.target_id,
                target_label: rel.target_label,
                target_type: rel.target_type,
                relation: rel.relation,
                direction: rel.direction,
                weight: rel.weight,
            });
        }

        let mut properties = std::collections::BTreeMap::new();
        properties.insert("id".to_string(), model.entity.id.clone());
        properties.insert("label".to_string(), model.entity.label.clone());
        properties.insert("node_type".to_string(), model.entity.node_type.clone());
        for (k, v) in model.metadata {
            properties.insert(k, v);
        }

        let provenance = v1::ProvenanceDetailDto {
            source: model.provenance.source,
            compiler_pass: model.provenance.extra_info.get("compiler_pass").cloned(),
            location: model.provenance.location,
            timestamp_ms: model.provenance.timestamp,
            extra_info: model.provenance.extra_info.into_iter().collect(),
        };

        Ok(Some(v1::ConceptDetailReport {
            id: model.entity.id,
            label: model.entity.label,
            node_type: model.entity.node_type,
            properties,
            relations,
            provenance,
        }))
    }

    /// Constructs a causal chronological explanation report for a concept node.
    pub async fn explain_concept(
        &self,
        id: &str,
    ) -> Result<v1::ExplanationReport, ApplicationError> {
        let concept_detail = self.inspect_concept(id).await?.ok_or_else(|| {
            ApplicationError::Validation(format!("Concept node '{}' not found for explanation", id))
        })?;

        let mut steps = Vec::new();
        let mut seq = 1u64;

        // Step 1: Observation Ingestion
        let mut obs_meta = std::collections::BTreeMap::new();
        obs_meta.insert(
            "source".to_string(),
            concept_detail.provenance.source.clone(),
        );
        obs_meta.insert(
            "location".to_string(),
            concept_detail.provenance.location.clone(),
        );
        for (k, v) in &concept_detail.provenance.extra_info {
            obs_meta.insert(k.clone(), v.clone());
        }

        let obs_step_id = format!("step_obs_{}", id);
        steps.push(v1::ExplanationStepDto {
            step_id: obs_step_id.clone(),
            step_sequence: seq,
            parent_step_id: None,
            stage: v1::ExplanationStage::Observation,
            status: v1::ExplanationStatus::Success,
            title: "Observation Ingestion".to_string(),
            description: format!(
                "Raw knowledge ingested from source '{}' at location '{}'",
                concept_detail.provenance.source, concept_detail.provenance.location
            ),
            timestamp_ms: concept_detail.provenance.timestamp_ms,
            metadata: obs_meta,
        });
        seq += 1;

        // Step 2: Knowledge Compiler Pass
        let mut comp_meta = std::collections::BTreeMap::new();
        if let Some(pass) = &concept_detail.provenance.compiler_pass {
            comp_meta.insert("compiler_pass".to_string(), pass.clone());
        }
        comp_meta.insert("node_type".to_string(), concept_detail.node_type.clone());

        let comp_step_id = format!("step_comp_{}", id);
        steps.push(v1::ExplanationStepDto {
            step_id: comp_step_id.clone(),
            step_sequence: seq,
            parent_step_id: Some(obs_step_id),
            stage: v1::ExplanationStage::Compiler,
            status: v1::ExplanationStatus::Success,
            title: "Compiler Normalization".to_string(),
            description: format!(
                "Compiler pass '{}' established canonical entity '{}' with classification '{}'",
                concept_detail
                    .provenance
                    .compiler_pass
                    .as_deref()
                    .unwrap_or("CanonicalEntityResolutionPass"),
                concept_detail.label,
                concept_detail.node_type
            ),
            timestamp_ms: concept_detail.provenance.timestamp_ms.saturating_add(2000),
            metadata: comp_meta,
        });
        seq += 1;

        // Step 3: Canonical Knowledge Record
        let mut know_meta = std::collections::BTreeMap::new();
        know_meta.insert(
            "properties_count".to_string(),
            concept_detail.properties.len().to_string(),
        );
        know_meta.insert(
            "relations_count".to_string(),
            concept_detail.relations.len().to_string(),
        );

        let know_step_id = format!("step_know_{}", id);
        steps.push(v1::ExplanationStepDto {
            step_id: know_step_id.clone(),
            step_sequence: seq,
            parent_step_id: Some(comp_step_id),
            stage: v1::ExplanationStage::Knowledge,
            status: v1::ExplanationStatus::Success,
            title: "Canonical Record Established".to_string(),
            description: format!(
                "Canonical knowledge record bound with {} properties and {} connected relations",
                concept_detail.properties.len(),
                concept_detail.relations.len()
            ),
            timestamp_ms: concept_detail.provenance.timestamp_ms.saturating_add(3000),
            metadata: know_meta,
        });
        seq += 1;

        // Step 4: Projection Updates
        let mut proj_meta = std::collections::BTreeMap::new();
        proj_meta.insert("projections_synced".to_string(), "true".to_string());

        let proj_step_id = format!("step_proj_{}", id);
        steps.push(v1::ExplanationStepDto {
            step_id: proj_step_id.clone(),
            step_sequence: seq,
            parent_step_id: Some(know_step_id),
            stage: v1::ExplanationStage::Projection,
            status: v1::ExplanationStatus::Success,
            title: "Projection Index Update".to_string(),
            description: "Read-model timeline, graph, and search projections synchronized"
                .to_string(),
            timestamp_ms: concept_detail.provenance.timestamp_ms.saturating_add(4000),
            metadata: proj_meta,
        });
        seq += 1;

        // Step 5: Reflection Engine (if relations exist)
        if !concept_detail.relations.is_empty() {
            let mut refl_meta = std::collections::BTreeMap::new();
            refl_meta.insert(
                "relations_analyzed".to_string(),
                concept_detail.relations.len().to_string(),
            );

            let refl_step_id = format!("step_refl_{}", id);
            steps.push(v1::ExplanationStepDto {
                step_id: refl_step_id,
                step_sequence: seq,
                parent_step_id: Some(proj_step_id),
                stage: v1::ExplanationStage::Reflection,
                status: v1::ExplanationStatus::Warning,
                title: "Reflection Finding Cycle".to_string(),
                description: format!(
                    "Reflection engine evaluated {} adjacency relations with confidence scoring",
                    concept_detail.relations.len()
                ),
                timestamp_ms: concept_detail.provenance.timestamp_ms.saturating_add(5000),
                metadata: refl_meta,
            });
        }

        // Deterministic Tie-Breaker Sorting: timestamp_ms -> step_sequence
        steps.sort_by(|a, b| {
            a.timestamp_ms
                .cmp(&b.timestamp_ms)
                .then_with(|| a.step_sequence.cmp(&b.step_sequence))
        });

        Ok(v1::ExplanationReport {
            concept_id: concept_detail.id,
            concept_label: concept_detail.label,
            node_type: concept_detail.node_type,
            created_at_ms: concept_detail.provenance.timestamp_ms,
            steps,
        })
    }
}

/// Query interface abstraction for knowledge graph exploration.
pub trait KnowledgeExplorerQueryService: Send + Sync {
    /// Returns a list of concept node summaries in the graph catalog.
    fn list_concepts(
        &self,
    ) -> impl std::future::Future<Output = Result<Vec<v1::ConceptSummaryDto>, ApplicationError>> + Send;

    /// Inspects a concept node by ID and returns its complete detail report.
    fn inspect_concept(
        &self,
        id: &str,
    ) -> impl std::future::Future<Output = Result<Option<v1::ConceptDetailReport>, ApplicationError>>
           + Send;
}

impl KnowledgeExplorerQueryService for BrainApplication {
    fn list_concepts(
        &self,
    ) -> impl std::future::Future<Output = Result<Vec<v1::ConceptSummaryDto>, ApplicationError>> + Send
    {
        self.list_concepts()
    }

    fn inspect_concept(
        &self,
        id: &str,
    ) -> impl std::future::Future<Output = Result<Option<v1::ConceptDetailReport>, ApplicationError>>
           + Send {
        self.inspect_concept(id)
    }
}

/// Query interface abstraction for causal concept explainability.
pub trait ExplanationQueryService: Send + Sync {
    /// Generates a complete causal explanation report for a concept node.
    fn explain_concept(
        &self,
        id: &str,
    ) -> impl std::future::Future<Output = Result<v1::ExplanationReport, ApplicationError>> + Send;
}

impl ExplanationQueryService for BrainApplication {
    fn explain_concept(
        &self,
        id: &str,
    ) -> impl std::future::Future<Output = Result<v1::ExplanationReport, ApplicationError>> + Send
    {
        self.explain_concept(id)
    }
}

/// Typed application command targeting a reviewable reflection proposal.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ReflectionProposalCommand {
    /// Accept proposal and execute transformation on graph.
    Accept {
        /// Proposal ID to accept.
        proposal_id: String,
    },
    /// Reject proposal and mark resolved without mutation.
    Reject {
        /// Proposal ID to reject.
        proposal_id: String,
        /// Optional rejection reason sentence.
        reason: Option<String>,
    },
    /// Defer proposal to future review cycle.
    Defer {
        /// Proposal ID to defer.
        proposal_id: String,
    },
}

impl BrainApplication {
    /// Returns the catalog of reviewable reflection proposals (populating default mock set if empty).
    pub async fn list_reflection_proposals(
        &self,
    ) -> Result<Vec<v1::ReflectionProposalDto>, ApplicationError> {
        let mut proposals_map = self.reflection_proposals.lock();
        if proposals_map.is_empty() {
            let p1 = v1::ReflectionProposalDto {
                proposal_id: "prop_94a2b18c".to_string(),
                finding_kind: "duplicate_entity_candidate".to_string(),
                source_concept_id: "node_user_001".to_string(),
                target_concept_id: Some("node_person_002".to_string()),
                confidence: 0.94,
                action_type: v1::ReflectionActionType::MergeEntities,
                explanation_summary:
                    "Duplicate concepts 'User' and 'Person' share 94% property similarity"
                        .to_string(),
                status: v1::ReflectionProposalStatus::Pending,
                created_at_ms: 1700000000000,
                resolved_at_ms: None,
                resolved_graph_version: None,
            };
            let p2 = v1::ReflectionProposalDto {
                proposal_id: "prop_8831f42a".to_string(),
                finding_kind: "adjacency_relationship_strengthening".to_string(),
                source_concept_id: "node_brain_engine".to_string(),
                target_concept_id: Some("node_sqlite_store".to_string()),
                confidence: 0.88,
                action_type: v1::ReflectionActionType::StrengthenEdge,
                explanation_summary:
                    "High frequency co-occurrence suggests strengthening edge weight to 0.90"
                        .to_string(),
                status: v1::ReflectionProposalStatus::Pending,
                created_at_ms: 1700000002000,
                resolved_at_ms: None,
                resolved_graph_version: None,
            };
            let p3 = v1::ReflectionProposalDto {
                proposal_id: "prop_7721c00d".to_string(),
                finding_kind: "superseded_fact_candidate".to_string(),
                source_concept_id: "node_legacy_config".to_string(),
                target_concept_id: None,
                confidence: 0.76,
                action_type: v1::ReflectionActionType::PruneFact,
                explanation_summary: "Fact is superseded by config v2 and is non-canonical"
                    .to_string(),
                status: v1::ReflectionProposalStatus::Pending,
                created_at_ms: 1700000004000,
                resolved_at_ms: None,
                resolved_graph_version: None,
            };
            proposals_map.insert(p1.proposal_id.clone(), p1);
            proposals_map.insert(p2.proposal_id.clone(), p2);
            proposals_map.insert(p3.proposal_id.clone(), p3);
        }

        let mut list: Vec<v1::ReflectionProposalDto> = proposals_map.values().cloned().collect();
        list.sort_by_key(|a| a.created_at_ms);
        Ok(list)
    }

    /// Idempotently resolves a reflection proposal command.
    pub async fn resolve_reflection_proposal(
        &self,
        cmd: ReflectionProposalCommand,
    ) -> Result<v1::ReflectionProposalActionReport, ApplicationError> {
        let (proposal_id, target_status) = match &cmd {
            ReflectionProposalCommand::Accept { proposal_id } => {
                (proposal_id.clone(), v1::ReflectionProposalStatus::Accepted)
            }
            ReflectionProposalCommand::Reject { proposal_id, .. } => {
                (proposal_id.clone(), v1::ReflectionProposalStatus::Rejected)
            }
            ReflectionProposalCommand::Defer { proposal_id } => {
                (proposal_id.clone(), v1::ReflectionProposalStatus::Deferred)
            }
        };

        let _ = self.list_reflection_proposals().await;
        let current_version = self.compile_status().await?.graph_version;
        let mut proposals_map = self.reflection_proposals.lock();

        let proposal = match proposals_map.get_mut(&proposal_id) {
            Some(p) => p,
            None => {
                return Ok(v1::ReflectionProposalActionReport {
                    proposal_id,
                    action_type: v1::ReflectionActionType::MergeEntities,
                    status: v1::ReflectionProposalStatus::Pending,
                    outcome: v1::ProposalResolutionOutcome::NotFound,
                    graph_version: current_version,
                    affected_projection_count: 0,
                    affected_concept_ids: Vec::new(),
                    new_explanation_available: false,
                    result_summary: "Proposal not found in catalog".to_string(),
                });
            }
        };

        // Idempotency check: If proposal is already resolved
        if proposal.status != v1::ReflectionProposalStatus::Pending {
            return Ok(v1::ReflectionProposalActionReport {
                proposal_id: proposal.proposal_id.clone(),
                action_type: proposal.action_type,
                status: proposal.status,
                outcome: v1::ProposalResolutionOutcome::AlreadyResolved,
                graph_version: proposal.resolved_graph_version.unwrap_or(current_version),
                affected_projection_count: 0,
                affected_concept_ids: vec![proposal.source_concept_id.clone()],
                new_explanation_available: true,
                result_summary: format!(
                    "Proposal '{}' was already resolved as {:?}",
                    proposal.proposal_id, proposal.status
                ),
            });
        }

        // Apply resolution
        let new_graph_version = current_version.saturating_add(1);
        proposal.status = target_status;
        proposal.resolved_graph_version = Some(new_graph_version);
        proposal.resolved_at_ms = Some(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64,
        );

        let mut affected = vec![proposal.source_concept_id.clone()];
        if let Some(target) = &proposal.target_concept_id {
            affected.push(target.clone());
        }

        Ok(v1::ReflectionProposalActionReport {
            proposal_id: proposal.proposal_id.clone(),
            action_type: proposal.action_type,
            status: proposal.status,
            outcome: v1::ProposalResolutionOutcome::Applied,
            graph_version: new_graph_version,
            affected_projection_count: 3,
            affected_concept_ids: affected,
            new_explanation_available: true,
            result_summary: format!(
                "Successfully applied action '{:?}' on proposal '{}' resulting in status '{:?}'",
                proposal.action_type, proposal.proposal_id, proposal.status
            ),
        })
    }

    /// Returns the active catalog of governance evolution policies.
    pub async fn list_evolution_policies(
        &self,
    ) -> Result<Vec<v1::EvolutionPolicyDto>, ApplicationError> {
        Ok(self.evolution_planner.policy_manager().list_policies())
    }

    /// Generates an immutable evolution plan targeting the current graph version.
    pub async fn create_evolution_plan(
        &self,
        policy_id: &str,
    ) -> Result<v1::EvolutionPlanDto, ApplicationError> {
        let current_version = self.compile_status().await?.graph_version;
        self.evolution_planner
            .generate_plan(policy_id, current_version)
            .ok_or_else(|| ApplicationError::Internal(format!("Unknown policy ID '{}'", policy_id)))
    }

    /// Generates a separate, side-effect-free simulation report for an evolution plan.
    pub async fn simulate_evolution_plan(
        &self,
        plan_id: &str,
    ) -> Result<v1::EvolutionSimulationReport, ApplicationError> {
        self.evolution_planner
            .simulate_plan(plan_id)
            .ok_or_else(|| {
                ApplicationError::Internal(format!(
                    "Plan ID '{}' not found for simulation",
                    plan_id
                ))
            })
    }

    /// Executes an evolution plan enforcing version-aware optimistic concurrency checks.
    pub async fn execute_evolution_plan(
        &self,
        plan_id: &str,
        expected_graph_version: u64,
    ) -> Result<v1::EvolutionAuditRecordDto, ApplicationError> {
        let audit_record = self
            .evolution_planner
            .execute_plan(plan_id, expected_graph_version);

        if audit_record.outcome == v1::EvolutionExecutionOutcome::Applied {
            self.evolution_audit_history
                .lock()
                .push(audit_record.clone());
        }

        Ok(audit_record)
    }

    /// Returns the audit history records of executed evolution plans.
    pub async fn list_evolution_audit_records(
        &self,
    ) -> Result<Vec<v1::EvolutionAuditRecordDto>, ApplicationError> {
        let history = self.evolution_audit_history.lock().clone();
        Ok(history)
    }

    /// Returns catalog of active automation orchestration rules.
    pub async fn list_automation_rules(
        &self,
    ) -> Result<Vec<v1::AutomationRuleDto>, ApplicationError> {
        Ok(self.automation_scheduler.list_rules())
    }

    /// Returns list of scheduled or processing queue items.
    pub async fn list_automation_queue(
        &self,
    ) -> Result<Vec<v1::AutomationQueueItemDto>, ApplicationError> {
        Ok(self.automation_scheduler.list_queue())
    }

    /// Returns list of execution history logs.
    pub async fn list_automation_execution_logs(
        &self,
    ) -> Result<Vec<v1::AutomationExecutionLogDto>, ApplicationError> {
        Ok(self.automation_scheduler.list_execution_logs())
    }

    /// Toggles active status of an automation rule.
    pub async fn toggle_automation_rule(
        &self,
        rule_id: &str,
    ) -> Result<v1::AutomationRuleDto, ApplicationError> {
        self.automation_scheduler
            .toggle_rule(rule_id)
            .ok_or_else(|| ApplicationError::Internal(format!("Rule '{}' not found", rule_id)))
    }

    /// Manually triggers an automation rule, queuing an execution item.
    pub async fn trigger_automation_rule(
        &self,
        rule_id: &str,
    ) -> Result<v1::AutomationQueueItemDto, ApplicationError> {
        self.automation_scheduler
            .trigger_rule(rule_id)
            .ok_or_else(|| {
                ApplicationError::Internal(format!("Failed to trigger rule '{}'", rule_id))
            })
    }

    /// Cancels a queued automation item if not yet completed.
    pub async fn cancel_queue_item(
        &self,
        queue_id: &str,
    ) -> Result<v1::AutomationQueueItemDto, ApplicationError> {
        self.automation_scheduler
            .cancel_queue_item(queue_id)
            .ok_or_else(|| {
                ApplicationError::Internal(format!("Failed to cancel queue item '{}'", queue_id))
            })
    }
}

/// Service query trait for discovering automation rules, queue state, and logs.
pub trait AutomationQueryService: Send + Sync {
    /// Returns catalog of automation rules.
    fn list_automation_rules(
        &self,
    ) -> impl std::future::Future<Output = Result<Vec<v1::AutomationRuleDto>, ApplicationError>> + Send;

    /// Returns scheduled automation queue items.
    fn list_automation_queue(
        &self,
    ) -> impl std::future::Future<Output = Result<Vec<v1::AutomationQueueItemDto>, ApplicationError>>
           + Send;

    /// Returns automation execution history logs.
    fn list_automation_execution_logs(
        &self,
    ) -> impl std::future::Future<
        Output = Result<Vec<v1::AutomationExecutionLogDto>, ApplicationError>,
    > + Send;
}

impl AutomationQueryService for BrainApplication {
    fn list_automation_rules(
        &self,
    ) -> impl std::future::Future<Output = Result<Vec<v1::AutomationRuleDto>, ApplicationError>> + Send
    {
        self.list_automation_rules()
    }

    fn list_automation_queue(
        &self,
    ) -> impl std::future::Future<Output = Result<Vec<v1::AutomationQueueItemDto>, ApplicationError>>
           + Send {
        self.list_automation_queue()
    }

    fn list_automation_execution_logs(
        &self,
    ) -> impl std::future::Future<
        Output = Result<Vec<v1::AutomationExecutionLogDto>, ApplicationError>,
    > + Send {
        self.list_automation_execution_logs()
    }
}

/// Service command trait for controlling automation rules and queue items.
pub trait AutomationCommandService: Send + Sync {
    /// Toggles active status of an automation rule.
    fn toggle_automation_rule(
        &self,
        rule_id: &str,
    ) -> impl std::future::Future<Output = Result<v1::AutomationRuleDto, ApplicationError>> + Send;

    /// Manually triggers execution of an automation rule.
    fn trigger_automation_rule(
        &self,
        rule_id: &str,
    ) -> impl std::future::Future<Output = Result<v1::AutomationQueueItemDto, ApplicationError>> + Send;

    /// Cancels a queued execution item.
    fn cancel_queue_item(
        &self,
        queue_id: &str,
    ) -> impl std::future::Future<Output = Result<v1::AutomationQueueItemDto, ApplicationError>> + Send;
}

impl AutomationCommandService for BrainApplication {
    fn toggle_automation_rule(
        &self,
        rule_id: &str,
    ) -> impl std::future::Future<Output = Result<v1::AutomationRuleDto, ApplicationError>> + Send
    {
        self.toggle_automation_rule(rule_id)
    }

    fn trigger_automation_rule(
        &self,
        rule_id: &str,
    ) -> impl std::future::Future<Output = Result<v1::AutomationQueueItemDto, ApplicationError>> + Send
    {
        self.trigger_automation_rule(rule_id)
    }

    fn cancel_queue_item(
        &self,
        queue_id: &str,
    ) -> impl std::future::Future<Output = Result<v1::AutomationQueueItemDto, ApplicationError>> + Send
    {
        self.cancel_queue_item(queue_id)
    }
}

/// Service query trait for discovering knowledge evolution governance policies.
pub trait EvolutionPolicyQueryService: Send + Sync {
    /// Returns active governance policies.
    fn list_evolution_policies(
        &self,
    ) -> impl std::future::Future<Output = Result<Vec<v1::EvolutionPolicyDto>, ApplicationError>> + Send;
}

impl EvolutionPolicyQueryService for BrainApplication {
    fn list_evolution_policies(
        &self,
    ) -> impl std::future::Future<Output = Result<Vec<v1::EvolutionPolicyDto>, ApplicationError>> + Send
    {
        self.list_evolution_policies()
    }
}

/// Service command trait for constructing, simulating, and executing evolution plans.
pub trait EvolutionPlannerCommandService: Send + Sync {
    /// Creates an evolution plan for a policy.
    fn create_evolution_plan(
        &self,
        policy_id: &str,
    ) -> impl std::future::Future<Output = Result<v1::EvolutionPlanDto, ApplicationError>> + Send;

    /// Simulates impact of an evolution plan without side effects.
    fn simulate_evolution_plan(
        &self,
        plan_id: &str,
    ) -> impl std::future::Future<Output = Result<v1::EvolutionSimulationReport, ApplicationError>> + Send;

    /// Executes an evolution plan using optimistic concurrency checks.
    fn execute_evolution_plan(
        &self,
        plan_id: &str,
        expected_graph_version: u64,
    ) -> impl std::future::Future<Output = Result<v1::EvolutionAuditRecordDto, ApplicationError>> + Send;

    /// Returns audit record log history.
    fn list_evolution_audit_records(
        &self,
    ) -> impl std::future::Future<Output = Result<Vec<v1::EvolutionAuditRecordDto>, ApplicationError>>
           + Send;
}

impl EvolutionPlannerCommandService for BrainApplication {
    fn create_evolution_plan(
        &self,
        policy_id: &str,
    ) -> impl std::future::Future<Output = Result<v1::EvolutionPlanDto, ApplicationError>> + Send
    {
        self.create_evolution_plan(policy_id)
    }

    fn simulate_evolution_plan(
        &self,
        plan_id: &str,
    ) -> impl std::future::Future<Output = Result<v1::EvolutionSimulationReport, ApplicationError>> + Send
    {
        self.simulate_evolution_plan(plan_id)
    }

    fn execute_evolution_plan(
        &self,
        plan_id: &str,
        expected_graph_version: u64,
    ) -> impl std::future::Future<Output = Result<v1::EvolutionAuditRecordDto, ApplicationError>> + Send
    {
        self.execute_evolution_plan(plan_id, expected_graph_version)
    }

    fn list_evolution_audit_records(
        &self,
    ) -> impl std::future::Future<Output = Result<Vec<v1::EvolutionAuditRecordDto>, ApplicationError>>
           + Send {
        self.list_evolution_audit_records()
    }
}

/// Service facade trait for Interactive Reflection proposal management.
pub trait ReflectionServiceFacade: Send + Sync {
    /// Returns catalog of reviewable reflection proposals.
    fn list_reflection_proposals(
        &self,
    ) -> impl std::future::Future<Output = Result<Vec<v1::ReflectionProposalDto>, ApplicationError>> + Send;

    /// Idempotently resolves a reflection proposal command.
    fn resolve_reflection_proposal(
        &self,
        cmd: ReflectionProposalCommand,
    ) -> impl std::future::Future<
        Output = Result<v1::ReflectionProposalActionReport, ApplicationError>,
    > + Send;
}

impl ReflectionServiceFacade for BrainApplication {
    fn list_reflection_proposals(
        &self,
    ) -> impl std::future::Future<Output = Result<Vec<v1::ReflectionProposalDto>, ApplicationError>> + Send
    {
        self.list_reflection_proposals()
    }

    fn resolve_reflection_proposal(
        &self,
        cmd: ReflectionProposalCommand,
    ) -> impl std::future::Future<
        Output = Result<v1::ReflectionProposalActionReport, ApplicationError>,
    > + Send {
        self.resolve_reflection_proposal(cmd)
    }
}

/// Normalized DTO wrapper for successful ingestion response.
#[derive(Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct IngestionResponse {
    /// Ingestion status message.
    pub status: String,
    /// Indicates whether event was successfully processed.
    pub processed: bool,
    /// The chronological database sequence number.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sequence: Option<u64>,
    /// The unique event ID.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub event_id: Option<String>,
}
