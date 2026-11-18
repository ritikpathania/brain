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
The product manifest records the versions returned by `rustc --version` and `cargo --version` during assembly. Verify those values against the release toolchain pin before accepting an archive.

### 3.2 Local Verification (`./package.sh`)

Run the orchestrator from the repository root:

```bash
(cd daemon && uv sync --frozen --no-install-project)
./package.sh
```

The orchestrator runs workspace and documentation quality gates, installs frozen frontend dependencies, and runs frontend typecheck and unit tests. The workspace gate runs the daemon with the managed CPython library path. The orchestrator builds the native CLI and daemon with the locked Cargo dependencies. The shared `scripts/verify-frontend-e2e.sh` gate uses those explicit binaries and requires a passing real-daemon E2E suite with zero skipped or todo tests before standalone frontend compilation.

The orchestrator verifies standalone identity and host paths, assembles the closed product, and proves daemon relocation with an extracted compiled Python extension. It then runs three fresh extractions and interactive PTY smokes against the complete archive and writes the SHA-256 checksum after success. The local macOS orchestrator names its archive `brain-darwin-arm64.tar.gz`.

### 3.3 Ordinary CI & Native Release Workflows

- **Ordinary CI (`.github/workflows/ci.yml`)**: The `sdk-and-packaging` job provisions Rust `1.99.1`, Python `3.15.2`, `uv`, and Bun `1.4.4`, runs `bash package.sh`, verifies the closed archive paths (`brain-dist/bin/brain`, `brain-dist/bin/brain-daemon`, `brain-dist/frontend/brain-frontend/brain-frontend`, and `brain-dist/product-manifest.json`), extracts into a unique runner-temporary directory, executes `brain-dist/bin/brain-daemon --version` and `brain-dist/bin/brain health` there, and removes that extraction on exit. It uses the pinned uv-managed standalone CPython for compilation and packaging.
- **Native Release Matrix (`.github/workflows/release.yml`)**: Runs the native macOS arm64 (`aarch64-apple-darwin`) and Linux x86_64 (`x86_64-unknown-linux-gnu`) matrix for a `v*` tag push, a push to a `release-verify/**` branch, or manual dispatch. Each native job uses `uv` to resolve the pinned standalone CPython from `daemon/.python-version` for compilation and bundling, runs `scripts/verify-frontend-e2e.sh`, builds and verifies the standalone frontend and daemon runtime relocation, and runs clean-extraction PTY smokes (3 on macOS, 1 on Linux).

## 4. Platform Verification Status

Verify each release candidate on macOS arm64 and Linux x86_64 using its exact source commit. Record the job URLs, archive SHA-256 digests, actual tool versions and clean-extraction smoke results in the release record. Ordinary CI also builds and checks an archive on each platform. Use the [release checklist](release-checklist.md) for the release acceptance criteria.
