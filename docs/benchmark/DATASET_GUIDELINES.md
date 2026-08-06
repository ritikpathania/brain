# Knowledge Quality Corpus (KQC) — Architecture & Governance Model

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

## 2. User Retrieval Failure Taxonomy

To complement diagnostic reports, retrieval failures are categorized into five root-cause classes:

| Failure Type | Description & Example | Primary Remediation Target |
|---|---|---|
| **Missed Alias** | Variant or shorthand unmapped (`pgsql` not mapped to `PostgreSQL`) | Expand `brain-services` synonym graph |
| **Poor Ranking** | Correct result retrieved at low position (Rank 8 vs Target Rank 1) | Re-calibrate RRF fusion & BM25 parameters |
| **Missing Knowledge** | Fact absent from ingested memory graph | Ingestion pipeline & observation extractor |
| **Wrong Synthesis** | Topic synthesis summary omitted key decision | Multi-node aggregation & synthesis prompt |
| **Context Failure** | Conversational follow-up turn misunderstood | Session history projection & turn buffer |

---

## 3. Product Outcome Measurement Dimensions

Engineering focuses on four measurable product dimensions:
- **Retrieval Quality**: Alias normalization, acronym resolution, ranking quality, context resolution.
- **Dataset Quality**: Coverage, diversity, real-world query representation, drift over time.
- **Operational Quality**: Mean/P95/P99 latency, peak RSS memory, query throughput, database growth.
- **User Quality**: Time-to-answer, query reformulation rate, search abandonment, relevance.

---

## 4. Closed Continuous Production Feedback Loop

```
Production Queries ──► Curation ──► KQC Packages ──► RQB Execution ──► Capability Health ──► Retrieval Fixes ──► Production ──┐
  ▲                                                                                                                            │
  └────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────┘
```

---

## 5. Measured Empirical Baseline vs Target Forecasts

| System Capability | Measured Baseline (v1.2) | Planning Target (v1.3) | Planning Target (v1.4) | Trend Status |
|---|:---:|:---:|:---:|:---:|
| **Alias Normalization** | **71.0%** *(Quality Warning)* | **$\ge 78.0\%$** | **$\ge 86.0\%$** | 🟢 Target Improving ↑ |
| **Context Resolution** | **96.0%** *(PASS)* | **$\ge 97.0\%$** | **$\ge 99.0\%$** | 🟢 Target Stable ↑ |
| **Temporal Ordering** | **100.0%** *(PASS)* | **$100.0\%$** | **$100.0\%$** | ⚪ Stable → |
| **Conflict Visibility** | **100.0%** *(PASS)* | **$100.0\%$** | **$100.0\%$** | ⚪ Stable → |

---

## 6. Productized KQC Asset Governance

KQC packages in `sdks/python/rqb/datasets/` follow formal product asset rules:
- **Bi-weekly Release Cadence**: Managed release cycle for new evaluation scenarios.
- **Semantic Versioning**: Independent package versioning (e.g. `aliases v3.2.0`).
- **Changelog & Provenance**: Tracked additions and updates per dataset package.
- **Deprecation Policy**: Formal RFC before deprecating ground-truth scenario assertions.

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
