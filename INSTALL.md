# brain Installation Guide

`brain` is a unified, standalone relational memory engine developer tool. It can be installed and ready to use in under two minutes on macOS and Linux.

---

## Prerequisites
Ensure the following are installed and available on your system `PATH`:
- **Python (>= 3.12)**: For out-of-band semantic NLP extraction.
- **Bun (>= 1.0)**: For TUI runtime execution. (Install via `curl -fsSL https://bun.sh/install | bash`).

---

## 1. Install via `cargo install` (Recommended for Rust developers)
If you have Cargo installed on your system, you can compile and install `brain` directly from source:

```bash
# Clone the repository
git clone <repo-url> brain-engine
cd brain-engine

# Build and install the binary globally
# Note: Ensure PYO3_PYTHON points to your python3 interpreter
PYO3_PYTHON=$(which python3) cargo install --path daemon --bin brain
```

Once installed, the binary `brain` will be available in your Cargo bin directory (usually `~/.cargo/bin/`).

---

## 2. Install via `uv tool install` (Recommended for Python developers)
`uv` is an extremely fast Python package and tool manager. You can install `brain` globally into a managed environment:

```bash
# Install directly from the local project directory
uv tool install ./daemon --with-editable
```

`uv` will invoke Maturin, build the binary, and expose `brain` directly in your global tool path.

---

## 3. Install via `bun install -g` / `npm install -g` (Recommended for JS/TS developers)
If you work in Node/Bun environments, you can install the CLI global wrapper which automatically builds and links the binary:

```bash
# Install globally from the cli subdirectory
cd cli
bun install -g
```

The global installer runs a `postinstall` step that automatically builds the release binary and makes `brain` available globally.

---

## Verification
To verify the installation, check the tool version and diagnostics:

```bash
# Check version
brain version

# Run system diagnostic checks
brain diagnostics

# Check config paths
brain config
```

---

## Quick Start
1. **Start the background memory engine**:
   ```bash
   brain daemon start
   ```
2. **Launch the interactive terminal UI**:
   ```bash
   brain
   # or explicitly:
   brain ui
   ```
3. **Stop the daemon**:
   ```bash
   brain daemon stop
   ```
