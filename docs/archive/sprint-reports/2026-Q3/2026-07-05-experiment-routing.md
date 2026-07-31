# Experiment Routing & Canary Deployments (Phase 15) Refined Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement experiment routing and canary deployments by introducing a structured `ExperimentRouter` layer executing deterministic sticky routing over multiple variants via FNV-1a hashing, governed by value-validated `TrafficAllocation` targets and `RoutingDecision` telemetry.

**Architecture:**
1. **TrafficAllocation**: Domain value object validating allocation sizes in `[0.0, 1.0]`.
2. **Variant**: Model variant binding a variant ID to a `WeightSnapshot`.
3. **ExperimentConfiguration**: Domain configuration binding experiment ID, variants, allocation targets, and the target `RoutingStrategy`.
4. **RoutingKey & RoutingStrategy**: Encapsulated parameters (e.g. SessionId, UserId) and deterministic FNV-1a hashing strategy (`StickyHashRouting`).
5. **RoutingDecision**: Provenance log detailing snapshot, variant ID, experiment ID, and routing rationale.
6. **ExperimentRouter**: Service orchestrator evaluating `RetrievalRequest` to yield a `RoutingDecision`.
7. **DefaultExperimentRouter & CanaryExperimentRouter**: Concrete routing engines wrapping baseline active providers.

**Tech Stack:** Rust, `brain-domain`, `brain-services`

## Global Constraints
* Maintain 100% test coverage and ensure zero dependencies on async/infrastructure in `brain-domain`.
* Keep all public traits, structs, and methods fully documented with doc comments to satisfy `#![deny(missing_docs)]`.
* Follow strictly test-driven development (TDD) by writing tests first or immediately alongside changes.

---

### Task 1: TrafficAllocation & Routing Configurations
**Files:**
* Create: `crates/brain-domain/src/retrieval/experiment.rs`
* Modify: `crates/brain-domain/src/retrieval/mod.rs`
* Test: `crates/brain-domain/tests/experiment_domain_tests.rs`

**Interfaces:**
* Consumes: `WeightSnapshot`, `SnapshotVersion`
* Produces: `TrafficAllocation`, `Variant`, `RoutingKey`, `RoutingStrategy`, `ExperimentConfiguration`, `RoutingDecision`

- [ ] **Step 1: Write TDD tests for TrafficAllocation & Configs**
  ```rust
  #[test]
  fn test_traffic_allocation_validation() {
      use brain_domain::retrieval::experiment::TrafficAllocation;
      assert!(TrafficAllocation::new(0.5).is_ok());
      assert!(TrafficAllocation::new(-0.1).is_err());
      assert!(TrafficAllocation::new(1.1).is_err());
  }
  ```
- [ ] **Step 2: Run test to verify it fails**
  Run: `PYO3_PYTHON=$(pwd)/daemon/.venv/bin/python cargo test -p brain-domain --test experiment_domain_tests`
  Expected: FAIL with compilation error
- [ ] **Step 3: Implement domain types in `experiment.rs`**
  Write `crates/brain-domain/src/retrieval/experiment.rs`:
  ```rust
  use crate::retrieval::models::WeightSnapshot;
  use crate::consolidation::MetricConstructionError;
  use std::sync::Arc;

  /// Holds a validated traffic allocation percentage in range [0.0, 1.0].
  #[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
  pub struct TrafficAllocation(f64);

  impl TrafficAllocation {
      /// Creates a new validated `TrafficAllocation`.
      pub fn new(val: f64) -> Result<Self, MetricConstructionError> {
          if !val.is_finite() {
              return Err(MetricConstructionError::NotFinite { val });
          }
          if val < 0.0 || val > 1.0 {
              return Err(MetricConstructionError::OutOfRange { val, min: 0.0, max: 1.0 });
          }
          Ok(Self(val))
      }

      /// Accesses the allocation value.
      pub fn value(&self) -> f64 {
          self.0
      }
  }

  /// Explicit variant binding an identifier to a snapshot.
  #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
  pub struct Variant {
      /// Unique variant identifier.
      pub id: String,
      /// Multitask weights configuration snapshot.
      pub snapshot: Arc<WeightSnapshot>,
  }

  /// Identifiers for routing traffic.
  #[derive(Debug, Clone, PartialEq, Hash)]
  pub enum RoutingKey {
      /// Stable session ID key.
      pub SessionId(String),
      /// Stable user ID key.
      pub UserId(String),
      /// Request-level key.
      pub RequestId(String),
      /// Fallback/static default routing.
      pub Constant,
  }

  /// Target algorithms defining how allocations are distributed.
  #[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
  pub enum RoutingStrategy {
      /// Sticky FNV-1a hash allocation.
      StickyHashRouting,
  }

  /// Domain configurations governing experiment routing tables.
  #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
  pub struct ExperimentConfiguration {
      /// Unique experiment identifier.
      pub id: String,
      /// Registered variants.
      pub variants: Vec<Variant>,
      /// Allocation ratios mapping variant IDs to target sizes.
      pub allocations: Vec<(String, TrafficAllocation)>,
      /// Configured routing strategy.
      pub routing_strategy: RoutingStrategy,
  }

  /// Telemetry report capturing evaluation metadata.
  #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
  pub struct RoutingDecision {
      /// Selected weight snapshot.
      pub snapshot: Arc<WeightSnapshot>,
      /// Selected variant identifier.
      pub variant_id: String,
      /// Experiment context identifier.
      pub experiment_id: String,
      /// Descriptive justification.
      pub reason: String,
  }
  ```
  Expose in `crates/brain-domain/src/retrieval/mod.rs`:
  ```rust
  /// Experiment configurations and routing decision models.
  pub mod experiment;
  ```
- [ ] **Step 4: Run test to verify it passes**
- [ ] **Step 5: Commit changes**
  Run: `git add crates/brain-domain && git commit -m "feat: implement TrafficAllocation and ExperimentConfiguration models"`

---

### Task 2: FNV-1a Hashing & ExperimentRouter Service
**Files:**
* Create: `crates/brain-services/src/retrieval/experiment.rs`
* Modify: `crates/brain-services/src/retrieval.rs`
* Modify: `crates/brain-services/src/lib.rs`
* Test: `crates/brain-services/tests/experiment_tests.rs`

**Interfaces:**
* Consumes: `ExperimentConfiguration`, `RoutingKey`, `RetrievalRequest`
* Produces: `ExperimentRouter` trait, `DefaultExperimentRouter`, `CanaryExperimentRouter`

- [ ] **Step 1: Write integration tests in `experiment_tests.rs`**
  ```rust
  #[test]
  fn test_canary_router_allocation_distribution() {
      // Setup allocations, assert sticky routing stability, and verify FNV-1a split percentages
  }
  ```
- [ ] **Step 2: Run test to verify it fails**
- [ ] **Step 3: Implement stable FNV-1a hash algorithm and routing logic**
  Write FNV-1a utility in `crates/brain-services/src/retrieval/experiment.rs`:
  ```rust
  fn fnv1a_hash(data: &str) -> u64 {
      let mut hash = 0xcbf29ce484222325;
      for byte in data.bytes() {
          hash ^= byte as u64;
          hash = hash.wrapping_mul(0x100000001b3);
      }
      hash
  }
  ```
  Write `ExperimentRouter`, `DefaultExperimentRouter`, and `CanaryExperimentRouter` in `crates/brain-services/src/retrieval/experiment.rs`:
  ```rust
  use std::sync::Arc;
  use brain_core::errors::BrainError;
  use brain_core::retrieval::RetrievalRequest;
  use brain_domain::retrieval::models::WeightSnapshot;
  use brain_domain::retrieval::experiment::{
      Variant, RoutingKey, RoutingStrategy, ExperimentConfiguration, RoutingDecision
  };
  use crate::retrieval::active_weights::ActiveWeightProvider;

  /// Interface for routing retrieval requests to appropriate weight snapshots.
  pub trait ExperimentRouter: Send + Sync {
      /// Dynamically routes the request and yields a detailed `RoutingDecision`.
      fn route_decision(&self, request: &RetrievalRequest) -> Result<RoutingDecision, BrainError>;
  }

  /// Default fallback router routing all traffic to the active baseline snapshot.
  pub struct DefaultExperimentRouter {
      provider: Arc<dyn ActiveWeightProvider>,
  }

  impl DefaultExperimentRouter {
      /// Creates a new `DefaultExperimentRouter`.
      pub fn new(provider: Arc<dyn ActiveWeightProvider>) -> Self {
          Self { provider }
      }
  }

  impl ExperimentRouter for DefaultExperimentRouter {
      fn route_decision(&self, _request: &RetrievalRequest) -> Result<RoutingDecision, BrainError> {
          let snapshot = self.provider.active_snapshot()?;
          Ok(RoutingDecision {
              snapshot,
              variant_id: "baseline".to_string(),
              experiment_id: "default".to_string(),
              reason: "Routed to default active snapshot".to_string(),
          })
      }
  }

  /// Canary router implementing multi-variant sticky FNV-1a hash routing.
  pub struct CanaryExperimentRouter {
      baseline_provider: Arc<dyn ActiveWeightProvider>,
      config: ExperimentConfiguration,
  }

  impl CanaryExperimentRouter {
      /// Creates a new `CanaryExperimentRouter`.
      pub fn new(baseline_provider: Arc<dyn ActiveWeightProvider>, config: ExperimentConfiguration) -> Self {
          Self { baseline_provider, config }
      }
  }

  impl ExperimentRouter for CanaryExperimentRouter {
      fn route_decision(&self, request: &RetrievalRequest) -> Result<RoutingDecision, BrainError> {
          let session_str = request.session_id.to_string();
          if session_str.is_empty() {
              // Fallback to baseline if no stable routing key is present (Deterministic default)
              let snapshot = self.baseline_provider.active_snapshot()?;
              return Ok(RoutingDecision {
                  snapshot,
                  variant_id: "baseline".to_string(),
                  experiment_id: self.config.id.clone(),
                  reason: "No stable session key; routed to baseline".to_string(),
              });
          }

          // Compute FNV-1a hash
          let hash_val = fnv1a_hash(&session_str);
          let fraction = (hash_val % 10000) as f64 / 10000.0;

          // Route according to cumulative traffic allocations
          let mut cumulative = 0.0;
          for (var_id, allocation) in &self.config.allocations {
              cumulative += allocation.value();
              if fraction < cumulative {
                  if let Some(variant) = self.config.variants.iter().find(|v| &v.id == var_id) {
                      return Ok(RoutingDecision {
                          snapshot: variant.snapshot.clone(),
                          variant_id: var_id.clone(),
                          experiment_id: self.config.id.clone(),
                          reason: format!("Routed to variant {} via sticky session hash ({:.4})", var_id, fraction),
                      });
                  }
              }
          }

          // Fallback to active baseline
          let snapshot = self.baseline_provider.active_snapshot()?;
          Ok(RoutingDecision {
              snapshot,
              variant_id: "baseline".to_string(),
              experiment_id: self.config.id.clone(),
              reason: "Exceeded allocation range; routed to baseline".to_string(),
          })
      }
  }
  ```
  Expose modules in `retrieval.rs` and `lib.rs`.
- [ ] **Step 4: Run test to verify it passes**
- [ ] **Step 5: Commit changes**

---

### Task 3: LearnedTemporalScorer Refactoring & Verification
**Files:**
* Modify: `crates/brain-services/src/retrieval/temporal.rs`
* Modify: `crates/brain-services/tests/temporal_calibration_tests.rs`
* Modify: `crates/brain-services/tests/learned_ranking_invariant_tests.rs`

- [ ] **Step 1: Refactor `LearnedTemporalScorer` in `temporal.rs`**
  ```rust
  pub struct LearnedTemporalScorer {
      weight_provider: Arc<dyn crate::retrieval::experiment::ExperimentRouter>,
      storage: Arc<SqliteStorage>,
      reference_time: TimePoint,
      recency_policy: RecencyPolicy,
  }
  ```
  Change `active_snapshot` resolution:
  ```rust
  let routing_decision = self.weight_provider.route_decision(request)?;
  let model = LinearRankingModel::new(routing_decision.snapshot.weights.clone());
  ```
- [ ] **Step 2: Update existing tests**
  Modify the scorer instantiations in `temporal_calibration_tests.rs`, `learned_ranking_invariant_tests.rs`, and any other tests to wrap active providers in `DefaultExperimentRouter`.
- [ ] **Step 3: Run entire workspace tests to verify compatibility**
  Run: `PYO3_PYTHON=$(pwd)/daemon/.venv/bin/python cargo test --all`
- [ ] **Step 4: Commit changes**

---

## Verification Plan

### Automated Tests
* Validate all tests across all modules pass cleanly:
  ```bash
  PYO3_PYTHON=$(pwd)/daemon/.venv/bin/python cargo test --all
  ```

### Invariants Verification
* **Routing Stability**: Verify identical session ID and configuration yield identical `RoutingDecision` outputs.
* **Stable Hash Consistency**: Validate FNV-1a hash output for test inputs matches expected constants.
* **Deterministic Default**: Verify empty session ID requests always route to baseline.
