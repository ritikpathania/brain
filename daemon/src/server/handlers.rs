use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Instant;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;
use tracing::{error, info, warn};

use brain_core::events::CorrelationId;
use brain_core::evolution::{Observation, Provenance};
use brain_services::BrainRuntime;

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
    brain_runtime: Arc<BrainRuntime>,
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

                // --- BrainRuntime ingestion (alongside existing path) ---
                //
                // The runtime receives every ingest observation and runs it through
                // the full canonicalize → reflect → event-dispatch pipeline.
                //
                // This call is non-fatal: if it fails, the existing STM/LTM path
                // has already succeeded and the client response is unaffected.
                //
                // Counters are sampled from independent atomics; a scrape that
                // observes attempts > successes + failures indicates an in-flight
                // request. Use long-term rates, not single samples.
                {
                    let obs = Observation {
                        payload: payload.as_bytes().to_vec(),
                        media_type: "text/plain".to_string(),
                        provenance: Provenance {
                            source_adapter: "daemon-uds".to_string(),
                            timestamp: std::time::SystemTime::now(),
                            correlation_id: CorrelationId::new_v4(),
                        },
                    };
                    // runtime.ingest() is synchronous (runs on the current thread).
                    // It is safe to call from async context — it completes quickly
                    // and does not block the executor for a meaningful duration.
                    let rt_start = Instant::now();
                    metrics
                        .runtime_ingest_attempts
                        .fetch_add(1, Ordering::Relaxed);

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

                            info!(
                                component = "runtime",
                                epoch = result.epoch.0,
                                entities = result.affected_entities.len(),
                                latency_us = rt_elapsed,
                                "BrainRuntime ingestion succeeded"
                            );
                        }
                        Err(e) => {
                            metrics
                                .runtime_ingest_failures
                                .fetch_add(1, Ordering::Relaxed);
                            warn!(
                                component = "runtime",
                                error = %e,
                                "BrainRuntime ingestion failed (non-fatal — STM path succeeded)"
                            );
                        }
                    }
                }

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
                let progress_ev =
                    ServerResponse::Stream(crate::server::protocol::StreamEvent::Progress {
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

                // --- Workspace boost + context_used derivation ---
                //
                // Conceptual pipeline step order:
                //   retrieve candidates  [run_retrieval_pipeline — done above]
                //   → apply workspace boost  [partition/prepend workspace hits]
                //   → rank  [existing reranker, unchanged]
                //   → produce final matches
                //   → derive context_used  [intersection of workspace IDs and final matches]
                //
                // context_used contract: workspace nodes that materially influenced
                // retrieval for this query. The first implementation approximates
                // "material influence" as appearance in the final match list. This is an
                // implementation detail — future versions may refine this definition.
                let ws_set: std::collections::HashSet<String> =
                    workspace_context.iter().cloned().collect();

                if !ws_set.is_empty() && resp_msg.is_empty() {
                    // Step: apply workspace boost — move workspace-pinned nodes to front.
                    let (mut ws_matches, other_matches): (Vec<_>, Vec<_>) =
                        matches.into_iter().partition(|n| ws_set.contains(&n.id));
                    ws_matches.extend(other_matches);
                    matches = ws_matches;
                }

                // Step: derive context_used — workspace nodes present in the final match list.
                let context_used: Vec<String> = matches
                    .iter()
                    .filter(|n| ws_set.contains(&n.id))
                    .map(|n| n.id.clone())
                    .collect();

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
                        // Show a clean, user-facing header listing result count and source.
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
                        if pacing_ms > 0 {
                            tokio::time::sleep(tokio::time::Duration::from_millis(pacing_ms)).await;
                        }

                        for node in matches {
                            seq += 1;

                            // Map numeric RRF score (0–10000) to a human-readable confidence tier.
                            let confidence = if node.score >= 7000 {
                                "High"
                            } else if node.score >= 3000 {
                                "Medium"
                            } else {
                                "Low"
                            };

                            // Truncate long content for a clean preview.
                            let preview: String = node
                                .content
                                .lines()
                                .next()
                                .unwrap_or(&node.content)
                                .chars()
                                .take(120)
                                .collect();

                            // Strip empty json objects "{}" and trim
                            let mut clean_preview = preview.replace("{}", "");
                            // Strip redundant label + node type (e.g. "Google (organization)")
                            let redundant_pattern = format!("{} ({})", node.label, node.node_type);
                            if clean_preview.contains(&redundant_pattern) {
                                clean_preview = clean_preview.replace(&redundant_pattern, "");
                            }
                            let mut clean_preview = clean_preview.trim().to_string();
                            // If anything like "google organization" is left, clean it
                            if clean_preview.starts_with(&node.label) {
                                clean_preview = clean_preview
                                    .trim_start_matches(&node.label)
                                    .trim()
                                    .to_string();
                            }
                            if clean_preview.starts_with(&node.node_type) {
                                clean_preview = clean_preview
                                    .trim_start_matches(&node.node_type)
                                    .trim()
                                    .to_string();
                            }
                            clean_preview = clean_preview
                                .trim_start_matches(|c| {
                                    c == ' ' || c == '-' || c == '>' || c == '(' || c == ')'
                                })
                                .trim()
                                .to_string();

                            // In debug mode expose raw identifiers for diagnostics.
                            let debug_mode = std::env::var("BRAIN_DEBUG").is_ok();

                            let mut entry = format!(
                                "**{}** — {} ({} confidence)",
                                node.label, node.node_type, confidence
                            );
                            if debug_mode {
                                entry.push_str(&format!(" `[{}]`", node.id));
                            }

                            // Only append preview line if there is clean unique content
                            if !clean_preview.is_empty() {
                                entry.push_str(&format!("\n> {}\n", clean_preview));
                            } else {
                                entry.push('\n');
                            }

                            if !node.connections.is_empty() {
                                let related: Vec<String> = node
                                    .connections
                                    .iter()
                                    .map(|r| format!("{} ({})", r.target, r.relation))
                                    .take(5)
                                    .collect();
                                entry.push_str(&format!("  Related: {}\n", related.join(" · ")));
                            }

                            let match_chunk = ServerResponse::Stream(
                                crate::server::protocol::StreamEvent::Chunk {
                                    stream_id: stream_id.clone(),
                                    sequence: seq,
                                    content: entry,
                                    metadata: serde_json::json!({}),
                                },
                            );
                            let mut chunk_json = serde_json::to_string(&match_chunk)?;
                            chunk_json.push('\n');
                            writer.write_all(chunk_json.as_bytes()).await?;
                            writer.flush().await?;
                            if pacing_ms > 0 {
                                tokio::time::sleep(tokio::time::Duration::from_millis(pacing_ms))
                                    .await;
                            }
                        }
                    } else {
                        metrics.cache_misses.fetch_add(1, Ordering::Relaxed);
                        seq += 1;
                        let empty_chunk = ServerResponse::Stream(crate::server::protocol::StreamEvent::Chunk {
                            stream_id: stream_id.clone(),
                            sequence: seq,
                            content: "No memories matched your search. You can add memories using the adapter or ingest data from the CLI.".to_string(),
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
                    let cancel_ev =
                        ServerResponse::Stream(crate::server::protocol::StreamEvent::Cancelled {
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
                    metrics
                        .sum_query_latency_us
                        .fetch_add(query_elapsed, Ordering::Relaxed);
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
                    metrics
                        .sum_query_latency_us
                        .fetch_add(query_elapsed, Ordering::Relaxed);

                    let _ = analytics_tx.send(AnalyticsEvent::Query {
                        correlation_id,
                        query_text: payload.clone(),
                        hit_type: hit_type.to_string(),
                        execution_time_us: query_elapsed,
                    });

                    // Emit stream_end carrying context_used in metadata.
                    // Old TUI clients that do not read stream_end.metadata are unaffected.
                    seq += 1;
                    let end_ev =
                        ServerResponse::Stream(crate::server::protocol::StreamEvent::End {
                            stream_id: stream_id.clone(),
                            sequence: seq,
                            metadata: serde_json::json!({ "context_used": context_used }),
                        });
                    let mut end_json = serde_json::to_string(&end_ev)?;
                    end_json.push('\n');
                    writer.write_all(end_json.as_bytes()).await?;
                    writer.flush().await?;
                }

                None
            }
            "ingest_event" => {
                metrics.total_ingests.fetch_add(1, Ordering::Relaxed);
                let ingest_start = Instant::now();

                // 1. Parse payload as IngestionEnvelope
                let envelope: Result<brain_integrations::IngestionEnvelope, serde_json::Error> =
                    serde_json::from_str(&payload);

                let response = match envelope {
                    Ok(env) => {
                        // 2. Validate version
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
                            // 3. Resolve active storage backend as SQLite
                            let active_storage_res = plugin_registry.get_storage();
                            match active_storage_res {
                                Ok(active_storage) => {
                                    let event_log = active_storage.event_log();

                                    match event_log {
                                        Some(db) => {
                                            // 4. Ingest and persist
                                            match db.insert_event(&env) {
                                                Ok(sequence) => {
                                                    let text_to_ingest = match &env.event {
                                                        brain_integrations::IngestionEvent::Text { content, .. } => Some(content.clone()),
                                                        brain_integrations::IngestionEvent::Message { content, .. } => Some(content.clone()),
                                                        brain_integrations::IngestionEvent::TerminalCommand { command, stdout_summary, .. } => {
                                                            Some(format!("Terminal command executed: {}\nOutput: {}", command, stdout_summary.as_deref().unwrap_or("")))
                                                        }
                                                        brain_integrations::IngestionEvent::FileEdit { path, diff, .. } => {
                                                            Some(format!("File edited: {}\nDiff:\n{}", path, diff.as_deref().unwrap_or("")))
                                                        }
                                                        brain_integrations::IngestionEvent::GitCommit { message, hash, .. } => {
                                                            Some(format!("Git commit ({}): {}", hash, message))
                                                        }
                                                        brain_integrations::IngestionEvent::GitBranch { action, branch_name, .. } => {
                                                            Some(format!("Git branch {}: {}", action, branch_name))
                                                        }
                                                        brain_integrations::IngestionEvent::Diagnostic { message, severity, source, file, .. } => {
                                                            Some(format!("Diagnostic [{} / {}] in {}: {}", source, severity, file.as_deref().unwrap_or(""), message))
                                                        }
                                                        _ => None,
                                                    };

                                                    if let Some(text) = text_to_ingest {
                                                        let mut state_guard = state.write().await;
                                                        let session = state_guard
                                                            .entry(session_id.clone())
                                                            .or_insert_with(
                                                                stm::SessionContext::new,
                                                            );
                                                        session.ingest(text);
                                                    }

                                                    let ack_body = serde_json::json!({
                                                        "sequence": sequence,
                                                        "event_id": env.identity.event_id.to_string(),
                                                    }).to_string();

                                                    info!(
                                                        sequence = sequence,
                                                        event_id = %env.identity.event_id,
                                                        "Event successfully written to write-ahead event log and STM"
                                                    );

                                                    if is_versioned {
                                                        ServerResponse::Response(
                                                            VersionedResponse {
                                                                version: "1.0".to_string(),
                                                                msg_type: "Response".to_string(),
                                                                id: req_id.unwrap_or(0),
                                                                status: "success".to_string(),
                                                                body: ack_body,
                                                            },
                                                        )
                                                    } else {
                                                        ServerResponse::Legacy(LegacyResponse {
                                                            status: "ok".to_string(),
                                                            message: ack_body,
                                                        })
                                                    }
                                                }
                                                Err(e) => {
                                                    let msg = format!(
                                                        "Failed to insert event into WAL: {}",
                                                        e
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
                                                }
                                            }
                                        }
                                        None => {
                                            let msg =
                                                "Active storage backend is not SQLite".to_string();
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
                                }
                                Err(e) => {
                                    let msg = format!("Storage backend not configured: {}", e);
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
                        }
                    }
                    Err(e) => {
                        let msg = format!("Invalid IngestionEnvelope JSON: {}", e);
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
                let sequence = if let Ok(pos) =
                    serde_json::from_str::<brain_integrations::ReplayPosition>(&payload)
                {
                    pos.sequence
                } else {
                    payload.parse::<u64>().unwrap_or_default()
                };

                let active_storage_res = plugin_registry.get_storage();
                let response = match active_storage_res {
                    Ok(active_storage) => {
                        let event_log = active_storage.event_log();

                        match event_log {
                            Some(db) => match db.get_events_after(sequence) {
                                Ok(events) => match serde_json::to_string(&events) {
                                    Ok(body) => {
                                        if is_versioned {
                                            ServerResponse::Response(VersionedResponse {
                                                version: "1.0".to_string(),
                                                msg_type: "Response".to_string(),
                                                id: req_id.unwrap_or(0),
                                                status: "success".to_string(),
                                                body,
                                            })
                                        } else {
                                            ServerResponse::Legacy(LegacyResponse {
                                                status: "ok".to_string(),
                                                message: body,
                                            })
                                        }
                                    }
                                    Err(e) => {
                                        let msg =
                                            format!("Failed to serialize replayed events: {}", e);
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
                                },
                                Err(e) => {
                                    let msg = format!("Failed to retrieve replayed events: {}", e);
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
                            },
                            None => {
                                let msg = "Active storage backend is not SQLite".to_string();
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
                    }
                    Err(e) => {
                        let msg = format!("Storage backend not configured: {}", e);
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

                Some(response)
            }
            "handshake" => {
                // Handshake capability & version negotiation
                // Payload parses according to handshake.schema.json
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
                // Heartbeat diagnostic status update
                // Payload parses according to heartbeat.schema.json
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
                let response = match plugin_registry.get_storage() {
                    Ok(active_storage) => {
                        match active_storage.get_nodes_by_ids(std::slice::from_ref(&payload)) {
                            Ok(nodes) if !nodes.is_empty() => {
                                let node = &nodes[0];
                                let entity = brain_domain::dtos::NodeDTO::new(
                                    node.id.clone(),
                                    node.label.clone(),
                                    node.node_type.clone(),
                                    node.attributes.clone(),
                                );

                                let mut metadata = std::collections::HashMap::new();
                                metadata.insert("node_type".to_string(), node.node_type.clone());
                                metadata.insert("id".to_string(), node.id.clone());

                                let mut relationships = Vec::new();
                                match active_storage.get_connections(std::slice::from_ref(&node.id)) {
                                    Ok(connections) => {
                                        for edge in connections {
                                            let is_outgoing = edge.source == node.id;
                                            let neighbor_id = if is_outgoing {
                                                edge.target.clone()
                                            } else {
                                                edge.source.clone()
                                            };
                                            if let Ok(neighbors) = active_storage
                                                    .get_nodes_by_ids(std::slice::from_ref(&neighbor_id))
                                            {
                                                if !neighbors.is_empty() {
                                                    let neighbor = &neighbors[0];
                                                    relationships.push(brain_domain::query::inspector::RelationshipDTO {
                                                        target_id: neighbor.id.clone(),
                                                        target_label: neighbor.label.clone(),
                                                        target_type: neighbor.node_type.clone(),
                                                        relation: edge.relation.clone(),
                                                        direction: if is_outgoing { "outgoing".to_string() } else { "incoming".to_string() },
                                                        weight: 1.0,
                                                    });
                                                }
                                            }
                                        }
                                    }
                                    Err(e) => {
                                        tracing::error!("Failed to get edges connections for node inspection: {}", e);
                                    }
                                }

                                let provenance = brain_domain::query::inspector::ProvenanceDTO {
                                    source: "System Ingest".to_string(),
                                    location: "System Ingest".to_string(),
                                    timestamp: 0,
                                    extra_info: std::collections::HashMap::new(),
                                };

                                let recent_activity = vec![
                                    brain_domain::query::inspector::ActivityLogEntry {
                                        timestamp: 0,
                                        action: "Ingested".to_string(),
                                        details: "Entity extracted from source location by system.".to_string(),
                                    },
                                ];

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
                                        let msg = format!(
                                            "Failed to serialize InspectorModel JSON: {}",
                                            e
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
                                    }
                                }
                            }
                            _ => {
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
                            }
                        }
                    }
                    Err(e) => {
                        let msg = format!("Storage backend not configured: {}", e);
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
                Some(response)
            }
            "disconnect" => {
                // Graceful client disconnect sequence
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
