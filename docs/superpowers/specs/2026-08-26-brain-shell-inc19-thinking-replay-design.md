# Brain Shell Inc 19 — Thinking-Block Persistence & Replay Design

Date: 2026-08-26
Status: Approved design (sectioned approval in chat, 2026-08-26)
Branch: `feature/brain-shell-inc19-thinking-replay` (from main @ 7a306d80)

## 0. Problem

Thinking blocks stream and render live but are lost everywhere below the
stream layer, so resumed sessions show no trace of them:

- **Live path works.** The daemon emits `thinking_start` / `thinking_delta`
  (text) / `thinking_end` (daemon-measured `duration_ms`) frames during
  `v1/generation/stream` (`daemon/src/transport/uds/handlers.rs:2234–2272`);
  the shell accumulates `live.thinkingText`, and on freeze `turnToRows`
  emits a thinking row with text + durationMs
  (`packages/brain-shell/src/ui/transcript/toRows.ts:22–29`).
- **Persist drops it.** Only `accumulated_response`, fed exclusively by
  `TextDelta` (`handlers.rs:2281`), becomes an Assistant message on
  successful completion (`handlers.rs:~2672`). No thinking accumulation
  exists anywhere daemon-side.
- **Replay cannot express it.** Stored sessions serialize as one serde JSON
  blob per session (`crates/brain-storage/src/store.rs:2212–2235`);
  messages carry a flat `{id, role, content}` where `MessageRole` has only
  User/Assistant/System/Tool (`crates/brain-domain/src/entities.rs:17`).
  `v1/session/load` maps roles to strings verbatim
  (`SessionMessageDto`, `daemon/src/server/protocol.rs:315`), and shell
  replay maps only user/assistant/tool roles, defaulting everything else
  to system rows (`packages/brain-shell/src/state/sessionReplay.ts:65–78`).

Goal: persist every completed thinking segment and show a collapsed
"✻ Thought for X.Xs" summary line at its chronological position when the
session is resumed.

## 1. Decisions (user-approved in brainstorming)

| Decision | Choice | Rejected alternatives |
|---|---|---|
| Replay fidelity | Summary line only; full text persisted, not rendered | Full text replay; collapsible interactive row |
| Persistence approach | **A**: new `MessageRole::Thinking` + v1 envelope in content + best-effort save at each `ThinkingEnd` | B: envelope on System role (semantically dishonest); C: sidecar table + DTO extension (largest contract change) |

Approach A rationale: direct precedent (`MessageRole::Tool` was added to
the same enum for transcript persistence in Inc 8; `tool_event` set the
v1-envelope-in-content convention), zero schema migration (sessions are
single serde JSON blobs), true interleaved ordering via mid-stream
persistence exactly like tool events, and graceful cross-version behavior.

## 2. Domain & Persistence (Rust)

**Domain** — add to `MessageRole`:

```rust
/// Reasoning block persisted as part of the transcript (Inc 19).
Thinking,
```

Serializes as `"thinking"` under the existing
`#[serde(rename_all = "lowercase")]`. Every exhaustive `match` the
compiler flags gains an arm; two are known today: the `Display` impl and
the `v1/session/load` role mapper (`handlers.rs:412`, → `"thinking"`).

**Envelope** — content of a Thinking message is exactly:

```json
{"type": "thinking_block", "v": 1, "text": "<full segment text>", "duration_ms": <u64>}
```

Mirrors Inc 8's `tool_event` envelope so duration rides inside content;
no storage or DTO schema changes.

**Stream loop** (`handlers.rs`, accumulator declared beside
`accumulated_response` ~:2182):

- `ThinkingStart` → clear `thinking_text`; existing `started_at` logic unchanged.
- `ThinkingDelta { text }` → append to `thinking_text`; frame emission unchanged.
- `ThinkingEnd` → after emitting the frame, if `!thinking_text.is_empty()`:
  take the text, build the envelope with the same measured duration the
  frame carries, then
  `session_aggregate.add_message(Message::new(MessageId::new(), MessageRole::Thinking, envelope))`
  and best-effort `storage.save_session(...)`. On save failure: log and
  continue generation — identical discipline to tool-event persistence
  (`handlers.rs:2486–2494`). Empty segments persist nothing.

**Ordering & cancel semantics.** Persisting at each `ThinkingEnd` lands
messages chronologically between tool events of the same stream.
Segments already ended persist even if the turn later cancels; an open
segment at cancel-time drops, consistent with Invariant 4's treatment of
partial assistant text. Concurrency model unchanged: the stream task
exclusively owns `session_aggregate` mid-stream.

## 3. Generation-Input Guard (critical)

Context assembly includes every non-`System` stored message in
model-bound history (`crates/brain-services/src/conversation.rs:358–361`).
That filter MUST become `!(System | Thinking)` so envelope JSON never
leaks into prompts and replayed thinking never burns budget tokens.

The daemon's own stream path needs no guard:
`model_messages` builds from the shell-authored request
(`handlers.rs:2058`) whose role mapping already drops unknown roles
(`filter_map … _ => return None`), and the shell never sends thinking
blocks (`BrainChatMessage` roles are user/assistant/system only).

Pre-existing observation, explicitly out of scope: Tool-role
`tool_event` envelopes currently ride that same non-`System` set into
context assembly. Not introduced by this work; not fixed here; flagged
for its own future increment.

## 4. Wire & Shell Replay

**Wire**: `SessionMessageDto` / `v1/session/load` body structure
unchanged; `role: String` gains the value `"thinking"`.
Shell `BrainMessage.role` union widens to include `'thinking'`
(`src/client/BrainBackendClient.ts:199`); the client's field mapping
already passes roles through untouched.

**Replay parser** (`sessionReplay.ts`), mirroring `toolCardFromContent`:

- Parse content as JSON; require object, `type === 'thinking_block'`,
  `v === 1`, string `text`; optional numeric `duration_ms` (absent → row
  without summary line, body still suppressed by `collapsed`).
- Valid → `{ kind:'thinking', id, text, durationMs?, collapsed: true }`.
  Full text stays available in the row model for any future expand
  feature; rendering stays summary-only.
- Invalid/malformed → falls through to the existing system-row fallback
  so corrupted history remains visible (house rule).

Strictness is deliberate: exactly `type:'thinking_block'`, `v:1`;
anything else — including future versions — renders as a visible system
row rather than being silently mis-rendered, matching how tool events behave.

**Row contract** (`contracts/messages.ts`): thinking variant gains one
optional additive field `collapsed?: boolean`. Live rows never set it.

**Rendering** (`ThinkingRowView`, `MessageRow.tsx`): italic body line
renders only when `!collapsed && text.trim().length > 0`; the
"✻ Thought for X.Xs" line renders exactly as today. Live frozen turns are
byte-for-byte unchanged; resumed sessions show summary lines only. This
also removes a latent wart: a text-less thinking row previously rendered
a stray lone `✻`.

**Cross-version matrix**:

| Daemon \ Shell | Old shell | New shell |
|---|---|---|
| Old daemon | status quo | status quo (no `"thinking"` roles ever arrive) |
| New daemon | unknown role → visible system row w/ raw envelope (degraded, honest) | full feature |

No migration anywhere; old sessions replay identically.

## 5. Testing Strategy

**Rust**
- Domain serde round-trip asserting `MessageRole::Thinking ⇄ "thinking"`.
- Storage round-trip: session containing a Thinking message survives
  `save_session` → `load_session` intact.
- Context-assembly exclusion: a Thinking message never enters
  `ContextWindow` messages while User/Assistant do.

**Shell (bun test, TDD)**
- `sessionReplay`: valid envelope → collapsed thinking row preserving
  text + durationMs; malformed content on a thinking role → system row;
  sessions without thinking roles produce output identical to today.
- View: collapsed row renders only "✻ Thought for X.Xs"; live-style
  (non-collapsed, non-empty text) row unchanged.
- Full-suite gate by failure identity against the documented pre-existing
  set; totals drift with untracked user-WIP test files and are not a gate.

**PTY smoke** (`scripts/ptySmokeInc19.py`, Inc 17 pattern): stub daemon
streams `thinking_start/delta/end` then token/finish; script asserts the
live "Thought for" line; then drives `/resume` against a stubbed
`v1/session/load` whose messages include
`{role:"thinking", content:<envelope>}` and asserts the summary line
renders while the body text never appears in the cumulative buffer.

## 6. Non-Goals

- Expand/collapse interaction on resumed rows (future increment; row
  model deliberately keeps the text to enable it cheaply).
- Redacted thinking support (no such `GenerationChunk` variant exists).
- Storage migration or versioning (blob format absorbs the new role).
- The pre-existing Tool-envelope-in-context leak (§3 observation).
- Any change to live thinking rendering or streaming behavior.

## 7. Constraints & Riders

- Preserve Brain architecture/domain/IPC/runtime/memory/provenance
  boundaries; the only domain change is the single documented enum
  variant plus required match arms surfaced by the compiler.
- No Claude/Anthropic-derived concepts anywhere.
- Commits contain ONLY explicitly-added paths; working-tree user WIP is
  never staged, reverted, or stashed; commit trailer
  `Co-Authored-By: Claude <noreply@anthropic.com>`.
- macOS cargo wrapper for every cargo invocation:
  `bash -c 'RUSTFLAGS="-C link-arg=-Wl,-rpath,/Library/Developer/CommandLineTools/Library/Frameworks" cargo …'`.
- Sole permitted cargo failure:
  `uds_security_audit_tests::test_security_path_traversal_and_invalid_identifiers`.
- Pushes to origin require explicit user authorization each time.
