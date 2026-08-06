# Contributing to the Knowledge Quality Corpus (KQC)

We welcome contributions to expand the Knowledge Quality Corpus (KQC)! Adding new evaluation scenarios improves benchmark representativeness across real-world developer queries.

---

## How to Add a New Scenario

### 1. Adding Canonical Aliases (`sdks/python/rqb/datasets/aliases.json`)

```json
{
  "canonical": "SQLite",
  "aliases": ["sqlite3", "sqlite db", "embedded relational database"],
  "sample_text": "SQLite is configured in WAL mode for local relational memory storage."
}
```

### 2. Adding Conflicting ADR Pairs (`sdks/python/rqb/datasets/conflicts.json`)

```json
{
  "id": "conflict-transport",
  "fact_a": "ADR-010 selected gRPC over HTTP/2 for IPC.",
  "fact_b": "ADR-018 migrated IPC transport to Unix Domain Sockets (UDS).",
  "query": "IPC transport mechanism",
  "expected_facts": ["gRPC", "Unix Domain Sockets"]
}
```

---

## Review Process

1. **Format Validation**: Ensure JSON syntax is valid.
2. **Deterministic Keys**: Verify `expected_facts` or `canonical` strings match ingested text accurately.
3. **No Threshold Drift**: Adding dataset scenarios must not bypass policy thresholds.
