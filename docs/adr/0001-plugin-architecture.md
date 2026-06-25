# ADR 0001: Plugin-Based Architecture for Standalone Relational Memory Engine

## Status
Accepted

## Context
The memory engine needs to integrate diverse embedding models, LLM providers, search algorithms, and persistent databases. Hardcoding provider-specific logic in the core results in poor modularity, limits extensions, and introduces unnecessary dependencies.

## Decision
We implement a stable, dynamic plugin-based architecture.
- Core capabilities (LLMs, Embeddings, Storage, Retrieval) are defined as Rust traits.
- The daemon embeds Python using PyO3 and Maturin to provide low-overhead interoperability.
- Python dynamic plugins can be dropped into a user-specific configuration directory (`~/.brain/plugins`) and are loaded at runtime.
- Long-running plugin executions (e.g. Python FFI, DuckDB synchronizations) are offloaded to dedicated thread pools via `tokio::task::spawn_blocking` to protect high-frequency ingestion hot paths.

## Consequences
- Enables custom providers without core modifications.
- Protects sub-millisecond core retrieval latency from slow external plugins.
- Slight overhead crossing the Rust/Python FFI boundaries.
