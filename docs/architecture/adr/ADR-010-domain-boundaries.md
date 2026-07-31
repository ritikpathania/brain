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

## Alternatives Considered
* **Anemic Domain**: Storing raw data structs in `brain-domain` and placing all business logic and validations in service layers. Rejected because it fragments logic and duplicates validations.
* **Layered Architecture**: Allowing domain packages to call services/repositories directly. Rejected because it introduces circular dependencies and complicates isolated testing.

## Related ADRs
* [ADR-004 (DDD Core Invariants)](file:///Users/ritikpathania/Developer/PyCharm/brain/docs/archive/historical-adrs/ADR-004.md)
* [ADR-012 (Value Objects)](file:///Users/ritikpathania/Developer/PyCharm/brain/docs/architecture/adr/ADR-012-value-objects.md)
* [ADR-016 (Pure Transformation Pipelines)](file:///Users/ritikpathania/Developer/PyCharm/brain/docs/architecture/adr/ADR-016-pure-transformation-pipelines.md)

## Expected Stability
Long-term. 
* **Review Trigger**: Moving from single-repository deployment to a distributed microservices ecosystem.
