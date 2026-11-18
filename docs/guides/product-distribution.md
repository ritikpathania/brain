---
status: active
owner: release
canonical: true
review_cycle: quarterly
last_reviewed: 2026-11-18
applies_to: v1.1+
---

# Brain Product Distribution Guide

This document describes the structure, packaging, verification, toolchain provenance, and installation of the official Brain product distribution archives.

## 1. Product Distribution Architecture

The product archive contains the CLI, daemon, standalone frontend, and their bundled runtime dependencies. The standalone frontend includes Bun (`1.4.4`). The extracted product still uses operating-system libraries.

```text
brain-dist/
├── bin/
│   ├── brain                 # Native CLI launcher
│   └── brain-daemon          # Relocatable background daemon
├── frontend/
│   └── brain-frontend/
│       ├── brain-frontend    # Standalone Bun 1.4.4 compiled terminal frontend
│       └── manifest.json     # Frontend component manifest (F)
├── runtime/
│   ├── python/               # Relocatable CPython 3.15.2 runtime prefix
│   └── python-files.json     # Python runtime leaf file inventory (P)
├── licenses/
│   ├── Brain-LICENSE.txt     # Brain core license
│   ├── Bun-LICENSE.txt       # Bun runtime license
│   ├── CPython-LICENSE.txt   # CPython base distribution license
│   ├── ...                   # Frontend and transitive dependency notices
│   └── notice-inventory.json # Audited notice manifest
├── runtime-manifest.json     # Daemon component manifest (D)
└── product-manifest.json     # Closed product manifest (M)
```

### 1.1 Closed Content Inventory & Acyclic Hash Graph

The product verifier checks the acyclic manifest chain before executing an extracted product:
1. **Frontend**: Standalone executable SHA-256 is recorded in `frontend/brain-frontend/manifest.json`.
2. **Daemon & Python**: The `brain-daemon` binary and `libpython3.15` dylib/shared library are recorded in `runtime-manifest.json`.
3. **Python Files**: All standard library files are inventoried in `runtime/python-files.json`.
4. **Product Manifest**: `product-manifest.json` directly hashes the CLI, frontend manifest, daemon manifest, python files manifest, and all notice files, forming a closed content inventory.

## 2. Installation & Usage

### 2.1 Verify and extract the archive

The tagged workflow uses `brain-macos-arm64.tar.gz` on macOS arm64 and `brain-linux-x86_64.tar.gz` on Linux x86_64. Download the archive and matching `.sha256` file into the same directory. For macOS arm64, run the following:

```bash
shasum -a 256 -c brain-macos-arm64.tar.gz.sha256 && tar -xzf brain-macos-arm64.tar.gz
```

### 2.2 Execution

Add the `bin/` directory to your `PATH` or invoke `bin/brain` directly:

```bash
export PATH="/path/to/brain-dist/bin:$PATH"
brain ui
```

> [!IMPORTANT]
> Keep `brain-dist/` together. The CLI resolves its sibling `brain-daemon` and the standalone frontend relative to its canonical executable path. You can put a symlink to `bin/brain` on your `PATH`.

## 3. Build, Toolchain Provenance & Packaging Verification

### 3.1 Toolchain Baseline & Provenance Reconciliation

- **MSRV & CI Pin (`1.99.1`)**: `Cargo.toml` (`rust-version = "1.99.1"`), `rust-toolchain.toml` (`channel = "1.99.1"`), and GitHub Actions workflows (`.github/workflows/ci.yml` and `.github/workflows/release.yml` via `dtolnay/rust-toolchain@1.99.1`) enforce Rust `1.99.1` as the minimum supported Rust version and CI baseline alongside Bun `1.4.4` and CPython `3.15.2`.
- **Local Maintainer Host Provenance (`1.100.0`)**: On the maintainer macOS arm64 workstation, `~/.rustup/toolchains/1.99.1-aarch64-apple-darwin` is a symlink to `stable-aarch64-apple-darwin`, which updated to `rustc 1.100.0 (0ef1bc386 2026-11-09)` and `cargo 1.100.0 (3acc79c0c 2026-11-05)`. Because `scripts/package-brain-product.sh` records the output of `rustc --version` and `cargo --version` at assembly time into `product-manifest.json`, local macOS archives built on that host record `rustc 1.100.0` / `cargo 1.100.0`, whereas GitHub Actions native matrix archives record `rustc 1.99.1` / `cargo 1.99.1`.

### 3.2 Local Verification (`./package.sh`)

Run the orchestrator from the repository root:

```bash
(cd daemon && uv sync --frozen --no-install-project)
./package.sh
```

The orchestrator runs workspace and documentation quality gates, installs frozen frontend dependencies, and runs frontend typecheck and unit tests. It builds the native CLI and daemon with the locked Cargo dependencies. The shared `scripts/verify-frontend-e2e.sh` gate uses those explicit binaries and requires a passing real-daemon E2E suite with zero skipped or todo tests before standalone frontend compilation.

The orchestrator verifies standalone identity and host paths, assembles the closed product, and proves daemon relocation with an extracted compiled Python extension. It then runs three fresh extractions and interactive PTY smokes against the complete archive and writes the SHA-256 checksum after success. The local macOS orchestrator names its archive `brain-darwin-arm64.tar.gz`.

### 3.3 Ordinary CI & Native Release Workflows

- **Ordinary CI (`.github/workflows/ci.yml`)**: The `sdk-and-packaging` job provisions Rust `1.99.1`, Python `3.15.2`, `uv`, and Bun `1.4.4`, runs `bash package.sh`, verifies the closed archive paths (`brain-dist/bin/brain`, `brain-dist/bin/brain-daemon`, `brain-dist/frontend/brain-frontend/brain-frontend`, and `brain-dist/product-manifest.json`), extracts into a unique runner-temporary directory, executes `brain-dist/bin/brain-daemon --version` and `brain-dist/bin/brain health` there, and removes that extraction on exit. It uses the pinned uv-managed standalone CPython for compilation and packaging.
- **Native Release Matrix (`.github/workflows/release.yml`)**: Runs the native macOS arm64 (`aarch64-apple-darwin`) and Linux x86_64 (`x86_64-unknown-linux-gnu`) matrix for a `v*` tag push, a push to a `release-verify/**` branch, or manual dispatch. Each native job uses `uv` to resolve the pinned standalone CPython from `daemon/.python-version` for compilation and bundling, runs `scripts/verify-frontend-e2e.sh`, builds and verifies the standalone frontend and daemon runtime relocation, and runs clean-extraction PTY smokes (3 on macOS, 1 on Linux).

## 4. Platform Verification Status

### 4.1 Phase 5 Native Matrix Verification (`f706d8ed`)

Both platform jobs passed on 2026-11-17 for source commit `f706d8eda57164661a8dbbeea9b0148bbb428868` in [GitHub Actions run `44020955003`](https://github.com/ritikpathania/brain/actions/runs/44020955003). Each passed 119 frontend unit and integration tests, 12 real-daemon E2E tests with zero skips, and the CPython relocation and compiled-extension probes.

| Target | Clean-extraction PTY smoke | Archive SHA-256 |
| --- | --- | --- |
| macOS arm64 (`aarch64-apple-darwin`) | [3 of 3 passed](https://github.com/ritikpathania/brain/actions/runs/44020955003/job/133832910636) | `4a90244d949f88df98eaf872df5c12a26b7f8e639950f95ead54fa690c3e0cab` |
| Linux x86_64 (`x86_64-unknown-linux-gnu`) | [1 of 1 passed](https://github.com/ritikpathania/brain/actions/runs/44020955003/job/133832910589) | `c618a56e0a054cd7b668468ee0c2ed719b1352813a4147e08f09593bc64ddcf9` |

### 4.2 Phase 6 Retirement Verification (`c81ece66`)

The Phase 6 legacy frontend retirement candidate (`c81ece666bd63a8345ddedad19ef004ec162b27c`) passed its local macOS arm64 `./package.sh` verification (1,196 Rust nextest cases, frontend typecheck, 120 frontend tests, 12 real-daemon E2E tests with zero skips, and 3/3 clean-extraction PTY smokes) with archive SHA-256 `b3677f24952b98aaf6f8d6d7d3828f6e20ea3cb405ae006d65898c58b1c48c89`. This local run covers macOS. The native follow-through below records the Linux and macOS results for its successor commit.

## Phase 6 native archive acceptance — 2026-11-18

[Release Packaging run 44255402374](https://github.com/ritikpathania/brain/actions/runs/44255402374) completed successfully for exact product source `8a6d020df833e267e08c251dc3b3cd77a67ebbdc` on `release-verify/brain-phase6`. The quality job and both native archive jobs passed. Each native job passed 125 frontend tests, frontend typecheck and 12 real-daemon E2E tests with zero skips. Both passed the relocated CPython startup and compiled-extension probes. The branch run skipped tag publication.

| Target | Clean-extraction PTY smoke | Archive SHA-256 |
| --- | --- | --- |
| macOS arm64 | [3 of 3 passed](https://github.com/ritikpathania/brain/actions/runs/44255402374/job/134640415293) | `819101f42f0c9d459750dbb4b355af08053b126cb6b917c13d4157d29d14de98` |
| Linux x86_64 | [1 of 1 passed](https://github.com/ritikpathania/brain/actions/runs/44255402374/job/134640415320) | `81fa28e2caab3e00d3331da1be8c0d8a58043398ebd371ee000ee41ced8188f5` |

Both archives were downloaded and independently checked against their uploaded ZIP digests and enclosed checksums. Product and frontend manifests agree on clean source `8a6d020d`, zero untracked inputs and source fingerprint `55265e8ba1b92cbaa05c459b9f326b800385ad1f9234e3bc2faf58001222605c`. Both record actual Rust/Cargo 1.99.1, Bun 1.4.4 and CPython 3.15.2. Neither includes the retired frontend or local oracle. The ordinary CI packaging consumer was exercised through its actual shell with controlled archives and failure injection. The separate ordinary CI matrix was not triggered by this verification-only branch.

The local workspace gate also passed all 1,196 nextest cases with zero skips, strict Clippy, formatting, contracts, architecture checks and zero active documentation violations. The local frontend suite passed 125 cases, real-daemon E2E passed 12, and the SDK passed 13 with two existing deprecation warnings. The hosted Python quality suite passed 58 cases with three pre-existing daemon-dependent skips and eight warnings.

This receipt documents the tested product source above. A subsequent documentation-only receipt commit does not claim a separately rebuilt archive. Each future product release candidate still requires its own native verification.

The combined `main` source includes changes beyond this receipt. Its native archive acceptance requires a run for its own source commit.
