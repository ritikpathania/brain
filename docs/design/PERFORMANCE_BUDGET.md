# Performance Budget

> **AUTHORITY NOTICE**: This document is a **supporting engineering specification** for `crates/brain-tui`, strictly subordinate to and governed by [`docs/design/CLAUDE_VISUAL_CONTRACT.md`](./CLAUDE_VISUAL_CONTRACT.md).


This document establishes the performance budgets, operational constraints, and latency boundaries for the Brain TUI client, aligning with our backend performance goals.

---

## 1. UX Performance Metrics

The frontend client must stay within these strict performance parameters:

| Metric | Target Budget | Upper Boundary (Violation) | Measurement Scope |
| :--- | :--- | :--- | :--- |
| **Startup Latency** | `< 20 ms` | `> 50 ms` | Executable launch to first frame draw |
| **Frame Render Time** | `< 5 ms` | `> 10 ms` | Ratatui `terminal.draw()` duration |
| **Streaming Chunk Latency**| `< 10 ms` | `> 25 ms` | Socket event arrival to typewriter render |
| **Idle CPU Usage** | `< 1 %` | `> 2 %` | CPU core utilization during idle state |
| **Animation Rate** | `10 - 15 FPS` | `> 20 FPS` | Redraw rate during spinners/streaming |
| **Peak Memory (RSS)** | `< 15 MB` | `> 30 MB` | Resident Set Size (excluding PyO3 extension) |

---

## 2. Invariants for Performance Enforcement

### 2.1. Startup Latency
* **Requirement**: The TUI must render the initial prompt and session layout immediately.
* **Mechanism**: Network lookups (Unix Domain Socket connection check) and filesystem sweeps (loading workspace folders) must run on separate background threads. The main thread draws the skeleton TUI instantly and displays an asynchronous `[Connecting...]` status badge without blocking on I/O.

### 2.2. Frame Render Optimization (Alloc-Free Drawing)
* **Requirement**: Keep Ratatui draw steps under 5ms.
* **Mechanism**:
  1. The layout partitioning and widget constraint calculations are derived strictly from cached width/height values.
  2. State reducers must pre-render or pre-wrap text blocks on update events rather than recalculating line wraps and paragraph bounds on every draw cycle.
  3. Avoid dynamic string allocations inside the `.draw()` closure. Reuse static format buffers where possible.

### 2.3. Animation Frame Rate Control
* **Requirement**: Avoid terminal flicker and high CPU by limiting redraws.
* **Mechanism**:
  1. During active animation states (spinners, typewriter streaming), the event loop triggers redrawing at a capped rate (10ms tick rate).
  2. In the idle state, the rendering tick is suspended. The draw thread blocks on incoming user key events or remote socket packets, dropping redraw rates to 0 FPS.
