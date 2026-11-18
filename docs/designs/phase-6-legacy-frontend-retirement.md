---
status: active
owner: cli
canonical: true
review_cycle: static
last_reviewed: 2026-11-18
applies_to: v1.1+
---

# Phase 6 legacy frontend retirement

The Phase 5 archives contain the first-party frontend ([`packages/brain-frontend`](../../packages/brain-frontend)). The Phase 6 inventory verifies that the CLI, CI, and local packaging also resolve that frontend, and that the real `brain ui` test passes with the legacy package absent from the tracked Git index. [Caller inventory](2026-11-17_DRIFT_REPORT.md) · [Native archive evidence](../guides/product-distribution.md#4-platform-verification-status)

Retire the tracked `packages/brain-shell` package and 36 manual `scripts/ptySmokeInc*.py` entry points from the tracked repository index. Treat their presentation-specific assertions and captured snapshots as historical evidence. Use the maintained first-party `bun run test` and `scripts/verify-frontend-e2e.sh` for the current product. Preserve the Rust no-fallback regression, the external parity archives, and the local working files that hold the private oracle.

The complete tracked legacy source, paths, file modes, and 35 historical increment PTY captures remain recoverable from `3d2cbf9924c18ccdad1be85f7c55ba43cdfa5d7f`. The original checkout's legacy source edits, untracked vendor tree, and local verification tooling are preserved in a private hash-verified archive (`.superpowers/phase6-preservation.zm6t8di0/legacy-working-tree.tar.gz`). The nested synthetic fixture has a separate verified Git bundle (`.superpowers/phase6-preservation.zm6t8di0/synthetic-repo.bundle`). The external `claude-upstream` target stays at its existing location.

## Implementation and acceptance

1. Verify the vendor manifest, copy local legacy inputs without following the upstream symlink, compare the archive entries byte-for-byte, and verify the synthetic fixture bundle.
2. Add a canonical CI regression (`packages/brain-frontend/tests/integration/frontendCi.test.ts`) for retired tracked paths (`git ls-files -- packages/brain-shell ':(glob)scripts/ptySmokeInc*.py'`), and verify that ordinary CI (`sdk-and-packaging` in `.github/workflows/ci.yml`) provisions Bun `1.4.4` and invokes extracted `brain-dist/bin/brain-daemon` and `brain-dist/bin/brain` from an owned temporary extraction directory.
3. Reconcile the [Release Checklist](../guides/release-checklist.md), [Product Distribution Guide](../guides/product-distribution.md), and [Project Roadmap](../../ROADMAP.md) with the maintained first-party frontend (`Ctrl+O` session picker in `keyRouter.ts`, bounded no-replay recovery, and toolchain provenance).
4. Run `cargo xtask verify`, first-party frontend typecheck and tests, and the real-daemon E2E gate. Build and smoke-test the candidate's native macOS archive through `package.sh`. Record which commit and platforms the verification covers.

## Acceptance record

The Phase 6 retirement was verified at `c81ece666bd63a8345ddedad19ef004ec162b27c` (documented at `221446efdf3db51cc08ba8d44bf1dad1c7e15511` on `codex/brain-phase6`) and preserved as the baseline for this release-verification candidate. Unrelated provider and retrieval edits remain in the original working checkout. A fresh local macOS arm64 `./package.sh` run on `c81ece66` exited zero: it passed all 1,196 nextest cases, frontend typecheck, 120 frontend tests, 12 real-daemon E2E tests with zero skips, relocated CPython startup and `_dbm` import, and all three clean-extraction PTY smokes. The verified archive contains one first-party frontend and zero retired entries. SHA-256 for `brain-darwin-arm64.tar.gz` is `b3677f24952b98aaf6f8d6d7d3828f6e20ea3cb405ae006d65898c58b1c48c89`.

**Toolchain Provenance Note**: While `Cargo.toml`, `rust-toolchain.toml`, and GitHub Actions pin Rust `1.99.1`, the maintainer macOS host maps `~/.rustup/toolchains/1.99.1-aarch64-apple-darwin` via symlink to `stable-aarch64-apple-darwin` (`rustc 1.100.0 (0ef1bc386 2026-11-09)` / `cargo 1.100.0 (3acc79c0c 2026-11-05)`), which is recorded in the local macOS archive's `product-manifest.json`.

**Native Linux Gate**: The native Linux (`x86_64-unknown-linux-gnu`) archive record in the distribution guide (`c618a56e0a054cd7b668468ee0c2ed719b1352813a4147e08f09593bc64ddcf9`, run `44020955003`) covers the earlier Phase 5 commit `f706d8eda57164661a8dbbeea9b0148bbb428868`. A native Linux package run via `.github/workflows/release.yml` on the integrated Phase 6 commit remains required before final release publication. [Native release evidence](../guides/product-distribution.md#4-platform-verification-status)

The private preservation archive verified 2,229 entries against the working files and all 1,922 vendor hashes against the oracle manifest. Its SHA-256 is `ecb8d4cdf5d6b28b820d8af0dd9e347338b324cfdf0f1968d1d2d0e0a3a139bc`. The synthetic fixture was independently cloned and checked at `cc91f01b96561579610a5f9deb08826ddc3d7853`. The bundle SHA-256 is `594850a5f561d05454e3c5f12a62108324b898ef997b7beaea50baba9c0cfc34`. The archive and bundle are stored with the local preservation receipt (`.superpowers/phase6-preservation.zm6t8di0/receipt.json`) outside the tracked candidate.

## Native verification follow-through

The current increment carries the synchronized CI consumer, release checklist and toolchain provenance onto the preserved Phase 6 baseline. The CI consumer must execute the extracted `brain-dist/bin` layout under `bash -e -o pipefail`, accept a complete large inventory, reject missing required files, and propagate package or binary failures. Packaging uses the pinned managed standalone CPython on both native hosts. No product runtime or frozen presentation changes are planned.

Acceptance requires focused consumer regression tests, frontend typecheck and owned tests, `cargo xtask verify`, and successful native Linux/macOS archive jobs for the exact committed source. Branch verification uploads artifacts without tag publication. The original checkout's working files and index are checked against a pre-execution snapshot.
