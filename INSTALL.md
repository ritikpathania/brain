# brain Installation Guide

`brain` is a unified, standalone relational memory engine developer tool. It can be installed and ready to use in under two minutes using Cargo.

---

## Prerequisites
Ensure the following are installed and available on your system `PATH`:
- **Rust (Cargo)**: Rust toolchain (version >= 1.70).
- **Python (>= 3.9)**: For out-of-band semantic NLP extraction.

---

## 1. Install via `cargo install` (Recommended)
You can compile and install `brain-v2` directly from source:

```bash
# Clone the repository
git clone <repo-url> brain-engine
cd brain-engine

# Build and install the binary globally
# Note: Ensure PYO3_PYTHON points to your python3 interpreter
PYO3_PYTHON=$(which python3) cargo install --path apps/brain-v2 --bin brain
```

Once installed, the binary `brain` will be available in your Cargo bin directory (usually `~/.cargo/bin/`).

---

## Verification
To verify the installation, check the tool version and diagnostics:

```bash
# Check version
brain --version
```

---

## Quick Start
1. **Start the background memory engine**:
   ```bash
   brain daemon
   ```
2. **Launch the interactive terminal UI**:
   ```bash
   brain
   ```
