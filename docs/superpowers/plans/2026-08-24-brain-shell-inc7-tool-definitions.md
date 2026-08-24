# Provider-Visible Tool Definitions (Inc 7) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Advertise the daemon's registered tools as `ToolDefinition`s on every `stream_generation` request for tool-capable models, without changing any execution semantic.

**Architecture:** One additive serde-default field (`input_schema`) on brain-core's `ToolMetadata`; a pure converter over an injected registry in `daemon/src/tools/mod.rs` (wrapper reads the global `ToolStack`); a capability gate before Inc 6's rounds loop clones the vec into every pass's `GenerationRequest.tools`. Mock gains an observe-only recorder so the gateway boundary is provable in-process.

**Tech Stack:** Rust (brain-core, brain-tools, brain-daemon crates), tokio, serde_json; Bun/Ink shell untouched.

**Spec:** `docs/superpowers/specs/2026-08-24-brain-shell-inc7-tool-definitions-design.md`

## Global Constraints

- Work happens on branch `feature/brain-shell-inc7-tool-definitions` (already created from main @ e579c685).
- Every cargo invocation on this Mac needs the rpath wrapper:
  `bash -c 'cd /Users/ritikpathania/Developer/PyCharm/brain && RUSTFLAGS="-C link-arg=-Wl,-rpath,/Library/Developer/CommandLineTools/Library/Frameworks" cargo test ...'`
- The daemon package is named **`brain-daemon`**, not `daemon`.
- The working tree carries ~1k files of pre-existing user WIP (modified/deleted/untracked). NEVER stage anything except explicitly-named paths; NEVER discard Cargo.lock or checkout paths wholesale.
- Commits: explicit-path `git add <paths>` only; trailer `Co-Authored-By: Claude <noreply@anthropic.com>`.
- Known-harmless noise: `error: daemon terminated` around git ops; CRLF warnings on fixtures.
- Baselines that must not move: shell suite 231 pass / 5 documented fails (MemoryContextTransformer drift); daemon `uds_security_audit_tests::test_security_path_traversal_and_invalid_identifiers` fails pre-existing (documented, out of scope).
- No Claude/Anthropic/vendor concepts in committed source; per-increment scan greps only ADDED lines:
  `git diff <spec-commit>..HEAD -- crates daemon packages scripts | grep '^+' | grep -icE "anthropic|api\.anthropic|claude"` → expect `0`.
- Advertisement never authorizes execution: permission round trip stays the sole gate.

---

### Task 1: `input_schema` field on `ToolMetadata` + fixture compile fixes

**Files:**
- Modify: `crates/brain-core/src/extensibility.rs:290-313` (add one field)
- Test: `crates/brain-core/tests/contract_tests.rs:43-56` (deser-default test + fixture literal)
- Modify: `crates/brain-tools/tests/tool_tests.rs:22-33` (fixture literal)
- Modify: `daemon/src/tools/bash_tool.rs:81-94` (literal gets `input_schema: None` placeholder; real schema lands in Task 2)

**Interfaces:**
- Consumes: existing `ToolMetadata` struct (no Default derive).
- Produces: `pub input_schema: Option<serde_json::Value>` on `ToolMetadata`, `#[serde(default)]` — every later task reads this field.

- [ ] **Step 1: Write the failing test + break the literals**

In `crates/brain-core/tests/contract_tests.rs`, append after `test_trait_object_compilation`:

```rust
#[test]
fn metadata_without_input_schema_deserializes_to_none() {
    // Old persisted metadata (pre-Inc 7 JSON) must keep loading; the field
    // is serde-defaulted so advertisement degrades to the loose schema.
    let meta: ToolMetadata = serde_json::from_str(
        r#"{
            "name": "legacy",
            "description": "d",
            "usage": "u",
            "version": "0",
            "author": "a",
            "required_permissions": [],
            "execution_policy": { "timeout_ms": 100 },
            "supports_streaming": false,
            "is_idempotent": true,
            "causes_side_effects": false
        }"#,
    )
    .expect("legacy metadata parses");
    assert!(meta.input_schema.is_none());
}
```

Also append `input_schema: None,` as the LAST field of BOTH existing literals:

- `contract_tests.rs` in `test_trait_object_compilation`: after `causes_side_effects: false,` add `input_schema: None,`
- `tool_tests.rs` in `MockTool::new`: after `causes_side_effects: false,` add `input_schema: None,`
- `daemon/src/tools/bash_tool.rs` in `BashTool::meta()`: after `causes_side_effects: true,` add `input_schema: None,`

- [ ] **Step 2: Run to verify it fails**

Run: `bash -c 'cd /Users/ritikpathania/Developer/PyCharm/brain && RUSTFLAGS="-C link-arg=-Wl,-rpath,/Library/Developer/CommandLineTools/Library/Frameworks" cargo test -p brain-core --test contract_tests 2>&1 | tail -5'`
Expected: FAIL — compile error `no field 'input_schema'` (red-by-typecheck is the correct red for a field addition).

- [ ] **Step 3: Add the field**

In `crates/brain-core/src/extensibility.rs`, inside `pub struct ToolMetadata`, after the `causes_side_effects` field:

```rust
    /// Indicates whether calling this tool alters external state.
    pub causes_side_effects: bool,
    /// Optional JSON Schema describing the tool's input object. Advertised
    /// to providers verbatim as `ToolDefinition.parameters`; `None`
    /// advertises a permissive `{"type":"object"}`.
    #[serde(default)]
    pub input_schema: Option<serde_json::Value>,
```

- [ ] **Step 4: Run to verify it passes**

Run: `bash -c 'cd /Users/ritikpathania/Developer/PyCharm/brain && RUSTFLAGS="-C link-arg=-Wl,-rpath,/Library/Developer/CommandLineTools/Library/Frameworks" cargo test -p brain-core --test contract_tests 2>&1 | grep -E "^test result" && RUSTFLAGS="-C link-arg=-Wl,-rpath,/Library/Developer/CommandLineTools/Library/Frameworks" cargo test -p brain-tools --lib 2>&1 | grep -E "^test result"'`
Expected: contract_tests PASS (including the new deser test); brain-tools PASS.

- [ ] **Step 5: Verify the whole workspace slice still builds and commit**

Run: `bash -c 'cd /Users/ritikpathania/Developer/PyCharm/brain && RUSTFLAGS="-C link-arg=-Wl,-rpath,/Library/Developer/CommandLineTools/Library/Frameworks" cargo test -p brain-daemon --lib 2>&1 | grep -E "^test result"'`
Expected: 36 passed / 0 failed (the bash_tool literal now carries `input_schema: None`).

```bash
git -C /Users/ritikpathania/Developer/PyCharm/brain add crates/brain-core/src/extensibility.rs crates/brain-core/tests/contract_tests.rs crates/brain-tools/tests/tool_tests.rs daemon/src/tools/bash_tool.rs
git -C /Users/ritikpathania/Developer/PyCharm/brain commit -m "feat(core): optional input_schema field on ToolMetadata

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### Task 2: BashTool advertises its real command schema

**Files:**
- Test: `daemon/src/tools/bash_tool.rs:158-164` (extend `metadata_requests_shell_permission`)
- Modify: `daemon/src/tools/bash_tool.rs:81-94` (replace the Task-1 `None` placeholder)

**Interfaces:**
- Consumes: `input_schema: Option<serde_json::Value>` from Task 1.
- Produces: `BashTool::meta().input_schema` == the canonical command schema (Task 3's fallback logic and any future consumer rely on it being `Some`).

- [ ] **Step 1: Extend the failing assertion**

In `daemon/src/tools/bash_tool.rs`, replace the body of `metadata_requests_shell_permission` with:

```rust
    #[test]
    fn metadata_requests_shell_permission() {
        let meta = BashTool::meta();
        assert_eq!(meta.name, "bash");
        assert!(meta.required_permissions.contains(&Permission::Shell));
        assert_eq!(meta.execution_policy.timeout_ms, 30_000);
        // Inc 7: the advertised schema mirrors execute()'s actual contract —
        // one non-empty string `command`, mandatory.
        let schema = meta.input_schema.as_ref().expect("bash advertises a schema");
        assert_eq!(schema["type"], "object");
        assert_eq!(
            schema["required"],
            serde_json::json!(["command"])
        );
        assert_eq!(schema["properties"]["command"]["type"], "string");
    }
```

- [ ] **Step 2: Run to verify it fails**

Run: `bash -c 'cd /Users/ritikpathania/Developer/PyCharm/brain && RUSTFLAGS="-C link-arg=-Wl,-rpath,/Library/Developer/CommandLineTools/Library/Frameworks" cargo test -p brain-daemon --lib bash_tool 2>&1 | tail -5'`
Expected: FAIL — `bash advertises a schema`.

- [ ] **Step 3: Set the real schema**

In `BashTool::meta()` replace `input_schema: None,` with:

```rust
            input_schema: Some(serde_json::json!({
                "type": "object",
                "properties": {
                    "command": {
                        "type": "string",
                        "description": "Shell command to execute."
                    }
                },
                "required": ["command"]
            })),
```

- [ ] **Step 4: Run to verify it passes**

Run: same command as Step 2.
Expected: PASS (all six bash_tool tests).

- [ ] **Step 5: Commit**

```bash
git -C /Users/ritikpathania/Developer/PyCharm/brain add daemon/src/tools/bash_tool.rs
git -C /Users/ritikpathania/Developer/PyCharm/brain commit -m "feat(daemon): bash tool owns its command input schema

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### Task 3: Pure converter `definitions_from` + `advertised_definitions`

**Files:**
- Test: `daemon/src/tools/mod.rs` (extend `stack_tests` or add sibling module)
- Modify: `daemon/src/tools/mod.rs` (append converter after `tool_stack()`)

**Interfaces:**
- Consumes: `ToolMetadata.input_schema` (Task 1); `ToolStack.registry` (existing); `brain_core::model::ToolDefinition { name, description, parameters }`.
- Produces: `fn definitions_from(registry: &dyn ToolRegistry) -> Vec<ToolDefinition>` and `fn advertised_definitions() -> Vec<ToolDefinition>` — Task 4 calls ONLY `advertised_definitions()`.

- [ ] **Step 1: Write the failing tests**

Append to `daemon/src/tools/mod.rs`:

```rust
#[cfg(test)]
mod definition_tests {
    use super::*;
    use brain_core::errors::BrainError;
    use brain_core::extensibility::{
        ExecutionContext, ExecutionPolicy, ExecutionResult, Permission, Tool, ToolRegistryImpl,
    };
    use std::collections::HashMap;

    struct FakeTool {
        meta: brain_core::extensibility::ToolMetadata,
    }
    impl Tool for FakeTool {
        fn metadata(&self) -> &brain_core::extensibility::ToolMetadata {
            &self.meta
        }
        fn execute(
            &self,
            _: &ExecutionContext,
            _: &HashMap<String, serde_json::Value>,
        ) -> Result<ExecutionResult, BrainError> {
            Ok(ExecutionResult::new(serde_json::Value::Null))
        }
    }

    fn fake(name: &str, description: &str, schema: Option<serde_json::Value>) -> FakeTool {
        FakeTool {
            meta: brain_core::extensibility::ToolMetadata {
                name: name.to_string(),
                description: description.to_string(),
                usage: String::new(),
                version: "0".to_string(),
                author: "test".to_string(),
                required_permissions: Vec::<Permission>::new(),
                execution_policy: ExecutionPolicy { timeout_ms: 100 },
                supports_streaming: false,
                is_idempotent: true,
                causes_side_effects: false,
                input_schema: schema,
            },
        }
    }

    #[test]
    fn definitions_are_name_sorted_with_schema_passthrough_and_fallback() {
        let registry = ToolRegistryImpl::default();
        registry
            .register_tool(Arc::new(fake(
                "zeta",
                "last",
                Some(serde_json::json!({"type": "string"})),
            )))
            .unwrap();
        registry.register_tool(Arc::new(fake("alpha", "first", None))).unwrap();

        let defs = definitions_from(&registry);
        assert_eq!(
            defs.iter().map(|d| d.name.as_str()).collect::<Vec<_>>(),
            vec!["alpha", "zeta"],
            "BTreeMap-backed list_tools is already name-sorted"
        );
        assert_eq!(defs[0].description, "first");
        assert_eq!(defs[0].parameters, serde_json::json!({"type": "object"}));
        assert_eq!(defs[1].parameters, serde_json::json!({"type": "string"}));
    }

    #[test]
    fn empty_registry_yields_no_definitions() {
        let registry = ToolRegistryImpl::default();
        assert!(definitions_from(&registry).is_empty());
    }

    #[test]
    fn global_stack_advertises_exactly_bash() {
        let defs = advertised_definitions();
        assert_eq!(defs.len(), 1);
        assert_eq!(defs[0].name, "bash");
        assert_eq!(defs[0].parameters["required"][0], "command");
    }
}
```

Note: the test module imports `BrainError` itself (the parent module does not).

- [ ] **Step 2: Run to verify it fails**

Run: `bash -c 'cd /Users/ritikpathania/Developer/PyCharm/brain && RUSTFLAGS="-C link-arg=-Wl,-rpath,/Library/Developer/CommandLineTools/Library/Frameworks" cargo test -p brain-daemon --lib definition_tests 2>&1 | tail -5'`
Expected: FAIL — `definitions_from` / `advertised_definitions` not found.

- [ ] **Step 3: Implement the converter**

In `daemon/src/tools/mod.rs`, after `pub fn tool_stack()`:

```rust
/// Maps a registry's tools to provider-facing definitions (Inc 7).
/// `list_tools()` is BTreeMap-backed, so output is name-sorted; tools
/// without an owned schema advertise a permissive object.
pub fn definitions_from(
    registry: &dyn brain_core::extensibility::ToolRegistry,
) -> Vec<brain_core::model::ToolDefinition> {
    registry
        .list_tools()
        .iter()
        .map(|tool| {
            let meta = tool.metadata();
            brain_core::model::ToolDefinition {
                name: meta.name.clone(),
                description: meta.description.clone(),
                parameters: meta
                    .input_schema
                    .clone()
                    .unwrap_or_else(|| serde_json::json!({"type": "object"})),
            }
        })
        .collect()
}

/// The definitions the daemon advertises to providers: everything the
/// shared executor stack can actually execute. Advertisement implies no
/// authorization — the permission round trip stays the sole gate.
pub fn advertised_definitions() -> Vec<brain_core::model::ToolDefinition> {
    definitions_from(&tool_stack().registry)
}
```

(The file-top import `use brain_core::extensibility::{Tool as _, ToolRegistry};` already brings the trait into scope so `.list_tools()` resolves.)

- [ ] **Step 4: Run to verify it passes**

Run: same command as Step 2.
Expected: PASS — 3 new tests; then run the whole lib: `cargo test -p brain-daemon --lib` → 39 passed / 0 failed.

- [ ] **Step 5: Commit**

```bash
git -C /Users/ritikpathania/Developer/PyCharm/brain add daemon/src/tools/mod.rs
git -C /Users/ritikpathania/Developer/PyCharm/brain commit -m "feat(daemon): registry-to-ToolDefinition converter

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### Task 4: Gate + wire into the rounds loop

**Files:**
- Modify: `daemon/src/transport/uds/handlers.rs:1936-1947`

**Interfaces:**
- Consumes: `crate::tools::advertised_definitions() -> Vec<brain_core::model::ToolDefinition>` (Task 3); `resolved_model_desc.supports_tools` (existing binding, :1871).
- Produces: every pass's `gen_request.tools` carries the definitions iff the resolved model supports tools. No later task consumes anything from this task — its deliverable is runtime behavior verified by regression.

There is no new observable test surface here (spec §7.4): the spawned daemon's provider cannot be introspected over UDS. This task's discipline is surgical-edit-plus-full-regression.

- [ ] **Step 1: Insert the gate before `'rounds:`**

Immediately AFTER these existing lines (~1936):

```rust
            let mut is_completed_successfully = false;
            let mut is_cancelled = false;
```

insert:

```rust

            // Increment 7: advertise executable tools to tool-capable models.
            // Built once per turn; every loop pass re-sends the same set
            // (providers are stateless). supports_tools=false keeps today's
            // exact empty-vec request shape.
            let advertised_tools = if resolved_model_desc.supports_tools {
                crate::tools::advertised_definitions()
            } else {
                Vec::new()
            };
```

- [ ] **Step 2: Replace the per-pass literal**

In the `gen_request` construction inside `'rounds:` (:1947), change exactly one line:

```rust
                    tools: advertised_tools.clone(),
```

- [ ] **Step 3: Full regression**

Run: `bash -c 'cd /Users/ritikpathania/Developer/PyCharm/brain && RUSTFLAGS="-C link-arg=-Wl,-rpath,/Library/Developer/CommandLineTools/Library/Frameworks" cargo test -p brain-daemon --lib --test uds_feedback_loop_tests --test uds_generation_tests --test uds_tool_execution_tests --test uds_permission_roundtrip_tests 2>&1 | grep -E "^test result"'`
Expected: identical counts to baseline — 36/0, 4/4, 3/3, 4/4, 3/3 (mock ignores `request.tools`, so populated tools change no scripted behavior; sequences untouched).

- [ ] **Step 4: Commit**

```bash
git -C /Users/ritikpathania/Developer/PyCharm/brain add daemon/src/transport/uds/handlers.rs
git -C /Users/ritikpathania/Developer/PyCharm/brain commit -m "feat(daemon): advertise tool definitions on generation requests

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### Task 5: Mock recorder proves the gateway boundary

**Files:**
- Test: `crates/brain-services/src/model/mock.rs` (new test module)
- Modify: `crates/brain-services/src/model/mock.rs` (struct field, both constructors, stream_generation entry)

**Interfaces:**
- Consumes: existing `DeterministicMockProvider::{new, with_models}`, `stream_generation` (mock.rs:205).
- Produces: `pub fn last_request_tools(&self) -> Vec<String>` — observe-only; no scripting behavior changes. Task 6's gate relies on this suite staying green alongside the others.

- [ ] **Step 1: Write the failing test**

Append to `crates/brain-services/src/model/mock.rs`:

```rust
#[cfg(test)]
mod recorded_tools_tests {
    use super::*;
    use brain_core::model::{ChatRole, ModelChatMessage, ToolDefinition};
    use futures::StreamExt;

    #[tokio::test]
    async fn stream_generation_records_advertised_tool_names() {
        let provider = DeterministicMockProvider::new();
        let request = GenerationRequest {
            model: "brain-default".to_string(),
            messages: vec![ModelChatMessage::text(ChatRole::User, "hi")],
            system_prompt: None,
            tools: vec![
                ToolDefinition {
                    name: "bash".to_string(),
                    description: "shell".to_string(),
                    parameters: serde_json::json!({"type": "object"}),
                },
                ToolDefinition {
                    name: "search".to_string(),
                    description: "find".to_string(),
                    parameters: serde_json::json!({"type": "object"}),
                },
            ],
            thinking_budget: None,
        };
        let stream = provider
            .stream_generation(request, CancellationToken::new())
            .await
            .unwrap();
        while let Some(chunk) = stream.next().await {
            let _ = chunk.unwrap();
        }
        assert_eq!(provider.last_request_tools(), vec![
            "bash".to_string(),
            "search".to_string()
        ]);
    }

    #[tokio::test]
    async fn no_tools_recorded_as_empty() {
        let provider = DeterministicMockProvider::new();
        let request = GenerationRequest {
            model: "brain-default".to_string(),
            messages: vec![ModelChatMessage::text(ChatRole::User, "hi")],
            system_prompt: None,
            tools: Vec::new(),
            thinking_budget: None,
        };
        let _ = provider
            .stream_generation(request, CancellationToken::new())
            .await
            .unwrap()
            .map(|_| ())
            .collect::<Vec<_>>()
            .await;
        assert!(provider.last_request_tools().is_empty());
    }
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `bash -c 'cd /Users/ritikpathania/Developer/PyCharm/brain && RUSTFLAGS="-C link-arg=-Wl,-rpath,/Library/Developer/CommandLineTools/Library/Frameworks" cargo test -p brain-services --lib recorded_tools_tests 2>&1 | tail -5'`
Expected: FAIL — no method `last_request_tools`.

- [ ] **Step 3: Implement the recorder**

Three edits in `crates/brain-services/src/model/mock.rs`:

(a) Struct field (after `sentinel_counter`):

```rust
    /// Observe-only log of the most recent request's advertised tool names.
    last_request_tools: Arc<Mutex<Vec<String>>>,
```

(b) Both constructors (`new()` and `with_models()`) gain the initializer:

```rust
            last_request_tools: Arc::new(Mutex::new(Vec::new())),
```

(c) Public accessor (after `set_default_response`):

```rust
    /// Names advertised by the most recent `stream_generation` request.
    pub fn last_request_tools(&self) -> Vec<String> {
        self.last_request_tools.lock().clone()
    }
```

(d) At the TOP of `async fn stream_generation`, immediately after the cancellation check:

```rust
        *self.last_request_tools.lock() =
            request.tools.iter().map(|t| t.name.clone()).collect();
```

Note: `request` is consumed by value today only later (moved into the error arm's `request.model.clone()`); reading `request.tools` BEFORE any move is safe and requires no signature change.

- [ ] **Step 4: Run to verify it passes**

Run: same command as Step 2, then the full services lib: `cargo test -p brain-services --lib`.
Expected: 44 passed / 0 failed (42 baseline + 2 new).

- [ ] **Step 5: Commit**

```bash
git -C /Users/ritikpathania/Developer/PyCharm/brain add crates/brain-services/src/model/mock.rs
git -C /Users/ritikpathania/Developer/PyCharm/brain commit -m "test(services): record advertised tool names at mock provider entry

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### Task 6: Full gates on the finished increment

**Files:**
- None created. Verification only; commit only if a gate forces a fix.

- [ ] **Step 1: Shell suite**

Run: `bash -c 'cd /Users/ritikpathania/Developer/PyCharm/brain/packages/brain-shell && bun test 2>&1 | tail -3'`
Expected: 231 pass / 5 fail — the documented baseline. Shell production files untouched; any NEW failure stops the increment.

- [ ] **Step 2: Rust workspace slice**

Run: `bash -c 'cd /Users/ritikpathania/Developer/PyCharm/brain && RUSTFLAGS="-C link-arg=-Wl,-rpath,/Library/Developer/CommandLineTools/Library/Frameworks" cargo test -p brain-core -p brain-tools -p brain-services -p brain-daemon 2>&1 | grep -E "^test result"'`
Expected: 0 failures anywhere EXCEPT the single documented pre-existing `uds_security_audit_tests::test_security_path_traversal_and_invalid_identifiers`. If cargo halts at that target first, rerun remaining targets explicitly (`--test uds_soak_and_operational_tests --test integration_uds_session`).

- [ ] **Step 3: Build gate**

Run: `bash -c 'cd /Users/ritikpathania/Developer/PyCharm/brain/packages/brain-shell && bun build src/main.tsx --outdir dist --target bun'`
Expected: successful bundle. Never commit `dist/`.

- [ ] **Step 4: Vendor-concept scan (added lines only)**

Run: `bash -c 'cd /Users/ritikpathania/Developer/PyCharm/brain && git diff 16258c88..HEAD -- crates daemon packages scripts | grep "^+" | grep -icE "anthropic|api\.anthropic|claude"'`
Expected: prints `0`.

- [ ] **Step 5: PTY regression smoke**

Run: `bash -c 'cd /Users/ritikpathania/Developer/PyCharm/brain && python3 scripts/ptySmokeInc6.py'; echo EXIT=$?`
Expected: 14/14 PASS, EXIT=0. Nothing in Inc 7 is user-visible ⇒ no new smoke script/fixtures; this is pure regression. Restore afterward if fixtures dirty: `git checkout -- packages/brain-shell/src/test/fixtures/pty/inc6/`.

- [ ] **Step 6: Report**

Summarize: commits landed (short hashes + subjects), test counts per suite, gates, and any deviations. Then proceed to the finishing-a-development-branch skill (base branch: main).

---

## Self-Review Record

- **Spec coverage:** §2 decisions → Tasks 1 (schema source), 4 (supports_tools gate), 3 (Approach A converter); §4.1 field → Task 1; §4.2 BashTool schema → Task 2; §4.3 converter signatures → Task 3 (exact names/types match); §4.4 wiring → Task 4; §4.5 recorder → Task 5; §5 error rows → covered by Task 3 fallback test, Task 3 empty-registry test, unchanged arms in Task 4 regression; §6 non-goals → no task touches those areas; §7 testing → Tasks 1–6 mirror items 1–6; §8 constraints → Global Constraints.
- **Placeholder scan:** none — every code step carries complete compilable source; the two "if the compiler flags it" notes name the exact fix inline.
- **Type consistency:** `definitions_from(registry: &dyn ToolRegistry)` / `advertised_definitions()` used identically in Tasks 3 and 4; `input_schema: Option<serde_json::Value>` spelled identically in Tasks 1, 2, 3; `last_request_tools() -> Vec<String>` matches between Task 5 steps 1 and 3; Task 4's expected counts equal the pre-increment baseline plus zero new e2e suites.
