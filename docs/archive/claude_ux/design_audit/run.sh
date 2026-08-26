#!/usr/bin/env bash
# Forensic Claude UX Reverse-Engineering Engine Entrypoint
# Usage: ./qa/claude_ux/design_audit/run.sh
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../../.." && pwd)"

cd "$PROJECT_ROOT"

echo "=== Forensic Claude UX Reverse-Engineering Engine ==="
echo "Date: $(date)"
echo ""

# Hard Guard: Record pre-run status of Brain crates
STATUS_BEFORE=$(git status --porcelain -- crates/brain-tui crates/brain-domain crates/brain-services 2>/dev/null || true)

echo "[Guard Check] Verifying zero side-effects to Brain crates before execution..."

# Step 1: Execute Path-Replay Screen Discovery & All Exploratory Engine Modules
echo "[Design Audit] Executing forensic discovery, command census, effort/color explorers, resume tester & workspace matrix..."
PYTHONPATH="$PROJECT_ROOT" python3 "$SCRIPT_DIR/runner.py"
echo ""

# Step 2: Generate Product Design Atlas Specification Document
echo "[Report Generator] Compiling CLAUDE_UX_DESIGN_ATLAS.md report..."
PYTHONPATH="$PROJECT_ROOT" python3 "$SCRIPT_DIR/report.py"
echo ""

# Hard Guard: Verify zero NEW modification to Brain source post-run
STATUS_AFTER=$(git status --porcelain -- crates/brain-tui crates/brain-domain crates/brain-services 2>/dev/null || true)
if [ "$STATUS_BEFORE" != "$STATUS_AFTER" ]; then
    echo "ERROR: Design audit modified Brain crates during execution!" >&2
    exit 1
fi
echo "[Guard Check] Zero side-effects to Brain crates confirmed post-run."

echo ""
echo "Forensic Claude UX Reverse-Engineering Engine Execution Complete."
echo "Documentation saved to: $PROJECT_ROOT/docs/research/CLAUDE_UX_DESIGN_ATLAS.md"
