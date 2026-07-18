#!/bin/bash
set -e

echo "=== Starting brain Release Packaging ==="

# 1. Compile release Rust binary
echo "Compiling Rust binary in release mode..."
PYO3_PYTHON=$(pwd)/daemon/.venv/bin/python cargo build --release --package brain

# 2. Assemble release directory
echo "Assembling package files..."
mkdir -p release
cp target/release/brain release/brain
cp INSTALL.md release/INSTALL.md
cp UPGRADE.md release/UPGRADE.md
cp README.md release/README.md

# 3. Create tarball
TAR_NAME="brain-$(uname -s | tr '[:upper:]' '[:lower:]')-$(uname -m).tar.gz"
echo "Creating tarball: ${TAR_NAME}..."
tar -czf ${TAR_NAME} -C release brain INSTALL.md UPGRADE.md README.md

# 4. Cleanup
rm -rf release
echo "=== Packaging Completed! Package generated: ${TAR_NAME} ==="

