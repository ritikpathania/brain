---
status: active
owner: architecture
canonical: true
review_cycle: quarterly
last_reviewed: 2026-07-30
applies_to: v0.8+
---

# Benchmarking Methodology & Harness Specification

The relational memory engine is engineered for low-latency terminal usage.

## Running Criterion Benchmarks

Rust benchmarks are configured using the `criterion` crate.

To run:
```bash
cd daemon
uv run cargo bench --bench memory_benchmarks
```

Benchmark output reports are saved in HTML format under `daemon/target/criterion/report/index.html`.

## Telemetry Performance Profile

Under normal workloads:
* **STM Cache Retrieval**: ~1.57 ms to 1.99 ms end-to-end.
* **SQLite persistence write**: ~3.2 ms.
* **Dynamic Python Extractor (PyO3 GIL invocation)**: ~250 ms to 450 ms (offloaded to threadpools out-of-band via `tokio::task::spawn_blocking` to prevent blocking).

