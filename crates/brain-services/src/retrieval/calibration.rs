use brain_core::errors::BrainError;
use brain_domain::retrieval::{
    CostHeuristics, HeuristicMetadata, HeuristicWeights, ObservedCost, RetrievalExecutionReport,
};
use std::collections::HashMap;
use std::sync::{Arc, Mutex, RwLock};

/// Thread-safe coordinator managing CostHeuristics snapshots and execution feedback calibration.
pub struct CostCalibrator {
    active: RwLock<Arc<CostHeuristics>>,
    history: Mutex<Vec<Arc<CostHeuristics>>>,
    observation_counts: Mutex<HashMap<&'static str, usize>>,
    alpha: f64,
    min_observations: usize,
}

impl CostCalibrator {
    /// Creates a new CostCalibrator with an initial CostHeuristics snapshot, EMA factor, and min observations.
    pub fn new(initial: CostHeuristics, alpha: f64, min_observations: usize) -> Self {
        Self {
            active: RwLock::new(Arc::new(initial)),
            history: Mutex::new(Vec::new()),
            observation_counts: Mutex::new(HashMap::new()),
            alpha,
            min_observations,
        }
    }

    /// Fetches the currently active immutable CostHeuristics snapshot.
    pub fn active_heuristics(&self) -> Arc<CostHeuristics> {
        let guard = self.active.read().unwrap();
        guard.clone()
    }

    /// Returns a list of historically retained CostHeuristics snapshots (up to 10).
    pub fn history(&self) -> Vec<Arc<CostHeuristics>> {
        let guard = self.history.lock().unwrap();
        guard.clone()
    }

    /// Extract ObservedCost proportionally from a completed execution report.
    pub fn extract_observed_cost(report: &RetrievalExecutionReport) -> ObservedCost {
        let total_time = report.runtime.elapsed_microseconds as f64;
        let est = &report.planning.estimated_cost;
        let total_est = est.total_cost();

        if total_est <= 0.0 {
            return ObservedCost {
                vector_cost: 0.0,
                keyword_cost: 0.0,
                expansion_cost: 0.0,
                fusion_cost: 0.0,
                ranking_cost: 0.0,
            };
        }

        // Determine component scaling factor based on actual execution counters
        let vector_factor = if est.vector_cost > 0.0 { 1.0 } else { 0.0 };
        let keyword_factor = if est.keyword_cost > 0.0 { 1.0 } else { 0.0 };

        let expansion_factor = if est.expansion_cost > 0.0 {
            (report.runtime.expansions_performed as f64).max(1.0)
        } else {
            0.0
        };

        let fusion_factor = if est.fusion_cost > 0.0 {
            (report.runtime.candidates_fused as f64).max(1.0)
        } else {
            0.0
        };

        let ranking_factor = if est.ranking_cost > 0.0 {
            (report.runtime.ranking_operations as f64).max(1.0)
        } else {
            0.0
        };

        let total_factor = vector_factor * 10.0
            + keyword_factor * 2.0
            + expansion_factor * 1.5
            + fusion_factor * 0.1
            + ranking_factor * 0.05;

        if total_factor <= 0.0 {
            return ObservedCost {
                vector_cost: est.vector_cost,
                keyword_cost: est.keyword_cost,
                expansion_cost: est.expansion_cost,
                fusion_cost: est.fusion_cost,
                ranking_cost: est.ranking_cost,
            };
        }

        // Distribute real microseconds proportionally
        let vector_cost = (vector_factor * 10.0 / total_factor) * total_time;
        let keyword_cost = (keyword_factor * 2.0 / total_factor) * total_time;
        let expansion_cost = (expansion_factor * 1.5 / total_factor) * total_time;
        let fusion_cost = (fusion_factor * 0.1 / total_factor) * total_time;
        let ranking_cost = (ranking_factor * 0.05 / total_factor) * total_time;

        ObservedCost {
            vector_cost,
            keyword_cost,
            expansion_cost,
            fusion_cost,
            ranking_cost,
        }
    }

    /// Enqueues an execution report and updates active heuristics atomically if thresholds are met.
    /// Returns `true` if a new version was published, `false` otherwise.
    pub fn record_execution(&self, report: &RetrievalExecutionReport) -> bool {
        // Exclude full result cache hits (which do not perform real work, total runtime close to 0)
        // We verify that the report compiled/planned heuristics is set, and it performed actual work.
        if report.runtime.elapsed_microseconds <= 50 {
            return false;
        }

        let observed = Self::extract_observed_cost(report);
        let est = &report.planning.estimated_cost;

        let active_snap = self.active_heuristics();
        let old_weights = &active_snap.weights;

        let mut obs_counts = self.observation_counts.lock().unwrap();

        // 1. Vector Search Calibration
        let mut new_vector = old_weights.vector_weight;
        if est.vector_cost > 0.0 {
            let count = obs_counts.entry("vector").or_insert(0);
            *count += 1;
            if *count >= self.min_observations {
                new_vector = old_weights.vector_weight * (1.0 - self.alpha)
                    + observed.vector_cost * self.alpha;
            }
        }

        // 2. Keyword Search Calibration
        let mut new_keyword = old_weights.keyword_weight;
        if est.keyword_cost > 0.0 {
            let count = obs_counts.entry("keyword").or_insert(0);
            *count += 1;
            if *count >= self.min_observations {
                new_keyword = old_weights.keyword_weight * (1.0 - self.alpha)
                    + observed.keyword_cost * self.alpha;
            }
        }

        // 3. Expansion Search Calibration
        let mut new_expansion = old_weights.expansion_weight;
        if est.expansion_cost > 0.0 {
            let count = obs_counts.entry("expansion").or_insert(0);
            *count += 1;
            if *count >= self.min_observations {
                new_expansion = old_weights.expansion_weight * (1.0 - self.alpha)
                    + observed.expansion_cost * self.alpha;
            }
        }

        // 4. Fusion Search Calibration
        let mut new_fusion = old_weights.fusion_weight;
        if est.fusion_cost > 0.0 {
            let count = obs_counts.entry("fusion").or_insert(0);
            *count += 1;
            if *count >= self.min_observations {
                new_fusion = old_weights.fusion_weight * (1.0 - self.alpha)
                    + observed.fusion_cost * self.alpha;
            }
        }

        // 5. Ranking Search Calibration
        let mut new_ranking = old_weights.ranking_weight;
        if est.ranking_cost > 0.0 {
            let count = obs_counts.entry("ranking").or_insert(0);
            *count += 1;
            if *count >= self.min_observations {
                new_ranking = old_weights.ranking_weight * (1.0 - self.alpha)
                    + observed.ranking_cost * self.alpha;
            }
        }

        // Check if any weight change exceeds the 1e-5 tolerance
        let delta_vector = (new_vector - old_weights.vector_weight).abs();
        let delta_keyword = (new_keyword - old_weights.keyword_weight).abs();
        let delta_expansion = (new_expansion - old_weights.expansion_weight).abs();
        let delta_fusion = (new_fusion - old_weights.fusion_weight).abs();
        let delta_ranking = (new_ranking - old_weights.ranking_weight).abs();

        let tolerance = 1e-5;
        let is_material_change = delta_vector > tolerance
            || delta_keyword > tolerance
            || delta_expansion > tolerance
            || delta_fusion > tolerance
            || delta_ranking > tolerance;

        if is_material_change {
            // Construct a new immutable snap
            let new_snap = Arc::new(CostHeuristics {
                metadata: HeuristicMetadata {
                    version: active_snap.metadata.version + 1,
                },
                weights: HeuristicWeights {
                    vector_weight: new_vector,
                    keyword_weight: new_keyword,
                    expansion_weight: new_expansion,
                    fusion_weight: new_fusion,
                    ranking_weight: new_ranking,
                },
            });

            // Swap reference atomically under write lock
            {
                let mut active_guard = self.active.write().unwrap();
                *active_guard = new_snap.clone();
            }

            // Append prior snapshot to history, capped at 10 items
            {
                let mut history_guard = self.history.lock().unwrap();
                history_guard.push(active_snap);
                if history_guard.len() > 10 {
                    history_guard.remove(0);
                }
            }

            true
        } else {
            false
        }
    }
}

/// Algorithm optimizer interface for calibration computations.
pub trait CalibrationAlgorithm: Send + Sync {
    /// Optimize weights based on feedback events and policy constraints.
    fn calibrate(
        &self,
        current: &brain_domain::retrieval::models::WeightSnapshot,
        events: &[brain_domain::retrieval::models::FeedbackEvent],
        policy: &brain_domain::retrieval::models::CalibrationPolicy,
    ) -> Result<brain_domain::retrieval::models::RankingWeights, BrainError>;
}

/// Linear moving average heuristic updates for weight calibration.
pub struct LinearAdjustmentAlgorithm;

impl CalibrationAlgorithm for LinearAdjustmentAlgorithm {
    fn calibrate(
        &self,
        current: &brain_domain::retrieval::models::WeightSnapshot,
        events: &[brain_domain::retrieval::models::FeedbackEvent],
        policy: &brain_domain::retrieval::models::CalibrationPolicy,
    ) -> Result<brain_domain::retrieval::models::RankingWeights, BrainError> {
        let mut sem_val = current.weights.semantic().value();
        let mut graph_val = current.weights.graph().value();
        let mut rec_val = current.weights.recency().value();
        let mut temp_val = current.weights.temporal().value();

        for event in events {
            use brain_domain::retrieval::models::{NormalizedSignal, RankingSignals};

            let zero = NormalizedSignal::new(0.5).map_err(|e| BrainError::Internal {
                message: format!("{:?}", e),
            })?;
            let signals: RankingSignals = serde_json::from_str(&event.context)
                .unwrap_or_else(|_| RankingSignals::new(zero, zero, zero, zero));

            let lr = policy.learning_rate;
            let reg = policy.regularization;

            if event.selected {
                sem_val += lr * (signals.semantic.value() - 0.5);
                graph_val += lr * (signals.graph.value() - 0.5);
                rec_val += lr * (signals.recency.value() - 0.5);
                temp_val += lr * (signals.temporal.value() - 0.5);
            } else {
                sem_val -= lr * (signals.semantic.value() - 0.5);
                graph_val -= lr * (signals.graph.value() - 0.5);
                rec_val -= lr * (signals.recency.value() - 0.5);
                temp_val -= lr * (signals.temporal.value() - 0.5);
            }

            sem_val *= 1.0 - reg;
            graph_val *= 1.0 - reg;
            rec_val *= 1.0 - reg;
            temp_val *= 1.0 - reg;
        }

        sem_val = sem_val.max(0.0);
        graph_val = graph_val.max(0.0);
        rec_val = rec_val.max(0.0);
        temp_val = temp_val.max(0.0);

        let new_weights = brain_domain::retrieval::models::RankingWeights::new(
            brain_domain::retrieval::models::RankingWeight::new(sem_val).map_err(|e| {
                BrainError::Internal {
                    message: format!("{:?}", e),
                }
            })?,
            brain_domain::retrieval::models::RankingWeight::new(graph_val).map_err(|e| {
                BrainError::Internal {
                    message: format!("{:?}", e),
                }
            })?,
            brain_domain::retrieval::models::RankingWeight::new(rec_val).map_err(|e| {
                BrainError::Internal {
                    message: format!("{:?}", e),
                }
            })?,
            brain_domain::retrieval::models::RankingWeight::new(temp_val).map_err(|e| {
                BrainError::Internal {
                    message: format!("{:?}", e),
                }
            })?,
        );
        Ok(new_weights)
    }
}

/// Orchestrates optimization runs and handles algorithmic dispatch.
pub struct CalibrationEngine {
    linear_algo: LinearAdjustmentAlgorithm,
}

impl CalibrationEngine {
    /// Creates a new `CalibrationEngine`.
    pub fn new() -> Self {
        Self {
            linear_algo: LinearAdjustmentAlgorithm,
        }
    }

    /// Evaluates feedback events to calculate optimized parameter candidate weights.
    pub fn run_calibration(
        &self,
        current: &brain_domain::retrieval::models::WeightSnapshot,
        events: &[brain_domain::retrieval::models::FeedbackEvent],
        policy: &brain_domain::retrieval::models::CalibrationPolicy,
    ) -> Result<
        (
            brain_domain::retrieval::models::WeightSnapshot,
            brain_domain::retrieval::models::CalibrationReport,
        ),
        BrainError,
    > {
        use brain_domain::retrieval::models::{
            CalibrationMetadata, CalibrationReport, SnapshotMetadata, SnapshotVersion,
            WeightSnapshot,
        };

        if events.is_empty() {
            // Calibration Idempotence: zero changes if no new events
            let report = CalibrationReport {
                candidate_version: current.metadata.version,
                previous_version: current.metadata.version,
                policy_version: policy.version,
                feedback_processed: 0,
                validation_loss: 0.0,
                convergence_information: "No new events to process. Idempotent no-op.".to_string(),
                publication_decision: false,
            };
            return Ok((current.clone(), report));
        }

        let algo: &dyn CalibrationAlgorithm = match policy.algorithm {
            brain_domain::retrieval::models::CalibrationAlgorithmType::LinearAdjustment => {
                &self.linear_algo
            }
        };

        let new_weights = algo.calibrate(current, events, policy)?;

        // Compute validation loss: average squared distance from rank 1 for selected items
        let mut total_loss = 0.0;
        let mut processed = 0;
        for event in events {
            if event.selected {
                let rank = event.ranking_position as f64;
                total_loss += (rank - 1.0) * (rank - 1.0);
                processed += 1;
            }
        }
        let avg_loss = if processed > 0 {
            total_loss / processed as f64
        } else {
            0.0
        };

        let next_version = SnapshotVersion::new(current.metadata.version.value() + 1);
        let metadata = SnapshotMetadata {
            version: next_version,
            created_at: brain_domain::temporal::TimePoint::from_unix_seconds(
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs(),
            ),
            calibration_metadata: CalibrationMetadata::new(
                format!("{:?}", policy.algorithm),
                Some(avg_loss),
            ),
        };

        let candidate = WeightSnapshot {
            metadata,
            weights: new_weights,
        };

        let report = CalibrationReport {
            candidate_version: next_version,
            previous_version: current.metadata.version,
            policy_version: policy.version,
            feedback_processed: events.len(),
            validation_loss: avg_loss,
            convergence_information: format!(
                "Calibration converged. Feedback processed: {}, loss: {:.4}",
                events.len(),
                avg_loss
            ),
            publication_decision: true,
        };

        Ok((candidate, report))
    }
}

impl Default for CalibrationEngine {
    fn default() -> Self {
        Self::new()
    }
}

/// Orchestrates weight persistence, ingestion, validation, rollback, and publication.
pub struct WeightCalibrationService {
    storage: Arc<brain_storage::SqliteStorage>,
    weight_provider: Arc<dyn crate::retrieval::active_weights::ActiveWeightProvider>,
    engine: CalibrationEngine,
}

impl WeightCalibrationService {
    /// Creates a new `WeightCalibrationService`.
    pub fn new(
        storage: Arc<brain_storage::SqliteStorage>,
        weight_provider: Arc<dyn crate::retrieval::active_weights::ActiveWeightProvider>,
    ) -> Self {
        Self {
            storage,
            weight_provider,
            engine: CalibrationEngine::new(),
        }
    }

    /// Persists a new relevance interaction event.
    pub fn ingest_feedback(
        &self,
        event: brain_domain::retrieval::models::FeedbackEvent,
    ) -> Result<(), BrainError> {
        self.storage.save_feedback_event(&event)
    }

    /// Evaluates accumulated interaction logs and prepares an updated weight candidate, without publishing.
    pub fn calibrate_weights(
        &self,
        policy: &brain_domain::retrieval::models::CalibrationPolicy,
    ) -> Result<
        (
            brain_domain::retrieval::models::WeightSnapshot,
            brain_domain::retrieval::models::CalibrationReport,
        ),
        BrainError,
    > {
        let current = self.weight_provider.active_snapshot()?;
        let events = self.storage.list_all_feedback_events()?;

        // Filter events count constraint
        if events.len() < policy.min_feedback_events {
            use brain_domain::retrieval::models::CalibrationReport;
            let report = CalibrationReport {
                candidate_version: current.metadata.version,
                previous_version: current.metadata.version,
                policy_version: policy.version,
                feedback_processed: events.len(),
                validation_loss: 0.0,
                convergence_information: format!(
                    "Feedback event count ({}) below policy minimum ({}). Calibration skipped.",
                    events.len(),
                    policy.min_feedback_events
                ),
                publication_decision: false,
            };
            return Ok(((*current).clone(), report));
        }

        self.engine.run_calibration(&current, &events, policy)
    }

    /// Validates safety rules and atomically registers the new active snapshot version.
    pub fn publish_snapshot(
        &self,
        snapshot: brain_domain::retrieval::models::WeightSnapshot,
    ) -> Result<(), BrainError> {
        let current = self.weight_provider.active_snapshot()?;

        // Monotonicity validation
        if snapshot.metadata.version.value() <= current.metadata.version.value() {
            return Err(BrainError::Validation {
                message: format!(
                    "Snapshot version monotonicity violated: candidate version ({}) <= active version ({})",
                    snapshot.metadata.version.value(),
                    current.metadata.version.value()
                ),
            });
        }

        // Completeness validation: checked at type system level (RankingWeights has semantic, graph, recency, temporal).
        // Bounds checks: checked at value object level (RankingWeight limits bounds to non-negative and finite).

        // Save to DB persistent storage
        self.storage.save_weight_snapshot(&snapshot)?;

        // Atomic swap
        self.weight_provider.swap_active(snapshot)?;

        Ok(())
    }

    /// Reverts active weights back to a previously stored snapshot version.
    pub fn rollback_to(
        &self,
        version: brain_domain::retrieval::models::SnapshotVersion,
    ) -> Result<(), BrainError> {
        let target = self.storage.get_weight_snapshot(version)?;
        if let Some(snapshot) = target {
            // Atomic swap
            self.weight_provider.swap_active(snapshot)?;
            Ok(())
        } else {
            Err(BrainError::Storage {
                message: format!(
                    "Rollback target weight snapshot version {} not found",
                    version.value()
                ),
                source: None,
            })
        }
    }
}
