use futures::StreamExt;
use std::collections::HashMap;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Instant;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::unix::OwnedWriteHalf;
use tokio::net::UnixStream;
use tokio_util::sync::CancellationToken;

use brain_application::context::ExecutionContext;
use brain_application::dispatcher::{ApplicationRequest, ApplicationResponse, RequestDispatcher};
use brain_core::extensibility::CancellationToken as CoreCancellationToken;

use crate::server::protocol::{ClientRequest, LegacyResponse, ServerResponse, VersionedEvent};
use crate::transport::uds::frontend_handshake;
use crate::transport::uds::router::ProtocolRouter;
use crate::transport::uds::session_id::{parse_canonical_session_id, CanonicalSessionIdError};
use crate::{DaemonMetrics, REQUEST_COUNTER};

/// Active generation state tracked in the daemon runtime.
pub struct ActiveGeneration {
    /// Session ID associated with the active generation.
    pub session_id: brain_domain::SessionId,
    /// Cooperative cancellation token.
    pub cancellation_token: CancellationToken,
    /// Arbitrates a terminal tool result against cancellation acknowledgement.
    /// The side which acquires this gate first defines the externally visible
    /// outcome for a just-completed tool invocation.
    pub tool_result_gate: Arc<tokio::sync::Mutex<()>>,
}

/// Adapts a generation's owned Tokio cancellation token to the core tool
/// contract. The adapter owns the token handle, so a running tool never
/// observes a borrowed connection or request lifetime.
#[derive(Clone)]
struct GenerationToolCancellation(CancellationToken);

impl CoreCancellationToken for GenerationToolCancellation {
    fn is_cancelled(&self) -> bool {
        self.0.is_cancelled()
    }
}

static GENERATION_REGISTRY: std::sync::OnceLock<
    Arc<tokio::sync::RwLock<HashMap<String, ActiveGeneration>>>,
> = std::sync::OnceLock::new();

static FEEDBACK_REGISTRY: std::sync::OnceLock<Arc<tokio::sync::RwLock<HashMap<String, String>>>> =
    std::sync::OnceLock::new();

struct PendingToolPermission {
    call_id: String,
    session_id: String,
    generation_id: String,
    tool_name: String,
    decision: tokio::sync::oneshot::Sender<bool>,
}

static PERMISSION_WAITERS: std::sync::OnceLock<
    Arc<tokio::sync::RwLock<HashMap<String, PendingToolPermission>>>,
> = std::sync::OnceLock::new();

/// Pending tool-permission decisions keyed by their daemon-issued request ID.
/// A stream can have more than one tool invocation, so both the request ID and
/// the tool-call ID are required for canonical clients to settle a request.
/// Resolution intentionally rides a second connection because the stream owns
/// its connection's read loop.
fn get_permission_waiters(
) -> &'static Arc<tokio::sync::RwLock<HashMap<String, PendingToolPermission>>> {
    PERMISSION_WAITERS.get_or_init(|| Arc::new(tokio::sync::RwLock::new(HashMap::new())))
}

static CONSOLIDATION_LOCK: std::sync::OnceLock<Arc<tokio::sync::Mutex<()>>> =
    std::sync::OnceLock::new();

static LAST_DEBOUNCE_SWEEP: std::sync::OnceLock<std::sync::atomic::AtomicU64> =
    std::sync::OnceLock::new();

/// Maximum allowed frame size on UDS transport (4 MB).
pub const MAX_UDS_FRAME_BYTES: usize = 4 * 1024 * 1024;

fn get_generation_registry() -> &'static Arc<tokio::sync::RwLock<HashMap<String, ActiveGeneration>>>
{
    GENERATION_REGISTRY.get_or_init(|| Arc::new(tokio::sync::RwLock::new(HashMap::new())))
}

fn get_feedback_registry() -> &'static Arc<tokio::sync::RwLock<HashMap<String, String>>> {
    FEEDBACK_REGISTRY.get_or_init(|| Arc::new(tokio::sync::RwLock::new(HashMap::new())))
}

fn get_consolidation_lock() -> &'static Arc<tokio::sync::Mutex<()>> {
    CONSOLIDATION_LOCK.get_or_init(|| Arc::new(tokio::sync::Mutex::new(())))
}

fn get_last_debounce_sweep() -> &'static std::sync::atomic::AtomicU64 {
    LAST_DEBOUNCE_SWEEP.get_or_init(|| std::sync::atomic::AtomicU64::new(0))
}

static SUBAGENT_SERVICE: std::sync::OnceLock<Arc<brain_services::subagent::SubagentService>> =
    std::sync::OnceLock::new();

/// Returns the daemon-wide SubagentService instance.
pub fn get_subagent_service() -> &'static Arc<brain_services::subagent::SubagentService> {
    SUBAGENT_SERVICE.get_or_init(|| {
        let repo_root = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
        let wt_service = brain_services::worktree::WorktreeService::new(repo_root)
            .ok()
            .map(Arc::new);
        Arc::new(brain_services::subagent::SubagentService::new(
            brain_services::subagent::DEFAULT_MAX_CONCURRENT_SUBAGENTS,
            wt_service,
        ))
    })
}

pub(crate) fn sanitize_security_string(input: &str) -> String {
    input
        .chars()
        .filter(|c| !c.is_control() || *c == '\t' || *c == '\n' || *c == '\r')
        .collect::<String>()
        .trim()
        .to_string()
}

pub(crate) fn sanitize_path_containment(path: &str) -> String {
    let clean = sanitize_security_string(path);
    clean.replace("../", "").replace("..\\", "")
}

async fn write_session_id_error(
    writer: &Arc<tokio::sync::Mutex<OwnedWriteHalf>>,
    is_versioned: bool,
    request_id: &serde_json::Value,
    error: CanonicalSessionIdError,
) -> Result<(), Box<dyn std::error::Error>> {
    let response = if is_versioned {
        serde_json::json!({
            "version": "1.0",
            "type": "Error",
            "id": request_id,
            "status": "error",
            "code": error.code(),
            "body": error.message(),
        })
    } else {
        serde_json::json!({
            "type": "error",
            "status": "error",
            "code": error.code(),
            "message": error.message(),
        })
    };
    let mut response_json = serde_json::to_string(&response)?;
    response_json.push('\n');
    let mut output = writer.lock().await;
    output.write_all(response_json.as_bytes()).await?;
    output.flush().await?;
    Ok(())
}

struct GenerationGuard {
    generation_id: String,
    registry: &'static Arc<tokio::sync::RwLock<HashMap<String, ActiveGeneration>>>,
    active: bool,
}

impl GenerationGuard {
    fn new(
        generation_id: String,
        registry: &'static Arc<tokio::sync::RwLock<HashMap<String, ActiveGeneration>>>,
    ) -> Self {
        Self {
            generation_id,
            registry,
            active: true,
        }
    }

    async fn defuse(mut self) {
        self.active = false;
        let mut reg = self.registry.write().await;
        reg.remove(&self.generation_id);
    }
}

impl Drop for GenerationGuard {
    fn drop(&mut self) {
        if self.active {
            let generation_id = self.generation_id.clone();
            let registry = self.registry;
            tokio::spawn(async move {
                let mut reg = registry.write().await;
                reg.remove(&generation_id);
            });
        }
    }
}

/// Handles an active UDS client connection, decoding, routing, and executing requests.
pub async fn handle_connection(
    stream: UnixStream,
    metrics: Arc<DaemonMetrics>,
    dispatcher: Arc<RequestDispatcher>,
    app: Arc<brain_application::BrainApplication>,
) -> Result<(), Box<dyn std::error::Error>> {
    let conn_token = CancellationToken::new();
    let (reader, writer) = stream.into_split();
    let writer = Arc::new(tokio::sync::Mutex::new(writer));
    let mut buf_reader = BufReader::new(reader);
    let mut line = String::new();
    let mut conn_registered_clients: Vec<String> = Vec::new();

    macro_rules! require_canonical_session_id {
        ($raw:expr, $is_versioned:expr, $request_id:expr) => {{
            match parse_canonical_session_id($raw) {
                Ok(session_id) => session_id,
                Err(error) => {
                    write_session_id_error(&writer, $is_versioned, &$request_id, error).await?;
                    continue;
                }
            }
        }};
    }

    while buf_reader.read_line(&mut line).await? > 0 {
        let request_str = line.trim();
        if request_str.is_empty() {
            line.clear();
            continue;
        }

        if request_str.len() > MAX_UDS_FRAME_BYTES {
            let response = ServerResponse::Legacy(LegacyResponse {
                status: "error".to_string(),
                message: format!(
                    "Payload exceeds maximum allowed frame size of {} bytes",
                    MAX_UDS_FRAME_BYTES
                ),
            });
            let mut response_json = serde_json::to_string(&response)?;
            response_json.push('\n');
            let mut w = writer.lock().await;
            w.write_all(response_json.as_bytes()).await?;
            w.flush().await?;
            line.clear();
            continue;
        }

        let ipc_start = Instant::now();
        let _correlation_id = REQUEST_COUNTER.fetch_add(1, Ordering::Relaxed);

        let request: ClientRequest = match serde_json::from_str(request_str) {
            Ok(req) => req,
            Err(e) => {
                let response = ServerResponse::Legacy(LegacyResponse {
                    status: "error".to_string(),
                    message: format!("Failed to parse request JSON: {}", e),
                });
                let mut response_json = serde_json::to_string(&response)?;
                response_json.push('\n');
                let mut w = writer.lock().await;
                w.write_all(response_json.as_bytes()).await?;
                w.flush().await?;
                line.clear();
                continue;
            }
        };

        line.clear();

        let fallback_empty_ctx = Vec::new();
        let (action_str, payload_string, req_id_val, is_versioned, workspace_context) =
            match &request {
                ClientRequest::Versioned(req) => (
                    req.action.as_str(),
                    req.body.clone(),
                    serde_json::json!(req.id),
                    true,
                    &req.workspace_context,
                ),
                ClientRequest::JsonRpc(req) => {
                    let payload_str = if let Some(ref b) = req.body {
                        b.clone()
                    } else if req.payload.is_string() {
                        req.payload.as_str().unwrap().to_string()
                    } else {
                        serde_json::to_string(&req.payload).unwrap_or_default()
                    };
                    (
                        req.action.as_str(),
                        payload_str,
                        req.id.clone(),
                        true,
                        &fallback_empty_ctx,
                    )
                }
                ClientRequest::Legacy(req) => (
                    req.action.as_str(),
                    req.payload.clone(),
                    serde_json::json!(0),
                    false,
                    &fallback_empty_ctx,
                ),
            };
        let action = action_str;
        let payload = payload_string.as_str();

        // Special UDS-specific commands handled at transport layer
        if action == "disconnect" {
            let response = if is_versioned {
                serde_json::json!({
                    "version": "1.0",
                    "type": "Response",
                    "id": req_id_val,
                    "status": "success",
                    "body": "disconnected ok"
                })
            } else {
                serde_json::json!({
                    "status": "ok",
                    "message": "disconnected ok"
                })
            };
            let mut response_json = serde_json::to_string(&response)?;
            response_json.push('\n');
            let mut w = writer.lock().await;
            w.write_all(response_json.as_bytes()).await?;
            w.flush().await?;
            return Ok(());
        }

        if action == "handshake" {
            let response = if is_versioned {
                serde_json::json!({
                    "version": "1.0",
                    "type": "Response",
                    "id": req_id_val,
                    "status": "success",
                    "body": "handshake ok"
                })
            } else {
                serde_json::json!({
                    "status": "ok",
                    "message": "handshake ok"
                })
            };
            let mut response_json = serde_json::to_string(&response)?;
            response_json.push('\n');
            let mut w = writer.lock().await;
            w.write_all(response_json.as_bytes()).await?;
            w.flush().await?;
            continue;
        }

        if action == "v1/frontend/handshake" {
            frontend_handshake::write_response(
                &writer,
                &req_id_val,
                payload,
                env!("CARGO_PKG_VERSION"),
            )
            .await?;
            continue;
        }

        if action == "fleet/register" || action == "v1/fleet/register" {
            let reg_req: brain_domain::ForegroundSession = match serde_json::from_str(payload) {
                Ok(req) => req,
                Err(e) => {
                    let response = if is_versioned {
                        serde_json::json!({
                            "version": "1.0",
                            "type": "Response",
                            "id": req_id_val,
                            "status": "error",
                            "body": format!("Invalid registration payload: {}", e)
                        })
                    } else {
                        serde_json::json!({
                            "status": "error",
                            "message": format!("Invalid registration payload: {}", e)
                        })
                    };
                    let mut response_json = serde_json::to_string(&response)?;
                    response_json.push('\n');
                    let mut w = writer.lock().await;
                    w.write_all(response_json.as_bytes()).await?;
                    w.flush().await?;
                    continue;
                }
            };

            let client_id = reg_req.client_id.clone();
            if !conn_registered_clients.contains(&client_id) {
                conn_registered_clients.push(client_id.clone());
            }

            let fleet = app.runtime().fleet_registry();
            fleet.register_session(reg_req);

            let response = if is_versioned {
                serde_json::json!({
                    "version": "1.0",
                    "type": "Response",
                    "id": req_id_val,
                    "status": "success",
                    "body": serde_json::json!({ "registered": true, "client_id": client_id })
                })
            } else {
                serde_json::json!({
                    "status": "ok",
                    "registered": true,
                    "client_id": client_id
                })
            };

            let mut response_json = serde_json::to_string(&response)?;
            response_json.push('\n');
            let mut w = writer.lock().await;
            w.write_all(response_json.as_bytes()).await?;
            w.flush().await?;
            continue;
        }

        if action == "fleet/unregister" || action == "v1/fleet/unregister" {
            #[derive(serde::Deserialize)]
            struct UnregReq {
                client_id: String,
            }
            if let Ok(unreg) = serde_json::from_str::<UnregReq>(payload) {
                let fleet = app.runtime().fleet_registry();
                fleet.unregister_session(&unreg.client_id);
                conn_registered_clients.retain(|c| c != &unreg.client_id);
            }

            let response = if is_versioned {
                serde_json::json!({
                    "version": "1.0",
                    "type": "Response",
                    "id": req_id_val,
                    "status": "success",
                    "body": serde_json::json!({ "unregistered": true })
                })
            } else {
                serde_json::json!({
                    "status": "ok",
                    "unregistered": true
                })
            };
            let mut response_json = serde_json::to_string(&response)?;
            response_json.push('\n');
            let mut w = writer.lock().await;
            w.write_all(response_json.as_bytes()).await?;
            w.flush().await?;
            continue;
        }

        if action == "fleet/snapshot" || action == "v1/fleet/snapshot" {
            #[derive(serde::Deserialize, Default)]
            struct SnapReq {
                client_id: Option<String>,
            }
            let snap_req: SnapReq = serde_json::from_str(payload).unwrap_or_default();
            let fleet = app.runtime().fleet_registry();
            let background_tasks = get_subagent_service().to_fleet_jobs();
            let snapshot = fleet.get_snapshot(snap_req.client_id.as_deref(), background_tasks);

            let response = if is_versioned {
                serde_json::json!({
                    "version": "1.0",
                    "type": "Response",
                    "id": req_id_val,
                    "status": "success",
                    "body": snapshot
                })
            } else {
                serde_json::json!({
                    "status": "ok",
                    "snapshot": snapshot
                })
            };
            let mut response_json = serde_json::to_string(&response)?;
            response_json.push('\n');
            let mut w = writer.lock().await;
            w.write_all(response_json.as_bytes()).await?;
            w.flush().await?;
            continue;
        }

        if action == "fleet/pin" || action == "v1/fleet/pin" {
            #[derive(serde::Deserialize)]
            struct PinReq {
                id: String,
                #[serde(default = "default_fleet_true")]
                pinned: bool,
            }
            fn default_fleet_true() -> bool {
                true
            }

            if let Ok(pin_req) = serde_json::from_str::<PinReq>(payload) {
                let fleet = app.runtime().fleet_registry();
                if pin_req.pinned {
                    fleet.pin(pin_req.id);
                } else {
                    fleet.unpin(&pin_req.id);
                }
            }

            let response = if is_versioned {
                serde_json::json!({
                    "version": "1.0",
                    "type": "Response",
                    "id": req_id_val,
                    "status": "success",
                    "body": serde_json::json!({ "success": true })
                })
            } else {
                serde_json::json!({ "status": "ok", "success": true })
            };
            let mut response_json = serde_json::to_string(&response)?;
            response_json.push('\n');
            let mut w = writer.lock().await;
            w.write_all(response_json.as_bytes()).await?;
            w.flush().await?;
            continue;
        }

        if action == "fleet/unpin" || action == "v1/fleet/unpin" {
            #[derive(serde::Deserialize)]
            struct UnpinReq {
                id: String,
            }
            if let Ok(unpin_req) = serde_json::from_str::<UnpinReq>(payload) {
                let fleet = app.runtime().fleet_registry();
                fleet.unpin(&unpin_req.id);
            }

            let response = if is_versioned {
                serde_json::json!({
                    "version": "1.0",
                    "type": "Response",
                    "id": req_id_val,
                    "status": "success",
                    "body": serde_json::json!({ "success": true })
                })
            } else {
                serde_json::json!({ "status": "ok", "success": true })
            };
            let mut response_json = serde_json::to_string(&response)?;
            response_json.push('\n');
            let mut w = writer.lock().await;
            w.write_all(response_json.as_bytes()).await?;
            w.flush().await?;
            continue;
        }

        if action == "fleet/reorder_pinned" || action == "v1/fleet/reorder_pinned" {
            #[derive(serde::Deserialize)]
            struct ReorderReq {
                order: Vec<String>,
            }
            if let Ok(reorder_req) = serde_json::from_str::<ReorderReq>(payload) {
                let fleet = app.runtime().fleet_registry();
                fleet.reorder_pinned(reorder_req.order);
            }

            let response = if is_versioned {
                serde_json::json!({
                    "version": "1.0",
                    "type": "Response",
                    "id": req_id_val,
                    "status": "success",
                    "body": serde_json::json!({ "success": true })
                })
            } else {
                serde_json::json!({ "status": "ok", "success": true })
            };
            let mut response_json = serde_json::to_string(&response)?;
            response_json.push('\n');
            let mut w = writer.lock().await;
            w.write_all(response_json.as_bytes()).await?;
            w.flush().await?;
            continue;
        }

        if action == "fleet/set_group" || action == "v1/fleet/set_group" {
            #[derive(serde::Deserialize)]
            struct GroupReq {
                id: String,
                group: String,
            }
            if let Ok(group_req) = serde_json::from_str::<GroupReq>(payload) {
                let fleet = app.runtime().fleet_registry();
                fleet.set_group(group_req.id, group_req.group);
            }

            let response = if is_versioned {
                serde_json::json!({
                    "version": "1.0",
                    "type": "Response",
                    "id": req_id_val,
                    "status": "success",
                    "body": serde_json::json!({ "success": true })
                })
            } else {
                serde_json::json!({ "status": "ok", "success": true })
            };
            let mut response_json = serde_json::to_string(&response)?;
            response_json.push('\n');
            let mut w = writer.lock().await;
            w.write_all(response_json.as_bytes()).await?;
            w.flush().await?;
            continue;
        }

        if action == "fleet/clear_group" || action == "v1/fleet/clear_group" {
            #[derive(serde::Deserialize)]
            struct ClearGroupReq {
                id: String,
            }
            if let Ok(clear_req) = serde_json::from_str::<ClearGroupReq>(payload) {
                let fleet = app.runtime().fleet_registry();
                fleet.clear_group(&clear_req.id);
            }

            let response = if is_versioned {
                serde_json::json!({
                    "version": "1.0",
                    "type": "Response",
                    "id": req_id_val,
                    "status": "success",
                    "body": serde_json::json!({ "success": true })
                })
            } else {
                serde_json::json!({ "status": "ok", "success": true })
            };
            let mut response_json = serde_json::to_string(&response)?;
            response_json.push('\n');
            let mut w = writer.lock().await;
            w.write_all(response_json.as_bytes()).await?;
            w.flush().await?;
            continue;
        }

        if action == "fleet/list_groups" || action == "v1/fleet/list_groups" {
            let fleet = app.runtime().fleet_registry();
            let groups = fleet.list_groups();

            let response = if is_versioned {
                serde_json::json!({
                    "version": "1.0",
                    "type": "Response",
                    "id": req_id_val,
                    "status": "success",
                    "body": serde_json::json!({ "groups": groups })
                })
            } else {
                serde_json::json!({ "status": "ok", "groups": groups })
            };
            let mut response_json = serde_json::to_string(&response)?;
            response_json.push('\n');
            let mut w = writer.lock().await;
            w.write_all(response_json.as_bytes()).await?;
            w.flush().await?;
            continue;
        }

        if action == "subagent/spawn" || action == "v1/subagent/spawn" {
            #[derive(serde::Deserialize)]
            struct SpawnReq {
                parent_session_id: String,
                role: String,
                prompt: String,
                #[serde(default)]
                agent_type: Option<String>,
                #[serde(default)]
                depth: Option<u32>,
                #[serde(default)]
                system_prompt: Option<String>,
                #[serde(default)]
                timeout_ms: Option<u64>,
                #[serde(default)]
                use_worktree: Option<bool>,
                #[serde(default)]
                worktree_name: Option<String>,
                #[serde(default)]
                kind: Option<String>,
                #[serde(default)]
                execution_mode: Option<String>,
            }

            let req: SpawnReq = match serde_json::from_str(payload) {
                Ok(r) => r,
                Err(e) => {
                    let response = if is_versioned {
                        serde_json::json!({
                            "version": "1.0",
                            "type": "Response",
                            "id": req_id_val,
                            "status": "error",
                            "body": format!("Invalid spawn payload: {}", e)
                        })
                    } else {
                        serde_json::json!({
                            "status": "error",
                            "message": format!("Invalid spawn payload: {}", e)
                        })
                    };
                    let mut response_json = serde_json::to_string(&response)?;
                    response_json.push('\n');
                    let mut w = writer.lock().await;
                    w.write_all(response_json.as_bytes()).await?;
                    w.flush().await?;
                    continue;
                }
            };

            let parent_id = match req.parent_session_id.parse::<ulid::Ulid>() {
                Ok(u) => brain_domain::SessionId(u),
                Err(e) => {
                    let response = if is_versioned {
                        serde_json::json!({
                            "version": "1.0",
                            "type": "Response",
                            "id": req_id_val,
                            "status": "error",
                            "body": format!("Invalid parent_session_id: {}", e)
                        })
                    } else {
                        serde_json::json!({
                            "status": "error",
                            "message": format!("Invalid parent_session_id: {}", e)
                        })
                    };
                    let mut response_json = serde_json::to_string(&response)?;
                    response_json.push('\n');
                    let mut w = writer.lock().await;
                    w.write_all(response_json.as_bytes()).await?;
                    w.flush().await?;
                    continue;
                }
            };

            let spawn_req = brain_services::subagent::SubagentSpawnRequest {
                parent_session_id: parent_id,
                role: req.role,
                agent_type: req.agent_type.unwrap_or_else(|| "task".to_string()),
                prompt: req.prompt,
                depth: req.depth.unwrap_or(1),
                system_prompt: req.system_prompt,
                timeout: req.timeout_ms.map(std::time::Duration::from_millis),
                use_worktree: req.use_worktree.unwrap_or(false),
                worktree_name: req.worktree_name,
                kind: req.kind.map(|k| match k.to_lowercase().as_str() {
                    "fork" => brain_domain::SubagentKind::Fork,
                    _ => brain_domain::SubagentKind::Named,
                }),
                execution_mode: req.execution_mode.map(|m| match m.to_lowercase().as_str() {
                    "background" => brain_domain::SubagentExecutionMode::Background,
                    _ => brain_domain::SubagentExecutionMode::Foreground,
                }),
            };

            let subagent_svc = get_subagent_service();
            match subagent_svc.spawn(spawn_req, None) {
                Ok(subagent_id) => {
                    let _ = subagent_svc.start(&subagent_id);
                    let response = if is_versioned {
                        serde_json::json!({
                            "version": "1.0",
                            "type": "Response",
                            "id": req_id_val,
                            "status": "success",
                            "body": serde_json::json!({
                                "subagent_id": subagent_id.to_string(),
                                "status": "running"
                            })
                        })
                    } else {
                        serde_json::json!({
                            "status": "ok",
                            "subagent_id": subagent_id.to_string(),
                            "status_text": "running"
                        })
                    };
                    let mut response_json = serde_json::to_string(&response)?;
                    response_json.push('\n');
                    let mut w = writer.lock().await;
                    w.write_all(response_json.as_bytes()).await?;
                    w.flush().await?;
                }
                Err(err) => {
                    let response = if is_versioned {
                        serde_json::json!({
                            "version": "1.0",
                            "type": "Response",
                            "id": req_id_val,
                            "status": "error",
                            "body": format!("Failed to spawn subagent: {}", err)
                        })
                    } else {
                        serde_json::json!({
                            "status": "error",
                            "message": format!("Failed to spawn subagent: {}", err)
                        })
                    };
                    let mut response_json = serde_json::to_string(&response)?;
                    response_json.push('\n');
                    let mut w = writer.lock().await;
                    w.write_all(response_json.as_bytes()).await?;
                    w.flush().await?;
                }
            }
            continue;
        }

        if action == "subagent/status" || action == "v1/subagent/status" {
            #[derive(serde::Deserialize)]
            struct StatusReq {
                subagent_id: String,
            }
            let req: Result<StatusReq, _> = serde_json::from_str(payload);
            let subagent = match req {
                Ok(r) => match r.subagent_id.parse::<ulid::Ulid>() {
                    Ok(u) => get_subagent_service().get_subagent(&brain_domain::SubagentId(u)),
                    Err(_) => None,
                },
                Err(_) => None,
            };

            let response = match subagent {
                Some(s) => {
                    if is_versioned {
                        serde_json::json!({
                            "version": "1.0",
                            "type": "Response",
                            "id": req_id_val,
                            "status": "success",
                            "body": s
                        })
                    } else {
                        serde_json::json!({
                            "status": "ok",
                            "subagent": s
                        })
                    }
                }
                None => {
                    if is_versioned {
                        serde_json::json!({
                            "version": "1.0",
                            "type": "Response",
                            "id": req_id_val,
                            "status": "error",
                            "body": "Subagent not found"
                        })
                    } else {
                        serde_json::json!({
                            "status": "error",
                            "message": "Subagent not found"
                        })
                    }
                }
            };
            let mut response_json = serde_json::to_string(&response)?;
            response_json.push('\n');
            let mut w = writer.lock().await;
            w.write_all(response_json.as_bytes()).await?;
            w.flush().await?;
            continue;
        }

        if action == "subagent/cancel" || action == "v1/subagent/cancel" {
            #[derive(serde::Deserialize)]
            struct CancelReq {
                subagent_id: String,
                #[serde(default)]
                reason: Option<String>,
            }
            let req: Result<CancelReq, _> = serde_json::from_str(payload);
            let cancel_res = match req {
                Ok(r) => match r.subagent_id.parse::<ulid::Ulid>() {
                    Ok(u) => get_subagent_service().cancel(
                        &brain_domain::SubagentId(u),
                        r.reason.as_deref().unwrap_or("Cancelled by user"),
                    ),
                    Err(e) => Err(brain_services::subagent::SubagentError::DomainError(
                        brain_domain::DomainError::ValidationError {
                            message: format!("Invalid subagent_id: {}", e),
                            rule_id: None,
                        },
                    )),
                },
                Err(e) => Err(brain_services::subagent::SubagentError::DomainError(
                    brain_domain::DomainError::ValidationError {
                        message: format!("Invalid payload: {}", e),
                        rule_id: None,
                    },
                )),
            };

            let response = match cancel_res {
                Ok(_) => {
                    if is_versioned {
                        serde_json::json!({
                            "version": "1.0",
                            "type": "Response",
                            "id": req_id_val,
                            "status": "success",
                            "body": serde_json::json!({ "cancelled": true })
                        })
                    } else {
                        serde_json::json!({ "status": "ok", "cancelled": true })
                    }
                }
                Err(e) => {
                    if is_versioned {
                        serde_json::json!({
                            "version": "1.0",
                            "type": "Response",
                            "id": req_id_val,
                            "status": "error",
                            "body": format!("Failed to cancel: {}", e)
                        })
                    } else {
                        serde_json::json!({
                            "status": "error",
                            "message": format!("Failed to cancel: {}", e)
                        })
                    }
                }
            };
            let mut response_json = serde_json::to_string(&response)?;
            response_json.push('\n');
            let mut w = writer.lock().await;
            w.write_all(response_json.as_bytes()).await?;
            w.flush().await?;
            continue;
        }

        if action == "subagent/list" || action == "v1/subagent/list" {
            #[derive(serde::Deserialize, Default)]
            struct ListReq {
                parent_session_id: Option<String>,
            }
            let req: ListReq = serde_json::from_str(payload).unwrap_or_default();
            let subagents = if let Some(parent_str) = req.parent_session_id {
                match parent_str.parse::<ulid::Ulid>() {
                    Ok(u) => get_subagent_service().list_for_parent(&brain_domain::SessionId(u)),
                    Err(_) => Vec::new(),
                }
            } else {
                get_subagent_service().list_all()
            };

            let response = if is_versioned {
                serde_json::json!({
                    "version": "1.0",
                    "type": "Response",
                    "id": req_id_val,
                    "status": "success",
                    "body": subagents
                })
            } else {
                serde_json::json!({
                    "status": "ok",
                    "subagents": subagents
                })
            };
            let mut response_json = serde_json::to_string(&response)?;
            response_json.push('\n');
            let mut w = writer.lock().await;
            w.write_all(response_json.as_bytes()).await?;
            w.flush().await?;
            continue;
        }

        if action == "session/start" || action == "v1/session/start" {
            let start_req: crate::server::protocol::StartSessionPayload = serde_json::from_str(
                payload,
            )
            .unwrap_or(crate::server::protocol::StartSessionPayload {
                session_id: None,
                title: None,
                workspace_path: None,
                system_prompt: None,
            });

            let parsed_sid = match start_req.session_id.as_deref() {
                Some(raw) => require_canonical_session_id!(raw, is_versioned, req_id_val),
                None => brain_domain::SessionId::new(),
            };
            let created_at_ms = chrono::Utc::now().timestamp_millis();
            let now_secs = (created_at_ms / 1000) as u64;

            let storage = app.runtime().sqlite_storage();
            use brain_core::repositories::SessionRepository;
            let existing = storage.load_session(&parsed_sid)?;
            let (sid, title, response_created_at_ms) = match existing {
                Some(existing) => (
                    existing.id.to_string(),
                    existing.title.0,
                    (existing.updated_at.0 * 1000) as i64,
                ),
                None => {
                    let title = start_req.title.unwrap_or_else(|| "New Session".to_string());
                    let mut session = brain_domain::Session::new(
                        parsed_sid,
                        brain_domain::SessionTitle(title.clone()),
                        brain_domain::SessionTimestamp(now_secs),
                    );
                    if let Some(ref sp) = start_req.system_prompt {
                        let cleaned = sp.trim().to_string();
                        session.system_prompt = if cleaned.is_empty() {
                            None
                        } else {
                            Some(cleaned)
                        };
                    }
                    storage.save_session(&parsed_sid, &session)?;
                    (parsed_sid.to_string(), title, created_at_ms)
                }
            };

            let resp_body = crate::server::protocol::StartSessionResponseBody {
                session_id: sid,
                title,
                created_at_ms: response_created_at_ms,
            };

            let response = if is_versioned {
                serde_json::json!({
                    "version": "1.0",
                    "type": "Response",
                    "id": req_id_val,
                    "status": "success",
                    "body": serde_json::to_string(&resp_body)?
                })
            } else {
                serde_json::json!({
                    "status": "ok",
                    "body": resp_body
                })
            };

            let mut response_json = serde_json::to_string(&response)?;
            response_json.push('\n');
            let mut w = writer.lock().await;
            w.write_all(response_json.as_bytes()).await?;
            w.flush().await?;
            continue;
        }

        if action == "session/list" || action == "v1/session/list" {
            let list_req: crate::server::protocol::ListSessionsPayload =
                serde_json::from_str(payload).unwrap_or_default();
            let limit = list_req.limit.unwrap_or(50);
            let offset = list_req.offset.unwrap_or(0);

            let storage = app.runtime().sqlite_storage();
            let raw_sessions = storage.list_sessions(limit, offset).unwrap_or_default();

            let summaries: Vec<crate::server::protocol::SessionSummaryDto> = raw_sessions
                .into_iter()
                .map(|s| {
                    let updated_ms = (s.updated_at.0 * 1000) as i64;
                    let created_ms = updated_ms;
                    crate::server::protocol::SessionSummaryDto {
                        session_id: s.id.to_string(),
                        title: s.title.0,
                        message_count: s.messages.len(),
                        created_at_ms: created_ms,
                        updated_at_ms: updated_ms,
                        workspace_path: list_req.workspace_path.clone(),
                        pinned: s.pinned,
                        archived: s.archived,
                        system_prompt: s.system_prompt.clone(),
                    }
                })
                .collect();

            let total = summaries.len();
            let resp_body = crate::server::protocol::ListSessionsResponseBody {
                sessions: summaries,
                total,
            };

            let response = if is_versioned {
                serde_json::json!({
                    "version": "1.0",
                    "type": "Response",
                    "id": req_id_val,
                    "status": "success",
                    "body": serde_json::to_string(&resp_body)?
                })
            } else {
                serde_json::json!({
                    "status": "ok",
                    "body": resp_body
                })
            };

            let mut response_json = serde_json::to_string(&response)?;
            response_json.push('\n');
            let mut w = writer.lock().await;
            w.write_all(response_json.as_bytes()).await?;
            w.flush().await?;
            continue;
        }

        if action == "session/load" || action == "v1/session/load" {
            let load_req: Result<crate::server::protocol::LoadSessionPayload, _> =
                serde_json::from_str(payload);
            match load_req {
                Ok(req) => {
                    let storage = app.runtime().sqlite_storage();
                    use brain_core::repositories::SessionRepository;
                    let parsed_sid =
                        require_canonical_session_id!(&req.session_id, is_versioned, req_id_val);
                    let maybe_session = storage.load_session(&parsed_sid).ok().flatten();

                    match maybe_session {
                        Some(session) => {
                            let updated_ms = (session.updated_at.0 * 1000) as i64;
                            let messages: Vec<crate::server::protocol::SessionMessageDto> = session
                                .messages
                                .iter()
                                .map(|m| {
                                    let role_str = match m.role {
                                        brain_domain::MessageRole::User => "user",
                                        brain_domain::MessageRole::Assistant => "assistant",
                                        brain_domain::MessageRole::System => "system",
                                        brain_domain::MessageRole::Tool => "tool",
                                        brain_domain::MessageRole::Thinking => "thinking",
                                    };
                                    crate::server::protocol::SessionMessageDto {
                                        id: m.id.to_string(),
                                        role: role_str.to_string(),
                                        content: m.content.clone(),
                                        timestamp: m.timestamp,
                                    }
                                })
                                .collect();

                            let detail = crate::server::protocol::SessionDetailDto {
                                session_id: session.id.to_string(),
                                title: session.title.0,
                                created_at_ms: updated_ms,
                                updated_at_ms: updated_ms,
                                workspace_path: None,
                                system_prompt: session.system_prompt.clone(),
                                messages,
                            };

                            let resp_body = crate::server::protocol::LoadSessionResponseBody {
                                session: detail,
                            };

                            let response = if is_versioned {
                                serde_json::json!({
                                    "version": "1.0",
                                    "type": "Response",
                                    "id": req_id_val,
                                    "status": "success",
                                    "body": serde_json::to_string(&resp_body)?
                                })
                            } else {
                                serde_json::json!({
                                    "status": "ok",
                                    "body": resp_body
                                })
                            };

                            let mut response_json = serde_json::to_string(&response)?;
                            response_json.push('\n');
                            let mut w = writer.lock().await;
                            w.write_all(response_json.as_bytes()).await?;
                            w.flush().await?;
                        }
                        None => {
                            let err_msg = format!("Session not found: {}", req.session_id);
                            let response = if is_versioned {
                                serde_json::json!({
                                    "version": "1.0",
                                    "type": "Error",
                                    "id": req_id_val,
                                    "status": "error",
                                    "code": "session_not_found",
                                    "body": err_msg
                                })
                            } else {
                                serde_json::json!({
                                    "status": "error",
                                    "code": "session_not_found",
                                    "message": err_msg
                                })
                            };
                            let mut response_json = serde_json::to_string(&response)?;
                            response_json.push('\n');
                            let mut w = writer.lock().await;
                            w.write_all(response_json.as_bytes()).await?;
                            w.flush().await?;
                        }
                    }
                }
                Err(e) => {
                    let response = if is_versioned {
                        serde_json::json!({
                            "version": "1.0",
                            "type": "Error",
                            "id": req_id_val,
                            "status": "error",
                            "body": format!("Invalid session/load payload: {}", e)
                        })
                    } else {
                        serde_json::json!({
                            "status": "error",
                            "message": format!("Invalid session/load payload: {}", e)
                        })
                    };
                    let mut response_json = serde_json::to_string(&response)?;
                    response_json.push('\n');
                    let mut w = writer.lock().await;
                    w.write_all(response_json.as_bytes()).await?;
                    w.flush().await?;
                }
            }
            continue;
        }

        if action == "session/fork" || action == "v1/session/fork" {
            #[derive(serde::Deserialize)]
            struct ForkPayload {
                #[serde(rename = "source_session_id", alias = "sourceSessionId")]
                source_session_id: String,
                #[serde(rename = "at_message_id", alias = "atMessageId", default)]
                at_message_id: Option<String>,
                #[serde(rename = "new_title", alias = "newTitle", default)]
                new_title: Option<String>,
            }

            let fork_req: Result<ForkPayload, _> = serde_json::from_str(payload);
            match fork_req {
                Ok(p) => {
                    let parsed_src = require_canonical_session_id!(
                        &p.source_session_id,
                        is_versioned,
                        req_id_val
                    );
                    let req = brain_integrations::dto::v1::ForkSessionRequest {
                        source_session_id: parsed_src.to_string(),
                        at_message_id: p.at_message_id,
                        new_title: p.new_title,
                    };
                    match app.fork_session(req).await {
                        Ok(res) => {
                            let response = if is_versioned {
                                serde_json::json!({
                                    "version": "1.0",
                                    "type": "Response",
                                    "id": req_id_val,
                                    "status": "success",
                                    "body": serde_json::to_string(&res)?
                                })
                            } else {
                                serde_json::json!({
                                    "status": "ok",
                                    "body": res
                                })
                            };
                            let mut response_json = serde_json::to_string(&response)?;
                            response_json.push('\n');
                            let mut w = writer.lock().await;
                            w.write_all(response_json.as_bytes()).await?;
                            w.flush().await?;
                        }
                        Err(e) => {
                            let err_msg = format!("Failed to fork session: {}", e);
                            let response = if is_versioned {
                                serde_json::json!({
                                    "version": "1.0",
                                    "type": "Error",
                                    "id": req_id_val,
                                    "status": "error",
                                    "body": err_msg
                                })
                            } else {
                                serde_json::json!({
                                    "status": "error",
                                    "message": err_msg
                                })
                            };
                            let mut response_json = serde_json::to_string(&response)?;
                            response_json.push('\n');
                            let mut w = writer.lock().await;
                            w.write_all(response_json.as_bytes()).await?;
                            w.flush().await?;
                        }
                    }
                }
                Err(e) => {
                    let err_msg = format!("Invalid session/fork payload: {}", e);
                    let response = if is_versioned {
                        serde_json::json!({
                            "version": "1.0",
                            "type": "Error",
                            "id": req_id_val,
                            "status": "error",
                            "body": err_msg
                        })
                    } else {
                        serde_json::json!({
                            "status": "error",
                            "message": err_msg
                        })
                    };
                    let mut response_json = serde_json::to_string(&response)?;
                    response_json.push('\n');
                    let mut w = writer.lock().await;
                    w.write_all(response_json.as_bytes()).await?;
                    w.flush().await?;
                }
            }
            continue;
        }

        if action == "session/rename" || action == "v1/session/rename" {
            #[derive(serde::Deserialize)]
            struct RenamePayload {
                #[serde(rename = "session_id", alias = "sessionId")]
                session_id: String,
                #[serde(rename = "new_title", alias = "newTitle")]
                new_title: String,
            }

            let rename_req: Result<RenamePayload, _> = serde_json::from_str(payload);
            match rename_req {
                Ok(p) => {
                    let parsed_sid =
                        require_canonical_session_id!(&p.session_id, is_versioned, req_id_val);
                    match app
                        .rename_session(&parsed_sid.to_string(), &p.new_title)
                        .await
                    {
                        Ok(()) => {
                            let response = if is_versioned {
                                serde_json::json!({
                                    "version": "1.0",
                                    "type": "Response",
                                    "id": req_id_val,
                                    "status": "success",
                                    "body": "ok"
                                })
                            } else {
                                serde_json::json!({
                                    "status": "ok",
                                    "message": "Session renamed successfully"
                                })
                            };
                            let mut response_json = serde_json::to_string(&response)?;
                            response_json.push('\n');
                            let mut w = writer.lock().await;
                            w.write_all(response_json.as_bytes()).await?;
                            w.flush().await?;
                        }
                        Err(e) => {
                            let err_msg = format!("Failed to rename session: {}", e);
                            let response = if is_versioned {
                                serde_json::json!({
                                    "version": "1.0",
                                    "type": "Error",
                                    "id": req_id_val,
                                    "status": "error",
                                    "body": err_msg
                                })
                            } else {
                                serde_json::json!({
                                    "status": "error",
                                    "message": err_msg
                                })
                            };
                            let mut response_json = serde_json::to_string(&response)?;
                            response_json.push('\n');
                            let mut w = writer.lock().await;
                            w.write_all(response_json.as_bytes()).await?;
                            w.flush().await?;
                        }
                    }
                }
                Err(e) => {
                    let err_msg = format!("Invalid session/rename payload: {}", e);
                    let response = if is_versioned {
                        serde_json::json!({
                            "version": "1.0",
                            "type": "Error",
                            "id": req_id_val,
                            "status": "error",
                            "body": err_msg
                        })
                    } else {
                        serde_json::json!({
                            "status": "error",
                            "message": err_msg
                        })
                    };
                    let mut response_json = serde_json::to_string(&response)?;
                    response_json.push('\n');
                    let mut w = writer.lock().await;
                    w.write_all(response_json.as_bytes()).await?;
                    w.flush().await?;
                }
            }
            continue;
        }

        if action == "session/set_system_prompt"
            || action == "v1/session/set_system_prompt"
            || action == "v1/session/update_instructions"
        {
            let prompt_req: Result<crate::server::protocol::SetSessionSystemPromptPayload, _> =
                serde_json::from_str(payload);
            match prompt_req {
                Ok(p) => {
                    let storage = app.runtime().sqlite_storage();
                    use brain_core::repositories::SessionRepository;
                    let parsed_sid =
                        require_canonical_session_id!(&p.session_id, is_versioned, req_id_val);
                    let maybe_session = storage.load_session(&parsed_sid).ok().flatten();
                    match maybe_session {
                        Some(mut session) => {
                            let now_secs = (chrono::Utc::now().timestamp_millis() / 1000) as u64;
                            session.set_system_prompt(
                                p.system_prompt,
                                brain_domain::SessionTimestamp(now_secs),
                            );
                            let _ = storage.save_session(&parsed_sid, &session);

                            let response = if is_versioned {
                                serde_json::json!({
                                    "version": "1.0",
                                    "type": "Response",
                                    "id": req_id_val,
                                    "status": "success",
                                    "body": "ok"
                                })
                            } else {
                                serde_json::json!({
                                    "status": "ok",
                                    "message": "System prompt updated successfully"
                                })
                            };
                            let mut response_json = serde_json::to_string(&response)?;
                            response_json.push('\n');
                            let mut w = writer.lock().await;
                            w.write_all(response_json.as_bytes()).await?;
                            w.flush().await?;
                        }
                        None => {
                            let err_msg = format!("Session '{}' not found", p.session_id);
                            let response = if is_versioned {
                                serde_json::json!({
                                    "version": "1.0",
                                    "type": "Error",
                                    "id": req_id_val,
                                    "status": "error",
                                    "body": err_msg
                                })
                            } else {
                                serde_json::json!({
                                    "status": "error",
                                    "message": err_msg
                                })
                            };
                            let mut response_json = serde_json::to_string(&response)?;
                            response_json.push('\n');
                            let mut w = writer.lock().await;
                            w.write_all(response_json.as_bytes()).await?;
                            w.flush().await?;
                        }
                    }
                }
                Err(e) => {
                    let err_msg = format!("Invalid session/set_system_prompt payload: {}", e);
                    let response = if is_versioned {
                        serde_json::json!({
                            "version": "1.0",
                            "type": "Error",
                            "id": req_id_val,
                            "status": "error",
                            "body": err_msg
                        })
                    } else {
                        serde_json::json!({
                            "status": "error",
                            "message": err_msg
                        })
                    };
                    let mut response_json = serde_json::to_string(&response)?;
                    response_json.push('\n');
                    let mut w = writer.lock().await;
                    w.write_all(response_json.as_bytes()).await?;
                    w.flush().await?;
                }
            }
            continue;
        }

        if action == "session/pin" || action == "v1/session/pin" {
            #[derive(serde::Deserialize)]
            struct PinPayload {
                #[serde(rename = "session_id", alias = "sessionId")]
                session_id: String,
                pinned: bool,
            }

            let pin_req: Result<PinPayload, _> = serde_json::from_str(payload);
            match pin_req {
                Ok(p) => {
                    let parsed_sid =
                        require_canonical_session_id!(&p.session_id, is_versioned, req_id_val);
                    match app.pin_session(&parsed_sid.to_string(), p.pinned).await {
                        Ok(()) => {
                            let response = if is_versioned {
                                serde_json::json!({
                                    "version": "1.0",
                                    "type": "Response",
                                    "id": req_id_val,
                                    "status": "success",
                                    "body": "ok"
                                })
                            } else {
                                serde_json::json!({
                                    "status": "ok",
                                    "message": "Session pin status updated successfully"
                                })
                            };
                            let mut response_json = serde_json::to_string(&response)?;
                            response_json.push('\n');
                            let mut w = writer.lock().await;
                            w.write_all(response_json.as_bytes()).await?;
                            w.flush().await?;
                        }
                        Err(e) => {
                            let err_msg = format!("Failed to update session pin status: {}", e);
                            let response = if is_versioned {
                                serde_json::json!({
                                    "version": "1.0",
                                    "type": "Error",
                                    "id": req_id_val,
                                    "status": "error",
                                    "body": err_msg
                                })
                            } else {
                                serde_json::json!({
                                    "status": "error",
                                    "message": err_msg
                                })
                            };
                            let mut response_json = serde_json::to_string(&response)?;
                            response_json.push('\n');
                            let mut w = writer.lock().await;
                            w.write_all(response_json.as_bytes()).await?;
                            w.flush().await?;
                        }
                    }
                }
                Err(e) => {
                    let err_msg = format!("Invalid session/pin payload: {}", e);
                    let response = if is_versioned {
                        serde_json::json!({
                            "version": "1.0",
                            "type": "Error",
                            "id": req_id_val,
                            "status": "error",
                            "body": err_msg
                        })
                    } else {
                        serde_json::json!({
                            "status": "error",
                            "message": err_msg
                        })
                    };
                    let mut response_json = serde_json::to_string(&response)?;
                    response_json.push('\n');
                    let mut w = writer.lock().await;
                    w.write_all(response_json.as_bytes()).await?;
                    w.flush().await?;
                }
            }
            continue;
        }

        if action == "session/archive" || action == "v1/session/archive" {
            #[derive(serde::Deserialize)]
            struct ArchivePayload {
                #[serde(rename = "session_id", alias = "sessionId", default)]
                session_id: Option<String>,
                #[serde(default)]
                id: Option<String>,
            }

            let sid_str = if let Ok(p) = serde_json::from_str::<ArchivePayload>(payload) {
                p.session_id
                    .or(p.id)
                    .unwrap_or_else(|| payload.trim().to_string())
            } else {
                payload.trim().to_string()
            };

            let parsed_sid = require_canonical_session_id!(&sid_str, is_versioned, req_id_val);
            match app.archive_session(&parsed_sid.to_string()).await {
                Ok(()) => {
                    let response = if is_versioned {
                        serde_json::json!({
                            "version": "1.0",
                            "type": "Response",
                            "id": req_id_val,
                            "status": "success",
                            "body": "ok"
                        })
                    } else {
                        serde_json::json!({
                            "status": "ok",
                            "message": "Session archived successfully"
                        })
                    };
                    let mut response_json = serde_json::to_string(&response)?;
                    response_json.push('\n');
                    let mut w = writer.lock().await;
                    w.write_all(response_json.as_bytes()).await?;
                    w.flush().await?;
                }
                Err(e) => {
                    let err_msg = format!("Failed to archive session: {}", e);
                    let response = if is_versioned {
                        serde_json::json!({
                            "version": "1.0",
                            "type": "Error",
                            "id": req_id_val,
                            "status": "error",
                            "body": err_msg
                        })
                    } else {
                        serde_json::json!({
                            "status": "error",
                            "message": err_msg
                        })
                    };
                    let mut response_json = serde_json::to_string(&response)?;
                    response_json.push('\n');
                    let mut w = writer.lock().await;
                    w.write_all(response_json.as_bytes()).await?;
                    w.flush().await?;
                }
            }
            continue;
        }

        if action == "session/restore" || action == "v1/session/restore" {
            #[derive(serde::Deserialize)]
            struct RestorePayload {
                #[serde(rename = "session_id", alias = "sessionId", default)]
                session_id: Option<String>,
                #[serde(default)]
                id: Option<String>,
            }

            let sid_str = if let Ok(p) = serde_json::from_str::<RestorePayload>(payload) {
                p.session_id
                    .or(p.id)
                    .unwrap_or_else(|| payload.trim().to_string())
            } else {
                payload.trim().to_string()
            };

            let parsed_sid = require_canonical_session_id!(&sid_str, is_versioned, req_id_val);
            match app.restore_session(&parsed_sid.to_string()).await {
                Ok(()) => {
                    let response = if is_versioned {
                        serde_json::json!({
                            "version": "1.0",
                            "type": "Response",
                            "id": req_id_val,
                            "status": "success",
                            "body": "ok"
                        })
                    } else {
                        serde_json::json!({
                            "status": "ok",
                            "message": "Session restored successfully"
                        })
                    };
                    let mut response_json = serde_json::to_string(&response)?;
                    response_json.push('\n');
                    let mut w = writer.lock().await;
                    w.write_all(response_json.as_bytes()).await?;
                    w.flush().await?;
                }
                Err(e) => {
                    let err_msg = format!("Failed to restore session: {}", e);
                    let response = if is_versioned {
                        serde_json::json!({
                            "version": "1.0",
                            "type": "Error",
                            "id": req_id_val,
                            "status": "error",
                            "body": err_msg
                        })
                    } else {
                        serde_json::json!({
                            "status": "error",
                            "message": err_msg
                        })
                    };
                    let mut response_json = serde_json::to_string(&response)?;
                    response_json.push('\n');
                    let mut w = writer.lock().await;
                    w.write_all(response_json.as_bytes()).await?;
                    w.flush().await?;
                }
            }
            continue;
        }

        if action == "session/delete" || action == "v1/session/delete" {
            #[derive(serde::Deserialize)]
            struct DeletePayload {
                #[serde(rename = "session_id", alias = "sessionId", default)]
                session_id: Option<String>,
                #[serde(default)]
                id: Option<String>,
            }

            let sid_str = if let Ok(p) = serde_json::from_str::<DeletePayload>(payload) {
                p.session_id
                    .or(p.id)
                    .unwrap_or_else(|| payload.trim().to_string())
            } else {
                payload.trim().to_string()
            };

            let parsed_sid = require_canonical_session_id!(&sid_str, is_versioned, req_id_val);
            match app.delete_session(&parsed_sid.to_string()).await {
                Ok(()) => {
                    let response = if is_versioned {
                        serde_json::json!({
                            "version": "1.0",
                            "type": "Response",
                            "id": req_id_val,
                            "status": "success",
                            "body": {
                                "success": true,
                                "session_id": parsed_sid.to_string()
                            }
                        })
                    } else {
                        serde_json::json!({
                            "status": "ok",
                            "success": true,
                            "session_id": parsed_sid.to_string(),
                            "message": "Session deleted successfully"
                        })
                    };
                    let mut response_json = serde_json::to_string(&response)?;
                    response_json.push('\n');
                    let mut w = writer.lock().await;
                    w.write_all(response_json.as_bytes()).await?;
                    w.flush().await?;
                }
                Err(e) => {
                    let err_msg = format!("Failed to delete session: {}", e);
                    let response = if is_versioned {
                        serde_json::json!({
                            "version": "1.0",
                            "type": "Error",
                            "id": req_id_val,
                            "status": "error",
                            "body": err_msg
                        })
                    } else {
                        serde_json::json!({
                            "status": "error",
                            "message": err_msg
                        })
                    };
                    let mut response_json = serde_json::to_string(&response)?;
                    response_json.push('\n');
                    let mut w = writer.lock().await;
                    w.write_all(response_json.as_bytes()).await?;
                    w.flush().await?;
                }
            }
            continue;
        }

        if action == "session/compact" || action == "v1/session/compact" {
            #[derive(serde::Deserialize)]
            struct CompactPayload {
                #[serde(rename = "session_id", alias = "sessionId")]
                session_id: String,
                #[serde(default)]
                summary: Option<String>,
                #[serde(
                    rename = "keep_last_n",
                    alias = "keepLastN",
                    default = "default_keep_last"
                )]
                keep_last_n: usize,
            }
            fn default_keep_last() -> usize {
                2
            }

            let compact_req: Result<CompactPayload, _> = serde_json::from_str(payload);
            match compact_req {
                Ok(req) => {
                    let parsed_sid =
                        require_canonical_session_id!(&req.session_id, is_versioned, req_id_val);
                    let storage = app.runtime().sqlite_storage();
                    use brain_core::repositories::SessionRepository;
                    match storage.load_session(&parsed_sid) {
                        Ok(Some(mut session)) => {
                            let original_count = session.messages.len();
                            let summary_text = req.summary.unwrap_or_else(|| {
                                format!(
                                    "Compacted conversation (pruned {} turns)",
                                    original_count.saturating_sub(req.keep_last_n)
                                )
                            });
                            let now_secs = std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .unwrap_or_default()
                                .as_secs();
                            let compact_res = session.compact(
                                summary_text.clone(),
                                brain_domain::SessionTimestamp(now_secs),
                                req.keep_last_n,
                            );
                            if let Err(e) = compact_res {
                                let err_msg = format!("Failed to compact session: {}", e);
                                let response = if is_versioned {
                                    serde_json::json!({
                                        "version": "1.0",
                                        "type": "Error",
                                        "id": req_id_val,
                                        "status": "error",
                                        "body": err_msg
                                    })
                                } else {
                                    serde_json::json!({ "status": "error", "message": err_msg })
                                };
                                let mut response_json = serde_json::to_string(&response)?;
                                response_json.push('\n');
                                let mut w = writer.lock().await;
                                w.write_all(response_json.as_bytes()).await?;
                                w.flush().await?;
                                continue;
                            }
                            let compacted_count = session.messages.len();
                            let save_res = storage.save_session(&parsed_sid, &session);
                            if let Err(e) = save_res {
                                let err_msg = format!("Failed to save compacted session: {}", e);
                                let response = if is_versioned {
                                    serde_json::json!({
                                        "version": "1.0",
                                        "type": "Error",
                                        "id": req_id_val,
                                        "status": "error",
                                        "body": err_msg
                                    })
                                } else {
                                    serde_json::json!({ "status": "error", "message": err_msg })
                                };
                                let mut response_json = serde_json::to_string(&response)?;
                                response_json.push('\n');
                                let mut w = writer.lock().await;
                                w.write_all(response_json.as_bytes()).await?;
                                w.flush().await?;
                                continue;
                            }
                            let response_body = serde_json::json!({
                                "status": "ok",
                                "session_id": parsed_sid.to_string(),
                                "original_message_count": original_count,
                                "compacted_message_count": compacted_count,
                                "summary": summary_text
                            });
                            let response = if is_versioned {
                                serde_json::json!({
                                    "version": "1.0",
                                    "type": "Response",
                                    "id": req_id_val,
                                    "status": "success",
                                    "body": response_body
                                })
                            } else {
                                serde_json::json!({
                                    "status": "ok",
                                    "data": response_body
                                })
                            };
                            let mut response_json = serde_json::to_string(&response)?;
                            response_json.push('\n');
                            let mut w = writer.lock().await;
                            w.write_all(response_json.as_bytes()).await?;
                            w.flush().await?;
                        }
                        Ok(None) => {
                            let err_msg = format!("Session '{}' not found", req.session_id);
                            let response = if is_versioned {
                                serde_json::json!({
                                    "version": "1.0",
                                    "type": "Error",
                                    "id": req_id_val,
                                    "status": "error",
                                    "code": "session_not_found",
                                    "body": err_msg
                                })
                            } else {
                                serde_json::json!({ "status": "error", "code": "session_not_found", "message": err_msg })
                            };
                            let mut response_json = serde_json::to_string(&response)?;
                            response_json.push('\n');
                            let mut w = writer.lock().await;
                            w.write_all(response_json.as_bytes()).await?;
                            w.flush().await?;
                        }
                        Err(e) => {
                            let err_msg = format!("Failed to load session: {}", e);
                            let response = if is_versioned {
                                serde_json::json!({
                                    "version": "1.0",
                                    "type": "Error",
                                    "id": req_id_val,
                                    "status": "error",
                                    "body": err_msg
                                })
                            } else {
                                serde_json::json!({ "status": "error", "message": err_msg })
                            };
                            let mut response_json = serde_json::to_string(&response)?;
                            response_json.push('\n');
                            let mut w = writer.lock().await;
                            w.write_all(response_json.as_bytes()).await?;
                            w.flush().await?;
                        }
                    }
                }
                Err(e) => {
                    let err_msg = format!("Invalid compact payload: {}", e);
                    let response = if is_versioned {
                        serde_json::json!({
                            "version": "1.0",
                            "type": "Error",
                            "id": req_id_val,
                            "status": "error",
                            "body": err_msg
                        })
                    } else {
                        serde_json::json!({ "status": "error", "message": err_msg })
                    };
                    let mut response_json = serde_json::to_string(&response)?;
                    response_json.push('\n');
                    let mut w = writer.lock().await;
                    w.write_all(response_json.as_bytes()).await?;
                    w.flush().await?;
                }
            }
            continue;
        }

        if action == "memory/consolidate" || action == "v1/memory/consolidate" {
            let start = std::time::Instant::now();
            let consolidate_req: Result<crate::server::protocol::ConsolidatePayload, _> =
                if payload.trim().is_empty() || payload.trim() == "{}" {
                    Ok(crate::server::protocol::ConsolidatePayload::default())
                } else {
                    serde_json::from_str(payload)
                };

            match consolidate_req {
                Ok(req) => {
                    let config_con = app.runtime().config().consolidation();
                    let promotion = req
                        .promotion_weight_threshold
                        .unwrap_or_else(|| config_con.promotion_weight_threshold());
                    let pruning = req
                        .pruning_weight_threshold
                        .unwrap_or_else(|| config_con.pruning_weight_threshold());
                    let staleness = req
                        .staleness_age_threshold_secs
                        .unwrap_or_else(|| config_con.staleness_age_threshold_secs());

                    let policy = brain_domain::ConsolidationPolicy {
                        promotion_weight_threshold: promotion,
                        pruning_weight_threshold: pruning,
                        staleness_age_threshold_secs: staleness,
                    };

                    if let Err(validation_err) = policy.validate() {
                        let response = if is_versioned {
                            serde_json::json!({
                                "version": "1.0",
                                "type": "Error",
                                "id": req_id_val,
                                "status": "error",
                                "body": format!("Invalid consolidation policy: {}", validation_err)
                            })
                        } else {
                            serde_json::json!({
                                "status": "error",
                                "message": format!("Invalid consolidation policy: {}", validation_err)
                            })
                        };
                        let mut response_json = serde_json::to_string(&response)?;
                        response_json.push('\n');
                        let mut w = writer.lock().await;
                        w.write_all(response_json.as_bytes()).await?;
                        w.flush().await?;
                        continue;
                    }

                    // Single-flight lock so concurrent sweeps are serialized
                    let _guard = get_consolidation_lock().lock().await;
                    let storage = app.runtime().sqlite_storage();

                    match storage.consolidate_memories(policy) {
                        Ok(actions) => {
                            let mut promoted = 0;
                            let mut merged = 0;
                            let mut archived = 0;
                            let mut pruned = 0;

                            for act in &actions {
                                match &act.action {
                                    brain_domain::ConsolidationActionType::PromoteToSemantic {
                                        ..
                                    } => {
                                        promoted += 1;
                                    }
                                    brain_domain::ConsolidationActionType::MergeNodes {
                                        ..
                                    } => {
                                        merged += 1;
                                    }
                                    brain_domain::ConsolidationActionType::ArchiveEdge {
                                        ..
                                    } => {
                                        archived += 1;
                                    }
                                    brain_domain::ConsolidationActionType::PruneEdge { .. } => {
                                        pruned += 1;
                                    }
                                }
                            }

                            let duration_ms = start.elapsed().as_millis() as u64;
                            let resp_body = crate::server::protocol::ConsolidationResponseBody {
                                actions_applied: actions.len(),
                                promoted,
                                merged,
                                archived,
                                pruned,
                                duration_ms,
                                errors: vec![],
                            };

                            let response = if is_versioned {
                                serde_json::json!({
                                    "version": "1.0",
                                    "type": "Response",
                                    "id": req_id_val,
                                    "status": "success",
                                    "body": resp_body
                                })
                            } else {
                                serde_json::json!({
                                    "status": "ok",
                                    "body": resp_body
                                })
                            };

                            let mut response_json = serde_json::to_string(&response)?;
                            response_json.push('\n');
                            let mut w = writer.lock().await;
                            w.write_all(response_json.as_bytes()).await?;
                            w.flush().await?;
                        }
                        Err(e) => {
                            let response = if is_versioned {
                                serde_json::json!({
                                    "version": "1.0",
                                    "type": "Error",
                                    "id": req_id_val,
                                    "status": "error",
                                    "body": format!("Consolidation sweep failed: {}", e)
                                })
                            } else {
                                serde_json::json!({
                                    "status": "error",
                                    "message": format!("Consolidation sweep failed: {}", e)
                                })
                            };
                            let mut response_json = serde_json::to_string(&response)?;
                            response_json.push('\n');
                            let mut w = writer.lock().await;
                            w.write_all(response_json.as_bytes()).await?;
                            w.flush().await?;
                        }
                    }
                }
                Err(e) => {
                    let response = if is_versioned {
                        serde_json::json!({
                            "version": "1.0",
                            "type": "Error",
                            "id": req_id_val,
                            "status": "error",
                            "body": format!("Invalid memory/consolidate payload: {}", e)
                        })
                    } else {
                        serde_json::json!({
                            "status": "error",
                            "message": format!("Invalid memory/consolidate payload: {}", e)
                        })
                    };
                    let mut response_json = serde_json::to_string(&response)?;
                    response_json.push('\n');
                    let mut w = writer.lock().await;
                    w.write_all(response_json.as_bytes()).await?;
                    w.flush().await?;
                }
            }
            continue;
        }

        if action == "model/list" || action == "v1/model/list" {
            let descriptors = app.model_gateway().list_models();
            let dtos: Vec<crate::server::protocol::ModelDescriptorDto> = descriptors
                .into_iter()
                .map(|d| crate::server::protocol::ModelDescriptorDto {
                    id: d.id,
                    name: d.name,
                    provider: d.provider,
                    context_window: d.context_window,
                    max_output_tokens: d.max_output_tokens,
                    supports_thinking: d.supports_thinking,
                    supports_tools: d.supports_tools,
                    is_default: d.is_default,
                })
                .collect();

            let resp_body = crate::server::protocol::ListModelsResponseBody { models: dtos };
            let response = if is_versioned {
                serde_json::json!({
                    "version": "1.0",
                    "type": "Response",
                    "id": req_id_val,
                    "status": "success",
                    "body": resp_body
                })
            } else {
                serde_json::json!({
                    "status": "ok",
                    "body": resp_body
                })
            };

            let mut response_json = serde_json::to_string(&response)?;
            response_json.push('\n');
            let mut w = writer.lock().await;
            w.write_all(response_json.as_bytes()).await?;
            w.flush().await?;
            continue;
        }

        // ── Inc 11: user-initiated shell passthrough (`!`) ──────────────
        // One command per short-lived connection. Validate → grant →
        // persist the user line → execute through THE SAME ToolStack as
        // agentic calls → respond once → persist the executed envelope.
        // Nothing here reaches a provider, and BashTool remains the only
        // code path that spawns /bin/bash.
        if action == "shell/exec" || action == "v1/shell/exec" {
            let exec_req: serde_json::Value = if payload.trim().is_empty() {
                serde_json::Value::Null
            } else {
                serde_json::from_str(payload).unwrap_or(serde_json::Value::Null)
            };
            let command = exec_req["command"]
                .as_str()
                .unwrap_or("")
                .trim()
                .to_string();
            let sid_str = exec_req["session_id"]
                .as_str()
                .or_else(|| exec_req["sessionId"].as_str())
                .unwrap_or("")
                .to_string();
            if sid_str.is_empty() {
                let response = if is_versioned {
                    serde_json::json!({
                        "version": "1.0", "type": "Error", "id": req_id_val,
                        "status": "error", "body": "missing sessionId"
                    })
                } else {
                    serde_json::json!({ "status": "error", "message": "missing sessionId" })
                };
                let mut rj = serde_json::to_string(&response)?;
                rj.push('\n');
                let mut w = writer.lock().await;
                w.write_all(rj.as_bytes()).await?;
                w.flush().await?;
                continue;
            }
            let parsed_session_id =
                require_canonical_session_id!(&sid_str, is_versioned, req_id_val);

            let storage = app.runtime().sqlite_storage();
            use brain_core::repositories::SessionRepository;
            let mut session_aggregate = match storage.load_session(&parsed_session_id) {
                Ok(Some(s)) => s,
                _ => {
                    let msg = format!("Session '{}' not found", sid_str);
                    let response = if is_versioned {
                        serde_json::json!({
                            "version": "1.0", "type": "Error", "id": req_id_val,
                            "status": "error", "body": msg
                        })
                    } else {
                        serde_json::json!({ "status": "error", "message": msg })
                    };
                    let mut rj = serde_json::to_string(&response)?;
                    rj.push('\n');
                    let mut w = writer.lock().await;
                    w.write_all(rj.as_bytes()).await?;
                    w.flush().await?;
                    continue;
                }
            };

            if command.is_empty() {
                let response = if is_versioned {
                    serde_json::json!({
                        "version": "1.0", "type": "Error", "id": req_id_val,
                        "status": "error", "body": "command must be a non-empty string"
                    })
                } else {
                    serde_json::json!({ "status": "error", "message": "command must be a non-empty string" })
                };
                let mut rj = serde_json::to_string(&response)?;
                rj.push('\n');
                let mut w = writer.lock().await;
                w.write_all(rj.as_bytes()).await?;
                w.flush().await?;
                continue;
            }

            // Backstop mirroring the stream arm's Invariant 8: the two arms
            // must never interleave save_session writes to one aggregate.
            {
                let registry = get_generation_registry();
                let reg = registry.read().await;
                if reg
                    .values()
                    .any(|active| active.session_id == parsed_session_id)
                {
                    let msg = format!("Session '{}' is busy with an active generation", sid_str);
                    let response = if is_versioned {
                        serde_json::json!({
                            "version": "1.0", "type": "Error", "id": req_id_val,
                            "status": "error", "body": msg
                        })
                    } else {
                        serde_json::json!({ "status": "error", "message": msg })
                    };
                    let mut rj = serde_json::to_string(&response)?;
                    rj.push('\n');
                    let mut w = writer.lock().await;
                    w.write_all(rj.as_bytes()).await?;
                    w.flush().await?;
                    continue;
                }
            }

            // Keystroke-as-grant (spec §2): the user typed and submitted this
            // Direct command execution (/shell or '!'): the interactive command itself
            // IS the consent act. We scope the required Shell permission strictly to this
            // execution so stack.permissions remains unpolluted and immutable.

            // Persist the attempted command upon acceptance (stream-arm
            // Invariant 4 twin): a crash mid-exec still leaves it recorded.
            let _ = session_aggregate.add_message(brain_domain::Message::new(
                brain_domain::MessageId::new(),
                brain_domain::MessageRole::User,
                format!("! {}", command),
            ));
            session_aggregate.autotitle();
            let _ = storage.save_session(&parsed_session_id, &session_aggregate);

            use brain_core::extensibility::{
                ExecutionContext as ToolExecutionContext, ToolRegistry,
            };
            let stack = crate::tools::tool_stack();
            let mut args_map: HashMap<String, serde_json::Value> = HashMap::new();
            args_map.insert("command".to_string(), serde_json::json!(command));
            let tool_ctx = ToolExecutionContext {
                session_id: parsed_session_id,
                working_dir: std::env::current_dir()
                    .unwrap_or_else(|_| std::path::PathBuf::from(".")),
                cancellation: Arc::new(brain_tools::CancellationTokenImpl::default()),
                deadline: None,
            };
            let tool_exec_started = std::time::Instant::now();

            let shell_permissions = stack
                .permissions
                .scoped_with([brain_core::extensibility::Permission::Shell]);
            match stack.registry.get_tool("bash") {
                Some(tool) => {
                    match stack
                        .executor
                        .execute(tool, &tool_ctx, &shell_permissions, &args_map)
                        .await
                    {
                        Ok(result) => {
                            let v = result.value();
                            let out_text = v["output"].as_str().unwrap_or("").to_string();
                            let is_err = v["is_error"].as_bool().unwrap_or(true);
                            let exit_code = v["exit_code"].as_i64().unwrap_or(-1);
                            let duration_ms = tool_exec_started.elapsed().as_millis() as u64;
                            let call_id = format!("shell-{}", uuid::Uuid::new_v4().simple());
                            let resp_body = serde_json::json!({
                                "call_id": call_id,
                                "name": "bash",
                                "input": { "command": command },
                                "outcome": "executed",
                                "output": out_text,
                                "is_error": is_err,
                                "exit_code": exit_code,
                                "duration_ms": duration_ms,
                            });
                            // Persist the standard executed-case envelope
                            // (byte-identical schema to the agentic site).
                            let envelope = serde_json::json!({
                                "type": "tool_event",
                                "v": 1,
                                "call_id": call_id,
                                "name": "bash",
                                "input": { "command": command },
                                "outcome": "executed",
                                "is_error": is_err,
                                "exit_code": exit_code,
                                "output": truncate_tool_output(&out_text),
                                "duration_ms": duration_ms,
                            });
                            let _ = session_aggregate.add_message(brain_domain::Message::new(
                                brain_domain::MessageId::new(),
                                brain_domain::MessageRole::Tool,
                                envelope.to_string(),
                            ));
                            if let Err(e) =
                                storage.save_session(&parsed_session_id, &session_aggregate)
                            {
                                tracing::warn!(
                                    error = %e,
                                    "tool event persistence failed; continuing"
                                );
                            }

                            // Clients can load the result as soon as the acknowledgement arrives.
                            let response = if is_versioned {
                                serde_json::json!({
                                    "version": "1.0", "type": "Response",
                                    "id": req_id_val, "status": "success",
                                    "body": resp_body
                                })
                            } else {
                                serde_json::json!({ "status": "ok", "body": resp_body })
                            };
                            let mut rj = serde_json::to_string(&response)?;
                            rj.push('\n');
                            let mut w = writer.lock().await;
                            w.write_all(rj.as_bytes()).await?;
                            w.flush().await?;
                        }
                        Err(e) => {
                            let msg = format!("shell exec failed: {}", e);
                            let response = if is_versioned {
                                serde_json::json!({
                                    "version": "1.0", "type": "Error",
                                    "id": req_id_val, "status": "error",
                                    "body": msg
                                })
                            } else {
                                serde_json::json!({ "status": "error", "message": msg })
                            };
                            let mut rj = serde_json::to_string(&response)?;
                            rj.push('\n');
                            let mut w = writer.lock().await;
                            w.write_all(rj.as_bytes()).await?;
                            w.flush().await?;
                        }
                    }
                }
                None => {
                    let response = if is_versioned {
                        serde_json::json!({
                            "version": "1.0", "type": "Error", "id": req_id_val,
                            "status": "error", "body": "bash tool is not registered"
                        })
                    } else {
                        serde_json::json!({ "status": "error", "message": "bash tool is not registered" })
                    };
                    let mut rj = serde_json::to_string(&response)?;
                    rj.push('\n');
                    let mut w = writer.lock().await;
                    w.write_all(rj.as_bytes()).await?;
                    w.flush().await?;
                }
            }
            continue;
        }

        if action == "model/resolve" || action == "v1/model/resolve" {
            let resolve_req: Result<crate::server::protocol::ResolveModelPayload, _> =
                if payload.trim().is_empty() || payload.trim() == "{}" {
                    Ok(crate::server::protocol::ResolveModelPayload::default())
                } else {
                    serde_json::from_str(payload)
                };

            match resolve_req {
                Ok(req) => match app.model_gateway().resolve_model(req.query.as_deref()) {
                    Ok(d) => {
                        let dto = crate::server::protocol::ModelDescriptorDto {
                            id: d.id,
                            name: d.name,
                            provider: d.provider,
                            context_window: d.context_window,
                            max_output_tokens: d.max_output_tokens,
                            supports_thinking: d.supports_thinking,
                            supports_tools: d.supports_tools,
                            is_default: d.is_default,
                        };
                        let resp_body =
                            crate::server::protocol::ResolveModelResponseBody { model: dto };

                        let response = if is_versioned {
                            serde_json::json!({
                                "version": "1.0",
                                "type": "Response",
                                "id": req_id_val,
                                "status": "success",
                                "body": resp_body
                            })
                        } else {
                            serde_json::json!({
                                "status": "ok",
                                "body": resp_body
                            })
                        };

                        let mut response_json = serde_json::to_string(&response)?;
                        response_json.push('\n');
                        let mut w = writer.lock().await;
                        w.write_all(response_json.as_bytes()).await?;
                        w.flush().await?;
                    }
                    Err(e) => {
                        let response = if is_versioned {
                            serde_json::json!({
                                "version": "1.0",
                                "type": "Error",
                                "id": req_id_val,
                                "status": "error",
                                "body": format!("Failed to resolve model: {}", e)
                            })
                        } else {
                            serde_json::json!({
                                "status": "error",
                                "message": format!("Failed to resolve model: {}", e)
                            })
                        };
                        let mut response_json = serde_json::to_string(&response)?;
                        response_json.push('\n');
                        let mut w = writer.lock().await;
                        w.write_all(response_json.as_bytes()).await?;
                        w.flush().await?;
                    }
                },
                Err(e) => {
                    let response = if is_versioned {
                        serde_json::json!({
                            "version": "1.0",
                            "type": "Error",
                            "id": req_id_val,
                            "status": "error",
                            "body": format!("Invalid model/resolve payload: {}", e)
                        })
                    } else {
                        serde_json::json!({
                            "status": "error",
                            "message": format!("Invalid model/resolve payload: {}", e)
                        })
                    };
                    let mut response_json = serde_json::to_string(&response)?;
                    response_json.push('\n');
                    let mut w = writer.lock().await;
                    w.write_all(response_json.as_bytes()).await?;
                    w.flush().await?;
                }
            }
            continue;
        }

        if action == "session/append_turn" || action == "v1/session/append_turn" {
            let append_req: Result<crate::server::protocol::AppendTurnPayload, _> =
                serde_json::from_str(payload);
            match append_req {
                Ok(req) => {
                    let now = req
                        .timestamp_ms
                        .unwrap_or_else(|| chrono::Utc::now().timestamp_millis());
                    let now_secs = (now / 1000) as u64;
                    let storage = app.runtime().sqlite_storage();
                    use brain_core::repositories::SessionRepository;

                    let parsed_msg_id = if let Some(ref tid) = req.turn_id {
                        if let Ok(id) = tid.parse::<brain_domain::MessageId>() {
                            id
                        } else {
                            use std::hash::{Hash, Hasher};
                            let mut hasher1 = std::collections::hash_map::DefaultHasher::new();
                            tid.hash(&mut hasher1);
                            let h1 = hasher1.finish();

                            let mut hasher2 = std::collections::hash_map::DefaultHasher::new();
                            (tid.as_str(), "turn_salt").hash(&mut hasher2);
                            let h2 = hasher2.finish();

                            let mut bytes = [0u8; 16];
                            bytes[0..8].copy_from_slice(&h1.to_be_bytes());
                            bytes[8..16].copy_from_slice(&h2.to_be_bytes());
                            brain_domain::MessageId(ulid::Ulid::from_bytes(bytes))
                        }
                    } else {
                        brain_domain::MessageId::new()
                    };
                    let msg_id = parsed_msg_id.to_string();

                    let parsed_sid =
                        require_canonical_session_id!(&req.session_id, is_versioned, req_id_val);
                    let mut session = storage
                        .load_session(&parsed_sid)
                        .ok()
                        .flatten()
                        .unwrap_or_else(|| {
                            brain_domain::Session::new(
                                parsed_sid,
                                brain_domain::SessionTitle("Session".to_string()),
                                brain_domain::SessionTimestamp(now_secs),
                            )
                        });

                    let role = match req.role.to_lowercase().as_str() {
                        "user" => brain_domain::MessageRole::User,
                        "assistant" => brain_domain::MessageRole::Assistant,
                        _ => brain_domain::MessageRole::System,
                    };

                    if let Some(existing) =
                        session.messages.iter_mut().find(|m| m.id == parsed_msg_id)
                    {
                        existing.content = req.content.clone();
                        existing.role = role;
                        existing.timestamp = now_secs;
                    } else {
                        let msg =
                            brain_domain::Message::new(parsed_msg_id, role, req.content.clone());
                        session.messages.push(msg);
                    }

                    session.updated_at = brain_domain::SessionTimestamp(now_secs);
                    session.autotitle();
                    let _ = storage.save_session(&parsed_sid, &session);

                    let resp_body = crate::server::protocol::AppendTurnResponseBody {
                        success: true,
                        message_id: msg_id,
                        session_id: req.session_id,
                    };

                    let response = if is_versioned {
                        serde_json::json!({
                            "version": "1.0",
                            "type": "Response",
                            "id": req_id_val,
                            "status": "success",
                            "body": resp_body
                        })
                    } else {
                        serde_json::json!({
                            "status": "ok",
                            "body": resp_body
                        })
                    };

                    let mut response_json = serde_json::to_string(&response)?;
                    response_json.push('\n');
                    let mut w = writer.lock().await;
                    w.write_all(response_json.as_bytes()).await?;
                    w.flush().await?;
                }
                Err(e) => {
                    let response = if is_versioned {
                        serde_json::json!({
                            "version": "1.0",
                            "type": "Error",
                            "id": req_id_val,
                            "status": "error",
                            "body": format!("Invalid append_turn payload: {}", e)
                        })
                    } else {
                        serde_json::json!({
                            "status": "error",
                            "message": format!("Invalid append_turn payload: {}", e)
                        })
                    };
                    let mut response_json = serde_json::to_string(&response)?;
                    response_json.push('\n');
                    let mut w = writer.lock().await;
                    w.write_all(response_json.as_bytes()).await?;
                    w.flush().await?;
                }
            }
            continue;
        }

        if action == "session/complete_turn" || action == "v1/session/complete_turn" {
            let comp_req: Result<crate::server::protocol::CompleteTurnPayload, _> =
                serde_json::from_str(payload);
            match comp_req {
                Ok(req) => {
                    let now = req
                        .timestamp_ms
                        .unwrap_or_else(|| chrono::Utc::now().timestamp_millis());
                    let now_secs = (now / 1000) as u64;
                    let storage = app.runtime().sqlite_storage();
                    use brain_core::repositories::SessionRepository;

                    let parsed_msg_id = if let Some(ref tid) = req.turn_id {
                        if let Ok(id) = tid.parse::<brain_domain::MessageId>() {
                            id
                        } else {
                            use std::hash::{Hash, Hasher};
                            let mut hasher1 = std::collections::hash_map::DefaultHasher::new();
                            tid.hash(&mut hasher1);
                            let h1 = hasher1.finish();

                            let mut hasher2 = std::collections::hash_map::DefaultHasher::new();
                            (tid.as_str(), "turn_salt").hash(&mut hasher2);
                            let h2 = hasher2.finish();

                            let mut bytes = [0u8; 16];
                            bytes[0..8].copy_from_slice(&h1.to_be_bytes());
                            bytes[8..16].copy_from_slice(&h2.to_be_bytes());
                            brain_domain::MessageId(ulid::Ulid::from_bytes(bytes))
                        }
                    } else {
                        brain_domain::MessageId::new()
                    };

                    let parsed_sid =
                        require_canonical_session_id!(&req.session_id, is_versioned, req_id_val);
                    let mut session = storage
                        .load_session(&parsed_sid)
                        .ok()
                        .flatten()
                        .unwrap_or_else(|| {
                            brain_domain::Session::new(
                                parsed_sid,
                                brain_domain::SessionTitle("Session".to_string()),
                                brain_domain::SessionTimestamp(now_secs),
                            )
                        });

                    if let Some(existing) =
                        session.messages.iter_mut().find(|m| m.id == parsed_msg_id)
                    {
                        existing.content = req.assistant_response.clone();
                        existing.role = brain_domain::MessageRole::Assistant;
                        existing.timestamp = now_secs;
                    } else {
                        let msg = brain_domain::Message::new(
                            parsed_msg_id,
                            brain_domain::MessageRole::Assistant,
                            req.assistant_response.clone(),
                        );
                        session.messages.push(msg);
                    }

                    session.updated_at = brain_domain::SessionTimestamp(now_secs);
                    let total_turns = session.messages.len();
                    let _ = storage.save_session(&parsed_sid, &session);

                    let resp_body = crate::server::protocol::CompleteTurnResponseBody {
                        success: true,
                        session_id: req.session_id,
                        total_turns,
                    };

                    let response = if is_versioned {
                        serde_json::json!({
                            "version": "1.0",
                            "type": "Response",
                            "id": req_id_val,
                            "status": "success",
                            "body": resp_body
                        })
                    } else {
                        serde_json::json!({
                            "status": "ok",
                            "body": resp_body
                        })
                    };

                    let mut response_json = serde_json::to_string(&response)?;
                    response_json.push('\n');
                    let mut w = writer.lock().await;
                    w.write_all(response_json.as_bytes()).await?;
                    w.flush().await?;
                }
                Err(e) => {
                    let response = if is_versioned {
                        serde_json::json!({
                            "version": "1.0",
                            "type": "Error",
                            "id": req_id_val,
                            "status": "error",
                            "body": format!("Invalid complete_turn payload: {}", e)
                        })
                    } else {
                        serde_json::json!({
                            "status": "error",
                            "message": format!("Invalid complete_turn payload: {}", e)
                        })
                    };
                    let mut response_json = serde_json::to_string(&response)?;
                    response_json.push('\n');
                    let mut w = writer.lock().await;
                    w.write_all(response_json.as_bytes()).await?;
                    w.flush().await?;
                }
            }
            continue;
        }

        if action == "memory/search" || action == "v1/memory/search" {
            let search_req: crate::server::protocol::SearchMemoryPayload = serde_json::from_str(
                payload,
            )
            .unwrap_or(crate::server::protocol::SearchMemoryPayload {
                session_id: None,
                query: payload.to_string(),
                workspace_path: None,
                limit: Some(10),
            });

            let query_text = search_req.query.clone();
            let limit = search_req.limit.unwrap_or(10);
            let mut matches = Vec::new();
            let context = ExecutionContext::default();
            let storage = app.runtime().sqlite_storage();
            if query_text.trim().is_empty() {
                // Blank queries mean "show me what's there": the retrieval
                // pipeline short-circuits blanks, so list stored concepts
                // directly (newest first) instead of returning a misleading
                // empty page.
                let mut nodes =
                    brain_core::repositories::NodeRepository::list_all(storage.as_ref())
                        .unwrap_or_default();
                nodes.retain(|n| matches!(n.node_type, brain_domain::NodeKind::Concept));
                nodes.sort_by(|a, b| {
                    b.updated_at
                        .cmp(&a.updated_at)
                        .then_with(|| a.label.cmp(&b.label))
                });
                for node in nodes.into_iter().take(limit) {
                    // Hoist helper results BEFORE moving node fields into the
                    // DTO — same borrow-order discipline as the typed arm.
                    let props = node.properties;
                    let excerpt =
                        super::memory_relations::preferred_excerpt(Some(&props), &node.label);
                    let scope = super::memory_relations::preferred_scope(Some(&props), "workspace");
                    let relations = super::memory_relations::materialize_relations(
                        Some(&node.id),
                        Some(&props),
                        None,
                        storage.as_ref(),
                    );
                    matches.push(crate::server::protocol::MemoryItemDto {
                        node_id: node.id.to_string(),
                        label: node.label,
                        excerpt,
                        score: 100,
                        channel: "knowledge_graph".to_string(),
                        timestamp: (node.updated_at * 1000) as i64,
                        scope,
                        relations,
                    });
                }
            } else {
                let search_query = brain_integrations::dto::v1::SearchQuery {
                    text: query_text.clone(),
                    kinds: None,
                    pagination: None,
                };

                let now = chrono::Utc::now().timestamp_millis();
                if let Ok(results) = app.search(search_query, &context).await {
                    for summary in results.into_iter().take(limit) {
                        let clean_title = if summary.title.trim().starts_with('{') {
                            if let Ok(v) =
                                serde_json::from_str::<serde_json::Value>(summary.title.trim())
                            {
                                v.get("content")
                                    .and_then(|c| c.as_str())
                                    .map(|s| s.to_string())
                                    .unwrap_or(summary.title.clone())
                            } else {
                                summary.title.clone()
                            }
                        } else {
                            summary.title.clone()
                        };

                        let score = summary
                            .metadata
                            .get("score")
                            .and_then(|s| s.parse::<i64>().ok())
                            .unwrap_or(100);

                        // Enrich from the stored node when the summary resolves
                        // to one: the application-layer projection drops node
                        // properties (SearchMetadata carries none), so relations/
                        // excerpt/scope are recovered here at the boundary.
                        // Sessions/messages fail the UUID parse and keep today's
                        // behavior exactly.
                        let node_id_opt = uuid::Uuid::parse_str(summary.id.trim())
                            .ok()
                            .map(brain_domain::NodeId);
                        let node_props = node_id_opt.as_ref().and_then(|nid| {
                            brain_core::repositories::NodeRepository::find_by_id(
                                storage.as_ref(),
                                nid,
                            )
                            .ok()
                            .flatten()
                            .map(|n| n.properties)
                        });
                        let relations = super::memory_relations::materialize_relations(
                            node_id_opt.as_ref(),
                            node_props.as_ref(),
                            summary.metadata.get("relations").map(|s| s.as_str()),
                            storage.as_ref(),
                        );
                        let excerpt = super::memory_relations::preferred_excerpt(
                            node_props.as_ref(),
                            &summary.body,
                        );
                        let scope = super::memory_relations::preferred_scope(
                            node_props.as_ref(),
                            "workspace",
                        );

                        matches.push(crate::server::protocol::MemoryItemDto {
                            node_id: summary.id,
                            label: clean_title,
                            excerpt,
                            score,
                            channel: "knowledge_graph".to_string(),
                            timestamp: now,
                            scope,
                            relations,
                        });
                    }
                }
            }

            let serialized_context = matches
                .iter()
                .map(|m| format!("[{}] {}", m.label, m.excerpt))
                .collect::<Vec<_>>()
                .join("\n");
            let count = matches.len();

            let resp_body = crate::server::protocol::SearchMemoryResponseBody {
                memories: matches,
                provenance: serde_json::json!({
                    "count": count,
                    "sources": ["brain-knowledge-graph"],
                    "channels": ["stm", "ltm"]
                }),
                token_count: serialized_context.len() / 4,
                serialized_context,
            };

            let response = if is_versioned {
                serde_json::json!({
                    "version": "1.0",
                    "type": "Response",
                    "id": req_id_val,
                    "status": "success",
                    "body": resp_body
                })
            } else {
                serde_json::json!({
                    "status": "ok",
                    "body": resp_body
                })
            };

            let mut response_json = serde_json::to_string(&response)?;
            response_json.push('\n');
            let mut w = writer.lock().await;
            w.write_all(response_json.as_bytes()).await?;
            w.flush().await?;
            continue;
        }

        if action == "memory/store" || action == "v1/memory/store" {
            let store_req: Result<crate::server::protocol::StoreMemoryPayload, _> =
                serde_json::from_str(payload);
            match store_req {
                Ok(req) => {
                    let node_id = brain_domain::NodeId::new();
                    let now = chrono::Utc::now().timestamp_millis();
                    let now_secs = (now / 1000) as u64;
                    let storage = app.runtime().sqlite_storage();
                    use brain_core::repositories::NodeRepository;
                    let mut props = std::collections::HashMap::new();
                    props.insert("content".to_string(), serde_json::json!(req.content));
                    props.insert(
                        "scope".to_string(),
                        serde_json::json!(req.scope.unwrap_or_else(|| "workspace".to_string())),
                    );
                    if let Some(ref rels) = req.relations {
                        props.insert("relations".to_string(), serde_json::json!(rels));
                    }
                    let node = brain_domain::Node::new(
                        node_id,
                        req.label.clone(),
                        brain_domain::NodeKind::Concept,
                    )
                    .with_properties(props)
                    .with_updated_at(now_secs);
                    let _ = storage.save(&node);

                    let resp_body = crate::server::protocol::StoreMemoryResponseBody {
                        success: true,
                        node_id: node_id.to_string(),
                    };

                    let response = if is_versioned {
                        serde_json::json!({
                            "version": "1.0",
                            "type": "Response",
                            "id": req_id_val,
                            "status": "success",
                            "body": resp_body
                        })
                    } else {
                        serde_json::json!({
                            "status": "ok",
                            "body": resp_body
                        })
                    };

                    let mut response_json = serde_json::to_string(&response)?;
                    response_json.push('\n');
                    let mut w = writer.lock().await;
                    w.write_all(response_json.as_bytes()).await?;
                    w.flush().await?;
                }
                Err(e) => {
                    let response = if is_versioned {
                        serde_json::json!({
                            "version": "1.0",
                            "type": "Error",
                            "id": req_id_val,
                            "status": "error",
                            "body": format!("Invalid memory/store payload: {}", e)
                        })
                    } else {
                        serde_json::json!({
                            "status": "error",
                            "message": format!("Invalid memory/store payload: {}", e)
                        })
                    };
                    let mut response_json = serde_json::to_string(&response)?;
                    response_json.push('\n');
                    let mut w = writer.lock().await;
                    w.write_all(response_json.as_bytes()).await?;
                    w.flush().await?;
                }
            }
            continue;
        }

        if action == "memory/delete" || action == "v1/memory/delete" {
            #[derive(serde::Deserialize)]
            struct DeleteMemoryPayload {
                #[serde(rename = "nodeId", alias = "node_id", alias = "id")]
                node_id: String,
            }

            match serde_json::from_str::<DeleteMemoryPayload>(payload) {
                Ok(req) => match app.delete_memory(&req.node_id).await {
                    Ok(()) => {
                        let resp_body = serde_json::json!({
                            "success": true,
                            "nodeId": req.node_id,
                        });
                        let response = if is_versioned {
                            serde_json::json!({
                                "version": "1.0",
                                "type": "Response",
                                "id": req_id_val,
                                "status": "success",
                                "body": serde_json::to_string(&resp_body)?
                            })
                        } else {
                            serde_json::json!({
                                "status": "ok",
                                "body": resp_body
                            })
                        };
                        let mut response_json = serde_json::to_string(&response)?;
                        response_json.push('\n');
                        let mut w = writer.lock().await;
                        w.write_all(response_json.as_bytes()).await?;
                        w.flush().await?;
                    }
                    Err(e) => {
                        let err_msg = e.to_string();
                        let response = if is_versioned {
                            serde_json::json!({
                                "version": "1.0",
                                "type": "Response",
                                "id": req_id_val,
                                "status": "error",
                                "body": err_msg
                            })
                        } else {
                            serde_json::json!({
                                "status": "error",
                                "error": err_msg
                            })
                        };
                        let mut response_json = serde_json::to_string(&response)?;
                        response_json.push('\n');
                        let mut w = writer.lock().await;
                        w.write_all(response_json.as_bytes()).await?;
                        w.flush().await?;
                    }
                },
                Err(e) => {
                    let err_msg = format!("Invalid memory/delete payload: {}", e);
                    let response = if is_versioned {
                        serde_json::json!({
                            "version": "1.0",
                            "type": "Response",
                            "id": req_id_val,
                            "status": "error",
                            "body": err_msg
                        })
                    } else {
                        serde_json::json!({
                            "status": "error",
                            "error": err_msg
                        })
                    };
                    let mut response_json = serde_json::to_string(&response)?;
                    response_json.push('\n');
                    let mut w = writer.lock().await;
                    w.write_all(response_json.as_bytes()).await?;
                    w.flush().await?;
                }
            }
            continue;
        }

        if action == "memory/link" || action == "v1/memory/link" {
            match serde_json::from_str::<crate::server::protocol::LinkMemoryPayload>(payload) {
                Ok(req) => {
                    let source_uuid = match uuid::Uuid::parse_str(req.source_node_id.trim()) {
                        Ok(u) => brain_domain::NodeId(u),
                        Err(e) => {
                            let err_msg = format!("Invalid source node ID: {}", e);
                            let response = if is_versioned {
                                serde_json::json!({
                                    "version": "1.0",
                                    "type": "Error",
                                    "id": req_id_val,
                                    "status": "error",
                                    "body": err_msg
                                })
                            } else {
                                serde_json::json!({ "status": "error", "error": err_msg })
                            };
                            let mut response_json = serde_json::to_string(&response)?;
                            response_json.push('\n');
                            let mut w = writer.lock().await;
                            w.write_all(response_json.as_bytes()).await?;
                            w.flush().await?;
                            continue;
                        }
                    };

                    let target_uuid = match uuid::Uuid::parse_str(req.target_node_id.trim()) {
                        Ok(u) => brain_domain::NodeId(u),
                        Err(e) => {
                            let err_msg = format!("Invalid target node ID: {}", e);
                            let response = if is_versioned {
                                serde_json::json!({
                                    "version": "1.0",
                                    "type": "Error",
                                    "id": req_id_val,
                                    "status": "error",
                                    "body": err_msg
                                })
                            } else {
                                serde_json::json!({ "status": "error", "error": err_msg })
                            };
                            let mut response_json = serde_json::to_string(&response)?;
                            response_json.push('\n');
                            let mut w = writer.lock().await;
                            w.write_all(response_json.as_bytes()).await?;
                            w.flush().await?;
                            continue;
                        }
                    };

                    let storage = app.runtime().sqlite_storage();
                    use brain_core::repositories::{EdgeRepository, NodeRepository};

                    // Server-side validation: verify both nodes exist
                    match NodeRepository::find_by_id(storage.as_ref(), &source_uuid) {
                        Ok(Some(_)) => {}
                        Ok(None) => {
                            let err_msg = format!("Source node '{}' not found", req.source_node_id);
                            let response = if is_versioned {
                                serde_json::json!({
                                    "version": "1.0",
                                    "type": "Error",
                                    "id": req_id_val,
                                    "status": "error",
                                    "body": err_msg
                                })
                            } else {
                                serde_json::json!({ "status": "error", "error": err_msg })
                            };
                            let mut response_json = serde_json::to_string(&response)?;
                            response_json.push('\n');
                            let mut w = writer.lock().await;
                            w.write_all(response_json.as_bytes()).await?;
                            w.flush().await?;
                            continue;
                        }
                        Err(e) => {
                            let err_msg = format!("Failed to lookup source node: {}", e);
                            let response = if is_versioned {
                                serde_json::json!({
                                    "version": "1.0",
                                    "type": "Error",
                                    "id": req_id_val,
                                    "status": "error",
                                    "body": err_msg
                                })
                            } else {
                                serde_json::json!({ "status": "error", "error": err_msg })
                            };
                            let mut response_json = serde_json::to_string(&response)?;
                            response_json.push('\n');
                            let mut w = writer.lock().await;
                            w.write_all(response_json.as_bytes()).await?;
                            w.flush().await?;
                            continue;
                        }
                    };

                    match NodeRepository::find_by_id(storage.as_ref(), &target_uuid) {
                        Ok(Some(_)) => {}
                        Ok(None) => {
                            let err_msg = format!("Target node '{}' not found", req.target_node_id);
                            let response = if is_versioned {
                                serde_json::json!({
                                    "version": "1.0",
                                    "type": "Error",
                                    "id": req_id_val,
                                    "status": "error",
                                    "body": err_msg
                                })
                            } else {
                                serde_json::json!({ "status": "error", "error": err_msg })
                            };
                            let mut response_json = serde_json::to_string(&response)?;
                            response_json.push('\n');
                            let mut w = writer.lock().await;
                            w.write_all(response_json.as_bytes()).await?;
                            w.flush().await?;
                            continue;
                        }
                        Err(e) => {
                            let err_msg = format!("Failed to lookup target node: {}", e);
                            let response = if is_versioned {
                                serde_json::json!({
                                    "version": "1.0",
                                    "type": "Error",
                                    "id": req_id_val,
                                    "status": "error",
                                    "body": err_msg
                                })
                            } else {
                                serde_json::json!({ "status": "error", "error": err_msg })
                            };
                            let mut response_json = serde_json::to_string(&response)?;
                            response_json.push('\n');
                            let mut w = writer.lock().await;
                            w.write_all(response_json.as_bytes()).await?;
                            w.flush().await?;
                            continue;
                        }
                    };

                    let relation_str = req
                        .relation
                        .as_deref()
                        .map(|s| s.trim())
                        .filter(|s| !s.is_empty())
                        .unwrap_or("related_to");
                    use std::str::FromStr;
                    let relation_kind = brain_domain::RelationKind::from_str(relation_str)
                        .unwrap_or(brain_domain::RelationKind::AssociatedWith);
                    let weight = req.weight.unwrap_or(1.0).clamp(0.0, 1.0);

                    let edge =
                        brain_domain::Edge::new(source_uuid, target_uuid, relation_kind, weight);

                    match EdgeRepository::save(storage.as_ref(), &edge) {
                        Ok(()) => {
                            let resp_body = crate::server::protocol::LinkMemoryResponseBody {
                                success: true,
                                source: source_uuid.to_string(),
                                target: target_uuid.to_string(),
                                relation: relation_kind.id().as_str().to_string(),
                            };
                            let response = if is_versioned {
                                serde_json::json!({
                                    "version": "1.0",
                                    "type": "Response",
                                    "id": req_id_val,
                                    "status": "success",
                                    "body": resp_body
                                })
                            } else {
                                serde_json::json!({
                                    "status": "ok",
                                    "body": resp_body
                                })
                            };
                            let mut response_json = serde_json::to_string(&response)?;
                            response_json.push('\n');
                            let mut w = writer.lock().await;
                            w.write_all(response_json.as_bytes()).await?;
                            w.flush().await?;
                        }
                        Err(e) => {
                            let err_msg = format!("Failed to save edge: {}", e);
                            let response = if is_versioned {
                                serde_json::json!({
                                    "version": "1.0",
                                    "type": "Error",
                                    "id": req_id_val,
                                    "status": "error",
                                    "body": err_msg
                                })
                            } else {
                                serde_json::json!({ "status": "error", "error": err_msg })
                            };
                            let mut response_json = serde_json::to_string(&response)?;
                            response_json.push('\n');
                            let mut w = writer.lock().await;
                            w.write_all(response_json.as_bytes()).await?;
                            w.flush().await?;
                        }
                    }
                }
                Err(e) => {
                    let err_msg = format!("Invalid memory/link payload: {}", e);
                    let response = if is_versioned {
                        serde_json::json!({
                            "version": "1.0",
                            "type": "Error",
                            "id": req_id_val,
                            "status": "error",
                            "body": err_msg
                        })
                    } else {
                        serde_json::json!({ "status": "error", "error": err_msg })
                    };
                    let mut response_json = serde_json::to_string(&response)?;
                    response_json.push('\n');
                    let mut w = writer.lock().await;
                    w.write_all(response_json.as_bytes()).await?;
                    w.flush().await?;
                }
            }
            continue;
        }

        if action == "memory/unlink" || action == "v1/memory/unlink" {
            match serde_json::from_str::<crate::server::protocol::UnlinkMemoryPayload>(payload) {
                Ok(req) => {
                    let source_uuid = match uuid::Uuid::parse_str(req.source_node_id.trim()) {
                        Ok(u) => brain_domain::NodeId(u),
                        Err(e) => {
                            let err_msg = format!("Invalid source node ID: {}", e);
                            let response = if is_versioned {
                                serde_json::json!({
                                    "version": "1.0",
                                    "type": "Error",
                                    "id": req_id_val,
                                    "status": "error",
                                    "body": err_msg
                                })
                            } else {
                                serde_json::json!({ "status": "error", "error": err_msg })
                            };
                            let mut response_json = serde_json::to_string(&response)?;
                            response_json.push('\n');
                            let mut w = writer.lock().await;
                            w.write_all(response_json.as_bytes()).await?;
                            w.flush().await?;
                            continue;
                        }
                    };

                    let target_uuid = match uuid::Uuid::parse_str(req.target_node_id.trim()) {
                        Ok(u) => brain_domain::NodeId(u),
                        Err(e) => {
                            let err_msg = format!("Invalid target node ID: {}", e);
                            let response = if is_versioned {
                                serde_json::json!({
                                    "version": "1.0",
                                    "type": "Error",
                                    "id": req_id_val,
                                    "status": "error",
                                    "body": err_msg
                                })
                            } else {
                                serde_json::json!({ "status": "error", "error": err_msg })
                            };
                            let mut response_json = serde_json::to_string(&response)?;
                            response_json.push('\n');
                            let mut w = writer.lock().await;
                            w.write_all(response_json.as_bytes()).await?;
                            w.flush().await?;
                            continue;
                        }
                    };

                    let relation_str = req
                        .relation
                        .as_deref()
                        .map(|s| s.trim())
                        .filter(|s| !s.is_empty())
                        .unwrap_or("related_to");
                    use std::str::FromStr;
                    let relation_kind = brain_domain::RelationKind::from_str(relation_str)
                        .unwrap_or(brain_domain::RelationKind::AssociatedWith);

                    let storage = app.runtime().sqlite_storage();
                    use brain_core::repositories::EdgeRepository;

                    let edge_id =
                        brain_domain::EdgeId::new(source_uuid, target_uuid, relation_kind.id());
                    match EdgeRepository::delete(storage.as_ref(), &edge_id) {
                        Ok(()) => {
                            let resp_body = crate::server::protocol::UnlinkMemoryResponseBody {
                                success: true,
                                source: source_uuid.to_string(),
                                target: target_uuid.to_string(),
                                relation: relation_kind.id().as_str().to_string(),
                            };
                            let response = if is_versioned {
                                serde_json::json!({
                                    "version": "1.0",
                                    "type": "Response",
                                    "id": req_id_val,
                                    "status": "success",
                                    "body": resp_body
                                })
                            } else {
                                serde_json::json!({
                                    "status": "ok",
                                    "body": resp_body
                                })
                            };
                            let mut response_json = serde_json::to_string(&response)?;
                            response_json.push('\n');
                            let mut w = writer.lock().await;
                            w.write_all(response_json.as_bytes()).await?;
                            w.flush().await?;
                        }
                        Err(e) => {
                            let err_msg = format!("Failed to delete edge: {}", e);
                            let response = if is_versioned {
                                serde_json::json!({
                                    "version": "1.0",
                                    "type": "Error",
                                    "id": req_id_val,
                                    "status": "error",
                                    "body": err_msg
                                })
                            } else {
                                serde_json::json!({ "status": "error", "error": err_msg })
                            };
                            let mut response_json = serde_json::to_string(&response)?;
                            response_json.push('\n');
                            let mut w = writer.lock().await;
                            w.write_all(response_json.as_bytes()).await?;
                            w.flush().await?;
                        }
                    }
                }
                Err(e) => {
                    let err_msg = format!("Invalid memory/unlink payload: {}", e);
                    let response = if is_versioned {
                        serde_json::json!({
                            "version": "1.0",
                            "type": "Error",
                            "id": req_id_val,
                            "status": "error",
                            "body": err_msg
                        })
                    } else {
                        serde_json::json!({ "status": "error", "error": err_msg })
                    };
                    let mut response_json = serde_json::to_string(&response)?;
                    response_json.push('\n');
                    let mut w = writer.lock().await;
                    w.write_all(response_json.as_bytes()).await?;
                    w.flush().await?;
                }
            }
            continue;
        }

        // ── Inc 45: Authoritative Daemon Diagnostics Telemetry (`v1/system/status`) ──
        if action == "system/status" || action == "v1/system/status" {
            let storage = app.runtime().sqlite_storage();
            let sessions_res = storage.list_sessions(10000, 0);
            let nodes_res = brain_core::repositories::NodeRepository::list_all(storage.as_ref());
            let edges_res = brain_core::repositories::EdgeRepository::list_all(storage.as_ref());

            let sqlite_healthy = sessions_res.is_ok() && nodes_res.is_ok() && edges_res.is_ok();
            let total_sessions = sessions_res.map(|s| s.len()).unwrap_or(0);
            let total_nodes = nodes_res.map(|n| n.len()).unwrap_or(0);
            let total_edges = edges_res.map(|e| e.len()).unwrap_or(0);

            let active_models_count = app.model_gateway().list_models().len();
            let uptime_seconds = app.runtime().status().uptime.as_secs();
            let status = if sqlite_healthy {
                "healthy"
            } else {
                "degraded"
            };

            let resp_body = crate::server::protocol::SystemStatusResponseBody {
                status: status.to_string(),
                version: env!("CARGO_PKG_VERSION").to_string(),
                uptime_seconds,
                sqlite_healthy,
                total_sessions,
                total_nodes,
                total_edges,
                active_models_count,
            };

            let response = if is_versioned {
                serde_json::json!({
                    "version": "1.0",
                    "type": "Response",
                    "id": req_id_val,
                    "status": "success",
                    "body": resp_body
                })
            } else {
                serde_json::json!({
                    "status": "ok",
                    "body": resp_body
                })
            };

            let mut response_json = serde_json::to_string(&response)?;
            response_json.push('\n');
            let mut w = writer.lock().await;
            w.write_all(response_json.as_bytes()).await?;
            w.flush().await?;
            continue;
        }

        if action == "search/query" || action == "v1/search/query" {
            let search_req: Result<crate::server::protocol::TranscriptSearchQueryPayload, _> =
                serde_json::from_str(payload);
            match search_req {
                Ok(req) => {
                    let query_str = req.query.trim().to_string();
                    if query_str.is_empty() {
                        let err_msg = "Search query must not be empty".to_string();
                        let response = if is_versioned {
                            serde_json::json!({
                                "version": "1.0",
                                "type": "Response",
                                "id": req_id_val,
                                "status": "error",
                                "body": err_msg
                            })
                        } else {
                            serde_json::json!({
                                "status": "error",
                                "error": err_msg
                            })
                        };
                        let mut response_json = serde_json::to_string(&response)?;
                        response_json.push('\n');
                        let mut w = writer.lock().await;
                        w.write_all(response_json.as_bytes()).await?;
                        w.flush().await?;
                        continue;
                    }

                    let kinds = req
                        .kinds
                        .unwrap_or_else(|| vec!["message".to_string(), "session".to_string()]);
                    let limit = req.limit.unwrap_or(20).clamp(1, 100);
                    let offset = req.offset.unwrap_or(0);

                    let search_query = brain_integrations::dto::v1::SearchQuery {
                        text: query_str,
                        kinds: Some(kinds),
                        pagination: Some(brain_integrations::dto::v1::PaginationSpec {
                            limit: Some(limit),
                            offset: Some(offset),
                        }),
                    };

                    let context = ExecutionContext::default();
                    match app.search(search_query, &context).await {
                        Ok(results) => {
                            let matches: Vec<crate::server::protocol::TranscriptSearchMatchDto> =
                                results
                                    .into_iter()
                                    .map(|s| {
                                        let session_id = s.metadata.get("session_id").cloned();
                                        let role = s.metadata.get("role").cloned();
                                        let score = s
                                            .metadata
                                            .get("score")
                                            .and_then(|v| v.parse::<i64>().ok());
                                        let snippet = if !s.body.is_empty() {
                                            Some(s.body.clone())
                                        } else {
                                            None
                                        };
                                        crate::server::protocol::TranscriptSearchMatchDto {
                                            id: s.id,
                                            kind: s.kind,
                                            session_id,
                                            title: s.title,
                                            body: s.body,
                                            snippet,
                                            role,
                                            score,
                                        }
                                    })
                                    .collect();

                            let total = matches.len();
                            let resp_body = crate::server::protocol::TranscriptSearchResponseBody {
                                matches,
                                total,
                            };

                            let response = if is_versioned {
                                serde_json::json!({
                                    "version": "1.0",
                                    "type": "Response",
                                    "id": req_id_val,
                                    "status": "success",
                                    "body": resp_body
                                })
                            } else {
                                serde_json::json!({
                                    "status": "ok",
                                    "body": resp_body
                                })
                            };

                            let mut response_json = serde_json::to_string(&response)?;
                            response_json.push('\n');
                            let mut w = writer.lock().await;
                            w.write_all(response_json.as_bytes()).await?;
                            w.flush().await?;
                        }
                        Err(e) => {
                            let err_msg = format!("Search failed: {}", e);
                            let response = if is_versioned {
                                serde_json::json!({
                                    "version": "1.0",
                                    "type": "Response",
                                    "id": req_id_val,
                                    "status": "error",
                                    "body": err_msg
                                })
                            } else {
                                serde_json::json!({
                                    "status": "error",
                                    "error": err_msg
                                })
                            };
                            let mut response_json = serde_json::to_string(&response)?;
                            response_json.push('\n');
                            let mut w = writer.lock().await;
                            w.write_all(response_json.as_bytes()).await?;
                            w.flush().await?;
                        }
                    }
                }
                Err(e) => {
                    let err_msg = format!("Invalid search/query payload: {}", e);
                    let response = if is_versioned {
                        serde_json::json!({
                            "version": "1.0",
                            "type": "Response",
                            "id": req_id_val,
                            "status": "error",
                            "body": err_msg
                        })
                    } else {
                        serde_json::json!({
                            "status": "error",
                            "error": err_msg
                        })
                    };
                    let mut response_json = serde_json::to_string(&response)?;
                    response_json.push('\n');
                    let mut w = writer.lock().await;
                    w.write_all(response_json.as_bytes()).await?;
                    w.flush().await?;
                }
            }
            continue;
        }

        if action == "tool/feedback" || action == "v1/tool/feedback" {
            let feedback_req: Result<crate::server::protocol::ToolExecutionFeedbackPayload, _> =
                serde_json::from_str(payload);
            match feedback_req {
                Ok(req) => {
                    let tool_use_id = req.identity.tool_use_id.clone();
                    let payload_hash = req.payload_hash.clone().unwrap_or_else(|| {
                        format!(
                            "{}:{}:{:?}:{:?}",
                            req.identity.session_id,
                            req.tool.name,
                            req.operation.path,
                            req.result.summary
                        )
                    });

                    let registry = get_feedback_registry();
                    let (is_duplicate, is_conflict) = {
                        let reg = registry.read().await;
                        if let Some(existing_hash) = reg.get(&tool_use_id) {
                            if existing_hash == &payload_hash {
                                (true, false)
                            } else {
                                (false, true)
                            }
                        } else {
                            (false, false)
                        }
                    };

                    if is_conflict {
                        let err_msg = format!(
                            "Conflicting tool feedback for existing toolUseId: {}",
                            tool_use_id
                        );
                        let response = if is_versioned {
                            serde_json::json!({
                                "version": "1.0",
                                "type": "Error",
                                "id": req_id_val,
                                "status": "error",
                                "body": err_msg
                            })
                        } else {
                            serde_json::json!({
                                "status": "error",
                                "message": err_msg
                            })
                        };
                        let mut response_json = serde_json::to_string(&response)?;
                        response_json.push('\n');
                        let mut w = writer.lock().await;
                        w.write_all(response_json.as_bytes()).await?;
                        w.flush().await?;
                        continue;
                    }

                    if is_duplicate {
                        let resp_body = crate::server::protocol::ToolFeedbackResponseBody {
                            success: true,
                            event_id: format!("fb_dup_{}", tool_use_id),
                            facts_ingested: 0,
                            entities_linked: vec![],
                            is_duplicate: true,
                        };
                        let response = if is_versioned {
                            serde_json::json!({
                                "version": "1.0",
                                "type": "Response",
                                "id": req_id_val,
                                "status": "success",
                                "body": resp_body
                            })
                        } else {
                            serde_json::json!({
                                "status": "ok",
                                "body": resp_body
                            })
                        };
                        let mut response_json = serde_json::to_string(&response)?;
                        response_json.push('\n');
                        let mut w = writer.lock().await;
                        w.write_all(response_json.as_bytes()).await?;
                        w.flush().await?;
                        continue;
                    }

                    // Ingest eligible knowledge facts
                    let storage = app.runtime().sqlite_storage();
                    use brain_core::repositories::NodeRepository;
                    let now = chrono::Utc::now().timestamp_millis();
                    let now_secs = (now / 1000) as u64;
                    let mut facts_ingested = 0;
                    let mut entities_linked = Vec::new();

                    let tool_name_lower = req.tool.name.to_lowercase();
                    if tool_name_lower == "write"
                        || tool_name_lower == "edit"
                        || tool_name_lower == "multiedit"
                        || tool_name_lower == "notebookedit"
                        || tool_name_lower == "enterworktree"
                    {
                        let raw_path = req
                            .operation
                            .path
                            .clone()
                            .unwrap_or_else(|| "modified_file".to_string());
                        let file_path = sanitize_path_containment(&raw_path);
                        let clean_content =
                            sanitize_security_string(&req.result.summary.unwrap_or_default());
                        let node_id = brain_domain::NodeId::new();
                        let mut props = std::collections::HashMap::new();
                        props.insert("content".to_string(), serde_json::json!(clean_content));
                        props.insert("scope".to_string(), serde_json::json!("workspace"));
                        props.insert("channel".to_string(), serde_json::json!("tool_feedback"));
                        props.insert("source".to_string(), serde_json::json!(req.tool.name));
                        props.insert("file_path".to_string(), serde_json::json!(file_path));

                        let label = format!("File: {}", file_path);
                        let node = brain_domain::Node::new(
                            node_id,
                            label,
                            brain_domain::NodeKind::Concept,
                        )
                        .with_properties(props)
                        .with_updated_at(now_secs);

                        let _ = storage.save(&node);
                        facts_ingested += 1;
                        entities_linked.push(node_id.to_string());
                    } else if tool_name_lower == "view" {
                        if let Some(ref raw_file_path) = req.operation.path {
                            let file_path = sanitize_path_containment(raw_file_path);
                            let node_id = brain_domain::NodeId::new();
                            let mut props = std::collections::HashMap::new();
                            props.insert(
                                "content".to_string(),
                                serde_json::json!(format!("Referenced file {}", file_path)),
                            );
                            props.insert("scope".to_string(), serde_json::json!("workspace"));
                            props.insert("channel".to_string(), serde_json::json!("tool_feedback"));
                            props.insert("source".to_string(), serde_json::json!("View"));
                            props.insert("file_path".to_string(), serde_json::json!(file_path));

                            let label = format!("Referenced: {}", file_path);
                            let node = brain_domain::Node::new(
                                node_id,
                                label,
                                brain_domain::NodeKind::Concept,
                            )
                            .with_properties(props)
                            .with_updated_at(now_secs);

                            let _ = storage.save(&node);
                            facts_ingested += 1;
                            entities_linked.push(node_id.to_string());
                        }
                    } else if tool_name_lower == "bash" && !req.result.is_error {
                        if let Some(ref cmd) = req.operation.command_name {
                            let node_id = brain_domain::NodeId::new();
                            let mut props = std::collections::HashMap::new();
                            props.insert(
                                "content".to_string(),
                                serde_json::json!(req.result.summary.unwrap_or_default()),
                            );
                            props.insert("scope".to_string(), serde_json::json!("workspace"));
                            props.insert("channel".to_string(), serde_json::json!("tool_feedback"));
                            props.insert("source".to_string(), serde_json::json!("Bash"));
                            props.insert("command".to_string(), serde_json::json!(cmd));

                            let label = format!("Command: {}", cmd);
                            let node = brain_domain::Node::new(
                                node_id,
                                label,
                                brain_domain::NodeKind::Concept,
                            )
                            .with_properties(props)
                            .with_updated_at(now_secs);

                            let _ = storage.save(&node);
                            facts_ingested += 1;
                            entities_linked.push(node_id.to_string());
                        }
                    }
                    // Ephemeral tools (Glob, Grep, ListDir) create 0 nodes

                    // Record in feedback registry
                    {
                        let mut reg = registry.write().await;
                        reg.insert(tool_use_id.clone(), payload_hash);
                    }

                    let event_id = format!("fb_evt_{}", uuid::Uuid::new_v4().simple());
                    let resp_body = crate::server::protocol::ToolFeedbackResponseBody {
                        success: true,
                        event_id,
                        facts_ingested,
                        entities_linked,
                        is_duplicate: false,
                    };

                    let response = if is_versioned {
                        serde_json::json!({
                            "version": "1.0",
                            "type": "Response",
                            "id": req_id_val,
                            "status": "success",
                            "body": resp_body
                        })
                    } else {
                        serde_json::json!({
                            "status": "ok",
                            "body": resp_body
                        })
                    };

                    let mut response_json = serde_json::to_string(&response)?;
                    response_json.push('\n');
                    let mut w = writer.lock().await;
                    w.write_all(response_json.as_bytes()).await?;
                    w.flush().await?;

                    // Debounced post-ingestion consolidation sweep (non-blocking, after socket flush)
                    if facts_ingested > 0 {
                        let now = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .map(|d| d.as_secs())
                            .unwrap_or(0);
                        let last =
                            get_last_debounce_sweep().load(std::sync::atomic::Ordering::Relaxed);
                        let debounce_window =
                            app.runtime().config().consolidation().debounce_secs();
                        if now.saturating_sub(last) >= debounce_window {
                            get_last_debounce_sweep()
                                .store(now, std::sync::atomic::Ordering::Relaxed);
                            let app_clone = app.clone();
                            tokio::spawn(async move {
                                let _guard = get_consolidation_lock().lock().await;
                                let policy = brain_domain::ConsolidationPolicy {
                                    promotion_weight_threshold: app_clone
                                        .runtime()
                                        .config()
                                        .consolidation()
                                        .promotion_weight_threshold(),
                                    pruning_weight_threshold: app_clone
                                        .runtime()
                                        .config()
                                        .consolidation()
                                        .pruning_weight_threshold(),
                                    staleness_age_threshold_secs: app_clone
                                        .runtime()
                                        .config()
                                        .consolidation()
                                        .staleness_age_threshold_secs(),
                                };
                                let storage = app_clone.runtime().sqlite_storage();
                                if let Err(e) = storage.consolidate_memories(policy) {
                                    tracing::debug!(
                                        "Background post-ingestion consolidation sweep failed: {:?}",
                                        e
                                    );
                                }
                            });
                        }
                    }
                }
                Err(e) => {
                    let response = if is_versioned {
                        serde_json::json!({
                            "version": "1.0",
                            "type": "Error",
                            "id": req_id_val,
                            "status": "error",
                            "body": format!("Invalid tool/feedback payload: {}", e)
                        })
                    } else {
                        serde_json::json!({
                            "status": "error",
                            "message": format!("Invalid tool/feedback payload: {}", e)
                        })
                    };
                    let mut response_json = serde_json::to_string(&response)?;
                    response_json.push('\n');
                    let mut w = writer.lock().await;
                    w.write_all(response_json.as_bytes()).await?;
                    w.flush().await?;
                }
            }
            continue;
        }

        if action == "v1/generation/cancel" || action == "cancel" {
            #[derive(serde::Deserialize)]
            struct CancelPayload {
                #[serde(rename = "generationId", alias = "generation_id", default)]
                generation_id: Option<String>,
                #[serde(rename = "sessionId", alias = "session_id", default)]
                session_id: Option<String>,
            }

            let cancel_req: CancelPayload =
                serde_json::from_str(payload).unwrap_or(CancelPayload {
                    generation_id: None,
                    session_id: None,
                });

            let registry = get_generation_registry();
            let active_generations: Vec<ActiveGeneration> = {
                let mut reg = registry.write().await;

                if let Some(ref gen_id) = cancel_req.generation_id {
                    reg.remove(gen_id).into_iter().collect()
                } else if let Some(ref s_id) = cancel_req.session_id {
                    let parsed_sid = require_canonical_session_id!(s_id, is_versioned, req_id_val);
                    let to_cancel: Vec<String> = reg
                        .iter()
                        .filter(|(_, active)| active.session_id == parsed_sid)
                        .map(|(k, _)| k.clone())
                        .collect();
                    to_cancel
                        .into_iter()
                        .filter_map(|generation_id| reg.remove(&generation_id))
                        .collect()
                } else {
                    reg.drain().map(|(_, active)| active).collect()
                }
            };

            // Serialize cancellation acknowledgement against tool-result
            // publication. If a result already holds the gate, it wins and is
            // emitted before this cancellation response; otherwise cancellation
            // wins and no later result can be published for that invocation.
            for active in active_generations {
                let _tool_result_guard = active.tool_result_gate.lock().await;
                active.cancellation_token.cancel();
            }

            let response = serde_json::json!({
                "type": "cancelled",
                "status": "ok"
            });
            let mut response_json = serde_json::to_string(&response)?;
            response_json.push('\n');
            let mut w = writer.lock().await;
            w.write_all(response_json.as_bytes()).await?;
            w.flush().await?;
            continue;
        }

        if action == "v1/tool/resolve" || action == "tool/resolve" {
            #[derive(serde::Deserialize)]
            struct ResolvePayload {
                #[serde(rename = "callId", alias = "call_id", default)]
                call_id: Option<String>,
                #[serde(
                    rename = "permissionRequestId",
                    alias = "permission_request_id",
                    default
                )]
                permission_request_id: Option<String>,
                #[serde(rename = "sessionId", alias = "session_id", default)]
                session_id: Option<String>,
                #[serde(rename = "generationId", alias = "generation_id", default)]
                generation_id: Option<String>,
                #[serde(rename = "toolName", alias = "tool_name", default)]
                tool_name: Option<String>,
                #[serde(default)]
                granted: bool,
            }

            let resolve_req: ResolvePayload =
                serde_json::from_str(payload).unwrap_or(ResolvePayload {
                    call_id: None,
                    permission_request_id: None,
                    session_id: None,
                    generation_id: None,
                    tool_name: None,
                    granted: false,
                });

            // A canonical resolution includes the exact Brain-issued session
            // ID. Validate it before inspecting or settling any waiter so an
            // invalid external string cannot bypass permission correlation.
            if let Some(session_id) = resolve_req.session_id.as_deref() {
                let _ = require_canonical_session_id!(session_id, is_versioned, req_id_val);
            }

            let pending_permission = {
                let mut waiters = get_permission_waiters().write().await;
                match (
                    resolve_req.permission_request_id.as_deref(),
                    resolve_req.call_id.as_deref(),
                    resolve_req.session_id.as_deref(),
                    resolve_req.generation_id.as_deref(),
                ) {
                    // Canonical clients must bind the decision to the exact
                    // daemon-issued permission request, tool call, session,
                    // and generation. An opaque request ID alone is not a
                    // substitute for stream correlation.
                    (
                        Some(permission_request_id),
                        Some(call_id),
                        Some(session_id),
                        Some(generation_id),
                    ) => match waiters.get(permission_request_id) {
                        Some(pending)
                            if pending.call_id == call_id
                                && pending.session_id == session_id
                                && pending.generation_id == generation_id
                                && resolve_req
                                    .tool_name
                                    .as_deref()
                                    .map(|tn| pending.tool_name == tn)
                                    .unwrap_or(true) =>
                        {
                            waiters.remove(permission_request_id)
                        }
                        _ => None,
                    },
                    // Preserve the legacy call-only resolver for the old shell
                    // while making ambiguity fail closed. Canonical frontend
                    // clients always use the branch above.
                    (None, Some(call_id), None, None) => {
                        let matching_ids: Vec<String> = waiters
                            .iter()
                            .filter(|(_, pending)| {
                                pending.call_id == call_id
                                    && resolve_req
                                        .tool_name
                                        .as_deref()
                                        .map(|tn| pending.tool_name == tn)
                                        .unwrap_or(true)
                            })
                            .map(|(permission_request_id, _)| permission_request_id.clone())
                            .collect();
                        if matching_ids.len() == 1 {
                            waiters.remove(&matching_ids[0])
                        } else {
                            None
                        }
                    }
                    _ => None,
                }
            };

            let outcome = match pending_permission {
                Some(pending) => pending.decision.send(resolve_req.granted).is_ok(),
                None => false,
            };

            let response = if outcome {
                serde_json::json!({ "type": "resolved", "status": "ok" })
            } else {
                serde_json::json!({
                    "type": "Error",
                    "status": "error",
                    "body": format!(
                        "Unknown, mismatched, or already-resolved tool permission request '{}'",
                        resolve_req
                            .permission_request_id
                            .or(resolve_req.call_id)
                            .unwrap_or_default()
                    )
                })
            };
            let mut response_json = serde_json::to_string(&response)?;
            response_json.push('\n');
            let mut w = writer.lock().await;
            w.write_all(response_json.as_bytes()).await?;
            w.flush().await?;
            continue;
        }

        if action == "v1/generation/stream" {
            let mut writer = writer.lock().await;
            metrics.total_queries.fetch_add(1, Ordering::Relaxed);

            #[derive(serde::Deserialize)]
            struct ThinkingConfigPayload {
                #[serde(default)]
                mode: Option<String>,
                #[serde(rename = "budgetTokens", alias = "budget_tokens", default)]
                budget_tokens: Option<usize>,
            }

            #[derive(serde::Deserialize)]
            struct StreamGenPayload {
                #[serde(rename = "sessionId", alias = "session_id")]
                session_id: Option<String>,
                #[serde(rename = "generationId", alias = "generation_id", default)]
                generation_id: Option<String>,
                #[serde(default)]
                messages: Vec<serde_json::Value>,
                #[serde(rename = "systemPrompt", alias = "system_prompt", default)]
                system_prompt: Option<String>,
                #[serde(default)]
                _tools: Vec<serde_json::Value>,
                #[serde(rename = "thinkingConfig", alias = "thinking_config", default)]
                thinking_config: Option<ThinkingConfigPayload>,
                #[serde(rename = "contextMaxTokens", alias = "context_max_tokens", default)]
                context_max_tokens: Option<usize>,
                #[serde(default)]
                model: Option<String>,
            }

            let stream_req: StreamGenPayload = match serde_json::from_str(payload) {
                Ok(p) => p,
                Err(e) => {
                    let err_frame = serde_json::json!({
                        "type": "error",
                        "error": format!("Invalid generation payload: {}", e),
                        "code": "invalid_payload",
                        "sequence": 0,
                        "status": "failed"
                    });
                    let mut json = serde_json::to_string(&err_frame)?;
                    json.push('\n');
                    writer.write_all(json.as_bytes()).await?;
                    writer.flush().await?;
                    continue;
                }
            };

            let generation_id = stream_req
                .generation_id
                .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

            let (session_id_str, parsed_session_id) = match stream_req.session_id.as_deref() {
                Some(raw) => match parse_canonical_session_id(raw) {
                    Ok(id) => (id.to_string(), id),
                    Err(error) => {
                        let err_frame = serde_json::json!({
                            "type": "error",
                            "generation_id": generation_id,
                            "session_id": raw,
                            "error": error.message(),
                            "code": error.code(),
                            "sequence": 0,
                            "status": "failed"
                        });
                        let mut json = serde_json::to_string(&err_frame)?;
                        json.push('\n');
                        writer.write_all(json.as_bytes()).await?;
                        writer.flush().await?;
                        continue;
                    }
                },
                None => {
                    let error = CanonicalSessionIdError::Missing;
                    let err_frame = serde_json::json!({
                        "type": "error",
                        "generation_id": generation_id,
                        "error": error.message(),
                        "code": error.code(),
                        "sequence": 0,
                        "status": "failed"
                    });
                    let mut json = serde_json::to_string(&err_frame)?;
                    json.push('\n');
                    writer.write_all(json.as_bytes()).await?;
                    writer.flush().await?;
                    continue;
                }
            };

            // Invariant 4: Validate session existence in storage. Sessions are
            // created explicitly (the workspace/session RFC lifecycle creates
            // the aggregate first, then streams turns into it), so a stream
            // naming a nonexistent id is rejected with session_not_found — it
            // must never fabricate an aggregate for a caller-supplied id.
            let storage = app.runtime().sqlite_storage();
            use brain_core::repositories::SessionRepository;
            let mut session_aggregate = match storage.load_session(&parsed_session_id) {
                Ok(Some(mut s)) => {
                    if let Some(ref sp) = stream_req.system_prompt {
                        let cleaned = sp.trim().to_string();
                        s.system_prompt = if cleaned.is_empty() {
                            None
                        } else {
                            Some(cleaned)
                        };
                    }
                    s
                }
                _ => {
                    let err_frame = serde_json::json!({
                        "type": "error",
                        "generation_id": generation_id,
                        "session_id": session_id_str,
                        "error": format!("Session '{}' not found", session_id_str),
                        "code": "session_not_found",
                        "sequence": 0,
                        "status": "failed"
                    });
                    let mut json = serde_json::to_string(&err_frame)?;
                    json.push('\n');
                    writer.write_all(json.as_bytes()).await?;
                    writer.flush().await?;
                    continue;
                }
            };

            // Invariant 8: Concurrency Check (At most one generation per session)
            let registry = get_generation_registry();
            {
                let reg = registry.read().await;
                if reg
                    .values()
                    .any(|active| active.session_id == parsed_session_id)
                {
                    let err_frame = serde_json::json!({
                        "type": "error",
                        "generation_id": generation_id,
                        "session_id": session_id_str,
                        "error": format!("Session '{}' is busy with an active generation", session_id_str),
                        "code": "session_busy",
                        "sequence": 0,
                        "status": "failed"
                    });
                    let mut json = serde_json::to_string(&err_frame)?;
                    json.push('\n');
                    writer.write_all(json.as_bytes()).await?;
                    writer.flush().await?;
                    continue;
                }
            }

            // Extract last user prompt
            let last_user_prompt = stream_req
                .messages
                .iter()
                .rev()
                .find_map(|m| {
                    if m.get("role").and_then(|r| r.as_str()) == Some("user") {
                        if let Some(content_str) = m.get("content").and_then(|c| c.as_str()) {
                            Some(content_str.to_string())
                        } else if let Some(blocks) = m.get("content").and_then(|c| c.as_array()) {
                            let text = blocks
                                .iter()
                                .filter_map(|b| {
                                    if b.get("type").and_then(|t| t.as_str()) == Some("text") {
                                        b.get("text").and_then(|t| t.as_str())
                                    } else {
                                        None
                                    }
                                })
                                .collect::<Vec<_>>()
                                .join("\n");
                            if !text.is_empty() {
                                Some(text)
                            } else {
                                None
                            }
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                })
                .unwrap_or_else(|| "Hello".to_string());

            // Invariant 4: Persist User Message upon acceptance
            let user_msg = brain_domain::Message::new(
                brain_domain::MessageId::new(),
                brain_domain::MessageRole::User,
                last_user_prompt.clone(),
            );
            let _ = session_aggregate.add_message(user_msg);
            session_aggregate.autotitle();
            let _ = storage.save_session(&parsed_session_id, &session_aggregate);

            // Register ActiveGeneration with CancellationToken child token
            let cancellation_token = conn_token.child_token();
            let tool_result_gate = Arc::new(tokio::sync::Mutex::new(()));
            {
                let mut reg = registry.write().await;
                reg.insert(
                    generation_id.clone(),
                    ActiveGeneration {
                        session_id: parsed_session_id,
                        cancellation_token: cancellation_token.clone(),
                        tool_result_gate: tool_result_gate.clone(),
                    },
                );
            }
            let gen_guard = GenerationGuard::new(generation_id.clone(), registry);

            // The persisted session is the authoritative conversation history.
            // The frontend supplies the current user turn solely so it can be
            // atomically appended above; it cannot replace historical context
            // with a divergent local transcript.
            let model_name = stream_req
                .model
                .unwrap_or_else(|| "brain-default".to_string());
            let mut model_messages: Vec<brain_core::model::ModelChatMessage> = session_aggregate
                .messages
                .iter()
                .filter_map(|message| {
                    let role = match message.role {
                        brain_domain::MessageRole::User => brain_core::model::ChatRole::User,
                        brain_domain::MessageRole::Assistant => {
                            brain_core::model::ChatRole::Assistant
                        }
                        brain_domain::MessageRole::System => brain_core::model::ChatRole::System,
                        // Thinking and tool transcript records remain durable
                        // history but are not model-chat messages in this
                        // basic text/thinking increment.
                        brain_domain::MessageRole::Thinking | brain_domain::MessageRole::Tool => {
                            return None
                        }
                    };
                    Some(brain_core::model::ModelChatMessage::text(
                        role,
                        &message.content,
                    ))
                })
                .collect();

            let gen_start_time = Instant::now();
            let trace_id = format!("tr_{}", uuid::Uuid::new_v4().simple());
            let gen_span = tracing::info_span!(
                "model_generation",
                trace_id = %trace_id,
                session_id = %parsed_session_id,
                generation_id = %generation_id,
                request_id = %req_id_val,
                model = %model_name,
            );
            let _gen_guard_span = gen_span.enter();

            // Phase 3: Memory Context Retrieval & Assembly
            let context_req = brain_core::context::ContextAssemblyRequest {
                prompt: last_user_prompt.clone(),
                session_id: parsed_session_id,
                workspace_id: None,
                max_tokens: stream_req.context_max_tokens.unwrap_or(2048),
                max_items: 10,
                reference_time: None,
            };
            let t_assemble_start = Instant::now();
            let context_snapshot = tokio::select! {
                biased;
                _ = cancellation_token.cancelled() => None,
                result = app.assemble_context(context_req) => match result {
                    Ok(snapshot) => Some(snapshot),
                    Err(error) => {
                        tracing::warn!(%error, "Memory context assembly failed; continuing generation");
                        None
                    }
                },
            };
            let assembly_latency_ms = t_assemble_start.elapsed().as_millis() as u64;

            // This is the only context/retrieval shape intended for a frontend
            // generation view model. It deliberately omits assembled content,
            // labels, ranking scores, and storage properties. `source_refs` are
            // opaque Brain references, not user-facing memory records.
            //
            // `retrieval_count` reports the opaque source references actually
            // included in this generation's context, rather than upstream
            // candidate count. It is independent of provider input-token
            // accounting below and must never be added to it.
            let frontend_context_metadata = context_snapshot.as_ref().map(|snapshot| {
                serde_json::json!({
                    "version": 1,
                    "retrieval_epoch_id": snapshot.epoch_id,
                    "retrieval_count": snapshot.provenance.count,
                    "retrieval_channels": snapshot.provenance.channels.iter()
                        .map(ToString::to_string)
                        .collect::<Vec<_>>(),
                    "context_token_count": snapshot.token_count,
                    "source_refs": snapshot.provenance.sources,
                })
            });

            let serialized_context = context_snapshot
                .as_ref()
                .map(|snapshot| snapshot.serialized_context.as_str())
                .filter(|context| !context.is_empty());
            let combined_system_prompt = if let Some(context) = serialized_context {
                match stream_req.system_prompt {
                    Some(sp) if !sp.trim().is_empty() => Some(format!("{context}\n\n{sp}")),
                    _ => Some(context.to_owned()),
                }
            } else {
                stream_req.system_prompt
            };

            // Authoritative single resolution via ModelGateway
            let resolved_model_desc = match app.model_gateway().resolve_model(Some(&model_name)) {
                Ok(desc) => desc,
                Err(e) => {
                    let err_packet = serde_json::json!({
                        "type": "error",
                        "generation_id": generation_id,
                        "session_id": session_id_str,
                        "sequence": 0,
                        "status": "failed",
                        "error": format!("Model resolution failed: {}", e)
                    });
                    let mut err_json = serde_json::to_string(&err_packet)?;
                    err_json.push('\n');
                    writer.write_all(err_json.as_bytes()).await?;
                    writer.flush().await?;
                    continue;
                }
            };

            let mut seq: u64 = 0;
            let mut thinking_started_at: Option<Instant> = None;

            let requested_thinking_budget = match stream_req.thinking_config {
                Some(ref cfg) => match cfg.mode.as_deref() {
                    Some("disabled") | Some("off") => None,
                    Some("enabled") | Some("adaptive") => cfg.budget_tokens.or(Some(2048)),
                    Some(_) => cfg.budget_tokens,
                    None => cfg.budget_tokens,
                },
                None => None,
            };

            let (thinking_supported, effective_thinking_budget) =
                if resolved_model_desc.supports_thinking {
                    let clamped = requested_thinking_budget
                        .map(|b| b.min(resolved_model_desc.max_output_tokens));
                    (true, clamped)
                } else {
                    (false, None)
                };

            // Frame 0: only the approved frontend context metadata contract.
            let mut start_metadata = serde_json::json!({
                "model": resolved_model_desc.id,
                "model_name": resolved_model_desc.name,
                "provider": resolved_model_desc.provider,
                "thinking_supported": thinking_supported,
                "effective_thinking_budget": effective_thinking_budget,
                "thinking_budget": effective_thinking_budget,
            });
            if let Some(context) = &frontend_context_metadata {
                start_metadata["brain_context"] = context.clone();
            }
            let start_packet = serde_json::json!({
                "type": "stream_start",
                "generation_id": generation_id,
                "session_id": session_id_str,
                "sequence": seq,
                "status": "in_progress",
                "metadata": start_metadata,
            });
            let mut start_json = serde_json::to_string(&start_packet)?;
            start_json.push('\n');
            writer.write_all(start_json.as_bytes()).await?;
            writer.flush().await?;

            let max_rounds =
                parse_max_rounds(std::env::var("BRAIN_TOOL_MAX_ROUNDS").ok().as_deref());
            let mut total_usage = brain_core::model::TokenUsage {
                input_tokens: 0,
                output_tokens: 0,
                ..Default::default()
            };
            let mut accumulated_response = String::new();
            let mut thinking_text = String::new();
            let mut is_completed_successfully = false;
            let mut is_cancelled = false;

            // Increment 7: advertise executable tools to tool-capable models.
            // Built once per turn; every loop pass re-sends the same set
            // (providers are stateless). supports_tools=false keeps today's
            // exact empty-vec request shape.
            let advertised_tools = if resolved_model_desc.supports_tools {
                crate::tools::advertised_definitions()
            } else {
                Vec::new()
            };

            // Increment 6: the agentic feedback loop. Each iteration drains
            // one provider pass; resolved tool calls (executed or denied)
            // feed back as assistant/user messages before the next pass.
            'rounds: for round_index in 0..max_rounds {
                if cancellation_token.is_cancelled() {
                    is_cancelled = true;
                    break 'rounds;
                }
                let gen_request = brain_core::model::GenerationRequest {
                    model: resolved_model_desc.id.clone(),
                    messages: model_messages.clone(),
                    system_prompt: combined_system_prompt.clone(),
                    tools: advertised_tools.clone(),
                    thinking_budget: effective_thinking_budget,
                };
                let mut stream_result = app
                    .model_gateway()
                    .stream_generation(gen_request, cancellation_token.clone())
                    .await;

                let mut pass_calls: Vec<PassToolUse> = Vec::new();
                let mut feedback: Vec<ToolFeedback> = Vec::new();
                let mut pass_text = String::new();
                let mut round_completed: Option<(String, brain_core::model::TokenUsage)> = None;

                match stream_result {
                    Ok(ref mut stream) => loop {
                        tokio::select! {
                            _ = cancellation_token.cancelled() => {
                                is_cancelled = true;
                                break;
                            }
                            chunk_opt = stream.next() => {
                                match chunk_opt {
                                    Some(Ok(chunk)) => {
                                        seq += 1;
                                        match chunk {
                                            brain_core::model::GenerationChunk::ThinkingStart => {
                                                thinking_started_at = Some(Instant::now());
                                                thinking_text.clear();
                                                let packet = serde_json::json!({
                                                    "type": "thinking_start",
                                                    "generation_id": generation_id,
                                                    "session_id": session_id_str,
                                                    "sequence": seq,
                                                    "status": "in_progress"
                                                });
                                                let mut j = serde_json::to_string(&packet)?;
                                                j.push('\n');
                                                writer.write_all(j.as_bytes()).await?;
                                                writer.flush().await?;
                                            }
                                            brain_core::model::GenerationChunk::ThinkingDelta { text } => {
                                                thinking_text.push_str(&text);
                                                let packet = serde_json::json!({
                                                    "type": "thinking_delta",
                                                    "generation_id": generation_id,
                                                    "session_id": session_id_str,
                                                    "sequence": seq,
                                                    "thinking": text,
                                                    "text": text,
                                                    "status": "in_progress"
                                                });
                                                let mut j = serde_json::to_string(&packet)?;
                                                j.push('\n');
                                                writer.write_all(j.as_bytes()).await?;
                                                writer.flush().await?;
                                            }
                                            brain_core::model::GenerationChunk::ThinkingEnd => {
                                                // Daemon-measured thinking duration: the
                                                // shell renders "Thought for X.Xs" from it.
                                                let measured_ms = thinking_started_at
                                                    .map(|t| t.elapsed().as_millis() as u64)
                                                    .unwrap_or(0);
                                                let packet = serde_json::json!({
                                                    "type": "thinking_end",
                                                    "duration_ms": measured_ms,
                                                    "generation_id": generation_id,
                                                    "session_id": session_id_str,
                                                    "sequence": seq,
                                                    "status": "in_progress"
                                                });
                                                let mut j = serde_json::to_string(&packet)?;
                                                j.push('\n');
                                                writer.write_all(j.as_bytes()).await?;
                                                writer.flush().await?;
                                                // Inc 19: persist the completed segment as a
                                                // transcript message (best-effort, exactly like
                                                // tool events — persistence never blocks
                                                // generation).
                                                if !thinking_text.is_empty() {
                                                    let envelope = serde_json::json!({
                                                        "type": "thinking_block",
                                                        "v": 1,
                                                        "text": thinking_text,
                                                        "duration_ms": measured_ms
                                                    });
                                                    let _ = session_aggregate.add_message(
                                                        brain_domain::Message::new(
                                                            brain_domain::MessageId::new(),
                                                            brain_domain::MessageRole::Thinking,
                                                            envelope.to_string(),
                                                        ),
                                                    );
                                                    if let Err(e) = storage.save_session(
                                                        &parsed_session_id,
                                                        &session_aggregate,
                                                    ) {
                                                        tracing::warn!(
                                                            error = %e,
                                                            "thinking block persistence failed; continuing"
                                                        );
                                                    }
                                                    thinking_text = String::new();
                                                }
                                            }
                                            brain_core::model::GenerationChunk::TextDelta { text } => {
                                                accumulated_response.push_str(&text);
                                                pass_text.push_str(&text);
                                                let packet = serde_json::json!({
                                                    "type": "token",
                                                    "generation_id": generation_id,
                                                    "session_id": session_id_str,
                                                    "sequence": seq,
                                                    "token": text,
                                                    "text": text,
                                                    "status": "in_progress"
                                                });
                                                let mut j = serde_json::to_string(&packet)?;
                                                j.push('\n');
                                                writer.write_all(j.as_bytes()).await?;
                                                writer.flush().await?;
                                            }
                                            brain_core::model::GenerationChunk::ToolUse { id, name, input } => {
                                                let call_id = id.clone();
                                                let tool_name = name.clone();
                                                pass_calls.push(PassToolUse {
                                                    call_id: call_id.clone(),
                                                    name: tool_name.clone(),
                                                    input: input.clone(),
                                                });

                                                // Forward the tool call itself first.
                                                let packet = serde_json::json!({
                                                    "type": "tool_use",
                                                    "generation_id": generation_id,
                                                    "session_id": session_id_str,
                                                    "sequence": seq,
                                                    "toolUse": {
                                                        "id": id,
                                                        "name": name,
                                                        "input": input
                                                    },
                                                    "status": "in_progress"
                                                });
                                                let mut j = serde_json::to_string(&packet)?;
                                                j.push('\n');
                                                writer.write_all(j.as_bytes()).await?;
                                                writer.flush().await?;

                                                // Permission gate: publish the request, then park
                                                // the stream until v1/tool/resolve delivers a verdict
                                                // (on ANY connection) or the timeout denies by default.
                                                seq += 1;
                                                let permission_request_id = format!(
                                                    "perm_{}",
                                                    uuid::Uuid::new_v4().simple()
                                                );
                                                let (tx, rx) = tokio::sync::oneshot::channel::<bool>();
                                                // Register before exposing the request frame so an
                                                // eager canonical frontend cannot resolve a valid
                                                // permission before its daemon-side waiter exists.
                                                get_permission_waiters()
                                                    .write()
                                                    .await
                                                    .insert(
                                                        permission_request_id.clone(),
                                                        PendingToolPermission {
                                                            call_id: call_id.clone(),
                                                            session_id: session_id_str.clone(),
                                                            generation_id: generation_id.clone(),
                                                            tool_name: tool_name.clone(),
                                                            decision: tx,
                                                        },
                                                    );
                                                let working_dir = std::env::current_dir()
                                                    .unwrap_or_else(|_| std::path::PathBuf::from("."));
                                                let security_eval = crate::tools::PathGuard::evaluate_tool_input(
                                                    &tool_name,
                                                    &packet["toolUse"]["input"],
                                                    &working_dir,
                                                );
                                                let reason = if let Some(ref warn) = security_eval.warning_message {
                                                    format!("Security warning: {warn}")
                                                } else {
                                                    "tool execution requires approval".to_string()
                                                };
                                                let perm_packet = serde_json::json!({
                                                    "type": "tool_permission_requested",
                                                    "generation_id": generation_id,
                                                    "session_id": session_id_str,
                                                    "sequence": seq,
                                                    "permission_request_id": permission_request_id,
                                                    "call_id": call_id,
                                                    "tool_name": tool_name,
                                                    "input": packet["toolUse"]["input"],
                                                    "reason": reason,
                                                    "security_context": {
                                                        "is_outside_workspace": security_eval.is_outside_workspace,
                                                        "is_sensitive_credential": security_eval.is_sensitive_credential,
                                                        "warning_message": security_eval.warning_message,
                                                    },
                                                    "status": "in_progress"
                                                });
                                                let mut pj = serde_json::to_string(&perm_packet)?;
                                                pj.push('\n');
                                                let sent = async {
                                                    writer.write_all(pj.as_bytes()).await?;
                                                    writer.flush().await
                                                }.await;
                                                if let Err(error) = sent {
                                                    get_permission_waiters().write().await.remove(&permission_request_id);
                                                    return Err(error.into());
                                                }

                                                let timeout_secs = std::env::var(
                                                    "BRAIN_TOOL_PERMISSION_TIMEOUT_SECS",
                                                )
                                                .ok()
                                                .and_then(|v| v.parse::<u64>().ok())
                                                .unwrap_or(300);
                                                let granted = tokio::select! {
                                                    _ = cancellation_token.cancelled() => false,
                                                    verdict = tokio::time::timeout(
                                                        std::time::Duration::from_secs(timeout_secs),
                                                        rx,
                                                    ) => verdict.ok().and_then(|r| r.ok()).unwrap_or(false),
                                                };
                                                get_permission_waiters()
                                                    .write()
                                                    .await
                                                    .remove(&permission_request_id);

                                                if cancellation_token.is_cancelled() {
                                                    is_cancelled = true;
                                                    break 'rounds;
                                                }

                                                if granted {
                                                    // Inc 5: the wire verdict is the
                                                    // executor-side authority; execute and
                                                    // report one tool_result frame. Aliased
                                                    // import: brain_application's
                                                    // ExecutionContext is already in scope.
                                                    use brain_core::extensibility::{
                                                        ExecutionContext as ToolExecutionContext,
                                                        ToolRegistry,
                                                    };
                                                    let stack = crate::tools::tool_stack();
                                                    let mut args_map: HashMap<String, serde_json::Value> =
                                                        HashMap::new();
                                                    if let Some(obj) =
                                                        packet["toolUse"]["input"].as_object()
                                                    {
                                                        for (k, v) in obj {
                                                            args_map.insert(k.clone(), v.clone());
                                                        }
                                                    }
                                                    let tool_ctx = ToolExecutionContext {
                                                        session_id: parsed_session_id,
                                                        working_dir: std::env::current_dir()
                                                            .unwrap_or_else(|_| {
                                                                std::path::PathBuf::from(".")
                                                            }),
                                                        cancellation: Arc::new(
                                                            GenerationToolCancellation(
                                                                cancellation_token.clone(),
                                                            ),
                                                        ),
                                                        deadline: None,
                                                    };
                                                    let tool_exec_started = std::time::Instant::now();
                                                    let execution =
                                                        match stack.registry.get_tool(&tool_name) {
                                                            Some(tool) => {
                                                                // Scope permission authorization strictly to this approved invocation.
                                                                // The shared stack.permissions remains pristine and unmodified.
                                                                let invocation_permissions = stack
                                                                    .permissions
                                                                    .scoped_with(
                                                                        tool.metadata()
                                                                            .required_permissions
                                                                            .iter()
                                                                            .copied(),
                                                                    );
                                                                stack
                                                                    .executor
                                                                    .execute(
                                                                        tool,
                                                                        &tool_ctx,
                                                                        &invocation_permissions,
                                                                        &args_map,
                                                                    )
                                                                    .await
                                                            }
                                                            None => Err(
                                                                brain_core::errors::BrainError::Internal {
                                                                    message: format!(
                                                                        "Unknown tool '{tool_name}'"
                                                                    ),
                                                                },
                                                            ),
                                                        };
                                                    let _tool_result_guard =
                                                        tool_result_gate.lock().await;
                                                    if cancellation_token.is_cancelled() {
                                                        is_cancelled = true;
                                                        break 'rounds;
                                                    }
                                                    // Reserve a wire sequence only when a result
                                                    // will actually be emitted. Cancellation can
                                                    // win while the executor is waiting; burning a
                                                    // result slot there would make the final
                                                    // `finished` frame appear to have a gap.
                                                    seq += 1;
                                                    let (out_text, is_err, exit_code, outcome) = match execution {
                                                        Ok(result) => {
                                                            let v = result.value().clone();
                                                            let is_error = v["is_error"]
                                                                .as_bool()
                                                                .unwrap_or(true);
                                                            (
                                                                v["output"]
                                                                    .as_str()
                                                                    .unwrap_or("")
                                                                    .to_string(),
                                                                is_error,
                                                                v["exit_code"].as_i64().unwrap_or(-1),
                                                                if is_error { "error" } else { "success" },
                                                            )
                                                        }
                                                        Err(brain_core::errors::BrainError::Timeout {
                                                            message,
                                                            ..
                                                        }) => (message, true, -1, "timed_out"),
                                                        Err(brain_core::errors::BrainError::Cancelled {
                                                            message,
                                                        }) => (message, true, -1, "cancelled"),
                                                        Err(e) => (format!("{e}"), true, -1, "error"),
                                                    };
                                                    feedback.push(ToolFeedback {
                                                        call_id: call_id.clone(),
                                                        output: out_text.clone(),
                                                        is_error: is_err,
                                                    });
                                                    // Inc 8: persist this outcome into
                                                    // the session transcript. Best-effort:
                                                    // persistence never blocks generation.
                                                    let envelope = serde_json::json!({
                                                        "type": "tool_event",
                                                        "v": 1,
                                                        "call_id": call_id.clone(),
                                                        "name": tool_name.clone(),
                                                        "input": packet["toolUse"]["input"].clone(),
                                                        "outcome": outcome,
                                                        "is_error": is_err,
                                                        "exit_code": exit_code,
                                                        "output": truncate_tool_output(&out_text),
                                                        "duration_ms": tool_exec_started.elapsed().as_millis() as u64,
                                                    });
                                                    let result_packet = serde_json::json!({
                                                        "type": "tool_result",
                                                        "generation_id": generation_id,
                                                        "session_id": session_id_str,
                                                        "sequence": seq,
                                                        "permission_request_id": permission_request_id,
                                                        "call_id": call_id,
                                                        "tool_name": tool_name,
                                                        "output": out_text,
                                                        "is_error": is_err,
                                                        "exit_code": exit_code,
                                                        "outcome": outcome,
                                                        // Inc 10: same clock as the persisted
                                                        // envelope, so live and replayed cards
                                                        // agree on duration.
                                                        "duration_ms": tool_exec_started.elapsed().as_millis() as u64,
                                                        "status": "in_progress"
                                                    });
                                                    let mut rj = serde_json::to_string(&result_packet)?;
                                                    rj.push('\n');
                                                    writer.write_all(rj.as_bytes()).await?;
                                                    writer.flush().await?;
                                                    let _ = session_aggregate.add_message(
                                                        brain_domain::Message::new(
                                                            brain_domain::MessageId::new(),
                                                            brain_domain::MessageRole::Tool,
                                                            envelope.to_string(),
                                                        ),
                                                    );
                                                    if let Err(e) = storage.save_session(
                                                        &parsed_session_id,
                                                        &session_aggregate,
                                                    ) {
                                                        tracing::warn!(
                                                            error = %e,
                                                            "tool event persistence failed; continuing"
                                                        );
                                                    }
                                                }

                                            if !granted {
                                                let _tool_result_guard = tool_result_gate.lock().await;
                                                if cancellation_token.is_cancelled() {
                                                    is_cancelled = true;
                                                    break 'rounds;
                                                }
                                                seq += 1;
                                                    let denied_packet = serde_json::json!({
                                                        "type": "tool_denied",
                                                        "generation_id": generation_id,
                                                        "session_id": session_id_str,
                                                        "sequence": seq,
                                                        "permission_request_id": permission_request_id,
                                                        "call_id": call_id,
                                                        "tool_name": tool_name,
                                                        "outcome": "denied",
                                                        "status": "in_progress"
                                                    });
                                                    let mut dj = serde_json::to_string(&denied_packet)?;
                                                    dj.push('\n');
                                                    writer.write_all(dj.as_bytes()).await?;
                                                    writer.flush().await?;
                                                    feedback.push(ToolFeedback {
                                                        call_id: call_id.clone(),
                                                        output: DENIED_FEEDBACK_TEXT.to_string(),
                                                        is_error: true,
                                                    });
                                                    // Inc 8: refusals are part of the
                                                    // honest transcript too.
                                                    let envelope = serde_json::json!({
                                                        "type": "tool_event",
                                                        "v": 1,
                                                        "call_id": call_id.clone(),
                                                        "name": tool_name.clone(),
                                                        "input": packet["toolUse"]["input"].clone(),
                                                        "outcome": "denied",
                                                        "is_error": true,
                                                        "exit_code": -1,
                                                        "output": DENIED_FEEDBACK_TEXT,
                                                        "duration_ms": 0,
                                                    });
                                                    let _ = session_aggregate.add_message(
                                                        brain_domain::Message::new(
                                                            brain_domain::MessageId::new(),
                                                            brain_domain::MessageRole::Tool,
                                                            envelope.to_string(),
                                                        ),
                                                    );
                                                    if let Err(e) = storage.save_session(
                                                        &parsed_session_id,
                                                        &session_aggregate,
                                                    ) {
                                                        tracing::warn!(
                                                            error = %e,
                                                            "tool event persistence failed; continuing"
                                                        );
                                                    }
                                                }
                                            }
                                            brain_core::model::GenerationChunk::Completed { finish_reason, usage } => {
                                                // Silent on the wire: the loop decides
                                                // whether this pass continues or emits
                                                // stream_end (spec §4.4). Restore the
                                                // sequence slot the shared chunk-entry
                                                // increment consumed — a burned slot here
                                                // would open a wire gap and abort the
                                                // shell's stream guard.
                                                seq -= 1;
                                                round_completed = Some((finish_reason, usage));
                                                break;
                                            }
                                        }
                                    }
                                    Some(Err(err)) => {
                                        seq += 1;
                                        let err_packet = serde_json::json!({
                                            "type": "error",
                                            "generation_id": generation_id,
                                            "session_id": session_id_str,
                                            "sequence": seq,
                                            "status": "failed",
                                            "error": err.to_string()
                                        });
                                        let mut err_json = serde_json::to_string(&err_packet)?;
                                        err_json.push('\n');
                                        writer.write_all(err_json.as_bytes()).await?;
                                        writer.flush().await?;
                                        break 'rounds;
                                    }
                                    None => {
                                        is_completed_successfully = true;
                                        break 'rounds;
                                    }
                                }
                            }
                        }
                    },
                    Err(err) => {
                        seq += 1;
                        let err_packet = serde_json::json!({
                            "type": "error",
                            "generation_id": generation_id,
                            "session_id": session_id_str,
                            "sequence": seq,
                            "status": "failed",
                            "error": err.to_string()
                        });
                        let mut err_json = serde_json::to_string(&err_packet)?;
                        err_json.push('\n');
                        writer.write_all(err_json.as_bytes()).await?;
                        writer.flush().await?;
                    }
                }

                is_cancelled |= cancellation_token.is_cancelled();
                if is_cancelled || round_completed.is_none() {
                    break 'rounds;
                }

                let (finish_reason, usage) = round_completed.take().unwrap();
                total_usage.input_tokens += usage.input_tokens;
                total_usage.output_tokens += usage.output_tokens;
                if let Some(r) = usage.cache_read_input_tokens {
                    *total_usage.cache_read_input_tokens.get_or_insert(0) += r;
                }
                if let Some(c) = usage.cache_creation_input_tokens {
                    *total_usage.cache_creation_input_tokens.get_or_insert(0) += c;
                }

                // Continuation rule (spec §3): keep going only when this pass
                // resolved at least one tool call AND rounds remain; otherwise
                // emit the sole terminating stream_end.
                let terminate_reason: Option<String> = if feedback.is_empty() {
                    Some(finish_reason)
                } else if round_index + 1 >= max_rounds {
                    Some("max_tool_rounds".to_string())
                } else {
                    model_messages.extend(feedback_messages(&pass_text, &pass_calls, &feedback));
                    None
                };

                if let Some(reason) = terminate_reason {
                    is_completed_successfully = true;
                    // Terminal stream_end owns a fresh sequence slot (the legacy
                    // strict-monotonic contract); mid-loop Completions restored
                    // theirs above.
                    seq += 1;
                    let mut metadata = serde_json::json!({
                        "inputTokens": total_usage.input_tokens,
                        "outputTokens": total_usage.output_tokens,
                    });
                    if let Some(tokens) = total_usage.cache_read_input_tokens {
                        metadata["cache_read_input_tokens"] = serde_json::json!(tokens);
                        metadata["cacheReadInputTokens"] = serde_json::json!(tokens);
                    }
                    if let Some(tokens) = total_usage.cache_creation_input_tokens {
                        metadata["cache_creation_input_tokens"] = serde_json::json!(tokens);
                        metadata["cacheCreationInputTokens"] = serde_json::json!(tokens);
                    }
                    let mut telemetry = serde_json::json!({
                        "generation_id": generation_id,
                        "session_id": session_id_str,
                        "total_duration_ms": gen_start_time.elapsed().as_millis() as u64,
                        "finish_reason": reason,
                    });
                    if let Some(snapshot) = &context_snapshot {
                        telemetry["retrieval_epoch_id"] = serde_json::json!(snapshot.epoch_id);
                        telemetry["candidates_retrieved"] =
                            serde_json::json!(snapshot.provenance.count);
                        telemetry["memories_assembled"] = serde_json::json!(snapshot.items.len());
                        telemetry["context_tokens_used"] = serde_json::json!(snapshot.token_count);
                        telemetry["assembly_latency_ms"] = serde_json::json!(assembly_latency_ms);
                    }
                    if let Some(context) = &frontend_context_metadata {
                        metadata["brain_context"] = context.clone();
                    }
                    metadata["telemetry"] = telemetry;
                    let end_packet = serde_json::json!({
                        "type": "stream_end",
                        "generation_id": generation_id,
                        "session_id": session_id_str,
                        "sequence": seq,
                        "status": "completed",
                        "response": accumulated_response,
                        "finish_reason": reason,
                        "metadata": metadata,
                    });
                    let mut end_json = serde_json::to_string(&end_packet)?;
                    end_json.push('\n');
                    writer.write_all(end_json.as_bytes()).await?;
                    writer.flush().await?;
                    break 'rounds;
                }
            } // 'rounds

            // Invariant 4: Persist assistant message ONLY on successful completion
            if is_completed_successfully && !is_cancelled && !accumulated_response.is_empty() {
                let assistant_msg = brain_domain::Message::new(
                    brain_domain::MessageId::new(),
                    brain_domain::MessageRole::Assistant,
                    accumulated_response.clone(),
                );
                let _ = session_aggregate.add_message(assistant_msg);
                let _ = storage.save_session(&parsed_session_id, &session_aggregate);
            }

            // Invariant 3: Exactly one terminal event
            seq += 1;
            let terminal_packet = if is_cancelled {
                serde_json::json!({
                    "type": "finished",
                    "generation_id": generation_id,
                    "session_id": session_id_str,
                    "sequence": seq,
                    "status": "cancelled"
                })
            } else if is_completed_successfully {
                serde_json::json!({
                    "type": "finished",
                    "generation_id": generation_id,
                    "session_id": session_id_str,
                    "sequence": seq,
                    "status": "completed"
                })
            } else {
                serde_json::json!({
                    "type": "finished",
                    "generation_id": generation_id,
                    "session_id": session_id_str,
                    "sequence": seq,
                    "status": "failed"
                })
            };

            let mut term_json = serde_json::to_string(&terminal_packet)?;
            term_json.push('\n');
            writer.write_all(term_json.as_bytes()).await?;
            writer.flush().await?;

            // Cleanup ActiveGeneration
            gen_guard.defuse().await;

            continue;
        }

        // For the TUI typewriter compatibility, we continue to run the legacy query stream logic
        // in the UDS transport if action is legacy "query"
        if action == "query" {
            let mut writer = writer.lock().await;
            metrics.total_queries.fetch_add(1, Ordering::Relaxed);
            let query_start = Instant::now();

            let stream_id = uuid::Uuid::new_v4().to_string();
            let start_ev = ServerResponse::Stream(crate::server::protocol::StreamEvent::Start {
                stream_id: stream_id.clone(),
                metadata: serde_json::json!({}),
            });
            let mut start_json = serde_json::to_string(&start_ev)?;
            start_json.push('\n');
            writer.write_all(start_json.as_bytes()).await?;
            writer.flush().await?;

            let mut seq = 0;
            seq += 1;
            let progress_ev1 =
                ServerResponse::Stream(crate::server::protocol::StreamEvent::Progress {
                    stream_id: stream_id.clone(),
                    sequence: seq,
                    progress: 0.1,
                    message: "Initializing semantic routing...".to_string(),
                    metadata: serde_json::json!({}),
                });
            let mut progress_json1 = serde_json::to_string(&progress_ev1)?;
            progress_json1.push('\n');
            writer.write_all(progress_json1.as_bytes()).await?;
            writer.flush().await?;

            // Execute search on application boundary (which calls search projection)
            let context = ExecutionContext::default();
            let search_query = brain_integrations::dto::v1::SearchQuery {
                text: payload.to_string(),
                kinds: None,
                pagination: None,
            };

            seq += 1;
            let progress_ev2 =
                ServerResponse::Stream(crate::server::protocol::StreamEvent::Progress {
                    stream_id: stream_id.clone(),
                    sequence: seq,
                    progress: 0.5,
                    message: "Running hybrid retrieval...".to_string(),
                    metadata: serde_json::json!({}),
                });
            let mut progress_json2 = serde_json::to_string(&progress_ev2)?;
            progress_json2.push('\n');
            writer.write_all(progress_json2.as_bytes()).await?;
            writer.flush().await?;

            let mut matches = Vec::new();
            let mut resp_msg = String::new();

            match app.search(search_query, &context).await {
                Ok(results) => {
                    let mut seen_contents = std::collections::HashSet::new();
                    for summary in results {
                        let clean_title = if summary.title.trim().starts_with('{') {
                            if let Ok(v) =
                                serde_json::from_str::<serde_json::Value>(summary.title.trim())
                            {
                                if let Some(content) = v.get("content").and_then(|c| c.as_str()) {
                                    content.to_string()
                                } else {
                                    summary.title.clone()
                                }
                            } else {
                                summary.title.clone()
                            }
                        } else {
                            summary.title.clone()
                        };

                        let norm_key = clean_title.trim().trim_matches('.').to_lowercase();
                        if !seen_contents.insert(norm_key) {
                            continue;
                        }

                        let score = summary
                            .metadata
                            .get("score")
                            .and_then(|s| s.parse::<i64>().ok())
                            .unwrap_or(100);

                        matches.push(crate::server::protocol::QueryResultNode {
                            id: summary.id.clone(),
                            label: clean_title,
                            node_type: "session_context".to_string(),
                            content: summary.body.clone(),
                            attributes: serde_json::to_value(&summary.metadata)
                                .unwrap_or(serde_json::json!({})),
                            score,
                            source: "STM".to_string(),
                            connections: Vec::new(),
                        });
                    }
                }
                Err(e) => {
                    resp_msg = format!("Search failed: {:?}", e);
                }
            }

            let ws_set: std::collections::HashSet<String> =
                workspace_context.iter().cloned().collect();
            if !ws_set.is_empty() && resp_msg.is_empty() {
                let (mut ws_matches, other_matches): (Vec<_>, Vec<_>) =
                    matches.into_iter().partition(|n| ws_set.contains(&n.id));
                ws_matches.extend(other_matches);
                matches = ws_matches;
            }

            let context_used: Vec<String> = matches
                .iter()
                .filter(|n| ws_set.contains(&n.id))
                .map(|n| n.id.clone())
                .collect();

            if resp_msg.is_empty() {
                if !matches.is_empty() {
                    seq += 1;
                    let plural = if matches.len() == 1 {
                        "result"
                    } else {
                        "results"
                    };
                    let header_chunk =
                        ServerResponse::Stream(crate::server::protocol::StreamEvent::Chunk {
                            stream_id: stream_id.clone(),
                            sequence: seq,
                            content: format!(
                                "Found {} {} from your memory graph:\n\n🟢 High confidence\n",
                                matches.len(),
                                plural
                            ),
                            metadata: serde_json::json!({}),
                        });
                    let mut chunk_json = serde_json::to_string(&header_chunk)?;
                    chunk_json.push('\n');
                    writer.write_all(chunk_json.as_bytes()).await?;
                    writer.flush().await?;

                    for node in matches {
                        seq += 1;
                        let output_line = format!("  • {}\n", node.label);
                        let text_chunk =
                            ServerResponse::Stream(crate::server::protocol::StreamEvent::Chunk {
                                stream_id: stream_id.clone(),
                                sequence: seq,
                                content: output_line,
                                metadata: serde_json::json!({}),
                            });
                        let mut chunk_json = serde_json::to_string(&text_chunk)?;
                        chunk_json.push('\n');
                        writer.write_all(chunk_json.as_bytes()).await?;
                        writer.flush().await?;
                    }
                } else {
                    seq += 1;
                    let empty_chunk =
                        ServerResponse::Stream(crate::server::protocol::StreamEvent::Chunk {
                            stream_id: stream_id.clone(),
                            sequence: seq,
                            content: "No matching context found in your memory graph.\n"
                                .to_string(),
                            metadata: serde_json::json!({}),
                        });
                    let mut chunk_json = serde_json::to_string(&empty_chunk)?;
                    chunk_json.push('\n');
                    writer.write_all(chunk_json.as_bytes()).await?;
                    writer.flush().await?;
                }
            } else {
                seq += 1;
                let error_chunk =
                    ServerResponse::Stream(crate::server::protocol::StreamEvent::Chunk {
                        stream_id: stream_id.clone(),
                        sequence: seq,
                        content: format!("Error running query: {}\n", resp_msg),
                        metadata: serde_json::json!({}),
                    });
                let mut chunk_json = serde_json::to_string(&error_chunk)?;
                chunk_json.push('\n');
                writer.write_all(chunk_json.as_bytes()).await?;
                writer.flush().await?;
            }

            let query_elapsed = query_start.elapsed().as_micros() as u64;
            metrics
                .sum_query_latency_us
                .fetch_add(query_elapsed, Ordering::Relaxed);

            seq += 1;
            let end_ev = ServerResponse::Stream(crate::server::protocol::StreamEvent::End {
                stream_id: stream_id.clone(),
                sequence: seq,
                metadata: serde_json::json!({ "context_used": context_used }),
            });
            let mut end_json = serde_json::to_string(&end_ev)?;
            end_json.push('\n');
            writer.write_all(end_json.as_bytes()).await?;
            writer.flush().await?;

            let ipc_elapsed = ipc_start.elapsed().as_micros() as u64;
            metrics
                .sum_ipc_latency_us
                .fetch_add(ipc_elapsed, Ordering::Relaxed);
            continue;
        }

        // Run through ProtocolRouter to parse wire to ApplicationRequest
        let app_req = match ProtocolRouter::route(&request) {
            Ok(Some(req)) => req,
            Ok(None) => {
                line.clear();
                continue;
            }
            Err(err_msg) => {
                let response = if is_versioned {
                    serde_json::json!({
                        "version": "1.0",
                        "type": "Error",
                        "id": req_id_val,
                        "status": "error",
                        "body": err_msg,
                    })
                } else {
                    serde_json::json!({
                        "status": "error",
                        "message": err_msg,
                    })
                };
                let mut response_json = serde_json::to_string(&response)?;
                response_json.push('\n');
                let mut w = writer.lock().await;
                w.write_all(response_json.as_bytes()).await?;
                w.flush().await?;
                line.clear();
                continue;
            }
        };

        // Execution details
        let context = ExecutionContext::default();
        let is_ingest = matches!(app_req, ApplicationRequest::Ingest(_));
        if is_ingest {
            metrics.total_ingests.fetch_add(1, Ordering::Relaxed);
            metrics
                .runtime_ingest_attempts
                .fetch_add(1, Ordering::Relaxed);
        }

        let rt_start = Instant::now();

        // Dispatch
        match dispatcher.dispatch(app_req, &context).await {
            Ok(app_resp) => {
                if is_ingest {
                    let rt_elapsed = rt_start.elapsed().as_micros() as u64;
                    metrics
                        .runtime_ingest_successes
                        .fetch_add(1, Ordering::Relaxed);
                    metrics
                        .runtime_ingest_latency_us
                        .fetch_add(rt_elapsed, Ordering::Relaxed);
                    if let Ok(mut reservoir) = metrics.runtime_latency_reservoir.lock() {
                        reservoir.observe(rt_elapsed);
                    }
                }

                // Map response DTO back to UDS protocol wire Response
                let response_body = match app_resp {
                    ApplicationResponse::Status(dto) => {
                        serde_json::to_string(&dto).unwrap_or_default()
                    }
                    ApplicationResponse::Metrics(dto) => {
                        serde_json::to_string(&dto).unwrap_or_default()
                    }
                    ApplicationResponse::Diagnostics(dto) => {
                        serde_json::to_string(&dto).unwrap_or_default()
                    }
                    ApplicationResponse::Capabilities(dto) => {
                        serde_json::to_string(&dto).unwrap_or_default()
                    }
                    ApplicationResponse::Search(dto) => {
                        serde_json::to_string(&dto).unwrap_or_default()
                    }
                    ApplicationResponse::Ingest(dto) => {
                        serde_json::to_string(&dto).unwrap_or_default()
                    }
                    ApplicationResponse::Replay(dto) => {
                        serde_json::to_string(&dto).unwrap_or_default()
                    }
                    ApplicationResponse::InspectNode(dto) => {
                        serde_json::to_string(&dto).unwrap_or_default()
                    }
                    ApplicationResponse::Subscribe(mut stream) => {
                        let writer_clone = Arc::clone(&writer);
                        tokio::spawn(async move {
                            while let Some(msg) = stream.next().await {
                                let versioned_event = ServerResponse::Event(VersionedEvent {
                                    version: "1.0".to_string(),
                                    msg_type: "Event".to_string(),
                                    event_name: "StreamMessage".to_string(),
                                    payload: serde_json::to_value(&msg)
                                        .unwrap_or(serde_json::Value::Null),
                                });
                                if let Ok(mut json) = serde_json::to_string(&versioned_event) {
                                    json.push('\n');
                                    let mut w = writer_clone.lock().await;
                                    let write_fut = w.write_all(json.as_bytes());
                                    if tokio::time::timeout(
                                        std::time::Duration::from_secs(2),
                                        write_fut,
                                    )
                                    .await
                                    .is_err()
                                    {
                                        tracing::error!("Slow consumer timed out on subscription channel. Dropping consumer.");
                                        break;
                                    }
                                    if w.flush().await.is_err() {
                                        break;
                                    }
                                }
                            }
                        });
                        serde_json::json!({ "status": "subscribed" }).to_string()
                    }
                    ApplicationResponse::ListProjectionStatus(dto) => {
                        serde_json::to_string(&dto).unwrap_or_default()
                    }
                    ApplicationResponse::RebuildProjection => {
                        serde_json::json!({ "status": "ok" }).to_string()
                    }
                    ApplicationResponse::Reflect(report) => {
                        serde_json::to_string(&report).unwrap_or_default()
                    }
                    ApplicationResponse::ReflectStatus(status) => {
                        serde_json::to_string(&status).unwrap_or_default()
                    }
                    ApplicationResponse::ReflectReport(report) => {
                        serde_json::to_string(&report).unwrap_or_default()
                    }
                    ApplicationResponse::ReflectSummary(summary) => {
                        serde_json::to_string(&summary).unwrap_or_default()
                    }
                    ApplicationResponse::ReflectFindings(findings) => {
                        serde_json::to_string(&findings).unwrap_or_default()
                    }
                    ApplicationResponse::CompileKnowledge(report) => {
                        serde_json::to_string(&report).unwrap_or_default()
                    }
                    ApplicationResponse::CompileStatus(status) => {
                        serde_json::to_string(&status).unwrap_or_default()
                    }
                    ApplicationResponse::CompileReport(report) => {
                        serde_json::to_string(&report).unwrap_or_default()
                    }
                    ApplicationResponse::CompileSummary(summary) => {
                        serde_json::to_string(&summary).unwrap_or_default()
                    }
                    ApplicationResponse::CompileDiagnostics(diagnostics) => {
                        serde_json::to_string(&diagnostics).unwrap_or_default()
                    }
                    ApplicationResponse::CompileStats(stats) => {
                        serde_json::to_string(&stats).unwrap_or_default()
                    }
                    ApplicationResponse::CompileIrSummary(ir_summary) => {
                        serde_json::to_string(&ir_summary).unwrap_or_default()
                    }
                    ApplicationResponse::ListSessions(body) => body,
                    ApplicationResponse::GetSession(body) => body,
                    ApplicationResponse::CreateSession(res) => {
                        serde_json::to_string(&res).unwrap_or_default()
                    }
                    ApplicationResponse::LoadSession(res) => {
                        serde_json::to_string(&res).unwrap_or_default()
                    }
                    ApplicationResponse::ForkSession(res) => {
                        serde_json::to_string(&res).unwrap_or_default()
                    }
                    ApplicationResponse::SessionOperationSuccess => {
                        serde_json::json!({ "status": "ok" }).to_string()
                    }
                    ApplicationResponse::Context(res) => {
                        serde_json::to_string(&res).unwrap_or_default()
                    }
                };

                let response = if is_versioned {
                    serde_json::json!({
                        "version": "1.0",
                        "type": "Response",
                        "id": req_id_val,
                        "status": "success",
                        "body": response_body,
                    })
                } else {
                    serde_json::json!({
                        "status": "ok",
                        "message": response_body,
                    })
                };

                let mut response_json = serde_json::to_string(&response)?;
                response_json.push('\n');
                let mut w = writer.lock().await;
                w.write_all(response_json.as_bytes()).await?;
                w.flush().await?;
            }
            Err(e) => {
                if is_ingest {
                    metrics
                        .runtime_ingest_failures
                        .fetch_add(1, Ordering::Relaxed);
                }
                let err_msg = format!("Dispatch failed: {:?}", e);
                let response = if is_versioned {
                    serde_json::json!({
                        "version": "1.0",
                        "type": "Error",
                        "id": req_id_val,
                        "status": "error",
                        "body": err_msg,
                    })
                } else {
                    serde_json::json!({
                        "status": "error",
                        "message": err_msg,
                    })
                };
                let mut response_json = serde_json::to_string(&response)?;
                response_json.push('\n');
                let mut w = writer.lock().await;
                w.write_all(response_json.as_bytes()).await?;
                w.flush().await?;
            }
        }

        let ipc_elapsed = ipc_start.elapsed().as_micros() as u64;
        metrics
            .sum_ipc_latency_us
            .fetch_add(ipc_elapsed, Ordering::Relaxed);
    }

    // Cleanup any registered clients for this connection on disconnect
    let fleet = app.runtime().fleet_registry();
    for client_id in conn_registered_clients {
        fleet.unregister_session(&client_id);
    }

    conn_token.cancel();
    Ok(())
}

/// One tool call observed in the current provider pass, recorded when its
/// ToolUse chunk arrives.
struct PassToolUse {
    call_id: String,
    name: String,
    input: serde_json::Value,
}

/// One resolved tool call from the current pass — executed output or a user
/// denial — destined for the next pass's feedback messages.
struct ToolFeedback {
    call_id: String,
    output: String,
    is_error: bool,
}

/// Fixed content carried by denial feedback entries (spec §4.2).
const DENIED_FEEDBACK_TEXT: &str = "User denied permission for this tool call.";

/// Maximum provider passes per turn when BRAIN_TOOL_MAX_ROUNDS is unset or
/// unparseable (spec §2).
const DEFAULT_MAX_TOOL_ROUNDS: u32 = 8;

/// Parses the per-turn tool-round cap: default 8, floored at 1, garbage ⇒
/// default. Pure so tests never mutate process environment.
fn parse_max_rounds(raw: Option<&str>) -> u32 {
    raw.and_then(|s| s.trim().parse::<u32>().ok())
        .map(|v| v.max(1))
        .unwrap_or(DEFAULT_MAX_TOOL_ROUNDS)
}

/// Builds the provider-visible feedback for a completed pass (spec §4.2): an
/// assistant message carrying the pass text (when non-empty) and ToolUse
/// blocks in arrival order, then a user message carrying one ToolResult per
/// resolved call in the same order.
fn feedback_messages(
    pass_text: &str,
    calls: &[PassToolUse],
    results: &[ToolFeedback],
) -> Vec<brain_core::model::ModelChatMessage> {
    use brain_core::model::{ChatRole, MessageContentBlock};

    let mut assistant_blocks: Vec<MessageContentBlock> = Vec::new();
    if !pass_text.is_empty() {
        assistant_blocks.push(MessageContentBlock::Text {
            text: pass_text.to_string(),
        });
    }
    for c in calls {
        assistant_blocks.push(MessageContentBlock::ToolUse {
            id: c.call_id.clone(),
            name: c.name.clone(),
            input: c.input.clone(),
        });
    }
    let assistant = brain_core::model::ModelChatMessage {
        role: ChatRole::Assistant,
        content: assistant_blocks,
    };

    let user_content = results
        .iter()
        .map(|r| MessageContentBlock::ToolResult {
            tool_use_id: r.call_id.clone(),
            content: r.output.clone(),
            is_error: r.is_error,
        })
        .collect::<Vec<_>>();
    let user = brain_core::model::ModelChatMessage {
        role: ChatRole::User,
        content: user_content,
    };

    vec![assistant, user]
}

/// Inc 8: persisted tool-event outputs are bounded so sessions stay small;
/// wire frames keep full text. Mirrors BashTool's marker idiom.
const TOOL_EVENT_OUTPUT_LIMIT_BYTES: usize = 4096;

fn truncate_tool_output(output: &str) -> String {
    if output.len() <= TOOL_EVENT_OUTPUT_LIMIT_BYTES {
        return output.to_string();
    }
    let mut cut = TOOL_EVENT_OUTPUT_LIMIT_BYTES;
    while cut > 0 && !output.is_char_boundary(cut) {
        cut -= 1;
    }
    let mut out = output[..cut].to_string();
    out.push_str("\n…[truncated]");
    out
}

#[cfg(test)]
mod generation_loop_tests {
    use super::*;

    fn call(id: &str) -> PassToolUse {
        PassToolUse {
            call_id: id.to_string(),
            name: "bash".to_string(),
            input: serde_json::json!({"command": "echo hi"}),
        }
    }

    fn executed(id: &str) -> ToolFeedback {
        ToolFeedback {
            call_id: id.to_string(),
            output: "hi\n".to_string(),
            is_error: false,
        }
    }

    #[test]
    fn parse_max_rounds_defaults_on_missing_and_garbage() {
        assert_eq!(parse_max_rounds(None), 8);
        assert_eq!(parse_max_rounds(Some("abc")), 8);
        assert_eq!(parse_max_rounds(Some("")), 8);
    }

    #[test]
    fn parse_max_rounds_parses_and_floors_at_one() {
        assert_eq!(parse_max_rounds(Some("3")), 3);
        assert_eq!(parse_max_rounds(Some("0")), 1);
        assert_eq!(parse_max_rounds(Some("  5  ")), 5);
    }

    #[test]
    fn feedback_messages_order_text_tools_results() {
        let msgs = feedback_messages("Working.", &[call("c1")], &[executed("c1")]);
        assert_eq!(msgs.len(), 2);
        assert_eq!(msgs[0].role, brain_core::model::ChatRole::Assistant);
        assert_eq!(
            msgs[0].content[0],
            brain_core::model::MessageContentBlock::Text {
                text: "Working.".to_string()
            }
        );
        assert_eq!(
            msgs[0].content[1],
            brain_core::model::MessageContentBlock::ToolUse {
                id: "c1".to_string(),
                name: "bash".to_string(),
                input: serde_json::json!({"command": "echo hi"}),
            }
        );
        assert_eq!(msgs[1].role, brain_core::model::ChatRole::User);
        assert_eq!(
            msgs[1].content[0],
            brain_core::model::MessageContentBlock::ToolResult {
                tool_use_id: "c1".to_string(),
                content: "hi\n".to_string(),
                is_error: false,
            }
        );
    }

    #[test]
    fn feedback_messages_omits_text_block_when_pass_had_no_text() {
        let msgs = feedback_messages("", &[call("c1")], &[executed("c1")]);
        assert_eq!(msgs[0].content.len(), 1);
        assert!(matches!(
            msgs[0].content[0],
            brain_core::model::MessageContentBlock::ToolUse { .. }
        ));
    }

    #[test]
    fn feedback_preserves_multi_call_ordering() {
        let msgs = feedback_messages(
            "",
            &[call("c1"), call("c2")],
            &[executed("c1"), executed("c2")],
        );
        let ids: Vec<String> = msgs[0]
            .content
            .iter()
            .filter_map(|b| match b {
                brain_core::model::MessageContentBlock::ToolUse { id, .. } => Some(id.clone()),
                _ => None,
            })
            .collect();
        assert_eq!(ids, vec!["c1".to_string(), "c2".to_string()]);
        let answered: Vec<String> = msgs[1]
            .content
            .iter()
            .filter_map(|b| match b {
                brain_core::model::MessageContentBlock::ToolResult { tool_use_id, .. } => {
                    Some(tool_use_id.clone())
                }
                _ => None,
            })
            .collect();
        assert_eq!(answered, vec!["c1".to_string(), "c2".to_string()]);
    }

    #[test]
    fn denial_feedback_shape_round_trips_through_helper() {
        let denial = ToolFeedback {
            call_id: "c9".to_string(),
            output: DENIED_FEEDBACK_TEXT.to_string(),
            is_error: true,
        };
        let msgs = feedback_messages("", &[call("c9")], &[denial]);
        assert_eq!(
            msgs[1].content[0],
            brain_core::model::MessageContentBlock::ToolResult {
                tool_use_id: "c9".to_string(),
                content: DENIED_FEEDBACK_TEXT.to_string(),
                is_error: true,
            }
        );
    }
}

#[cfg(test)]
mod tool_event_tests {
    use super::*;

    #[test]
    fn under_and_at_limit_pass_through_unchanged() {
        let small = "short output".to_string();
        assert_eq!(truncate_tool_output(&small), small);
        let exact = "a".repeat(TOOL_EVENT_OUTPUT_LIMIT_BYTES);
        assert_eq!(truncate_tool_output(&exact), exact);
        assert!(!truncate_tool_output(&exact).contains("[truncated]"));
    }

    #[test]
    fn over_limit_ascii_is_cut_with_marker() {
        let big = "b".repeat(TOOL_EVENT_OUTPUT_LIMIT_BYTES + 100);
        let cut = truncate_tool_output(&big);
        assert!(cut.ends_with("\n…[truncated]"));
        // Body holds at most the limit bytes; the marker is appended after.
        assert!(cut.len() < TOOL_EVENT_OUTPUT_LIMIT_BYTES + "\n…[truncated]".len() + 8);
    }

    #[test]
    fn multibyte_output_cuts_on_char_boundary_without_panicking() {
        // 'é' is 2 bytes; 3000 copies = 6000 bytes > 4096, odd cut candidates land mid-char.
        let big = "é".repeat(3000);
        let cut = truncate_tool_output(&big);
        assert!(cut.ends_with("\n…[truncated]"));
        assert!(cut.is_char_boundary(cut.len() - "\n…[truncated]".len()));
    }

    #[test]
    fn empty_output_stays_empty() {
        assert_eq!(truncate_tool_output(""), "");
    }
}
