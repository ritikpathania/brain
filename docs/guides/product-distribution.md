---
status: active
owner: release
canonical: true
review_cycle: quarterly
last_reviewed: 2026-11-17
applies_to: v1.1+
---

# Brain Product Distribution Guide

This document describes the structure, packaging, verification, and installation of the official Brain product distribution archives.

## 1. Product Distribution Architecture

The product archive contains the CLI, daemon, standalone frontend, and their bundled runtime dependencies. The standalone frontend includes Bun. The extracted product still uses operating-system libraries.

```
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

The product verifier checks the acyclic manifest chain before executing an extracted product.
1. **Frontend**: Standalone executable SHA-256 is recorded in `frontend/brain-frontend/manifest.json`.
2. **Daemon & Python**: The `brain-daemon` binary and `libpython3.15` dylib are recorded in `runtime-manifest.json`.
3. **Python Files**: All standard library files are inventoried in `runtime/python-files.json`.
4. **Product Manifest**: `product-manifest.json` directly hashes the CLI, frontend manifest, daemon manifest, python files manifest, and all notice files, forming a closed content inventory.

## 2. Installation & Usage

### 2.1 Verify and extract the archive

The tagged workflow uses `brain-macos-arm64.tar.gz` on macOS arm64 and `brain-linux-x86_64.tar.gz` on Linux x86_64. Download the archive and matching `.sha256` file into the same directory. For macOS arm64, run the following.

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
> Keep `brain-dist/` together. The CLI resolves its sibling `brain-daemon` and the standalone frontend relative to its canonical executable path. You can put a symlink to `bin/brain` on your PATH.

## 3. Build & Packaging Verification

### Local verification

Use Rust 1.99.1, Bun 1.4.4, and the frozen daemon environment with CPython 3.15.2. Run the orchestrator from the repository root.

```bash
(cd daemon && uv sync --frozen --no-install-project)
./package.sh
```

The orchestrator runs workspace and documentation quality gates, installs frozen frontend dependencies, and runs frontend typecheck and unit tests. It builds the native CLI and daemon with the locked Cargo dependencies. The shared `scripts/verify-frontend-e2e.sh` gate uses those explicit binaries and requires a passing real-daemon E2E suite with zero skipped or todo tests before standalone frontend compilation.

The orchestrator verifies standalone identity and host paths, assembles the closed product, and proves daemon relocation with an extracted compiled Python extension. It then runs three fresh extractions and interactive PTY smokes against the complete archive and writes the SHA-256 checksum after success. Those smokes exercise launch from an unrelated working directory, bundled frontend and daemon resolution, generation and saved history, and cleanup. The local macOS orchestrator names its archive `brain-darwin-arm64.tar.gz`.

### Native workflow

[Release Packaging](../../.github/workflows/release.yml) runs the native macOS arm64 and Linux x86_64 matrix for a `v*` tag push, a push to a `release-verify/**` branch, or manual dispatch. Each native job uses uv to resolve the pinned standalone CPython from `daemon/.python-version` for compilation and bundling. It runs the real-daemon E2E gate, standalone and daemon verification, and clean-extraction product smoke against the archive it uploads. macOS requires three extractions and Linux requires one. Both platform jobs run to completion independently. A version-matching tag and successful quality and native matrix jobs are required for publication. Verification-only branch and manual runs upload GitHub Actions artifacts.

## 4. Platform Verification Status

| Target | Recorded evidence |
| --- | --- |
| macOS arm64 (`aarch64-apple-darwin`) | **PASS** on 2026-11-17 for clean source commit `6cd25406d1e2818267f527fc405cc267464554ae`. The 12-case frontend E2E gate, CPython relocation and compiled-extension probe, and all three clean-extraction product smokes passed. Archive SHA-256 `30c507c7c16a95c6cd40e2283c705bfda9e7dc7c611c419859e1409670a9f02c`. |
| Linux x86_64 (`x86_64-unknown-linux-gnu`) | Native archive verification remains pending in the checked-in record. The matrix configuration is in place. |

Record the exact source commit, native target, completed test result, and archive SHA-256 for each candidate. Use the [Phase 5 implementation plan](../superpowers/plans/2026-10-01-brain-frontend-phase-5.md#november-2026-verification-record) for the historical macOS evidence.
