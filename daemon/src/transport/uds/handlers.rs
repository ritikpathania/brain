use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Instant;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;

use brain_application::context::ExecutionContext;
use brain_application::dispatcher::{ApplicationRequest, ApplicationResponse, RequestDispatcher};

use crate::server::protocol::{
    ClientRequest, LegacyResponse, ServerResponse, VersionedError, VersionedEvent,
    VersionedResponse,
};
use crate::transport::uds::router::ProtocolRouter;
use crate::{DaemonMetrics, REQUEST_COUNTER};

/// Handles an active UDS client connection, decoding, routing, and executing requests.
pub async fn handle_connection(
    stream: UnixStream,
    metrics: Arc<DaemonMetrics>,
    dispatcher: Arc<RequestDispatcher>,
    app: Arc<brain_application::BrainApplication>,
) -> Result<(), Box<dyn std::error::Error>> {
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

        let (action, payload, req_id, is_versioned, workspace_context) = match &request {
            ClientRequest::Versioned(req) => (
                req.action.as_str(),
                req.body.as_str(),
                Some(req.id),
                true,
                &req.workspace_context,
            ),
            ClientRequest::Legacy(req) => (
                req.action.as_str(),
                req.payload.as_str(),
                None,
                false,
                &Vec::new(),
            ),
        };

        // Special UDS-specific commands handled at transport layer
        if action == "disconnect" {
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
            let mut w = writer.lock().await;
            w.write_all(response_json.as_bytes()).await?;
            w.flush().await?;
            return Ok(());
        }

        if action == "handshake" {
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
            let mut response_json = serde_json::to_string(&response)?;
            response_json.push('\n');
            let mut w = writer.lock().await;
            w.write_all(response_json.as_bytes()).await?;
            w.flush().await?;
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
                    for summary in results {
                        // Reconstruct QueryResultNode for backward compatibility
                        matches.push(crate::server::protocol::QueryResultNode {
                            id: summary.id.clone(),
                            label: summary.title.clone(),
                            node_type: "session_context".to_string(),
                            content: summary.body.clone(),
                            attributes: serde_json::to_value(&summary.metadata)
                                .unwrap_or(serde_json::json!({})),
                            score: 8000, // parity score
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
                        let output_line =
                            format!("**{}** — {} (High confidence)\n", node.id, node.label);
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
                    ServerResponse::Error(VersionedError {
                        version: "1.0".to_string(),
                        msg_type: "Error".to_string(),
                        id: req_id.unwrap_or(0),
                        status: "error".to_string(),
                        body: err_msg,
                    })
                } else {
                    ServerResponse::Legacy(LegacyResponse {
                        status: "error".to_string(),
                        message: err_msg,
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
                };

                let response = if is_versioned {
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
                    ServerResponse::Error(VersionedError {
                        version: "1.0".to_string(),
                        msg_type: "Error".to_string(),
                        id: req_id.unwrap_or(0),
                        status: "error".to_string(),
                        body: err_msg,
                    })
                } else {
                    ServerResponse::Legacy(LegacyResponse {
                        status: "error".to_string(),
                        message: err_msg,
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

    Ok(())
}
