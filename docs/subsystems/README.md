# Subsystem Domain Navigation Hubs

Welcome to the Brain Subsystem Navigation Hubs. This directory provides workflow-oriented landing pages for each major subsystem in the project.

Each hub connects the subsystem's **Architecture** ("Why?"), **Reference** ("Exactly how?"), **Owning Crate**, **Active ADRs/RFCs**, **Operations**, and **Test Invariants** into a single landing page without moving underlying files.

---

## Subsystem Navigation Catalog

* **[Storage Subsystem Hub](storage.md)**
  * **Domain**: SQLite engine, WAL configuration, vector BLOB indexing, search projection synchronization.
  * **Crate**: [`crates/brain-storage`](../../crates/brain-storage/README.md)

* **[Knowledge Compiler Hub](compiler.md)**
  * **Domain**: Mutation authority, reconciliation passes, AST generation, duplicate/contradiction detection.
  * **Crate**: [`crates/brain-domain`](../../crates/brain-domain/README.md) & [`crates/brain-services`](../../crates/brain-services/README.md)

* **[Retrieval Engine Hub](retrieval.md)**
  * **Domain**: Hybrid BM25 lexical search, IVF vector search, Reciprocal Rank Fusion (RRF), temporal scoring.
  * **Crate**: [`crates/brain-services`](../../crates/brain-services/README.md)

* **[Terminal User Interface (TUI) Hub](tui.md)**
  * **Domain**: Ratatui differential immediate-mode rendering, alt-screen event loop, theme tokens, peek panel.
  * **Crate**: [`crates/brain-tui`](../../crates/brain-tui/README.md)

* **[IPC & Wire Protocol Hub](protocol.md)**
  * **Domain**: Unix Domain Sockets (UDS), frame codecs, versioned requests, HTTP Prometheus telemetry (`/metrics`).
  * **Crates**: [`crates/brain-integrations`](../../crates/brain-integrations/README.md), [`daemon`](../../daemon/README.md)

* **[Plugin Architecture Hub](plugins.md)**
  * **Domain**: Maturin / PyO3 FFI boundaries, dynamic Python extractors, custom LLM provider traits.
  * **Crates**: [`crates/brain-plugins`](../../crates/brain-plugins/README.md), [`crates/brain-python`](../../crates/brain-python/README.md), [`sdks/python`](../../sdks/python/README.md)
