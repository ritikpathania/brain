# Reliability & Fault Recovery Validation Report

**Milestone:** `Long-Duration Reliability & Fault Recovery Validation`  
**Tier:** `CI_Baseline` (30 Cycles)  
**Recorded Date:** `2026-08-07`  

---

## 1. Two-Tier Reliability Testing Strategy

| Tier | Duration / Cycles | Purpose | Execution Trigger |
| :--- | :--- | :--- | :--- |
| **CI Reliability Baseline** | 30 cycles (~1.5 min) | Catch immediate memory/FD/thread leaks & socket regressions | Every Pull Request & CI run |
| **Extended Soak** | 6–12 hours (Nightly/Release) | Validate long-duration stability, cache fragmentation, and memory drift | Scheduled Nightly & Pre-release |

---

## 2. Empirically Validated Telemetry Metrics

| Metric | Measurement Methodology | Recorded Value | Gate Threshold | Status |
| :--- | :--- | :--- | :--- | :---: |
| **Steady-State RSS Growth** | `scripts/soak_runner.sh` (`ps` memory footprint) | **0.17 %** | $\le 5.0\%$ growth | ✅ **Pass** |
| **File Descriptor Delta ($\Delta$ FDs)** | `lsof -p <pid>` open file descriptor count | **+0** (26 FDs) | $\le +2$ lingering FDs | ✅ **Pass** |
| **Thread Count Delta ($\Delta$ Threads)** | `ps -M <pid>` active thread count | **+0** (13 threads) | $\le +1$ lingering thread | ✅ **Pass** |
| **Query Latency Drift** | `brain query` P95 response drift | **-0.3 ms** | Zero latency drift | ✅ **Pass** |
| **UDS Socket Health Rate** | `brain health` IPC ping across cycles | **100.0 %** (30/30) | $100.0\%$ healthy | ✅ **Pass** |
| **Idle CPU Utilization** | `ps -o %cpu` sampled when idle | **0.0 %** | $\le 1.0\%$ CPU | ✅ **Pass** |

---

## 3. Fault Recovery Validation Scenarios

| Scenario | Simulated Failure | Recovery Action & Result | Status |
| :--- | :--- | :--- | :---: |
| **Daemon Restart Persistence** | `brain daemon stop` while holding memory entries | Restarted daemon; `brain query` successfully retrieved graph records | ✅ **Pass** |
| **Abrupt Socket File Deletion** | Deleted `~/.brain/daemon.sock` file directly | Restarted daemon; UDS socket recreated & `brain health` returned `OK` | ✅ **Pass** |
| **Graceful `SIGTERM` Termination** | Sent `kill -TERM "$DAEMON_PID"` | Daemon cleaned up database locks and closed UDS socket cleanly (<0.2s) | ✅ **Pass** |

---

## 4. Automated CI Gate Evaluator

The `scripts/check_soak_gates.py` script enforces steady-state thresholds on `target/soak_report.json`:

```bash
python3 scripts/check_soak_gates.py target/soak_report.json
```

Output:
```text
=== Steady-State Reliability Gate Evaluation ===
[PASS] RSS Steady Growth: 0.17% (threshold: <= 5.0%)
[PASS] File Descriptor Delta: +0 (threshold: <= +2)
[PASS] Thread Delta: +0 (threshold: <= +1)
[PASS] Socket Health Rate: 100.0% (threshold: 100.0%)

✅ All steady-state reliability gates passed!
```
