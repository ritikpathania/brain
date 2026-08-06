#!/usr/bin/env python3
"""
Policy-Driven, Weighted, Trend-Aware Retrieval Quality Benchmark (RQB) Runner v2.0.0
"""

import json
import os
import sys
import time
from typing import Dict, Any, List

SCRIPT_DIR = os.path.dirname(os.path.abspath(__file__))
if SCRIPT_DIR not in sys.path:
    sys.path.insert(0, SCRIPT_DIR)

from engine import IsolatedHarness, VectorEvaluation, CoverageScoreEngine
from registry import VectorRegistry

CONFIG_FILE = os.path.join(SCRIPT_DIR, "benchmark_config.json")
DATASET_DIR = os.path.join(SCRIPT_DIR, "datasets")
VECTORS_DIR = os.path.join(SCRIPT_DIR, "vectors")
HISTORY_FILE = os.path.join(SCRIPT_DIR, "history", "history.json")
REPORT_PATH = "/Users/ritikpathania/.gemini/antigravity/brain/f7fa1886-8185-4eb1-b01d-17637485d527/rqb_report.md"

def make_sparkbar(val: float, higher_is_better: bool = True, width: int = 10) -> str:
    display_val = val if higher_is_better else max(0.0, 1.0 - val)
    filled = int(round(display_val * width))
    return "█" * filled + "░" * (width - filled)

def main():
    print("⚡ Launching Config-Driven Policy Benchmark Engine v2.0.0...")

    # Load policy config
    with open(CONFIG_FILE, "r") as f:
        config = json.load(f)

    config_map = {v["vector_id"]: v for v in config.get("vectors", [])}

    # Load evaluators
    VectorRegistry.load_vector_modules(VECTORS_DIR)
    evaluators = VectorRegistry.get_evaluators()

    for ev in evaluators:
        if ev.vector_id in config_map:
            ev.configure(config_map[ev.vector_id])

    harness = IsolatedHarness()
    harness.start()

    evaluations: List[VectorEvaluation] = []
    try:
        for v in evaluators:
            print(f"  • Running Vector {v.vector_id}: {v.name} (weight={v.weight})...")
            result = v.evaluate(harness, DATASET_DIR)
            evaluations.append(result)

        coverage_score, coverage_level, corpus_breakdown = CoverageScoreEngine.calculate_score(harness, DATASET_DIR)
    finally:
        harness.stop()

    # Load multi-version history
    history: Dict[str, Any] = {}
    if os.path.exists(HISTORY_FILE):
        with open(HISTORY_FILE, "r") as f:
            history = json.load(f)

    prev_version = "v1.0.1"
    prev_metrics = history.get(prev_version, {}).get("metrics", {})

    # Compute Weighted RRR
    total_weight = sum(e.weight for e in evaluations)
    regressed_weight = 0

    for e in evaluations:
        prev_val = prev_metrics.get(e.name)
        if prev_val is not None:
            if e.higher_is_better and e.metric_value < prev_val:
                regressed_weight += e.weight
            elif not e.higher_is_better and e.metric_value > prev_val:
                regressed_weight += e.weight

    weighted_rrr = (regressed_weight / total_weight) if total_weight > 0 else 0.0

    # Persist current run to history
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

    has_engine_fail = any(e.status_badge == "🔴 ENGINE FAIL" for e in evaluations)
    has_quality_fail = any(e.status_badge == "🟡 QUALITY BELOW TARGET" for e in evaluations)

    if has_engine_fail:
        overall_status = "🔴 ENGINE FAIL (Execution Error in Vector Engine)"
    elif has_quality_fail:
        overall_status = "🟡 QUALITY BELOW TARGET (Threshold Unmet)"
    else:
        overall_status = "🟢 PASS (Release Approved)"

    report = []
    report.append("# Retrieval Quality Benchmark (RQB) Report v2.0.0 — Brain v1.1.0\n")
    report.append(f"**Execution Environment**: Ephemeral Isolated Harness (`tempfile` UDS + SQLite + Dynamic Port)\n")
    report.append(f"**Benchmark Date**: {time.strftime('%Y-%m-%d %H:%M:%S UTC', time.gmtime())}\n")
    report.append(f"**Engine Build**: `brain-daemon v1.1.0`\n")
    report.append(f"**Policy Version**: `benchmark_config.json v2.0.0`\n\n")

    report.append("## Executive Summary\n")
    report.append(f"- **Functional Retrieval Gate**: `{func_passed}/{len(functional)}` vectors passed ({func_pass_rate:.0f}% Pass Rate)\n")
    report.append(f"- **Quality Metrics Gate**: `{qual_passed}/{len(quality)}` quality targets met ({qual_pass_rate:.0f}% Target Met)\n")
    report.append(f"- **Weighted Retrieval Regression Rate (Weighted RRR)**: `{weighted_rrr:.1%}` ({regressed_weight}/{total_weight} weighted score regressed from {prev_version})\n")
    report.append(f"- **Algorithmic Coverage Score**: `{coverage_score:.1f}/100` (`{coverage_level}`)\n")
    report.append(f"- **Overall RQB Status**: `{overall_status}`\n\n")

    report.append("--- \n\n")
    report.append("## 1. Functional Correctness Vectors (Deterministic Pass/Fail)\n\n")
    report.append("| # | Vector Name | Weight | Badge Status | Metric Name | Threshold | Value | Details |\n")
    report.append("|---|---|:---:|:---:|---|:---:|:---:|---|\n")

    for e in functional:
        val_str = f"{e.metric_value:.2f}" if isinstance(e.metric_value, float) else str(e.metric_value)
        thresh_str = f"{e.threshold:.2f}" if isinstance(e.threshold, float) else str(e.threshold)
        report.append(f"| {e.vector_id} | **{e.name}** | `w={e.weight}` | {e.status_badge} | {e.metric_name} | `{thresh_str}` | `{val_str}` | {e.details} |\n")

    report.append("\n---\n\n")
    report.append("## 2. Retrieval Quality Vectors (Quantitative Heuristic Metrics)\n\n")
    report.append("| # | Vector Name | Weight | Badge Status | Score % | Metric Name | Threshold | Value | Details |\n")
    report.append("|---|---|:---:|:---:|:---:|---|:---:|:---:|---| \n")

    for e in quality:
        val_str = f"{e.metric_value:.2f}" if isinstance(e.metric_value, float) else str(e.metric_value)
        thresh_str = f"{e.threshold:.2f}" if isinstance(e.threshold, float) else str(e.threshold)
        report.append(f"| {e.vector_id} | **{e.name}** | `w={e.weight}` | {e.status_badge} | `{e.score*100:.1f}%` | {e.metric_name} | `{thresh_str}` | `{val_str}` | {e.details} |\n")

    report.append("\n---\n\n")
    report.append("## 3. Persistent Release Time-Series & Weighted RRR (v1.0.0 → v1.0.1 → v1.1.0)\n\n")
    report.append("| Vector Name | Weight | Direction | v1.0.0 | v1.0.1 | v1.1.0 | Sparkbar | Delta | RRR Status |\n")
    report.append("|---|:---:|:---:|:---:|:---:|:---:|---|:---:|:---:|\n")

    v100_metrics = history.get("v1.0.0", {}).get("metrics", {})

    for e in evaluations:
        p0 = v100_metrics.get(e.name, 0.0)
        p1 = prev_metrics.get(e.name, 0.0)
        c_val = e.metric_value
        delta = c_val - p1
        delta_str = f"+{delta:.2f}" if delta >= 0 else f"{delta:.2f}"
        bar = make_sparkbar(c_val, higher_is_better=e.higher_is_better)
        dir_str = "Maximize ↑" if e.higher_is_better else "Minimize ↓"

        if e.higher_is_better:
            t_status = "🟢 IMPROVED" if delta > 0 else ("⚪ STABLE" if delta == 0 else "🔴 REGRESSED")
        else:
            t_status = "🟢 IMPROVED" if delta < 0 else ("⚪ STABLE" if delta == 0 else "🔴 REGRESSED")

        report.append(f"| **{e.name}** | `w={e.weight}` | `{dir_str}` | `{p0:.2f}` | `{p1:.2f}` | `{c_val:.2f}` | `{bar}` | `{delta_str}` | {t_status} |\n")

    report.append("\n---\n\n")
    report.append("## 4. Algorithmic Dataset Coverage Index & Corpus Size\n\n")
    report.append(f"- **Dataset Coverage Score**: `{corpus_breakdown['coverage_score']:.1f} / 100` ({coverage_level})\n")
    report.append(f"- **Active Benchmark Scenarios**: `{corpus_breakdown['scenarios_count']}` datasets loaded\n")
    report.append(f"- **Queries Run**: `{corpus_breakdown['queries_run']}` queries executed over UDS IPC\n")
    report.append(f"- **Ingested Documents**: `{corpus_breakdown['docs_ingested']}` test records\n")
    report.append(f"- **Estimated Token Count**: `{corpus_breakdown['tokens_est']}` ingested tokens\n\n")

    report.append("--- \n\n")
    report.append("## 3-Gate Quality System Architecture (EBRA + RQB + OPB)\n\n")
    report.append("```\n")
    report.append("                          Brain Quality System\n")
    report.append("                                    │\n")
    report.append("        ┌───────────────────────────┼───────────────────────────┐\n")
    report.append("        ▼                           ▼                           ▼\n")
    report.append("   EBRA Gate                   RQB Gate                    OPB Gate\n")
    report.append(" (Release Engineering)       (Retrieval Quality)          (Operational Performance)\n")
    report.append("  • cargo xtask verify         • Config-Driven Policy       • P50/P95/P99 Latency\n")
    report.append("  • 1225 Unit/Integration      • Algorithmic Coverage Score • Soak Memory Growth\n")
    report.append("  • Clippy & Rustfmt           • Weighted RRR Metric        • 10k/100k Node Scale\n")
    report.append("  • Protocol Monotonicity      • Failure Badges (3 Tiers)   • Query Throughput\n")
    report.append("```\n")

    with open(REPORT_PATH, "w", encoding="utf-8") as f:
        f.write("".join(report))

    print(f"\n✅ RQB v2.0.0 execution complete. Report written to {REPORT_PATH}")

if __name__ == "__main__":
    main()
