#!/usr/bin/env bash
# Brain vs Claude UX Audit Harness
# Usage: ./scripts/ux_audit/run.sh
set -euo pipefail

OUT_DIR="docs/research/claude-ux-evidence"
SNAP_DIR="$OUT_DIR"

mkdir -p "$SNAP_DIR"/{startup,prompt,slash,ctrl-k,workspace,streaming,themes,responsive}

echo "=== Brain vs Claude UX Audit Harness ==="
echo "Output: $SNAP_DIR"
echo ""

# Check for Claude
if command -v claude &>/dev/null; then
    CLAUDE_BIN=$(which claude)
    CLAUDE_VERSION=$(claude --version 2>/dev/null || echo 'unknown')
    echo "Claude found: $CLAUDE_BIN ($CLAUDE_VERSION)"
else
    echo "Claude not found in PATH. Skipping Claude tests."
fi

# Capture Brain snapshots (uses cargo test)
echo "Generating Brain snapshots..."
cd "$(dirname "$0")/../.."
cargo test -p brain-tui --test visual_snapshots -- --nocapture 2>&1 | grep -E 'test snapshot|ok|FAILED'

echo "Audit complete. See $SNAP_DIR"
