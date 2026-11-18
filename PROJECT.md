# Brain project execution

## Phase 6 legacy frontend retirement

The first-party CLI and release contract is recorded in the [architecture overview](docs/architecture/overview.md) and the [Phase 6 design and acceptance plan](docs/designs/phase-6-legacy-frontend-retirement.md).

| Work | Owner | Scope |
| --- | --- | --- |
| Preservation and retirement | Phase 6 implementer | Private preservation receipt, `packages/brain-shell`, and `scripts/ptySmokeInc*.py` |
| Current frontend verification and handbook | Phase 6 implementer | `packages/brain-frontend/tests/integration/frontendCi.test.ts`, roadmap, and active frontend documentation |
| Independent verification | Phase 6 reviewer | Read-only candidate diff, historical recovery paths, and acceptance evidence |

## Phase 6 native release follow-through

| Work | Owner | Exclusive files |
| --- | --- | --- |
| Ordinary CI consumer, invalid parent bridge command and executable regression | CI implementer | `.github/workflows/ci.yml`, `package.sh`, `packages/brain-frontend/tests/integration/frontendCi.test.ts`, `packages/brain-frontend/tests/integration/releaseWorkflow.test.ts` |
| Scope, handbook and evidence | Root implementer | Phase 6 design, release and distribution guides, `GUIDELINES.md`, roadmap, invariant ledger and dead ends |
| Independent acceptance review | Read-only reviewer | Candidate diff, existing frontend contracts and verification evidence |

The native follow-through ran the workspace and frontend gates on the preserved Phase 6 baseline. GitHub verified the exact candidate on Linux and macOS, and the [acceptance record](docs/guides/product-distribution.md#phase-6-native-archive-acceptance--2026-11-18) preserves the job URLs, source SHA, and archive digests. That run preceded the integration of the local provider and retrieval work.

## Main branch consolidation

| Work | Owner | Exclusive files |
| --- | --- | --- |
| Backend integration | Backend implementer | `apps/`, `crates/`, `daemon/`, `Cargo.lock`, `xtask/` |
| Handbook reconciliation | Documentation implementer | Root handbook files other than `PROJECT.md`, `docs/`, and `qa/reports/` |
| CI, verification and Git | Coordinator | `.github/`, `package.sh`, `packages/brain-frontend/tests/integration/`, `PROJECT.md`, snapshot and ref operations |

The integrated tree keeps Phase 6's verified retirement and package fixes and incorporates the current Brain source and tests from the local checkout. Existing historical documents and QA captures remain available. The private oracle, research tree and generated files remain local. The combined tree must pass the offline workspace and frontend gates before `main` is pushed. Branch tips must be reachable from the published `main` before their refs are retired.
