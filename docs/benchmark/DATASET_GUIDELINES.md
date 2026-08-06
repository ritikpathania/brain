# Knowledge Quality Corpus (KQC) — Dataset Guidelines

The **Knowledge Quality Corpus (KQC)** represents the ground-truth evaluation data powering the Retrieval Quality Benchmark (RQB). KQC is treated as a first-class asset separate from the RQB execution engine.

---

## 1. Dataset Package Standards

All KQC dataset packages in `sdks/python/rqb/datasets/` must conform to the following standards:

1. **Independent Package Versioning**: Every dataset file must specify a `version` field (e.g. `v3.1.0`).
2. **Canonical Entity Normalization**: Canonical labels must be unambiguous, unique, and documented.
3. **No Synthetic Oversimplification**: Include real-world variation (abbreviations, typos, context-mixed queries).
4. **Deterministic Expected Facts**: Define precise verification keys (`expected_facts`, `canonical`, `expected_newer_keyword`).

---

## 2. Capability Package Taxonomies

| Capability Package | Directory / File | Target Corpus Scale | Purpose |
|---|---|:---:|---|
| **Knowledge Retrieval** | `datasets/aliases.json`, `typos.json` | 800+ Scenarios | Canonical entity mapping, shorthand resolution, typo tolerance |
| **Reasoning & Conflicts** | `datasets/conflicts.json`, `synthesis.json` | 350+ Scenarios | Disagreeing ADR handling, multidimensional topic synthesis |
| **Memory Evolution** | `datasets/temporal.json` | 100+ Sequences | Recency-weighted timestamp ordering |
| **User Traces** | `datasets/traces.json` | 500+ Traces | Anonymized historical user query traces |

---

## 3. Evidence-Based Engineering Directive

> **Retrieval engine optimizations MUST be driven by empirical RQB benchmark failures, not by subjective intuition.**

When an RQB run surfaces a shortfall (such as Synonym & Alias coverage at 0.71 vs 0.75 target), engineering effort targets that specific vector until the quality threshold is satisfied.
