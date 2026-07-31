# External Adapters Hardening Implementation Plan (Corrected)

**Goal:** Establish `BrainApplication` as the canonical API boundary inside the daemon, introduce a transport-agnostic typed `RequestDispatcher`, and transition transport adapters (UDS, HTTP, CLI, Ratatui, SDK) to communicate strictly against stable DTOs.

**Architecture:**
```text
Clients (CLI, TUI, SDK, HTTP)
         │
         ▼ (Wire protocols / HTTP paths)
   ProtocolRouter (Decodes wire to typed request, warns on deprecated aliases)
         │
         ▼ (ApplicationRequest)
  RequestDispatcher (Routes requests to BrainApplication)
         │
         ▼ (Application API facade)
  BrainApplication
         │
         ▼ (Composition root)
   BrainRuntime (Orchestrates WAL log insertion, canonicalization, and reflection)
```

---

## Global Constraints

- **Typed Boundary**: The core dispatcher must only accept the typed `ApplicationRequest` enum and return the typed `ApplicationResponse` enum. Protocol decoding (from raw UDS lines or HTTP paths) must occur entirely within the transport layer's `ProtocolRouter`.
- **WAL Invariant**: WAL logging is a core guarantee of the ingestion pipeline. `BrainRuntime::ingest` owns WAL log insertion, making it identical for any present or future in-process caller.
- **Hiding Storage Primitives**: `BrainRuntime` and `BrainApplication` must not expose low-level storage methods like `insert_event` or `find_node_by_id`. They only expose high-level behavioral operations: `replay()` and `inspect_node()`.
- **Alias Deprecation**: Legacy action fallbacks in the UDS `ProtocolRouter` (e.g. `"status"`, `"query"`) are treated as deprecated. Upon matching, a tracing warning must be logged. These aliases are scheduled for deletion in the next protocol version.

---

## Proposed Changes

### Component 1: Ingestion & Telemetry Behaviors

#### [MODIFY] [brain-services/Cargo.toml](file:///Users/ritikpathania/Developer/PyCharm/brain/crates/brain-services/Cargo.toml)
- Add dependency: `brain-integrations = { path = "../brain-integrations" }`.

#### [MODIFY] [brain_runtime.rs](file:///Users/ritikpathania/Developer/PyCharm/brain/crates/brain-services/src/brain_runtime.rs)
- Add `sqlite_storage: Arc<SqliteStorage>` as a field on `BrainRuntime`. Save it in `BrainRuntime::new`.
- Update `ingest` signature:
  ```rust
  pub fn ingest(&self, envelope: brain_integrations::IngestionEnvelope) -> Result<IngestionResult, BrainError>
  ```
- Implement WAL logging and canonicalization sequencing inside `ingest`:
  1. Insert/deduplicate the event in the WAL: `self.sqlite_storage.insert_event(&envelope)?`.
  2. Map the envelope to the internal `Observation` entity.
  3. Dispatch ingestion to `self.canonicalizer.canonicalize(obs)`.
  4. Return `IngestionResult` containing the database sequence number and event ID.
- Implement runtime behavioral methods:
  - `pub fn replay(&self, after_sequence: u64) -> Result<Vec<IngestionEnvelope>, BrainError>`: queries the database WAL.
  - `pub fn inspect_node(&self, id_str: &str) -> Result<brain_domain::query::inspector::InspectorModel, BrainError>`: queries the node and connections to construct the inspector model.
- *Note: Ensure no low-level operations like `insert_event` or `find_by_id` are exposed publicly.*

#### [MODIFY] [application.rs](file:///Users/ritikpathania/Developer/PyCharm/brain/crates/brain-application/src/application.rs)
- Modify `IngestionResponse` to include `sequence` and `event_id` fields.
- Update `BrainApplication::ingest` to forward the envelope directly to `self.runtime.ingest()`.
- Implement delegator methods:
  - `pub async fn replay(&self, after_sequence: u64) -> Result<Vec<IngestionEnvelope>, ApplicationError>`
  - `pub async fn inspect_node(&self, id_str: &str) -> Result<InspectorModel, ApplicationError>`

---

### Component 2: Request Dispatcher

#### [NEW] [dispatcher.rs](file:///Users/ritikpathania/Developer/PyCharm/brain/crates/brain-application/src/dispatcher.rs)
Define the typed request, response, and request dispatcher inside `brain-application`:

```rust
use crate::context::ExecutionContext;
use crate::errors::ApplicationError;
use crate::application::{BrainApplication, IngestionResponse};
use brain_services::query::SearchQuery;
use brain_integrations::IngestionEnvelope;
use brain_domain::query::inspector::InspectorModel;
use crate::dto::v1;
use std::sync::Arc;

pub enum ApplicationRequest {
    Status,
    Metrics,
    Diagnostics,
    Capabilities,
    Search(SearchQuery),
    Ingest(IngestionEnvelope),
    Replay { after_sequence: u64 },
    InspectNode { id: String },
}

pub enum ApplicationResponse {
    Status(v1::Status),
    Metrics(v1::Metrics),
    Diagnostics(v1::Diagnostics),
    Capabilities(Vec<v1::Capability>),
    Search(Vec<v1::SearchSummary>),
    Ingest(IngestionResponse),
    Replay(Vec<IngestionEnvelope>),
    InspectNode(InspectorModel),
}

pub struct RequestDispatcher {
    app: Arc<BrainApplication>,
}

impl RequestDispatcher {
    pub fn new(app: Arc<BrainApplication>) -> Self {
        Self { app }
    }

    pub async fn dispatch(
        &self,
        req: ApplicationRequest,
        context: &ExecutionContext,
    ) -> Result<ApplicationResponse, ApplicationError> {
        match req {
            ApplicationRequest::Status => {
                Ok(ApplicationResponse::Status(self.app.status()))
            }
            ApplicationRequest::Metrics => {
                Ok(ApplicationResponse::Metrics(self.app.metrics()))
            }
            ApplicationRequest::Diagnostics => {
                Ok(ApplicationResponse::Diagnostics(self.app.diagnostics()))
            }
            ApplicationRequest::Capabilities => {
                Ok(ApplicationResponse::Capabilities(self.app.discover_capabilities()))
            }
            ApplicationRequest::Search(query) => {
                let results = self.app.search(query, context).await?;
                Ok(ApplicationResponse::Search(results))
            }
            ApplicationRequest::Ingest(envelope) => {
                let res = self.app.ingest(envelope, context).await?;
                Ok(ApplicationResponse::Ingest(res))
            }
            ApplicationRequest::Replay { after_sequence } => {
                let events = self.app.replay(after_sequence).await?;
                Ok(ApplicationResponse::Replay(events))
            }
            ApplicationRequest::InspectNode { id } => {
                let model = self.app.inspect_node(&id).await?;
                Ok(ApplicationResponse::InspectNode(model))
            }
        }
    }
}
```

---

### Component 3: Daemon Transport Refactoring

#### [MODIFY] [daemon/Cargo.toml](file:///Users/ritikpathania/Developer/PyCharm/brain/daemon/Cargo.toml)
- Add dependency: `brain-application = { path = "../crates/brain-application" }`.

#### [NEW] [transport/mod.rs](file:///Users/ritikpathania/Developer/PyCharm/brain/daemon/src/transport/mod.rs)
- Expose `uds` and `http` submodules.

#### [NEW] [transport/uds/router.rs](file:///Users/ritikpathania/Developer/PyCharm/brain/daemon/src/transport/uds/router.rs)
- Implement a UDS protocol decoder mapping UDS actions and parsing payloads into the typed `ApplicationRequest` enum.
- Log a tracing/stderr warning whenever legacy actions (e.g. `"status"`, `"query"`, `"ingest_event"`, `"inspect_node"`, `"replay"`) are mapped:
  ```rust
  tracing::warn!(action = %action, "UDS Protocol Router: Received deprecated legacy action name. Please upgrade client to use versioned actions.");
  ```

#### [NEW] [transport/uds/handlers.rs](file:///Users/ritikpathania/Developer/PyCharm/brain/daemon/src/transport/uds/handlers.rs)
- Refactor connection stream reading to invoke the `ProtocolRouter` and pass decoded typed requests to the `RequestDispatcher`.
- Stream dynamic application updates for `v1/subscribe` subscriptions.

#### [NEW] [transport/http/handlers.rs](file:///Users/ritikpathania/Developer/PyCharm/brain/daemon/src/transport/http/handlers.rs)
- Parse incoming health requests to `ApplicationRequest::Status` or `/metrics` to `ApplicationRequest::Metrics`.
- Translate status response:
  - If status health is `"healthy"`, return `200 OK` containing JSON.
  - If status health is anything else, return `503 Service Unavailable`.

---

### Component 4: External Client Parity

#### [MODIFY] [brain-sdk-rs/Cargo.toml](file:///Users/ritikpathania/Developer/PyCharm/brain/crates/brain-sdk-rs/Cargo.toml)
- Add dependency: `brain-application = { path = "../brain-application" }`.

#### [MODIFY] [client.rs](file:///Users/ritikpathania/Developer/PyCharm/brain/crates/brain-sdk-rs/src/client.rs)
- Mirror `BrainApplication` one-to-one with matching type-safe client methods:
  - `pub async fn status(&self) -> Result<v1::Status, BrainSdkError>`
  - `pub async fn metrics(&self) -> Result<v1::Metrics, BrainSdkError>`
  - `pub async fn diagnostics(&self) -> Result<v1::Diagnostics, BrainSdkError>`
  - `pub async fn capabilities(&self) -> Result<Vec<v1::Capability>, BrainSdkError>`
  - `pub async fn search(&self, query: SearchQuery) -> Result<Vec<v1::SearchSummary>, BrainSdkError>`
  - `pub async fn subscribe(&self) -> Result<mpsc::Receiver<v1::Event>, BrainSdkError>`

#### [MODIFY] [main.rs](file:///Users/ritikpathania/Developer/PyCharm/brain/crates/brain-cli-adapter/src/main.rs)
- Implement subcommands: `status`, `metrics`, `diagnostics`, `capabilities`, `search --text "..."`.
- Retrieve values via the SDK client and serialize them directly as DTO JSON to stdout.

#### [MODIFY] [client.rs](file:///Users/ritikpathania/Developer/PyCharm/brain/crates/brain-tui/src/client.rs)
- Update UDS requests to output versioned action strings (e.g. `v1/search` and `v1/inspect_node`).

---

## Verification Plan

### Automated Tests
- Run unit/integration suites verifying typed dispatching and protocol router fallbacks:
  - `cargo test -p brain-application`
  - `cargo test -p brain-sdk-rs`
  - `cargo test --workspace`

### Manual Verification
- Compile and start the daemon: `cargo run -p brain-daemon`
- Verify that CLI outputs match expectations:
  - `cargo run -p brain-cli-adapter status` -> Prints Status DTO.
  - `cargo run -p brain-cli-adapter metrics` -> Prints Metrics DTO.
- Access health endpoints:
  - `curl -i http://localhost:8080/status` -> 200 OK or 503 Service Unavailable.
