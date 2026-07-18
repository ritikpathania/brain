#!/usr/bin/env python3
"""
Sprint 7 — BrainRuntime Parity Soak Test
=========================================

Sends N ingest observations to a running brain daemon over its UDS socket,
then queries /metrics/json to produce a parity summary comparing the legacy
ingestion path against the BrainRuntime path.

Usage
-----
    python3 scripts/soak_test.py [OPTIONS]

Options
-------
    --socket PATH       UDS socket path (default: ~/.brain/daemon.sock)
    --metrics-port PORT HTTP metrics port (default: 8080)
    --count N           Number of observations to send (default: 100)
    --interval SECS     Delay between observations in seconds (default: 0.05)
    --verbose           Print each request/response pair

Example
-------
    # Quick smoke test (100 observations, ~5 seconds)
    python3 scripts/soak_test.py

    # Full soak (1000 observations over ~60 seconds)
    python3 scripts/soak_test.py --count 1000 --interval 0.06

Snapshot consistency note
-------------------------
Parity counters are sampled from independent atomics. A scrape that observes
attempts > successes + failures indicates an in-flight request. Use long-term
trends and rates for migration decisions, not single samples.

This script queries metrics once, after all observations complete — by then
all in-flight requests have settled, making the final snapshot reliable for
the soak duration.
"""

import argparse
import json
import socket
import sys
import time
import urllib.error
import urllib.request
from pathlib import Path


# ---------------------------------------------------------------------------
# Protocol helpers
# ---------------------------------------------------------------------------

def _make_ingest_request(req_id: int, text: str) -> bytes:
    """Build a versioned ingest request as a newline-terminated JSON line."""
    msg = {
        "version": "1.0",
        "type": "Request",
        "id": req_id,
        "action": "ingest",
        "body": text,
    }
    return (json.dumps(msg) + "\n").encode()


def _parse_response(line: str) -> dict:
    """Parse a single JSON response line."""
    try:
        return json.loads(line.strip())
    except json.JSONDecodeError:
        return {"status": "parse_error", "raw": line.strip()}


# ---------------------------------------------------------------------------
# Soak runner
# ---------------------------------------------------------------------------

def run_soak(
    socket_path: Path,
    metrics_port: int,
    count: int,
    interval: float,
    verbose: bool,
) -> dict:
    """
    Send `count` ingest observations over a single UDS connection, then read
    back all responses. Returns raw response list.
    """
    observations = [
        f"[soak-{i:04d}] The brain daemon is being exercised by the Sprint 7 soak test. "
        f"Observation index {i} of {count}. "
        f"Timestamp: {time.time():.6f}."
        for i in range(count)
    ]

    responses = []
    sock = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)

    try:
        sock.connect(str(socket_path))
        sock_file = sock.makefile("rwb", buffering=0)

        send_start = time.monotonic()

        for i, text in enumerate(observations):
            req_id = i + 1
            payload = _make_ingest_request(req_id, text)
            sock.sendall(payload)

            if verbose:
                print(f"  → [{req_id:04d}] sent ({len(text)} chars)", flush=True)

            if interval > 0:
                time.sleep(interval)

        send_elapsed = time.monotonic() - send_start

        # Read all responses (one per request)
        recv_start = time.monotonic()
        for i in range(count):
            raw = sock_file.readline()
            if not raw:
                break
            resp = _parse_response(raw.decode())
            responses.append(resp)

            if verbose:
                print(f"  ← [{i+1:04d}] status={resp.get('status', '?')}", flush=True)

        recv_elapsed = time.monotonic() - recv_start

    finally:
        sock.close()

    return {
        "responses": responses,
        "send_elapsed": send_elapsed,
        "recv_elapsed": recv_elapsed,
    }


# ---------------------------------------------------------------------------
# Metrics fetch
# ---------------------------------------------------------------------------

def fetch_metrics(port: int) -> dict:
    """Query GET /metrics/json from the health server."""
    url = f"http://127.0.0.1:{port}/metrics/json"
    try:
        with urllib.request.urlopen(url, timeout=5) as resp:
            return json.loads(resp.read())
    except urllib.error.URLError as e:
        print(f"  [warn] Could not reach metrics endpoint at {url}: {e}", file=sys.stderr)
        return {}


# ---------------------------------------------------------------------------
# Report
# ---------------------------------------------------------------------------

def print_report(
    count: int,
    interval: float,
    timing: dict,
    before: dict,
    after: dict,
    success_threshold: float,
    latency_threshold: float,
) -> None:
    responses = timing["responses"]
    successes = sum(1 for r in responses if r.get("status") == "success")
    errors = len(responses) - successes

    # Parity deltas (after - before, guarded against missing keys)
    def delta(key: str) -> float:
        return after.get(key, 0) - before.get(key, 0)

    rt_attempts   = delta("runtime_ingest_attempts")
    rt_successes  = delta("runtime_ingest_successes")
    rt_failures   = delta("runtime_ingest_failures")
    legacy_count  = delta("total_ingests")

    rt_success_rate = after.get("runtime_ingest_success_rate", 0.0)
    rt_avg_lat_us   = after.get("runtime_avg_ingest_latency_us", 0.0)
    lg_avg_lat_us   = after.get("legacy_avg_ingest_latency_us", 0.0)
    rt_lat_ratio    = after.get("runtime_ingest_latency_ratio", 0.0)

    # Wall time
    total_wall = timing["send_elapsed"] + timing["recv_elapsed"]

    width = 62
    sep = "─" * width

    print()
    print(f"Soak Test Results ({count} observations, {total_wall:.1f}s wall time)")
    print(sep)
    print(f"{'Legacy ingests:':<32} {int(legacy_count):>6} / {count:<6}  "
          f"({legacy_count/count*100:.1f}%)")
    print(f"{'Runtime attempts:':<32} {int(rt_attempts):>6} / {count:<6}  "
          f"({rt_attempts/count*100:.1f}%)")
    print(f"{'Runtime successes:':<32} {int(rt_successes):>6} / {count:<6}  "
          f"({rt_successes/count*100:.1f}%)")
    print(f"{'Runtime failures:':<32} {int(rt_failures):>6} / {count:<6}  "
          f"({rt_failures/count*100:.1f}%)")
    print(f"{'Protocol errors (client):':<32} {errors:>6}")
    print()
    rt_canon_us = after.get("runtime_avg_canonicalization_us", 0.0)
    rt_reflect_us = after.get("runtime_avg_reflection_us", 0.0)
    rt_dispatch_us = after.get("runtime_avg_dispatch_us", 0.0)
    p50_us = after.get("runtime_p50_latency_us", 0.0)
    p95_us = after.get("runtime_p95_latency_us", 0.0)
    p99_us = after.get("runtime_p99_latency_us", 0.0)

    other_us = max(0.0, rt_avg_lat_us - (rt_canon_us + rt_reflect_us + rt_dispatch_us))

    print()
    print("Latency (avg over soak window)")
    print(f"  {'Legacy path:':<28} {lg_avg_lat_us:>8.1f} µs")
    print(f"  {'Runtime path:':<28} {rt_avg_lat_us:>8.1f} µs")
    print(f"  {'Ratio (runtime / legacy):':<28} {rt_lat_ratio:>8.2f}x   "
          f"{'✓ parity' if rt_lat_ratio < latency_threshold else '⚠ regression'}")
    print("  Percentiles:")
    print(f"    {'p50:':<26} {p50_us:>8.1f} µs")
    print(f"    {'p95:':<26} {p95_us:>8.1f} µs")
    print(f"    {'p99:':<26} {p99_us:>8.1f} µs")
    print()
    if rt_avg_lat_us > 0:
        print("Stage Breakdown (avg over successful runtime ingests)")
        print(f"  {'Canonicalization:':<28} {rt_canon_us:>8.1f} µs  ({rt_canon_us / rt_avg_lat_us * 100.0:>5.1f}%)")
        print(f"  {'Reflection:':<28} {rt_reflect_us:>8.1f} µs  ({rt_reflect_us / rt_avg_lat_us * 100.0:>5.1f}%)")
        print(f"  {'Dispatch:':<28} {rt_dispatch_us:>8.1f} µs  ({rt_dispatch_us / rt_avg_lat_us * 100.0:>5.1f}%)")
        print(f"  {'Other / overhead:':<28} {other_us:>8.1f} µs  ({other_us / rt_avg_lat_us * 100.0:>5.1f}%)")
        print()
    print("Divergence")
    print("  not yet measured (Phase 2+ — canonical digest comparison)")
    print()

    # Migration signal
    reliable = rt_success_rate >= success_threshold
    fast_enough = rt_lat_ratio < latency_threshold or rt_lat_ratio == 0.0
    print("Migration readiness signal")
    print(f"  Policy thresholds (operational, not architectural):")
    print(f"    success  ≥ {success_threshold*100:.0f}%    "
          f"latency < {latency_threshold:.1f}×")
    print(f"  Reliable (≥{success_threshold*100:.0f}% success):    "
          f"{'✓ yes' if reliable  else '✗ no '} "
          f"({rt_success_rate*100:.1f}%)")
    print(f"  Latency acceptable (<{latency_threshold:.1f}×):    "
          f"{'✓ yes' if fast_enough else '✗ no '} "
          f"({rt_lat_ratio:.2f}×)")
    if reliable and fast_enough and rt_attempts > 0:
        print()
        print("  → Evidence supports runtime-first evaluation.")
        print("    Collect more data before deciding ingress authority.")
    elif rt_attempts == 0:
        print()
        print("  → No runtime attempts recorded. Check daemon logs.")
    else:
        print()
        print("  → Evidence does not yet support runtime-first migration.")
        print("    Investigate failures before proceeding.")
    print()


# ---------------------------------------------------------------------------
# Entry point
# ---------------------------------------------------------------------------

def main() -> None:
    parser = argparse.ArgumentParser(
        description="Sprint 7 BrainRuntime parity soak test",
        formatter_class=argparse.RawDescriptionHelpFormatter,
    )
    parser.add_argument(
        "--socket",
        default=str(Path.home() / ".brain" / "daemon.sock"),
        help="UDS socket path (default: ~/.brain/daemon.sock)",
    )
    parser.add_argument(
        "--metrics-port",
        type=int,
        default=8080,
        metavar="PORT",
        help="Metrics HTTP port (default: 8080)",
    )
    parser.add_argument(
        "--count",
        type=int,
        default=100,
        metavar="N",
        help="Number of observations to send (default: 100)",
    )
    parser.add_argument(
        "--interval",
        type=float,
        default=0.05,
        metavar="SECS",
        help="Delay between sends in seconds (default: 0.05)",
    )
    parser.add_argument(
        "--verbose",
        action="store_true",
        help="Print each request/response pair",
    )
    parser.add_argument(
        "--success-threshold",
        type=float,
        default=0.99,
        metavar="RATE",
        help="Minimum runtime success rate to signal readiness (default: 0.99 = 99%%). "
             "Operational policy — tighten as evidence accumulates.",
    )
    parser.add_argument(
        "--latency-threshold",
        type=float,
        default=3.0,
        metavar="RATIO",
        help="Maximum runtime/legacy latency ratio to signal readiness (default: 3.0×). "
             "Operational policy — tighten as evidence accumulates.",
    )
    args = parser.parse_args()

    socket_path = Path(args.socket)
    if not socket_path.exists():
        print(
            f"error: daemon socket not found at {socket_path}\n"
            "       Is the daemon running? Try: brain daemon run",
            file=sys.stderr,
        )
        sys.exit(1)

    print(f"Connecting to {socket_path}")
    print(f"Sending {args.count} observations (interval={args.interval}s)...")
    print()

    # Snapshot metrics before
    print("Fetching baseline metrics...")
    before = fetch_metrics(args.metrics_port)

    # Run soak
    soak_start = time.monotonic()
    timing = run_soak(
        socket_path=socket_path,
        metrics_port=args.metrics_port,
        count=args.count,
        interval=args.interval,
        verbose=args.verbose,
    )
    soak_elapsed = time.monotonic() - soak_start
    print(f"Soak complete in {soak_elapsed:.1f}s. Fetching final metrics...")

    # Allow any final in-flight async work to settle
    time.sleep(0.2)

    # Snapshot metrics after
    after = fetch_metrics(args.metrics_port)

    # Print report
    print_report(
        count=args.count,
        interval=args.interval,
        timing=timing,
        before=before,
        after=after,
        success_threshold=args.success_threshold,
        latency_threshold=args.latency_threshold,
    )


if __name__ == "__main__":
    main()
