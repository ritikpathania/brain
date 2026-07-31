---
status: active
owner: architecture
canonical: true
review_cycle: quarterly
last_reviewed: 2026-07-30
applies_to: v0.8+
---

# Brain Maintenance & Operational Guide

This document details how to upgrade, configure, clean up, and maintain the `brain` developer tool runtime.

---

## State & Data Directory Layout
All tool data, logs, databases, and IPC sockets are isolated under the user's home directory at `~/.brain/`.

- **Database (SQLite)**: `~/.brain/brain.db`
- **Daemon Socket**: `~/.brain/daemon.sock`
- **Daemon PID File**: `~/.brain/daemon.pid`
- **Daemon Logs**: `~/.brain/daemon.log`

---

## 1. Upgrading via Cargo

Pull the latest source updates and re-run cargo install:
```bash
cd brain-engine
git pull
PYO3_PYTHON=$(which python3) cargo install --path apps/brain --force --bin brain
```

---

## 2. System Reset & Cleanup
If you wish to reset the local engine state and clear cached memory graphs:

```bash
# Stop running daemon process
brain daemon stop

# Remove runtime database & logs
rm -rf ~/.brain/
```
