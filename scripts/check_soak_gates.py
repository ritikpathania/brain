#!/usr/bin/env python3
import sys
import json

GATES = {
    "max_rss_growth_pct": 5.0,     # Max 5.0% steady-state RSS growth
    "max_fd_delta": 2,             # Max 2 lingering file descriptors
    "max_thread_delta": 1,         # Max 1 lingering thread
    "min_socket_health_rate": 1.0  # 100% socket health rate
}

def main():
    soak_file = sys.argv[1] if len(sys.argv) > 1 else "target/soak_report.json"
    with open(soak_file) as f:
        report = json.load(f)["steady_state"]

    print("=== Steady-State Reliability Gate Evaluation ===")
    failed = False

    rss_growth = report.get("rss_steady_growth_pct", 0)
    fd_delta = report.get("fd_delta", 0)
    thread_delta = report.get("thread_delta", 0)
    health_rate = report.get("socket_health_rate", 0)

    # RSS growth check
    if rss_growth > GATES["max_rss_growth_pct"]:
        print(f"[FAIL] RSS Steady Growth: {rss_growth}% (threshold: <= {GATES['max_rss_growth_pct']}%)")
        failed = True
    else:
        print(f"[PASS] RSS Steady Growth: {rss_growth}% (threshold: <= {GATES['max_rss_growth_pct']}%)")

    # FD leak check
    if fd_delta > GATES["max_fd_delta"]:
        print(f"[FAIL] File Descriptor Delta: +{fd_delta} (threshold: <= +{GATES['max_fd_delta']})")
        failed = True
    else:
        print(f"[PASS] File Descriptor Delta: +{fd_delta} (threshold: <= +{GATES['max_fd_delta']})")

    # Thread leak check
    if thread_delta > GATES["max_thread_delta"]:
        print(f"[FAIL] Thread Delta: +{thread_delta} (threshold: <= +{GATES['max_thread_delta']})")
        failed = True
    else:
        print(f"[PASS] Thread Delta: +{thread_delta} (threshold: <= +{GATES['max_thread_delta']})")

    # Socket health check
    if health_rate < GATES["min_socket_health_rate"]:
        print(f"[FAIL] Socket Health Rate: {health_rate * 100}% (threshold: {GATES['min_socket_health_rate'] * 100}%)")
        failed = True
    else:
        print(f"[PASS] Socket Health Rate: {health_rate * 100}% (threshold: {GATES['min_socket_health_rate'] * 100}%)")

    if failed:
        print("\n❌ Reliability gates failed!")
        sys.exit(1)
    else:
        print("\n✅ All steady-state reliability gates passed!")
        sys.exit(0)

if __name__ == "__main__":
    main()
