#!/usr/bin/env python3
import sys
import json

THRESHOLDS = {
    "cold_startup_ms": 0.10,   # Max 10% regression
    "sampled_rss_kb": 0.05,    # Max 5% growth
    "idle_cpu_percent": 1.0,   # Max 1.0% absolute CPU
}

def main():
    if len(sys.argv) < 3:
        print("Usage: check_perf.py <baseline.json> <current.json>")
        sys.exit(1)

    with open(sys.argv[1]) as f:
        baseline = json.load(f)["metrics"]
    with open(sys.argv[2]) as f:
        current = json.load(f)["metrics"]

    failed = False
    print("=== Performance Regression Gate Evaluation ===")

    for key, max_allowed in THRESHOLDS.items():
        base_val = float(baseline.get(key, 0))
        curr_val = float(current.get(key, 0))

        if base_val == 0:
            delta_pct = 0.0
        else:
            delta_pct = (curr_val - base_val) / base_val

        status = "PASS"
        if key == "idle_cpu_percent":
            if curr_val > max_allowed:
                status = "FAIL"
                failed = True
        elif delta_pct > max_allowed:
            status = "FAIL"
            failed = True

        print(f"[{status}] {key}: baseline={base_val}, current={curr_val} (delta={delta_pct*100:+.2f}%, threshold=+{max_allowed*100:.1f}%)")

    if failed:
        print("\n❌ Performance regression gate failed!")
        sys.exit(1)
    else:
        print("\n✅ Performance regression gate passed!")
        sys.exit(0)

if __name__ == "__main__":
    main()
