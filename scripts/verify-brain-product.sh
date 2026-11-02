#!/usr/bin/env bash
# Verify a complete Brain product delivery via fresh clean extraction and interactive PTY smoke.
#
# Supports:
#   scripts/verify-brain-product.sh --artifact <root> --target <triple>
#   scripts/verify-brain-product.sh --archive <tar.gz> --target <triple> [--repeat <count>]

set -euo pipefail

usage() {
    cat <<'EOF'
Usage:
  scripts/verify-brain-product.sh --artifact <directory> --target <triple>
  scripts/verify-brain-product.sh --archive <tar.gz> --target <triple> [--repeat <count>]

Requires PYO3_PYTHON to name the explicit CPython 3.15 build interpreter.
EOF
}

artifact_root=""
archive_file=""
target_triple=""
repeat=1

while [ "$#" -gt 0 ]; do
    case "$1" in
        --artifact)
            artifact_root="${2:-}"
            shift 2
            ;;
        --archive)
            archive_file="${2:-}"
            shift 2
            ;;
        --target)
            target_triple="${2:-}"
            shift 2
            ;;
        --repeat)
            repeat="${2:-}"
            shift 2
            ;;
        --help|-h)
            usage
            exit 0
            ;;
        *)
            echo "error: unknown argument: $1" >&2
            usage >&2
            exit 2
            ;;
    esac
done

if [ -z "$target_triple" ]; then
    echo "error: --target is required" >&2
    usage >&2
    exit 2
fi

if [ -z "$artifact_root" ] && [ -z "$archive_file" ]; then
    echo "error: either --artifact or --archive is required" >&2
    usage >&2
    exit 2
fi

if [ -n "$artifact_root" ] && [ -n "$archive_file" ]; then
    echo "error: cannot specify both --artifact and --archive" >&2
    usage >&2
    exit 2
fi

case "$repeat" in
    ''|*[!0-9]*)
        echo "error: --repeat must be a positive integer" >&2
        exit 2
        ;;
esac
if [ "$repeat" -lt 1 ]; then
    echo "error: --repeat must be at least 1" >&2
    exit 2
fi

repository_root="$(cd "$(dirname "$0")/.." && pwd -P)"
daemon_python="${PYO3_PYTHON:-$repository_root/daemon/.venv/bin/python}"
if [ ! -x "$daemon_python" ]; then
    echo "error: missing python verifier interpreter: $daemon_python" >&2
    exit 2
fi

staging_parent=""
cleanup_temp() {
    if [ -n "$staging_parent" ] && [ -d "$staging_parent" ]; then
        rm -rf "$staging_parent"
    fi
}
trap cleanup_temp EXIT INT TERM

# If --artifact was supplied, validate it first then create a temporary archive
if [ -n "$artifact_root" ]; then
    if [ ! -d "$artifact_root" ]; then
        echo "error: artifact directory does not exist: $artifact_root" >&2
        exit 1
    fi
    echo "Validating stage artifact at $artifact_root..."
    "$daemon_python" "$repository_root/scripts/release/product_manifest.py" \
        --artifact "$artifact_root" \
        --target "$target_triple"

    staging_parent="$(mktemp -d /tmp/bp-arch.XXXXXX)"
    archive_file="$staging_parent/brain-product.tar.gz"
    echo "Creating archive $archive_file..."
    # Top-level directory inside archive is brain-dist
    mkdir -p "$staging_parent/brain-dist"
    cp -R "$artifact_root/"* "$staging_parent/brain-dist/"
    tar -czf "$archive_file" -C "$staging_parent" brain-dist
fi

if [ ! -f "$archive_file" ]; then
    echo "error: archive file does not exist: $archive_file" >&2
    exit 1
fi

echo "Starting clean-extraction product smoke on $archive_file (repeat=$repeat)..."

iteration=1
while [ "$iteration" -le "$repeat" ]; do
    echo "--- Extraction iteration $iteration/$repeat ---"
    extract_parent="$(mktemp -d /tmp/bp-ext.XXXXXX)"
    runtime_parent="$(mktemp -d /tmp/bp-run.XXXXXX)"
    unrelated_cwd="$(mktemp -d /tmp/bp-cwd.XXXXXX)"

    cleanup_iteration() {
        rm -rf "$extract_parent" "$runtime_parent" "$unrelated_cwd"
    }

    # Extract tarball safely
    tar -xzf "$archive_file" -C "$extract_parent"

    if [ -d "$extract_parent/brain-dist" ]; then
        product_dir="$extract_parent/brain-dist"
    else
        product_dir="$extract_parent"
    fi

    # 1. Validate product manifest and closed inventory
    "$daemon_python" "$repository_root/scripts/release/product_manifest.py" \
        --artifact "$product_dir" \
        --target "$target_triple"

    # 2. Validate host paths
    "$daemon_python" "$repository_root/scripts/release/host_paths.py" \
        --artifact "$product_dir" \
        --checkout "$repository_root" \
        --target "$target_triple"

    # 3. Validate permissions
    for exe in "bin/brain" "bin/brain-daemon" "frontend/brain-frontend/brain-frontend"; do
        if [ ! -x "$product_dir/$exe" ]; then
            echo "error: extracted binary not executable: $exe" >&2
            cleanup_iteration
            exit 1
        fi
    done

    # 4. Run interactive PTY smoke test without Bun on PATH
    "$daemon_python" "$repository_root/scripts/release/product_smoke.py" \
        --extracted "$product_dir" \
        --runtime-root "$runtime_parent" \
        --cwd "$unrelated_cwd"

    cleanup_iteration
    echo "Clean extraction iteration $iteration/$repeat PASSED."
    iteration=$((iteration + 1))
done

echo "All $repeat product verification iterations PASSED successfully!"
