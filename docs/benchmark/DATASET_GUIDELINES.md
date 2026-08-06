# Knowledge Quality Corpus (KQC) — Architecture & Capability Guidelines

The **Knowledge Quality Corpus (KQC)** represents the ground-truth evaluation data powering the Retrieval Quality Benchmark (RQB). KQC is treated as a first-class asset separate from the RQB execution engine.

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

## 2. Multi-Release Capability Trend Tracking

| System Capability | v1.2 Baseline | v1.3 Target | v1.4 Target | Trend Status |
|---|:---:|:---:|:---:|:---:|
| **Alias Normalization** | **71.0%** | **78.0%** | **86.0%** | 🟢 Improving ↑ |
| **Context Resolution** | **96.0%** | **97.0%** | **99.0%** | 🟢 Stable ↑ |
| **Temporal Ordering** | **100.0%** | **100.0%** | **100.0%** | ⚪ Stable → |
| **Conflict Visibility** | **100.0%** | **100.0%** | **100.0%** | ⚪ Stable → |

---

## 3. Real-World User Query Feedback Loop

```
User Query Logs ──► Curated & Anonymized ──► KQC Packages ──► RQB Execution ──► Capability Health ──► Retrieval Improvements
```

---

## 4. Practical Definition of "Contract Stability"

**"Stable Public Contracts" at v2.2.0 means STABLE PUBLIC INTERFACES, NOT ZERO CODE EDITS.**

- 🔒 **Stable Public Contracts**: Interfaces, report schema, evaluator lifecycle, policy JSON structure.
- 🔓 **Evolvable Maintenance**: Correctness bug fixes, performance optimizations, statistical accuracy fixes, dependency updates.

---

## 5. Non-Goals of the RQB Platform

The RQB platform intentionally does **NOT**:
- **Determine Product Release Readiness**: The **EBRA Gate** (`cargo xtask verify`) owns release gating.
- **Benchmark Operational Load & Scalability**: The **OPB Gate** owns 24-hour RSS memory soak and load scaling.
- **Automatically Tune Retrieval Algorithms**: RQB provides empirical metrics; engine developers optimize algorithms.
- **Replace Human & Exploratory Evaluation**: RQB measures automated scenarios; dogfooding evaluates subjective feel.
- **Define Retrieval Architecture**: RQB evaluates outputs; `brain-services` owns internal domain design.

---

## 6. Structured Diagnostic Schema for Explainable Failures

```json
{
  "vector_id": 2,
  "severity": "Critical",
  "expectation": "Canonical entity PostgreSQL",
  "observation": "Returned SQLite at rank 3",
  "probable_causes": [
    "Alias graph lookup missing 'pgsql' variant mapping"
  ],
  "suggested_capability": "Alias Normalization",
  "supporting_evidence": [
    "UDS stream chunk contains 'SQLite'"
  ]
}
```

---

## 7. Composable Logical Assertion Language

```json
{
  "id": "alias-postgres-composite",
  "canonical": "PostgreSQL",
  "aliases": ["postgres", "pgsql", "postgres database"],
  "sample_text": "PostgreSQL is configured as the primary relational database for metadata.",
  "expected": {
    "all": [
      { "contains": "PostgreSQL" },
      { "rank": { "entity": "PostgreSQL", "max": 1 } },
      { "score": { "entity": "PostgreSQL", "min": 0.80 } }
    ],
    "none": [
      { "contains": "MySQL" }
    ]
  }
}
```

---

## 8. Evidence-Based Engineering Directive

> **Retrieval engine optimizations MUST be driven by empirical RQB benchmark failures, not by subjective intuition.**

Engineering effort directly targets empirical shortfalls surfaced by RQB:
- **Primary Target**: Alias Normalization ($71.0\%$ health vs $75.0\%$ threshold target).
- **Target Capability**: `Alias Normalization` in `brain-services`.
