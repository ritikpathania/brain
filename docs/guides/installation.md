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
- **Rust (Cargo)**: Rust toolchain (version >= 1.75).
- **Python (>= 3.10)**: For out-of-band semantic NLP extraction.
- **`uv`**: Fast Python package manager (`curl -LsSf https://astral.sh/uv/install.sh | sh`).

---

## 1. Install via `cargo install` (Recommended)
You can compile and install `brain` directly from source:

```bash
# Clone the repository
git clone https://github.com/org/brain.git brain-engine
cd brain-engine

# Initialize Python virtualenv and PyO3 dependencies
make setup

# Build and install the binary globally
PYO3_PYTHON=$(pwd)/daemon/.venv/bin/python cargo install --path apps/brain --bin brain
```

Once installed, the binary `brain` will be available in your Cargo bin directory (usually `~/.cargo/bin/`).

---

## 2. Verification
To verify the installation, check the tool version:

```bash
brain --version
```

---

## 3. Quick Start
1. **Start the background memory engine daemon**:
   ```bash
   brain daemon
   ```
2. **Launch the interactive terminal UI**:
   ```bash
   brain
   ```
