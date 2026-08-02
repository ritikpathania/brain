#!/bin/bash
set -e

echo "=== Starting brain Release Packaging ==="

# 0. Quality Gate: Verify workspace before packaging
echo "Running quality verification gate..."
PYO3_PYTHON=$(pwd)/daemon/.venv/bin/python cargo xtask verify

# 1. Compile release Rust binaries
echo "Compiling Rust CLI binary in release mode..."
PYO3_PYTHON=$(pwd)/daemon/.venv/bin/python cargo build --release --package brain

echo "Compiling Rust Daemon binary in release mode..."
PYO3_PYTHON=$(pwd)/daemon/.venv/bin/python cargo build --release --manifest-path daemon/Cargo.toml --bin brain-daemon

# 2. Assemble release directory
echo "Assembling package files..."
rm -rf release
mkdir -p release
cp target/release/brain release/brain
if [ -f daemon/target/release/brain-daemon ]; then
    cp daemon/target/release/brain-daemon release/brain-daemon
elif [ -f target/release/brain-daemon ]; then
    cp target/release/brain-daemon release/brain-daemon
else
    echo "ERROR: brain-daemon binary not found after build!"
    exit 1
fi
cp docs/guides/installation.md release/INSTALL.md
cp README.md release/README.md
cp CHANGELOG.md release/CHANGELOG.md
cp LICENSE release/LICENSE 2>/dev/null || true
cp -r docs release/docs

# 3. Create tarball
TAR_NAME="brain-$(uname -s | tr '[:upper:]' '[:lower:]')-$(uname -m).tar.gz"
echo "Creating tarball: ${TAR_NAME}..."
tar -czf ${TAR_NAME} -C release .

# 4. Cleanup
rm -rf release
echo "=== Packaging Completed! Package generated: ${TAR_NAME} ==="


