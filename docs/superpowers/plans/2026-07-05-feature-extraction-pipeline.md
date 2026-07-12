# Feature Extraction Pipeline (Phase 13) Refined Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Refactor temporal search ranking into a pipeline separating raw feature extraction, signal normalization, and model evaluation to make adding future features lightweight, robust, and clean.

**Architecture:** 
1. `RawFeatureVector` holds raw feature floats.
2. `NormalizationContext` specifies how features should be scaled (starting with `BatchMinMax` context).
3. `FeatureNormalizer` trait consumes raw vectors and `NormalizationContext`, scaling them into `[0.0, 1.0]` `RankingSignals`.
4. `FeatureExtractor` trait extracts raw vectors from domain nodes, relying purely on `RepositorySet` and a slice of `TemporalEdge` (decoupled from direct database dependencies).
5. `FeatureExtractionReport` preserves audit provenance for explainability.
6. `FeaturePipelineReporter` orchestrates compiling these reports from extractor/normalizer inputs and outputs, keeping reporting orthogonal to computation.
7. `LearnedTemporalScorer` coordinates extraction, normalization, and scoring.

**Tech Stack:** Rust, `brain-domain`, `brain-services`

## Global Constraints
* Maintain 100% test coverage and ensure zero dependencies on async/infrastructure in `brain-domain`.
* Keep all public traits, structs, and methods fully documented with doc comments to satisfy `#![deny(missing_docs)]`.
* Follow strictly test-driven development (TDD) by writing tests first or immediately alongside changes.

---

### Task 1: Domain Feature Types & Normalizer
**Files:**
* Create: `crates/brain-domain/src/retrieval/features.rs`
* Modify: `crates/brain-domain/src/retrieval/mod.rs`
* Test: `crates/brain-domain/tests/feature_pipeline_tests.rs`

**Interfaces:**
* Consumes: `NormalizedSignal`, `RankingSignals` from `brain_domain::retrieval::models`
* Produces: `RawFeatureVector`, `NormalizationContext`, `FeatureNormalizer` trait, `MinMaxNormalizer` implementation, `FeatureExtractionReport`, `FeaturePipelineReporter`

- [ ] **Step 1: Write the failing tests in `feature_pipeline_tests.rs`**
  ```rust
  #[test]
  fn test_min_max_normalizer_invariants() {
      use brain_domain::retrieval::features::{RawFeatureVector, NormalizationContext, FeatureNormalizer, MinMaxNormalizer};
      let normalizer = MinMaxNormalizer;
      let raw = vec![
          RawFeatureVector { semantic: 10.0, graph: 1.0, recency: 0.1, temporal: 5.0 },
          RawFeatureVector { semantic: 20.0, graph: 3.0, recency: 0.9, temporal: 5.0 },
      ];
      let context = NormalizationContext::BatchMinMax;
      let signals = normalizer.normalize(&raw, &context).unwrap();
      assert_eq!(signals.len(), 2);
      // Min values should map to 0.0, Max to 1.0
      assert_eq!(signals[0].semantic.value(), 0.0);
      assert_eq!(signals[1].semantic.value(), 1.0);
      // Constant values (like temporal = 5.0) should map to 1.0
      assert_eq!(signals[0].temporal.value(), 1.0);
      assert_eq!(signals[1].temporal.value(), 1.0);
  }
  ```
- [ ] **Step 2: Run test to verify it fails**
  Run: `PYO3_PYTHON=$(pwd)/daemon/.venv/bin/python cargo test -p brain-domain --test feature_pipeline_tests`
  Expected: FAIL with compilation error (module/structs not defined)
- [ ] **Step 3: Implement `RawFeatureVector`, `NormalizationContext`, `FeatureNormalizer`, `MinMaxNormalizer`, `FeatureExtractionReport`, and `FeaturePipelineReporter`**
  Write `crates/brain-domain/src/retrieval/features.rs`:
  ```rust
  use crate::retrieval::models::{NormalizedSignal, RankingSignals};
  use crate::identifiers::NodeId;
  use brain_core::errors::BrainError;

  /// Holds raw numerical features extracted for a candidate node.
  #[derive(Debug, Clone, PartialEq)]
  pub struct RawFeatureVector {
      /// Raw semantic match score.
      pub semantic: f64,
      /// Raw graph centrality degree.
      pub graph: f64,
      /// Raw time decay preference weight.
      pub recency: f64,
      /// Raw total temporal edge observation count.
      pub temporal: f64,
  }

  /// Explicit context/strategy details for normalizing features.
  #[derive(Debug, Clone, PartialEq)]
  pub enum NormalizationContext {
      /// Normalize dynamically using min-max values of the active batch.
      BatchMinMax,
      /// Normalize using fixed ranges.
      FixedRanges {
          /// Semantic score bounds.
          semantic_range: (f64, f64),
          /// Graph score bounds.
          graph_range: (f64, f64),
          /// Recency decay bounds.
          recency_range: (f64, f64),
          /// Temporal density bounds.
          temporal_range: (f64, f64),
      },
  }

  /// Trait defining normalization strategies for scaling raw features.
  pub trait FeatureNormalizer: Send + Sync {
      /// Normalizes raw features into ranking signal value objects.
      fn normalize(&self, raw: &[RawFeatureVector], context: &NormalizationContext) -> Result<Vec<RankingSignals>, BrainError>;
  }

  /// Min-max scaling normalizer mapping ranges to [0.0, 1.0].
  pub struct MinMaxNormalizer;

  impl FeatureNormalizer for MinMaxNormalizer {
      fn normalize(&self, raw: &[RawFeatureVector], context: &NormalizationContext) -> Result<Vec<RankingSignals>, BrainError> {
          if raw.is_empty() {
              return Ok(Vec::new());
          }

          let (min_sem, max_sem, min_graph, max_graph, min_rec, max_rec, min_temp, max_temp) = match context {
              NormalizationContext::BatchMinMax => {
                  let min_s = raw.iter().map(|v| v.semantic).fold(f64::INFINITY, f64::min);
                  let max_s = raw.iter().map(|v| v.semantic).fold(f64::NEG_INFINITY, f64::max);
                  let min_g = raw.iter().map(|v| v.graph).fold(f64::INFINITY, f64::min);
                  let max_g = raw.iter().map(|v| v.graph).fold(f64::NEG_INFINITY, f64::max);
                  let min_r = raw.iter().map(|v| v.recency).fold(f64::INFINITY, f64::min);
                  let max_r = raw.iter().map(|v| v.recency).fold(f64::NEG_INFINITY, f64::max);
                  let min_t = raw.iter().map(|v| v.temporal).fold(f64::INFINITY, f64::min);
                  let max_t = raw.iter().map(|v| v.temporal).fold(f64::NEG_INFINITY, f64::max);
                  (min_s, max_s, min_g, max_g, min_r, max_r, min_t, max_t)
              }
              NormalizationContext::FixedRanges { semantic_range, graph_range, recency_range, temporal_range } => {
                  (semantic_range.0, semantic_range.1, graph_range.0, graph_range.1, recency_range.0, recency_range.1, temporal_range.0, temporal_range.1)
              }
          };

          let norm = |val: f64, min: f64, max: f64| -> f64 {
              if max == min || val.is_nan() {
                  1.0
              } else {
                  ((val - min) / (max - min)).clamp(0.0, 1.0)
              }
          };

          let mut result = Vec::with_capacity(raw.len());
          for v in raw {
              let sem = NormalizedSignal::new(norm(v.semantic, min_sem, max_sem))?;
              let graph = NormalizedSignal::new(norm(v.graph, min_graph, max_graph))?;
              let rec = NormalizedSignal::new(norm(v.recency, min_rec, max_rec))?;
              let temp = NormalizedSignal::new(norm(v.temporal, min_temp, max_temp))?;

              result.push(RankingSignals::new(sem, graph, rec, temp));
          }
          Ok(result)
      }
  }

  /// Provenance audit trail documenting raw-to-normalized feature transformations.
  #[derive(Debug, Clone, PartialEq)]
  pub struct FeatureExtractionReport {
      /// Target candidate node.
      pub node_id: NodeId,
      /// Originally computed raw feature vector.
      pub raw_features: RawFeatureVector,
      /// Normalization context configurations.
      pub normalization_context: NormalizationContext,
      /// Generated normalized signal parameters.
      pub normalized_signals: RankingSignals,
  }

  /// Utility to build feature reports compile-time separated from extractors and normalizers.
  pub struct FeaturePipelineReporter;

  impl FeaturePipelineReporter {
      /// Build feature extraction reports from inputs and outputs.
      pub fn build_reports(
          nodes: &[crate::Node],
          raw: &[RawFeatureVector],
          normalized: &[RankingSignals],
          context: &NormalizationContext,
      ) -> Vec<FeatureExtractionReport> {
          let mut reports = Vec::with_capacity(raw.len());
          for i in 0..raw.len() {
              reports.push(FeatureExtractionReport {
                  node_id: nodes[i].id,
                  raw_features: raw[i].clone(),
                  normalization_context: context.clone(),
                  normalized_signals: normalized[i].clone(),
              });
          }
          reports
      }
  }
  ```
  Expose the module in `crates/brain-domain/src/retrieval/mod.rs`:
  ```rust
  /// Ranking signals and model feature representation.
  pub mod features;
  ```
- [ ] **Step 4: Run test to verify it passes**
  Run: `PYO3_PYTHON=$(pwd)/daemon/.venv/bin/python cargo test -p brain-domain --test feature_pipeline_tests`
  Expected: PASS
- [ ] **Step 5: Commit changes**
  Run: `git add crates/brain-domain && git commit -m "feat: add domain features, normalizer trait, minmax normalizer, and provenance reports"`

---

### Task 2: Service Feature Extractor
**Files:**
* Create: `crates/brain-services/src/retrieval/feature_extractor.rs`
* Modify: `crates/brain-services/src/retrieval.rs`
* Test: `crates/brain-services/tests/feature_extractor_tests.rs`

**Interfaces:**
* Consumes: `RawFeatureVector` from `brain_domain::retrieval::features`, `RepositorySet` from `brain_core::repositories`
* Produces: `FeatureExtractor` trait, `DefaultFeatureExtractor` implementation decoupled from direct SQLite access

- [ ] **Step 1: Write failing test in `feature_extractor_tests.rs`**
  ```rust
  #[test]
  fn test_default_feature_extractor() {
      use brain_services::retrieval::feature_extractor::{FeatureExtractor, DefaultFeatureExtractor};
      // Scaffolding test for extraction from nodes with mock repository view
  }
  ```
- [ ] **Step 2: Run test to verify failure**
  Run: `PYO3_PYTHON=$(pwd)/daemon/.venv/bin/python cargo test -p brain-services --test feature_extractor_tests`
  Expected: FAIL with compilation error
- [ ] **Step 3: Implement `FeatureExtractor` trait and `DefaultFeatureExtractor`**
  Write `crates/brain-services/src/retrieval/feature_extractor.rs`:
  ```rust
  use brain_core::errors::BrainError;
  use brain_core::retrieval::RetrievalRequest;
  use brain_core::repositories::RepositorySet;
  use brain_domain::{Node, NodeId, temporal::{RecencyPolicy, TimePoint, TemporalEdge}};
  use brain_domain::retrieval::features::RawFeatureVector;

  /// Interface for extracting raw ranking features from nodes.
  pub trait FeatureExtractor: Send + Sync {
      /// Computes raw features for candidate nodes based on search request and temporal observations.
      fn extract(
          &self,
          request: &RetrievalRequest,
          nodes: &[Node],
          temporal_edges: &[TemporalEdge],
          repos: &dyn RepositorySet,
      ) -> Result<Vec<RawFeatureVector>, BrainError>;
  }

  /// Concrete implementation of FeatureExtractor decoupled from concrete SQLite dependencies.
  pub struct DefaultFeatureExtractor {
      reference_time: TimePoint,
      recency_policy: RecencyPolicy,
  }

  impl DefaultFeatureExtractor {
      /// Creates a new `DefaultFeatureExtractor`.
      pub fn new(reference_time: TimePoint, recency_policy: RecencyPolicy) -> Self {
          Self { reference_time, recency_policy }
      }
  }

  impl FeatureExtractor for DefaultFeatureExtractor {
      fn extract(
          &self,
          request: &RetrievalRequest,
          nodes: &[Node],
          temporal_edges: &[TemporalEdge],
          repos: &dyn RepositorySet,
      ) -> Result<Vec<RawFeatureVector>, BrainError> {
          let mut node_recency = std::collections::HashMap::new();
          let mut node_temp_count = std::collections::HashMap::new();

          for te in temporal_edges {
              let t = te.observed_at.unix_seconds();
              node_recency.entry(te.edge.source)
                  .and_modify(|existing| *existing = std::cmp::max(*existing, t))
                  .or_insert(t);
              node_recency.entry(te.edge.target)
                  .and_modify(|existing| *existing = std::cmp::max(*existing, t))
                  .or_insert(t);

              *node_temp_count.entry(te.edge.source).or_insert(0) += 1;
              *node_temp_count.entry(te.edge.target).or_insert(0) += 1;
          }

          let mut raw_vectors = Vec::with_capacity(nodes.len());
          for node in nodes {
              let semantic = crate::retrieval::source::calculate_node_match_score(node, &request.query) as f64;
              
              // Query graph connections via RepositorySet abstraction
              let graph = repos.edges().get_connections(&node.id)?.len() as f64;

              let obs_time = node_recency.get(&node.id).cloned().unwrap_or(0);
              let recency = self.recency_policy.compute_weight(
                  1.0,
                  TimePoint::from_unix_seconds(obs_time),
                  self.reference_time,
              );
              let temporal = node_temp_count.get(&node.id).cloned().unwrap_or(0) as f64;

              raw_vectors.push(RawFeatureVector { semantic, graph, recency, temporal });
          }
          Ok(raw_vectors)
      }
  }
  ```
  Declare the module in `crates/brain-services/src/retrieval.rs`:
  ```rust
  /// Feature extraction pipeline for learned ranking.
  pub mod feature_extractor;
  ```
- [ ] **Step 4: Run test to verify it passes**
  Run: `PYO3_PYTHON=$(pwd)/daemon/.venv/bin/python cargo test -p brain-services --test feature_extractor_tests`
  Expected: PASS
- [ ] **Step 5: Commit changes**
  Run: `git add crates/brain-services && git commit -m "feat: implement repository-decoupled DefaultFeatureExtractor"`

---

### Task 3: Refactor Scorer Integration & Verification
**Files:**
* Modify: `crates/brain-services/src/retrieval/temporal.rs`
* Test: `crates/brain-services/tests/temporal_calibration_tests.rs`
* Test: `crates/brain-services/tests/learned_ranking_invariant_tests.rs`

**Interfaces:**
* Consumes: `FeatureExtractor`, `MinMaxNormalizer`
* Produces: Refactored `LearnedTemporalScorer` coordinating decoupled stages

- [ ] **Step 1: Refactor `LearnedTemporalScorer` inside `temporal.rs`**
  ```rust
  impl RankingStrategy for LearnedTemporalScorer {
      fn rank(&self, request: &RetrievalRequest, nodes: Vec<Node>) -> Result<Vec<Node>, BrainError> {
          if nodes.is_empty() {
              return Ok(nodes);
          }

          use brain_domain::retrieval::models::{LinearRankingModel, RankingModel};
          use brain_domain::retrieval::features::{MinMaxNormalizer, FeatureNormalizer, NormalizationContext};
          use crate::retrieval::feature_extractor::{FeatureExtractor, DefaultFeatureExtractor};

          // 1. Fetch temporal edges and wrap in ProjectedRepositoryView decorator
          let temp_edges = self.storage.list_all_temporal_edges()?;
          let snapshot = brain_domain::temporal::TemporalProjector::project(&temp_edges, &brain_domain::temporal::TemporalQuery {
              reference_time: self.reference_time,
              visibility: brain_domain::temporal::TemporalVisibility::new(vec![]),
              recency_policy: self.recency_policy.clone(),
          });
          let projected_repos = ProjectedRepositoryView::new(
              self.storage.clone() as Arc<dyn RepositorySet>,
              snapshot,
          );

          // 2. Extract raw features using FeatureExtractor (decoupled from storage dependencies)
          let extractor = DefaultFeatureExtractor::new(
              self.reference_time,
              self.recency_policy.clone(),
          );
          let raw_features = extractor.extract(request, &nodes, &temp_edges, &projected_repos)?;

          // 3. Normalize features using FeatureNormalizer & NormalizationContext
          let normalizer = MinMaxNormalizer;
          let context = NormalizationContext::BatchMinMax;
          let normalized_signals = normalizer.normalize(&raw_features, &context)?;

          // 4. Load active weights model and score nodes
          let active_snapshot = self.weight_provider.active_snapshot()?;
          let model = LinearRankingModel::new(active_snapshot.weights.clone());

          let mut scored_nodes = Vec::with_capacity(nodes.len());
          for (idx, node) in nodes.into_iter().enumerate() {
              let score = model.score(&normalized_signals[idx]);
              scored_nodes.push((node, score));
          }

          // 5. Sort descending, fallback to ID
          scored_nodes.sort_by(|a, b| {
              b.1.partial_cmp(&a.1)
                  .unwrap_or(std::cmp::Ordering::Equal)
                  .then_with(|| a.0.id.0.cmp(&b.0.id.0))
          });

          Ok(scored_nodes.into_iter().map(|(n, _)| n).collect())
      }
  }
  ```
- [ ] **Step 2: Run all tests in the workspace**
  Run: `PYO3_PYTHON=$(pwd)/daemon/.venv/bin/python cargo test --all`
  Expected: PASS
- [ ] **Step 3: Commit changes**
  Run: `git add crates/brain-services && git commit -m "refactor: integrate FeatureExtractor and MinMaxNormalizer into LearnedTemporalScorer"`

---

## Verification Plan

### Automated Tests
* Validate all tests across all modules pass cleanly:
  ```bash
  PYO3_PYTHON=$(pwd)/daemon/.venv/bin/python cargo test --all
  ```

### Invariants Verification
* **Feature Completeness**: Assert `RawFeatureVector` fields map 1:1 to model weights.
* **Normalization Stability**: Assert identical features normalize to byte-for-byte identical values under `NormalizationContext::BatchMinMax`.
* **Feature Ordering**: Assert normalized signal order maps index-for-index to candidates.
