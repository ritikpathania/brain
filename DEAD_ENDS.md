# Dead ends

## 1. Observing Loader Variables Through a Misclassified Fixture Script

- **Iteration** — Phase 6 native release follow-through, 2026-11-18.
- **Approach tried** — Place JavaScript fixture code directly in a `.sh` file launched through Bun while exercising the package orchestrator.
- **Why it failed** — Bun selected shell parsing for that extension. A system-shell wrapper would also strip DYLD variables on macOS and obscure the child boundary being measured.
- **Resolution** — Keep executable fixture code in `.js` files and expose the expected `.sh` paths through symlinks. Assert exact child environments under both empty and existing inherited loader paths.
- **Files touched** — `packages/brain-frontend/tests/integration/releaseWorkflow.test.ts`; no product runtime change was needed for this fixture correction.
