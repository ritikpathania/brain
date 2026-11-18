# Contributing to the Relational Memory Engine

Welcome! Thank you for contributing to the Standalone Relational Memory Engine (`brain`). This guide outlines the development standards, Pull Request (PR) workflow, and design decision-making processes.

---

## 🛠️ Coding Standards

To maintain code quality, consistency, and performance across the Rust daemon, TypeScript frontend and Python SDK.

### Rust and TypeScript
- **Formatting**: Always format code using `cargo fmt` prior to committing.
- **Diagnostics & Warnings**: All code must compile with zero warnings under `cargo clippy --all-targets -- -D warnings`.
- **Terminal frontend components**
  - When editing terminal components, follow the design token primitives and layout contracts detailed in **[GUIDELINES.md](GUIDELINES.md)** and **[AGENTS.md](AGENTS.md)**.
  - Never hardcode hexadecimal, RGB, or ANSI escape colors in Brain-authored frontend components; always reference theme-based tokens.

### Python tooling and SDK
- **Dependency Management**: Powered by **[uv](https://github.com/astral-sh/uv)**. All dependencies must be tracked in `uv.lock`. Run `uv sync` to set up environments.
- **Linting & Formatting**: Enforced via **[Ruff](https://github.com/astral-sh/ruff)**. Run `uv run ruff format .` and `uv run ruff check . --fix`.
- **Type Safety**: Enforced via **[ty](https://github.com/astral-sh/ty)**. All scripts/modules must pass static type checking via `uv run ty check`.
- **Language Diagnostics**: Integrated with Pyrefly language diagnostics. Run `uv run pyrefly check` to analyze code health.

---

## 🔄 Pull Request Workflow

We follow a structured Git and review workflow to ensure code health and prevent regression:

1. **Isolation**: Always branch off the latest `main`.
2. **Local Verification**:
   - Run the workspace verification gate.
     ```bash
     cargo xtask verify
     ```
   - Build the development binaries using the installation guide, then run the maintained frontend typecheck, owned tests and real-daemon tests. [Installation](docs/guides/installation.md#2-build-and-run-from-a-checkout)
     ```bash
     (cd packages/brain-frontend && bun run typecheck && bun run test)
     BRAIN_CLI_BIN="$PWD/target/debug/brain" \
       BRAIN_DAEMON_BIN="$PWD/target/debug/brain-daemon" \
       ./scripts/verify-frontend-e2e.sh
     ```
   - Ensure the python environment is clean:
     ```bash
     cd daemon && uv run ruff check . && uv run ty check
     ```
3. **Commit Discipline**:
   - Write clean, imperative commit messages (e.g., `feat: implement vector search strategy`, `docs: reorder reference manuals`).
   - Keep commits granular. Separate general documentation moves from structural code or design spec revisions.
4. **Documentation Invariants**:
   - Every feature PR must register new documentation files in the **[Documentation Index](docs/README.md)** and comply with **[GOVERNANCE.md](docs/governance/GOVERNANCE.md)**.

---

## 🔒 Engineering Philosophy

### Every defect should leave the system harder to break than before.

When fixing a defect, strengthen the project's executable guardrails where
practical. Depending on the nature of the defect, this may be a regression
test, an architectural invariant, a contract validation, a performance
baseline, or another automated check that prevents the same class of failure
from silently returning.

This is not a mandate to add a new test for every one-line fix. It is a
commitment to ask the question — and to act on it when the answer is clear.

### Guardrail taxonomy

The project classifies invariants by the kind of promise they protect, not by
the tool used to protect them. This answers the question *"where does this
guardrail belong?"* rather than just *"should I add one?"*

| Invariant violated | Preferred guardrail |
|---|---|
| Runtime behavior regressed | Regression or integration test |
| Architectural boundary crossed | `ArchitectureRule` in `crates/brain-arch-tests/` |
| Public API or schema drifted | `cargo xtask verify-contracts` |
| Performance characteristic regressed | Benchmark baseline |
| Documentation promise broken | Documentation check *(add when the pain is real, not before)* |

### Checklist for defect fixes

When closing a bug, work through this list before marking the PR ready:

- [ ] Did runtime behavior regress?
  → Add or strengthen a regression test.

- [ ] Did an architectural boundary fail?
  → Add or update an ArchitectureRule in crates/brain-arch-tests/.

- [ ] Did a public contract drift?
  → Update and re-run: cargo xtask verify-contracts

- [ ] Did performance regress?
  → Add or update a benchmark baseline.

- [ ] Did documentation describe an invariant that was violated?
  → Update the documentation and add an executable check if feasible.

The left-hand side describes the *kind of promise that was broken*. The
right-hand side describes *where to encode the protection*. If tool names
change in future, the left-hand side still reads correctly.

---

## Design review

Follow the [phase workflow](AGENTS.md#phase-workflow) for substantial changes. Record durable decisions affecting architecture or published contracts in [ADRs](docs/architecture/adr) and update the canonical documentation.

An ADR records the decision, alternatives, trade-offs, and the evidence supporting it. [Use the RFC template](docs/architecture/rfc/RFC_TEMPLATE.md) when discussion needs a standalone proposal.
