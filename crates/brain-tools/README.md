# brain-tools

## Purpose
Core system tools (filesystem, git, shell, memory, LLM).

## Responsibilities
* Implement individual executable tools mapping to the abstract `Tool` interface.
* Expose tool metadata including timeout limits, permissions, and idempotency guarantees.
* Orchestrate system-level action executions safely.

## Dependencies
* **Allowed:** `brain-domain`, `brain-core`.
* **Forbidden:** TUI components or direct database engines.

## Public Interfaces
* System tools implementations (e.g. Git, Shell, Filesystem, Memory, LLM wrappers) and registries.

## Owner
Extensibility Team
