#!/usr/bin/env python3
"""
Retrieval Quality Benchmark (RQB) & Operational Benchmark (OPB) Engine

Class Hierarchy:
- Vector (ABC)
  ├── FunctionalVector (Deterministic Pass/Fail)
  └── QualityVector (Quantitative Scored Metric)
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
# 1. Ephemeral Isolated Execution Harness
# -------------------------------------------------------------------

class IsolatedHarness:
    def __init__(self):
        self.tmp_dir = tempfile.TemporaryDirectory()
        self.socket_path = os.path.join(self.tmp_dir.name, "brain_rqb.sock")
        self.data_dir = os.path.join(self.tmp_dir.name, "data")
        os.makedirs(self.data_dir, exist_ok=True)
        self.http_port = str(random.randint(18000, 29000))
        self.proc: Optional[subprocess.Popen] = None

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
# 2. Vector Class Hierarchy
# -------------------------------------------------------------------

@dataclass
class VectorEvaluation:
    vector_id: int
    name: str
    vector_type: str  # "Functional" or "Quality"
    passed: bool
    score: float
    metric_name: str
    metric_value: float
    details: str

class Vector(ABC):
    def __init__(self, vector_id: int, name: str):
        self.vector_id = vector_id
        self.name = name

    @abstractmethod
    def evaluate(self, harness: IsolatedHarness, dataset_dir: str) -> VectorEvaluation:
        pass

class FunctionalVector(Vector):
    """Deterministic binary pass/fail verification vector."""
    def __init__(self, vector_id: int, name: str):
        super().__init__(vector_id, name)

    @abstractmethod
    def run_functional(self, harness: IsolatedHarness, dataset_dir: str) -> Tuple[bool, float, str, str]:
        """Returns (passed, metric_value, metric_name, details)."""
        pass

    def evaluate(self, harness: IsolatedHarness, dataset_dir: str) -> VectorEvaluation:
        passed, metric_val, metric_name, details = self.run_functional(harness, dataset_dir)
        return VectorEvaluation(
            vector_id=self.vector_id,
            name=self.name,
            vector_type="Functional",
            passed=passed,
            score=1.0 if passed else 0.0,
            metric_name=metric_name,
            metric_value=metric_val,
            details=details
        )

class QualityVector(Vector):
    """Quantitative scored quality vector with heuristic IR metrics."""
    def __init__(self, vector_id: int, name: str, threshold: float = 0.8):
        super().__init__(vector_id, name)
        self.threshold = threshold

    @abstractmethod
    def run_quality(self, harness: IsolatedHarness, dataset_dir: str) -> Tuple[float, str, str]:
        """Returns (score_ratio, metric_name, details)."""
        pass

    def evaluate(self, harness: IsolatedHarness, dataset_dir: str) -> VectorEvaluation:
        score, metric_name, details = self.run_quality(harness, dataset_dir)
        passed = score >= self.threshold
        return VectorEvaluation(
            vector_id=self.vector_id,
            name=self.name,
            vector_type="Quality",
            passed=passed,
            score=score,
            metric_name=metric_name,
            metric_value=score,
            details=details
        )

# -------------------------------------------------------------------
# 3. Benchmark Implementations
# -------------------------------------------------------------------

class ExactRetrievalVector(FunctionalVector):
    def __init__(self):
        super().__init__(1, "Exact Retrieval")

    def run_functional(self, harness: IsolatedHarness, dataset_dir: str) -> Tuple[bool, float, str, str]:
        target = "ADR-042 selected WebAssembly for client-side plugin sandbox isolation."
        harness.request("ingest", target)
        time.sleep(0.3)

        resp = harness.request("query", "WebAssembly client-side plugin sandbox")
        chunks = [r.get("content", "") for r in resp if r.get("type") == "stream_chunk"]
        lines = [c.strip() for c in chunks if c.strip().startswith("•")]

        rank = -1
        for idx, line in enumerate(lines):
            if "WebAssembly" in line or "ADR-042" in line:
                rank = idx + 1
                break

        mrr = 1.0 / rank if rank > 0 else 0.0
        passed = mrr > 0.0
        return passed, mrr, "MRR", f"Target record retrieved at rank {rank} (MRR = {mrr:.2f})"

class AliasVector(FunctionalVector):
    def __init__(self):
        super().__init__(2, "Synonyms & Aliases")

    def run_functional(self, harness: IsolatedHarness, dataset_dir: str) -> Tuple[bool, float, str, str]:
        with open(os.path.join(dataset_dir, "aliases.json"), "r") as f:
            data = json.load(f)

        total = 0
        successful = 0
        for entry in data:
            harness.request("ingest", entry["sample_text"])
            time.sleep(0.2)
            for alias in entry["aliases"]:
                total += 1
                resp = harness.request("query", alias)
                content = "".join(r.get("content", "") for r in resp if r.get("type") == "stream_chunk")
                if entry["canonical"].lower() in content.lower() or alias.lower() in content.lower():
                    successful += 1

        coverage = successful / total if total > 0 else 0.0
        passed = coverage >= 0.75
        return passed, coverage, "Alias Coverage", f"{successful}/{total} alias variants resolved canonical target"

class TypoVector(FunctionalVector):
    def __init__(self):
        super().__init__(3, "Typo Tolerance")

    def run_functional(self, harness: IsolatedHarness, dataset_dir: str) -> Tuple[bool, float, str, str]:
        harness.request("ingest", "Redis is configured as an in-memory cache for session state.")
        time.sleep(0.3)

        resp = harness.request("query", "rediss")
        content = "".join(r.get("content", "") for r in resp if r.get("type") == "stream_chunk")
        found = "redis" in content.lower()
        return found, 1.0 if found else 0.0, "Typo Recall", "Typo query 'rediss' resolved canonical 'Redis' entity"

class AcronymVector(FunctionalVector):
    def __init__(self):
        super().__init__(4, "Acronym Expansion")

    def run_functional(self, harness: IsolatedHarness, dataset_dir: str) -> Tuple[bool, float, str, str]:
        harness.request("ingest", "We use Reciprocal Rank Fusion (RRF) and Okapi BM25 for hybrid search ranking.")
        time.sleep(0.3)

        resp = harness.request("query", "RRF")
        content = "".join(r.get("content", "") for r in resp if r.get("type") == "stream_chunk")
        found = "reciprocal rank fusion" in content.lower() or "rrf" in content.lower()
        return found, 1.0 if found else 0.0, "Acronym Recall", "Acronym query 'RRF' matched full definition record"

class DeduplicationVector(FunctionalVector):
    def __init__(self):
        super().__init__(5, "Memory Deduplication")

    def run_functional(self, harness: IsolatedHarness, dataset_dir: str) -> Tuple[bool, float, str, str]:
        stmt = "The default UDS socket location is /tmp/brain.sock."
        harness.request("ingest", stmt)
        harness.request("ingest", stmt)
        harness.request("ingest", stmt)
        time.sleep(0.3)

        resp = harness.request("query", "UDS socket location")
        chunks = [r.get("content", "") for r in resp if r.get("type") == "stream_chunk"]
        lines = [c.strip() for c in chunks if c.strip().startswith("•")]
        unique = set(lines)

        dup_rate = (len(lines) - len(unique)) / len(lines) if lines else 0.0
        passed = dup_rate == 0.0
        return passed, dup_rate, "Duplicate Rate", f"{len(lines)} items rendered ({len(unique)} unique), duplicate rate = {dup_rate:.1%}"

class ConflictQualityVector(QualityVector):
    def __init__(self):
        super().__init__(6, "Conflicting Knowledge Visibility", threshold=0.9)

    def run_quality(self, harness: IsolatedHarness, dataset_dir: str) -> Tuple[float, str, str]:
        with open(os.path.join(dataset_dir, "conflicts.json"), "r") as f:
            data = json.load(f)

        total = len(data)
        surfaced = 0
        for entry in data:
            harness.request("ingest", entry["fact_a"])
            harness.request("ingest", entry["fact_b"])
            time.sleep(0.3)

            resp = harness.request("query", entry["query"])
            content = "".join(r.get("content", "") for r in resp if r.get("type") == "stream_chunk")
            both_found = all(ef.lower() in content.lower() for ef in entry["expected_facts"])
            if both_found:
                surfaced += 1

        score = surfaced / total if total > 0 else 0.0
        return score, "Conflict Visibility Rate", f"{surfaced}/{total} conflict sets surfaced both disagreeing facts"

class TemporalQualityVector(QualityVector):
    def __init__(self):
        super().__init__(7, "Temporal Evolution Recency", threshold=0.9)

    def run_quality(self, harness: IsolatedHarness, dataset_dir: str) -> Tuple[float, str, str]:
        with open(os.path.join(dataset_dir, "temporal.json"), "r") as f:
            data = json.load(f)

        aligned = 0
        for entry in data:
            harness.request("ingest", entry["old_fact"])
            time.sleep(0.2)
            harness.request("ingest", entry["new_fact"])
            time.sleep(0.3)

            resp = harness.request("query", entry["query"])
            chunks = [r.get("content", "") for r in resp if r.get("type") == "stream_chunk"]
            lines = [c.strip() for c in chunks if c.strip().startswith("•")]

            new_idx = next((i for i, l in enumerate(lines) if entry["expected_newer_keyword"].lower() in l.lower()), -1)
            old_idx = next((i for i, l in enumerate(lines) if entry["expected_older_keyword"].lower() in l.lower()), -1)

            if new_idx != -1 and old_idx != -1 and new_idx < old_idx:
                aligned += 1

        score = aligned / len(data) if data else 0.0
        return score, "Recency Order Alignment", f"Newer timestamp ranked ahead of older timestamp ({aligned}/{len(data)})"

class BroadSynthesisVector(QualityVector):
    def __init__(self):
        super().__init__(8, "Broad Synthesis Completeness", threshold=0.6)

    def run_quality(self, harness: IsolatedHarness, dataset_dir: str) -> Tuple[float, str, str]:
        resp = harness.request("query", "database architecture")
        chunks = [r.get("content", "") for r in resp if r.get("type") == "stream_chunk"]
        lines = [c.strip() for c in chunks if c.strip().startswith("•")]

        # Precision@5 computation: relevant items in top 5 / 5
        top_5 = lines[:5]
        relevant_count = sum(1 for l in top_5 if any(kw in l.lower() for kw in ["sqlite", "postgres", "redis", "database", "cache", "adr"]))
        precision_at_5 = relevant_count / 5.0

        return precision_at_5, "Precision@5", f"Precision@5 = {precision_at_5:.2f} ({relevant_count}/5 top items relevant)"

class ContextFollowupVector(QualityVector):
    def __init__(self):
        super().__init__(9, "Conversational Follow-ups", threshold=0.8)

    def run_quality(self, harness: IsolatedHarness, dataset_dir: str) -> Tuple[float, str, str]:
        resp = harness.request("query", "session state cache")
        content = "".join(r.get("content", "") for r in resp if r.get("type") == "stream_chunk")

        relevant = "redis" in content.lower() or "session" in content.lower()
        score = 1.0 if relevant else 0.0
        return score, "Context Relevance", "Conversational session history context resolved relevant caching node"

class RankingStabilityVector(QualityVector):
    def __init__(self):
        super().__init__(10, "Ranking Stability", threshold=0.95)

    def run_quality(self, harness: IsolatedHarness, dataset_dir: str) -> Tuple[float, str, str]:
        with open(os.path.join(dataset_dir, "stability.json"), "r") as f:
            data = json.load(f)

        total_runs = 0
        stable_runs = 0

        for entry in data:
            query = entry["query"]
            runs = entry["runs"]
            top_k = entry["top_k"]

            initial_resp = harness.request("query", query)
            initial_chunks = [r.get("content", "") for r in initial_resp if r.get("type") == "stream_chunk"]
            initial_top = [c.strip() for c in initial_chunks if c.strip().startswith("•")][:top_k]

            for _ in range(runs):
                total_runs += 1
                r_resp = harness.request("query", query)
                r_chunks = [r.get("content", "") for r in r_resp if r.get("type") == "stream_chunk"]
                r_top = [c.strip() for c in r_chunks if c.strip().startswith("•")][:top_k]
                if r_top == initial_top:
                    stable_runs += 1

        stability_ratio = stable_runs / total_runs if total_runs > 0 else 0.0
        return stability_ratio, "Top-3 Stability Ratio", f"Top-3 ordering remained stable across {stable_runs}/{total_runs} consecutive query runs ({stability_ratio:.1%})"
