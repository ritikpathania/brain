# Benchmarking & Performance

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

## TUI Rendering Performance Profiling

In addition to core daemon benchmarks, the TUI includes a React-level rendering benchmark suite using the React `<Profiler>` API to profile the interface's commit and rendering speed.

### Running Render Benchmarks
To run the deterministic, manual render update profiling loop:
```bash
cd cli
bun run benchmark:render
```
This profiles Logs, Markdown, Resize, and Theme workloads synchronously inside a single-pass loop and saves a schema-versioned JSON report `benchmark_render_report.json` with metadata.

### Comparing Render Performance
To display a formatted percentage change trend report comparing the current run to a baseline or analyzing chronological progression:
```bash
# Compare the current run to benchmark_render_baseline.json
bun run benchmark:compare

# Compare average commit trends over the last 5 runs (rolling-average smoothed)
bun run benchmark:compare --last 5
```
Threshold classification flags highlight performance shifts: `< ±2%` is `[Stable]`, `2–10%` is `[Noticeable]`, and `>10%` is `[Significant]`.

