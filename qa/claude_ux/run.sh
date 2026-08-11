#!/usr/bin/env bash
# Empirical Claude Code macOS TUI QA Harness & State-Graph Specification Engine
# Usage: ./qa/claude_ux/run.sh
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"

cd "$PROJECT_ROOT"

echo "=== Empirical Claude Code macOS TUI QA Harness ==="
echo "Date: $(date)"
echo ""

# Hard Guard: Record pre-run status of Brain crates
STATUS_BEFORE=$(git status --porcelain -- crates/brain-tui crates/brain-domain crates/brain-services 2>/dev/null || true)

echo "[Guard Check] Verifying zero side-effects to Brain crates before execution..."

# Step 1: Run Harness Regression Unit Tests (Tests A through R)
echo "[Test Suite] Running harness regression unit tests (Tests A through R)..."
if ! PYTHONPATH="$PROJECT_ROOT" python3 "$SCRIPT_DIR/tests/test_harness.py"; then
    echo "ERROR: Harness unit tests failed! Execution aborted." >&2
    exit 1
fi
echo "[Test Suite] All 18 harness unit tests PASSED."
echo ""

# Step 2: Run Phase 13 Smoke Test Suite (Negative & Positive Test Verification)
echo "[Smoke Test] Running positive and negative smoke test verification..."
if ! PYTHONPATH="$PROJECT_ROOT" python3 "$SCRIPT_DIR/smoke_test.py"; then
    echo "ERROR: Harness smoke test failed! Full execution aborted." >&2
    exit 1
fi
echo "[Smoke Test] Smoke test suite PASSED."
echo ""

# Step 3: Source Keyboard Audit
echo "[Source Audit] Scanning 1,800+ TypeScript source files for keyboard handlers..."
PYTHONPATH="$PROJECT_ROOT" python3 "$SCRIPT_DIR/audit/source_keyboard_auditor.py"
echo ""

# Step 4: Runtime Keyboard & State-Graph Discovery
echo "[State-Graph] Exploring TUI state transitions (state x key -> next_state)..."
PYTHONPATH="$PROJECT_ROOT" python3 "$SCRIPT_DIR/discovery/graph_explorer.py"
echo ""

# Step 5: Runtime Keyboard Discovery Engine
echo "[Keyboard Discovery] Exercising safe keybindings in real Terminal.app TUI..."
PYTHONPATH="$PROJECT_ROOT" python3 "$SCRIPT_DIR/audit/runtime_keyboard_tester.py"
echo ""

# Step 6: Feature Matrix Compiler
echo "[Feature Matrix] Compiling feature matrix JSON and Markdown docs..."
PYTHONPATH="$PROJECT_ROOT" python3 "$SCRIPT_DIR/audit/feature_matrix_auditor.py"
echo ""

# Step 7: Decoupled 2-Layer Matrix Runner with 4-Way Metric Equality Enforcement
echo "[Matrix Run] Executing 100 isolated Claude sessions..."
PYTHONPATH="$PROJECT_ROOT" python3 -c "
import sys
import json
from pathlib import Path
from datetime import datetime

from qa.claude_ux.scenarios.runner import ScenarioRunner
from qa.claude_ux.extraction.geometry import GeometryExtractor
from qa.claude_ux.extraction.visual import VisualExtractor
from qa.claude_ux.extraction.contact_sheet import ContactSheetGenerator

timestamp = datetime.now().strftime('%Y%m%d_%H%M%S')
run_dir = Path('$PROJECT_ROOT/qa/claude_ux/runs') / timestamp
run_dir.mkdir(parents=True, exist_ok=True)

runner = ScenarioRunner(run_dir)
manifest = runner.execute_matrix()

geom_ext = GeometryExtractor(run_dir)
geom_res = geom_ext.process_run(manifest)

vis_ext = VisualExtractor(run_dir)
vis_ext.extract_visual_spec()

cs_gen = ContactSheetGenerator(run_dir)
html_path = cs_gen.generate_html_report(manifest)
md_path = cs_gen.generate_markdown(manifest)

# Write machine-readable baseline.json
baseline_path = Path('$PROJECT_ROOT/qa/claude_ux/baseline.json')
with open(baseline_path, 'w') as f:
    json.dump({
        'claude_version': '2.1.226',
        'timestamp': timestamp,
        'summary': manifest.get('summary'),
        'geometry_checks': geom_res,
        'run_dir': str(run_dir.relative_to(Path('$PROJECT_ROOT')))
    }, f, indent=2)

print(f'\nHarness execution complete. Run artifacts saved to: {run_dir}')
print(f'HTML Report: {html_path}')
"

# Hard Guard: Verify zero NEW modification to Brain source post-run
STATUS_AFTER=$(git status --porcelain -- crates/brain-tui crates/brain-domain crates/brain-services 2>/dev/null || true)
if [ "$STATUS_BEFORE" != "$STATUS_AFTER" ]; then
    echo "ERROR: Harness modified Brain crates during execution!" >&2
    exit 1
fi
echo "[Guard Check] Zero side-effects to Brain crates confirmed post-run."

echo ""
echo "Empirical Claude UX Audit Result Summary"
echo "──────────────────────────────────────────────"
python3 -c "
import json
with open('$PROJECT_ROOT/qa/claude_ux/baseline.json') as f:
    data = json.load(f)
s = data.get('summary', {})
g = data.get('geometry_checks', {})
print(f\"Claude version:           {data.get('claude_version')}\")
print(f\"Total Executed Sessions:  {s.get('pass', 0) + s.get('fail', 0) + s.get('invalid', 0)}\")
print(f\"Successful Sessions:      {s.get('pass', 0)}\")
print(f\"Failed Sessions:          {s.get('fail', 0)}\")
print(f\"Invalid Sessions:         {s.get('invalid', 0)}\")
print(f\"4-Way Metric Invariant:  {s.get('metric_equality_status', 'UNKNOWN')}\")
print(f\"  - Capture Decisions:    {s.get('capture_decision_count', 0)}\")
print(f\"  - Successful Captures:  {s.get('successful_capture_count', 0)}\")
print(f\"  - Filesystem PNGs:      {s.get('filesystem_png_count', 0)}\")
print(f\"  - OCR Runs Executed:    {s.get('ocr_run_count', 0)}\")
print(f\"Exact Geometry Checks:    {g.get('status', 'NOT_AVAILABLE')} ({g.get('exact_measured_count', 0)} PASS, {g.get('unavailable_count', 0)} UNAVAILABLE)\")
"
echo "──────────────────────────────────────────────"
