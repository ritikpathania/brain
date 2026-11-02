---
status: active
owner: release
canonical: true
review_cycle: quarterly
last_reviewed: 2026-11-02
applies_to: v1.1+
---

# Brain Product Distribution Guide

This document describes the structure, packaging, verification, and installation of the official Brain product distribution archives.

## 1. Product Distribution Architecture

Brain is distributed as a self-contained, relocatable application bundle containing both native executables and their runtime dependencies:

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

The distribution enforces strict, acyclic cryptographic verification before any execution:
1. **Frontend**: Standalone executable SHA-256 is recorded in `frontend/brain-frontend/manifest.json`.
2. **Daemon & Python**: The `brain-daemon` binary and `libpython3.15` dylib are recorded in `runtime-manifest.json`.
3. **Python Files**: All standard library files are inventoried in `runtime/python-files.json`.
4. **Product Manifest**: `product-manifest.json` directly hashes the CLI, frontend manifest, daemon manifest, python files manifest, and all notice files, forming a closed content inventory.

## 2. Installation & Usage

### 2.1 Extraction
Extract the full product archive:
```bash
tar -xzf brain-macos-arm64.tar.gz
```

### 2.2 Execution
Add the `bin/` directory to your `PATH` or invoke `bin/brain` directly:
```bash
export PATH="/path/to/brain-dist/bin:$PATH"
brain ui
```

> [!IMPORTANT]
> Do not copy only `bin/brain` to another location without its sibling directories. The CLI launcher resolves its sibling `brain-daemon` and `frontend/brain-frontend` relative to its canonical binary location. Creating a symbolic link to `bin/brain` is fully supported.

## 3. Build & Packaging Verification

To build and verify the full product package locally:

```bash
./package.sh
```

`package.sh` executes the full quality pipeline:
1. Workspace quality verification (`cargo xtask verify`, `cargo xtask docs all`, `cargo fmt`, clippy with `-D warnings`).
2. First-party frontend quality (`bun install --frozen-lockfile`, `typecheck`, `test`).
3. Recompiles release binaries with remapped path prefixes to prevent host path leaks.
4. Standalone frontend compilation and host path scanner.
5. Product assembly via `scripts/package-brain-product.sh`.
6. Relocatable daemon verification via `scripts/verify-daemon-runtime.sh`.
7. Clean-extraction interactive PTY smoke verification (3 repetitions) via `scripts/verify-brain-product.sh`.

## 4. Platform Verification Status

- **macOS arm64 (`aarch64-apple-darwin`)**: **PASS** (re-baselined under Rust 1.99.1, CPython 3.15.2, Bun 1.4.4; verified with 3/3 fresh extraction cycles).
- **Linux x86_64 (`x86_64-unknown-linux-gnu`)**: **PENDING** (configured in release CI matrix; pending CI execution evidence).
