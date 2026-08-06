# Knowledge Quality Corpus (KQC) — Architecture & Guidelines

The **Knowledge Quality Corpus (KQC)** represents the ground-truth evaluation data powering the Retrieval Quality Benchmark (RQB). KQC is treated as a first-class asset separate from the RQB execution engine.

---

## 1. Non-Goals of the RQB Platform

The RQB platform intentionally does **NOT**:
- **Determine Product Release Readiness**: The **EBRA Gate** (`cargo xtask verify`) owns release gating.
- **Benchmark Operational Load & Scalability**: The **OPB Gate** owns 24-hour RSS memory soak and load scaling.
- **Automatically Tune Retrieval Algorithms**: RQB provides empirical metrics; engine developers optimize algorithms.
- **Replace Human & Exploratory Evaluation**: RQB measures automated scenarios; dogfooding evaluates subjective feel.
- **Define Retrieval Architecture**: RQB evaluates outputs; `brain-services` owns internal domain design.

---

## 2. Practical Definition of "Engine Freeze"

**"Engine Freeze" at v2.2.0 means STABLE PUBLIC CONTRACTS, NOT ZERO CODE EDITS.**

| Category | Policy Status | Permitted Activities |
|---|:---:|---|
| **Public Contracts** | 🔒 **Frozen** | Stable interfaces, report schema, evaluator lifecycle, policy JSON structure |
| **Maintenance** | 🔓 **Evolvable** | Correctness bug fixes, performance optimizations, statistical accuracy fixes, dependency updates |

---

## 3. Structured Diagnostic Schema for Explainable Failures

When a vector shortfalls, diagnostic reports emit a structured JSON schema mapping to abstract capabilities:

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

## 4. Corpus Diversity & Quality Metrics

Corpus maturity is measured by **representativeness and diversity**, not just raw scenario counts:

| Corpus Metric | Target Distribution | Purpose |
|---|---|---|
| **Domain Diversity** | Architecture, Code, CLI, APIs, Docs | Ensures multi-domain retrieval balance |
| **Query Length Distribution** | Short (1-2 words), Medium (3-6), Long (7+) | Tests short-hand vs long conversational queries |
| **Ambiguity & Contradiction Density** | 15% Ambiguous, 10% Conflicting | Evaluates engine behavior under human vagueness |
| **Entity Frequency** | Zipfian real-world distribution | Prevents over-indexing on synthetic rare terms |

---

## 5. Composable Logical Assertion Language

Dataset outcome expectations use composable boolean logical operators (`all`, `any`, `none`, `exactly_one`):

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

## 6. Five-Layer Quality Architecture

```
EBRA Gate           ──► Release correctness & release gate
RQB Engine          ──► Benchmark execution & harness (FROZEN v2.2.0)
KQC Datasets        ──► Ground-truth corpus packages (aliases, conflicts, etc.)
Assertion Language  ──► Declarative evaluation rules (all/any/none/rank/score)
Retrieval Engine    ──► System under test (brain-services hybrid search)
```

---

## 7. Evidence-Based Engineering Directive

> **Retrieval engine optimizations MUST be driven by empirical RQB benchmark failures, not by subjective intuition.**

Engineering effort directly targets empirical shortfalls surfaced by RQB:
- **Primary Target**: Synonym & Alias Coverage ($0.71$ vs $0.75$ threshold target).
- **Target Capability**: `Alias Normalization` in `brain-services`.
