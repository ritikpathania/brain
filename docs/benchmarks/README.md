# Performance benchmarks

- Use the [graph benchmark template](graph_benchmarks.md) to measure retrieval, relationship expansion and projections.
- Run `scripts/perf_runner.sh` to capture daemon startup latency, memory and idle CPU. Use `scripts/check_perf.py` to compare two reports collected under the same conditions.
- Run `scripts/retrieval-check.sh` for the maintained retrieval regression baselines.
