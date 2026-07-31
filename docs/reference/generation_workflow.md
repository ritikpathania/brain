---
status: active
owner: architecture
canonical: true
review_cycle: quarterly
last_reviewed: 2026-07-30
applies_to: v0.8+
---

# Deterministic Contract Generation Workflow

This guide details the code generation process for exporting application interface contracts (DTOs) from Rust to client SDKs (such as TypeScript).

---

## 1. Directory Structure

All generated contract files are exported to the workspace root under a unified layout:
```text
generated/
    typescript/
        types.ts      # Committed client SDK type definitions
```

* **Committed Files**: Output client types (like `generated/typescript/types.ts`) are checked into Git so client SDK consumers can import them natively without running the Rust compile chain.
* **Ignored Files**: Temporary intermediate structures or draft validation outputs generated during builds are added to `.gitignore`.

---

## 2. Developer Workflow

When modifying or adding Application DTOs inside `crates/brain-integrations/src/`:

1. **Modify the DTO**: Edit Rust DTO structs and enums (e.g. inside `identity.rs`, `envelope.rs`, or `events.rs`). Keep derives clean:
   ```rust
   #[derive(Serialize, Deserialize, PartialEq, Type)]
   pub struct MyNewDTO { ... }
   ```
2. **Register the Contract**: Register the new DTO in the explicit contract registry inside `xtask/src/main.rs`:
   ```rust
   let types_to_export = vec![
       ...
       ("MyNewDTO", specta::ts::export::<brain_integrations::MyNewDTO>(&config)?),
   ];
   ```
3. **Regenerate Contracts**: Run the `xtask` contract generation runner:
   ```bash
   cargo xtask generate-contracts
   ```
4. **Run Verification Tests**: Execute local verification checks to assert serialization consistency against golden snapshots:
   ```bash
   cargo test -p brain-integrations
   ```
5. **Inspect Changes**: Review the contract changes in Git:
   ```bash
   git diff generated/typescript/types.ts
   ```

---

## 3. Pipeline Invariants

The contract generation pipeline enforces several safety properties:

* **Atomic Overwrites**: The `xtask` runner writes generated files to a temporary directory (`temp_generated/`) first and validates syntax completeness before atomically moving them to `generated/typescript/`. This prevents corrupting generated files if generation fails halfway through.
* **Zero Manual Edits**: Developers must never modify the generated TypeScript files manually. CI builds will verify freshness by regenerating files and asserting that `git diff --exit-code` remains clean.
* **Golden Snapshots**: Predefined test payloads are serialized and compared byte-for-byte against static snapshots in `crates/brain-integrations/tests/golden/`. Any change in field order, spelling, or value structures will fail the build until snapshots are intentionally updated.
