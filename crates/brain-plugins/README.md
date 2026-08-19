# brain-plugins

## Purpose
Extensible plugin manager, lifecycle registry, and tool dispatching.

## Responsibilities
* Manage registration, capability discovery, and lifecycle execution of plugins and tools.
* Provide sandboxed execution boundaries for external extensions.

## Boundaries & Constraints
* **Allowed Dependencies:** `brain-domain`, `brain-core`, `async-trait`.
* **Forbidden Dependencies:** `brain-storage`, `brain-tui`, `pyo3`.

## Public API & Facades
* `PluginManager`, `ToolRegistry`, `PluginRegistryLookup`.

## Invariants Protected
* Capability-oriented extensibility.

## Canonical References
* Specification: `../../docs/reference/plugin-api.md`

## Testing & Verification
* `cargo test -p brain-plugins`

## Maintainer
See `CODEOWNERS`.
