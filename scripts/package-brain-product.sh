#!/usr/bin/env bash
# Assemble and verify the complete Brain product distribution layout.
#
# Stable layout:
#   <artifact>/bin/brain
#   <artifact>/bin/brain-daemon
#   <artifact>/frontend/brain-frontend/brain-frontend
#   <artifact>/frontend/brain-frontend/manifest.json
#   <artifact>/runtime/python/...
#   <artifact>/runtime-manifest.json
#   <artifact>/runtime/python-files.json
#   <artifact>/licenses/...
#   <artifact>/product-manifest.json

set -euo pipefail

usage() {
    cat <<'EOF'
Usage: scripts/package-brain-product.sh --cli <path> --daemon <path> --frontend <built-F-directory> --output <new-directory> --target <triple>

Requires PYO3_PYTHON to name the CPython 3.15 build interpreter. The output path must not exist.
EOF
}

cli_binary=""
daemon_binary=""
frontend_dir=""
output_directory=""
target_triple=""

while [ "$#" -gt 0 ]; do
    case "$1" in
        --cli)
            cli_binary="${2:-}"
            shift 2
            ;;
        --daemon)
            daemon_binary="${2:-}"
            shift 2
            ;;
        --frontend)
            frontend_dir="${2:-}"
            shift 2
            ;;
        --output)
            output_directory="${2:-}"
            shift 2
            ;;
        --target)
            target_triple="${2:-}"
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

if [ -z "$cli_binary" ] || [ -z "$daemon_binary" ] || [ -z "$frontend_dir" ] || [ -z "$output_directory" ] || [ -z "$target_triple" ]; then
    echo "error: --cli, --daemon, --frontend, --output, and --target are required" >&2
    usage >&2
    exit 2
fi

if [ -e "$output_directory" ]; then
    echo "error: output directory already exists: $output_directory" >&2
    exit 1
fi

if [ ! -f "$cli_binary" ] || [ ! -x "$cli_binary" ]; then
    echo "error: CLI binary not found or not executable: $cli_binary" >&2
    exit 1
fi

if [ ! -f "$daemon_binary" ] || [ ! -x "$daemon_binary" ]; then
    echo "error: daemon binary not found or not executable: $daemon_binary" >&2
    exit 1
fi

if [ ! -d "$frontend_dir" ] || [ ! -f "$frontend_dir/brain-frontend" ] || [ ! -f "$frontend_dir/manifest.json" ]; then
    echo "error: frontend directory must contain brain-frontend and manifest.json: $frontend_dir" >&2
    exit 1
fi

if [ -z "${PYO3_PYTHON:-}" ] || [ ! -x "$PYO3_PYTHON" ]; then
    echo "error: PYO3_PYTHON must name the explicit CPython 3.15 verifier interpreter" >&2
    exit 2
fi

repository_root="$(cd "$(dirname "$0")/.." && pwd -P)"

# Step 1: Package daemon runtime into output directory
"$repository_root/scripts/package-daemon-runtime.sh" \
    --daemon "$daemon_binary" \
    --output "$output_directory" \
    --target "$target_triple"

# Step 2: Install CLI binary
cp "$cli_binary" "$output_directory/bin/brain"
chmod 0755 "$output_directory/bin/brain"

# Step 3: Install first-party frontend component
mkdir -p "$output_directory/frontend/brain-frontend"
cp "$frontend_dir/brain-frontend" "$output_directory/frontend/brain-frontend/brain-frontend"
cp "$frontend_dir/manifest.json" "$output_directory/frontend/brain-frontend/manifest.json"
chmod 0755 "$output_directory/frontend/brain-frontend/brain-frontend"

# Step 4: Collect licenses
mkdir -p "$output_directory/licenses"
"$PYO3_PYTHON" "$repository_root/scripts/release/licenses.py" \
    --licenses-json "$repository_root/scripts/release/licenses.json" \
    --output "$output_directory/licenses"

# Step 5: Extract source identity and build tools
tmp_dir="$(mktemp -d /tmp/brain-assemble.XXXXXX)"
cleanup() {
    rm -rf "$tmp_dir"
}
trap cleanup EXIT INT TERM

source_json="$tmp_dir/source.json"
tools_json="$tmp_dir/tools.json"

"$PYO3_PYTHON" - <<PY
import json
from pathlib import Path

fe_manifest = json.loads(Path("$frontend_dir/manifest.json").read_text())
source = fe_manifest.get("source", {})
Path("$source_json").write_text(json.dumps(source, indent=2))
PY

rustc_version="$(rustc --version)"
cargo_version="$(cargo --version)"
bun_version="$(bun --version)"
python_version="$("$PYO3_PYTHON" -c 'import sys; print(".".join(map(str, sys.version_info[:3])))')"

cat <<EOF > "$tools_json"
{
  "rustc": "$rustc_version",
  "cargo": "$cargo_version",
  "bun": "$bun_version",
  "python": "$python_version"
}
EOF

# Step 6: Assemble product manifest
"$PYO3_PYTHON" "$repository_root/scripts/release/product_manifest.py" \
    --assemble \
    --artifact "$output_directory" \
    --target "$target_triple" \
    --source "$source_json" \
    --build-tools "$tools_json"

# Step 7: Validate assembled product
"$PYO3_PYTHON" "$repository_root/scripts/release/product_manifest.py" \
    --artifact "$output_directory" \
    --target "$target_triple"

echo "Assembled and verified Brain product distribution at $output_directory"
