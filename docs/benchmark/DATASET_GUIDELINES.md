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

## 2. End-to-End User Task Success & Evidence Utilization Metrics

To complement retrieval-centric evaluation, user task success, score calibration, and evidence utilization are tracked across product dimensions:

| Product Metric | Measurement Target | User Impact |
|---|---|---|
| **Task Success Rate** | Goal accomplishment without query reformulation | Measures overall query efficacy |
| **First Answer Success** | Relevant target returned on initial query turn | Eliminates user friction |
| **Reformulation Rate** | Frequency of query retries or rewordings | Identifies retrieval ambiguity |
| **Citation Accuracy** | Cited memory nodes directly support synthesized answer | Ensures ground-truth factual rigor |
| **Confidence Calibration** | Confidence scores (e.g. 0.90) correlate with empirical accuracy (90%) | Enables reliable threshold filtering |
| **Source Utilization** | Synthesized answer effectively utilizes all relevant retrieved evidence | Prevents over-reliance on a single source |

---

## 3. User Retrieval Failure Taxonomy

To complement diagnostic reports, retrieval shortfalls are classified into five root-cause categories:

| Failure Type | Description & Example | Primary Remediation Target |
|---|---|---|
| **Missed Alias** | Variant or shorthand unmapped (`pgsql` not mapped to `PostgreSQL`) | Expand `brain-services` synonym graph |
| **Poor Ranking** | Correct result retrieved at low position (Rank 8 vs Target Rank 1) | Re-calibrate RRF fusion & BM25 parameters |
| **Missing Knowledge** | Fact absent from ingested memory graph | Ingestion pipeline & observation extractor |
| **Wrong Synthesis** | Topic synthesis summary omitted key decision | Multi-node aggregation & synthesis prompt |
| **Context Failure** | Conversational follow-up turn misunderstood | Session history projection & turn buffer |

---

## 4. Product Outcome Measurement Dimensions

Engineering focuses on four measurable product dimensions:
- **Retrieval Quality**: Alias normalization, acronym resolution, ranking quality, context resolution.
- **Dataset Quality**: Coverage, diversity, real-world query representation, drift over time.
- **Operational Quality**: Mean/P95/P99 latency, peak RSS memory, query throughput, database growth.
- **User Quality**: Time-to-answer, query reformulation rate, search abandonment, relevance.

---

## 5. Closed Continuous Production Feedback Loop

```
Production Queries ──► Curation ──► KQC Packages ──► RQB Execution ──► Capability Health ──► Retrieval Fixes ──► Production ──┐
  ▲                                                                                                                            │
  └────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────┘
```

---

## 6. Measured Empirical Baseline vs Target Forecasts

| System Capability | Measured Baseline (v1.2) | Planning Target (v1.3) | Planning Target (v1.4) | Trend Status |
|---|:---:|:---:|:---:|:---:|
| **Alias Normalization** | **71.0%** *(Quality Warning)* | **$\ge 78.0\%$** | **$\ge 86.0\%$** | 🟢 Target Improving ↑ |
| **Context Resolution** | **96.0%** *(PASS)* | **$\ge 97.0\%$** | **$\ge 99.0\%$** | 🟢 Target Stable ↑ |
| **Temporal Ordering** | **100.0%** *(PASS)* | **$100.0\%$** | **$100.0\%$** | ⚪ Stable → |
| **Conflict Visibility** | **100.0%** *(PASS)* | **$100.0\%$** | **$100.0\%$** | ⚪ Stable → |

---

## 7. Practical Definition of "Contract Stability"

**"Stable Public Contracts" at v2.2.0 means STABLE PUBLIC INTERFACES, NOT ZERO CODE EDITS.**

- 🔒 **Stable Public Contracts**: Interfaces, report schema, evaluator lifecycle, policy JSON structure.
- 🔓 **Evolvable Maintenance**: Correctness bug fixes, performance optimizations, statistical accuracy fixes, dependency updates.

---

## 8. Non-Goals of the RQB Platform

The RQB platform intentionally does **NOT**:
- **Determine Product Release Readiness**: The **EBRA Gate** (`cargo xtask verify`) owns release gating.
- **Benchmark Operational Load & Scalability**: The **OPB Gate** owns 24-hour RSS memory soak and load scaling.
- **Automatically Tune Retrieval Algorithms**: RQB provides empirical metrics; engine developers optimize algorithms.
- **Replace Human & Exploratory Evaluation**: RQB measures automated scenarios; dogfooding evaluates subjective feel.
- **Define Retrieval Architecture**: RQB evaluates outputs; `brain-services` owns internal domain design.
