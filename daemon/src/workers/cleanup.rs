use std::collections::HashMap;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Instant;
use tracing::{error, info, info_span};

use crate::plugins::PluginRegistry;
use crate::workers::shadow;
use crate::{DaemonMetrics, GlobalState};

fn get_kpp_mode(registry: &PluginRegistry) -> brain_domain::bkf::KppMode {
    let mode_str = if let Ok(val) = std::env::var("KPP_MODE") {
        val
    } else {
        registry.config.kpp_mode.clone()
    };
    match mode_str.to_lowercase().as_str() {
        "disabled" => brain_domain::bkf::KppMode::Disabled,
        "active" => brain_domain::bkf::KppMode::Active,
        _ => brain_domain::bkf::KppMode::Shadow,
    }
}

async fn run_kpp_pipeline(
    session_id: &str,
    epoch: u64,
    nodes: &[crate::stm::TempNode],
    storage: &Arc<dyn crate::plugins::traits::StorageBackend>,
    write_to_db: bool,
) -> Result<brain_domain::bkf::CompiledKnowledge, String> {
    use brain_domain::bkf::{
        Observation, ConversationObservation, ObservationIR,
        KnowledgeCompiler, KnowledgeOptimizer, SqliteProjection
    };
    use brain_domain::DomainEvent;

    let obs_id = format!("obs-stm-{}-{}", session_id, epoch);

    // Event 1: ObservationReceived
    let _ = storage.log_kpp_event(&DomainEvent::KppObservationReceived {
        id: obs_id.clone(),
        source: "stm".to_string(),
    });

    // Parse stm nodes into combined text
    let combined_text = nodes
        .iter()
        .map(|n| n.content.as_str())
        .collect::<Vec<_>>()
        .join("\n");

    let obs = Observation::Conversation(ConversationObservation {
        conversation_id: format!("stm-epoch-{}", epoch),
        session_id: session_id.to_string(),
        prompt: combined_text,
        response: None,
    });

    let obs_ir = ObservationIR::parse(
        obs_id.clone(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
        obs,
        HashMap::new(),
    );

    // Event 2: ObservationParsed
    let _ = storage.log_kpp_event(&DomainEvent::KppObservationParsed {
        id: obs_id.clone(),
        nodes_count: obs_ir.metadata.len(),
        edges_count: 0,
    });

    // Event 3: CompilationStarted
    let _ = storage.log_kpp_event(&DomainEvent::KppCompilationStarted {
        id: obs_id.clone(),
    });

    // Compile
    let compiler = KnowledgeCompiler::new_default();
    let compile_res = compiler.compile(&obs_ir).map_err(|e| e.to_string())?;

    // Event 4: CompilationCompleted
    let _ = storage.log_kpp_event(&DomainEvent::KppCompilationCompleted {
        id: obs_id.clone(),
        nodes_count: compile_res.output.nodes.len(),
        edges_count: compile_res.output.edges.len(),
        diagnostics_count: compile_res.diagnostics.len(),
    });

    // Optimize
    let optimizer = KnowledgeOptimizer::new_default();
    let optimize_res = optimizer.optimize(compile_res.output).map_err(|e| e.to_string())?;

    // Event 5: OptimizationCompleted
    let _ = storage.log_kpp_event(&DomainEvent::KppOptimizationCompleted {
        id: obs_id.clone(),
        nodes_count: optimize_res.output.nodes.len(),
        edges_count: optimize_res.output.edges.len(),
        diagnostics_count: optimize_res.diagnostics.len(),
    });

    // Compute SQLite Projection Deltas
    let sqlite_projection = SqliteProjection;
    let deltas = sqlite_projection.calculate_delta(None, &optimize_res.output).map_err(|e| e.to_string())?;

    // Event 6: ProjectionCalculated
    let _ = storage.log_kpp_event(&DomainEvent::KppProjectionCalculated {
        id: obs_id.clone(),
        sqlite_ops_count: deltas.len(),
    });

    if write_to_db && !deltas.is_empty() {
        storage.apply_kpp_ops(&deltas)?;
        // Event 7: ProjectionApplied
        let _ = storage.log_kpp_event(&DomainEvent::KppProjectionApplied {
            id: obs_id.clone(),
        });
    }

    Ok(optimize_res.output)
}

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

            let active_storage = match plugin_registry.get_storage() {
                Ok(storage) => storage,
                Err(e) => {
                    error!("Failed to resolve active storage backend: {}", e);
                    continue;
                }
            };

            let mode = get_kpp_mode(&plugin_registry);
            let mut legacy_graph = None;
            let mut kpp_graph = None;

            // 1. Run Legacy Pipeline if Enabled or Shadow
            if mode == brain_domain::bkf::KppMode::Disabled || mode == brain_domain::bkf::KppMode::Shadow {
                let active_extractor = match plugin_registry.get_extractor() {
                    Ok(extractor) => extractor,
                    Err(e) => {
                        error!("Failed to resolve active extractor: {}", e);
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
                        legacy_graph = Some(graph.clone());
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

            // 2. Run KPP Pipeline if Enabled (Active or Shadow)
            if mode == brain_domain::bkf::KppMode::Active || mode == brain_domain::bkf::KppMode::Shadow {
                let write_to_db = mode == brain_domain::bkf::KppMode::Active;
                let storage_clone = Arc::clone(&active_storage);
                let nodes_clone = nodes.clone();
                let sess_id = session_id.clone();

                let kpp_result = run_kpp_pipeline(&sess_id, epoch, &nodes_clone, &storage_clone, write_to_db).await;

                match kpp_result {
                    Ok(compiled) => {
                        kpp_graph = Some(compiled);
                        info!(
                            nodes_count = kpp_graph.as_ref().unwrap().nodes.len(),
                            edges_count = kpp_graph.as_ref().unwrap().edges.len(),
                            "KPP Pipeline execution completed successfully (write={})",
                            write_to_db
                        );
                    }
                    Err(kpp_err) => {
                        error!("KPP Pipeline error: {}", kpp_err);
                    }
                }
            }

            // 3. Compare Legacy and KPP if running in Shadow Mode
            if mode == brain_domain::bkf::KppMode::Shadow {
                if let (Some(legacy), Some(kpp)) = (&legacy_graph, &kpp_graph) {
                    let diff = shadow::ShadowComparator::compare(legacy, kpp);
                    if !diff.mismatches.is_empty() {
                        info!(
                            mismatches_count = diff.mismatches.len(),
                            "KPP Shadow Mode comparison finished: detected graph differences!"
                        );
                        for mismatch in &diff.mismatches {
                            info!("  Difference details: {:?}", mismatch);
                        }
                    } else {
                        info!("KPP Shadow Mode comparison finished: 100% equivalence verified!");
                    }
                }
            }

            // 4. Run Reflection Engine and Planner if enabled
            if plugin_registry.config.enable_reflection {
                if let Some(ref kpp) = kpp_graph {
                    let reflection_engine = brain_domain::bkf::ReflectionEngine::new();
                    let planner = brain_domain::bkf::Planner::new();
                    let findings = reflection_engine.analyze(kpp);
                    let plan = planner.plan(&findings);

                    info!(
                        findings_count = findings.items.len(),
                        operations_count = plan.operations.len(),
                        "KPP Offline Reflection critique completed."
                    );
                    if !findings.items.is_empty() {
                        info!("  Critique Findings: {:?}", findings.items);
                        info!("  Suggested Rewrite Plan Rationale: {}", plan.rationale);
                        info!("  Suggested Rewrite Plan Operations: {:?}", plan.operations);
                    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::sqlite::LtmDatabase;
    use crate::storage::{ExtractedEdge, ExtractedGraph, ExtractedNode};
    use crate::stm::TempNode;
    use brain_domain::bkf::{IRNode, IREdge, KnowledgeLifecycle, KnowledgeValidity, KnowledgeVersionState, CompiledKnowledge};

    #[tokio::test]
    async fn test_kpp_pipeline_execution_in_cleanup() {
        let db = Arc::new(LtmDatabase::new(":memory:").unwrap());
        let backend: Arc<dyn crate::plugins::traits::StorageBackend> = db.clone();

        let temp_nodes = vec![
            TempNode {
                id: "n1".to_string(),
                epoch: 1,
                content: "entity: SQLite [Database]\nentity: Postgres [Database]\nrelation: SQLite -> Postgres [depends_on]".to_string(),
                timestamp: 1700000000,
            }
        ];

        let result = run_kpp_pipeline("sess-1", 1, &temp_nodes, &backend, true).await;
        if let Err(e) = &result {
            println!("run_kpp_pipeline error: {}", e);
        }
        assert!(result.is_ok());
        let compiled = result.unwrap();

        // Verify entity and relation exist in the compiled graph
        assert!(compiled.nodes.iter().any(|n| n.id == "node-sqlite"));
        assert!(compiled.edges.iter().any(|e| e.source == "node-sqlite" && e.target == "node-postgres"));

        // Verify event log contains logged events
        let conn_guard = db.conn.lock().unwrap();
        let count: i64 = conn_guard.query_row("SELECT COUNT(*) FROM event_log", [], |r| r.get(0)).unwrap();
        assert!(count >= 5, "Expected at least 5 logged KPP pipeline events, found {}", count);
    }

    #[test]
    fn test_shadow_mode_comparator_perfect_match() {
        let legacy = ExtractedGraph {
            nodes: vec![
                ExtractedNode {
                    id: "node-sqlite".to_string(),
                    label: "SQLite".to_string(),
                    node_type: "Database".to_string(),
                    attributes: serde_json::Value::Object(serde_json::Map::new()),
                }
            ],
            edges: vec![
                ExtractedEdge {
                    source: "node-sqlite".to_string(),
                    target: "node-postgres".to_string(),
                    relation: "depends_on".to_string(),
                }
            ],
        };

        let compiled = CompiledKnowledge {
            nodes: vec![
                IRNode {
                    id: "node-sqlite".to_string(),
                    label: "SQLite".to_string(),
                    entity_type: "Database".to_string(),
                    attributes: serde_json::Map::new(),
                    lifecycle: KnowledgeLifecycle::Observed,
                    validity: KnowledgeValidity::Unverified,
                    version_state: KnowledgeVersionState::Current,
                }
            ],
            edges: vec![
                IREdge {
                    id: "edge-1".to_string(),
                    source: "node-sqlite".to_string(),
                    target: "node-postgres".to_string(),
                    relation: "depends_on".to_string(),
                    weight: 1.0,
                    lifecycle: KnowledgeLifecycle::Observed,
                    validity: KnowledgeValidity::Unverified,
                    version_state: KnowledgeVersionState::Current,
                }
            ],
        };

        let diff = shadow::ShadowComparator::compare(&legacy, &compiled);
        assert!(diff.mismatches.is_empty(), "Expected no mismatches, found: {:?}", diff.mismatches);
    }

    #[test]
    fn test_shadow_mode_comparator_with_mismatches() {
        let legacy = ExtractedGraph {
            nodes: vec![
                ExtractedNode {
                    id: "node-sqlite".to_string(),
                    label: "SQLite".to_string(),
                    node_type: "Database".to_string(),
                    attributes: serde_json::Value::Object(serde_json::Map::new()),
                },
                ExtractedNode {
                    id: "node-postgres".to_string(),
                    label: "Postgres".to_string(),
                    node_type: "Database".to_string(),
                    attributes: serde_json::Value::Object(serde_json::Map::new()),
                }
            ],
            edges: vec![
                ExtractedEdge {
                    source: "node-sqlite".to_string(),
                    target: "node-postgres".to_string(),
                    relation: "depends_on".to_string(),
                }
            ],
        };

        // KPP compiled graph missing Postgres and missing the depends_on relationship
        let compiled = CompiledKnowledge {
            nodes: vec![
                IRNode {
                    id: "node-sqlite".to_string(),
                    label: "SQLite".to_string(),
                    entity_type: "Database".to_string(),
                    attributes: serde_json::Map::new(),
                    lifecycle: KnowledgeLifecycle::Observed,
                    validity: KnowledgeValidity::Unverified,
                    version_state: KnowledgeVersionState::Current,
                }
            ],
            edges: Vec::new(),
        };

        let diff = shadow::ShadowComparator::compare(&legacy, &compiled);
        assert_eq!(diff.mismatches.len(), 2);
        assert!(diff.mismatches.contains(&shadow::DiffItem::MissingEntity {
            id: "node-postgres".to_string(),
            label: "Postgres".to_string(),
        }));
        assert!(diff.mismatches.contains(&shadow::DiffItem::MissingRelationship {
            source: "node-sqlite".to_string(),
            target: "node-postgres".to_string(),
            relation: "depends_on".to_string(),
        }));
    }
}
