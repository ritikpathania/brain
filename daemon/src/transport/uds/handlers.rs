use futures::StreamExt;
use std::collections::HashMap;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Instant;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;
use tokio_util::sync::CancellationToken;

use brain_application::context::ExecutionContext;
use brain_application::dispatcher::{ApplicationRequest, ApplicationResponse, RequestDispatcher};

use crate::server::protocol::{ClientRequest, LegacyResponse, ServerResponse, VersionedEvent};
use crate::transport::uds::router::ProtocolRouter;
use crate::{DaemonMetrics, REQUEST_COUNTER};

/// Active generation state tracked in the daemon runtime.
pub struct ActiveGeneration {
    /// Session ID associated with the active generation.
    pub session_id: brain_domain::SessionId,
    /// Cooperative cancellation token.
    pub cancellation_token: CancellationToken,
}

static GENERATION_REGISTRY: std::sync::OnceLock<
    Arc<tokio::sync::RwLock<HashMap<String, ActiveGeneration>>>,
> = std::sync::OnceLock::new();

static FEEDBACK_REGISTRY: std::sync::OnceLock<Arc<tokio::sync::RwLock<HashMap<String, String>>>> =
    std::sync::OnceLock::new();

static PERMISSION_WAITERS: std::sync::OnceLock<
    Arc<tokio::sync::RwLock<HashMap<String, tokio::sync::oneshot::Sender<bool>>>>,
> = std::sync::OnceLock::new();

/// Pending tool-permission decisions keyed by tool-use call ID. The stream
/// task parks a oneshot sender here; any connection may deliver the verdict
/// via v1/tool/resolve (resolution intentionally rides a second connection —
/// the stream occupies its own connection's read loop).
fn get_permission_waiters()
-> &'static Arc<tokio::sync::RwLock<HashMap<String, tokio::sync::oneshot::Sender<bool>>>>
{
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

pub(crate) fn parse_session_id_flexible(s: &str) -> brain_domain::SessionId {
    if let Ok(id) = s.parse::<brain_domain::SessionId>() {
        id
    } else {
        use std::hash::{Hash, Hasher};
        let mut hasher1 = std::collections::hash_map::DefaultHasher::new();
        s.hash(&mut hasher1);
        let h1 = hasher1.finish();

        let mut hasher2 = std::collections::hash_map::DefaultHasher::new();
        (s, "session_salt").hash(&mut hasher2);
        let h2 = hasher2.finish();

        let mut bytes = [0u8; 16];
        bytes[0..8].copy_from_slice(&h1.to_be_bytes());
        bytes[8..16].copy_from_slice(&h2.to_be_bytes());
        brain_domain::SessionId(ulid::Ulid::from_bytes(bytes))
    }
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

        if action == "session/start" || action == "v1/session/start" {
            let start_req: crate::server::protocol::StartSessionPayload = serde_json::from_str(
                payload,
            )
            .unwrap_or(crate::server::protocol::StartSessionPayload {
                session_id: None,
                title: None,
                workspace_path: None,
            });

            let parsed_sid = start_req
                .session_id
                .as_deref()
                .map(parse_session_id_flexible)
                .unwrap_or_default();
            let sid = parsed_sid.to_string();
            let title = start_req.title.unwrap_or_else(|| "New Session".to_string());
            let created_at_ms = chrono::Utc::now().timestamp_millis();
            let now_secs = (created_at_ms / 1000) as u64;

            let storage = app.runtime().sqlite_storage();
            use brain_core::repositories::SessionRepository;
            let session = brain_domain::Session::new(
                parsed_sid,
                brain_domain::SessionTitle(title.clone()),
                brain_domain::SessionTimestamp(now_secs),
            );
            let _ = storage.save_session(&parsed_sid, &session);

            let resp_body = crate::server::protocol::StartSessionResponseBody {
                session_id: sid,
                title,
                created_at_ms,
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
                    let parsed_sid = parse_session_id_flexible(&req.session_id);
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

                    let parsed_sid = parse_session_id_flexible(&req.session_id);
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
                            let msg = brain_domain::Message::new(
                                parsed_msg_id,
                                role,
                                req.content.clone(),
                            );
                            session.messages.push(msg);
                        }

                    session.updated_at = brain_domain::SessionTimestamp(now_secs);
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
                    let mut total_turns = 1;
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

                    let parsed_sid = parse_session_id_flexible(&req.session_id);
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
                    total_turns = session.messages.len();
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

                    let relations = summary
                        .metadata
                        .get("relations")
                        .and_then(|s| serde_json::from_str::<Vec<serde_json::Value>>(s).ok())
                        .unwrap_or_default();

                    matches.push(crate::server::protocol::MemoryItemDto {
                        node_id: summary.id,
                        label: clean_title,
                        excerpt: summary.body,
                        score,
                        channel: "knowledge_graph".to_string(),
                        timestamp: now,
                        scope: "workspace".to_string(),
                        relations,
                    });
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
                        let clean_content = sanitize_security_string(&req.result.summary.unwrap_or_default());
                        let node_id = brain_domain::NodeId::new();
                        let mut props = std::collections::HashMap::new();
                        props.insert(
                            "content".to_string(),
                            serde_json::json!(clean_content),
                        );
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
            let mut reg = registry.write().await;

            if let Some(ref gen_id) = cancel_req.generation_id {
                if let Some(active) = reg.remove(gen_id) {
                    active.cancellation_token.cancel();
                }
            } else if let Some(ref s_id) = cancel_req.session_id {
                let parsed_sid = parse_session_id_flexible(s_id);
                let to_cancel: Vec<String> = reg
                    .iter()
                    .filter(|(_, active)| active.session_id == parsed_sid)
                    .map(|(k, _)| k.clone())
                    .collect();
                for k in to_cancel {
                    if let Some(active) = reg.remove(&k) {
                        active.cancellation_token.cancel();
                    }
                }
            } else {
                for (_, active) in reg.drain() {
                    active.cancellation_token.cancel();
                }
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
                #[serde(default)]
                granted: bool,
            }

            let resolve_req: ResolvePayload =
                serde_json::from_str(payload).unwrap_or(ResolvePayload {
                    call_id: None,
                    granted: false,
                });

            let outcome = match resolve_req.call_id.clone() {
                Some(call_id) => {
                    let waiter = get_permission_waiters().write().await.remove(&call_id);
                    match waiter {
                        Some(tx) => tx.send(resolve_req.granted).is_ok(),
                        None => false,
                    }
                }
                None => false,
            };

            let response = if outcome {
                serde_json::json!({ "type": "resolved", "status": "ok" })
            } else {
                serde_json::json!({
                    "type": "Error",
                    "status": "error",
                    "body": format!(
                        "Unknown or already-resolved tool call '{}'",
                        resolve_req.call_id.unwrap_or_default()
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
                _thinking_config: Option<serde_json::Value>,
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

            let is_explicit_session = stream_req.session_id.is_some();

            let generation_id = stream_req
                .generation_id
                .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

            let (session_id_str, parsed_session_id) = match stream_req.session_id {
                Some(ref s) => {
                    if s.trim().is_empty() {
                        let err_frame = serde_json::json!({
                            "type": "error",
                            "generation_id": generation_id,
                            "session_id": s,
                            "error": "Session ID cannot be empty",
                            "code": "missing_session_id",
                            "sequence": 0,
                            "status": "failed"
                        });
                        let mut json = serde_json::to_string(&err_frame)?;
                        json.push('\n');
                        writer.write_all(json.as_bytes()).await?;
                        writer.flush().await?;
                        continue;
                    }
                    let id = parse_session_id_flexible(s);
                    (s.clone(), id)
                }
                None => {
                    let id = brain_domain::SessionId::new();
                    (id.to_string(), id)
                }
            };

            // Invariant 4: Validate session existence in storage. Sessions are
            // created explicitly (the workspace/session RFC lifecycle creates
            // the aggregate first, then streams turns into it), so a stream
            // naming a nonexistent id is rejected with session_not_found — it
            // must never fabricate an aggregate for a caller-supplied id.
            // Only implicit streams (no sessionId in the request) mint one.
            let storage = app.runtime().sqlite_storage();
            use brain_core::repositories::SessionRepository;
            let mut session_aggregate = if is_explicit_session {
                match storage.load_session(&parsed_session_id) {
                    Ok(Some(s)) => s,
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
                }
            } else {
                let now_secs = (chrono::Utc::now().timestamp_millis() / 1000) as u64;
                let new_sess = brain_domain::Session::new(
                    parsed_session_id,
                    brain_domain::SessionTitle("Interactive Session".to_string()),
                    brain_domain::SessionTimestamp(now_secs),
                );
                let _ = storage.save_session(&parsed_session_id, &new_sess);
                new_sess
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
                        m.get("content")
                            .and_then(|c| c.as_str())
                            .map(|content_str| content_str.to_string())
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
            let _ = storage.save_session(&parsed_session_id, &session_aggregate);

            // Register ActiveGeneration with CancellationToken child token
            let cancellation_token = conn_token.child_token();
            {
                let mut reg = registry.write().await;
                reg.insert(
                    generation_id.clone(),
                    ActiveGeneration {
                        session_id: parsed_session_id,
                        cancellation_token: cancellation_token.clone(),
                    },
                );
            }
            let gen_guard = GenerationGuard::new(generation_id.clone(), registry);

            // Convert request to ModelChatMessage and ToolDefinition
            let model_name = stream_req
                .model
                .unwrap_or_else(|| "brain-default".to_string());
            let mut model_messages: Vec<brain_core::model::ModelChatMessage> = stream_req
                .messages
                .iter()
                .filter_map(|m| {
                    let role = match m.get("role").and_then(|r| r.as_str()) {
                        Some("user") => brain_core::model::ChatRole::User,
                        Some("assistant") => brain_core::model::ChatRole::Assistant,
                        Some("system") => brain_core::model::ChatRole::System,
                        _ => return None,
                    };
                    let content_text = m.get("content").and_then(|c| c.as_str()).unwrap_or("");
                    Some(brain_core::model::ModelChatMessage::text(
                        role,
                        content_text,
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
                max_tokens: 2048,
                max_items: 10,
                reference_time: None,
            };
            let t_assemble_start = Instant::now();
            let context_snapshot = app.assemble_context(context_req).await.unwrap_or_else(|_| {
                brain_core::context::ContextSnapshot {
                    epoch_id: "epoch-fallback".to_string(),
                    items: Vec::new(),
                    token_count: 0,
                    provenance: brain_core::context::MemoryProvenance::default(),
                    serialized_context: String::new(),
                }
            });
            let assembly_latency_ms = t_assemble_start.elapsed().as_millis() as u64;

            let combined_system_prompt = if !context_snapshot.serialized_context.is_empty() {
                match stream_req.system_prompt {
                    Some(sp) if !sp.trim().is_empty() => {
                        Some(format!("{}\n\n{}", context_snapshot.serialized_context, sp))
                    }
                    _ => Some(context_snapshot.serialized_context.clone()),
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

            // Frame 0: stream_start with memory provenance & telemetry metadata
            let start_packet = serde_json::json!({
                "type": "stream_start",
                "generation_id": generation_id,
                "session_id": session_id_str,
                "sequence": seq,
                "status": "in_progress",
                "metadata": {
                    "model": resolved_model_desc.id,
                    "model_name": resolved_model_desc.name,
                    "provider": resolved_model_desc.provider,
                    "memory_provenance": {
                        "count": context_snapshot.provenance.count,
                        "sources": context_snapshot.provenance.sources,
                        "channels": context_snapshot.provenance.channels.iter().map(|c| format!("{:?}", c).to_lowercase()).collect::<Vec<_>>(),
                        "min_score": context_snapshot.provenance.min_score,
                        "max_score": context_snapshot.provenance.max_score,
                        "epoch_id": context_snapshot.provenance.epoch_id,
                    },
                    "telemetry": {
                        "generation_id": generation_id,
                        "session_id": session_id_str,
                        "retrieval_epoch_id": context_snapshot.epoch_id,
                        "candidates_retrieved": context_snapshot.provenance.count,
                        "memories_assembled": context_snapshot.items.len(),
                        "context_tokens_used": context_snapshot.token_count,
                        "assembly_latency_ms": assembly_latency_ms,
                    }
                }
            });
            let mut start_json = serde_json::to_string(&start_packet)?;
            start_json.push('\n');
            writer.write_all(start_json.as_bytes()).await?;
            writer.flush().await?;

            let max_rounds =
                parse_max_rounds(std::env::var("BRAIN_TOOL_MAX_ROUNDS").ok().as_deref());
            let mut total_usage =
                brain_core::model::TokenUsage { input_tokens: 0, output_tokens: 0 };
            let mut accumulated_response = String::new();
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
                    thinking_budget: None,
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
                                            let packet = serde_json::json!({
                                                "type": "thinking_end",
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
                                            let perm_packet = serde_json::json!({
                                                "type": "tool_permission_requested",
                                                "generation_id": generation_id,
                                                "session_id": session_id_str,
                                                "sequence": seq,
                                                "call_id": call_id,
                                                "tool_name": tool_name,
                                                "input": packet["toolUse"]["input"],
                                                "reason": "tool execution requires approval",
                                                "status": "in_progress"
                                            });
                                            let mut pj = serde_json::to_string(&perm_packet)?;
                                            pj.push('\n');
                                            writer.write_all(pj.as_bytes()).await?;
                                            writer.flush().await?;

                                            let (tx, rx) = tokio::sync::oneshot::channel::<bool>();
                                            get_permission_waiters()
                                                .write()
                                                .await
                                                .insert(call_id.clone(), tx);
                                            let timeout_secs = std::env::var(
                                                "BRAIN_TOOL_PERMISSION_TIMEOUT_SECS",
                                            )
                                            .ok()
                                            .and_then(|v| v.parse::<u64>().ok())
                                            .unwrap_or(300);
                                            let granted = tokio::time::timeout(
                                                std::time::Duration::from_secs(timeout_secs),
                                                rx,
                                            )
                                            .await
                                            .ok()
                                            .and_then(|r| r.ok())
                                            .unwrap_or(false);
                                            get_permission_waiters()
                                                .write()
                                                .await
                                                .remove(&call_id);

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
                                                stack
                                                    .permissions
                                                    .grant(brain_core::extensibility::Permission::Shell);
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
                                                        brain_tools::CancellationTokenImpl::default(),
                                                    ),
                                                    deadline: None,
                                                };
                                                seq += 1;
                                                let execution =
                                                    match stack.registry.get_tool(&tool_name) {
                                                        Some(tool) => stack
                                                            .executor
                                                            .execute(
                                                                tool,
                                                                &tool_ctx,
                                                                &stack.permissions,
                                                                &args_map,
                                                            )
                                                            .await,
                                                        None => Err(
                                                            brain_core::errors::BrainError::Internal {
                                                                message: format!(
                                                                    "Unknown tool '{tool_name}'"
                                                                ),
                                                            },
                                                        ),
                                                    };
                                                let (out_text, is_err, exit_code) = match execution {
                                                    Ok(result) => {
                                                        let v = result.value().clone();
                                                        (
                                                            v["output"]
                                                                .as_str()
                                                                .unwrap_or("")
                                                                .to_string(),
                                                            v["is_error"].as_bool().unwrap_or(true),
                                                            v["exit_code"].as_i64().unwrap_or(-1),
                                                        )
                                                    }
                                                    Err(e) => (format!("{e}"), true, -1),
                                                };
                                                feedback.push(ToolFeedback {
                                                    call_id: call_id.clone(),
                                                    name: tool_name.clone(),
                                                    input: packet["toolUse"]["input"].clone(),
                                                    output: out_text.clone(),
                                                    is_error: is_err,
                                                });
                                                let result_packet = serde_json::json!({
                                                    "type": "tool_result",
                                                    "generation_id": generation_id,
                                                    "session_id": session_id_str,
                                                    "sequence": seq,
                                                    "call_id": call_id,
                                                    "tool_name": tool_name,
                                                    "output": out_text,
                                                    "is_error": is_err,
                                                    "exit_code": exit_code,
                                                    "status": "in_progress"
                                                });
                                                let mut rj = serde_json::to_string(&result_packet)?;
                                                rj.push('\n');
                                                writer.write_all(rj.as_bytes()).await?;
                                                writer.flush().await?;
                                            }

                                            if !granted {
                                                seq += 1;
                                                let denied_packet = serde_json::json!({
                                                    "type": "tool_denied",
                                                    "generation_id": generation_id,
                                                    "session_id": session_id_str,
                                                    "sequence": seq,
                                                    "call_id": call_id,
                                                    "tool_name": tool_name,
                                                    "status": "in_progress"
                                                });
                                                let mut dj = serde_json::to_string(&denied_packet)?;
                                                dj.push('\n');
                                                writer.write_all(dj.as_bytes()).await?;
                                                writer.flush().await?;
                                                feedback.push(ToolFeedback {
                                                    call_id: call_id.clone(),
                                                    name: tool_name.clone(),
                                                    input: packet["toolUse"]["input"].clone(),
                                                    output: DENIED_FEEDBACK_TEXT.to_string(),
                                                    is_error: true,
                                                });
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

            if is_cancelled || round_completed.is_none() {
                break 'rounds;
            }

            let (finish_reason, usage) = round_completed.take().unwrap();
            total_usage.input_tokens += usage.input_tokens;
            total_usage.output_tokens += usage.output_tokens;

            // Continuation rule (spec §3): keep going only when this pass
            // resolved at least one tool call AND rounds remain; otherwise
            // emit the sole terminating stream_end.
            let terminate_reason: Option<String> = if feedback.is_empty() {
                Some(finish_reason)
            } else if round_index + 1 >= max_rounds {
                Some("max_tool_rounds".to_string())
            } else {
                model_messages.extend(feedback_messages(
                    &pass_text,
                    &pass_calls,
                    &feedback,
                ));
                None
            };

            if let Some(reason) = terminate_reason {
                is_completed_successfully = true;
                // Terminal stream_end owns a fresh sequence slot (the legacy
                // strict-monotonic contract); mid-loop Completions restored
                // theirs above.
                seq += 1;
                let total_duration_ms = gen_start_time.elapsed().as_millis() as u64;
                let end_packet = serde_json::json!({
                    "type": "stream_end",
                    "generation_id": generation_id,
                    "session_id": session_id_str,
                    "sequence": seq,
                    "status": "completed",
                    "response": accumulated_response,
                    "finish_reason": reason,
                    "metadata": {
                        "inputTokens": total_usage.input_tokens,
                        "outputTokens": total_usage.output_tokens,
                        "telemetry": {
                            "generation_id": generation_id,
                            "session_id": session_id_str,
                            "retrieval_epoch_id": context_snapshot.epoch_id,
                            "candidates_retrieved": context_snapshot.provenance.count,
                            "memories_assembled": context_snapshot.items.len(),
                            "context_tokens_used": context_snapshot.token_count,
                            "assembly_latency_ms": assembly_latency_ms,
                            "total_duration_ms": total_duration_ms,
                            "finish_reason": reason,
                        }
                    }
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
    name: String,
    input: serde_json::Value,
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
            name: "bash".to_string(),
            input: serde_json::json!({"command": "echo hi"}),
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
            brain_core::model::MessageContentBlock::Text { text: "Working.".to_string() }
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
            name: "bash".to_string(),
            input: serde_json::json!({}),
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
