#!/usr/bin/env python3
"""
Policy-Driven, Severity-Graded, Statistically Grounded RQB Platform Runner v2.2.0
"""

import json
import math
import os
import sys
import time
from typing import Dict, Any, List

SCRIPT_DIR = os.path.dirname(os.path.abspath(__file__))
if SCRIPT_DIR not in sys.path:
    sys.path.insert(0, SCRIPT_DIR)

from engine import IsolatedHarness, VectorEvaluation, CoverageScoreEngine, get_file_sha256, get_git_commit
from registry import VectorRegistry

CONFIG_FILE = os.path.join(SCRIPT_DIR, "benchmark_config.json")
RELEASE_POLICY_FILE = os.path.join(SCRIPT_DIR, "release_policy.json")
DATASET_DIR = os.path.join(SCRIPT_DIR, "datasets")
VECTORS_DIR = os.path.join(SCRIPT_DIR, "vectors")
HISTORY_FILE = os.path.join(SCRIPT_DIR, "history", "history.json")
REPORT_PATH = "/Users/ritikpathania/.gemini/antigravity/brain/f7fa1886-8185-4eb1-b01d-17637485d527/rqb_report.md"

def make_sparkbar(val: float, higher_is_better: bool = True, width: int = 10) -> str:
    display_val = val if higher_is_better else max(0.0, 1.0 - val)
    filled = int(round(display_val * width))
    return "█" * filled + "░" * (width - filled)

def main():
    print("⚡ Launching Policy-Driven Severity-Graded RQB Platform v2.2.0...")

    with open(CONFIG_FILE, "r") as f:
        config = json.load(f)

    with open(RELEASE_POLICY_FILE, "r") as f:
        release_policy = json.load(f)

    config_map = {v["vector_id"]: v for v in config.get("vectors", [])}
    pkg_versions = config.get("dataset_packages", {})
    policy_hash = get_file_sha256(CONFIG_FILE)
    rel_policy_hash = get_file_sha256(RELEASE_POLICY_FILE)
    git_commit = get_git_commit()
    seed = 42

    VectorRegistry.load_vector_modules(VECTORS_DIR)
    evaluators = VectorRegistry.get_evaluators()

    for ev in evaluators:
        if ev.vector_id in config_map:
            ev.configure(config_map[ev.vector_id])

    harness = IsolatedHarness(seed=seed)
    harness.start()

    evaluations: List[VectorEvaluation] = []
    try:
        for v in evaluators:
            print(f"  • Running Vector {v.vector_id}: {v.name} (severity={v.severity}, weight={v.weight})...")
            result = v.evaluate(harness, DATASET_DIR)
            evaluations.append(result)

        coverage_score, coverage_level, corpus_breakdown = CoverageScoreEngine.calculate_published_score(harness, DATASET_DIR)
    finally:
        harness.stop()

    # Latency continuous statistics
    latencies = harness.perf_metrics.query_latencies_ms
    n_lat = len(latencies)
    mean_lat = sum(latencies) / n_lat if n_lat > 0 else 0.0
    sorted_lat = sorted(latencies) if n_lat > 0 else [0.0]

    median_lat = sorted_lat[n_lat // 2] if n_lat > 0 else 0.0
    p95_idx = max(0, int(math.ceil(0.95 * n_lat)) - 1)
    p99_idx = max(0, int(math.ceil(0.99 * n_lat)) - 1)
    p95_lat = sorted_lat[p95_idx]
    p99_lat = sorted_lat[p99_idx]

    variance = sum((x - mean_lat) ** 2 for x in latencies) / n_lat if n_lat > 0 else 0.0
    stddev_lat = math.sqrt(variance)

    # Load history
    history: Dict[str, Any] = {}
    if os.path.exists(HISTORY_FILE):
        with open(HISTORY_FILE, "r") as f:
            history = json.load(f)

    prev_version = "v1.0.1"
    prev_metrics = history.get(prev_version, {}).get("metrics", {})

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

    # Severity Tier Health Calculations
    critical_vecs = [e for e in evaluations if e.severity == "Critical"]
    major_vecs = [e for e in evaluations if e.severity == "Major"]
    minor_vecs = [e for e in evaluations if e.severity == "Minor"]

    crit_passed = sum(1 for e in critical_vecs if e.passed)
    maj_passed = sum(1 for e in major_vecs if e.passed)
    min_passed = sum(1 for e in minor_vecs if e.passed)

    crit_pct = (crit_passed / len(critical_vecs)) * 100.0 if critical_vecs else 100.0
    maj_pct = (maj_passed / len(major_vecs)) * 100.0 if major_vecs else 100.0
    min_pct = (min_passed / len(minor_vecs)) * 100.0 if minor_vecs else 100.0

    # Release Policy Enforcement
    rel_rules = release_policy.get("rules", {})
    block_crit = rel_rules.get("critical_vectors_pass_required", True) and crit_passed < len(critical_vecs)
    block_rrr = weighted_rrr > rel_rules.get("max_allowed_weighted_rrr", 0.05)
    block_err = rel_rules.get("zero_engine_failures_required", True) and any(e.status_badge == "🔴 ENGINE FAIL" for e in evaluations)

    if block_err:
        overall_status = "🔴 ENGINE FAIL (Execution Error in Vector Engine)"
    elif block_crit:
        overall_status = "🟡 RELEASE BLOCKED (Critical Vector Target Unmet)"
    elif block_rrr:
        overall_status = "🟡 RELEASE BLOCKED (Weighted RRR Threshold Exceeded)"
    else:
        overall_status = "🟢 PASS (Release Approved by Release Policy)"

    current_version = "v1.1.0"
    history[current_version] = {
        "timestamp": time.strftime("%Y-%m-%d %H:%M:%S UTC", time.gmtime()),
        "provenance": {
            "git_commit": git_commit,
            "policy_hash": policy_hash,
            "release_policy_hash": rel_policy_hash,
            "seed": seed,
            "dataset_packages": pkg_versions,
            "runner_version": "v2.2.0"
        },
        "metrics": {e.name: e.metric_value for e in evaluations}
    }
    with open(HISTORY_FILE, "w") as f:
        json.dump(history, f, indent=2)

    report = []
    report.append("# Retrieval Quality Benchmark (RQB) Report v2.2.0 — Brain v1.1.0\n")
    report.append(f"**Execution Environment**: Ephemeral Isolated Harness (`tempfile` UDS + SQLite + Dynamic Port)\n")
    report.append(f"**Benchmark Date**: {time.strftime('%Y-%m-%d %H:%M:%S UTC', time.gmtime())}\n")
    report.append(f"**Provenance Metadata**: `Commit: {git_commit}` | `Policy: {policy_hash}` | `Release Policy: {rel_policy_hash}` | `Seed: {seed} (Deterministic)`\n")
    report.append(f"**Dataset Packages**: `aliases {pkg_versions.get('aliases')}` | `conflicts {pkg_versions.get('conflicts')}` | `temporal {pkg_versions.get('temporal')}` | `stability {pkg_versions.get('stability')}`\n")
    report.append(f"**Engine Build**: `brain-daemon v1.1.0`\n\n")

    report.append("## Executive Summary\n")
    report.append(f"- **Critical Severity Tier**: `{crit_passed}/{len(critical_vecs)}` passed (**{crit_pct:.1f}%** Pass Rate) — `Exact Retrieval`, `Synonyms & Aliases`\n")
    report.append(f"- **Major Severity Tier**: `{maj_passed}/{len(major_vecs)}` passed (**{maj_pct:.1f}%** Pass Rate) — Deduplication, Conflicts, Temporal, Synthesis, Context\n")
    report.append(f"- **Minor Severity Tier**: `{min_passed}/{len(minor_vecs)}` passed (**{min_pct:.1f}%** Pass Rate) — Ranking Stability\n")
    report.append(f"- **Weighted Retrieval Regression Rate (Weighted RRR)**: `{weighted_rrr:.1%}` ({regressed_weight}/{total_weight} weighted points regressed)\n")
    report.append(f"- **Algorithmic Coverage Score**: `{coverage_score:.1f}/100` (`{coverage_level}`)\n")
    report.append(f"- **Overall RQB Status**: `{overall_status}`\n\n")

    report.append("--- \n\n")
    report.append("## 1. Functional Correctness Vectors (Deterministic Pass/Fail)\n\n")
    report.append("| # | Vector Name | Severity | Weight | Badge Status | Metric Name | Sample N | 95% Wilson CI | Threshold | Value | Details |\n")
    report.append("|---|---|:---:|:---:|:---:|---|:---:|:---:|:---:|:---:|---|\n")

    functional = [e for e in evaluations if e.vector_type == "Functional"]
    for e in functional:
        val_str = f"{e.metric_value:.2f}" if isinstance(e.metric_value, float) else str(e.metric_value)
        thresh_str = f"{e.threshold:.2f}" if isinstance(e.threshold, float) else str(e.threshold)
        ci_str = f"[{e.ci_lower:.2f} - {e.ci_upper:.2f}]"
        report.append(f"| {e.vector_id} | **{e.name}** | `{e.severity}` | `w={e.weight}` | {e.status_badge} | {e.metric_name} | `N={e.sample_size_n}` | `{ci_str}` | `{thresh_str}` | `{val_str}` | {e.details} |\n")

    report.append("\n---\n\n")
    report.append("## 2. Retrieval Quality Assessment (Quantitative Heuristic Metrics)\n\n")
    report.append("| # | Vector Name | Severity | Weight | Badge Status | Score % | Metric Name | Sample N | 95% Wilson CI | Threshold | Value | Details |\n")
    report.append("|---|---|:---:|:---:|:---:|:---:|---|:---:|:---:|:---:|:---:|---| \n")

    quality = [e for e in evaluations if e.vector_type == "Quality"]
    for e in quality:
        val_str = f"{e.metric_value:.2f}" if isinstance(e.metric_value, float) else str(e.metric_value)
        thresh_str = f"{e.threshold:.2f}" if isinstance(e.threshold, float) else str(e.threshold)
        ci_str = f"[{e.ci_lower:.2f} - {e.ci_upper:.2f}]"
        report.append(f"| {e.vector_id} | **{e.name}** | `{e.severity}` | `w={e.weight}` | {e.status_badge} | `{e.score*100:.1f}%` | {e.metric_name} | `N={e.sample_size_n}` | `{ci_str}` | `{thresh_str}` | `{val_str}` | {e.details} |\n")

    report.append("\n---\n\n")
    report.append("## 3. Persistent Release Time-Series & Provenance (v1.0.0 → v1.0.1 → v1.1.0)\n\n")
    report.append("> ⚠️ *Note: Historical baselines for v1.0.0 and v1.0.1 were initialized from seeded initial baselines. Archived production executions begin with v1.1.0.*\n\n")
    report.append("| Vector Name | Severity | Weight | Direction | v1.0.0 (Seeded) | v1.0.1 (Seeded) | v1.1.0 (Live) | Sparkbar | Delta | RRR Status |\n")
    report.append("|---|:---:|:---:|:---:|:---:|:---:|:---:|---|:---:|:---:|\n")

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

        report.append(f"| **{e.name}** | `{e.severity}` | `w={e.weight}` | `{dir_str}` | `{p0:.2f}` | `{p1:.2f}` | `{c_val:.2f}` | `{bar}` | `{delta_str}` | {t_status} |\n")

    report.append("\n---\n\n")
    report.append("## 4. Continuous Operational Performance Statistics (OPB Precursor)\n\n")
    report.append("| Latency & Performance Dimension | Measurement Value | Sample Size ($N$) | Operational Target |\n")
    report.append("|---|:---:|:---:|:---:|\n")
    report.append(f"| **Mean Query Latency** | `{mean_lat:.2f} ms` | `N={n_lat}` | `< 50.0 ms` |\n")
    report.append(f"| **Median Query Latency (P50)** | `{median_lat:.2f} ms` | `N={n_lat}` | `< 40.0 ms` |\n")
    report.append(f"| **Latency Standard Deviation (StdDev)** | `{stddev_lat:.2f} ms` | `N={n_lat}` | Low Variance |\n")
    report.append(f"| **P95 Query Latency** | `{p95_lat:.2f} ms` | `N={n_lat}` | `< 100.0 ms` |\n")
    report.append(f"| **P99 Query Latency** | `{p99_lat:.2f} ms` | `N={n_lat}` | `< 150.0 ms` |\n")
    report.append(f"| **Dataset Coverage Level** | **{coverage_score:.1f} / 100** | **{corpus_breakdown['scenarios_total']} Datasets** | **{coverage_level}** |\n")

    report.append("\n---\n\n")
    report.append("## 3-Gate Quality System Architecture (EBRA + RQB + OPB)\n\n")
    report.append("```\n")
    report.append("                          Brain Quality System\n")
    report.append("                                    │\n")
    report.append("        ┌───────────────────────────┼───────────────────────────┐\n")
    report.append("        ▼                           ▼                           ▼\n")
    report.append("   EBRA Gate                    RQB Gate                   OPB Gate\n")
    report.append(" (Release Gate)          (Retrieval Quality)       (Operational Performance)\n")
    report.append("  • cargo xtask verify         • Severity Tiers (Crit/Maj)  • Mean, Median, StdDev\n")
    report.append("  • 1225 Unit/Integration      • Independent Datasets       • P95/P99 Latency Bounds\n")
    report.append("  • Clippy & Rustfmt           • Decoupled Release Policy   • Memory & Cost Bounds\n")
    report.append("  • Protocol Monotonicity      • 95% Wilson CI Ranges       • 10k/100k Node Scale\n")
    report.append("```\n")

    with open(REPORT_PATH, "w", encoding="utf-8") as f:
        f.write("".join(report))

    print(f"\n✅ RQB v2.2.0 platform execution complete. Report written to {REPORT_PATH}")

if __name__ == "__main__":
    main()
