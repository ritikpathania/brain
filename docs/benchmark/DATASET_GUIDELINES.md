# Knowledge Quality Corpus (KQC) — Quality Architecture & Guidelines v1.0

The **Knowledge Quality Corpus (KQC)** represents the ground-truth evaluation data powering the Retrieval Quality Benchmark (RQB). KQC is treated as a first-class product asset separate from the RQB execution engine.

---

## 1. Five-Tier Quality System Architecture

```
EBRA Gate           ──► Release correctness & release gate (1,225/1,225 PASS)
RQB Engine          ──► Benchmark execution & harness (STABLE CONTRACTS v2.2.0)
KQC Datasets        ──► Ground-truth corpus packages (aliases, conflicts, etc.)
Assertion Language  ──► Declarative evaluation rules (all/any/none/rank/score)
Retrieval Engine    ──► System under test (brain-services hybrid search)
```

---

## 2. Unified `BenchmarkResult` Schema Contract & Quality Orchestrator

A lightweight **Quality Orchestrator** manages execution ordering, provenance, shared reporting, and CI integration across specialized benchmark suites. All suites emit a single, independently versioned `BenchmarkResult` schema contract (`schema_version: "1.0.0"`, `kind: "BenchmarkResult"`):

```json
{
  "schema_version": "1.0.0",
  "kind": "BenchmarkResult",
  "suite_id": "RQB",
  "benchmark_id": "vector-2-aliases",
  "capability": "Alias Normalization",
  "status": "QUALITY_WARNING",
  "score": 0.71,
  "confidence": { "lower": 0.36, "upper": 0.92, "level": 0.95 },
  "latency": { "mean_ms": 29.56, "p95_ms": 58.91 },
  "evidence": ["5/7 alias variants resolved canonical target"],
  "provenance": { "git_commit": "e777880", "policy_hash": "ecf5732" },
  "recommendations": ["Expand brain-services synonym graph for 'pgsql' mapping"]
}
```

```
                            Quality Orchestrator (Unified Schema)
                                              │
        ┌───────────────────┬─────────────────┴───────────────────┬───────────────────┐
        ▼                   ▼                                     ▼                   ▼
     EBRA Gate          RQB Suite                             Agent Suite         OPB Suite
  (Release Gate)   (Retrieval Quality)                    (Planning & Tools)   (Operational Load)
   • 1225 Unit/Int  • Alias Normalization                 • Tool Selection     • 24h Memory Soak
   • Protocol Mono  • Context & Recency                   • Planning Accuracy  • P95/P99 Latency
   • Clippy & Fmt   • Reasoning & Synthesis               • Error Recovery     • Scale Throughput
```

| Suite | Core Responsibility | Verification Target / Primary Metric |
|---|---|---|
| **EBRA Gate** | *"Is the software correct and releasable?"* | `cargo xtask verify` (1,225/1,225 PASS) |
| **RQB Suite** | *"Does retrieval return the right knowledge?"* | Alias Coverage (71%), Precision@5, Conflict Visibility |
| **Agent Suite** | *"Can the system plan and execute correctly?"* | Tool selection accuracy, planning, error recovery |
| **OPB Suite** | *"Can the system sustain production load?"* | Mean/P95/P99 latency, 24h RSS memory soak, throughput |

---

## 3. Operational Quality Infrastructure

Usability tooling around the framework focuses on operational usability without increasing engine complexity:
- **Historical Capability Dashboards**: Visualizing multi-release capability trends ($v1.2 \rightarrow v1.3 \rightarrow v1.4$).
- **CI Quality Gates**: Comparing pull request benchmark runs against baseline `main` branch time-series data.
- **Automatic Regression Triage**: Grouping related vector failures into single engineering issues.
- **Dataset Provenance & Ownership Tracking**: Maintaining author provenance, SHA-256 policy hashes, and dataset versioning.

---

## 4. End-to-End User Task Success & Evidence Utilization Metrics

| Product Metric | Measurement Target | User Impact |
|---|---|---|
| **Task Success Rate** | Goal accomplishment without query reformulation | Measures overall query efficacy |
| **First Answer Success** | Relevant target returned on initial query turn | Eliminates user friction |
| **Reformulation Rate** | Frequency of query retries or rewordings | Identifies retrieval ambiguity |
| **Citation Accuracy** | Cited memory nodes directly support synthesized answer | Ensures ground-truth factual rigor |
| **Confidence Calibration** | Confidence scores (e.g. 0.90) correlate with empirical accuracy (90%) | Enables reliable threshold filtering |
| **Source Utilization** | Synthesized answer effectively utilizes all relevant retrieved evidence | Prevents over-reliance on a single source |

---

## 5. User Retrieval Failure Taxonomy

| Failure Type | Description & Example | Primary Remediation Target |
|---|---|---|
| **Missed Alias** | Variant or shorthand unmapped (`pgsql` not mapped to `PostgreSQL`) | Expand `brain-services` synonym graph |
| **Poor Ranking** | Correct result retrieved at low position (Rank 8 vs Target Rank 1) | Re-calibrate RRF fusion & BM25 parameters |
| **Missing Knowledge** | Fact absent from ingested memory graph | Ingestion pipeline & observation extractor |
| **Wrong Synthesis** | Topic synthesis summary omitted key decision | Multi-node aggregation & synthesis prompt |
| **Context Failure** | Conversational follow-up turn misunderstood | Session history projection & turn buffer |

---

## 6. Product Outcome Measurement Dimensions

Engineering focuses on four measurable product dimensions:
- **Retrieval Quality**: Alias normalization, acronym resolution, ranking quality, context resolution.
- **Dataset Quality**: Coverage, diversity, real-world query representation, drift over time.
- **Operational Quality**: Mean/P95/P99 latency, peak RSS memory, query throughput, database growth.
- **User Quality**: Time-to-answer, query reformulation rate, search abandonment, relevance.

---

## 7. Closed Continuous Production Feedback Loop

```
Production Queries ──► Curation ──► KQC Packages ──► RQB Execution ──► Capability Health ──► Retrieval Fixes ──► Production ──┐
  ▲                                                                                                                            │
  └────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────┘
```

---

## 8. Measured Empirical Baseline vs Target Forecasts

| System Capability | Measured Baseline (v1.2) | Planning Target (v1.3) | Planning Target (v1.4) | Trend Status |
|---|:---:|:---:|:---:|:---:|
| **Alias Normalization** | **71.0%** *(Quality Warning)* | **$\ge 78.0\%$** | **$\ge 86.0\%$** | 🟢 Target Improving ↑ |
| **Context Resolution** | **96.0%** *(PASS)* | **$\ge 97.0\%$** | **$\ge 99.0\%$** | 🟢 Target Stable ↑ |
| **Temporal Ordering** | **100.0%** *(PASS)* | **$100.0\%$** | **$100.0\%$** | ⚪ Stable → |
| **Conflict Visibility** | **100.0%** *(PASS)* | **$100.0\%$** | **$100.0\%$** | ⚪ Stable → |

---

## 9. Practical Definition of "Contract Stability"

**"Stable Public Contracts" at v2.2.0 means STABLE PUBLIC INTERFACES, NOT ZERO CODE EDITS.**

- 🔒 **Stable Public Contracts**: Interfaces, report schema, evaluator lifecycle, policy JSON structure.
- 🔓 **Evolvable Maintenance**: Correctness bug fixes, performance optimizations, statistical accuracy fixes, dependency updates.

---

## 10. Non-Goals of the RQB Platform

The RQB platform intentionally does **NOT**:
- **Determine Product Release Readiness**: The **EBRA Gate** (`cargo xtask verify`) owns release gating.
- **Benchmark Operational Load & Scalability**: The **OPB Gate** owns 24-hour RSS memory soak and load scaling.
- **Automatically Tune Retrieval Algorithms**: RQB provides empirical metrics; engine developers optimize algorithms.
- **Replace Human & Exploratory Evaluation**: RQB measures automated scenarios; dogfooding evaluates subjective feel.
- **Define Retrieval Architecture**: RQB evaluates outputs; `brain-services` owns internal domain design.
