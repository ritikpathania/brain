# Knowledge Quality Corpus (KQC) — Dataset & Assertion Guidelines

The **Knowledge Quality Corpus (KQC)** represents the ground-truth evaluation data powering the Retrieval Quality Benchmark (RQB). KQC is treated as a first-class asset separate from the RQB execution engine.

---

## 1. Declarative Assertion Primitives & Rich Outcome Schemas

Dataset items define explicit outcome criteria to detect ranking and score regressions:

```json
{
  "id": "alias-postgres",
  "canonical": "PostgreSQL",
  "aliases": ["postgres", "pgsql", "postgres database"],
  "sample_text": "PostgreSQL is configured as the primary relational database for metadata.",
  "expected": {
    "must_contain": ["PostgreSQL"],
    "must_not_contain": ["MySQL"],
    "rank_constraints": {
      "PostgreSQL": "<=1"
    },
    "score_constraints": {
      "PostgreSQL": {
        "min": 0.80
      }
    }
  }
}
```

### Core Assertion Primitives

| Assertion Primitive | Purpose | Verification Behavior |
|---|---|---|
| `must_contain` | Term Presence | Fails if required terms are absent from retrieved candidate lines |
| `must_not_contain` | Negative Constraint | Fails if forbidden terms appear in retrieved candidate lines |
| `rank_constraints` | Position Bound | Fails if canonical term is ranked lower than maximum target position (e.g. `<=1`) |
| `score_constraints` | Confidence Bound | Fails if confidence score drops below minimum threshold (e.g. `min: 0.80`) |
| `ordering` | Sequential Ranking | Fails if temporal sequence is not ordered chronologically (`newest_first`) |
| `contains_all` | Conflict Surface | Fails if both disagreeing facts are not surfaced simultaneously |

---

## 2. Capability Package Architecture

KQC is organized into independent capability packages:

```
sdks/python/rqb/datasets/
├── aliases/         (dataset.json, README.md, CHANGELOG.md, VERSION)
├── conflicts/       (dataset.json, README.md, CHANGELOG.md, VERSION)
├── temporal/        (dataset.json, README.md, CHANGELOG.md, VERSION)
├── synthesis/       (dataset.json, README.md, CHANGELOG.md, VERSION)
├── multilingual/    (dataset.json, README.md, CHANGELOG.md, VERSION)
├── traces/          (dataset.json, README.md, CHANGELOG.md, VERSION)
├── code/            (dataset.json, README.md, CHANGELOG.md, VERSION)
└── architecture/    (dataset.json, README.md, CHANGELOG.md, VERSION)
```

---

## 3. Evidence-Based Engineering Directive

> **Retrieval engine optimizations MUST be driven by empirical RQB benchmark failures, not by subjective intuition.**

Engineering effort targets empirical shortfalls surfaced by RQB:
- **Primary Target**: Synonym & Alias Coverage ($0.71$ vs $0.75$ threshold target).
- **Action Plan**: Expand `brain-services` alias graph normalization and synonym dictionary lookup.
