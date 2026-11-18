# Brain Documentation Index

## Documentation Philosophy

The handbook covers current behavior and planned work. The invariant ledger records past failures that still inform maintained checks.

---

## Directory Index

* [Root architecture](../ARCHITECTURE.md), [engineering guidelines](../GUIDELINES.md), [roadmap](../ROADMAP.md), and [invariant ledger](../REGRESSION-PREVENTION-AND-INVARIANTS.md) provide entry points for maintainers.

*   **[subsystems/](subsystems/README.md)**: **Workflow-Oriented Subsystem Navigation Hubs** (Storage, Compiler, Retrieval, TUI, Protocol, Plugins).
*   **[architecture/](architecture/README.md)**: Enduring core architectural overview, invariants, capability concepts, and runtime designs.
    *   **[overview.md](architecture/overview.md)**: The canonical system layout and request life cycle walkthrough.
    *   **[principles.md](architecture/principles.md)**: Enduring rules of the system architecture (e.g. single authority, stateless adapters).
*   **[reference/](reference/README.md)**: Technical specifications (UDS socket wire schemas, storage definitions, etc.).
*   **[guides/](guides/README.md)**: Installation, maintenance, and release checklists.
*   **[governance/](governance/GOVERNANCE.md)**: Repository governance policy, metadata standards, and CI rules.
*   **[product/](product/README.md)**: Product requirements and targets.
*   **[benchmarks/](benchmarks/README.md)**: Performance benchmark suites and measurement guidance.

## Development and verification references

* [Capability development policy](development/CAPABILITY_DEVELOPMENT.md)
* [Knowledge Quality Corpus guidelines](benchmark/DATASET_GUIDELINES.md) and [dataset contributions](benchmark/CONTRIBUTING_DATASETS.md)
* [Evidence-based release acceptance proposal](process/evidence_based_release_acceptance.md)
