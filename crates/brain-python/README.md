# brain-python

## Purpose
Strictly encapsulated PyO3 CPython FFI boundary and plugin loader.

## Responsibilities
* Encapsulate raw PyO3 bindings and CPython 3.12 GIL runtime management.
* Expose safe, Rust-native traits for semantic extraction and custom Python plugins.
* Prevent PyO3 types and symbols from leaking into upstream workspace crates.

## Boundaries & Constraints
* **Allowed Dependencies:** `brain-domain`, `brain-core`, `pyo3`.
* **Forbidden Dependencies:** `brain-storage`, `brain-tui`, `brain-services`.

## Public API & Facades
* Safe Python runtime wrappers (`PythonPluginLoader`, `ExtractorApi`).

## Invariants Protected
* Strict PyO3 encapsulation barrier (Invariant 6).

## Canonical References
* Specification: `../../docs/reference/plugin-api.md`

## Testing & Verification
* `cargo test -p brain-python`

## Maintainer
See `CODEOWNERS`.
