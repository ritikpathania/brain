#!/usr/bin/env bash
# First-party frontend verification is a blocking build-host quality gate.
set -euo pipefail

artifact=""
brain_version=""
target=""
while [ "$#" -gt 0 ]; do
    case "$1" in
        --artifact|--brain-version|--target)
            if [ "$#" -lt 2 ] || [ -z "$2" ]; then
                echo "error: $1 requires a value" >&2
                exit 2
            fi
            case "$1" in
                --artifact) artifact="$2" ;;
                --brain-version) brain_version="$2" ;;
                --target) target="$2" ;;
            esac
            shift 2
            ;;
        *) echo "error: unknown verifier argument: $1" >&2; exit 2 ;;
    esac
done
if [ -z "$artifact" ] || [ -z "$brain_version" ] || [ -z "$target" ]; then
    echo "error: --artifact, --brain-version and --target are required" >&2
    exit 2
fi
if [ -z "${PYO3_PYTHON:-}" ] || [ ! -x "$PYO3_PYTHON" ]; then
    echo "error: PYO3_PYTHON must name the explicit CPython 3.15.2 build verifier" >&2
    exit 2
fi
"$PYO3_PYTHON" -c 'import sys; assert sys.version_info[:3] == (3, 15, 2), "verifier requires CPython 3.15.2"'
repository_root="$(cd "$(dirname "$0")/.." && pwd -P)"
case "$(uname -s):$(uname -m)" in
    Darwin:arm64) native_target="aarch64-apple-darwin" ;;
    Linux:x86_64) native_target="x86_64-unknown-linux-gnu" ;;
    *) echo "error: unsupported native verification platform" >&2; exit 2 ;;
esac
if [ "$target" != "$native_target" ]; then
    echo "error: verifier requires its native target" >&2
    exit 2
fi
"$PYO3_PYTHON" "$repository_root/scripts/release/artifact_manifest.py" \
    --artifact "$artifact" --brain-version "$brain_version" --target "$target"
"$PYO3_PYTHON" "$repository_root/scripts/release/host_paths.py" \
    --artifact "$artifact" --checkout "$repository_root" --target "$target"
