use crate::application::{BrainApplication, IngestionResponse};
use crate::context::ExecutionContext;
use crate::dto::v1;
use crate::errors::ApplicationError;
use brain_domain::query::inspector::InspectorModel;
use brain_integrations::IngestionEnvelope;
use std::sync::Arc;
/// Represents a strongly-typed request dispatched to the BrainApplication api surface.
pub enum ApplicationRequest {
    /// Retrieve runtime status
    Status,
    /// Retrieve operational metrics
    Metrics,
    /// Retrieve telemetry diagnostics
    Diagnostics,
    /// List active and supported engine capabilities
    Capabilities,
    /// Perform a graph node query search
    Search(v1::SearchQuery),
    /// Ingest a source adapter event envelope
    Ingest(Box<IngestionEnvelope>),
    /// Replay historical ingestion events after a sequence number
    Replay {
        /// Sequence number to search after
        after_sequence: u64,
    },
    /// Retrieve full relation details for a node
    InspectNode {
        /// UUID string format of the node
        id: String,
    },
    /// Subscribe to the runtime event stream with optional resume sequence
    Subscribe {
        /// Starting sequence number
        after_sequence: Option<u64>,
    },
    /// List active projection engine states
    ListProjectionStatus,
    /// Trigger manual rebuild of a specific projection
    RebuildProjection {
        /// Name of the projection
        name: String,
    },
    /// Trigger manual reflection engine cycle
    Reflect,
    /// Retrieve reflection background scheduler status
    ReflectStatus,
    /// Retrieve most recent reflection report
    ReflectReport,
    /// Retrieve lightweight reflection summary
    ReflectSummary,
    /// Retrieve active unmerged reflection findings
    ReflectFindings,
}

/// Represents the corresponding strongly-typed response returned by the RequestDispatcher.
pub enum ApplicationResponse {
    /// Uptime and health metadata
    Status(v1::Status),
    /// Diagnostic operations counters
    Metrics(v1::Metrics),
    /// Execution failures log
    Diagnostics(v1::Diagnostics),
    /// List of registration capabilities
    Capabilities(Vec<v1::Capability>),
    /// Matched nodes summary
    Search(Vec<v1::SearchSummary>),
    /// Event WAL write and reflection confirmation
    Ingest(IngestionResponse),
    /// Replayed WAL envelopes
    Replay(Vec<IngestionEnvelope>),
    /// Inspected node relations structure
    InspectNode(Box<InspectorModel>),
    /// Subscription event stream
    Subscribe(crate::subscription::EventStream),
    /// Status list of projections
    ListProjectionStatus(Vec<v1::ProjectionStatus>),
    /// Trigger confirmation
    RebuildProjection,
    /// Manual reflection cycle report
    Reflect(v1::ReflectionReport),
    /// Reflection scheduler status report
    ReflectStatus(v1::ReflectionStatusReport),
    /// Most recent reflection execution report
    ReflectReport(Option<v1::ReflectionReport>),
    /// Reflection status summary
    ReflectSummary(v1::ReflectionSummaryDto),
    /// Active reflection findings
    ReflectFindings(Vec<v1::ReflectionFindingDto>),
}

/// Transport-agnostic request router routing typed requests directly to the application layer.
pub struct RequestDispatcher {
    app: Arc<BrainApplication>,
}

impl RequestDispatcher {
    /// Creates a new RequestDispatcher wrapper around the BrainApplication.
    pub fn new(app: Arc<BrainApplication>) -> Self {
        Self { app }
    }

    /// Dispatches a typed request and returns a matching typed response.
    pub async fn dispatch(
        &self,
        req: ApplicationRequest,
        context: &ExecutionContext,
    ) -> Result<ApplicationResponse, ApplicationError> {
        match req {
            ApplicationRequest::Status => Ok(ApplicationResponse::Status(self.app.status())),
            ApplicationRequest::Metrics => Ok(ApplicationResponse::Metrics(self.app.metrics())),
            ApplicationRequest::Diagnostics => {
                Ok(ApplicationResponse::Diagnostics(self.app.diagnostics()))
            }
            ApplicationRequest::Capabilities => Ok(ApplicationResponse::Capabilities(
                self.app.discover_capabilities(),
            )),
            ApplicationRequest::Search(query) => {
                let results = self.app.search(query, context).await?;
                Ok(ApplicationResponse::Search(results))
            }
            ApplicationRequest::Ingest(envelope) => {
                let res = self.app.ingest(*envelope, context).await?;
                Ok(ApplicationResponse::Ingest(res))
            }
            ApplicationRequest::Replay { after_sequence } => {
                let events = self.app.replay(after_sequence).await?;
                Ok(ApplicationResponse::Replay(events))
            }
            ApplicationRequest::InspectNode { id } => {
                let model = self.app.inspect_node(&id).await?;
                Ok(ApplicationResponse::InspectNode(Box::new(model)))
            }
            ApplicationRequest::Subscribe { after_sequence } => {
                let stream = self.app.subscribe(after_sequence);
                Ok(ApplicationResponse::Subscribe(stream))
            }
            ApplicationRequest::ListProjectionStatus => {
                let list = self.app.list_projections().await?;
                Ok(ApplicationResponse::ListProjectionStatus(list))
            }
            ApplicationRequest::RebuildProjection { name } => {
                self.app.rebuild_projection(&name).await?;
                Ok(ApplicationResponse::RebuildProjection)
            }
            ApplicationRequest::Reflect => {
                let report = self.app.reflect().await?;
                Ok(ApplicationResponse::Reflect(report))
            }
            ApplicationRequest::ReflectStatus => {
                let status = self.app.reflect_status().await?;
                Ok(ApplicationResponse::ReflectStatus(status))
            }
            ApplicationRequest::ReflectReport => {
                let report = self.app.last_reflection_report().await?;
                Ok(ApplicationResponse::ReflectReport(report))
            }
            ApplicationRequest::ReflectSummary => {
                let summary = self.app.reflect_summary().await?;
                Ok(ApplicationResponse::ReflectSummary(summary))
            }
            ApplicationRequest::ReflectFindings => {
                let findings = self.app.active_reflection_findings().await?;
                Ok(ApplicationResponse::ReflectFindings(findings))
            }
        }
    }
}
