# Brain project execution

Use the [architecture overview](docs/architecture/overview.md) for workspace boundaries and the [roadmap](ROADMAP.md) for planned work.

| Area | Maintained contract | Verification |
| --- | --- | --- |
| Runtime, storage and retrieval | `BrainRuntime` owns engine composition. `ApplicationRuntime` wires startup and `BrainApplication` exposes the application interface. The daemon serves versioned UDS contracts. Retrieval prioritizes stored constraints within scoped context budgets. | `cargo xtask verify`, plus the [retrieval baseline suite](docs/subsystems/retrieval.md) for ranking changes. |
| Terminal frontend | The first-party frontend owns presentation and consumes typed daemon contracts through its adapter. | Frontend typecheck and tests, then the [real-daemon E2E gate](docs/subsystems/tui.md). |
| SDKs and protocol adapters | SDKs consume the versioned contracts. Transport adapters use the shared application interface. | SDK tests and workspace contract verification. |
| Product delivery | Native packages verify their own CLI, daemon, standalone frontend and relocated Python runtime. | The [release checklist](docs/guides/release-checklist.md) requires exact-source archive verification. |

Record fixes and their maintained checks in the [invariant ledger](REGRESSION-PREVENTION-AND-INVARIANTS.md).

## Phase ownership

Before starting a phase, record its dependencies, acceptance checks, reviewer, and exclusive file ownership here. The [roadmap](ROADMAP.md) tracks the work sequence and the [test matrix](TEST_INFRA.md) maps checks to the contracts above.
