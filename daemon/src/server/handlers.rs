use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Instant;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;
use tracing::{error, info};

use crate::plugins::PluginRegistry;
use crate::retrieval::pipeline::run_retrieval_pipeline;
use crate::server::protocol::{
    ClientRequest, LegacyResponse, ServerResponse, VersionedError, VersionedResponse,
};
use crate::stm;
use crate::storage::duckdb::AnalyticsEvent;
use crate::{DaemonMetrics, GlobalState, REQUEST_COUNTER};

pub async fn handle_connection(
    mut stream: UnixStream,
    state: GlobalState,
    plugin_registry: Arc<PluginRegistry>,
    metrics: Arc<DaemonMetrics>,
    analytics_tx: tokio::sync::mpsc::UnboundedSender<AnalyticsEvent>,
) -> Result<(), Box<dyn std::error::Error>> {
    let (reader, mut writer) = stream.split();
    let mut buf_reader = BufReader::new(reader);
    let mut line = String::new();

    // In a typical single-user companion CLI, use a fixed local session identifier
    let session_id = "default_user_session".to_string();

    while buf_reader.read_line(&mut line).await? > 0 {
        let request_str = line.trim();
        if request_str.is_empty() {
            line.clear();
            continue;
        }

        let ipc_start = Instant::now();
        let correlation_id = REQUEST_COUNTER.fetch_add(1, Ordering::Relaxed);

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

        // 1. Ensure the session exists in volatile STM cache
        {
            let mut state_guard = state.write().await;
            state_guard
                .entry(session_id.clone())
                .or_insert_with(stm::SessionContext::new);
        }

        let (action, payload, req_id, is_versioned) = match request {
            ClientRequest::Versioned(req) => (req.action, req.body, Some(req.id), true),
            ClientRequest::Legacy(req) => (req.action, req.payload, None, false),
        };

        let response = match action.as_str() {
            "ingest" => {
                metrics.total_ingests.fetch_add(1, Ordering::Relaxed);
                let ingest_start = Instant::now();

                let mut state_guard = state.write().await;
                let session = state_guard.get_mut(&session_id).unwrap();

                let node = session.ingest(payload.clone());
                let ingest_elapsed = ingest_start.elapsed().as_micros() as u64;
                metrics
                    .sum_ingest_latency_us
                    .fetch_add(ingest_elapsed, Ordering::Relaxed);

                info!(
                    node_id = %node.id,
                    epoch = node.epoch,
                    correlation_id = correlation_id,
                    "Node successfully ingested to STM cache"
                );

                let _ = analytics_tx.send(AnalyticsEvent::Ingest {
                    correlation_id,
                    node_id: node.id.clone(),
                    content_length: payload.len() as u64,
                });

                let msg = format!(
                    "Ingested node '{}' (Epoch {}) successfully",
                    node.id, node.epoch
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
            "query" => {
                metrics.total_queries.fetch_add(1, Ordering::Relaxed);
                let query_start = Instant::now();

                let stream_id = format!("stream-{}", correlation_id);
                let pacing_ms = std::env::var("BRAIN_STREAM_PACING_MS")
                    .ok()
                    .and_then(|v| v.parse::<u64>().ok())
                    .unwrap_or(0);

                // 1. Send StreamEvent::Start
                let start_ev = ServerResponse::Stream(crate::server::protocol::StreamEvent::Start {
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
                let progress_ev = ServerResponse::Stream(crate::server::protocol::StreamEvent::Progress {
                    stream_id: stream_id.clone(),
                    sequence: seq,
                    progress: 0.1,
                    message: "Starting query retrieval...".to_string(),
                    metadata: serde_json::json!({}),
                });
                let mut progress_json = serde_json::to_string(&progress_ev)?;
                progress_json.push('\n');
                writer.write_all(progress_json.as_bytes()).await?;
                writer.flush().await?;
                if pacing_ms > 0 {
                    tokio::time::sleep(tokio::time::Duration::from_millis(pacing_ms)).await;
                }

                // Read from volatile STM cache first
                let (window_clone, index_clone) = {
                    let state_guard = state.read().await;
                    if let Some(session) = state_guard.get(&session_id) {
                        (
                            session
                                .interaction_sliding_window
                                .iter()
                                .cloned()
                                .collect::<Vec<_>>(),
                            session.index.clone(),
                        )
                    } else {
                        (Vec::new(), stm::STMIndex::new())
                    }
                };

                // Send progress update 2
                seq += 1;
                let progress_ev2 = ServerResponse::Stream(crate::server::protocol::StreamEvent::Progress {
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
                if pacing_ms > 0 {
                    tokio::time::sleep(tokio::time::Duration::from_millis(pacing_ms)).await;
                }

                let query_payload = payload.clone();
                let registry_clone = Arc::clone(&plugin_registry);

                let retrieve_res = tokio::task::spawn_blocking(move || {
                    let active_retrieval = registry_clone.get_retrieval()?;
                    let active_ranking = registry_clone.get_ranking()?;

                    let active_storage = registry_clone.get_storage().ok();
                    let active_embedding = registry_clone.get_embedding().ok();

                    run_retrieval_pipeline(
                        &query_payload,
                        &index_clone,
                        &window_clone,
                        &*active_retrieval,
                        &*active_ranking,
                        active_storage.as_deref(),
                        active_embedding.as_deref(),
                    )
                })
                .await;

                let mut matches = Vec::new();
                let mut resp_msg = String::new();
                match retrieve_res {
                    Ok(Ok(candidates)) => {
                        matches = candidates;
                    }
                    Ok(Err(e)) => {
                        error!("Query retrieval/ranking failed: {}", e);
                        resp_msg = format!("Query retrieval/ranking failed: {}", e);
                    }
                    Err(join_err) => {
                        error!("Blocking join error during query retrieval: {}", join_err);
                        resp_msg =
                            format!("Blocking join error during query retrieval: {}", join_err);
                    }
                }

                let mut hit_type = "None";
                if resp_msg.is_empty() {
                    if !matches.is_empty() {
                        let has_stm = matches.iter().any(|m| m.source == "STM");
                        let has_ltm = matches.iter().any(|m| m.source == "LTM");

                        if has_stm {
                            metrics.cache_hits.fetch_add(1, Ordering::Relaxed);
                        } else {
                            metrics.cache_misses.fetch_add(1, Ordering::Relaxed);
                        }

                        hit_type = if has_stm && has_ltm {
                            "Hybrid"
                        } else if has_stm {
                            "STM"
                        } else {
                            "LTM"
                        };

                        // Send header chunk
                        seq += 1;
                        let header_chunk = ServerResponse::Stream(crate::server::protocol::StreamEvent::Chunk {
                            stream_id: stream_id.clone(),
                            sequence: seq,
                            content: format!("Found {} matches via Hybrid Retrieval:", matches.len()),
                            metadata: serde_json::json!({}),
                        });
                        let mut chunk_json = serde_json::to_string(&header_chunk)?;
                        chunk_json.push('\n');
                        writer.write_all(chunk_json.as_bytes()).await?;
                        writer.flush().await?;
                        if pacing_ms > 0 {
                            tokio::time::sleep(tokio::time::Duration::from_millis(pacing_ms)).await;
                        }

                        for node in matches {
                            seq += 1;
                            let attrs_str = serde_json::to_string(&node.attributes).unwrap_or_default();
                            let match_chunk = ServerResponse::Stream(crate::server::protocol::StreamEvent::Chunk {
                                stream_id: stream_id.clone(),
                                sequence: seq,
                                content: format!(
                                    "\n  • [{}] source='{}' score={} label='{}' type='{}' attributes='{}'",
                                    node.id, node.source, node.score, node.label, node.node_type, attrs_str
                                ),
                                metadata: serde_json::json!({}),
                            });
                            let mut chunk_json = serde_json::to_string(&match_chunk)?;
                            chunk_json.push('\n');
                            writer.write_all(chunk_json.as_bytes()).await?;
                            writer.flush().await?;
                            if pacing_ms > 0 {
                                tokio::time::sleep(tokio::time::Duration::from_millis(pacing_ms)).await;
                            }

                            if !node.connections.is_empty() {
                                for rel in node.connections {
                                    seq += 1;
                                    let rel_chunk = ServerResponse::Stream(crate::server::protocol::StreamEvent::Chunk {
                                        stream_id: stream_id.clone(),
                                        sequence: seq,
                                        content: format!(
                                            "\n    └── [Graph Relation]: [{}] --({})--> [{}]",
                                            rel.source, rel.relation, rel.target
                                        ),
                                        metadata: serde_json::json!({}),
                                    });
                                    let mut chunk_json = serde_json::to_string(&rel_chunk)?;
                                    chunk_json.push('\n');
                                    writer.write_all(chunk_json.as_bytes()).await?;
                                    writer.flush().await?;
                                    if pacing_ms > 0 {
                                        tokio::time::sleep(tokio::time::Duration::from_millis(pacing_ms)).await;
                                    }
                                }
                            }
                        }
                    } else {
                        metrics.cache_misses.fetch_add(1, Ordering::Relaxed);
                        seq += 1;
                        let empty_chunk = ServerResponse::Stream(crate::server::protocol::StreamEvent::Chunk {
                            stream_id: stream_id.clone(),
                            sequence: seq,
                            content: "No matching nodes found in STM or LTM persistent database.".to_string(),
                            metadata: serde_json::json!({}),
                        });
                        let mut chunk_json = serde_json::to_string(&empty_chunk)?;
                        chunk_json.push('\n');
                        writer.write_all(chunk_json.as_bytes()).await?;
                        writer.flush().await?;
                        if pacing_ms > 0 {
                            tokio::time::sleep(tokio::time::Duration::from_millis(pacing_ms)).await;
                        }
                    }
                } else {
                    seq += 1;
                    let cancel_ev = ServerResponse::Stream(crate::server::protocol::StreamEvent::Cancelled {
                        stream_id: stream_id.clone(),
                        sequence: seq,
                        metadata: serde_json::json!({}),
                    });
                    let mut cancel_json = serde_json::to_string(&cancel_ev)?;
                    cancel_json.push('\n');
                    writer.write_all(cancel_json.as_bytes()).await?;
                    writer.flush().await?;

                    let err_resp = ServerResponse::Error(VersionedError {
                        version: "1.0".to_string(),
                        msg_type: "Error".to_string(),
                        id: req_id.unwrap_or(correlation_id),
                        status: "error".to_string(),
                        body: resp_msg,
                    });
                    let mut err_json = serde_json::to_string(&err_resp)?;
                    err_json.push('\n');
                    writer.write_all(err_json.as_bytes()).await?;
                    writer.flush().await?;
                    
                    let query_elapsed = query_start.elapsed().as_micros() as u64;
                    metrics.sum_query_latency_us.fetch_add(query_elapsed, Ordering::Relaxed);
                    let _ = analytics_tx.send(AnalyticsEvent::Query {
                        correlation_id,
                        query_text: payload.clone(),
                        hit_type: "None".to_string(),
                        execution_time_us: query_elapsed,
                    });
                    return Ok(());
                }

                if resp_msg.is_empty() {
                    let query_elapsed = query_start.elapsed().as_micros() as u64;
                    metrics.sum_query_latency_us.fetch_add(query_elapsed, Ordering::Relaxed);

                    let _ = analytics_tx.send(AnalyticsEvent::Query {
                        correlation_id,
                        query_text: payload.clone(),
                        hit_type: hit_type.to_string(),
                        execution_time_us: query_elapsed,
                    });

                    // Send StreamEvent::End
                    seq += 1;
                    let end_ev = ServerResponse::Stream(crate::server::protocol::StreamEvent::End {
                        stream_id: stream_id.clone(),
                        sequence: seq,
                        metadata: serde_json::json!({}),
                    });
                    let mut end_json = serde_json::to_string(&end_ev)?;
                    end_json.push('\n');
                    writer.write_all(end_json.as_bytes()).await?;
                    writer.flush().await?;
                }

                None
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
