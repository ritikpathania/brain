# Brain Shell Verification Gates

The first-party terminal frontend uses `bun run typecheck` and `bun run test` from `packages/brain-frontend`. The shared E2E gate runs actual CLI, daemon, and PTY tests with zero required skips. [Current product verification](../guides/product-distribution.md#3-build--packaging-verification)

## Historical shell reproduction

The legacy package, its 36 manual PTY scripts, and 35 tracked increment PTY snapshots are preserved at commit `3d2cbf9924c18ccdad1be85f7c55ba43cdfa5d7f`. Recover that revision into a separate worktree from the candidate repository.

```bash
git worktree add --detach ../brain-legacy-reference 3d2cbf9924c18ccdad1be85f7c55ba43cdfa5d7f
git show 3d2cbf9924c18ccdad1be85f7c55ba43cdfa5d7f:docs/development/BRAIN_SHELL_VERIFICATION_GATES.md
```

Many historical PTY scripts use the original machine's absolute checkout path and overwrite captures when run. Use a disposable reproduction tree and adapt those paths before attempting a replay. The tracked symlink and gitlink preserve their reference identities. The private preservation receipt covers the local oracle, legacy working files, and the separate synthetic-fixture Git bundle. [Preservation scope](../designs/phase-6-legacy-frontend-retirement.md)

The two historical comparator tests still refer to `oracle_parity_engine`. Recover `scripts/oracle_parity_engine.py` from `cc6f5acbdaa5ebfeeaf0c9ea834d5f7b1b7f5e5b` into a disposable module directory and put that directory on `PYTHONPATH` to rerun them with `pyte`. Those synthetic tests cover comparator behavior. The historic parity result file is available at `095b4a885bf9d59d9c40b3ffc27dd058179895d7:packages/brain-shell/src/test/product_parity_gate_results.json`.
