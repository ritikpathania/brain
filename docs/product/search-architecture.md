---
status: active
owner: retrieval
canonical: true
review_cycle: quarterly
last_reviewed: 2026-07-30
applies_to: v0.8+
---

# Product Search Architecture Specification

This document defines the product-level search requirements and user experience goals for the Brain hybrid search engine.

---

## Functional Requirements
- **Sub-50ms Latency**: Queries return candidate sets in under 50ms.
- **Hybrid Fusion**: Combines FTS5 lexical keyword matches with dense BLOB vector embeddings.
- **Temporal Relevance**: Re-weights candidate rankings based on fact validity timelines and decay functions.
