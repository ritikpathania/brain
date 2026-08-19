---
status: active
owner: architecture
canonical: true
review_cycle: quarterly
last_reviewed: 2026-07-30
applies_to: v0.8+
---

# Documentation Governance & Policy Specification

This document defines the formal engineering rules, metadata schemas, canonical fact policies, and maintenance guidelines governing all documentation across the `brain` repository workspace.

---

## 1. Frontmatter Metadata Policy

### Policy Scoping Rule
> **Frontmatter is required for normative and canonical documentation, optional for indexes and generated or historical documents.**

### Frontmatter Schema (For Normative Specs, RFCs, ADRs, References & Guides):
Every active normative document under `docs/` must begin with standard YAML frontmatter:

```yaml
---
status: active # active | draft | deprecated | historical
owner: architecture # architecture | storage | protocol | tui | sdk | cli | governance
canonical: true # true if authoritative source for technical facts
review_cycle: quarterly # monthly | quarterly | annual | static
last_reviewed: YYYY-MM-DD # YYYY-MM-DD
applies_to: v0.8+ # version range constraint
---
```

### Frontmatter Exemption Matrix:
The following document types are explicitly **exempt** from mandatory frontmatter:
- Directory index pages (`README.md`)
- Release notes (`release_notes_v1.md`)
- Sprint reports and retrospectives
- Performance benchmark outputs and telemetry
- Historical archives under `docs/archive/`
- Authoring templates (`*_TEMPLATE.md`)
- Generated API contract files

---

## 2. Canonical Fact Policy

### The Single Fact Principle
> **Every technical fact has exactly one canonical source document. Supporting documents summarize and link to the canonical source.**

- **Behavioral Definitions**: Storage schemas, wire frame codecs, and protocol specifications are defined strictly in their canonical reference files (e.g. `docs/reference/storage.md`, `docs/reference/protocol.md`).
- **No Inline Duplication**: Supporting documents (such as root `README.md`, `AGENTS.md`, or crate READMEs) must summarize high-level intent and provide markdown links to the canonical reference, rather than copying JSON frames or SQL DDL statements.

---

## 3. Structural README Rules

### Root `README.md` Guidelines
The repository root `README.md` must remain lightweight (~100–150 lines max). It must strictly contain:
1. **Project Purpose & Pitch** (What Brain is)
2. **High-Level System Architecture Diagram**
3. **5-Minute Quickstart Summary**
4. **Workspace Overview Table** (Listing all 25 workspace crates)
5. **Documentation Map & Links** (Pointers to `docs/`)

Deep architectural breakdowns, SQL schemas, and complete protocol wire definitions belong under `docs/`.

### Crate `README.md` Guidelines
Crate READMEs (`crates/*/README.md`) must remain intentionally concise (~50–100 lines max) and adhere to a 4-section template:
1. **`Purpose`**: 1–2 sentence description of crate identity.
2. **`Public Surface`**: Core traits, structs, or entrypoints exposed.
3. **`Out of Scope`**: What explicitly does not belong in this crate.
4. **`Documentation Links`**: Pointers to canonical specs in `docs/`.

---

## 4. Non-Destructive Deprecation Protocol

When relocating or replacing high-traffic root documents (such as `DESIGN.md` or `PLUGINS.md`):
1. **Split & Relocate**: Extract canonical specifications into designated subdirectories under `docs/`.
2. **Deprecation Notice**: Replace the root file content with a prominent warning banner:
   ```markdown
   > [!WARNING]
   > This document has been relocated to [docs/subsystems/storage.md](../subsystems/storage.md).
   > This root pointer is deprecated and will be removed in a future release.
   ```
3. **Grace Period**: Retain the deprecation pointer file for one full release cycle before deletion to allow contributors to update links and bookmarks.

---

## 5. Document Lifecycle & RFC / ADR Rules

### Architectural Decision Records (ADRs)
- Active ADRs reside under `docs/architecture/adr/`.
- Historical or superseded ADRs reside under `docs/archive/historical/adrs/`.
- ADRs use strict sequential numbering: `ADR-010-domain-boundaries.md`.

### Requests for Comments (RFCs)
- Active RFCs reside under `docs/architecture/rfc/`.
- RFC filenames maintain 3-digit identifiers (e.g. `RFC-001.md` .. `RFC-012-reflection-engine.md`).
- Document headers carry full descriptive titles (`# RFC-001 — Storage Layer`).

---

## 6. Automation & CI Verification Rules

1. **Verification Tooling**: Workspace documentation is validated using `cargo xtask docs <subcommand>`.
2. **Non-Destructive Execution**: The `cargo xtask docs` suite performs read-only discovery, validation, and reporting. It never mutates files during verification checks.
3. **CI Integration**: The `cargo xtask docs all` command executes as a blocking quality gate in CI Pull Request workflows.

---

## 7. Documentation Quality Target Matrix

All active documentation in the repository must meet these automated quality targets enforced by `cargo xtask docs all`:

| Metric | Target | Enforced Scope |
| :--- | :---: | :--- |
| **Active Broken Links** | **0** | All files outside `docs/archive/` |
| **Frontmatter Violations** | **0** | Normative specs under `architecture/`, `reference/`, `guides/`, `governance/` |
| **Subsystem Violations** | **0** | Required mini-handbooks under `docs/subsystems/` |
| **Invalid Code Snippets** | **0** | JSON code blocks in reference specifications |
| **Duplicate Canonical Ownership** | **0** | Unique `canonical: true` claims across normative docs |
| **Missing Crate READMEs** | **0** | All 25 Cargo workspace member crates & Python SDK |
| **Active Orphan Documents** | **0** | Active documents outside `docs/archive/` |

> *Note: Archive metrics under `docs/archive/` are informational to preserve historical context without blocking CI.*

