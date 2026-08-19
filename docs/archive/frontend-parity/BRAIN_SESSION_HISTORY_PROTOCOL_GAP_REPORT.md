# Forensic Report — Brain Session History Protocol Gap Analysis

> **Document Status**: Forensic Source Trace & Protocol Gap Audit  
> **Authoritative Sources Inspected**:
> - `crates/brain-tui/src/client.rs` (lines 520–605)
> - `daemon/src/transport/uds/handlers.rs` (lines 1–573)
> - `daemon/src/transport/uds/router.rs` (lines 1–269)
> - `crates/brain-services/src/session.rs` (lines 50–165)
> - `crates/brain-storage/src/store.rs` (lines 2100–2190)
> **Author**: Antigravity AI  
> **Date**: 2026-08-14  

---

```text
==================================================
SESSION HISTORY PROTOCOL GAP AUDIT
==================================================
UDS SESSION ENUMERATION: AVAILABLE (action: "list_sessions")
UDS SESSION HISTORY RETRIEVAL: NOT EXPOSED (crates/brain-tui line 601)
INTERNAL SERVICE / STORAGE API: AVAILABLE (SessionService::load_session & SQLite store)
ACTION TAKEN: STOPPED CODING — Awaiting user guidance / approval for protocol extension
```

---

## 1. Trace Answers to Required Questions

### 1. Is there already an existing UDS action/API that retrieves session history?
**No.**  
Inspecting `daemon/src/transport/uds/router.rs` and `daemon/src/transport/uds/handlers.rs`, the UDS router supports:
- `v1/status`, `v1/metrics`, `v1/diagnostics`, `v1/capabilities`
- `v1/search`, `v1/ingest`, `v1/replay`, `v1/inspect_node`, `v1/subscribe`
- `v1/projections`, `v1/rebuild_projection`
- `query`, `list_sessions`, `handshake`, `disconnect`

There is **no UDS action/handler** that accepts a session ID and returns its historical conversation messages.

### 2. Is there an existing client method for loading session messages?
In `crates/brain-tui/src/client.rs` (lines 599–603), the `BrainClient` trait defines `load_session(&self, id: SessionId) -> Result<Vec<Message>, BrainError>`, but its UDS implementation explicitly returns:
```rust
async fn load_session(&self, _id: SessionId) -> Result<Vec<Message>, BrainError> {
    Err(BrainError::Internal {
        message: "Unsupported: historical message loading in standalone daemon".to_string(),
    })
}
```

### 3. Is session history already available through an existing runtime/session API?
**Yes, in the internal domain and storage services**:
- `brain-services/src/session.rs`: `SessionService::load_session(&self, id: &SessionId) -> Result<Session, BrainError>`
- `brain-storage/src/store.rs`: `load_session_conn(&self, id: &SessionId)` which retrieves messages from SQLite `sessions` and `messages` tables.

The backend domain and storage engine already support full multi-turn conversation retrieval, but the daemon's UDS router has not exposed it to external UDS clients.

### 4. What exact wire request and response represent historical messages?
Currently, none exist in the UDS daemon. The proposed minimal wire framing is:
- **Request**:
  ```json
  {"version": "1.0", "type": "Request", "id": 1, "action": "v1/sessions/get", "body": "{\"session_id\": \"sess_abc123\"}"}
  ```
- **Response**:
  ```json
  {
    "version": "1.0",
    "type": "Response",
    "id": 1,
    "status": "success",
    "body": "[{\"id\":\"msg_1\",\"role\":\"user\",\"content\":\"Explain ADR-001\",\"timestamp\":\"2026-08-14T00:00:00Z\"},{\"id\":\"msg_2\",\"role\":\"assistant\",\"content\":\"ADR-001 defines...\";\"timestamp\":\"2026-08-14T00:00:01Z\"}]"
  }
  ```

### 5. Can the frontend obtain the complete timeline without modifying Rust?
**No.** Without a UDS wire endpoint in the daemon, external UDS clients (including `packages/brain-frontend`) can enumerate sessions via `list_sessions`, but cannot load historical conversation messages across daemon restarts without modifying the UDS daemon.

### 6. Identified Protocol Gap
- **Location**: `daemon/src/transport/uds/router.rs` and `daemon/src/transport/uds/handlers.rs`.
- **Gap**: Missing route for `v1/sessions/get` (or legacy `load_session`) delegating to `app.load_session(session_id)`.

---

## 2. Proposed Minimal Protocol Extension

If approved, the smallest possible extension would be:
1. In `daemon/src/transport/uds/router.rs`: Add action `"v1/sessions/get"` mapping to `ApplicationRequest::GetSession { session_id }`.
2. In `brain-application/src/dispatcher.rs`: Dispatch to `session_service.load_session(&session_id)` and return `ApplicationResponse::Session(messages)`.
3. In `daemon/src/transport/uds/handlers.rs`: Serialize session messages to UDS response JSON.

---

## 3. Strict Compliance Notice

In accordance with your hard directive:
> *"If history is NOT exposed by the existing protocol, identify the exact protocol gap and STOP before implementation. Do not invent a new wire format or fabricate PresentationState history."*

**We have stopped and made zero code modifications.** Awaiting your decision.
