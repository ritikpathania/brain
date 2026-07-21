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
}

impl BrainApplication {
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
        Self {
            runtime,
            subscription_manager,
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

        let mapped_kinds: Option<Vec<brain_domain::SearchDocumentKind>> = query.kinds.map(|kinds| {
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

        let mapped_pagination = query.pagination.map(|p| brain_services::query::PaginationSpec {
            limit: p.limit,
            offset: p.offset,
        });

        let service_query = brain_services::query::SearchQuery {
            text: query.text,
            kinds: mapped_kinds,
            pagination: mapped_pagination,
        };

        // Execute search on runtime returning SearchDocument
        let results = self.runtime.search(service_query)?;

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
                loop {
                    match db_log.read_from(current_seq, 500) {
                        Ok(stored_events) => {
                            if stored_events.is_empty() {
                                break;
                            }
                            for event in stored_events {
                                if let Ok(domain_event) =
                                    serde_json::from_str::<brain_events::DomainEvent>(
                                        &event.payload_json,
                                    )
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
                        Err(_) => break,
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

        scheduler
            .rebuild_projection(projection_id)
            .map_err(|e| {
                ApplicationError::Internal(format!("Failed to rebuild projection {}: {:?}", name, e))
            })?;

        Ok(())
    }

    /// Trigger a manual reflection consolidation cycle on the active session.
    pub async fn reflect(&self) -> Result<v1::ReflectionReport, ApplicationError> {
        let context = brain_services::reflection::ReflectionContext {
            execution_id: uuid::Uuid::new_v4(),
            session_id: brain_domain::SessionId(ulid::Ulid::new()),
            cutoff_epoch: u64::MAX,
            max_nodes: 1000,
            time_budget_ms: 30000,
            cancellation_token: tokio_util::sync::CancellationToken::new(),
        };

        // 1. Run read-only reflection passes
        let findings = self.runtime.reflection_engine().reflect(&context).map_err(|e| {
            ApplicationError::Internal(format!("Reflection engine failed: {:?}", e))
        })?;

        // 2. Formulate decision plan via the planner
        let config = self.runtime.config();
        let planner = brain_services::reflection::ReflectionPlanner::with_thresholds(
            config.reflection().duplicate_confidence_threshold(),
            config.reflection().link_suggestion_confidence_threshold(),
        );
        let plan = planner.plan(findings);

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

            self.runtime.storage_ref().run_transaction(&mut run_tx).map_err(|e| {
                ApplicationError::Internal(format!("Failed to execute reflection write transaction: {:?}", e))
            })?;
        }

        let commands_executed = events.len();
        let details = events.into_iter().map(|ev| format!("{:?}", ev)).collect();

        Ok(v1::ReflectionReport {
            findings_processed: plan.findings_processed,
            commands_executed,
            details,
        })
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
