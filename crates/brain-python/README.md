# brain-python

## Purpose
PyO3 host execution block and in-process Python adapter.

## Responsibilities
* Manage the embedded CPython runtime lifecycle and Global Interpreter Lock (GIL) safety.
* Implement GIL-releasing FFI wrappers for Python agent execution.
* Strictly encapsulate the `pyo3` dependency so that no other crate compiles against it.

## Dependencies
* **Allowed:** `brain-core`, `pyo3`.
* **Forbidden:** `brain-storage`, `brain-services`, `brain-tui`.

## Public Interfaces
* Python runtime handles and execution executors.

## Owner
Python Integration Lead
