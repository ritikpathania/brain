---
status: active
owner: architecture
canonical: true
review_cycle: quarterly
last_reviewed: 2026-11-18
applies_to: v1.1+
---

# Release Checklist — Brain First-Party Terminal Frontend (`brain ui`)

This document lists the canonical quality checks and acceptance criteria required for releasing the Brain CLI, background UDS daemon, and first-party terminal frontend ([`packages/brain-frontend`](../../packages/brain-frontend)). Every release candidate must pass these automated and interactive verification gates before tag publication.

> **Scope Note**: The legacy `packages/brain-shell` prototype and its 36 manual `scripts/ptySmokeInc*.py` scripts were retired in Phase 6 ([Phase 6 retirement record](../designs/phase-6-legacy-frontend-retirement.md)). All UI acceptance checks below apply strictly to the maintained first-party frontend (`packages/brain-frontend`) launched via `brain ui`.

---

## 1. Daemon Lifecycle, Handshake & Connection Recovery
- [ ] **Daemon Lifecycle**: Starting `brain daemon start` (or auto-starting via `brain ui`) binds the Unix Domain Socket (`daemon.sock`) in the effective Brain runtime directory and stops cleanly with `brain daemon stop`.
- [ ] **Versioned UDS Handshake**: Launching `brain ui` performs `v1/frontend/handshake` against `brain-daemon`, validating protocol compatibility before hydrating session state.
- [ ] **Startup Failure Projection**: If the daemon socket is unavailable or rejects the handshake, the terminal interaction controller projects a visible failed/incompatible connection state rather than hanging or crashing (`packages/brain-frontend/tests/state/interactions.test.ts`).
- [ ] **Bounded No-Replay Reconnection (`recovery.ts`)**:
  - After a transient socket disconnect, connection recovery performs a fresh `v1/frontend/handshake`, revalidates the authoritative `SessionId`, settles any interrupted generation once, and preserves the composer draft and restores composer focus.
  - Interrupted user turns, tool executions, and permission decisions are **never** automatically replayed (`packages/brain-frontend/tests/state/recovery.test.ts`).
  - Protocol incompatibility and `SessionNotFoundError` stop recovery immediately without retry loops.

---

## 2. Conversation, Streaming & Tool Permission Lifecycle
- [ ] **Authoritative Transcript & Thinking States**: Submitting a single-line or multiline draft appends the user turn and projects separate thinking (`MessageRole::Thinking`) and streaming assistant text states without exposing raw transport envelopes (`packages/brain-frontend/tests/presentation/rendering.test.tsx`).
- [ ] **Single-Flight Cancellation**: With composer focus, pressing `Esc` or `Ctrl+C` during an active turn dispatches `v1/generation/cancel` once (holding an early cancellation intent until `stream_start` if needed) and settles pending tool or permission rows cleanly without leaving stranded placeholders.
- [ ] **Correlated Permission Decisions**: Pending tool permission prompts take keyboard input priority (`y`/`n` allow/deny) without implicit `Enter` approval or composer text leakage, dispatching exactly one correlated resolution to `brain-daemon`. Escape denies the pending permission, and Ctrl+C cancels the turn (`packages/brain-frontend/tests/state/phase3.test.ts`).
- [ ] **Bounded Retrieval Context (`/context`)**: Running `/context` displays session-scoped retrieved-source and context-token counts from the allowlisted `metadata.brain_context` projection. Raw source references and internal ranking scores stay out of the panel.

---

## 3. Viewport Scrolling & Composer Input Routing
- [ ] **Transcript Scroll Ownership**:
  - The transcript viewport follows streaming output by default.
  - Scrolling upward locks the user's scroll anchor so incoming stream chunks, tool updates, or terminal settlement do not jump the viewport (`packages/brain-frontend/tests/state/interactions.test.ts`).
  - Returning to the bottom of the transcript re-engages auto-follow.
- [ ] **Composer Viewport & Key Routing (`keyRouter.ts`)**:
  - Wide Unicode characters and long multiline drafts wrap cleanly within physical row bounds while keeping the cursor visible on compact terminals (`packages/brain-frontend/tests/presentation/composerViewport.test.ts`).
  - Unsupported `Ctrl+<key>` combinations are safely ignored (`packages/brain-frontend/src/presentation/input/keyRouter.ts`) rather than leaking control characters into the composer draft.

---

## 4. Session Management
- [ ] **Session Picker (`Ctrl+O`)**: Pressing `Ctrl+O` opens the session picker overlay, and closing or selecting a session restores composer focus (`packages/brain-frontend/tests/presentation/terminalApp.test.tsx`).
- [ ] **Authoritative ULID Sessions**: Session creation, listing and loading operate on Brain-issued ULID `SessionId` values through `v1/session/create`, `v1/session/list` and `v1/session/load`, without synthesizing client-side fallback IDs.
- [ ] **Session History**: Loading a saved session restores its authoritative transcript without resubmitting an earlier prompt. Session rename, pin, archive and deletion controls are outside this frontend checklist.

---

## 5. Automated Packaging & Cross-Platform Release Gates
- [ ] **Workspace & Docs Gate**: `cargo xtask verify` and `cargo xtask docs all` pass with zero active documentation violations.
- [ ] **First-Party Frontend Suite**: `bun run typecheck`, `bun run test`, and `./scripts/verify-frontend-e2e.sh` pass with zero skipped or todo tests.
- [ ] **Closed Product Archive Verification (`./package.sh` & `.github/workflows/release.yml`)**:
  - `scripts/package-brain-product.sh` and `scripts/verify-daemon-runtime.sh` verify the acyclic hash graph (`product-manifest.json`, `runtime-manifest.json`, `frontend/brain-frontend/manifest.json`, `runtime/python-files.json`, and `licenses/notice-inventory.json`).
  - `scripts/verify-brain-product.sh` passes clean-extraction interactive PTY smokes on both `aarch64-apple-darwin` (3 passes) and `x86_64-unknown-linux-gnu` (1 pass) for the exact release commit before tag publication.
