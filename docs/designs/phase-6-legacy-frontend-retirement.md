---
status: active
owner: cli
canonical: true
review_cycle: static
last_reviewed: 2026-11-17
applies_to: v1.1+
---

# Phase 6 legacy frontend retirement

The Phase 5 archives contain the first-party frontend. The Phase 6 inventory verifies that the CLI, CI, and local packaging also resolve that frontend, and that the real `brain ui` test passes with the legacy package physically absent. [Caller inventory](2026-11-17_DRIFT_REPORT.md) · [Native archive evidence](../guides/product-distribution.md#4-platform-verification-status)

Retire the tracked `packages/brain-shell` package and 36 manual `scripts/ptySmokeInc*.py` entry points from the candidate. Treat their presentation-specific assertions and captured snapshots as historical evidence. Use the maintained first-party `bun run test` and `scripts/verify-frontend-e2e.sh` for the current product. Preserve the Rust no-fallback regression, the external parity archives, and the working tree that holds the private oracle.

The complete tracked legacy source, paths, file modes, and 35 historical increment PTY captures remain recoverable from `3d2cbf9924c18ccdad1be85f7c55ba43cdfa5d7f`. The original checkout's legacy source edits, untracked vendor tree, and local verification tooling are preserved in a private hash-verified archive. The nested synthetic fixture has a separate verified Git bundle. The external `claude-upstream` target stays at its existing location.

## Implementation and acceptance

1. Verify the vendor manifest, copy local legacy inputs without following the upstream symlink, compare the archive entries byte-for-byte, and verify the synthetic fixture bundle.
2. Add a canonical CI regression for retired tracked paths. Observe failure on the old tree and then remove the inventoried legacy package and manual PTY entry points from the candidate.
3. Update the handbook and roadmap. Preserve historical parity material and its original scope. Verify legacy paths are absent from the candidate index, runtime source is unaffected, and historical tree objects are available.
4. Run `cargo xtask verify`, first-party frontend typecheck and tests, and the real-daemon E2E gate. Build and smoke-test the candidate's native macOS archive through `package.sh`. Record which commit and platforms the new verification covers.
