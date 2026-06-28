# Contributing to the Relational Memory Engine

Welcome! Thank you for contributing to the Standalone Relational Memory Engine (`brain`). This guide outlines the development standards, Pull Request (PR) workflow, and design decision-making processes.

---

## 🛠️ Coding Standards

To maintain code quality, consistency, and performance across both the Rust engine and Python daemon:

### Rust Codebase (Core, Services, & TUI Client)
- **Formatting**: Always format code using `cargo fmt` prior to committing.
- **Diagnostics & Warnings**: All code must compile with zero warnings under `cargo clippy --all-targets -- -D warnings`.
- **CLI TUI Components**: 
  - When editing terminal components in `crates/brain-tui/`, use the design token primitives detailed in **[DESIGN.md](file:///Users/ritikpathania/Developer/PyCharm/brain/DESIGN.md)**.
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
   - Every feature PR must update the project-level walkthrough at **[WALKTHROUGH.md](file:///Users/ritikpathania/Developer/PyCharm/brain/WALKTHROUGH.md)** and register any new documents in the **[Documentation Index](file:///Users/ritikpathania/Developer/PyCharm/brain/docs/README.md)**.

---

## 🏛️ RFC & ADR Process

For significant architectural changes, we use a two-stage design alignment process:

### 1. Request for Comments (RFC)
For proposing new major features, APIs, protocol changes, or core designs:
- Create a new markdown file under **[rfc/](file:///Users/ritikpathania/Developer/PyCharm/brain/docs/architecture/rfc/)** following the template structure in **[RFC_TEMPLATE.md](file:///Users/ritikpathania/Developer/PyCharm/brain/docs/architecture/rfc/RFC_TEMPLATE.md)**.
- Share the RFC with the team/maintainers for collaborative feedback.

### 2. Architectural Decision Records (ADR)
Once a design is finalized, or when recording foundational architectural decisions (e.g., opting for Ratatui over Ink, adopting SQLite BLOB storage for vectors):
- Author an ADR under **[adr/](file:///Users/ritikpathania/Developer/PyCharm/brain/docs/architecture/adr/)**.
- Name the file sequentially using the pattern `ADR-XXX.md` (e.g., `ADR-009.md`).
- Document the context, choices, alternatives considered, and consequences.
