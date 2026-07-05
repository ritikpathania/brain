# ADR-010: Domain Boundaries

## Status
Accepted

## Context
As the relational memory engine grew, we needed to ensure it remained scalable, testable, and free from circular or leaking dependencies. Hardcoding async runtimes, database drivers, FFI layers, or networking protocols inside the core logic makes business rules difficult to test and binds the domain logic to specific infrastructure choices.

## Decision
We enforce a strict boundary between the core domain layer (`crates/brain-domain`) and the services layer (`crates/brain-services`):
1. `brain-domain` lies at the bottom of the dependency tree. It has **zero outgoing dependencies** on async runtimes, database connections, loggers, or FFI modules.
2. Business invariants, validations, and mathematical scoring computations must be encapsulated directly within domain aggregates and entities (e.g. `Edge`, `Conversation`, `DecisionTreeDefinition`).
3. External integrations (SQLite storage, Python FFI, IPC streaming, TUI layouts) are orchestrations owned exclusively by `brain-services`.

## Consequences
* **Testability**: Domain models can be tested instantly in-memory without starting transaction suites or mocking database connections.
* **Architecture Integrity**: Clean separation prevents circular dependency issues and ensures new developer additions do not accidentally leak service details into the core domain.
* **Overhead**: Minor translation boundary overhead between domain value objects and database/network DTO models.
