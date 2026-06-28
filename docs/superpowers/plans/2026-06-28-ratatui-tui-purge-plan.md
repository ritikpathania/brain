# Ratatui TUI Client Migration (Milestone 8: Purge) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Safely delete the legacy TypeScript React/Ink frontend, clean up workspace config scripts, update user/contributor documentation, and verify zero remnants of node-runtime bindings remain.

---

## Exit Checklist (Completion Criteria)
- [ ] **Criteria 1**: The legacy `cli/` folder is deleted.
- [ ] **Criteria 2**: No `Ink` or `React` code imports remain in the codebase.
- [ ] **Criteria 3**: Obsolete JS-build configurations (lockfiles, `package.sh` references) are removed or updated.
- [ ] **Criteria 4**: Workspace compiles from a clean checkout using only Cargo/Rust.
- [ ] **Criteria 5**: All documentation onboarding instructions reference only the native Rust client.
- [ ] **Criteria 6**: Final Purge Report is written under `docs/migration/purge_report.md`.

---

### Task 1: Verify Replacement & Update Documentation

- [ ] **Step 1: Check native client default launcher**
  Verify that compiling and running the main workspace app (e.g. `brain-v2`) boots the new Ratatui TUI.
- [ ] **Step 2: Update onboarding instructions**
  Update the main `README.md`, `Makefile`, and `INSTALL.md` files to remove all references to `cli/` and `bun/npm/node`, documenting only Cargo and native Rust steps.
- [ ] **Step 3: Update package.sh**
  Update or rewrite `package.sh` to compile `brain-v2` in release mode, omitting the legacy asset-bundling step.
- [ ] **Step 4: Commit**
  Commit Task 1: `git add . && git commit -m "docs: update main README, Makefile, and build scripts for native Rust client default"`

---

### Task 2: Purge Legacy Frontend & Assets

- [ ] **Step 1: Delete cli/ directory**
  Remove the `cli/` directory.
- [ ] **Step 2: Search and purge Ink/React remnants**
  Search the workspace to verify no residual React, Ink, or Yoga references exist.
- [ ] **Step 3: Commit Purge**
  Commit Task 2: `git rm -r cli && git commit -m "chore: purge legacy cli typescript react/ink codebase"`

---

### Task 3: Final Verification & Purge Report

- [ ] **Step 3.1: Execute clean build verification**
  Run `cargo clean && PYO3_PYTHON=... cargo build` to verify clean compilation from scratch.
- [ ] **Step 3.2: Create Purge Report**
  Create `docs/migration/purge_report.md` detailing:
  - Scope of removals
  - Justification of safety (what replaced it)
  - Quantitative impacts (dependency counts, repository size, build file savings)
- [ ] **Step 3.3: Commit Report**
  Commit Task 3: `git add . && git commit -m "docs(tui): complete native ratatui purge report"`
