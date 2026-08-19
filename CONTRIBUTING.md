# Contributing to the Relational Memory Engine

Welcome! Thank you for contributing to the Standalone Relational Memory Engine (`brain`). This guide outlines the development standards, Pull Request (PR) workflow, and design decision-making processes.

---

## 🛠️ Coding Standards

To maintain code quality, consistency, and performance across both the Rust engine and Python daemon:

### Rust Codebase (Core, Services, & TUI Client)
- **Formatting**: Always format code using `cargo fmt` prior to committing.
- **Diagnostics & Warnings**: All code must compile with zero warnings under `cargo clippy --all-targets -- -D warnings`.
- **CLI TUI Components**: 
  - When editing terminal components in `crates/brain-tui/`, use the design token primitives and layout contracts detailed in **[CLAUDE_VISUAL_CONTRACT.md](docs/design/CLAUDE_VISUAL_CONTRACT.md)** and **[CLAUDE_COMPONENT_MODEL.md](docs/design/CLAUDE_COMPONENT_MODEL.md)**.
  - Never hardcode hexadecimal, RGB, or ANSI escape colors; always reference theme-based tokens.

### Python Codebase (Semantic Daemon)
- **Dependency Management**: Powered by **[uv](https://github.com/astral-sh/uv)**. All dependencies must be tracked in `uv.lock`. Run `uv sync` to set up environments.
- **Linting & Formatting**: Enforced via **[Ruff](https://github.com/astral-sh/ruff)**. Run `uv run ruff format .` and `uv run ruff check . --fix`.
- **Type Safety**: Enforced via **[ty](https://github.com/astral-sh/ty)**. All scripts/modules must pass static type checking via `uv run ty check`.
- **Language Diagnostics**: Integrated with Pyrefly language diagnostics. Run `uv run pyrefly check` to analyze code health.

---

## 🔄 Pull Request Workflow

We follow a structured Git and review workflow to ensure code health and prevent regression:

1. **Isolation**: Always branch off the latest `main`.
2. **Local Verification**:
   - Run core unit and integration tests:
     ```bash
     cargo test
     ```
   - Run TUI rendering parity and state tests:
     ```bash
     PYO3_PYTHON=$(pwd)/daemon/.venv/bin/python cargo test -p brain-tui
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

## 🏛️ RFC & ADR Process

For significant architectural changes, we use a two-stage design alignment process:

### 1. Request for Comments (RFC)
For proposing new major features, APIs, protocol changes, or core designs:
- Create a new markdown file under **[rfc/](docs/architecture/rfc)** following the template structure in **[RFC_TEMPLATE.md](docs/architecture/rfc/RFC_TEMPLATE.md)**.
- Share the RFC with the team/maintainers for collaborative feedback.

### 2. Architectural Decision Records (ADR)
Once a design is finalized, or when recording foundational architectural decisions (e.g., opting for Ratatui over Ink, adopting SQLite BLOB storage for vectors):
- Author an ADR under **[adr/](docs/architecture/adr)**.
- Name the file sequentially using the pattern `ADR-XXX.md` (e.g., `ADR-009.md`).
- Document the context, choices, alternatives considered, and consequences.
