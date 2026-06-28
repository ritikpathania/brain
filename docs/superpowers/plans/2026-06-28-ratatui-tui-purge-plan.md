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

## Repository Impact Metrics
We will measure and log the following before/after metrics inside the final report:
- **Files Removed**: Number of legacy TS/JS configuration files deleted.
- **Lines Removed**: Net lines of TypeScript and React/Ink code deleted.
- **Workspace Packages**: Number of active packages in the cargo/bun configurations.
- **Node Dependencies**: Total count of active node modules (reduced to 0).

---

## Commit Workflow Sequence
The purge will be executed atomically following this sequence:
1. **Pre-Purge Audits**: Verify default TUI runtime paths and document dead references.
2. **Atomic Deletion Commit**: Delete `cli/` and config files in a single, dedicated git commit.
3. **Post-Purge Scan & Negative Verification**: Run repository-wide search for retired identifiers, confirm absence of Node/JS-related configuration and CI steps, verify clean compilation and fresh-clone onboarding checks.
4. **Sign-off**: Author and commit the final Purge Report detailing the post-purge snapshot, repository state summary, and repository status conclusion.

---

### Task 1: Verify Replacement & Update Documentation

- [ ] **Step 1: Check native client default launcher**
  Verify that compiling and running the main workspace app (e.g. `brain-v2`) boots the new Ratatui TUI.
- [ ] **Step 2: Update onboarding instructions**
  Update the main `README.md`, `Makefile`, and `INSTALL.md` files to remove all references to `cli/` and `bun/npm/node`, documenting only Cargo and native Rust steps.
- [ ] **Step 3: Update package.sh**
  Update or rewrite `package.sh` to compile `brain-v2` in release mode, omitting the legacy asset-bundling step.
- [ ] **Step 4: Commit Documentation updates**
  Commit Task 1: `git add . && git commit -m "docs: update main README, Makefile, and build scripts for native Rust client default"`

---

### Task 2: Purge Legacy Frontend & Assets

- [ ] **Step 1: Conduct Dead Reference Audit**
  Scan CI workflows, shell scripts, Makefiles, documentation, Cargo metadata, and comments to ensure no contributor instructions or scripts mention the old client (`cli/` or bun/npm tasks).
- [ ] **Step 2: Delete cli/ directory & config files**
  Delete the `cli/` directory and any root configuration files (e.g. `package.json`, lockfiles).
- [ ] **Step 3: Commit Purge (Atomic Commit)**
  Commit Task 2: `git rm -r cli && git commit -m "chore: purge legacy cli typescript react/ink codebase"`

---

### Task 3: Final Verification & Purge Report

- [ ] **Step 3.1: Execute clean build verification**
  Run `cargo clean && PYO3_PYTHON=... cargo build` to verify clean compilation from scratch.
- [ ] **Step 3.2: Verify fresh-clone onboarding experience**
  Perform verification simulating a fresh clone. Confirm a contributor can clone, read the updated documentation, and run the native build successfully without needing Bun, Node, or Ink.
- [ ] **Step 3.3: Post-Purge Repository Scan & Negative Verification**
  Perform a final search across all code, comments, scripts, and documentation for retired terms (`cli/`, `Ink`, `React`, `bun`, `npm`, `yoga`) to ensure no dead references remain. Confirm that no required `package.json` exists for build, and no CI jobs/onboarding paths expect Bun/Node.js tooling.
- [ ] **Step 3.4: Documentation Consistency Check**
  Verify that README.md, INSTALL.md, migration reports, purge report, and onboarding guides describe the exact same supported workflows and application entry points.
- [ ] **Step 3.5: Create Purge Report**
  Create `docs/migration/purge_report.md` detailing:
  - Commit Hash & Date immediately after completion.
  - **Repository State Summary**: Active frontend technology (Ratatui TUI), supported build toolchain (Rust Cargo), supported package manager (Cargo), primary application entry point (`apps/brain-v2`), supported contributor workflow.
  - Cargo workspace status and number of remaining workspace crates (13 crates).
  - Confirmation of clean checkout builds.
  - Scope of legacy files and directories removed.
  - Justification of safety (what replaced it).
  - Quantitative impacts (before vs. after metrics table).
  - References pointing back to `parity_audit_report.md` and `native_ratatui_migration_report.md` for earlier test evidence and baseline results, avoiding content duplication.
  - **Repository Status Conclusion**: Concise plain-language summary confirming legacy React/Ink retirement, native Ratatui default integration, Cargo-only build workflows, and exit checklist execution.
- [ ] **Step 3.6: Commit Report**
  Commit Task 3: `git add . && git commit -m "docs(tui): complete native ratatui purge report"`
