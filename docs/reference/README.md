# Reference Manuals & Specifications

This directory contains the detailed reference manuals, schemas, and specifications for the Relational Memory Engine's internal and external interfaces.

## Document Index

*   **[protocol.md](protocol.md)**: Details the Unix Domain Socket (UDS) streaming protocol, including versioned requests, monotonic sequence tracking, compatibility constraints, and the frontend adapter event contract. Rendering remains frontend-owned.
*   **[plugin-api.md](plugin-api.md)**: Defines the Python plugin traits and PyO3 FFI boundary contracts compiled through Cargo. Used for creating and integrating custom semantic extractors or downstream memory processors.
*   **[storage.md](storage.md)**: Documents the SQLite database schema and read projections. Explains raw BLOB storage for vector embeddings, relational graph indices, and search projection synchronization.
*   **[benchmarking.md](benchmarking.md)**: Outlines the benchmarking methodology, Criterion test harness configs, and performance profiling guidelines for tracking execution hot-paths and regression checks.
*   **[cli-ux-comparison.md](cli-ux-comparison.md)**: Terminal & CLI UX design comparison across AI coding tools.
*   **[generation_workflow.md](generation_workflow.md)**: Deterministic contract generation workflow from Rust to TypeScript/Python.
