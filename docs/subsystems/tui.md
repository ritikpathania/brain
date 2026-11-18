---
status: active
owner: tui
canonical: false
review_cycle: quarterly
last_reviewed: 2026-09-30
applies_to: v1.1+
subsystem: tui
owns:
  - frontend integration boundary
depends_on:
  - protocol
  - sessions
used_by:
  - cli
canonical_specs:
  - docs/reference/protocol.md
  - ARCHITECTURE.md
adrs: []
rfcs: []
---

# Canonical Frontend Integration Handbook

> **Governance role:** This is an integration handbook, not a visual-design authority. The canonical development frontend is the independently authored first-party `packages/brain-frontend`.

## Purpose

Brain's terminal product boundary is:

```text
Brain CLI and services
  → versioned UDS frontend protocol
  → owned TypeScript Brain adapter/projection
  → frontend-neutral state
  → first-party Brain terminal presentation
```

The CLI first honors an explicit development override in `BRAIN_FRONTEND_ROOT`. Otherwise it checks for a validated product bundle next to the canonical CLI binary and then resolves `packages/brain-frontend` from the repository containing the executable. The selected frontend directory becomes the child working directory.

## Responsibilities

- Resolve the first-party packaged executable or development package and launch it with the effective daemon socket configuration.
- Require `v1/frontend/handshake` before Brain-backed work is usable.
- Keep Brain authoritative for sessions, transcript history, generation lifecycle, tool execution, permission settlement, reconnect reconciliation, and retrieval metadata.
- Map only versioned protocol DTOs into the frontend adapter; Rust domain/storage structures do not enter presentation components.
- Keep rendering, keyboard, focus, scrolling, and terminal behavior in the frontend presentation/input layer, above protocol and neutral state.

## Runtime flow

```text
brain ui
  → resolve explicit BRAIN_FRONTEND_ROOT development override
    or executable-relative packaged frontend
    or executable-relative repository package
  → ensure/connect daemon
  → execute the standalone frontend or run the development entry point with Bun
  → frontend adapter connects over UDS
  → v1/frontend/handshake negotiates protocol and capabilities
  → Brain-backed session and generation operations become available
```

For development, the manifest must identify `@brain/frontend`, provide a version, and start `bun run src/cli/main.ts`. The launcher resolves Bun from `PATH` or explicit `BRAIN_FRONTEND_RUNTIME`. An invalid override or missing source package or Bun causes a nonzero launch failure.

For an installation, the CLI validates the executable and packaged identity before launching the standalone frontend with its bundled runtime. An invalid detected installation fails closed. After daemon preflight, Unix terminal ownership passes to the selected executable, preserving exit and signal status.

## Operational boundaries

- Canonical CI verifies the owned in-repo package; it does not fetch external/private source.
- Packaged artifact resolution uses closed manifests. Release acceptance requires an interactive PTY smoke from the exact archived product. See the [distribution guide](../guides/product-distribution.md).
- The frontend does not execute Brain tools locally. Brain owns tool identity, permission resolution, process lifecycle, and results.
- Live generation uses lexical search, bounded graph expansion, and vector recall when a configured provider and stored embeddings are available. Visible pinned constraints take priority within the item and token budgets.
- Retrieval metadata uses the allowlisted `brain_context` contract. If context assembly fails, the daemon logs the failure and continues generation with the field absent. The adapter clears a prior summary only after an explicit successful snapshot with zero results.

## Verification

The launcher and daemon protocol suites verify discovery, handshake, sessions/history, streams, tools/permissions, reconnect, and safe context metadata. CI requires frozen Bun installation, full package typecheck, owned unit/state/render tests, and isolated real-daemon E2E including actual `brain ui` PTY launch/shutdown. Missing daemon/CLI prerequisites fail; required tests do not skip. Daemon-side Bash tests retain process-group timeout/TERM/KILL/reap proof rather than duplicating it in the UI. The product packaging orchestrator verifies the standalone build and runs a separate PTY smoke after extracting the complete archive. Source launch and compiled frontend identity checks do not establish acceptance of a full product archive.

## References

- [UDS protocol reference](../reference/protocol.md)
- [Architecture constraints](../../ARCHITECTURE.md#runtime-invariants)
- [Product distribution guide](../guides/product-distribution.md)
