# Richer Ranking Models (Phase 16) Refined Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Introduce non-linear ranking models behind the polymorphic `RankingModel` interface, separating serializable `DecisionTreeDefinition` from executable `DecisionTreeRankingModel` in `brain-domain`, and parsing/routing them loudly via `ModelDeserializer` in `brain-services`.

**Architecture:**
1. **FeatureId**: Enum replacing raw strings for feature indexing (`Semantic`, `Graph`, `Recency`, `Temporal`).
2. **SplitThreshold & LeafScore**: Value objects validating finite floats.
3. **DecisionTreeDefinition**: Immutable domain model containing serializable `DecisionTreeNode` representations.
4. **DecisionTreeRankingModel**: Executable `RankingModel` implementing split decisions over `RankingSignals`.
5. **ModelDeserializer & Resolver**: Infrastructure factory in `brain-services` parsing JSON tree parameters, returning explicit error states instead of silent fallbacks.
6. **Scorer Integration**: Refactor `LearnedTemporalScorer` to resolve ranking models via the new `ModelDeserializer`.

**Tech Stack:** Rust, `brain-domain`, `brain-services`

## Global Constraints
* Keep all domain models pure and dependency-free.
* Implement strict validation checking on all float thresholds and scores (rejecting NaNs/infinity).
* Provide exhaustive unit test coverage for FBDTs (Fast Boosted Decision Trees) split traversals.

---

### Task 1: Domain Value Objects & DecisionTree
**Files:**
* Modify: `crates/brain-domain/src/retrieval/models.rs`
* Test: `crates/brain-domain/tests/ranking_model_domain_tests.rs`

- [ ] **Step 1: Write TDD tests for Tree Invariants**
  Verify serialization round-trips, validation errors for infinite thresholds, and correct leaf score returns.
- [ ] **Step 2: Run test to verify it fails**
  Run: `PYO3_PYTHON=$(pwd)/daemon/.venv/bin/python cargo test -p brain-domain --test ranking_model_domain_tests`
  Expected: FAIL with compilation error
- [ ] **Step 3: Implement domain types in `models.rs`**
  Add the following types to `crates/brain-domain/src/retrieval/models.rs`:
  ```rust
  use crate::consolidation::MetricConstructionError;

  /// Identifier for ranking signals.
  #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
  pub enum FeatureId {
      /// Semantic similarity signal.
      Semantic,
      /// Graph-based connectivity signal.
      Graph,
      /// Recency/decay signal.
      Recency,
      /// Projected temporal edge score.
      Temporal,
  }

  /// Holds a validated split threshold value.
  #[derive(Debug, Clone, Copy, PartialEq, PartialOrd, serde::Serialize, serde::Deserialize)]
  pub struct SplitThreshold(f64);

  impl SplitThreshold {
      /// Creates a new validated `SplitThreshold`.
      pub fn new(val: f64) -> Result<Self, MetricConstructionError> {
          if !val.is_finite() {
              return Err(MetricConstructionError::NotFinite { val });
          }
          Ok(Self(val))
      }

      /// Accesses the underlying threshold value.
      pub fn value(&self) -> f64 {
          self.0
      }
  }

  /// Holds a validated leaf score value.
  #[derive(Debug, Clone, Copy, PartialEq, PartialOrd, serde::Serialize, serde::Deserialize)]
  pub struct LeafScore(f64);

  impl LeafScore {
      /// Creates a new validated `LeafScore`.
      pub fn new(val: f64) -> Result<Self, MetricConstructionError> {
          if !val.is_finite() {
              return Err(MetricConstructionError::NotFinite { val });
          }
          Ok(Self(val))
      }

      /// Accesses the underlying score value.
      pub fn value(&self) -> f64 {
          self.0
      }
  }

  /// Serializable decision tree node definition.
  #[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
  pub enum DecisionTreeNode {
      /// Internal split node.
      Split {
          /// Feature dimension to split on.
          feature: FeatureId,
          /// Split threshold.
          threshold: SplitThreshold,
          /// Left branch (val < threshold).
          left: Box<DecisionTreeNode>,
          /// Right branch (val >= threshold).
          right: Box<DecisionTreeNode>,
      },
      /// Terminal leaf node.
      Leaf {
          /// Return score.
          score: LeafScore,
      },
  }

  /// Serializable package for decision tree configs.
  #[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
  pub struct DecisionTreeDefinition {
      /// Root node of the tree.
      pub root: DecisionTreeNode,
  }

  /// Executable model evaluating decision tree definitions over ranking signals.
  #[derive(Debug, Clone)]
  pub struct DecisionTreeRankingModel {
      definition: DecisionTreeDefinition,
  }

  impl DecisionTreeRankingModel {
      /// Creates a new `DecisionTreeRankingModel`.
      pub fn new(definition: DecisionTreeDefinition) -> Self {
          Self { definition }
      }

      fn evaluate_node(node: &DecisionTreeNode, signals: &RankingSignals) -> f64 {
          match node {
              DecisionTreeNode::Leaf { score } => score.value(),
              DecisionTreeNode::Split { feature, threshold, left, right } => {
                  let val = match feature {
                      FeatureId::Semantic => signals.semantic.value(),
                      FeatureId::Graph => signals.graph.value(),
                      FeatureId::Recency => signals.recency.value(),
                      FeatureId::Temporal => signals.temporal.value(),
                  };
                  if val < threshold.value() {
                      Self::evaluate_node(left, signals)
                  } else {
                      Self::evaluate_node(right, signals)
                  }
              }
          }
      }
  }

  impl RankingModel for DecisionTreeRankingModel {
      fn version(&self) -> RankingModelVersion {
          RankingModelVersion::V2DecisionTree
      }

      fn score(&self, signals: &RankingSignals) -> f64 {
          Self::evaluate_node(&self.definition.root, signals)
      }
  }
  ```
  And add `V2DecisionTree` to `RankingModelVersion` and extend `CalibrationMetadata` with optional parameters fields:
  ```rust
  #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
  pub enum RankingModelVersion {
      /// Linear model.
      V1Linear,
      /// Decision Tree model.
      V2DecisionTree,
  }
  ```
- [ ] **Step 4: Run test to verify it passes**
- [ ] **Step 5: Commit changes**

---

### Task 2: Infrastructure Model Deserializer
**Files:**
* Modify: `crates/brain-services/src/retrieval/active_weights.rs` (or create a dedicated module)
* Test: `crates/brain-services/tests/ranking_model_integration_tests.rs`

- [ ] **Step 1: Write integration tests in `ranking_model_integration_tests.rs`**
  Verify that invalid tree formats result in loud errors, while valid ones instantiate `DecisionTreeRankingModel`.
- [ ] **Step 2: Run test to verify it fails**
- [ ] **Step 3: Implement `ModelDeserializer`**
  Add definition in `crates/brain-services/src/retrieval/active_weights.rs` or create `crates/brain-services/src/retrieval/model_resolver.rs`:
  ```rust
  use brain_core::errors::BrainError;
  use brain_domain::retrieval::models::{RankingModel, WeightSnapshot, RankingModelVersion, LinearRankingModel};
  use brain_domain::retrieval::models::{DecisionTreeRankingModel, DecisionTreeDefinition};

  /// Diagnostic error during model resolution.
  #[derive(Debug, Clone, thiserror::Error)]
  pub enum ModelResolutionError {
      /// Deserialization failed.
      #[error("JSON Deserialization failed: {0}")]
      DeserializationFailed(String),
      /// Missing parameters for tree model.
      #[error("Missing parameters for DecisionTree model")]
      MissingParameters,
  }

  /// Service converting snapshots into executable ranking models.
  pub struct ModelDeserializer;

  impl ModelDeserializer {
      /// Deserializes a model loudly or returns a custom error.
      pub fn resolve(
          snapshot: &WeightSnapshot,
          model_version: RankingModelVersion,
          parameters: Option<&str>,
      ) -> Result<Box<dyn RankingModel>, BrainError> {
          match model_version {
              RankingModelVersion::V2DecisionTree => {
                  let json_str = parameters.ok_or_else(|| BrainError::Internal {
                      message: "Missing parameters for DecisionTree model".to_string(),
                  })?;
                  let def = serde_json::from_str::<DecisionTreeDefinition>(json_str)
                      .map_err(|e| BrainError::Internal {
                          message: format!("DecisionTree parsing failed: {:?}", e),
                      })?;
                  Ok(Box::new(DecisionTreeRankingModel::new(def)))
              }
              RankingModelVersion::V1Linear => {
                  Ok(Box::new(LinearRankingModel::new(snapshot.weights.clone())))
              }
          }
      }
  }
  ```
- [ ] **Step 4: Run test to verify it passes**
- [ ] **Step 5: Commit changes**

---

### Task 3: Scorer Integration
**Files:**
* Modify: `crates/brain-services/src/retrieval/temporal.rs`

- [ ] **Step 1: Refactor `LearnedTemporalScorer` in `temporal.rs`**
  ```rust
  // Replace:
  // let model = LinearRankingModel::new(routing_decision.snapshot.weights.clone());
  // With loud resolution:
  let model_version = routing_decision.snapshot.metadata.calibration_metadata.model_version()
      .unwrap_or(RankingModelVersion::V1Linear);
  let params = routing_decision.snapshot.metadata.calibration_metadata.parameters(); // or equivalent
  let model = crate::retrieval::model_resolver::ModelDeserializer::resolve(
      &routing_decision.snapshot,
      model_version,
      params,
  )?;
  ```
- [ ] **Step 2: Run all tests in the workspace**
  Run: `PYO3_PYTHON=$(pwd)/daemon/.venv/bin/python cargo test --all`
- [ ] **Step 3: Commit changes**

---

## Verification Plan

### Automated Tests
* Validate all tests across all modules pass cleanly:
  ```bash
  PYO3_PYTHON=$(pwd)/daemon/.venv/bin/python cargo test --all
  ```

### Invariants Verification
* **Model Serialization Round Trip**: Verify that a structured FBDT model round-trips to JSON and back, producing an identical `DecisionTreeDefinition`.
* **Explicit Factory Failure**: Verify that passing a corrupted JSON string or missing parameters yields an explicit error, preventing silent routing regressions.
