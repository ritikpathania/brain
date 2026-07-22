#!/usr/bin/env bash
# scripts/retrieval-check.sh
#
# CI regression gate for retrieval quality and performance.
#
# Usage:
#   ./scripts/retrieval-check.sh                              # Run against latest baseline
#   ./scripts/retrieval-check.sh --baseline path/to/v0.7.0.json
#
# Exit codes:
#   0  No regressions detected.
#   1  One or more quality metric regressions detected.
#   2  Configuration or binary error.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"

DEFAULT_BASELINE="${REPO_ROOT}/docs/reference/retrieval-baselines/v0.7.0.json"
BASELINE="${DEFAULT_BASELINE}"
EXTRA_ARGS=()

while [[ $# -gt 0 ]]; do
    case "$1" in
        --baseline)
            BASELINE="$2"
            shift 2
            ;;
        --graph-depth)
            EXTRA_ARGS+=("--graph-depth" "$2")
            shift 2
            ;;
        *)
            echo "Unknown argument: $1"
            exit 2
            ;;
    esac
done

if [[ ! -f "${BASELINE}" ]]; then
    echo "[ERROR] Baseline file not found: ${BASELINE}"
    echo "        Run: cargo run --bin eval_cli -- --write ${BASELINE}"
    exit 2
fi

echo "=== Brain Retrieval Quality Gate ==="
echo "Baseline: ${BASELINE}"
echo ""

# Build the eval CLI in release mode for representative latency numbers
echo "Building eval_cli (release)..."
cargo build --release --bin eval_cli -q

EVAL_BIN="${REPO_ROOT}/target/release/eval_cli"

if [[ ! -x "${EVAL_BIN}" ]]; then
    echo "[ERROR] eval_cli binary not found at ${EVAL_BIN}"
    exit 2
fi

echo "Running evaluation against baseline..."
echo ""

# Run comparison – eval_cli exits non-zero if regressions are detected
if "${EVAL_BIN}" --baseline "${BASELINE}" "${EXTRA_ARGS[@]}"; then
    echo ""
    echo "[PASS] Retrieval quality gate passed."
    exit 0
else
    echo ""
    echo "[FAIL] Retrieval quality gate failed: regressions detected."
    exit 1
fi
