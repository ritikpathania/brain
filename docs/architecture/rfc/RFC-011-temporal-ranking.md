# RFC-011: Configurable Temporal Ranking

**Author:** Antigravity AI  
**Date:** 2026-07-21  
**Status:** Proposed  
**Reference RFCs:** [RFC-001](file:///Users/ritikpathania/Developer/PyCharm/brain/docs/architecture/rfc/RFC-001.md) / [RFC-008](file:///Users/ritikpathania/Developer/PyCharm/brain/docs/architecture/rfc/RFC-008.md) / [RFC-010](file:///Users/ritikpathania/Developer/PyCharm/brain/docs/architecture/rfc/RFC-010.md)

---

## 1. Executive Summary

As the Brain Knowledge Graph scales, retrieving nodes solely based on lexical (FTS) and semantic (vector) similarity leads to stale context dilution. Fresh observations and recently updated concept nodes are often more relevant to immediate developer activities than older historical facts. 

This RFC proposes the design of a **deterministic, configurable temporal ranking stage** that integrates into the retrieval pipeline. By treating time decay as a post-fusion reranking concern, we preserve the purity of Reciprocal Rank Fusion (RRF) as our primary multi-channel consolidation mechanism. Specifically, we explicitly separate **Fusion** (which candidates survive) from **Reranking** (in what order they appear) by introducing a dedicated `Reranker` trait and a pipeline-integrated `Reranker` chain. Under this design, temporal ranking is implemented as a `TemporalReranker` within this chain, using configurable decay functions relative to a request-scoped reference time.

---

## 2. Affected Subsystems

- `crates/brain-core`:
  - [`retrieval.rs`](file:///Users/ritikpathania/Developer/PyCharm/brain/crates/brain-core/src/retrieval.rs): Add `reference_time` to `RetrievalRequest` to support reproducible, deterministic evaluations.
- `crates/brain-config`:
  - [`schema.rs`](file:///Users/ritikpathania/Developer/PyCharm/brain/crates/brain-config/src/schema.rs): Add settings definitions for temporal ranking (enabled, decay model, half-life, parameters).
- `crates/brain-services`:
  - `retrieval/ranking/`: Implement `Reranker` trait and `TemporalReranker`.
  - [`pipeline.rs`](file:///Users/ritikpathania/Developer/PyCharm/brain/crates/brain-services/src/retrieval/pipeline.rs): Register the new reranker chain in `MemoryPipeline`.

---

## 3. Proposal Details: Architecture & Pipeline Placement

### Decoupling Fusion and Reranking

We distinguish between two fundamentally different stages of the retrieval pipeline:
*   **Fusion (RRF)**: Answers: *"Which candidates should survive?"* It takes arbitrary candidate sources and normalizes them into a single consolidated set.
*   **Reranking**: Answers: *"In what order should those candidates appear?"* It acts on the fused candidates, sorting them based on orthogonal parameters like time, personalization, diversity, or semantic context.

To codify this separation, we introduce the `Reranker` trait:

```rust
pub struct RerankContext<'a> {
    pub request: &'a RetrievalRequest,
    pub config: &'a TemporalRankingSettings,
    // Extensibility: placeholders for future personalization context, telemetry, or A/B experiment tags
}

pub trait Reranker: Send + Sync {
    fn rerank(
        &self,
        candidates: Vec<Candidate>,
        context: &RerankContext,
    ) -> Result<Vec<Candidate>, BrainError>;
}
```

The retrieval pipeline is then structured as a sequential pipeline:

```
               RetrievalRequest (with reference_time)
                         │
                         ▼
             ┌───────────────────────┐
             │   Candidate Retrieval │
             │  Fts / Vector Search  │
             └───────────┬───────────┘
                         │ Candidates
                         ▼
             ┌───────────────────────┐
             │   Fusion Stage (RRF)  │
             └───────────┬───────────┘
                         │ Fused List (Unsorted Ranks)
                         ▼
             ┌───────────────────────┐
             │     Reranker Chain    │
             │  ├─ TemporalReranker  │ ◄── Reads updated_at / decay config
             │  ├─ DiversityReranker │
             │  └─ ...               │
             └───────────┬───────────┘
                         │ Reranked candidates
                         ▼
             ┌───────────────────────┐
             │ Relationship Expander │
             └───────────┬───────────┘
                         │ Rich DTOs
                         ▼
             ┌───────────────────────┐
             │       Projector       │
             └───────────────────────┘
```

### Rationale for Decoupled Reranking Chain

1. **Long-Term Extensibility**: Designing reranking as a chain allows future developers to insert personalization, diversity, context-awareness, or machine-learned rerankers (e.g. LambdaMART) without altering the core retrieval or fusion logic.
2. **RRF Purity Invariant**: RRF remains simple, parameter-free (except for standard $k=60$), and order-only. It avoids mixing raw score magnitudes with rank weights.
3. **Performance Optimization**: Expensive reranking math (like logarithmic or exponential calculations) is executed only on the top-K fused candidates (typically $K \le 100$) rather than on thousands of raw source candidates.

---

## 4. Scoring & Decay Models

The `TemporalReranker` adjusts the reciprocal rank score ($S_{\text{RRF}}$) of each candidate $c$ based on the time elapsed ($\Delta t = T - t_c$) between the request reference time $T$ and the candidate node's update timestamp $t_c$ (where $t_c \le T$).

$$S_{\text{final}}(c) = S_{\text{RRF}}(c) \cdot f(\Delta t)$$

We support four decay functions $f(\Delta t)$:

### A. Exponential Decay

$$f(\Delta t) = e^{-\lambda \Delta t} \quad \text{where} \quad \textstyle \lambda = \frac{\ln(2)}{t_{1/2}}$$

- **Tuning Parameters**: Half-life ($t_{1/2}$) representing the duration after which the temporal weight is halved (e.g., $t_{1/2} = 86400$ seconds / 1 day).
- **Advantages**: Smooth, continuous, matches natural human memory decay.
- **Disadvantages**: Can decay extremely rapidly if $t_{1/2}$ is set too low.

### B. Logarithmic Decay

$$f(\Delta t) = \frac{1}{1 + \alpha \ln(1 + \Delta t)}$$

- **Tuning Parameters**: Scaling coefficient $\alpha > 0$.
- **Advantages**: Very slow decay rate in the long tail. Ensures older historical nodes maintain distinct priority relative to each other, avoiding temporal starvation.
- **Disadvantages**: Slower initial decay.

### C. Linear Decay

$$f(\Delta t) = \max\left(0, 1 - \frac{\Delta t}{W}\right)$$

- **Tuning Parameters**: Time window $W$ (seconds).
- **Advantages**: Extremely simple to compute.
- **Disadvantages**: Hard cliff at $\Delta t = W$ discards older temporal differences, leading to rank ties.

### D. Piecewise / Step Decay

$$f(\Delta t) = \begin{cases} 
1.0 & \text{if } \Delta t < W_{\text{recent}} \\
\gamma_1 & \text{if } W_{\text{recent}} \le \Delta t < W_{\text{medium}} \\
\gamma_2 & \text{if } \Delta t \ge W_{\text{medium}}
\end{cases} \quad \text{where} \quad 1.0 > \gamma_1 > \gamma_2$$

- **Tuning Parameters**: Epoch boundaries ($W$) and scale multipliers ($\gamma$).
- **Advantages**: Maps to distinct user-facing buckets (e.g., "Active", "Recent", "Archive").
- **Disadvantages**: Discontinuous boundaries lead to unstable rank jumps at threshold limits.

---

## 5. Determinism & Reproducibility

To ensure temporal ranking adheres to **ADR-013 (Behavioral Invariants)** and **ADR-014 (Deterministic Execution)**, the following rules are enforced:

1. **Request-Scoped Reference Time**: `RetrievalRequest` is extended with an optional `reference_time` (seconds since Unix epoch). The pipeline must use this value as $T$. If `reference_time` is `None`, the service layer resolves it using `SystemTime::now()` *once* at the entry point of the pipeline and propagates it. This ensures that downstream rerankers, evaluations, and replays use a frozen, static reference time.
2. **Strict Monotonicity**: Time differences must be non-negative. If $t_c > T$ due to minor clock skew, the difference is clamped: $\Delta t = \max(0, T - t_c)$.
3. **Deterministic Tie-Breaking**: If two candidates receive identical $S_{\text{final}}$ scores, ties must be resolved lexicographically using the UUID bytes of the `NodeId` ascending.

---

## 6. Contract Changes (Interfaces & Types)

### Rust Crate Signature Changes

#### [`crates/brain-core/src/retrieval.rs`](file:///Users/ritikpathania/Developer/PyCharm/brain/crates/brain-core/src/retrieval.rs)

```diff
 pub struct RetrievalRequest {
     pub session_id: SessionId,
     pub query: String,
     pub limit: usize,
     pub exclude_ids: HashSet<NodeId>,
     pub deadline: Option<Instant>,
     pub explain: bool,
     pub graph_depth: Option<usize>,
     pub expand_relations: bool,
+    /// Reference timestamp for temporal ranking. 
+    /// Injected explicitly during testing/benchmarking to ensure determinism.
+    pub reference_time: Option<u64>,
 }
```

#### [`crates/brain-config/src/schema.rs`](file:///Users/ritikpathania/Developer/PyCharm/brain/crates/brain-config/src/schema.rs)

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemporalRankingSettings {
    pub enabled: bool,
    pub model: DecayModel,
    pub half_life_seconds: u64,
    pub scaling_factor: f64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DecayModel {
    Exponential,
    Logarithmic,
    Linear,
    Uniform, // Zero decay: f(dt) = 1.0
}
```

---

## 7. Backward Compatibility & Rollout Plan

To ensure high system stability, we decouple the architectural implementation of the `Reranker` chain from runtime tuning parameters:

1. **Phase 1: Default to Uniform (No Decay)**
   - **Default Config**: The initial implementation defaults to `DecayModel::Uniform` (or `enabled: false`), which evaluates $f(\Delta t) = 1.0$ for all candidates.
   - **Verification**: Asserts 100% backward compatibility and validates that the introduction of the reranker chain introduces no regressions on existing baseline evaluations.
2. **Phase 2: Comparative Production Experiments**
   - Run parallel A/B experiments using the evaluation harness over representative workloads comparing `Logarithmic` and `Exponential` models.
   - Telemetry tracks latency impact under Criterion benchmarks and retrieval quality changes (MRR, Recall@10) via `eval_cli`.
3. **Phase 3: Evidence-Based Promotion**
   - Promote a decay model (e.g. logarithmic decay) to default only after evaluation data demonstrates statistically significant advantages in retrieval relevance over the baseline.

---

## 8. Evaluation & Benchmarking Plan

### Latency Targets & Budgets
- **Budget**: Temporal rerank execution time must be $\le 0.5\text{ ms}$ at $K = 100$.
- **Latency Verification**: Measured using Criterion in the existing `graph_benchmarks.rs` suite.

### Quality Evaluation Metrics
- **Recall@K (K=1, 5, 10)**: Must not regress under `enabled: false`.
- **Mean Reciprocal Rank (MRR)**: Should show positive improvement when evaluated against temporal ground truths (scenarios prioritizing recency).

### CI Integration & Gates
The regression checks are run during PR validation:
```bash
# Verify no regressions on flat baselines
./scripts/retrieval-check.sh --baseline docs/reference/retrieval-baselines/v0.8.0.json
```

---

## 9. Alternatives Considered

### Alternative A: Modify RRF to Accept Temporal Ranks
Instead of a separate reranking stage, adjust RRF scores directly:
$$S_{\text{RRF}}(c) = \sum_{m \in M} \frac{1}{\text{rank}_m(c) + k} \cdot g(\Delta t)$$
- **Why Rejected**: This couples temporal ranking directly into the fusion stage, making RRF non-standard and hard to debug. 

### Alternative B: Pre-filtering Candidates (Temporal Pre-Filtering)
Exclude nodes older than a threshold $W$ before running retrieval.
- **Why Rejected**: Hard boundaries cause a total loss of recall for older nodes that might be highly relevant (e.g. foundational library documentation created 2 years ago). Pre-filtering is too destructive for general-purpose retrieval.

---

## 10. Risks & Mitigations

- **Risk: Temporal Starvation**: High recency weights could starve older, highly relevant semantic matches.
  - *Mitigation*: The rollout plan mandates evaluating logarithmic decay (which has a slower long-tail dropoff) against exponential decay before promoting any model to default.
- **Risk: Request Clocks Inconsistency**: Clock skew between distributed client/daemon processes could cause anomalous time-decay calculations.
  - *Mitigation*: Reference time $T$ is resolved at the runtime facade layer if not provided by the client, ensuring consistent intra-request times.
