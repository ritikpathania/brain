---
status: active
owner: cli
canonical: false
review_cycle: static
last_reviewed: 2026-11-17
applies_to: v1.1+
---

# Phase 6 legacy frontend caller inventory

Audited the repair branch at `a0dcb934ef52ba78e84aec205d5b26cf9fb0b958` and the local-only callers in the active `main` workspace. The two checksum-verified Phase 5 archives were built from `f706d8eda57164661a8dbbeea9b0148bbb428868`. An independent reviewer checked the caller inventory. [Native release evidence](../guides/product-distribution.md#4-platform-verification-status)

## `2026-10-01-brain-frontend-phase-5-design.md`

No drift detected.

## `BRAIN_SHELL_VERIFICATION_GATES.md`

- The old guide's canonical verification language describes a manual, machine-specific legacy workflow. It includes direct legacy bundling and PTY scripts, while current CI tests the first-party package. [Legacy verification guide](../development/BRAIN_SHELL_VERIFICATION_GATES.md) · [Canonical CI](../../.github/workflows/ci.yml)
- The tracked parity documentation refers to `packages/brain-shell/vendor/claude/` and `product_parity_gate_results.json`, which are absent from the clean repair checkout. The two root comparator tests import `oracle_parity_engine`, which is also absent there. These references need provenance or a maintained replacement before anyone relies on them as runnable gates. [Baseline receipt](../architecture/claude_feature_baseline_certification.json) · [Parity plan](../architecture/SUPERSET_IMPLEMENTATION_PLAN.md) · [Comparator test](../../tests/test_oracle_comparator.py)

## Callers and retention

| Caller or asset | Verified use and disposition |
| --- | --- |
| Canonical runtime and release | Zero production, build, or CI callers of `packages/brain-shell` in the audited checkout. Keep the Rust launcher regression that asserts failure instead of falling back to the legacy shell. [Launcher regression](../../apps/brain/tests/frontend_launcher_tests.rs) |
| Manual legacy PTY harness | The legacy package exposes `start` and `test:pty`. The 36 tracked `scripts/ptySmokeInc*.py` files invoke the legacy package or consume its fixtures. They correspond to increments 1, 2, 3, 4, 5, 6, 9, 11, 15, 17, 19, 20, 21, 24, 26–32, and 34–48. The legacy package contains 35 tracked increment PTY snapshots. Preserve those scripts and fixtures together until their historical or maintained status is decided. The package and scripts are available at the pinned pre-retirement commit. [Historical reproduction](../development/BRAIN_SHELL_VERIFICATION_GATES.md) |
| Active workspace launcher and oracle | The untracked `scripts/brain-shell-launcher.sh` explicitly executes the legacy package. The local `scripts/oracle/verify_vendor_manifest.py` opens its frozen vendor directory. The local manifest SHA-256 is `89b755fdb1e65535ee85433d58f228b996c571412cf6d32a2c3f8e7a2982d0bb`. All 1,922 vendor files matched, with zero missing, modified, or extra files. Preserve this tree and its manifest as a matched set. |
| Dirty legacy tree | The active workspace has 54 modified tracked entries and 1,987 untracked files under `packages/brain-shell`. Of those untracked files, 1,922 are the verified vendor tree and 65 are other local files. These inputs are outside the committed repair snapshot. |
| External oracle pointer and synthetic fixture | The tracked `packages/brain-shell/claude-upstream` symlink points outside the repository and is excluded from legacy typechecking. The tracked synthetic fixture is a Git link to `cc91f01b96561579610a5f9deb08826ddc3d7853` without a `.gitmodules` entry. Retain the reference identity and confirm the fixture's provenance before moving it. |
| Standalone evidence and documentation | The frontend parity archive, historical Claude UX baseline and traces, and the README preview are separate assets that still have documentation and provenance value. [Parity archive](../archive/frontend-parity/README.md) · [Claude UX baseline](../archive/claude_ux/baseline.json) · [README preview reference](../../README.md) |

## Verification without the legacy shell

- Both native archives passed their SHA-256 checks and contained exactly one `brain-dist/frontend/brain-frontend/brain-frontend` executable. Neither contained `brain-shell`, `claude-upstream`, or `vendor/claude` entries. [Native release evidence](../guides/product-distribution.md#4-platform-verification-status)
- A disposable sparse Git checkout at `a0dcb934` excluded `packages/brain-shell`. It reused the installed Bun dependencies and pinned CPython from the repair checkout. Typecheck, all 119 frontend unit and integration tests, and all 12 real-daemon tests passed with zero skips. The real `brain ui` test launched from an unrelated working directory without `BRAIN_FRONTEND_ROOT`. It used the CLI and daemon from the verified macOS release archive. [Launcher E2E tests](../../packages/brain-frontend/tests/e2e/cliLauncher.test.ts)
- A preliminary plain Git export could run typecheck but lacked the Git metadata and pinned interpreter required by the packaging tests. The successful sparse checkout provided both inputs.

## Pre-retirement decision

The inventory is complete for the repair branch and the identified local workspace callers. Canonical delivery is independent of the legacy package. Keep Phase 6 deletion on hold while the legacy launcher, manual verification suite, and frozen oracle have live or reproducible uses. Before a removal candidate, record what replaces each maintained check and preserve the local-only artifacts with their existing hashes. The historical parity evidence stays with its original scope.

Phase 6 subsequently applied the retirement and preservation plan. [Phase 6 design and acceptance plan](phase-6-legacy-frontend-retirement.md)
