#!/bin/bash
set -e

echo "=== Starting brain Release Packaging ==="

# 1. Bundle TUI JS assets
echo "Bundling CLI TUI..."
cd cli
bun run build
cd ..

# 2. Compile release Rust binary
echo "Compiling Rust binary in release mode..."
cd daemon
PYO3_PYTHON=$(pwd)/.venv/bin/python cargo build --release --bin brain
cd ..

# 3. Assemble release directory
echo "Assembling package files..."
mkdir -p release
cp daemon/target/release/brain release/brain
cp INSTALL.md release/INSTALL.md
cp UPGRADE.md release/UPGRADE.md
cp README.md release/README.md

# 4. Create tarball
TAR_NAME="brain-$(uname -s | tr '[:upper:]' '[:lower:]')-$(uname -m).tar.gz"
echo "Creating tarball: ${TAR_NAME}..."
tar -czf ${TAR_NAME} -C release brain INSTALL.md UPGRADE.md README.md

# 5. Cleanup
rm -rf release
echo "=== Packaging Completed! Package generated: ${TAR_NAME} ==="
