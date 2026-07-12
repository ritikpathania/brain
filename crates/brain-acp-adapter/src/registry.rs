use std::sync::Arc;
use futures_util::future::BoxFuture;
use brain_application::{BrainApplication, ExecutionContext, ApplicationError, IngestionResponse};
use brain_integrations::IngestionEnvelope;
use brain_services::query::{SearchQuery, SearchSummary};
use brain_adapter_core::Capability;

/// Type alias for the Brain Capability Registry.
pub type CapabilityRegistry = brain_adapter_core::CapabilityRegistry<BrainApplication, ExecutionContext, ApplicationError>;
/// Type alias for ACP capability registry compatibility.
pub type CapabilityExposureRegistry = CapabilityRegistry;

/// Helper function to create and populate the Brain capability registry.
pub fn create_registry() -> CapabilityRegistry {
    let mut reg = CapabilityRegistry::new();
    reg.register(Arc::new(IngestCapability));
    reg.register(Arc::new(SearchCapability));
    reg.register(Arc::new(CancelCapability));
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
        "Ingest standard trace messages and agent state modifications."
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
        "Search the full-text memories database index."
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

// ----------------------------------------------------------------------
// Concrete Capability Cancel (Testing Abort Propagation)
// ----------------------------------------------------------------------
/// Capability struct mapping to cancel testing operations.
pub struct CancelCapability;

impl Capability<BrainApplication, ExecutionContext, ApplicationError> for CancelCapability {
    type Request = serde_json::Value;
    type Response = serde_json::Value;

    fn name(&self) -> &'static str {
        "cancel_test"
    }

    fn description(&self) -> &'static str {
        "A capability designed to test token cancellation propagation."
    }

    fn input_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {}
        })
    }

    fn execute<'a>(
        &'a self,
        _app: &'a BrainApplication,
        _req: Self::Request,
        ctx: &'a ExecutionContext,
    ) -> BoxFuture<'a, Result<Self::Response, ApplicationError>> {
        Box::pin(async move {
            for _ in 0..100 {
                if ctx.is_cancelled() {
                    return Err(ApplicationError::Cancelled("Mock cancelled".to_string()));
                }
                tokio::time::sleep(std::time::Duration::from_millis(5)).await;
            }
            Ok(serde_json::json!({ "status": "completed" }))
        })
    }
}
