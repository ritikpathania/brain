---
status: active
owner: cli
canonical: true
review_cycle: quarterly
last_reviewed: 2026-07-30
applies_to: v0.8+
---

# Brain Installation Guide

`brain` is a unified, standalone relational memory engine developer tool. It can be installed and ready to use in under two minutes using Cargo.

---

## Prerequisites
Ensure the following are installed and available on your system `PATH`:
- **Rust (Cargo)**: Rust toolchain (MSRV >= 1.99.1).
- **Python (>= 3.10)**: For out-of-band semantic NLP extraction.
- **`uv`**: Fast Python package manager (`curl -LsSf https://astral.sh/uv/install.sh | sh`).

---

## 1. Install via Pre-built Product Archive (Recommended)

Brain is distributed as a self-contained product bundle containing the native CLI, relocatable daemon runtime, and standalone compiled terminal frontend with zero external Bun/Node runtime prerequisites.

Download the latest release archive for your platform, extract it, and add `bin/` to your PATH:

```bash
tar -xzf brain-macos-arm64.tar.gz
export PATH="/path/to/brain-dist/bin:$PATH"
brain ui
```

See [Product Distribution Guide](product-distribution.md) for full details on bundle structure and verification.

---

## 2. Install from Source via `cargo install` (Development)
You can compile and install `brain` directly from source:

```bash
# Clone the repository
git clone https://github.com/ritikpathania/brain.git brain-engine
cd brain-engine

# Initialize Python virtualenv and PyO3 dependencies
make setup

# Build and install binaries globally into Cargo bin (~/.cargo/bin/)
PYO3_PYTHON=$(pwd)/daemon/.venv/bin/python cargo install --path apps/brain --bin brain
PYO3_PYTHON=$(pwd)/daemon/.venv/bin/python cargo install --manifest-path daemon/Cargo.toml --bin brain-daemon
```

Once installed, the binaries `brain` and `brain-daemon` will be available in your Cargo bin directory (usually `~/.cargo/bin/`).

---

## 3. Verification
To verify the installation, check the tool version:

```bash
brain --version
brain-daemon --version
```

---

## 4. Quick Start
1. **Start the background memory engine daemon**:
   ```bash
   brain daemon start
   ```
2. **Launch the interactive terminal UI**:
   ```bash
   brain
   ```
