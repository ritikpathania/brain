#!/usr/bin/env python3
"""
Retrieval Quality Benchmark (RQB) Engine v2.2.0 — Production Benchmark Platform

Features:
- 95% Wilson Score Confidence Intervals for sample metrics
- Operational Performance Normalization (Mean & P95 Query Latency, RSS Memory)
- Dataset Schema Versioning (v1.2.0)
- Deterministic Random Seed Provenance (Seed: 42)
- Explicit Failure Taxonomy & Published Mathematical Coverage Formula
"""

from abc import ABC, abstractmethod
from dataclasses import dataclass, field
import hashlib
import json
import math
import os
import random
import socket
import subprocess
import sys
import tempfile
import time
from typing import List, Dict, Any, Optional, Tuple

DAEMON_BIN = "/Users/ritikpathania/Developer/PyCharm/brain/target/debug/brain-daemon"
PYTHON_VENV = "/Users/ritikpathania/Developer/PyCharm/brain/daemon/.venv/bin/python"

# -------------------------------------------------------------------
# 1. Statistical Confidence Interval Utilities
# -------------------------------------------------------------------

def calculate_wilson_ci(p_hat: float, n: int, z: float = 1.96) -> Tuple[float, float]:
    """Computes 95% Wilson Score Confidence Interval for binomial proportion."""
    if n <= 0:
        return (0.0, 0.0)
    denom = 1.0 + (z**2 / n)
    centre_adj = p_hat + (z**2 / (2 * n))
    spread = z * math.sqrt(max(0.0, (p_hat * (1.0 - p_hat) / n) + (z**2 / (4 * (n**2)))))
    lower = max(0.0, (centre_adj - spread) / denom)
    upper = min(1.0, (centre_adj + spread) / denom)
    return (lower, upper)

def get_file_sha256(filepath: str) -> str:
    if not os.path.exists(filepath):
        return "missing"
    hasher = hashlib.sha256()
    with open(filepath, "rb") as f:
        for chunk in iter(lambda: f.read(4096), b""):
            hasher.update(chunk)
    return hasher.hexdigest()[:12]

def get_git_commit() -> str:
    try:
        res = subprocess.run(["git", "rev-parse", "HEAD"], capture_output=True, text=True, check=True)
        return res.stdout.strip()[:10]
    except Exception:
        return "unknown"

# -------------------------------------------------------------------
# 2. Ephemeral Execution Harness with Latency & Memory Tracking
# -------------------------------------------------------------------

@dataclass
class PerformanceMetrics:
    total_docs_ingested: int = 0
    total_tokens_est: int = 0
    total_queries_run: int = 0
    query_latencies_ms: List[float] = field(default_factory=list)
    peak_rss_mb: float = 0.0

class IsolatedHarness:
    def __init__(self, seed: int = 42):
        self.seed = seed
        random.seed(seed)
        self.tmp_dir = tempfile.TemporaryDirectory()
        self.socket_path = os.path.join(self.tmp_dir.name, "brain_rqb.sock")
        self.data_dir = os.path.join(self.tmp_dir.name, "data")
        os.makedirs(self.data_dir, exist_ok=True)
        self.http_port = str(random.randint(18000, 29000))
        self.proc: Optional[subprocess.Popen] = None
        self.perf_metrics = PerformanceMetrics()

    def start(self):
        env = os.environ.copy()
        env["PYO3_PYTHON"] = PYTHON_VENV
        env["BRAIN_SOCKET_PATH"] = self.socket_path
        env["BRAIN_DATA_DIR"] = self.data_dir
        env["BRAIN_HEALTH_PORT"] = self.http_port

        self.proc = subprocess.Popen(
            [DAEMON_BIN, "daemon", "run"],
            env=env,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE
        )

        for _ in range(40):
            if os.path.exists(self.socket_path):
                time.sleep(0.3)
                return
            time.sleep(0.1)

        stderr_out = self.proc.stderr.read().decode('utf-8') if self.proc.stderr else ""
        raise RuntimeError(f"Isolated Harness failed to start. Stderr:\n{stderr_out}")

    def request(self, action: str, payload: str) -> List[Dict[str, Any]]:
        t0 = time.perf_counter()
        if action == "ingest":
            self.perf_metrics.total_docs_ingested += 1
            self.perf_metrics.total_tokens_est += len(payload.split())
        elif action == "query":
            self.perf_metrics.total_queries_run += 1

        sock = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        sock.connect(self.socket_path)
        req = json.dumps({"action": action, "payload": payload}) + "\n"
        sock.sendall(req.encode('utf-8'))

        responses = []
        buffer = ""
        sock.settimeout(5.0)
        try:
            while True:
                chunk = sock.recv(4096).decode('utf-8')
                if not chunk:
                    break
                buffer += chunk
                while "\n" in buffer:
                    line, buffer = buffer.split("\n", 1)
                    if line.strip():
                        try:
                            obj = json.loads(line)
                            responses.append(obj)
                            if obj.get("type") == "stream_end":
                                sock.close()
                                dt_ms = (time.perf_counter() - t0) * 1000.0
                                if action == "query":
                                    self.perf_metrics.query_latencies_ms.append(dt_ms)
                                return responses
                        except json.JSONDecodeError:
                            pass
        except socket.timeout:
            pass
        sock.close()
        dt_ms = (time.perf_counter() - t0) * 1000.0
        if action == "query":
            self.perf_metrics.query_latencies_ms.append(dt_ms)
        return responses

    def stop(self):
        if self.proc:
            self.proc.terminate()
            try:
                self.proc.wait(timeout=3)
            except subprocess.TimeoutExpired:
                self.proc.kill()
        self.tmp_dir.cleanup()

# -------------------------------------------------------------------
# 3. Vector Hierarchy & Failure Taxonomy
# -------------------------------------------------------------------

@dataclass
class VectorEvaluation:
    vector_id: int
    name: str
    vector_type: str  # "Functional" or "Quality"
    passed: bool
    status_badge: str  # "🟢 PASS" | "🔴 ENGINE FAIL" | "🟡 QUALITY BELOW TARGET"
    score: float
    metric_name: str
    metric_value: float
    sample_size_n: int
    ci_lower: float
    ci_upper: float
    details: str
    weight: int = 1
    higher_is_better: bool = True
    threshold: float = 0.8

class Vector(ABC):
    def __init__(self, vector_id: int, name: str, higher_is_better: bool = True):
        self.vector_id = vector_id
        self.name = name
        self.higher_is_better = higher_is_better
        self.weight = 1
        self.threshold = 0.8

    def configure(self, cfg: Dict[str, Any]):
        self.weight = cfg.get("weight", 1)
        self.threshold = cfg.get("threshold", 0.8)
        self.higher_is_better = (cfg.get("direction", "maximize") == "maximize")

    @abstractmethod
    def evaluate(self, harness: IsolatedHarness, dataset_dir: str) -> VectorEvaluation:
        pass

class FunctionalVector(Vector):
    """Deterministic binary pass/fail verification vector."""
    def __init__(self, vector_id: int, name: str, higher_is_better: bool = True):
        super().__init__(vector_id, name, higher_is_better)

    @abstractmethod
    def run_functional(self, harness: IsolatedHarness, dataset_dir: str) -> Tuple[bool, float, str, int, str, bool]:
        """Returns (passed, metric_value, metric_name, sample_size_n, details, engine_error)."""
        pass

    def evaluate(self, harness: IsolatedHarness, dataset_dir: str) -> VectorEvaluation:
        try:
            passed, metric_val, metric_name, sample_n, details, engine_err = self.run_functional(harness, dataset_dir)
            ci_low, ci_high = calculate_wilson_ci(metric_val, sample_n)

            if engine_err:
                status = "🔴 ENGINE FAIL"
            elif not passed:
                status = "🟡 QUALITY BELOW TARGET"
            else:
                status = "🟢 PASS"
        except Exception as ex:
            passed = False
            metric_val = 0.0
            metric_name = "Execution Error"
            sample_n = 0
            ci_low, ci_high = (0.0, 0.0)
            details = f"Unhandled exception in evaluator: {str(ex)}"
            status = "🔴 ENGINE FAIL"

        return VectorEvaluation(
            vector_id=self.vector_id,
            name=self.name,
            vector_type="Functional",
            passed=passed,
            status_badge=status,
            score=1.0 if passed else 0.0,
            metric_name=metric_name,
            metric_value=metric_val,
            sample_size_n=sample_n,
            ci_lower=ci_low,
            ci_upper=ci_high,
            details=details,
            weight=self.weight,
            higher_is_better=self.higher_is_better,
            threshold=self.threshold
        )

class QualityVector(Vector):
    """Quantitative scored quality vector with heuristic IR metrics."""
    def __init__(self, vector_id: int, name: str, threshold: float = 0.8, higher_is_better: bool = True):
        super().__init__(vector_id, name, higher_is_better)
        self.threshold = threshold

    @abstractmethod
    def run_quality(self, harness: IsolatedHarness, dataset_dir: str) -> Tuple[float, str, int, str, bool]:
        """Returns (score_ratio, metric_name, sample_size_n, details, engine_error)."""
        pass

    def evaluate(self, harness: IsolatedHarness, dataset_dir: str) -> VectorEvaluation:
        try:
            score, metric_name, sample_n, details, engine_err = self.run_quality(harness, dataset_dir)
            ci_low, ci_high = calculate_wilson_ci(score, sample_n)

            if engine_err:
                passed = False
                status = "🔴 ENGINE FAIL"
            elif self.higher_is_better and score < self.threshold:
                passed = False
                status = "🟡 QUALITY BELOW TARGET"
            elif not self.higher_is_better and score > self.threshold:
                passed = False
                status = "🟡 QUALITY BELOW TARGET"
            else:
                passed = True
                status = "🟢 PASS"
        except Exception as ex:
            passed = False
            score = 0.0
            metric_name = "Execution Error"
            sample_n = 0
            ci_low, ci_high = (0.0, 0.0)
            details = f"Unhandled exception in evaluator: {str(ex)}"
            status = "🔴 ENGINE FAIL"

        return VectorEvaluation(
            vector_id=self.vector_id,
            name=self.name,
            vector_type="Quality",
            passed=passed,
            status_badge=status,
            score=score,
            metric_name=metric_name,
            metric_value=score,
            sample_size_n=sample_n,
            ci_lower=ci_low,
            ci_upper=ci_high,
            details=details,
            weight=self.weight,
            higher_is_better=self.higher_is_better,
            threshold=self.threshold
        )

# -------------------------------------------------------------------
# 4. Mathematical Coverage Score Engine
# -------------------------------------------------------------------

class CoverageScoreEngine:
    @staticmethod
    def calculate_published_score(harness: IsolatedHarness, dataset_dir: str) -> Tuple[float, str, Dict[str, Any]]:
        alias_file = os.path.join(dataset_dir, "aliases.json")
        conflict_file = os.path.join(dataset_dir, "conflicts.json")
        temporal_file = os.path.join(dataset_dir, "temporal.json")
        stability_file = os.path.join(dataset_dir, "stability.json")

        alias_count = len(json.load(open(alias_file))) if os.path.exists(alias_file) else 0
        conflict_count = len(json.load(open(conflict_file))) if os.path.exists(conflict_file) else 0
        temporal_count = len(json.load(open(temporal_file))) if os.path.exists(temporal_file) else 0
        stability_count = len(json.load(open(stability_file))) if os.path.exists(stability_file) else 0

        scenarios_total = alias_count + conflict_count + temporal_count + stability_count
        queries_run = harness.perf_metrics.total_queries_run
        docs_ingested = harness.perf_metrics.total_docs_ingested
        tokens_est = harness.perf_metrics.total_tokens_est

        s_scenarios = min(100.0, (scenarios_total / 10.0) * 100.0)
        c_corpus = min(100.0, (docs_ingested / 20.0) * 100.0)
        q_queries = min(100.0, (queries_run / 50.0) * 100.0)
        r_repetition = 100.0 if queries_run >= 40 else (queries_run / 40.0) * 100.0

        coverage_score = (0.35 * s_scenarios) + (0.25 * c_corpus) + (0.20 * q_queries) + (0.20 * r_repetition)
        coverage_score = min(100.0, max(0.0, coverage_score))

        if coverage_score < 30.0:
            level = "LOW (Initial Baseline)"
        elif coverage_score < 60.0:
            level = "MODERATE (Standard Suite)"
        elif coverage_score < 85.0:
            level = "HIGH (Comprehensive Suite)"
        else:
            level = "EXTENSIVE (Production Soak Suite)"

        breakdown = {
            "s_scenarios": s_scenarios,
            "c_corpus": c_corpus,
            "q_queries": q_queries,
            "r_repetition": r_repetition,
            "coverage_score": coverage_score,
            "level": level,
            "scenarios_total": scenarios_total,
            "queries_run": queries_run,
            "docs_ingested": docs_ingested,
            "tokens_est": tokens_est
        }
        return coverage_score, level, breakdown
