# Knowledge Quality Corpus (KQC) — Architecture & Capability Guidelines

The **Knowledge Quality Corpus (KQC)** represents the ground-truth evaluation data powering the Retrieval Quality Benchmark (RQB). KQC is treated as a first-class asset separate from the RQB execution engine.

---

## 1. Five-Tier Quality System Architecture

```
EBRA Gate           ──► Release correctness & release gate (1,225/1,225 PASS)
RQB Engine          ──► Benchmark execution & harness (FROZEN v2.2.0)
KQC Datasets        ──► Ground-truth corpus packages (aliases, conflicts, etc.)
Assertion Language  ──► Declarative evaluation rules (all/any/none/rank/score)
Retrieval Engine    ──► System under test (brain-services hybrid search)
```

---

## 2. Capability Health & Coverage Matrix

Future RQB reports evaluate performance by high-level **Capability Health** and track **Dataset Scenario Coverage** *(Illustrative Target Examples — Evaluated at Runtime)*:

| System Capability | Dataset Coverage *(Illustrative)* | Capability Health | Pervasiveness / Domain Evidence | Target Action |
|---|:---:|:---:|---|---|
| **Alias Normalization** | **642 scenarios** | **71.0%** (Quality Warning) | Pervasive across Architecture & Trace datasets (18 failures) | Expand `brain-services` synonym graph |
| **Temporal Ordering** | **188 scenarios** | **100.0%** (PASS) | Clean across Temporal decision chains (0 failures) | Maintain recency score boosting |
| **Conflict Visibility** | **121 scenarios** | **100.0%** (PASS) | Clean across ADR conflict sets (0 failures) | Maintain dual-fact surfacing |
| **Context Resolution** | **487 scenarios** | **96.0%** (PASS) | Isolated edge case in long-turn session traces | Minor context projection tuning |

---

## 3. Capability Registry & Ownership Schema

System capabilities are registered in an explicit **Capability Registry**:

```json
{
  "capability_id": "alias_normalization",
  "name": "Alias Normalization",
  "description": "Maps shorthand and alias variants to canonical entities",
  "owning_subsystem": "brain-services::alias_normalizer",
  "related_vectors": [2],
  "related_packages": ["aliases"]
}
```

---

## 4. Non-Goals of the RQB Platform

The RQB platform intentionally does **NOT**:
- **Determine Product Release Readiness**: The **EBRA Gate** (`cargo xtask verify`) owns release gating.
- **Benchmark Operational Load & Scalability**: The **OPB Gate** owns 24-hour RSS memory soak and load scaling.
- **Automatically Tune Retrieval Algorithms**: RQB provides empirical metrics; engine developers optimize algorithms.
- **Replace Human & Exploratory Evaluation**: RQB measures automated scenarios; dogfooding evaluates subjective feel.
- **Define Retrieval Architecture**: RQB evaluates outputs; `brain-services` owns internal domain design.

---

## 5. Practical Definition of "Engine Freeze"

**"Engine Freeze" at v2.2.0 means STABLE PUBLIC CONTRACTS, NOT ZERO CODE EDITS.**

- 🔒 **Frozen Public Contracts**: Stable interfaces, report schema, evaluator lifecycle, policy JSON structure.
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
