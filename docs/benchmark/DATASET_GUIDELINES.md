# Knowledge Quality Corpus (KQC) — Quality Architecture & Guidelines v1.0 (Platform-Complete)

The **Knowledge Quality Corpus (KQC)** represents the ground-truth evaluation data powering the Retrieval Quality Benchmark (RQB). KQC is treated as a first-class product asset separate from the RQB execution engine.

---

## 1. Five-Tier Quality System Architecture (v1 Platform-Complete)

The quality architecture is **v1 Platform-Complete**:

```
EBRA Gate           ──► Release correctness & release gate (1,225/1,225 PASS)
RQB Engine          ──► Benchmark execution & harness (STABLE CONTRACTS v2.2.0)
KQC Datasets        ──► Ground-truth corpus packages (aliases, conflicts, etc.)
Assertion Language  ──► Declarative evaluation rules (all/any/none/rank/score)
Retrieval Engine    ──► System under test (brain-services hybrid search)
```

---

## 2. Declarative Benchmark Execution Manifests (`benchmark-manifest.yaml`)

Future multi-suite benchmark runs are declared via reproducible YAML manifests:

```yaml
schema: "1.0"

suites:
  - EBRA
  - RQB
  - Agent
  - OPB

datasets:
  - aliases@3.2.0
  - conflicts@2.0.0

environment:
  seed: 42

outputs:
  benchmark-result@1.0.0
```

---

## 3. Machine-Readable `BenchmarkResult` JSON Schema Contract

All specialized benchmark suites conform to the formal machine-readable JSON Schema contract located at [`schemas/benchmark-result/1.0.0/benchmark-result.schema.json`](file:///Users/ritikpathania/Developer/PyCharm/brain/schemas/benchmark-result/1.0.0/benchmark-result.schema.json).

### Environmental Reproducibility Metadata
All benchmark runs log complete environmental reproducibility metadata inside the `provenance` block:

```json
{
  "schema_version": "1.0.0",
  "kind": "BenchmarkResult",
  "metadata": {
    "suite_id": "RQB",
    "benchmark_id": "vector-2-aliases",
    "timestamp": "2026-08-06T13:27:00Z"
  },
  "benchmark": {
    "capability": {
      "id": "alias_normalization",
      "display_name": "Alias Normalization"
    },
    "execution_status": "COMPLETED",
    "quality_status": "QUALITY_WARNING",
    "severity": "Critical"
  },
  "measurements": {
    "score": 0.71,
    "confidence": { "lower": 0.36, "upper": 0.92, "level": 0.95 },
    "latency": { "mean_ms": 29.56, "p95_ms": 58.91 }
  },
  "evidence": ["5/7 alias variants resolved canonical target"],
  "provenance": {
    "git_commit": "e7778802ec",
    "policy_hash": "ecf5732",
    "seed": 42,
    "environment": {
      "os": "macOS ARM64",
      "rust_version": "1.82.0",
      "python_version": "3.11.9",
      "dataset_hash": "a0993f3"
    }
  },
  "diagnostics": {
    "probable_causes": ["Missing 'pgsql' variant mapping"]
  },
  "recommendations": ["Expand brain-services synonym graph for 'pgsql' mapping"]
}
```

---

## 4. Operational Quality Infrastructure & CI Governance

Usability tooling around the framework focuses on operational usability without increasing engine complexity:
- **CI Executable Governance**: Automated schema validation enforcing output compliance against `benchmark-result.schema.json`.
- **Contract & Compatibility Testing**: Multi-version compatibility verification across `schemas/benchmark-result/compatibility/`.
- **Historical Capability Dashboards**: Visualizing multi-release capability trends ($v1.2 \rightarrow v1.3 \rightarrow v1.4$).
- **CI Quality Gates**: Comparing pull request benchmark runs against baseline `main` branch time-series data.
- **Automatic Regression Triage**: Grouping related vector failures into single engineering issues.

---

## 5. End-to-End User Task Success & Evidence Utilization Metrics

| Product Metric | Measurement Target | User Impact |
|---|---|---|
| **Task Success Rate** | Goal accomplishment without query reformulation | Measures overall query efficacy |
| **First Answer Success** | Relevant target returned on initial query turn | Eliminates user friction |
| **Reformulation Rate** | Frequency of query retries or rewordings | Identifies retrieval ambiguity |
| **Citation Accuracy** | Cited memory nodes directly support synthesized answer | Ensures ground-truth factual rigor |
| **Confidence Calibration** | Confidence scores (e.g. 0.90) correlate with empirical accuracy (90%) | Enables reliable threshold filtering |
| **Source Utilization** | Synthesized answer effectively utilizes all relevant retrieved evidence | Prevents over-reliance on a single source |

---

## 6. User Retrieval Failure Taxonomy

| Failure Type | Description & Example | Primary Remediation Target |
|---|---|---|
| **Missed Alias** | Variant or shorthand unmapped (`pgsql` not mapped to `PostgreSQL`) | Expand `brain-services` synonym graph |
| **Poor Ranking** | Correct result retrieved at low position (Rank 8 vs Target Rank 1) | Re-calibrate RRF fusion & BM25 parameters |
| **Missing Knowledge** | Fact absent from ingested memory graph | Ingestion pipeline & observation extractor |
| **Wrong Synthesis** | Topic synthesis summary omitted key decision | Multi-node aggregation & synthesis prompt |
| **Context Failure** | Conversational follow-up turn misunderstood | Session history projection & turn buffer |

---

## 7. Product Outcome Measurement Dimensions

Engineering focuses on four measurable product dimensions:
- **Retrieval Quality**: Alias normalization, acronym resolution, ranking quality, context resolution.
- **Dataset Quality**: Coverage, diversity, real-world query representation, drift over time.
- **Operational Quality**: Mean/P95/P99 latency, peak RSS memory, query throughput, database growth.
- **User Quality**: Time-to-answer, query reformulation rate, search abandonment, relevance.

---

## 8. Closed Continuous Production Feedback Loop

```
Production Queries ──► Curation ──► KQC Packages ──► RQB Execution ──► Capability Health ──► Retrieval Fixes ──► Production ──┐
  ▲                                                                                                                            │
  └────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────┘
```

---

## 9. Measured Empirical Baseline vs Target Forecasts

| System Capability | Measured Baseline (v1.2) | Planning Target (v1.3) | Planning Target (v1.4) | Trend Status |
|---|:---:|:---:|:---:|:---:|
| **Alias Normalization** | **71.0%** *(Quality Warning)* | **$\ge 78.0\%$** | **$\ge 86.0\%$** | 🟢 Target Improving ↑ |
| **Context Resolution** | **96.0%** *(PASS)* | **$\ge 97.0\%$** | **$\ge 99.0\%$** | 🟢 Target Stable ↑ |
| **Temporal Ordering** | **100.0%** *(PASS)* | **$100.0\%$** | **$100.0\%$** | ⚪ Stable → |
| **Conflict Visibility** | **100.0%** *(PASS)* | **$100.0\%$** | **$100.0\%$** | ⚪ Stable → |

---

## 10. Practical Definition of "Contract Stability" & DAG Schema Migration Chaining

**"Stable Public Contracts" at v2.2.0 means STABLE PUBLIC INTERFACES, NOT ZERO CODE EDITS.**

- 🔒 **Stable Public Contracts**: Interfaces, report schema, evaluator lifecycle, policy JSON structure.
- 🔓 **Evolvable Maintenance**: Correctness bug fixes, performance optimizations, statistical accuracy fixes, dependency updates.
- 🛠️ **Library-First DAG Migration Chaining**: Schema migrations are structured as a Directed Acyclic Graph (`1.0.0 -> 2.0.0 -> 3.0.0`). The migration engine composes adjacent step transitions automatically via `BenchmarkResultMigrator`, keeping individual version transitions small, isolated, and unit-testable.

```rust
pub trait BenchmarkResultMigrator {
    fn from_version(&self) -> Version;
    fn to_version(&self) -> Version;
    fn migrate(&self, result: BenchmarkResult) -> Result<BenchmarkResult>;
}
```

---

## 11. Non-Goals of the RQB Platform

The RQB platform intentionally does **NOT**:
- **Determine Product Release Readiness**: The **EBRA Gate** (`cargo xtask verify`) owns release gating.
- **Benchmark Operational Load & Scalability**: The **OPB Gate** owns 24-hour RSS memory soak and load scaling.
- **Automatically Tune Retrieval Algorithms**: RQB provides empirical metrics; engine developers optimize algorithms.
- **Replace Human & Exploratory Evaluation**: RQB measures automated scenarios; dogfooding evaluates subjective feel.
- **Define Retrieval Architecture**: RQB evaluates outputs; `brain-services` owns internal domain design.
