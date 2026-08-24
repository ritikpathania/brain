# Increment 7 — Provider-Visible Tool Definitions Design

**Status:** Approved design, awaiting implementation plan.
**Extends:** `docs/superpowers/specs/2026-08-24-brain-shell-inc6-agentic-feedback-loop-design.md` (whose §6 non-goal "advertising `ToolDefinition`s to providers" this increment closes).
**Date:** 2026-08-24

## 1. Context and Goal

Brain's provider interface has carried an unused contract since its
inception: `GenerationRequest.tools: Vec<ToolDefinition>` (`crates/brain-core/src/model.rs:167`),
typed as `ToolDefinition { name, description, parameters }` (`model.rs:32`),
referenced by nothing — the daemon hardcodes `tools: Vec::new()`. On the other
side of the core boundary, the daemon's `ToolStack` (`daemon/src/tools/mod.rs`)
registers executable tools whose `ToolMetadata` describes capabilities but
carries no input schema.

Increment 7 builds the smallest Brain-owned bridge between the two: registered
tools are advertised to the model provider on every pass of the Inc 6 agentic
loop, without changing a single execution semantic. The model learns what it
may call; permission remains the only thing that lets it.

**Goal:** every `stream_generation` request for a tool-capable model carries
the registry-derived `Vec<ToolDefinition>`; requests for models without tool
support remain byte-identical to today (`tools: []`).

## 2. Decisions (settled during brainstorming)

| Question | Decision |
|---|---|
| Where does a tool's `parameters` JSON Schema come from? | One additive serde-default field on `ToolMetadata`: `input_schema: Option<serde_json::Value>`. Each tool owns its real schema next to `execute()`; `None` advertises a permissive `{"type":"object"}`. |
| Gate on model capability? | Yes — advertise only when the resolved `ModelDescriptor.supports_tools` is true; otherwise exactly today's empty vec. |
| Bridge shape? | Approach A: a pure converter function over an injected registry in `daemon/src/tools`, plus a one-line wiring diff at the existing call site. No new traits, no new API surface in brain-core beyond the field. |

## 3. Architecture

```
brain-core/extensibility.rs     ToolMetadata += input_schema: Option<serde_json::Value>
daemon/src/tools/mod.rs         fn definitions_from(registry: &dyn ToolRegistry) -> Vec<ToolDefinition>   (pure)
                                fn advertised_definitions() -> Vec<ToolDefinition>   (wrapper over tool_stack())
daemon/…/handlers.rs            pre-loop: defs = if supports_tools { advertised_definitions() } else { vec![] }
                                per-pass: tools: defs.clone()
```

### Layer impact

| Layer | Change |
|---|---|
| `crates/brain-core/src/extensibility.rs` | **Only brain-core change**: one `#[serde(default)] pub input_schema: Option<serde_json::Value>` field on `ToolMetadata`. No trait, signature, or behavioral change. |
| `daemon/src/tools/bash_tool.rs` | Metadata literal gains the real command schema. |
| `crates/brain-core/tests/contract_tests.rs`, `crates/brain-tools/tests/tool_tests.rs` | Fixture literals gain `input_schema: None`. |
| `daemon/src/tools/mod.rs` | Pure converter + wrapper + unit tests. |
| `daemon/src/transport/uds/handlers.rs` | Capability gate before the rounds loop; `tools: defs.clone()` replaces `tools: Vec::new()` (:1947). |
| `crates/brain-services/src/model/mock.rs` | Test-infrastructure addition only (§4.5); inert for all existing behavior. |
| brain-tools, executor, permission manager | None. |
| brain-shell, wire protocol, frames, sequences | **None.** |

Invariants carried forward: definitions are built once per turn outside the
rounds loop and ride every pass identically (providers are stateless);
advertisement never implies authorization — the Inc 5 permission round trip
stays the sole execution gate; unknown tool names emitted by a provider keep
Inc 5's failed-result path untouched.

## 4. Components

### 4.1 The metadata field

```rust
/// Optional JSON Schema describing the tool's input object. Advertised to
/// providers verbatim as `ToolDefinition.parameters`; `None` advertises a
/// permissive `{"type":"object"}`.
#[serde(default)]
pub input_schema: Option<serde_json::Value>,
```

Deserialization of old persisted metadata keeps working via `#[serde(default)]`.
The three struct-literal construction sites are updated at compile time
(`bash_tool.rs:81` real schema; both test fixtures `None`).

### 4.2 BashTool's schema

```json
{"type":"object",
 "properties":{"command":{"type":"string","description":"Shell command to execute."}},
 "required":["command"]}
```

Mirrors `execute()`'s actual contract (non-empty string `command`, else `Err`).

### 4.3 The converter (pure, infallible)

```rust
fn definitions_from(registry: &dyn brain_core::extensibility::ToolRegistry)
    -> Vec<brain_core::model::ToolDefinition>
```

Maps `registry.list_tools()` — already name-sorted by `ToolRegistryImpl`'s
BTreeMap — to `ToolDefinition { name, description, parameters:
meta.input_schema.clone().unwrap_or(json!({"type":"object"})) }`.

```rust
fn advertised_definitions() -> Vec<brain_core::model::ToolDefinition>
```
Thin wrapper calling `definitions_from(&tool_stack().registry)`. Registry lock
poisoning panics identically to today's execute path; empty registry yields an
empty vec.

### 4.4 Wiring

After model resolution, before `'rounds:`:

```rust
let advertised_tools = if resolved_model_desc.supports_tools {
    crate::tools::advertised_definitions()
} else {
    Vec::new()
};
```

Each pass builds `gen_request` with `tools: advertised_tools.clone()`.

### 4.5 Mock recorder (test infrastructure only)

`DeterministicMockProvider` gains `last_request_tools(&self) -> Vec<String>`
(backed by `Arc<Mutex<Vec<String>>>`, recorded at `stream_generation` entry,
same pattern as the scripted queue). Existing mock behavior — scripting,
sentinels, defaults — is untouched; the recorder only observes.

## 5. Error Handling

| Event | Behavior |
|---|---|
| Tool with no `input_schema` | Advertised with loose `{"type":"object"}` — never omitted. |
| Empty / unregistered tool set | Empty vec rides the request; harmless. |
| Poisoned registry lock | Panics, same risk profile as the existing execute path; no new surface. |
| Model emits a call for an unadvertised name | Inc 5 behavior unchanged: lookup miss → failed tool result fed back by the Inc 6 loop. |
| `supports_tools == false` | Request byte-identical to today (`tools: []`). |

## 6. Non-Goals

- Any execution-semantics change (executor, timeout, cancellation, permission flow).
- Any brain-shell production change or new wire frame/field.
- Dynamic tool registration/removal mid-turn; per-session or per-permission advertisement filtering.
- Validating model-emitted inputs against schemas beyond BashTool's existing execute contract.
- Provider-specific wire mapping (each future provider maps `ToolDefinition` itself).
- Persisting tool events into session history.
- Resolving the security-audit contradiction (documented separately).

## 7. Testing Strategy

TDD throughout; red-green-commit per task.

1. **Core/tool units** — fixture literals compile with `input_schema: None`;
   BashTool metadata test pins the command schema (`required == ["command"]`,
   string type).
2. **Converter units** (daemon lib): inject a fresh `ToolRegistryImpl` with two
   fake tools → assert name-sorted order, description mapping, real-schema
   pass-through, `None → {"type":"object"}` fallback, empty registry → empty vec.
3. **Gateway-boundary proof** (brain-services, in-process): record two
   definitions, stream through `ModelGateway.stream_generation`, assert
   `last_request_tools()` returns them intact.
4. **Daemon e2e regression** — no new suite can observe the spawned daemon's
   provider; safety = every existing UDS suite green *unchanged* with tools now
   populated (mock ignores them), plus review of the one-line call-site diff.
5. **Gates** — bun baseline unchanged (231 pass / 5 documented fails), canonical
   build gate, diff-scoped added-line vendor scan, inc6 PTY smoke rerun exit 0
   as regression only (nothing user-visible ⇒ no new smoke/fixtures).
6. Full `cargo test -p brain-services -p brain-daemon` on merged main before
   finishing.

## 8. Project Constraints Carried Forward

- Preserve Brain's architecture, domain model, IPC contracts, runtime, memory,
  retrieval, graph, provenance, agents, adapter boundaries.
- No Claude/Anthropic models, APIs, authentication, pricing, billing, or LLM-
  vendor product concepts.
- Stack unchanged: Bun + React 19 + Ink 7 + yoga-layout shell; Rust daemon.
- Every commit contains only explicitly-added paths (`git add <paths>`);
  commit trailer `Co-Authored-By: Claude <noreply@anthropic.com>`.
- macOS builds need
  `RUSTFLAGS="-C link-arg=-Wl,-rpath,/Library/Developer/CommandLineTools/Library/Frameworks"`.
