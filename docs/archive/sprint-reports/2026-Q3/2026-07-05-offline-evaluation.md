# Offline Evaluation Framework (Phase 14) Refined Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement an offline evaluation framework to quantitatively compare candidate weight snapshots against baseline weight snapshots using domain-validated metric value objects (`NdcgScore`, `MrrScore`, `RecallScore`, `PrecisionScore`), dynamic `PublicationPolicy` strategies, and reproducible `EvaluationDataset` instances.

**Architecture:**
1. **Domain Value Objects**: `NdcgScore`, `MrrScore`, `RecallScore`, `PrecisionScore` validating values in `[0.0, 1.0]`.
2. **RelevanceJudgment**: Query relevance labels for candidate nodes.
3. **EvaluationTestCase & EvaluationDataset**: Reproducible collection of candidate lists and judgments.
4. **MetricCalculator**: Separated component computing metrics from ranked nodes and relevance judgments.
5. **PublicationPolicy**: Strategy trait (with `NoRegressionPolicy` as primary implementation) deciding `PublicationRecommendation`.
6. **EvaluationContext**: Aggregated parameters including dataset, depth K, normalizer context, publication policy, and repositories.
7. **EvaluationReport & EvaluationMetadata**: Audit-log grade report containing metrics comparisons, publication recommendations, and metadata context.
8. **OfflineEvaluator**: Coordinator service executing the evaluation context pipeline.

**Tech Stack:** Rust, `brain-domain`, `brain-services`

## Global Constraints
* Maintain 100% test coverage and ensure zero dependencies on async/infrastructure in `brain-domain`.
* Keep all public traits, structs, and methods fully documented with doc comments to satisfy `#![deny(missing_docs)]`.
* Follow strictly test-driven development (TDD) by writing tests first or immediately alongside changes.

---

### Task 1: Domain Metric Value Objects & EvaluationDataset
**Files:**
* Create: `crates/brain-domain/src/retrieval/evaluation.rs`
* Modify: `crates/brain-domain/src/retrieval/mod.rs`
* Test: `crates/brain-domain/tests/evaluation_domain_tests.rs`

**Interfaces:**
* Consumes: `NodeId`, `SnapshotVersion`
* Produces: `NdcgScore`, `MrrScore`, `RecallScore`, `PrecisionScore`, `RelevanceJudgment`, `EvaluationTestCase`, `EvaluationDataset`

- [ ] **Step 1: Write failing tests for Metric Value Objects**
  ```rust
  #[test]
  fn test_metric_value_objects_invariants() {
      use brain_domain::retrieval::evaluation::{NdcgScore, MrrScore};
      assert!(NdcgScore::new(0.5).is_ok());
      assert!(NdcgScore::new(-0.1).is_err());
      assert!(NdcgScore::new(1.1).is_err());
      assert!(NdcgScore::new(f64::NAN).is_err());
  }
  ```
- [ ] **Step 2: Run test to verify it fails**
  Run: `PYO3_PYTHON=$(pwd)/daemon/.venv/bin/python cargo test -p brain-domain --test evaluation_domain_tests`
  Expected: FAIL with compilation error
- [ ] **Step 3: Implement domain value objects and dataset**
  Write `crates/brain-domain/src/retrieval/evaluation.rs`:
  ```rust
  use crate::identifiers::NodeId;
  use crate::retrieval::models::SnapshotVersion;
  use crate::consolidation::MetricConstructionError;

  macro_rules! define_metric_score {
      ($name:ident, $doc:expr) => {
          #[doc = $doc]
          #[derive(Debug, Clone, Copy, PartialEq, PartialOrd, serde::Serialize, serde::Deserialize)]
          pub struct $name(f64);

          impl $name {
              /// Creates a new validated metric score between 0.0 and 1.0.
              pub fn new(val: f64) -> Result<Self, MetricConstructionError> {
                  if !val.is_finite() {
                      return Err(MetricConstructionError::NotFinite { val });
                  }
                  if val < 0.0 || val > 1.0 {
                      return Err(MetricConstructionError::OutOfRange { val, min: 0.0, max: 1.0 });
                  }
                  Ok(Self(val))
              }

              /// Accesses the underlying score.
              pub fn value(&self) -> f64 {
                  self.0
              }
          }
      };
  }

  define_metric_score!(NdcgScore, "Normalized Discounted Cumulative Gain score scaled [0.0, 1.0].");
  define_metric_score!(MrrScore, "Mean Reciprocal Rank score scaled [0.0, 1.0].");
  define_metric_score!(RecallScore, "Recall score scaled [0.0, 1.0].");
  define_metric_score!(PrecisionScore, "Precision score scaled [0.0, 1.0].");

  /// Relevance label mapping query-node pairs.
  #[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
  pub struct RelevanceJudgment {
      /// Target candidate node under evaluation.
      pub node_id: NodeId,
      /// Relevance score (0.0 for irrelevant, 1.0+ for relevant).
      pub score: f64,
  }

  /// Evaluation case containing query context and expected judgments.
  #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
  pub struct EvaluationTestCase {
      /// Search query context.
      pub query: String,
      /// Candidates available for ranking.
      pub candidates: Vec<crate::Node>,
      /// Associated temporal edges active during ranking.
      pub temporal_edges: Vec<crate::temporal::TemporalEdge>,
      /// Relevance judgments for the query.
      pub judgments: Vec<RelevanceJudgment>,
  }

  /// Immutable evaluation dataset package.
  #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
  pub struct EvaluationDataset {
      /// Unique name or ID of evaluation dataset.
      pub version: String,
      /// Test cases matching the dataset profile.
      pub cases: Vec<EvaluationTestCase>,
  }
  ```
  Expose `evaluation` submodule in `crates/brain-domain/src/retrieval/mod.rs`:
  ```rust
  /// Metric value objects, datasets, and report structures.
  pub mod evaluation;
  ```
- [ ] **Step 4: Run test to verify it passes**
  Run: `PYO3_PYTHON=$(pwd)/daemon/.venv/bin/python cargo test -p brain-domain --test evaluation_domain_tests`
  Expected: PASS
- [ ] **Step 5: Commit changes**
  Run: `git add crates/brain-domain && git commit -m "feat: add domain metric value objects, RelevanceJudgment, and EvaluationDataset"`

---

### Task 2: MetricCalculator & PublicationPolicy
**Files:**
* Create: `crates/brain-domain/src/retrieval/evaluation_policy.rs` (or add to `evaluation.rs` directly since they are domain objects)
* Modify: `crates/brain-domain/src/retrieval/evaluation.rs`
* Test: `crates/brain-domain/tests/evaluation_domain_tests.rs`

**Interfaces:**
* Consumes: `NdcgScore`, `MrrScore`, `RecallScore`, `PrecisionScore`
* Produces: `EvaluationMetrics`, `EvaluationComparison`, `MetricCalculator`, `PublicationRecommendation`, `PublicationPolicy` trait, `NoRegressionPolicy` implementation, `EvaluationMetadata`, `EvaluationReport`

- [ ] **Step 1: Write failing TDD tests in `evaluation_domain_tests.rs` for MetricCalculator**
  ```rust
  #[test]
  fn test_metric_calculator_ndcg_and_mrr() {
      // Setup ranked lists and judgments, verify output matches expected NDCG and MRR calculations
  }
  ```
- [ ] **Step 2: Run test to verify failure**
  Run: `PYO3_PYTHON=$(pwd)/daemon/.venv/bin/python cargo test -p brain-domain --test evaluation_domain_tests`
  Expected: FAIL
- [ ] **Step 3: Implement `EvaluationMetrics`, `EvaluationComparison`, `MetricCalculator`, `PublicationPolicy`, and `EvaluationReport` in `evaluation.rs`**
  Append to `crates/brain-domain/src/retrieval/evaluation.rs`:
  ```rust
  /// Holds all evaluated metric scores.
  #[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
  pub struct EvaluationMetrics {
      /// NDCG at depth K.
      pub ndcg_k: NdcgScore,
      /// Mean Reciprocal Rank.
      pub mrr: MrrScore,
      /// Recall at depth K.
      pub recall_k: RecallScore,
      /// Precision at depth K.
      pub precision_k: PrecisionScore,
  }

  /// Comparison between candidate model and baseline model performance.
  #[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
  pub struct EvaluationComparison {
      /// Baseline model performance metrics.
      pub baseline: EvaluationMetrics,
      /// Candidate model performance metrics.
      pub candidate: EvaluationMetrics,
      /// Difference in NDCG (candidate - baseline).
      pub ndcg_improvement: f64,
      /// Difference in MRR (candidate - baseline).
      pub mrr_improvement: f64,
  }

  /// Computes metrics over ranked nodes.
  pub struct MetricCalculator;

  impl MetricCalculator {
      /// Computes Precision@K, Recall@K, MRR, and NDCG@K for a ranked candidate set.
      pub fn compute_metrics(
          ranked: &[NodeId],
          judgments: &[RelevanceJudgment],
          k: usize,
      ) -> Result<EvaluationMetrics, MetricConstructionError> {
          let judgment_map: std::collections::HashMap<NodeId, f64> = judgments.iter()
              .map(|j| (j.node_id, j.score))
              .collect();

          let k_limit = std::cmp::min(k, ranked.len());
          let top_k = &ranked[..k_limit];

          let total_relevant = judgments.iter().filter(|j| j.score > 0.0).count();
          let mut relevant_retrieved = 0;
          for &id in top_k {
              if let Some(&score) = judgment_map.get(&id) {
                  if score > 0.0 {
                      relevant_retrieved += 1;
                  }
              }
          }

          let precision = if k > 0 { relevant_retrieved as f64 / k as f64 } else { 0.0 };
          let recall = if total_relevant > 0 {
              relevant_retrieved as f64 / total_relevant as f64
          } else {
              1.0
          };

          let mut rr = 0.0;
          for (idx, &id) in ranked.iter().enumerate() {
              if let Some(&score) = judgment_map.get(&id) {
                  if score > 0.0 {
                      rr = 1.0 / (idx + 1) as f64;
                      break;
                  }
              }
          }

          let mut dcg = 0.0;
          for (idx, &id) in top_k.iter().enumerate() {
              if let Some(&rel) = judgment_map.get(&id) {
                  dcg += (2.0f64.powf(rel) - 1.0) / ((idx + 2) as f64).log2();
              }
          }

          let mut ideal_judgments: Vec<f64> = judgments.iter().map(|j| j.score).collect();
          ideal_judgments.sort_by(|a, b| b.partial_cmp(a).unwrap_or(std::cmp::Ordering::Equal));
          let ideal_k_limit = std::cmp::min(k, ideal_judgments.len());

          let mut idcg = 0.0;
          for (idx, &rel) in ideal_judgments[..ideal_k_limit].iter().enumerate() {
              idcg += (2.0f64.powf(rel) - 1.0) / ((idx + 2) as f64).log2();
          }

          let ndcg = if idcg > 0.0 { dcg / idcg } else { 1.0 };

          Ok(EvaluationMetrics {
              ndcg_k: NdcgScore::new(ndcg)?,
              mrr: MrrScore::new(rr)?,
              recall_k: RecallScore::new(recall)?,
              precision_k: PrecisionScore::new(precision)?,
          })
      }
  }

  /// Recommendation decision for candidate publication.
  #[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
  pub enum PublicationRecommendation {
      /// Performance is sufficient; approved.
      Approve,
      /// Performance degraded; rejected.
      Reject {
          /// Reason for rejection.
          reason: String,
      },
  }

  /// Trait defining candidate publication validation strategies.
  pub trait PublicationPolicy: Send + Sync {
      /// Evaluates comparison metrics and yields a recommendation decision.
      fn evaluate_recommendation(&self, comparison: &EvaluationComparison) -> PublicationRecommendation;
      /// Returns the policy identifier.
      fn name(&self) -> &'static str;
  }

  /// Policy approving candidates as long as NDCG does not degrade.
  #[derive(Clone, Copy)]
  pub struct NoRegressionPolicy;

  impl PublicationPolicy for NoRegressionPolicy {
      fn evaluate_recommendation(&self, comparison: &EvaluationComparison) -> PublicationRecommendation {
          if comparison.ndcg_improvement >= 0.0 {
              PublicationRecommendation::Approve
          } else {
              PublicationRecommendation::Reject {
                  reason: format!("Candidate NDCG degraded by {:.4}", -comparison.ndcg_improvement),
              }
          }
      }

      fn name(&self) -> &'static str {
          "NoRegressionPolicy"
      }
  }

  /// Metadata defining parameters used during evaluation.
  #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
  pub struct EvaluationMetadata {
      /// Dataset version used.
      pub dataset_version: String,
      /// Evaluation timestamp.
      pub timestamp: u64,
      /// Depth cutoff parameter.
      pub k: usize,
      /// Normalizer strategy description.
      pub normalizer_strategy: String,
      /// Publication policy label.
      pub publication_policy: String,
  }

  /// Audit-grade evaluation comparison summary.
  #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
  pub struct EvaluationReport {
      /// Candidate weight snapshot version.
      pub candidate_version: SnapshotVersion,
      /// Baseline weight snapshot version.
      pub baseline_version: SnapshotVersion,
      /// Metric comparison details.
      pub comparison: EvaluationComparison,
      /// Recommendation outcome.
      pub recommendation: PublicationRecommendation,
      /// Context metadata configurations.
      pub metadata: EvaluationMetadata,
  }
  ```
- [ ] **Step 4: Run test to verify it passes**
  Run: `PYO3_PYTHON=$(pwd)/daemon/.venv/bin/python cargo test -p brain-domain --test evaluation_domain_tests`
  Expected: PASS
- [ ] **Step 5: Commit changes**
  Run: `git add crates/brain-domain && git commit -m "feat: implement MetricCalculator, NoRegressionPolicy, and EvaluationReport structures"`

---

### Task 3: Service OfflineEvaluator Implementation
**Files:**
* Create: `crates/brain-services/src/retrieval/evaluator.rs`
* Modify: `crates/brain-services/src/retrieval.rs`
* Modify: `crates/brain-services/src/lib.rs`
* Test: `crates/brain-services/tests/offline_evaluation_tests.rs`

**Interfaces:**
* Consumes: `EvaluationDataset`, `FeatureExtractor`, `FeatureNormalizer`, `PublicationPolicy`
* Produces: `EvaluationContext`, `OfflineEvaluator` service executing evaluation pipeline

- [ ] **Step 1: Write integration tests in `offline_evaluation_tests.rs`**
  ```rust
  #[test]
  fn test_offline_evaluator_lifecycle_and_determinism() {
      // Setup dataset, evaluator, baseline, candidate, verify report outputs are deterministically byte-for-byte identical
  }
  ```
- [ ] **Step 2: Run test to verify it fails**
  Run: `PYO3_PYTHON=$(pwd)/daemon/.venv/bin/python cargo test -p brain-services --test offline_evaluation_tests`
  Expected: FAIL
- [ ] **Step 3: Implement `EvaluationContext` and `OfflineEvaluator`**
  Write `crates/brain-services/src/retrieval/evaluator.rs`:
  ```rust
  use std::sync::Arc;
  use brain_core::errors::BrainError;
  use brain_core::repositories::RepositorySet;
  use brain_core::retrieval::RetrievalRequest;
  use brain_domain::retrieval::models::{LinearRankingModel, RankingModel, WeightSnapshot};
  use brain_domain::retrieval::features::{MinMaxNormalizer, FeatureNormalizer, NormalizationContext};
  use brain_domain::retrieval::evaluation::{
      EvaluationDataset, EvaluationMetrics, EvaluationComparison, PublicationPolicy,
      EvaluationMetadata, EvaluationReport, MetricCalculator, NdcgScore, MrrScore, RecallScore, PrecisionScore
  };
  use crate::retrieval::feature_extractor::{FeatureExtractor, DefaultFeatureExtractor};

  /// Bundles dependencies and context for running evaluations.
  pub struct EvaluationContext<'a> {
      /// Target dataset to evaluate on.
      pub dataset: &'a EvaluationDataset,
      /// Depth parameter K.
      pub k: usize,
      /// Normalizer strategy description.
      pub normalizer_strategy: NormalizationContext,
      /// Publication policy.
      pub publication_policy: &'a dyn PublicationPolicy,
      /// Source repositories context.
      pub repos: &'a dyn RepositorySet,
      /// Clock provider for timestamps.
      pub clock: &'a dyn brain_domain::temporal::Clock,
  }

  /// Service orchestrating offline evaluation of weight snapshots.
  pub struct OfflineEvaluator {
      extractor: Arc<dyn FeatureExtractor>,
      normalizer: Arc<dyn FeatureNormalizer>,
  }

  impl OfflineEvaluator {
      /// Creates a new `OfflineEvaluator`.
      pub fn new(extractor: Arc<dyn FeatureExtractor>, normalizer: Arc<dyn FeatureNormalizer>) -> Self {
          Self { extractor, normalizer }
      }

      /// Executes offline evaluations on the given context.
      pub fn evaluate(
          &self,
          candidate: &WeightSnapshot,
          baseline: &WeightSnapshot,
          context: &EvaluationContext,
      ) -> Result<EvaluationReport, BrainError> {
          let mut baseline_ndcg = 0.0;
          let mut baseline_mrr = 0.0;
          let mut baseline_recall = 0.0;
          let mut baseline_precision = 0.0;

          let mut candidate_ndcg = 0.0;
          let mut candidate_mrr = 0.0;
          let mut candidate_recall = 0.0;
          let mut candidate_precision = 0.0;

          let mut evaluated_cases = 0;
          let baseline_model = LinearRankingModel::new(baseline.weights.clone());
          let candidate_model = LinearRankingModel::new(candidate.weights.clone());

          for case in &context.dataset.cases {
              if case.candidates.is_empty() {
                  continue;
              }

              let request = RetrievalRequest {
                  session_id: brain_domain::SessionId::new(),
                  query: case.query.clone(),
                  limit: case.candidates.len(),
                  exclude_ids: std::collections::HashSet::new(),
                  deadline: None,
              };

              let raw = self.extractor.extract(&request, &case.candidates, &case.temporal_edges, context.repos)?;
              let norm = self.normalizer.normalize(&raw, &context.normalizer_strategy)
                  .map_err(|e| BrainError::Internal { message: format!("{:?}", e) })?;

              let b_ranked = self.rank_candidates(&case.candidates, &norm, &baseline_model);
              let c_ranked = self.rank_candidates(&case.candidates, &norm, &candidate_model);

              let b_met = MetricCalculator::compute_metrics(&b_ranked, &case.judgments, context.k)
                  .map_err(|e| BrainError::Internal { message: format!("{:?}", e) })?;
              let c_met = MetricCalculator::compute_metrics(&c_ranked, &case.judgments, context.k)
                  .map_err(|e| BrainError::Internal { message: format!("{:?}", e) })?;

              baseline_ndcg += b_met.ndcg_k.value();
              baseline_mrr += b_met.mrr.value();
              baseline_recall += b_met.recall_k.value();
              baseline_precision += b_met.precision_k.value();

              candidate_ndcg += c_met.ndcg_k.value();
              candidate_mrr += c_met.mrr.value();
              candidate_recall += c_met.recall_k.value();
              candidate_precision += c_met.precision_k.value();

              evaluated_cases += 1;
          }

          let eval_count = if evaluated_cases > 0 { evaluated_cases as f64 } else { 1.0 };

          let baseline_metrics = EvaluationMetrics {
              ndcg_k: NdcgScore::new(baseline_ndcg / eval_count)
                  .map_err(|e| BrainError::Internal { message: format!("{:?}", e) })?,
              mrr: MrrScore::new(baseline_mrr / eval_count)
                  .map_err(|e| BrainError::Internal { message: format!("{:?}", e) })?,
              recall_k: RecallScore::new(baseline_recall / eval_count)
                  .map_err(|e| BrainError::Internal { message: format!("{:?}", e) })?,
              precision_k: PrecisionScore::new(baseline_precision / eval_count)
                  .map_err(|e| BrainError::Internal { message: format!("{:?}", e) })?,
          };

          let candidate_metrics = EvaluationMetrics {
              ndcg_k: NdcgScore::new(candidate_ndcg / eval_count)
                  .map_err(|e| BrainError::Internal { message: format!("{:?}", e) })?,
              mrr: MrrScore::new(candidate_mrr / eval_count)
                  .map_err(|e| BrainError::Internal { message: format!("{:?}", e) })?,
              recall_k: RecallScore::new(candidate_recall / eval_count)
                  .map_err(|e| BrainError::Internal { message: format!("{:?}", e) })?,
              precision_k: PrecisionScore::new(candidate_precision / eval_count)
                  .map_err(|e| BrainError::Internal { message: format!("{:?}", e) })?,
          };

          let ndcg_improvement = candidate_metrics.ndcg_k.value() - baseline_metrics.ndcg_k.value();
          let mrr_improvement = candidate_metrics.mrr.value() - baseline_metrics.mrr.value();

          let comparison = EvaluationComparison {
              baseline: baseline_metrics,
              candidate: candidate_metrics,
              ndcg_improvement,
              mrr_improvement,
          };

          let recommendation = context.publication_policy.evaluate_recommendation(&comparison);

          let metadata = EvaluationMetadata {
              dataset_version: context.dataset.version.clone(),
              timestamp: context.clock.now().unix_seconds(),
              k: context.k,
              normalizer_strategy: format!("{:?}", context.normalizer_strategy),
              publication_policy: context.publication_policy.name().to_string(),
          };

          Ok(EvaluationReport {
              candidate_version: candidate.metadata.version.clone(),
              baseline_version: baseline.metadata.version.clone(),
              comparison,
              recommendation,
              metadata,
          })
      }

      fn rank_candidates(
          &self,
          candidates: &[brain_domain::Node],
          signals: &[brain_domain::retrieval::models::RankingSignals],
          model: &LinearRankingModel,
      ) -> Vec<brain_domain::NodeId> {
          let mut scored: Vec<(brain_domain::NodeId, f64)> = candidates.iter().enumerate().map(|(idx, node)| {
              let score = model.score(&signals[idx]);
              (node.id, score)
          }).collect();

          scored.sort_by(|a, b| {
              b.1.partial_cmp(&a.1)
                  .unwrap_or(std::cmp::Ordering::Equal)
                  .then_with(|| a.0.0.cmp(&b.0.0))
          });

          scored.into_iter().map(|(id, _)| id).collect()
      }
  }
  ```
  Declare modules in `retrieval.rs` and `lib.rs`.
- [ ] **Step 4: Run test to verify it passes**
  Run: `PYO3_PYTHON=$(pwd)/daemon/.venv/bin/python cargo test -p brain-services --test offline_evaluation_tests`
  Expected: PASS
- [ ] **Step 5: Commit changes**
  Run: `git add crates/brain-services && git commit -m "feat: implement OfflineEvaluator and EvaluationContext in brain-services"`

---

## Verification Plan

### Automated Tests
* Validate all tests across all modules pass cleanly:
  ```bash
  PYO3_PYTHON=$(pwd)/daemon/.venv/bin/python cargo test --all
  ```

### Invariants Verification
* **Ranking Determinism**: Verify calling `.evaluate` twice with identical context outputs a byte-for-byte identical `EvaluationReport` object.
* **Metric Bounds**: Confirm zero metric results evaluate NDCG to `1.0`.
* **Zero evaluation bias**: Confirm empty lists are skipped and averages are computed correctly over non-empty evaluated cases.
* **Evaluation Dataset Order Independence**: Confirm that executing evaluation over the same dataset in two different orders produces mathematically identical metrics.
* **Metric Monotonicity**: Confirm that for a fixed ranking, increasing the relevance score of a node never decreases NDCG or DCG.
