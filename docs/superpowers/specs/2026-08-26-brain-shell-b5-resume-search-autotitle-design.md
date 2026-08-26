# Brain Shell B5 (Inc 20) — Resume Search & Auto-Titles Design

*Committed as part of the Brain shell program. Reference-only: the Claude Code tree at `/Users/ritikpathania/Developer/claude-code` informs UX parity goals, never code.*

## 0. Problem

The `/resume` picker is unusable past a handful of sessions, for two compounding reasons:

1. **Every auto-created session is titled `"New Session"`** — `SessionTitle::default()` (`crates/brain-domain/src/identifiers.rs:67`), applied by `v1/session/start` when no title arrives (`daemon/src/transport/uds/handlers.rs:303`) and passed explicitly by `BrainSessionStore.ts:111`. Picker rows are indistinguishable.
2. **The picker has no search.** `resumeChoices()` filters archived, sorts pinned-then-recency, truncates to `RESUME_MAX_ITEMS = 8` — nothing else (`packages/brain-shell/src/ui/overlays/resumePickerLogic.ts:24-30`). Older sessions are simply unreachable.

Audit provenance: Inc 12 full re-audit Tier B item **B5** ("Resume fuzzy-filter + auto-titles", classification *partial*, risk L-M).

## 1. Decisions (user-approved in brainstorming)

| Decision | Choice |
|---|---|
| Scope | Auto-titles + fuzzy search filter + current-session marker. Live preview pane **deferred** (needs new summary data over the wire) |
| Filter UX | Type-away capture — printable keystrokes feed the query immediately; backspace edits; ↑↓ navigate; enter resumes; esc cancels |
| Match semantics | Case-insensitive subsequence with light scoring (contiguous runs, word-boundary hits rank higher) |
| Title derivation | Backfill on next turn: pure domain rule fires where user messages already persist, from the aggregate's first user message; capped to the picker column |
| Shell capture mechanism | Resolver wildcard — new binding-table rows + a `printable` pseudo-key probe in `resolveAction` (keyboard authority stays data-driven) |
| Daemon placement | Domain helper on the aggregate called before existing `save_session` calls (not a services policy object) |

## 2. Auto-Titles (Rust)

### 2.1 Rule

Pure method on the aggregate:

```rust
impl Session {
    /// One-time default-title backfill (B5): if the title is still the
    /// default and any user message exists, rename from its first line.
    pub fn autotitle(&mut self);
}
```

Semantics:

- Fires only when `self.title == SessionTitle::default()`. Any other title (user-renamed or previously derived) is permanent.
- Source: the **first `MessageRole::User` message** in message order (bang-command messages `"! …"` qualify — faithful labeling).
- Derivation (`derive_title`, shared helper): first non-empty line; collapse internal whitespace runs to single spaces; if > 43 chars, truncate at 43 and append `…` (46-column budget matches the view's existing `slice(0, 46)` so no double-truncation). Exactly-43 stays untruncated. Empty/whitespace-only source → no change (retried harmlessly on later turns).
- Idempotent by construction: after one successful derivation the default check fails forever.

### 2.2 Call sites (exactly three)

Each already persists a user message then calls `storage.save_session`; insert `session_aggregate.autotitle();` immediately before that save:

| Site | Path |
|---|---|
| Generation-stream user persist ("Invariant 4") | `daemon/src/transport/uds/handlers.rs:2039` |
| Bang-command persist (Invariant 4 twin) | `handlers.rs:840` |
| `session/append_turn` save | `handlers.rs:1131` |

No schema change (title is an existing column), no RPC change, no new failure mode: retitling rides saves whose best-effort error semantics already exist.

### 2.3 Untouched by design

`"New Session"` literals in `brain-application/application.rs:1860`, `brain-services/session.rs:42`, `conversation.rs:1024`, `stub.rs:34` are service-layer defaults outside the transcript-persist path this increment targets; consolidating them is not B5 scope. The one test constructing `SessionTitle("New Session")` (`serialization_tests.rs:233`) asserts nothing about titles and is unaffected.

## 3. Fuzzy Filter & Marker (Shell)

### 3.1 Keybinding resolver extension

`packages/brain-shell/src/keybindings/resolve.ts`:

- New table rows (append-only per "later increments extend, never reorder"):
  - `{ action: 'overlay:insert', context: 'overlay', key: 'printable' }`
  - `{ action: 'overlay:backspace', context: 'overlay', key: 'backspace' }`
- `resolveAction`: after the exact-match walk misses, if `keyId` is a single plain character (length 1, not a control chord — `strokeToKey` already canonicalizes modifiers/named keys ahead of the literal), re-probe the pseudo-id `'printable'`. Overlays without the row see zero change (theme picker, dialogs).
- `strokeToKey` unchanged — printables already fall through to the literal char.

### 3.2 Pure logic — `resumePickerLogic.ts`

- `fuzzyScore(query: string, text: string): number | null` — greedy case-insensitive subsequence walk; `null` = not a match; score rewards contiguous runs and word-boundary starts.
- `applyQueryEdit(query: string, action: string, input: string): string` — pure insert/backspace reducer; unknown actions return input unchanged.
- `resumeChoices(summaries, nowMs, query?)`:
  - Empty/absent query → byte-identical to today's behavior (pinned-first recency top-8).
  - Non-empty → score ALL non-archived summaries with `fuzzyScore` over `title`; drop nulls; rank score desc, tie-break `updatedAtMs` desc; take 8.
- `RESUME_MAX_ITEMS` governs both modes.

### 3.3 View & AppShell

- `ResumePickerView`: query line under the header (`› <query>`); dimmed `●` marker prefix on the row whose id equals the live session id; "No sessions match" row when filtered empty; hint line gains nothing essential (existing hints suffice).
- `AppShell`: `resumeQuery` state; overlay handler extends with `overlay:insert` / `overlay:backspace` cases calling `applyQueryEdit`; items recomputed from the already-fetched summaries via `resumeChoices(all, Date.now(), resumeQuery)`; selection clamped to filtered length; live session id passed down for the marker. Resuming the current session remains allowed (harmless reload).

## 4. Error Handling & Edge Cases

**Titles:** empty/whitespace prompt → no-op retry next turn; multi-line → first non-empty line, collapsed; long → 43+`…`; bang commands title faithfully including the `! ` prefix; retitle persistence failure inherits existing best-effort save semantics (log-and-continue); renamed sessions never touched; concurrency safe — runs on the connection's aggregate inside the same serialization regime as the existing saves, adds no shared state.

**Filter:** query longer than any title → clean empty state; enter-on-empty is a guarded no-op; selection clamped every keystroke; only single plain characters map to `overlay:insert` so ctrl/meta chords and named keys never intercept navigation; paste bursts degrade gracefully (coalesced chunk filters literally rather than breaking ↑↓/enter/esc); archived always excluded; pinned bonus applies only to empty-query ordering; both modes share one cap path.

## 5. Testing Strategy

**Rust (TDD)** — new `crates/brain-domain/tests/session_autotitle_tests.rs`: derives from first user message when default; leaves non-default untouched; whitespace-only keeps default; multi-line first-line collapse; 43-cap with ellipsis; exactly-43 untruncated; bang message valid source; second call post-retitle no-op. Integration: one UDS test creates an untitled session, streams a turn, asserts `v1/session/load` returns the derived persisted title (survives restart). Workspace gate via the macOS cargo wrapper (`RUSTFLAGS="-C link-arg=-Wl,-rpath,/Library/Developer/CommandLineTools/Library/Frameworks"`), `--no-fail-fast`, unfiltered log capture; success = failures ⊆ {permitted `uds_security_audit_tests::test_security_path_traversal_and_invalid_identifiers`} ∪ {documented parallel-load flake family}.

**Shell (TDD)** — extend `resumePickerLogic.test.ts`: `fuzzyScore` exact/contiguous/boundary/scattered/null; `applyQueryEdit` insert/backspace/passthrough; `resumeChoices` empty-query equivalence, scored ranking, archived exclusion, 8-cap, recency tie-break. New view-text test: query line, `●` marker, empty state. Bun gate: absolute counts drift-tolerant, failure identities must equal the documented five.

**PTY smoke** — new `scripts/ptySmokeInc20.py` against real daemon+shell: seed two titled sessions via RPC; open `/resume`; type a fuzzy fragment; assert only the matching row remains; enter; assert resumed replay. Second flow: `●` sits on the live session row. Third flow: untitled session shows derived auto-title in picker after one turn.

**Cross-cutting:** touched-file tsc vs pristine-main probe (ambient-only deltas); diff-scoped vendor scan vs merge-base expecting zero; commit-per-task, explicit paths only, trailer `Co-Authored-By: Claude <noreply@anthropic.com>`.

## 6. Non-Goals

- Live preview pane (needs summary wire enrichment — deferred candidate for a future increment).
- Session rename UI; list-time display-title derivation for legacy rows (legacy rows rename on their next turn, per approved decision).
- Consolidating service-layer `"New Session"` defaults outside the transcript-persist path.
- Any IPC contract, storage-schema, or `session/list` response change.
- Claude/Anthropic-derived concepts anywhere.

## 7. Constraints & Riders

- Preserve Brain's architecture, domain model, IPC contracts, runtime, memory, retrieval, graph, provenance, agents, adapter boundaries.
- Stack: Bun + React 19 + Ink 7 + yoga-layout + Rust daemon.
- Every commit contains ONLY explicitly-added paths (`git add <paths>`, never `git add .`); working-tree user WIP (~3.7k dirty paths) is never staged, stashed, or reverted.
- Pushes to origin require explicit user approval each time.
- macOS cargo wrapper for EVERY cargo invocation (see §5).
- Sole permitted cargo failure: `uds_security_audit_tests::test_security_path_traversal_and_invalid_identifiers`.
- Commit trailer on every commit: `Co-Authored-By: Claude <noreply@anthropic.com>`.
