---
status: active
owner: cli
canonical: true
review_cycle: quarterly
last_reviewed: 2026-11-17
applies_to: v1.1+
---

# Brain Installation Guide

## 1. Install a product archive

The product bundle includes the native CLI, relocatable daemon CPython runtime, and standalone terminal frontend. The frontend runs with its bundled Bun runtime.

Download the archive and checksum for your platform and follow the [distribution guide](product-distribution.md#2-installation--usage) to verify and extract them. Keep the extracted directory together.

```bash
export PATH="/path/to/brain-dist/bin:$PATH"
brain --version
brain-daemon --version
brain ui
```

## 2. Build and run from a checkout

Install the repository's Rust 1.99.1 toolchain, uv and CPython 3.15.2 for the daemon, and Bun 1.4.4 for the source frontend. Then run the following from the repository root.

```bash
make setup
(cd packages/brain-frontend && bun install --frozen-lockfile)
make build-daemon
make build-brain
./target/debug/brain ui
```

The checkout launcher resolves the frontend from the repository that contains its executable. It uses Bun from `PATH`, or the explicit `BRAIN_FRONTEND_RUNTIME` executable. On macOS, if the development daemon needs a Python framework search path, set it from the daemon's interpreter before launch.

```bash
export DYLD_FALLBACK_FRAMEWORK_PATH="$(daemon/.venv/bin/python -c 'import sysconfig; print(sysconfig.get_config_var("LIBDIR"))')"
```

## 3. Install CLI binaries with Cargo (development)

Complete the dependency setup from the previous section. The following commands install binaries into Cargo's bin directory.

```bash
PYO3_PYTHON="$PWD/daemon/.venv/bin/python" cargo install --locked --path apps/brain --bin brain
PYO3_PYTHON="$PWD/daemon/.venv/bin/python" cargo install --locked --path daemon --bin brain-daemon
export BRAIN_FRONTEND_ROOT="$PWD/packages/brain-frontend"
brain ui
```

For source UI launch with these globally installed binaries, keep the checkout and set `BRAIN_FRONTEND_ROOT` to its frontend package directory.
