use std::collections::HashMap;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Instant;
use tracing::{error, info, info_span};

use crate::plugins::PluginRegistry;
use crate::{DaemonMetrics, GlobalState};

pub async fn start_cleanup_worker(
    global_state: GlobalState,
    metrics: Arc<DaemonMetrics>,
    plugin_registry: Arc<PluginRegistry>,
) {
    loop {
        tokio::time::sleep(tokio::time::Duration::from_secs(30)).await;

        let mut batches_to_consolidate = HashMap::new();
        {
            let mut state = global_state.write().await;
            for (session_id, session) in state.iter_mut() {
                let old_epoch = session.rotate_epoch();
                let drained_nodes = session.drain_epoch(old_epoch);
                if !drained_nodes.is_empty() {
                    batches_to_consolidate.insert(session_id.clone(), (old_epoch, drained_nodes));
                }
                metrics.stm_queue_depth.store(
                    session.interaction_sliding_window.len() as u64,
                    Ordering::Relaxed,
                );
            }
        }

        for (session_id, (epoch, nodes)) in batches_to_consolidate {
            let indexing_span =
                info_span!("background_consolidation", session_id = %session_id, epoch = epoch);
            let _enter = indexing_span.enter();

            let active_extractor = match plugin_registry.get_extractor() {
                Ok(extractor) => extractor,
                Err(e) => {
                    error!("Failed to resolve active extractor: {}", e);
                    continue;
                }
            };

            let active_storage = match plugin_registry.get_storage() {
                Ok(storage) => storage,
                Err(e) => {
                    error!("Failed to resolve active storage backend: {}", e);
                    continue;
                }
            };

            let ffi_start = Instant::now();
            let extractor_clone = Arc::clone(&active_extractor);
            let nodes_clone = nodes.clone();

            let ffi_result = tokio::task::spawn_blocking(move || {
                let python_span = info_span!("python_extraction");
                let _enter = python_span.enter();
                extractor_clone.extract(&nodes_clone)
            })
            .await;

            let ffi_elapsed = ffi_start.elapsed().as_micros() as u64;
            metrics
                .sum_extraction_latency_us
                .fetch_add(ffi_elapsed, Ordering::Relaxed);

            match ffi_result {
                Ok(Ok(graph)) => {
                    let sql_start = Instant::now();
                    let db_span = info_span!("sqlite_upsert");
                    let _db_enter = db_span.enter();

                    let nodes_count = graph.nodes.len();
                    let edges_count = graph.edges.len();
                    let storage_clone = Arc::clone(&active_storage);
                    let nodes_to_embed = graph.nodes.clone();
                    let write_result = tokio::task::spawn_blocking(move || {
                        storage_clone.write_graph(&graph.nodes, &graph.edges)
                    })
                    .await;

                    match write_result {
                        Ok(Ok(())) => {
                            let sql_elapsed = sql_start.elapsed().as_micros() as u64;
                            metrics
                                .sum_sqlite_latency_us
                                .fetch_add(sql_elapsed, Ordering::Relaxed);

                            info!(
                                nodes_count = nodes_count,
                                edges_count = edges_count,
                                "Consolidation task successfully committed batch to storage backend"
                            );

                            if let Ok(emb_provider) = plugin_registry.get_embedding() {
                                if emb_provider.name() != "noop" {
                                    let emb_provider = Arc::clone(&emb_provider);
                                    let storage_clone2 = Arc::clone(&active_storage);
                                    let nodes_for_emb = nodes_to_embed.clone();
                                    tokio::task::spawn_blocking(move || {
                                        let mut embs = Vec::new();
                                        for node in &nodes_for_emb {
                                            let text = format!(
                                                "{} ({}) {}",
                                                node.label,
                                                node.node_type,
                                                serde_json::to_string(&node.attributes)
                                                    .unwrap_or_default()
                                            );
                                            match emb_provider.embed(&text) {
                                                Ok(vec) => {
                                                    embs.push((node.id.clone(), vec));
                                                }
                                                Err(e) => {
                                                    error!("Failed to generate embedding for node '{}': {}", node.id, e);
                                                }
                                            }
                                        }
                                        if !embs.is_empty() {
                                            if let Err(e) = storage_clone2.write_embeddings(&embs) {
                                                error!("Failed to write embeddings to storage backend: {}", e);
                                            } else {
                                                info!("Successfully generated and saved {} embeddings to database", embs.len());
                                            }
                                        }
                                    });
                                }
                            }
                        }
                        Ok(Err(db_err)) => {
                            error!(
                                "Failed to write session '{}' Epoch {} to storage backend: {}",
                                session_id, epoch, db_err
                            );
                        }
                        Err(join_err) => {
                            error!("Blocking join error for storage write: {}", join_err);
                        }
                    }
                }
                Ok(Err(extractor_err)) => {
                    error!("Extractor error: {}", extractor_err);
                }
                Err(join_err) => {
                    error!("Blocking join error for extraction: {}", join_err);
                }
            }
        }

        // Sync/Export
        let active_exporter = plugin_registry.get_exporter().ok();
        let active_storage = plugin_registry.get_storage().ok();

        if let (Some(exporter), Some(storage)) = (active_exporter, active_storage.clone()) {
            let exporter_clone = Arc::clone(&exporter);
            let storage_clone = Arc::clone(&storage);
            let export_res =
                tokio::task::spawn_blocking(move || exporter_clone.export(&*storage_clone)).await;

            match export_res {
                Ok(Ok(())) => {
                    info!("Incremental sync/export completed successfully");
                }
                Ok(Err(sync_err)) => {
                    error!("Failed to incrementally sync/export: {}", sync_err);
                }
                Err(join_err) => {
                    error!("Blocking join error for export: {}", join_err);
                }
            }
        }

        // Decay weights in storage backend
        if let Some(storage) = active_storage {
            let storage_clone = Arc::clone(&storage);
            let decay_res = tokio::task::spawn_blocking(move || {
                storage_clone.decay_weights(604800.0, 0.1) // 7-day half-life, 0.1 threshold
            })
            .await;

            match decay_res {
                Ok(Ok(())) => {
                    info!("Graph relationship decay sweep completed successfully");
                }
                Ok(Err(decay_err)) => {
                    error!("Failed to decay graph relationships: {}", decay_err);
                }
                Err(join_err) => {
                    error!("Blocking join error for decay: {}", join_err);
                }
            }
        }
    }
}
