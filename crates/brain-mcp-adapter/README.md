# brain-mcp-adapter

## Purpose
Model Context Protocol (MCP) server adapter exposing Brain capabilities to AI coding agents.

## Responsibilities
* Translate Model Context Protocol JSON-RPC requests to Brain ApplicationRuntime commands.
* Expose memory search, entity inspection, and graph query MCP tools.

## Boundaries & Constraints
* **Allowed Dependencies:** `brain-domain`, `brain-core`, `brain-application`, `serde_json`.
* **Forbidden Dependencies:** `brain-storage`, `brain-tui`, `pyo3`.

## Public API & Facades
* MCP Server handler.

## Invariants Protected
* Stateless protocol adapter isolation (ADR-020).

## Canonical References
* Specification: `../../docs/architecture/overview.md`

## Testing & Verification
* `cargo test -p brain-mcp-adapter`

## Maintainer
See `CODEOWNERS`.
