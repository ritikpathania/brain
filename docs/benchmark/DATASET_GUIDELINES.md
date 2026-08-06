# Knowledge Quality Corpus (KQC) — Architecture & Asset Guidelines

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

## 2. Closed Continuous Production Feedback Loop

```
Production Queries ──► Curation ──► KQC Packages ──► RQB Execution ──► Capability Health ──► Retrieval Fixes ──► Production ──┐
  ▲                                                                                                                            │
  └────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────┘
```

---

## 3. Measured Empirical Baseline vs Target Forecasts

| System Capability | Measured Baseline (v1.2) | Planning Target (v1.3) | Planning Target (v1.4) | Trend Status |
|---|:---:|:---:|:---:|:---:|
| **Alias Normalization** | **71.0%** *(Quality Warning)* | **$\ge 78.0\%$** | **$\ge 86.0\%$** | 🟢 Target Improving ↑ |
| **Context Resolution** | **96.0%** *(PASS)* | **$\ge 97.0\%$** | **$\ge 99.0\%$** | 🟢 Target Stable ↑ |
| **Temporal Ordering** | **100.0%** *(PASS)* | **$100.0\%$** | **$100.0\%$** | ⚪ Stable → |
| **Conflict Visibility** | **100.0%** *(PASS)* | **$100.0\%$** | **$100.0\%$** | ⚪ Stable → |

---

## 4. Productized KQC Asset Governance

KQC packages in `sdks/python/rqb/datasets/` follow formal product asset rules:
- **Bi-weekly Release Cadence**: Managed release cycle for new evaluation scenarios.
- **Semantic Versioning**: Independent package versioning (e.g. `aliases v3.2.0`).
- **Changelog & Provenance**: Tracked additions and updates per dataset package.
- **Deprecation Policy**: Formal RFC before deprecating ground-truth scenario assertions.

---

## 5. Practical Definition of "Contract Stability"

**"Stable Public Contracts" at v2.2.0 means STABLE PUBLIC INTERFACES, NOT ZERO CODE EDITS.**

- 🔒 **Stable Public Contracts**: Interfaces, report schema, evaluator lifecycle, policy JSON structure.
- 🔓 **Evolvable Maintenance**: Correctness bug fixes, performance optimizations, statistical accuracy fixes, dependency updates.

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
