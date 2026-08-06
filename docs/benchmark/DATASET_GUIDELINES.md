# Knowledge Quality Corpus (KQC) — Assertion Language & Guidelines

The **Knowledge Quality Corpus (KQC)** represents the ground-truth evaluation data powering the Retrieval Quality Benchmark (RQB). KQC is treated as a first-class asset separate from the RQB execution engine.

---

## 1. Composable Logical Assertion Language

KQC dataset items define composable logical assertions using boolean operators (`all`, `any`, `none`, `exactly_one`):

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

```json
{
  "id": "alias-version-flexibility",
  "expected": {
    "any": [
      { "contains": "PostgreSQL" },
      { "contains": "PostgreSQL 17" }
    ]
  }
}
```

---

## 2. Categorized Assertion Taxonomy

Assertions are categorized into seven domain classes:

```
Assertion
├── RetrievalAssertion   (contains, must_not_contain)
├── RankingAssertion     (rank.max, ordering)
├── ScoringAssertion     (score.min)
├── TemporalAssertion    (newest_first)
├── VisibilityAssertion  (surface_both)
├── StabilityAssertion   (stability_ratio_min)
└── PerformanceAssertion (latency.max_ms)
```

---

## 3. Five-Layer Quality Architecture

```
EBRA Gate           ──► Release correctness & release gate
RQB Engine          ──► Benchmark execution & harness (FROZEN v2.2.0)
KQC Datasets        ──► Ground-truth corpus packages (aliases, conflicts, etc.)
Assertion Language  ──► Declarative evaluation rules (all/any/none/rank/score)
Retrieval Engine    ──► System under test (brain-services hybrid search)
```

---

## 4. Capability Package Architecture

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

## 5. Evidence-Based Engineering Directive

> **Retrieval engine optimizations MUST be driven by empirical RQB benchmark failures, not by subjective intuition.**

Engineering effort directly targets empirical shortfalls surfaced by RQB:
- **Primary Target**: Synonym & Alias Coverage ($0.71$ vs $0.75$ threshold target).
- **Action Plan**: Expand `brain-services` alias graph normalization and synonym dictionary lookup.
