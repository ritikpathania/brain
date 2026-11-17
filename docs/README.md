# Brain Documentation Index

## Documentation Philosophy

> Active documentation describes the system as it exists today. The archive indexes historical design discussions, migration plans, and superseded architectures. Some records retain their original paths for provenance.

---

## Directory Index

*   **[subsystems/](subsystems/README.md)**: **Workflow-Oriented Subsystem Navigation Hubs** (Storage, Compiler, Retrieval, TUI, Protocol, Plugins).
*   **[architecture/](architecture/README.md)**: Enduring core architectural overview, invariants, capability concepts, and runtime designs.
    *   **[overview.md](architecture/overview.md)**: The canonical system layout and request life cycle walkthrough.
    *   **[principles.md](architecture/principles.md)**: Enduring rules of the system architecture (e.g. single authority, stateless adapters).
*   **[reference/](reference/README.md)**: Technical specifications (UDS socket wire schemas, storage definitions, etc.).
*   **[design/](design/README.md)**: Historical TUI design contracts, theme tokens, and interaction models.
*   **[guides/](guides/README.md)**: Installation, maintenance, and release checklists.
*   **[governance/](governance/GOVERNANCE.md)**: Repository governance policy, metadata standards, and CI rules.
*   **[engineering/](engineering/README.md)**: Active engineering implementation plans and technical design specifications.
*   **[product/](product/README.md)**: Product requirements and release notes.
*   **[benchmarks/](benchmarks/README.md)**: Performance benchmark run reports.
*   **[archive/](archive/README.md)**: Historical migration files, deprecated designs, sprint walkthroughs, and obsolete ADR logs.

## Development and verification references

* [Capability development policy](development/CAPABILITY_DEVELOPMENT.md)
* [Knowledge Quality Corpus guidelines](benchmark/DATASET_GUIDELINES.md) and [dataset contributions](benchmark/CONTRIBUTING_DATASETS.md)
* [Evidence-based release acceptance proposal](process/evidence_based_release_acceptance.md)
* [First-party frontend Phase 5 design](superpowers/specs/2026-10-01-brain-frontend-phase-5-design.md) and [implementation plan and evidence](superpowers/plans/2026-10-01-brain-frontend-phase-5.md)
* [Phase 6 legacy frontend retirement](designs/phase-6-legacy-frontend-retirement.md), [caller inventory](designs/2026-11-17_DRIFT_REPORT.md), and [historical reproduction](development/BRAIN_SHELL_VERIFICATION_GATES.md)
