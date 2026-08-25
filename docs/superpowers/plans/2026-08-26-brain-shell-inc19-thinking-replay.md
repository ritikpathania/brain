# Thinking-Block Persistence & Replay (Inc 19) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Persist every completed thinking segment as a transcript message and render a collapsed "✻ Thought for X.Xs" summary line when the session is resumed.

**Architecture:** Add `MessageRole::Thinking` to the brain-domain enum (serde `"thinking"`, zero schema change — sessions are single serde JSON blobs). The daemon's stream loop accumulates thinking deltas and persists a `thinking_block` v1 envelope at each `ThinkingEnd` with best-effort saves (the Inc 8 tool-event discipline). Context assembly excludes Thinking messages from model-bound history. The shell widens its replay union, parses the envelope tolerantly into a `collapsed` thinking row, and renders summary-only.

**Tech Stack:** Rust workspace (brain-domain, brain-services, brain-storage, brain-daemon, brain-python) + Bun/TypeScript shell (React 19 + Ink 7) + Python PTY smoke.

**Spec:** `docs/superpowers/specs/2026-08-26-brain-shell-inc19-thinking-replay-design.md`

## Global Constraints

- Preserve Brain architecture/domain/IPC/runtime/memory/provenance boundaries; the only domain change is the single documented enum variant plus required match arms.
- No Claude/Anthropic-derived concepts anywhere.
- Every commit contains ONLY explicitly-added paths (`git add <paths>`, NEVER `git add .`). The working tree holds ~1k uncommitted user-WIP files: never stage, revert, or stash them. NEVER run `git stash`.
- Commit trailer on every commit: `Co-Authored-By: Claude <noreply@anthropic.com>`.
- macOS cargo wrapper for EVERY cargo invocation:
  `bash -c 'RUSTFLAGS="-C link-arg=-Wl,-rpath,/Library/Developer/CommandLineTools/Library/Frameworks" cargo …'`
- Sole permitted cargo failure: `uds_security_audit_tests::test_security_path_traversal_and_invalid_identifiers`. Known load flake if it appears: `uds_feedback_loop_tests::shell_exec_runs_command_and_persists_standalone_turn` (rerun standalone to prove).
- Pushes to origin require explicit user authorization each time.
- Branch: `feature/brain-shell-inc19-thinking-replay` off main @ 7a306d80. Work in place; no worktrees.

---

### Task 1: Domain variant + compile riders + serde/storage proofs

**Files:**
- Modify: `crates/brain-domain/src/entities.rs:15–50` (`MessageRole` enum, `Display`, `FromStr`)
- Create: `crates/brain-domain/tests/message_role_thinking_tests.rs`
- Create: `crates/brain-storage/tests/message_role_thinking_storage_tests.rs`
- Modify: `crates/brain-python/src/api.rs:275–280` (exhaustive-match rider)
- Modify: `daemon/src/transport/uds/handlers.rs:416–421` (exhaustive-match rider)

**Interfaces:**
- Consumes: existing `MessageRole` (serde `rename_all = "lowercase"`), existing `SessionRepository::{save_session, load_session}`, `Session::new(SessionId, SessionTitle, SessionTimestamp)`, `Message::new(MessageId, MessageRole, String)`.
- Produces: `MessageRole::Thinking` — serializes as `"thinking"`, displays as `"thinking"`, parses from `"thinking"` via `FromStr`. All later tasks rely on this variant existing and round-tripping through storage blobs.

- [ ] **Step 1: Write the failing domain tests**

Create `crates/brain-domain/tests/message_role_thinking_tests.rs`, mirroring the Inc 8 precedent file `message_role_tool_tests.rs`:

```rust
//! Inc 19: the Thinking message role persists reasoning blocks into
//! session transcripts.
use brain_domain::MessageRole;
use std::str::FromStr;

#[test]
fn thinking_variant_displays_as_lowercase_thinking() {
    assert_eq!(MessageRole::Thinking.to_string(), "thinking");
}

#[test]
fn thinking_variant_serializes_and_deserializes_as_thinking() {
    let json = serde_json::to_string(&MessageRole::Thinking).unwrap();
    assert_eq!(json, r#""thinking""#);
    let back: MessageRole = serde_json::from_str(&json).unwrap();
    assert_eq!(back, MessageRole::Thinking);
}

#[test]
fn thinking_variant_parses_from_str() {
    assert_eq!(MessageRole::from_str("thinking").unwrap(), MessageRole::Thinking);
}
```

- [ ] **Step 2: Run domain tests to verify they fail**

Run: `bash -c 'RUSTFLAGS="-C link-arg=-Wl,-rpath,/Library/Developer/CommandLineTools/Library/Frameworks" cargo test -p brain-domain --test message_role_thinking_tests'`
Expected: COMPILE ERROR — `no associated item named 'Thinking' found` on `MessageRole`.

- [ ] **Step 3: Add the variant and fix every exhaustive match**

In `crates/brain-domain/src/entities.rs`, add after the `Tool` variant (line ~23):

```rust
    /// Agentic-loop tool outcome persisted as part of the transcript (Inc 8).
    Tool,
    /// Reasoning block persisted as part of the transcript (Inc 19).
    Thinking,
```

Extend the `Display` impl (line ~30):

```rust
            Self::Tool => write!(f, "tool"),
            Self::Thinking => write!(f, "thinking"),
```

Extend `FromStr` (line ~42) BEFORE the catchall arm — without this, `"thinking"` silently parses as User:

```rust
            "tool" => Self::Tool,
            "thinking" => Self::Thinking,
            _ => Self::User,
```

Fix the two other exhaustive matches the compiler will flag:

`crates/brain-python/src/api.rs:275–280`:

```rust
                match msg.role {
                    MessageRole::User => "user",
                    MessageRole::Assistant => "assistant",
                    MessageRole::System => "system",
                    MessageRole::Tool => "tool",
                    MessageRole::Thinking => "thinking",
                },
```

`daemon/src/transport/uds/handlers.rs:416–421` (the `v1/session/load` role mapper):

```rust
                                    let role_str = match m.role {
                                        brain_domain::MessageRole::User => "user",
                                        brain_domain::MessageRole::Assistant => "assistant",
                                        brain_domain::MessageRole::System => "system",
                                        brain_domain::MessageRole::Tool => "tool",
                                        brain_domain::MessageRole::Thinking => "thinking",
                                    };
```

Note: these two riders are required here solely so the workspace compiles; their behavior is exercised by later tasks. If any OTHER exhaustive match surfaces during `cargo check`, give it the same one-line arm — do not refactor surrounding code.

- [ ] **Step 4: Run domain tests + workspace check**

Run: `bash -c 'RUSTFLAGS="-C link-arg=-Wl,-rpath,/Library/Developer/CommandLineTools/Library/Frameworks" cargo test -p brain-domain --test message_role_thinking_tests && RUSTFLAGS="-C link-arg=-Wl,-rpath,/Library/Developer/CommandLineTools/Library/Frameworks" cargo check --workspace'`
Expected: 3 passed; check completes without errors.

- [ ] **Step 5: Write the failing storage round-trip test**

Create `crates/brain-storage/tests/message_role_thinking_storage_tests.rs` (imports mirror `storage_tests.rs`):

```rust
//! Inc 19: a Thinking-role message persists and reloads inside a session blob.
use brain_core::repositories::SessionRepository;
use brain_domain::{
    Message, MessageId, MessageRole, Session, SessionId, SessionTimestamp, SessionTitle,
};
use brain_storage::{SqliteStorage, TestStorage};

#[test]
fn thinking_message_survives_session_round_trip() {
    let test_store = TestStorage::new();
    let store: &SqliteStorage = test_store.storage();

    let sid = SessionId::new();
    let mut session = Session::new(
        sid.clone(),
        SessionTitle("Inc 19".to_string()),
        SessionTimestamp(1_756_200_000),
    );
    session
        .add_message(Message::new(
            MessageId::new(),
            MessageRole::User,
            "hi".to_string(),
        ))
        .unwrap();
    let envelope =
        r#"{"type":"thinking_block","v":1,"text":"checking the stream loop","duration_ms":800}"#
            .to_string();
    session
        .add_message(Message::new(
            MessageId::new(),
            MessageRole::Thinking,
            envelope.clone(),
        ))
        .unwrap();
    session
        .add_message(Message::new(
            MessageId::new(),
            MessageRole::Assistant,
            "done".to_string(),
        ))
        .unwrap();

    SessionRepository::save_session(store, &sid, &session).unwrap();

    let reloaded = SessionRepository::load_session(store, &sid)
        .unwrap()
        .expect("session must reload");
    assert_eq!(reloaded.messages.len(), 3);
    assert_eq!(reloaded.messages[1].role, MessageRole::Thinking);
    assert_eq!(reloaded.messages[1].content, envelope);
}
```

If `TestStorage::storage()` returns a different concrete type than `SqliteStorage`, drop the local type annotation on `store` and let inference decide (match how `storage_tests.rs` uses it).

- [ ] **Step 6: Run the storage test**

Run: `bash -c 'RUSTFLAGS="-C link-arg=-Wl,-rpath,/Library/Developer/CommandLineTools/Library/Frameworks" cargo test -p brain-storage --test message_role_thinking_storage_tests'`
Expected: PASS (serde already handles the new variant generically). If it FAILS on serialization, stop and investigate — nothing downstream may proceed on broken storage.

- [ ] **Step 7: Commit**

```bash
git add crates/brain-domain/src/entities.rs crates/brain-domain/tests/message_role_thinking_tests.rs crates/brain-storage/tests/message_role_thinking_storage_tests.rs crates/brain-python/src/api.rs daemon/src/transport/uds/handlers.rs
git commit -m "feat(domain): Thinking message role persists reasoning blocks

MessageRole gains a Thinking variant (serde \"thinking\") with Display
and FromStr arms, plus the two exhaustive-match riders in the python
binding and the v1/session/load mapper that the compiler requires.
Domain serde and sqlite blob round-trip proofs included."
```

(Trailer line appended: `Co-Authored-By: Claude <noreply@anthropic.com>`.)

---

### Task 2: Context-assembly exclusion

**Files:**
- Modify: `crates/brain-services/src/conversation.rs:357–361` (non-system filter)
- Create: `crates/brain-services/tests/context_thinking_exclusion_tests.rs`

**Interfaces:**
- Consumes: `ContextBuilder::build(counter: &dyn TokenCounter, budget: ContextBudget, history: &[Message], summary: Option<ConversationSummary>, retrieved_memories: Vec<MemoryDTO>) -> ContextWindow`; `ContextWindow::messages()`; `WordSpaceTokenCounter`; `ContextBudget { max_tokens, reserved_system_tokens, reserved_completion_tokens }`.
- Produces: guarantee that `MessageRole::Thinking` never enters `ContextWindow.messages()` — relied on by Task 3 (envelopes stay out of prompts).

- [ ] **Step 1: Write the failing exclusion test**

Create `crates/brain-services/tests/context_thinking_exclusion_tests.rs`:

```rust
//! Inc 19: persisted thinking blocks must never enter model-bound context.
use brain_domain::{Message, MessageId, MessageRole};
use brain_services::conversation::{ContextBudget, ContextBuilder, WordSpaceTokenCounter};

#[test]
fn thinking_messages_are_excluded_from_context_window() {
    let counter = WordSpaceTokenCounter;
    let budget = ContextBudget {
        max_tokens: 100,
        reserved_system_tokens: 5,
        reserved_completion_tokens: 5,
    };
    let history = vec![
        Message::new(MessageId::new(), MessageRole::System, "sys".to_string()),
        Message::new(MessageId::new(), MessageRole::User, "question".to_string()),
        Message::new(
            MessageId::new(),
            MessageRole::Thinking,
            r#"{"type":"thinking_block","v":1,"text":"hidden","duration_ms":10}"#.to_string(),
        ),
        Message::new(MessageId::new(), MessageRole::Assistant, "answer".to_string()),
    ];

    let window = ContextBuilder::build(&counter, budget, &history, None, vec![]);
    let roles: Vec<MessageRole> = window.messages().iter().map(|m| m.role).collect();
    assert_eq!(
        roles,
        vec![MessageRole::System, MessageRole::User, MessageRole::Assistant],
        "Thinking envelopes must not reach generation input"
    );
}
```

- [ ] **Step 2: Run it to verify it fails**

Run: `bash -c 'RUSTFLAGS="-C link-arg=-Wl,-rpath,/Library/Developer/CommandLineTools/Library/Frameworks" cargo test -p brain-services --test context_thinking_exclusion_tests'`
Expected: FAIL — received vec has 4 entries including `Thinking` (today the filter only excludes System).

- [ ] **Step 3: Apply the guard**

In `crates/brain-services/src/conversation.rs`, replace lines 357–361:

```rust
        let non_system_messages: Vec<Message> = history
            .iter()
            .filter(|m| !matches!(m.role, brain_domain::MessageRole::System))
            .cloned()
            .collect();
```

with:

```rust
        // Inc 19: Thinking messages carry persisted reasoning envelopes and
        // are transcript-only — never generation input.
        let non_system_messages: Vec<Message> = history
            .iter()
            .filter(|m| {
                !matches!(
                    m.role,
                    brain_domain::MessageRole::System | brain_domain::MessageRole::Thinking
                )
            })
            .cloned()
            .collect();
```

(The variable keeps its name to minimize churn; the comment carries the semantics.)

- [ ] **Step 4: Run the test to verify it passes**

Run: `bash -c 'RUSTFLAGS="-C link-arg=-Wl,-rpath,/Library/Developer/CommandLineTools/Library/Frameworks" cargo test -p brain-services --test context_thinking_exclusion_tests && RUSTFLAGS="-C link-arg=-Wl,-rpath,/Library/Developer/CommandLineTools/Library/Frameworks" cargo test -p brain-services --test conversation_tests'`
Expected: exclusion test passes AND the pre-existing conversation suite stays green (the filter change is strictly narrowing for pre-existing data, which contains no Thinking messages).

- [ ] **Step 5: Commit**

```bash
git add crates/brain-services/src/conversation.rs crates/brain-services/tests/context_thinking_exclusion_tests.rs
git commit -m "feat(services): keep persisted thinking blocks out of generation input

Context assembly excluded only System messages; Thinking-role
transcript envelopes would have ridden the same set into prompts and
budget accounting. Narrow the filter and lock it with a test."
```

(Trailer appended.)

---

### Task 3: Stream-loop persistence (daemon)

**Files:**
- Modify: `daemon/src/transport/uds/handlers.rs:2182` (accumulator), `:2232–2272` (three thinking arms)

**Interfaces:**
- Consumes: `MessageRole::Thinking` (Task 1); the existing tool-event persistence idiom at `handlers.rs:2479–2494` (`add_message` + best-effort `save_session` + log-and-continue).
- Produces: sessions whose blobs contain `thinking_block` envelopes between their tool events and assistant message — consumed by Task 4's replay parser via `v1/session/load`. Envelope schema (exact bytes matter to Task 4): `{"type":"thinking_block","v":1,"text":<string>,"duration_ms":<number>}`.

No dedicated Rust unit test exists for the stream loop (spec §5 assigns end-to-end proof to Task 5's smoke); verification here is compile + review against the exact code below.

- [ ] **Step 1: Declare the accumulator**

At `handlers.rs:2182`, beside `let mut accumulated_response = String::new();`, add:

```rust
        let mut accumulated_response = String::new();
        let mut thinking_text = String::new();
```

- [ ] **Step 2: Extend the three thinking arms**

In the `GenerationChunk` match (~:2232), each arm changes minimally. `ThinkingStart` clears the accumulator (existing frame emission unchanged):

```rust
                                        brain_core::model::GenerationChunk::ThinkingStart => {
                                            thinking_started_at = Some(Instant::now());
                                            thinking_text.clear();
                                            let packet = serde_json::json!({
```

(rest of the arm unchanged.)

`ThinkingDelta` appends before building the packet — borrow ends before `text` moves into `json!`:

```rust
                                        brain_core::model::GenerationChunk::ThinkingDelta { text } => {
                                            thinking_text.push_str(&text);
                                            let packet = serde_json::json!({
                                                "type": "thinking_delta",
```

(rest of the arm unchanged.)

`ThinkingEnd` hoists the measured duration so the frame and the envelope carry the SAME value, then persists exactly like tool events do:

```rust
                                        brain_core::model::GenerationChunk::ThinkingEnd => {
                                            // Daemon-measured thinking duration: the
                                            // shell renders "Thought for X.Xs" from it.
                                            let measured_ms = thinking_started_at
                                                .map(|t| t.elapsed().as_millis() as u64)
                                                .unwrap_or(0);
                                            let packet = serde_json::json!({
                                                "type": "thinking_end",
                                                "duration_ms": measured_ms,
                                                "generation_id": generation_id,
                                                "session_id": session_id_str,
                                                "sequence": seq,
                                                "status": "in_progress"
                                            });
                                            let mut j = serde_json::to_string(&packet)?;
                                            j.push('\n');
                                            writer.write_all(j.as_bytes()).await?;
                                            writer.flush().await?;
                                            // Inc 19: persist the completed segment as a
                                            // transcript message (best-effort, exactly like
                                            // tool events — persistence never blocks
                                            // generation).
                                            if !thinking_text.is_empty() {
                                                let envelope = serde_json::json!({
                                                    "type": "thinking_block",
                                                    "v": 1,
                                                    "text": thinking_text,
                                                    "duration_ms": measured_ms
                                                });
                                                let _ = session_aggregate.add_message(
                                                    brain_domain::Message::new(
                                                        brain_domain::MessageId::new(),
                                                        brain_domain::MessageRole::Thinking,
                                                        envelope.to_string(),
                                                    ),
                                                );
                                                if let Err(e) = storage.save_session(
                                                    &parsed_session_id,
                                                    &session_aggregate,
                                                ) {
                                                    tracing::warn!(
                                                        target: "brain::transport::uds",
                                                        error = %e,
                                                        "thinking block persistence failed; continuing"
                                                    );
                                                }
                                                thinking_text = String::new();
                                            }
                                        }
```

Replace the old arm body entirely (it previously computed `duration_ms` inline inside `json!`). Keep the original comment lines about daemon-measured duration.

- [ ] **Step 3: Compile and review**

Run: `bash -c 'RUSTFLAGS="-C link-arg=-Wl,-rpath,/Library/Developer/CommandLineTools/Library/Frameworks" cargo check -p brain-daemon && RUSTFLAGS="-C link-arg=-Wl,-rpath,/Library/Developer/CommandLineTools/Library/Frameworks" cargo test -p brain-daemon --test uds_security_audit_tests'`
Expected: check clean; audit suite shows its usual single permitted failure (`test_security_path_traversal_and_invalid_identifiers`) and no NEW failures.

- [ ] **Step 4: Commit**

```bash
git add daemon/src/transport/uds/handlers.rs
git commit -m "feat(daemon): persist completed thinking segments at ThinkingEnd

The stream loop drops thinking text today — only TextDelta feeds the
persisted assistant message. Accumulate deltas per segment and append a
thinking_block v1 envelope message at each ThinkingEnd with the same
best-effort save_session discipline as tool events, so resumed sessions
can replay where thinking happened. Empty segments persist nothing;
open segments at cancel time drop, consistent with Invariant 4."
```

(Trailer appended.)

---

### Task 4: Shell replay pipeline

**Files:**
- Modify: `packages/brain-shell/src/client/BrainBackendClient.ts:199–205` (`BrainMessage.role`)
- Modify: `packages/brain-shell/src/contracts/messages.ts:198` (thinking row variant)
- Modify: `packages/brain-shell/src/state/sessionReplay.ts` (parser + dispatch)
- Modify: `packages/brain-shell/src/ui/transcript/MessageRow.tsx:26–42` (`ThinkingRowView`)
- Test: `packages/brain-shell/src/test/state/sessionReplay.test.ts` (extend)
- Test: `packages/brain-shell/src/test/ui/transcript/thinkingRowView.test.tsx` (create)

**Interfaces:**
- Consumes: Task 3's envelope over `v1/session/load` as `{role:'thinking', content:<envelope JSON>}`.
- Produces: `sessionToRows` maps valid envelopes to `{ kind:'thinking', id, text, durationMs?, collapsed:true }`; `TranscriptRow` thinking variant gains optional `collapsed?: boolean`. Task 5's smoke relies on both.

- [ ] **Step 1: Write the failing replay tests**

Append to `packages/brain-shell/src/test/state/sessionReplay.test.ts` (reuse its existing `session(...)` fixture helper):

```typescript
describe('sessionToRows: persisted thinking blocks (Inc 19)', () => {
  const thinkingEnvelope = (over: Record<string, unknown>) =>
    JSON.stringify({ type: 'thinking_block', v: 1, ...over });

  test('valid envelope becomes a collapsed thinking row keeping text and duration', () => {
    const rows = sessionToRows(
      session([
        { id: 'u1', role: 'user', content: 'hello' },
        {
          id: 't1',
          role: 'thinking',
          content: thinkingEnvelope({ text: 'secret reasoning', duration_ms: 800 }),
        },
        { id: 'a1', role: 'assistant', content: 'answer' },
      ]),
    );
    expect(rows).toEqual([
      { kind: 'user', id: 'u1', text: 'hello' },
      { kind: 'thinking', id: 't1', text: 'secret reasoning', durationMs: 800, collapsed: true },
      { kind: 'assistant', id: 'a1', markdown: 'answer' },
    ]);
  });

  test('envelope without duration yields a collapsed row without durationMs', () => {
    const rows = sessionToRows(
      session([
        { id: 't2', role: 'thinking', content: thinkingEnvelope({ text: 'bare' }) },
      ]),
    );
    expect(rows).toEqual([
      { kind: 'thinking', id: 't2', text: 'bare', collapsed: true },
    ]);
  });

  test('malformed thinking content falls back to a visible system row', () => {
    const raw = JSON.stringify({ type: 'other', v: 1 });
    const rows = sessionToRows(
      session([
        { id: 'x1', role: 'thinking', content: 'not json' },
        { id: 'x2', role: 'thinking', content: raw },
      ]),
    );
    expect(rows).toEqual([
      { kind: 'system', id: 'x1', text: 'not json' },
      { kind: 'system', id: 'x2', text: raw },
    ]);
  });
});
```

- [ ] **Step 2: Run replay tests to verify they fail**

Run: `cd /Users/ritikpathania/Developer/PyCharm/brain/packages/brain-shell && bun test src/test/state/sessionReplay.test.ts`
Expected: FAIL — `'thinking'` is not assignable to the `BrainMessage.role` union (type error surfaced by bun) and rows fall back to system.

- [ ] **Step 3: Widen the wire union and implement the parser + mapping**

`packages/brain-shell/src/client/BrainBackendClient.ts:199–205` — role union becomes:

```typescript
export interface BrainMessage {
  id: string;
  role: 'user' | 'assistant' | 'system' | 'tool' | 'thinking';
  content: string;
  timestampMs?: number;
}
```

`packages/brain-shell/src/state/sessionReplay.ts` — add below `toolCardFromContent`:

```typescript
/** Wire shape of the persisted Inc 19 thinking_block envelope. */
interface ThinkingEnvelope {
  type?: unknown;
  v?: unknown;
  text?: unknown;
  duration_ms?: unknown;
}

/**
 * Parse a persisted thinking_block envelope. Returns undefined when the
 * content isn't a v1 envelope — the caller then falls back to a plain
 * system row so malformed history stays visible.
 */
function thinkingFromContent(content: string): { text: string; durationMs?: number } | undefined {
  let env: ThinkingEnvelope;
  try {
    const parsed: unknown = JSON.parse(content);
    if (typeof parsed !== 'object' || parsed === null || Array.isArray(parsed)) return undefined;
    env = parsed as ThinkingEnvelope;
  } catch {
    return undefined;
  }
  if (!(env.type === 'thinking_block' && env.v === 1 && typeof env.text === 'string'))
    return undefined;
  return typeof env.duration_ms === 'number'
    ? { text: env.text, durationMs: env.duration_ms }
    : { text: env.text };
}
```

And insert the dispatch arm in `sessionToRows` between the `role === 'tool'` branch and the final system fallback:

```typescript
    if (m.role === 'thinking') {
      const th = thinkingFromContent(text);
      if (th !== undefined) {
        return [
          {
            kind: 'thinking' as const,
            id,
            text: th.text,
            ...(th.durationMs !== undefined ? { durationMs: th.durationMs } : {}),
            collapsed: true,
          },
        ];
      }
    }
```

- [ ] **Step 4: Run replay tests to verify they pass**

Run: `bun test src/test/state/sessionReplay.test.ts`
Expected: ALL pass, including the pre-existing Inc 9 describe block.

- [ ] **Step 5: Write the failing view test, then update contract + view**

Create `packages/brain-shell/src/test/ui/transcript/thinkingRowView.test.tsx` (walker pattern mirrors `src/test/ui/overlays/permissionDialogView.test.tsx`):

```tsx
import { describe, expect, test } from 'bun:test';
import * as React from 'react';
import { PALETTES } from '../../../state/palettes.js';
import { ThinkingRowView } from '../../../ui/transcript/MessageRow.js';

function textOf(el: React.ReactElement): string {
  const walk = (node: React.ReactNode): string => {
    if (node === null || node === undefined || typeof node === 'boolean') return '';
    if (typeof node === 'string' || typeof node === 'number') return String(node);
    if (Array.isArray(node)) return node.map(walk).join('');
    const el2 = node as React.ReactElement;
    if (el2.props && typeof el2.props === 'object' && 'children' in el2.props) {
      return walk((el2.props as { children?: React.ReactNode }).children);
    }
    return '';
  };
  return walk(el);
}

const row = (over: Record<string, unknown>) =>
  ({ kind: 'thinking', id: 't', text: 'hidden reasoning', ...over }) as React.ComponentProps<
    typeof ThinkingRowView
  >['row'];

describe('ThinkingRowView collapse (Inc 19)', () => {
  test('live-style row renders summary plus italic body', () => {
    const out = textOf(
      <ThinkingRowView row={row({ durationMs: 3200 })} tokens={PALETTES.dark} />,
    );
    expect(out).toContain('Thought for 3.2s');
    expect(out).toContain('hidden reasoning');
  });

  test('collapsed replay row renders ONLY the summary line', () => {
    const out = textOf(
      <ThinkingRowView
        row={row({ durationMs: 800, collapsed: true })}
        tokens={PALETTES.dark}
      />,
    );
    expect(out).toContain('✻ Thought for 0.8s');
    expect(out).not.toContain('hidden reasoning');
  });

  test('collapsed row without duration renders nothing but stays mounted', () => {
    const out = textOf(
      <ThinkingRowView row={row({ collapsed: true })} tokens={PALETTES.dark} />,
    );
    expect(out).toBe('');
  });
});
```

Run: `bun test src/test/ui/transcript/thinkingRowView.test.tsx`
Expected: FAIL — TS rejects `collapsed` (not yet on the contract) and/or the body still renders.

Now apply both edits.

`packages/brain-shell/src/contracts/messages.ts:198`:

```typescript
  | { kind: 'thinking'; id: string; text: string; durationMs?: number; collapsed?: boolean }
```

`packages/brain-shell/src/ui/transcript/MessageRow.tsx` — `ThinkingRowView` body line gets the guard (summary line untouched):

```tsx
export function ThinkingRowView(props: {
  row: Extract<TranscriptRow, { kind: 'thinking' }>;
  tokens: BrainTokens;
}): React.ReactElement {
  const { row, tokens } = props;
  const showBody = !row.collapsed && row.text.trim().length > 0;
  return (
    <Box flexDirection="column">
      {row.durationMs !== undefined ? (
        <Text dimColor>✻ Thought for {(row.durationMs / 1000).toFixed(1)}s</Text>
      ) : null}
      {showBody ? (
        <Text dimColor italic color={tokens.subtle}>
          {'✻ '}
          {row.text}
        </Text>
      ) : null}
    </Box>
  );
}
```

Run: `bun test src/test/ui/transcript/thinkingRowView.test.tsx src/test/state/sessionReplay.test.ts src/test/state/sessionControllerThinking.test.ts`
Expected: all green, including the pre-existing controller thinking lifecycle suite (live path untouched).

- [ ] **Step 6: Full shell suite by failure identity**

Run: `bun test`
Expected: totals drift freely (untracked user-WIP files pollute them); the failure set must be EXACTLY the documented pre-existing identities: visualCellParity ×2, sessionSemanticIntegration, brainMemoryIntegration Gate 5, brainTurnTransformer Scenario 8 — no new failures.

- [ ] **Step 7: Commit**

```bash
git add packages/brain-shell/src/client/BrainBackendClient.ts packages/brain-shell/src/contracts/messages.ts packages/brain-shell/src/state/sessionReplay.ts packages/brain-shell/src/ui/transcript/MessageRow.tsx packages/brain-shell/src/test/state/sessionReplay.test.ts packages/brain-shell/src/test/ui/transcript/thinkingRowView.test.tsx
git commit -m "feat(brain-shell): replay persisted thinking as collapsed summary rows

v1/session/load now delivers thinking-role envelope messages; parse
them tolerantly (strict type/v gate, malformed falls back to visible
system rows) into thinking rows flagged collapsed, and render only the
'Thought for X.Xs' summary line while keeping full text in the row
model. Live rendering is unchanged."
```

(Trailer appended.)

---

### Task 5: PTY smoke — live thinking + resume replay

**Files:**
- Create: `scripts/ptySmokeInc19.py`

**Interfaces:**
- Consumes: everything above, end to end. Stub daemon serves `v1/session/create`, `v1/generation/stream` (with thinking frames), `session/list`, `v1/session/load`.
- Produces: executable proof that turn 1 shows the live/frozen "Thought for" line and `/resume` shows ONLY summary lines (body text absent from the cumulative buffer).

Model the script on `scripts/ptySmokeInc17.py` (same skeleton: ROWS/COLS 30×100, ANSI-stripping cumulative buffer, `expect()` helper, teardown ctrl+c/SIGKILL). Key differences:

- `SOCK = "/tmp/brain-inc19-smoke.sock"`; no config file needed.
- Stub handlers:
  - `v1/session/create` → `{"id": rid, "status": "success", "body": {"session_id": "stub-s19"}}`
  - `v1/generation/stream` → frames: `stream_start(seq 0)` → sleep 0.2 → `thinking_start(seq 1)` → sleep 0.2 → `thinking_delta(seq 2, thinking/text "inc19-live-thinking-marker")` → sleep 0.2 → `thinking_end(seq 3, duration_ms 800)` → `token(seq 4, "Live answer.")` → `finished(seq 5, status completed)`
  - module-level constants computed inside each handler at request time: `NOW_MS = int(time.time() * 1000)`, `NOW_S = int(time.time())`
  - `session/list` → `{"id": rid, "status": "success", "body": {"sessions": [{"sessionId": "stub-s19", "title": "S19", "updatedAtMs": NOW_MS, "createdAtMs": NOW_MS, "archived": False}], "total": 1}}`
  - `v1/session/load` → body:
    ```python
    {"session": {
        "id": "stub-s19", "title": "S19",
        "created_at_ms": NOW_MS, "updated_at_ms": NOW_MS,
        "archived": False, "pinned": False,
        "messages": [
            {"id": "m1", "role": "user", "content": "hello", "timestamp": NOW_S},
            {"id": "m2", "role": "thinking", "timestamp": NOW_S,
             "content": json.dumps({"type": "thinking_block", "v": 1,
                                    "text": "SECRET-REPLAY-BODY-MARKER",
                                    "duration_ms": 800})},
            {"id": "m3", "role": "assistant", "content": "Replayed answer.", "timestamp": NOW_S},
        ]}}
    ```
- Flow assertions (each via `expect`, exit nonzero on any FAIL):
  1. `welcome-wordmark`: "◆ BRAIN"
  2. `launch-prompt`: "❯"
  3. Type `think hard` + `\r`, then `frozen-thought-summary`: "Thought for" appears (turn froze)
  4. `live-answer`: "Live answer."
  5. Type `/resume` + `\r`, pump 0.5, then `\r` (picker index 0) — then:
  6. `resume-notice`: "Resumed"
  7. `replay-answer`: "Replayed answer."
  8. `replay-summary-line`: "✻ Thought for 0.8s"
  9. Cumulative-buffer absence checks (sound once flow completes):
     - `replay-body-hidden`: `"SECRET-REPLAY-BODY-MARKER" not in clean(buf)` (the resume replay never rendered it)
     - `live-thinking-body-visible-once`: `"inc19-live-thinking-marker" in clean(buf)` — proves assertion 3 exercised a REAL thinking stream rather than a vacuous pass (live bodies are allowed on screen during turn 1; only replayed ones are suppressed)

- [ ] **Step 1: Write the script** per the skeleton and stub contract above.
- [ ] **Step 2: Run it** — `python3 scripts/ptySmokeInc19.py` from the repo root. Expected: all 9 PASS, exit 0. If `resume-notice` never lands, first suspect the picker interaction timing (raise pumps to 1.0s) before suspecting the replay mapping.
- [ ] **Step 3: Commit**

```bash
git add scripts/ptySmokeInc19.py
git commit -m "test(smoke): Inc 19 wire-level thinking persistence & replay proof

Stub daemon streams real thinking frames on turn 1 (body visible live),
then /resume replays a stored thinking_block envelope: the summary line
renders and the body text never appears post-resume."
```

(Trailer appended.)

---

## Finishing

After Task 5: run the full verification battery, then use superpowers:finishing-a-development-branch (menu options 1/2/3 verbatim):

1. Shell: `bun test` from `packages/brain-shell` — failure identities match the documented set; new tests green.
2. Rust: wrapper `cargo test --workspace` — aggregate must be 1 failed = the permitted audit test (known flake procedure if `uds_feedback_loop_tests::shell_exec…` appears).
3. tsc: `bunx tsc --noEmit` from `packages/brain-shell` — no errors referencing touched files beyond the documented ambient class (TS2591/TS2307).
4. Vendor scan: diff-scoped grep vs main for vendor markers — clean on added lines.
