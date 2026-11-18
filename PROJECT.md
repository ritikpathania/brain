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

Execution uses the preserved Phase 6 commit as the baseline. Unrelated local provider and retrieval edits remain in the original checkout. Run focused CI tests, frontend typecheck/unit tests, `cargo xtask verify`, then commit and push the exact candidate to `release-verify/brain-phase6` for native Linux and macOS archive verification. Record job URLs, source SHA, artifact digests and smoke counts after completion. Verify that the original working-file and index snapshot remains unchanged.
