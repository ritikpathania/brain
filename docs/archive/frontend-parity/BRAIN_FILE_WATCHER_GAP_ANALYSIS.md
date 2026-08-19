# Phase 4.4B Backend Gap Analysis: OS Filesystem Watcher

> **Document Status**: Blocked by Missing Backend Capability  
> **Status**: `BLOCKED BY MISSING BACKEND CAPABILITY`  
> **Target Subsystem**: Daemon / `brain-services` in-process file watcher  
> **Author**: Antigravity AI  
> **Date**: 2026-08-14  

---

```text
================================================================================
PHASE 4.4B GAP ANALYSIS
================================================================================
CAPABILITY: OS-Level Filesystem Watcher (inotify / fsevent / kqueue)
STATUS: BLOCKED BY MISSING BACKEND CAPABILITY
RUST MODIFICATIONS: ZERO (Deferred to future backend milestone)
CURRENT INGESTION ARCHITECTURE: Push-Based API (v1/ingest)
================================================================================
```

---

## 1. Technical Assessment

1. **Current Backend Ingestion Model**:
   - Ingestion is currently **explicit and push-based**.
   - External CLI tools, subagents, and IDE extensions submit `IngestionEnvelope` payloads via the native `v1/ingest` UDS route or A2A/ACP adapter protocols.
   - The engine processes observations, appends to the SQLite WAL event log, and pushes real-time `TaskProgress` and `ProjectionInvalidated` notifications to active subscribers via `v1/subscribe`.

2. **Absence of In-Process Watcher**:
   - The Brain Rust daemon (`daemon/`) and services layer (`crates/brain-services/`) do not currently embed a background filesystem event watcher library (such as `notify-rs`).
   - Consequently, raw filesystem file modifications on disk do not automatically trigger internal ingestion without an external trigger.

3. **Strategic Invariant Protection**:
   - Adding an OS filesystem watcher requires new async background tasks, path filtering rules, `.gitignore` traversal, debounce queues, and memory budgeting in `brain-services`.
   - In accordance with the project's **Framework Evolution Guardrails** and the frozen frontend boundary, no backend filesystem watcher library was added during frontend integration phases.

---

## 2. Recommendation for Future Backend Milestones

A future backend engineering phase can decide whether Brain should:
1. **Embed an in-process filesystem watcher** in `brain-services` using `notify-rs` to automatically push `IngestionEnvelope` events on file changes.
2. **Accept external watcher/IDE events** via Language Server Protocol (LSP) or IDE sidecars.
3. **Remain explicitly push-based** via `v1/ingest` and CLI tools.
