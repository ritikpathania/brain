#!/usr/bin/env python3
"""
Retrieval Quality Benchmark (RQB) Engine v2.0.0

Features:
- Config-Driven Benchmark Policy (benchmark_config.json)
- Explicit Failure Taxonomy (PASS, ENGINE FAIL, QUALITY BELOW TARGET)
- Algorithmic Coverage & Corpus Size Index
- Ephemeral Execution Harness
"""

from abc import ABC, abstractmethod
from dataclasses import dataclass, field
import json
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
# 1. Ephemeral Isolated Execution Harness with Corpus Tracking
# -------------------------------------------------------------------

@dataclass
class CorpusMetrics:
    total_docs_ingested: int = 0
    total_tokens_est: int = 0
    total_queries_run: int = 0
    alias_entities_count: int = 0
    conflict_facts_count: int = 0

class IsolatedHarness:
    def __init__(self):
        self.tmp_dir = tempfile.TemporaryDirectory()
        self.socket_path = os.path.join(self.tmp_dir.name, "brain_rqb.sock")
        self.data_dir = os.path.join(self.tmp_dir.name, "data")
        os.makedirs(self.data_dir, exist_ok=True)
        self.http_port = str(random.randint(18000, 29000))
        self.proc: Optional[subprocess.Popen] = None
        self.corpus_metrics = CorpusMetrics()

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
        if action == "ingest":
            self.corpus_metrics.total_docs_ingested += 1
            self.corpus_metrics.total_tokens_est += len(payload.split())
        elif action == "query":
            self.corpus_metrics.total_queries_run += 1

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
                                return responses
                        except json.JSONDecodeError:
                            pass
        except socket.timeout:
            pass
        sock.close()
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
# 2. Vector Class Hierarchy & Failure Taxonomy
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
    def run_functional(self, harness: IsolatedHarness, dataset_dir: str) -> Tuple[bool, float, str, str, bool]:
        """Returns (passed, metric_value, metric_name, details, engine_error)."""
        pass

    def evaluate(self, harness: IsolatedHarness, dataset_dir: str) -> VectorEvaluation:
        try:
            passed, metric_val, metric_name, details, engine_err = self.run_functional(harness, dataset_dir)
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
    def run_quality(self, harness: IsolatedHarness, dataset_dir: str) -> Tuple[float, str, str, bool]:
        """Returns (score_ratio, metric_name, details, engine_error)."""
        pass

    def evaluate(self, harness: IsolatedHarness, dataset_dir: str) -> VectorEvaluation:
        try:
            score, metric_name, details, engine_err = self.run_quality(harness, dataset_dir)
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
            details=details,
            weight=self.weight,
            higher_is_better=self.higher_is_better,
            threshold=self.threshold
        )

# -------------------------------------------------------------------
# 3. Algorithmic Dataset & Benchmark Coverage Calculator
# -------------------------------------------------------------------

class CoverageScoreEngine:
    @staticmethod
    def calculate_score(harness: IsolatedHarness, dataset_dir: str) -> Tuple[float, str, Dict[str, Any]]:
        # Count datasets and entities
        alias_file = os.path.join(dataset_dir, "aliases.json")
        conflict_file = os.path.join(dataset_dir, "conflicts.json")
        temporal_file = os.path.join(dataset_dir, "temporal.json")
        stability_file = os.path.join(dataset_dir, "stability.json")

        alias_count = len(json.load(open(alias_file))) if os.path.exists(alias_file) else 0
        conflict_count = len(json.load(open(conflict_file))) if os.path.exists(conflict_file) else 0
        temporal_count = len(json.load(open(temporal_file))) if os.path.exists(temporal_file) else 0
        stability_count = len(json.load(open(stability_file))) if os.path.exists(stability_file) else 0

        scenarios_total = alias_count + conflict_count + temporal_count + stability_count
        queries_run = harness.corpus_metrics.total_queries_run
        docs_ingested = harness.corpus_metrics.total_docs_ingested
        tokens_est = harness.corpus_metrics.total_tokens_est

        # Algorithmic Coverage Score formula (0..100)
        raw_score = (
            (scenarios_total * 4.0) +
            (queries_run * 0.5) +
            (docs_ingested * 2.0) +
            (tokens_est * 0.05)
        )
        score = min(100.0, max(0.0, raw_score))

        if score < 30.0:
            level = "LOW (Initial Baseline)"
        elif score < 60.0:
            level = "MODERATE (Standard Suite)"
        elif score < 85.0:
            level = "HIGH (Comprehensive Suite)"
        else:
            level = "EXTENSIVE (Production Soak Suite)"

        breakdown = {
            "scenarios_count": scenarios_total,
            "queries_run": queries_run,
            "docs_ingested": docs_ingested,
            "tokens_est": tokens_est,
            "coverage_score": score,
            "level": level
        }
        return score, level, breakdown
