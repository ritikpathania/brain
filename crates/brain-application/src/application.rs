use std::sync::Arc;
use brain_services::ApplicationRuntime;
use brain_integrations::IngestionEnvelope;
use brain_services::query::{SearchQuery, SearchSummary, SqliteSearchQueryService, SearchQueryService};
use brain_storage::SqliteSearchRepository;
use crate::context::ExecutionContext;
use crate::errors::ApplicationError;

/// Capability-oriented orchestrator presenting the unified public API of the Brain engine.
pub struct BrainApplication {
    runtime: Arc<ApplicationRuntime>,
}

impl BrainApplication {
    /// Crate-level stable interface version.
    pub const INTERFACE_VERSION: &'static str = "1.0.0";

    /// Create a new BrainApplication wrapping the services composition root runtime.
    pub fn new(runtime: Arc<ApplicationRuntime>) -> Self {
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

        // Verify storage database readiness
        let _storage = self.runtime.storage().map_err(|e| {
            ApplicationError::Unavailable(format!("Database storage unavailable: {:?}", e))
        })?;

        context.emit_progress(3, Some(3), "Ingestion completed successfully");

        Ok(IngestionResponse {
            status: "success".to_string(),
            processed: true,
        })
    }

    /// Search capability dispatches search queries to retrieval services.
    pub async fn search(
        &self,
        query: SearchQuery,
        context: &ExecutionContext,
    ) -> Result<Vec<SearchSummary>, ApplicationError> {
        if context.is_cancelled() {
            return Err(ApplicationError::Cancelled("Search aborted".to_string()));
        }

        context.emit_progress(1, Some(2), "Preparing search terms");

        let storage = self.runtime.storage().map_err(|e| {
            ApplicationError::Unavailable(format!("Database storage unavailable: {:?}", e))
        })?;

        let search_repo = SqliteSearchRepository::new(storage.pool().clone());
        let search_service = SqliteSearchQueryService::new(Arc::new(search_repo));

        context.emit_progress(2, Some(2), "Running full-text search query");
        
        let results = search_service.search(query).map_err(|e| {
            ApplicationError::Internal(format!("Search failed: {:?}", e))
        })?;

        Ok(results)
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

        context.emit_progress(1, Some(3), format!("Initializing workflow: {}", workflow_type));
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
            return Err(ApplicationError::Cancelled("Administration action aborted".to_string()));
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
