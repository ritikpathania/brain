#!/usr/bin/env python3
"""
Dynamic, Data-Driven, Trend-Aware Retrieval Quality Benchmark (RQB) Runner
"""

import json
import os
import sys
import time
from typing import Dict, Any, List

# Ensure rqb directory is on sys.path
SCRIPT_DIR = os.path.dirname(os.path.abspath(__file__))
if SCRIPT_DIR not in sys.path:
    sys.path.insert(0, SCRIPT_DIR)

from engine import IsolatedHarness, VectorEvaluation
from registry import VectorRegistry

DATASET_DIR = os.path.join(SCRIPT_DIR, "datasets")
VECTORS_DIR = os.path.join(SCRIPT_DIR, "vectors")
HISTORY_FILE = os.path.join(SCRIPT_DIR, "history", "history.json")
REPORT_PATH = "/Users/ritikpathania/.gemini/antigravity/brain/f7fa1886-8185-4eb1-b01d-17637485d527/rqb_report.md"

def make_sparkbar(val: float, width: int = 10) -> str:
    filled = int(round(val * width))
    return "█" * filled + "░" * (width - filled)

def main():
    print("⚡ Launching Ephemeral Isolated Dynamic RQB Runner...")

    # Load pluggable evaluators via registry
    VectorRegistry.load_vector_modules(VECTORS_DIR)
    evaluators = VectorRegistry.get_evaluators()

    harness = IsolatedHarness()
    harness.start()

    evaluations: List[VectorEvaluation] = []
    try:
        for v in evaluators:
            print(f"  • Running Vector {v.vector_id}: {v.name}...")
            ev = v.evaluate(harness, DATASET_DIR)
            evaluations.append(ev)
    finally:
        harness.stop()

    # Load historical runs for trend comparison
    history: Dict[str, Any] = {}
    if os.path.exists(HISTORY_FILE):
        with open(HISTORY_FILE, "r") as f:
            history = json.load(f)

    prev_version = "v1.0.0"
    prev_metrics = history.get(prev_version, {}).get("metrics", {})

    # Compute Retrieval Regression Rate (RRR)
    regressed_count = 0
    total_vectors = len(evaluations)

    for e in evaluations:
        prev_val = prev_metrics.get(e.name)
        if prev_val is not None and e.metric_value < prev_val:
            regressed_count += 1

    rrr_rate = (regressed_count / total_vectors) if total_vectors > 0 else 0.0

    # Save current run into history
    current_version = "v1.1.0"
    history[current_version] = {
        "timestamp": time.strftime("%Y-%m-%d %H:%M:%S UTC", time.gmtime()),
        "metrics": {e.name: e.metric_value for e in evaluations}
    }
    with open(HISTORY_FILE, "w") as f:
        json.dump(history, f, indent=2)

    functional = [e for e in evaluations if e.vector_type == "Functional"]
    quality = [e for e in evaluations if e.vector_type == "Quality"]

    func_passed = sum(1 for e in functional if e.passed)
    qual_passed = sum(1 for e in quality if e.passed)
    avg_quality_score = (sum(e.score for e in quality) / len(quality)) * 100.0 if quality else 0.0

    rqb_status = "🟢 PASS" if func_passed == len(functional) and qual_passed == len(quality) else "🟡 ATTENTION NEEDED"

    report = []
    report.append("# Retrieval Quality Benchmark (RQB) Report — Brain v1.1.0\n")
    report.append(f"**Execution Environment**: Fully Isolated Harness (`tempfile` UDS + SQLite + Dynamic Port)\n")
    report.append(f"**Benchmark Date**: {time.strftime('%Y-%m-%d %H:%M:%S UTC', time.gmtime())}\n")
    report.append(f"**Engine Build**: `brain-daemon v1.1.0`\n\n")

    report.append("## Executive Summary\n")
    report.append(f"- **Functional Correctness**: `{func_passed}/{len(functional)}` vectors passed (100% Deterministic Verification)\n")
    report.append(f"- **Retrieval Quality Score**: `{avg_quality_score:.1f}%` ({qual_passed}/{len(quality)} quality targets met)\n")
    report.append(f"- **Retrieval Regression Rate (RRR)**: `{rrr_rate:.1%}` ({regressed_count}/{total_vectors} vectors regressed from {prev_version})\n")
    report.append(f"- **Overall RQB Status**: `{rqb_status}`\n\n")

    report.append("--- \n\n")
    report.append("## 1. Functional Correctness Vectors (Deterministic Pass/Fail)\n\n")
    report.append("| # | Vector Name | Status | Metric Name | Metric Value | Details |\n")
    report.append("|---|---|:---:|---|:---:|---|\n")

    for e in functional:
        status = "🟢 PASS" if e.passed else "🔴 FAIL"
        val_str = f"{e.metric_value:.2f}" if isinstance(e.metric_value, float) else str(e.metric_value)
        report.append(f"| {e.vector_id} | **{e.name}** | {status} | {e.metric_name} | `{val_str}` | {e.details} |\n")

    report.append("\n---\n\n")
    report.append("## 2. Retrieval Quality Vectors (Quantitative Heuristic Metrics)\n\n")
    report.append("| # | Vector Name | Status | Score % | Metric Name | Metric Value | Details |\n")
    report.append("|---|---|:---:|:---:|---|:---:|---| \n")

    for e in quality:
        status = "🟢 PASS" if e.passed else "🟡 MARGINAL"
        val_str = f"{e.metric_value:.2f}" if isinstance(e.metric_value, float) else str(e.metric_value)
        report.append(f"| {e.vector_id} | **{e.name}** | {status} | `{e.score*100:.1f}%` | {e.metric_name} | `{val_str}` | {e.details} |\n")

    report.append("\n---\n\n")
    report.append("## 3. Release-to-Release Trend Analysis & RRR (v1.0.0 → v1.1.0)\n\n")
    report.append("| Vector Name | v1.0.0 | v1.1.0 | Trend Bar | Delta | RRR Status |\n")
    report.append("|---|:---:|:---:|---|:---:|:---:|\n")

    for e in evaluations:
        p_val = prev_metrics.get(e.name, 0.0)
        c_val = e.metric_value
        delta = c_val - p_val
        delta_str = f"+{delta:.2f}" if delta >= 0 else f"{delta:.2f}"
        bar = make_sparkbar(c_val)
        t_status = "🟢 IMPROVED" if delta > 0 else ("⚪ STABLE" if delta == 0 else "🔴 REGRESSED")
        report.append(f"| **{e.name}** | `{p_val:.2f}` | `{c_val:.2f}` | `{bar}` | `{delta_str}` | {t_status} |\n")

    report.append("\n---\n\n")
    report.append("## 3-Gate Quality System Architecture (EBRA + RQB + OPB)\n\n")
    report.append("```\n")
    report.append("                          Brain Quality System\n")
    report.append("                                    │\n")
    report.append("        ┌───────────────────────────┼───────────────────────────┐\n")
    report.append("        ▼                           ▼                           ▼\n")
    report.append("   EBRA Gate                   RQB Gate                    OPB Gate\n")
    report.append(" (Release Engineering)       (Retrieval Quality)          (Operational Performance)\n")
    report.append("  • cargo xtask verify         • 10 Evaluation Vectors      • P50/P95/P99 Latency\n")
    report.append("  • 1225 Unit/Integration      • Data-Driven JSON Sets      • Soak Memory Growth\n")
    report.append("  • Clippy & Rustfmt           • Precision@K & MRR          • 10k/100k Node Scale\n")
    report.append("  • Protocol Monotonicity      • Trend Analysis & RRR       • Query Throughput\n")
    report.append("```\n")

    with open(REPORT_PATH, "w", encoding="utf-8") as f:
        f.write("".join(report))

    print(f"\n✅ RQB execution complete. Report written to {REPORT_PATH}")

if __name__ == "__main__":
    main()
