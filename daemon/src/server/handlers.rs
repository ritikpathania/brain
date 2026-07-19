use std::str::FromStr;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Instant;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;
use tracing::{error, info};

use brain_core::events::CorrelationId;
use brain_core::evolution::{Observation, Provenance};
use brain_services::BrainRuntime;

use crate::server::protocol::{
    ClientRequest, LegacyResponse, ServerResponse, VersionedError, VersionedResponse,
};
use crate::{DaemonMetrics, REQUEST_COUNTER};

pub async fn handle_connection(
    mut stream: UnixStream,
    metrics: Arc<DaemonMetrics>,
    brain_runtime: Arc<BrainRuntime>,
) -> Result<(), Box<dyn std::error::Error>> {
    let (reader, mut writer) = stream.split();
    let mut buf_reader = BufReader::new(reader);
    let mut line = String::new();

    // Use a fixed local session identifier
    let _session_id = "default_user_session".to_string();

    while buf_reader.read_line(&mut line).await? > 0 {
        let request_str = line.trim();
        if request_str.is_empty() {
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
                writer.write_all(response_json.as_bytes()).await?;
                writer.flush().await?;
                line.clear();
                continue;
            }
        };

        line.clear();

        let (action, payload, req_id, is_versioned, workspace_context) = match request {
            ClientRequest::Versioned(req) => (
                req.action,
                req.body,
                Some(req.id),
                true,
                req.workspace_context,
            ),
            ClientRequest::Legacy(req) => (req.action, req.payload, None, false, Vec::new()),
        };

        let response = match action.as_str() {
            "ingest" => {
                metrics.total_ingests.fetch_add(1, Ordering::Relaxed);
                let ingest_start = Instant::now();

                // 1. Run BrainRuntime Ingestion directly
                let obs = Observation {
                    payload: payload.as_bytes().to_vec(),
                    media_type: "text/plain".to_string(),
                    provenance: Provenance {
                        source_adapter: "daemon-uds".to_string(),
                        timestamp: std::time::SystemTime::now(),
                        correlation_id: CorrelationId::new_v4(),
                    },
                };

                metrics
                    .runtime_ingest_attempts
                    .fetch_add(1, Ordering::Relaxed);

                let rt_start = Instant::now();
                match brain_runtime.ingest(obs) {
                    Ok(result) => {
                        let rt_elapsed = rt_start.elapsed().as_micros() as u64;
                        metrics
                            .runtime_ingest_successes
                            .fetch_add(1, Ordering::Relaxed);
                        metrics
                            .runtime_ingest_latency_us
                            .fetch_add(rt_elapsed, Ordering::Relaxed);

                        metrics.runtime_canonicalization_latency_us.fetch_add(
                            result.stage_timings.canonicalization.as_micros() as u64,
                            Ordering::Relaxed,
                        );
                        metrics.runtime_reflection_latency_us.fetch_add(
                            result.stage_timings.reflection.as_micros() as u64,
                            Ordering::Relaxed,
                        );
                        metrics.runtime_dispatch_latency_us.fetch_add(
                            result.stage_timings.dispatch.as_micros() as u64,
                            Ordering::Relaxed,
                        );

                        if let Ok(mut reservoir) = metrics.runtime_latency_reservoir.lock() {
                            reservoir.observe(rt_elapsed);
                        }

                        let node_id_str = result
                            .affected_entities
                            .first()
                            .map(|nid| nid.0.to_string())
                            .unwrap_or_else(|| "".to_string());
                        let target_epoch = result.epoch.0;

                        info!(
                            component = "runtime",
                            epoch = target_epoch,
                            entities = result.affected_entities.len(),
                            latency_us = rt_elapsed,
                            "BrainRuntime ingestion succeeded"
                        );

                        let ingest_elapsed = ingest_start.elapsed().as_micros() as u64;
                        metrics
                            .sum_ingest_latency_us
                            .fetch_add(ingest_elapsed, Ordering::Relaxed);

                        let msg = format!(
                            "Ingested node '{}' (Epoch {}) successfully",
                            node_id_str, target_epoch
                        );

                        if is_versioned {
                            Some(ServerResponse::Response(VersionedResponse {
                                version: "1.0".to_string(),
                                msg_type: "Response".to_string(),
                                id: req_id.unwrap_or(0),
                                status: "success".to_string(),
                                body: msg,
                            }))
                        } else {
                            Some(ServerResponse::Legacy(LegacyResponse {
                                status: "ok".to_string(),
                                message: msg,
                            }))
                        }
                    }
                    Err(e) => {
                        metrics
                            .runtime_ingest_failures
                            .fetch_add(1, Ordering::Relaxed);
                        error!(
                            component = "runtime",
                            error = %e,
                            "Authoritative BrainRuntime ingestion failed"
                        );

                        let err_msg = format!("Ingestion failed: {:?}", e);
                        if is_versioned {
                            Some(ServerResponse::Error(VersionedError {
                                version: "1.0".to_string(),
                                msg_type: "Error".to_string(),
                                id: req_id.unwrap_or(0),
                                status: "error".to_string(),
                                body: err_msg,
                            }))
                        } else {
                            Some(ServerResponse::Legacy(LegacyResponse {
                                status: "error".to_string(),
                                message: err_msg,
                            }))
                        }
                    }
                }
            }
            "query" => {
                metrics.total_queries.fetch_add(1, Ordering::Relaxed);
                let query_start = Instant::now();

                // 1. Send StreamEvent::Start
                let stream_id = uuid::Uuid::new_v4().to_string();
                let start_ev =
                    ServerResponse::Stream(crate::server::protocol::StreamEvent::Start {
                        stream_id: stream_id.clone(),
                        metadata: serde_json::json!({}),
                    });
                let mut start_json = serde_json::to_string(&start_ev)?;
                start_json.push('\n');
                writer.write_all(start_json.as_bytes()).await?;
                writer.flush().await?;

                let mut seq = 0;

                // Send progress update 1
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

                let query_payload = payload.clone();
                let limit = 20;

                // 2. Run query through BrainRuntime projection
                let rt_corr_id = CorrelationId::new_v4();
                let search_query = brain_services::SearchProjectionQuery {
                    query: query_payload.clone(),
                    limit,
                };
                let search_projector = brain_services::SearchProjector;

                let mut matches = Vec::new();
                let mut resp_msg = String::new();

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

                let runtime_res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    brain_runtime.query_projection(&search_projector, &search_query, rt_corr_id)
                }));

                match runtime_res {
                    Ok(projection_result) => {
                        for (node, score) in projection_result.items {
                            let node_edges: Vec<_> = projection_result
                                .edges
                                .iter()
                                .filter(|e| e.source == node.id || e.target == node.id)
                                .map(|e| crate::server::protocol::ExtractedEdge {
                                    source: e.source.to_string(),
                                    target: e.target.to_string(),
                                    relation: e.relation.to_string(),
                                })
                                .collect();

                            matches.push(crate::server::protocol::QueryResultNode {
                                id: node.id.to_string(),
                                label: node.label.clone(),
                                node_type: "session_context".to_string(),
                                content: node.label.clone(),
                                attributes: serde_json::json!({
                                    "epoch": node.provenance.extracted_at,
                                    "timestamp": node.provenance.extracted_at,
                                }),
                                score,
                                source: "STM".to_string(),
                                connections: node_edges,
                            });
                        }
                    }
                    Err(_) => {
                        error!("BrainRuntime query projection panicked");
                        resp_msg = "Query projection execution panicked".to_string();
                    }
                }

                // Workspace context boost
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
                                    "Found {} {} from your memory graph:\n",
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
                            let confidence = if node.score >= 7000 {
                                "High"
                            } else if node.score >= 3000 {
                                "Medium"
                            } else {
                                "Low"
                            };

                            let preview: String = node
                                .content
                                .lines()
                                .next()
                                .unwrap_or(&node.content)
                                .chars()
                                .take(120)
                                .collect();

                            let mut clean_preview = preview.replace("{}", "");
                            let redundant_pattern = format!("{} ({})", node.label, node.node_type);
                            if clean_preview.contains(&redundant_pattern) {
                                clean_preview = clean_preview.replace(&redundant_pattern, "");
                            }
                            let clean_preview = clean_preview.trim().to_string();

                            let output_line = format!(
                                "**{}** — {} ({} confidence)\n",
                                clean_preview, node.label, confidence
                            );

                            let text_chunk = ServerResponse::Stream(
                                crate::server::protocol::StreamEvent::Chunk {
                                    stream_id: stream_id.clone(),
                                    sequence: seq,
                                    content: output_line,
                                    metadata: serde_json::json!({}),
                                },
                            );
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

                None
            }
            "ingest_event" => {
                metrics.total_ingests.fetch_add(1, Ordering::Relaxed);
                let ingest_start = Instant::now();

                let envelope: Result<brain_integrations::IngestionEnvelope, serde_json::Error> =
                    serde_json::from_str(&payload);

                let response = match envelope {
                    Ok(env) => {
                        if env.event_model_version != "1.0" {
                            let msg = format!(
                                "Unsupported event model version: {}",
                                env.event_model_version
                            );
                            if is_versioned {
                                ServerResponse::Error(VersionedError {
                                    version: "1.0".to_string(),
                                    msg_type: "Error".to_string(),
                                    id: req_id.unwrap_or(0),
                                    status: "error".to_string(),
                                    body: msg,
                                })
                            } else {
                                ServerResponse::Legacy(LegacyResponse {
                                    status: "error".to_string(),
                                    message: msg,
                                })
                            }
                        } else {
                            let text_to_ingest = match &env.event {
                                brain_integrations::IngestionEvent::Text { content, .. } => {
                                    Some(content.clone())
                                }
                                brain_integrations::IngestionEvent::Message { content, .. } => {
                                    Some(content.clone())
                                }
                                brain_integrations::IngestionEvent::TerminalCommand {
                                    command,
                                    stdout_summary,
                                    ..
                                } => Some(format!(
                                    "Terminal command executed: {}\nOutput: {}",
                                    command,
                                    stdout_summary.as_deref().unwrap_or("")
                                )),
                                brain_integrations::IngestionEvent::FileEdit {
                                    path, diff, ..
                                } => Some(format!(
                                    "File edited: {}\nDiff:\n{}",
                                    path,
                                    diff.as_deref().unwrap_or("")
                                )),
                                brain_integrations::IngestionEvent::GitCommit {
                                    message,
                                    hash,
                                    ..
                                } => Some(format!("Git commit ({}): {}", hash, message)),
                                brain_integrations::IngestionEvent::GitBranch {
                                    action,
                                    branch_name,
                                    ..
                                } => Some(format!("Git branch {}: {}", action, branch_name)),
                                brain_integrations::IngestionEvent::Diagnostic {
                                    message,
                                    severity,
                                    source,
                                    file,
                                    ..
                                } => Some(format!(
                                    "Diagnostic [{} / {}] in {}: {}",
                                    source,
                                    severity,
                                    file.as_deref().unwrap_or(""),
                                    message
                                )),
                                _ => None,
                            };

                            if let Some(text) = text_to_ingest {
                                let obs = Observation {
                                    payload: text.as_bytes().to_vec(),
                                    media_type: "text/plain".to_string(),
                                    provenance: Provenance {
                                        source_adapter: "daemon-uds-event".to_string(),
                                        timestamp: std::time::SystemTime::now(),
                                        correlation_id: CorrelationId::new_v4(),
                                    },
                                };
                                let _ = brain_runtime.ingest(obs);
                            }

                            let ack_body = serde_json::json!({
                                "sequence": 1,
                                "event_id": env.identity.event_id.to_string(),
                            })
                            .to_string();

                            if is_versioned {
                                ServerResponse::Response(VersionedResponse {
                                    version: "1.0".to_string(),
                                    msg_type: "Response".to_string(),
                                    id: req_id.unwrap_or(0),
                                    status: "success".to_string(),
                                    body: ack_body,
                                })
                            } else {
                                ServerResponse::Legacy(LegacyResponse {
                                    status: "ok".to_string(),
                                    message: ack_body,
                                })
                            }
                        }
                    }
                    Err(e) => {
                        let msg = format!("Failed to parse IngestionEnvelope: {}", e);
                        if is_versioned {
                            ServerResponse::Error(VersionedError {
                                version: "1.0".to_string(),
                                msg_type: "Error".to_string(),
                                id: req_id.unwrap_or(0),
                                status: "error".to_string(),
                                body: msg,
                            })
                        } else {
                            ServerResponse::Legacy(LegacyResponse {
                                status: "error".to_string(),
                                message: msg,
                            })
                        }
                    }
                };

                let ingest_elapsed = ingest_start.elapsed().as_micros() as u64;
                metrics
                    .sum_ingest_latency_us
                    .fetch_add(ingest_elapsed, Ordering::Relaxed);

                Some(response)
            }
            "replay" => {
                let response = if is_versioned {
                    ServerResponse::Response(VersionedResponse {
                        version: "1.0".to_string(),
                        msg_type: "Response".to_string(),
                        id: req_id.unwrap_or(0),
                        status: "success".to_string(),
                        body: "[]".to_string(),
                    })
                } else {
                    ServerResponse::Legacy(LegacyResponse {
                        status: "ok".to_string(),
                        message: "[]".to_string(),
                    })
                };
                Some(response)
            }
            "handshake" => {
                let response = if is_versioned {
                    ServerResponse::Response(VersionedResponse {
                        version: "1.0".to_string(),
                        msg_type: "Response".to_string(),
                        id: req_id.unwrap_or(0),
                        status: "success".to_string(),
                        body: "handshake ok".to_string(),
                    })
                } else {
                    ServerResponse::Legacy(LegacyResponse {
                        status: "ok".to_string(),
                        message: "handshake ok".to_string(),
                    })
                };
                Some(response)
            }
            "heartbeat" => {
                let response = if is_versioned {
                    ServerResponse::Response(VersionedResponse {
                        version: "1.0".to_string(),
                        msg_type: "Response".to_string(),
                        id: req_id.unwrap_or(0),
                        status: "success".to_string(),
                        body: "{\"uptime_ms\":1000}".to_string(),
                    })
                } else {
                    ServerResponse::Legacy(LegacyResponse {
                        status: "ok".to_string(),
                        message: "{\"uptime_ms\":1000}".to_string(),
                    })
                };
                Some(response)
            }
            "inspect_node" => {
                let mut node_opt = None;
                let mut connections = Vec::new();

                let storage = brain_runtime.storage_ref();
                let _ = storage.run_transaction(&mut |tx| {
                    let repos = tx.repositories();
                    if let Ok(node_id) = brain_domain::NodeId::from_str(&payload) {
                        if let Ok(Some(n)) = repos.nodes().find_by_id(&node_id) {
                            node_opt = Some(n);
                            if let Ok(edges) = repos.edges().get_connections(&node_id) {
                                connections = edges;
                            }
                        }
                    }
                    Ok(())
                });

                let response = if let Some(node) = node_opt {
                    let mut relationships = Vec::new();
                    let _ = storage.run_transaction(&mut |tx| {
                        let repos = tx.repositories();
                        for edge in &connections {
                            let is_outgoing = edge.source == node.id;
                            let neighbor_id = if is_outgoing {
                                edge.target
                            } else {
                                edge.source
                            };
                            if let Ok(Some(neighbor)) = repos.nodes().find_by_id(&neighbor_id) {
                                relationships.push(
                                    brain_domain::query::inspector::RelationshipDTO {
                                        target_id: neighbor.id.to_string(),
                                        target_label: neighbor.label.clone(),
                                        target_type: format!("{:?}", neighbor.node_type)
                                            .to_lowercase(),
                                        relation: edge.relation.to_string(),
                                        direction: if is_outgoing {
                                            "outgoing".to_string()
                                        } else {
                                            "incoming".to_string()
                                        },
                                        weight: 1.0,
                                    },
                                );
                            }
                        }
                        Ok(())
                    });

                    let entity = brain_domain::dtos::NodeDTO::new(
                        node.id.to_string(),
                        node.label.clone(),
                        format!("{:?}", node.node_type).to_lowercase(),
                        serde_json::json!(node.properties),
                    );

                    let mut metadata = std::collections::HashMap::new();
                    metadata.insert(
                        "node_type".to_string(),
                        format!("{:?}", node.node_type).to_lowercase(),
                    );
                    metadata.insert("id".to_string(), node.id.to_string());

                    let provenance = brain_domain::query::inspector::ProvenanceDTO {
                        source: "System Ingest".to_string(),
                        location: "System Ingest".to_string(),
                        timestamp: 0,
                        extra_info: std::collections::HashMap::new(),
                    };

                    let recent_activity = vec![brain_domain::query::inspector::ActivityLogEntry {
                        timestamp: 0,
                        action: "Ingested".to_string(),
                        details: "Entity extracted from source location by system.".to_string(),
                    }];

                    let model = brain_domain::query::inspector::InspectorModel {
                        entity,
                        metadata,
                        relationships,
                        provenance,
                        retrieval_explanation: None,
                        recent_activity,
                    };

                    match serde_json::to_string(&model) {
                        Ok(response_body) => {
                            if is_versioned {
                                ServerResponse::Response(VersionedResponse {
                                    version: "1.0".to_string(),
                                    msg_type: "Response".to_string(),
                                    id: req_id.unwrap_or(0),
                                    status: "success".to_string(),
                                    body: response_body,
                                })
                            } else {
                                ServerResponse::Legacy(LegacyResponse {
                                    status: "ok".to_string(),
                                    message: response_body,
                                })
                            }
                        }
                        Err(e) => {
                            let msg = format!("Failed to serialize InspectorModel JSON: {}", e);
                            if is_versioned {
                                ServerResponse::Error(VersionedError {
                                    version: "1.0".to_string(),
                                    msg_type: "Error".to_string(),
                                    id: req_id.unwrap_or(0),
                                    status: "error".to_string(),
                                    body: msg,
                                })
                            } else {
                                ServerResponse::Legacy(LegacyResponse {
                                    status: "error".to_string(),
                                    message: msg,
                                })
                            }
                        }
                    }
                } else {
                    let msg = format!("Entity not found for node ID: {}", payload);
                    if is_versioned {
                        ServerResponse::Error(VersionedError {
                            version: "1.0".to_string(),
                            msg_type: "Error".to_string(),
                            id: req_id.unwrap_or(0),
                            status: "error".to_string(),
                            body: msg,
                        })
                    } else {
                        ServerResponse::Legacy(LegacyResponse {
                            status: "error".to_string(),
                            message: msg,
                        })
                    }
                };

                Some(response)
            }
            "disconnect" => {
                let response = if is_versioned {
                    ServerResponse::Response(VersionedResponse {
                        version: "1.0".to_string(),
                        msg_type: "Response".to_string(),
                        id: req_id.unwrap_or(0),
                        status: "success".to_string(),
                        body: "disconnected ok".to_string(),
                    })
                } else {
                    ServerResponse::Legacy(LegacyResponse {
                        status: "ok".to_string(),
                        message: "disconnected ok".to_string(),
                    })
                };

                let mut response_json = serde_json::to_string(&response)?;
                response_json.push('\n');
                writer.write_all(response_json.as_bytes()).await?;
                writer.flush().await?;
                return Ok(());
            }
            _ => {
                let msg = format!("Malformed request: unknown action '{}'", action);
                if is_versioned {
                    Some(ServerResponse::Error(VersionedError {
                        version: "1.0".to_string(),
                        msg_type: "Error".to_string(),
                        id: req_id.unwrap_or(0),
                        status: "error".to_string(),
                        body: msg,
                    }))
                } else {
                    Some(ServerResponse::Legacy(LegacyResponse {
                        status: "error".to_string(),
                        message: msg,
                    }))
                }
            }
        };

        let ipc_elapsed = ipc_start.elapsed().as_micros() as u64;
        metrics
            .sum_ipc_latency_us
            .fetch_add(ipc_elapsed, Ordering::Relaxed);

        if let Some(resp) = response {
            let mut response_json = serde_json::to_string(&resp)?;
            response_json.push('\n');

            writer.write_all(response_json.as_bytes()).await?;
            writer.flush().await?;
        }
    }

    Ok(())
}
