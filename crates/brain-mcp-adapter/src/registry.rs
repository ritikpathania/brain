use brain_adapter_core::Capability;
use brain_application::{ApplicationError, BrainApplication, ExecutionContext, IngestionResponse};
use brain_integrations::IngestionEnvelope;
use brain_application::dto::v1::SearchSummary;
use brain_services::query::SearchQuery;
use futures_util::future::BoxFuture;
use std::sync::Arc;

/// Type alias for the Brain Capability Registry.
pub type CapabilityRegistry =
    brain_adapter_core::CapabilityRegistry<BrainApplication, ExecutionContext, ApplicationError>;

/// Helper function to create and populate the Brain capability registry.
pub fn create_registry() -> CapabilityRegistry {
    let mut reg = CapabilityRegistry::new();
    reg.register(Arc::new(IngestCapability));
    reg.register(Arc::new(SearchCapability));
    reg
}

// ----------------------------------------------------------------------
// Concrete Capability Ingest
// ----------------------------------------------------------------------
/// Capability struct mapping to ingest operations.
pub struct IngestCapability;

impl Capability<BrainApplication, ExecutionContext, ApplicationError> for IngestCapability {
    type Request = IngestionEnvelope;
    type Response = IngestionResponse;

    fn name(&self) -> &'static str {
        "ingest"
    }

    fn description(&self) -> &'static str {
        "Ingest standard trace messages and agent state modifications into the relational memory engine."
    }

    fn input_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "event_model_version": { "type": "string" },
                "identity": { "type": "object" },
                "event": { "type": "object" }
            },
            "required": ["event_model_version", "identity", "event"]
        })
    }

    fn execute<'a>(
        &'a self,
        app: &'a BrainApplication,
        req: Self::Request,
        ctx: &'a ExecutionContext,
    ) -> BoxFuture<'a, Result<Self::Response, ApplicationError>> {
        Box::pin(async move { app.ingest(req, ctx).await })
    }
}

// ----------------------------------------------------------------------
// Concrete Capability Search
// ----------------------------------------------------------------------
/// Capability struct mapping to search operations.
pub struct SearchCapability;

impl Capability<BrainApplication, ExecutionContext, ApplicationError> for SearchCapability {
    type Request = SearchQuery;
    type Response = Vec<SearchSummary>;

    fn name(&self) -> &'static str {
        "search"
    }

    fn description(&self) -> &'static str {
        "Search the full-text memories database index using custom terms."
    }

    fn input_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "text": { "type": "string" },
                "kinds": { "type": "array", "items": { "type": "string" } },
                "pagination": { "type": "object" }
            },
            "required": ["text"]
        })
    }

    fn execute<'a>(
        &'a self,
        app: &'a BrainApplication,
        req: Self::Request,
        ctx: &'a ExecutionContext,
    ) -> BoxFuture<'a, Result<Self::Response, ApplicationError>> {
        Box::pin(async move { app.search(req, ctx).await })
    }
}
