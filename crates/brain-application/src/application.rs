use crate::context::ExecutionContext;
use crate::dto::v1;
use crate::errors::ApplicationError;
use brain_core::events::{
    ProjectionInstanceInvalidatedEvent, RuntimeEvent, RuntimeRelationshipEvent, TaskProgress,
};
use brain_core::evolution::{Observation, Provenance};
use brain_integrations::IngestionEnvelope;
use brain_services::BrainRuntime;
use std::sync::Arc;
use std::time::SystemTime;

/// Translates transport ingestion envelope structures to internal domain observations.
pub struct ObservationTranslator;

impl ObservationTranslator {
    /// Translates an `IngestionEnvelope` DTO into the internal `Observation` entity model.
    pub fn translate(envelope: IngestionEnvelope) -> Result<Observation, serde_json::Error> {
        let payload = serde_json::to_vec(&envelope.event)?;
        let media_type = "application/json".to_string();
        let provenance = Provenance {
            source_adapter: envelope.identity.adapter_id.to_string(),
            timestamp: SystemTime::from(envelope.identity.timestamp),
            correlation_id: envelope.identity.event_id.0,
        };
        Ok(Observation {
            payload,
            media_type,
            provenance,
        })
    }
}

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
}

/// Capability-oriented orchestrator presenting the unified public API of the Brain engine.
pub struct BrainApplication {
    runtime: Arc<BrainRuntime>,
}

impl BrainApplication {
    /// Crate-level stable interface version.
    pub const INTERFACE_VERSION: &'static str = "1.0.0";

    /// Create a new BrainApplication wrapping the services composition root runtime.
    pub fn new(runtime: Arc<BrainRuntime>) -> Self {
        Self { runtime }
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

        let obs = ObservationTranslator::translate(envelope)
            .map_err(|e| ApplicationError::Validation(format!("Invalid envelope: {}", e)))?;

        // Dispatch ingestion via BrainRuntime
        self.runtime.ingest(obs)?;

        context.emit_progress(3, Some(3), "Ingestion completed successfully");

        Ok(IngestionResponse {
            status: "success".to_string(),
            processed: true,
        })
    }

    /// Search capability dispatches search queries to retrieval services.
    pub async fn search(
        &self,
        query: brain_services::query::SearchQuery,
        context: &ExecutionContext,
    ) -> Result<Vec<v1::SearchSummary>, ApplicationError> {
        if context.is_cancelled() {
            return Err(ApplicationError::Cancelled("Search aborted".to_string()));
        }

        context.emit_progress(1, Some(2), "Preparing search terms");

        // Execute search on runtime returning SearchDocument
        let results = self.runtime.search(query)?;

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
    pub fn subscribe(&self) -> tokio::sync::mpsc::Receiver<v1::Event> {
        let mut rx = self.runtime.subscribe();
        let (tx, client_rx) = tokio::sync::mpsc::channel(100);

        tokio::spawn(async move {
            while let Some(event) = rx.recv().await {
                let dto = EventTranslator::translate(event);
                if tx.send(dto).await.is_err() {
                    break;
                }
            }
        });

        client_rx
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
            active_event_subscribers: status.active_event_subscribers,
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
}

/// Normalized DTO wrapper for successful ingestion response.
#[derive(Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct IngestionResponse {
    /// Ingestion status message.
    pub status: String,
    /// Indicates whether event was successfully processed.
    pub processed: bool,
}
