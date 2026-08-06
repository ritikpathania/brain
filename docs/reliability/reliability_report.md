# Long-Duration Reliability & Fault Recovery Report

## Overview

This report documents empirical baseline measurements collected during long-duration steady-state soak testing and fault recovery / failure injection validation of the `brain` daemon and CLI subsystems.

- **Date:** 2026-08-07
- **Environment:** macOS ARM64 / Release Build (`--release`)
- **Status:** PASS (100% Gate Compliance)

---

## 1. Steady-State Reliability Telemetry

Telemetry was collected across 30 continuous soak cycles involving concurrent memory ingestion, relational graph queries, and UDS health polling.

| Metric | Measured Baseline Value | Gate Threshold | Gate Status |
| :--- | :--- | :--- | :--- |
| **Soak Cycles** | 30 | N/A | N/A |
| **Initial RSS** | 18,576 KB (18.14 MB) | N/A | N/A |
| **Peak RSS** | 19,104 KB (18.66 MB) | N/A | N/A |
| **Final RSS** | 19,104 KB (18.66 MB) | N/A | N/A |
| **Steady-State RSS Growth** | **0.17%** | $\le 5.0\%$ | **PASS** |
| **Max Open File Descriptors** | 26 | N/A | N/A |
| **FD Delta ($\Delta$ FDs)** | **+0** | $\le +2$ | **PASS** |
| **Max Threads** | 13 | N/A | N/A |
| **Thread Delta ($\Delta$ Threads)** | **+0** | $\le +1$ | **PASS** |
| **Socket Health Rate** | **100.0%** | $100.0\%$ | **PASS** |

---

## 2. Fault Recovery & Failure Injection Validation

The fault recovery test suite evaluated system resilience under abrupt service restarts and UDS file deletion scenarios.

| Scenario | Objective | Observed Result | Status |
| :--- | :--- | :--- | :--- |
| **Daemon Restart Recovery** | Verify data persistence and socket re-binding after daemon stop/start cycles | Memory graph ingest retrieved successfully after restart | **PASS** |
| **Socket File Deletion Recovery** | Remove `~/.brain/daemon.sock` while daemon is stopped and restart | Daemon successfully re-created UDS socket; health check passed | **PASS** |
| **Graceful SIGTERM Exit** | Send `SIGTERM` signal to running daemon process | Daemon caught `SIGTERM` and terminated gracefully within < 0.2s | **PASS** |

---

## 3. Automated Gate Evaluation Output

```text
=== Steady-State Reliability Gate Evaluation ===
[PASS] RSS Steady Growth: 0.17% (threshold: <= 5.0%)
[PASS] File Descriptor Delta: +0 (threshold: <= +2)
[PASS] Thread Delta: +0 (threshold: <= +1)
[PASS] Socket Health Rate: 100.0% (threshold: 100.0%)

✅ All steady-state reliability gates passed!
```

---

## 4. Telemetry Raw Data Summaries

- **Soak Telemetry Report:** [`target/soak_report.json`](file:///Users/ritikpathania/Developer/PyCharm/brain/target/soak_report.json)
- **Fault Recovery Report:** [`target/fault_recovery_report.json`](file:///Users/ritikpathania/Developer/PyCharm/brain/target/fault_recovery_report.json)
