#!/usr/bin/env bash
# Assemble, verify, and package the complete Brain product distribution.
#
# Orchestrates:
# 1. Quality & documentation gates (xtask verify, docs all)
# 2. First-party frontend quality (frozen install, typecheck, unit tests)
# 3. Release binary compilation (CLI + daemon with remapped path prefix)
# 4. Required real-daemon E2E, standalone build, and frontend artifact verification
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

python_library_directory="$("$daemon_python" - <<'PY'
import sysconfig
print(sysconfig.get_config_var("LIBDIR"))
PY
)"
export PYO3_PYTHON="$daemon_python"
export DYLD_FALLBACK_FRAMEWORK_PATH="$python_library_directory"

# Cargo's encoded flags preserve each remap as one argument, even with spaces.
rustflags=()
if [[ -n "${CARGO_ENCODED_RUSTFLAGS:-}" ]]; then
    IFS=$'\x1f' read -r -a rustflags <<< "$CARGO_ENCODED_RUSTFLAGS"
elif [[ -n "${RUSTFLAGS:-}" ]]; then
    read -r -a rustflags <<< "$RUSTFLAGS"
fi
rustflags+=("--remap-path-prefix=$HOME=." "--remap-path-prefix=$repository_root=.")
release_rustflags="$(IFS=$'\x1f'; printf '%s' "${rustflags[*]}")"

target_triple="$(rustc -vV | awk '/^host: / { print $2 }')"
brain_version="$("$daemon_python" -c 'import tomllib; print(tomllib.load(open("apps/brain/Cargo.toml", "rb"))["package"]["version"])')"
release_cli="$repository_root/target/release/brain"
release_daemon="$repository_root/target/release/brain-daemon"

echo "=== Brain Product Orchestrator ==="
echo "Target: $target_triple"

echo "Step 1: Running quality gates..."
LD_LIBRARY_PATH="$python_library_directory${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}" \
    DYLD_FALLBACK_LIBRARY_PATH="$python_library_directory${DYLD_FALLBACK_LIBRARY_PATH:+:$DYLD_FALLBACK_LIBRARY_PATH}" \
    cargo xtask verify
cargo xtask docs all

echo "Step 2: Checking first-party frontend..."
(
    cd packages/brain-frontend
    bun install --frozen-lockfile
    bun run typecheck
    bun run test
)

echo "Step 3: Compiling release binaries with remapped prefixes..."
CARGO_ENCODED_RUSTFLAGS="$release_rustflags" cargo build --release --package brain --package brain-daemon --locked

echo "Step 4: Running required frontend E2E and building the standalone artifact..."
BRAIN_CLI_BIN="$release_cli" BRAIN_DAEMON_BIN="$release_daemon" \
    LD_LIBRARY_PATH="$python_library_directory${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}" \
    DYLD_FALLBACK_LIBRARY_PATH="$python_library_directory${DYLD_FALLBACK_LIBRARY_PATH:+:$DYLD_FALLBACK_LIBRARY_PATH}" \
    ./scripts/verify-frontend-e2e.sh
(
    cd packages/brain-frontend
    bun run build
)
./scripts/verify-frontend-artifact.sh \
    --artifact packages/brain-frontend/dist/brain-frontend \
    --brain-version "$brain_version" \
    --target "$target_triple"

staging_parent="$(mktemp -d /tmp/brain-product-pkg.XXXXXX)"
artifact_root="$staging_parent/brain-dist-stage"
cleanup() {
    rm -rf "$staging_parent"
}
trap cleanup EXIT INT TERM

echo "Step 5: Assembling product layout and manifests..."
./scripts/package-brain-product.sh \
    --cli "$release_cli" \
    --daemon "$release_daemon" \
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
