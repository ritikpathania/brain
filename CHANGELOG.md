# Changelog

All notable changes to the Brain Relational Memory Engine will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

---

## [1.0.0] - 2026-07-31

### Added
- **Relational Memory Engine**: SQLite FTS5 lexical + vector BLOB hybrid search fusion runtime (`BrainRuntime`).
- **Knowledge Compiler**: 6-pass deterministic reconciliation engine for ontology graph nodes and edges.
- **Terminal User Interface (TUI)**: Ratatui alt-screen console with typewriter queue streaming, command palette (`Ctrl+P`), sidebar session navigation, and markdown rendering.
- **Background IPC Listener Daemon (`brain-daemon`)**: Unix Domain Socket (UDS) newline-delimited JSON IPC listener supporting legacy and versioned streaming frames.
- **HTTP Observability Server**: Diagnostics and Prometheus exposition endpoints on `http://127.0.0.1:8080` (`/health`, `/ready`, `/status`, `/diagnostics`, `/metrics`, `/metrics/json`).
- **Python Plugin Subsystem**: FFI bridge powered by PyO3 and Maturin for dynamic `~/.brain/plugins/*.py` extensions with isolated exception handling.

### Fixed
- **CLI `--socket-path` Flag Binding**: Updated `apps/brain` to propagate `--socket-path` parameter via `BRAIN_SOCKET_PATH` into `UdsClient`, `socket_is_alive()`, and configuration displays.
- **Daemon Socket Startup Race Condition**: Added socket readiness polling in `DaemonHost::start()` so pipeline invocations (`brain daemon start && brain health`) succeed deterministically.
- **Makefile Build Target Alignment**: Aligned `make build-daemon` to build `./brain-daemon` and `make build-brain` to build the standalone `brain` CLI binary.
- **Installation Documentation**: Added `uv` dependency and `make setup` virtualenv initialization instructions to `docs/guides/installation.md`.
