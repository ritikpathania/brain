#!/usr/bin/env python3
"""
Dynamic, Data-Driven, Trend-Aware Retrieval Quality Benchmark (RQB) Runner v1.2.0
"""

import json
import os
import sys
import time
from typing import Dict, Any, List

SCRIPT_DIR = os.path.dirname(os.path.abspath(__file__))
if SCRIPT_DIR not in sys.path:
    sys.path.insert(0, SCRIPT_DIR)

from engine import IsolatedHarness, VectorEvaluation
from registry import VectorRegistry

DATASET_DIR = os.path.join(SCRIPT_DIR, "datasets")
VECTORS_DIR = os.path.join(SCRIPT_DIR, "vectors")
HISTORY_FILE = os.path.join(SCRIPT_DIR, "history", "history.json")
REPORT_PATH = "/Users/ritikpathania/.gemini/antigravity/brain/f7fa1886-8185-4eb1-b01d-17637485d527/rqb_report.md"

def make_sparkbar(val: float, higher_is_better: bool = True, width: int = 10) -> str:
    display_val = val if higher_is_better else max(0.0, 1.0 - val)
    filled = int(round(display_val * width))
    return "█" * filled + "░" * (width - filled)

def main():
    print("⚡ Launching Ephemeral Isolated Dynamic RQB Runner v1.2.0...")

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

    # Load persistent history
    history: Dict[str, Any] = {}
    if os.path.exists(HISTORY_FILE):
        with open(HISTORY_FILE, "r") as f:
            history = json.load(f)

    prev_version = "v1.0.0"
    prev_metrics = history.get(prev_version, {}).get("metrics", {})

    # Compute direction-aware Retrieval Regression Rate (RRR)
    regressed_count = 0
    total_vectors = len(evaluations)

    for e in evaluations:
        prev_val = prev_metrics.get(e.name)
        if prev_val is not None:
            if e.higher_is_better and e.metric_value < prev_val:
                regressed_count += 1
            elif not e.higher_is_better and e.metric_value > prev_val:
                regressed_count += 1

    rrr_rate = (regressed_count / total_vectors) if total_vectors > 0 else 0.0

    # Save current run to history
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
    func_pass_rate = (func_passed / len(functional)) * 100.0 if functional else 0.0
    qual_pass_rate = (qual_passed / len(quality)) * 100.0 if quality else 0.0

    overall_status = "🟢 PASS (Release Approved)" if func_passed == len(functional) and qual_passed == len(quality) else "🟡 ATTENTION NEEDED (Blocked on Functional Gate)"

    report = []
    report.append("# Retrieval Quality Benchmark (RQB) Report — Brain v1.1.0\n")
    report.append(f"**Execution Environment**: Fully Isolated Harness (`tempfile` UDS + SQLite + Dynamic Port)\n")
    report.append(f"**Benchmark Date**: {time.strftime('%Y-%m-%d %H:%M:%S UTC', time.gmtime())}\n")
    report.append(f"**Engine Build**: `brain-daemon v1.1.0`\n\n")

    report.append("## Executive Summary\n")
    report.append(f"- **Functional Retrieval Gate**: `{func_passed}/{len(functional)}` vectors passed ({func_pass_rate:.0f}% Pass Rate)\n")
    report.append(f"- **Quality Metrics Gate**: `{qual_passed}/{len(quality)}` quality targets met ({qual_pass_rate:.0f}% Target Met)\n")
    report.append(f"- **Retrieval Regression Rate (RRR)**: `{rrr_rate:.1%}` ({regressed_count}/{total_vectors} vectors regressed from {prev_version})\n")
    report.append(f"- **Overall RQB Status**: `{overall_status}`\n\n")

    report.append("--- \n\n")
    report.append("## 1. Functional Correctness Vectors (Deterministic Binary Pass/Fail)\n\n")
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
    report.append("## 3. Direction-Aware Trend Analysis & RRR (v1.0.0 → v1.1.0)\n\n")
    report.append("| Vector Name | Direction | v1.0.0 | v1.1.0 | Trend Bar | Delta | RRR Status |\n")
    report.append("|---|:---:|:---:|:---:|---|:---:|:---:|\n")

    for e in evaluations:
        p_val = prev_metrics.get(e.name, 0.0)
        c_val = e.metric_value
        delta = c_val - p_val
        delta_str = f"+{delta:.2f}" if delta >= 0 else f"{delta:.2f}"
        bar = make_sparkbar(c_val, higher_is_better=e.higher_is_better)
        direction_str = "Maximize ↑" if e.higher_is_better else "Minimize ↓"

        if e.higher_is_better:
            t_status = "🟢 IMPROVED" if delta > 0 else ("⚪ STABLE" if delta == 0 else "🔴 REGRESSED")
        else:
            t_status = "🟢 IMPROVED" if delta < 0 else ("⚪ STABLE" if delta == 0 else "🔴 REGRESSED")

        report.append(f"| **{e.name}** | `{direction_str}` | `{p_val:.2f}` | `{c_val:.2f}` | `{bar}` | `{delta_str}` | {t_status} |\n")

    report.append("\n---\n\n")
    report.append("## 4. Benchmark Confidence & Dataset Coverage Index\n\n")
    report.append("| Evaluation Dimension | Dataset Fixture | Active Test Cases | Coverage Confidence Level |\n")
    report.append("|---|---|:---:|:---:|\n")
    report.append("| **Canonical Aliases** | `datasets/aliases.json` | 3 Entities (7 Variant Queries) | Moderate (Baseline) |\n")
    report.append("| **Conflicting Knowledge** | `datasets/conflicts.json` | 2 Conflict Sets (4 Facts) | Moderate (Baseline) |\n")
    report.append("| **Temporal Evolution** | `datasets/temporal.json` | 1 Sequence Pair (2 Facts) | Initial (Baseline) |\n")
    report.append("| **Ranking Stability** | `datasets/stability.json` | 2 Queries (40 Total Runs) | High (40 Runs) |\n")
    report.append("| **Overall Benchmark Confidence** | `Combined Datasets` | **10 Active Benchmark Scenarios** | **Moderate Confidence** |\n")

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
    report.append("  • Clippy & Rustfmt           • Direction-Aware RRR        • 10k/100k Node Scale\n")
    report.append("  • Protocol Monotonicity      • Dataset Coverage Index     • Query Throughput\n")
    report.append("```\n")

    with open(REPORT_PATH, "w", encoding="utf-8") as f:
        f.write("".join(report))

    print(f"\n✅ RQB execution complete. Report written to {REPORT_PATH}")

if __name__ == "__main__":
    main()
