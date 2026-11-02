#!/usr/bin/env bash
# Assemble, verify, and package the complete Brain product distribution.
#
# Orchestrates:
# 1. Quality & documentation gates (xtask verify, docs all)
# 2. First-party frontend quality (frozen install, typecheck, tests, standalone build)
# 3. Release binary compilation (CLI + daemon with remapped path prefix)
# 4. Frontend artifact verification (hash, loader, and host-path scans)
# 5. Product assembly (CLI + daemon + standalone frontend + licenses + manifests)
# 6. Daemon relocation verification
# 7. Clean-extraction interactive PTY product smoke verification (3 repetitions)
# 8. Final archive and checksum generation

set -euo pipefail

repository_root="$(cd "$(dirname "$0")" && pwd -P)"
cd "$repository_root"

daemon_python="$repository_root/daemon/.venv/bin/python"
if [ ! -x "$daemon_python" ]; then
    echo "error: missing frozen daemon interpreter: $daemon_python" >&2
    exit 2
fi

python_library_directory="$($daemon_python - <<'PY'
import sysconfig
print(sysconfig.get_config_var("LIBDIR"))
PY
)"
export PYO3_PYTHON="$daemon_python"
export DYLD_FALLBACK_FRAMEWORK_PATH="$python_library_directory"
export RUSTFLAGS="${RUSTFLAGS:-} --remap-path-prefix=$HOME=."

target_triple="$(rustc -vV | awk '/^host: / { print $2 }')"

echo "=== Brain Product Orchestrator ==="
echo "Target: $target_triple"

echo "Step 1: Running quality gates..."
cargo xtask verify
cargo xtask docs all

echo "Step 2: Checking first-party frontend..."
(
    cd packages/brain-frontend
    bun install --frozen-lockfile
    bun run typecheck
    bun run test
    bun run build
)

echo "Step 3: Compiling release binaries with remapped prefixes..."
cargo build --release --package brain --package brain-daemon --locked

echo "Step 4: Verifying frontend standalone artifact..."
./scripts/verify-frontend-artifact.sh \
    --artifact packages/brain-frontend/dist/brain-frontend \
    --brain-version "1.1.0" \
    --target "$target_triple"

staging_parent="$(mktemp -d /tmp/brain-product-pkg.XXXXXX)"
artifact_root="$staging_parent/brain-dist-stage"
cleanup() {
    rm -rf "$staging_parent"
}
trap cleanup EXIT INT TERM

echo "Step 5: Assembling product layout and manifests..."
./scripts/package-brain-product.sh \
    --cli "$repository_root/target/release/brain" \
    --daemon "$repository_root/target/release/brain-daemon" \
    --frontend "$repository_root/packages/brain-frontend/dist/brain-frontend" \
    --output "$artifact_root" \
    --target "$target_triple"

echo "Step 6: Verifying daemon runtime relocation inside product..."
./scripts/verify-daemon-runtime.sh --artifact "$artifact_root" --repeat 1

echo "Step 7: Creating distribution archive..."
archive_name="brain-$(uname -s | tr '[:upper:]' '[:lower:]')-$(uname -m).tar.gz"
mkdir -p "$staging_parent/brain-dist"
cp -R "$artifact_root/"* "$staging_parent/brain-dist/"
tar -czf "$archive_name" -C "$staging_parent" brain-dist

echo "Step 8: Running clean-extraction product smoke verification (3 passes)..."
./scripts/verify-brain-product.sh \
    --archive "$archive_name" \
    --target "$target_triple" \
    --repeat 3

shasum -a 256 "$archive_name" > "$archive_name.sha256"
cat "$archive_name.sha256"

echo "=== Brain product packaging completed successfully: $archive_name ==="
