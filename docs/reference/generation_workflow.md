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

The generator writes the same TypeScript definitions to two tracked files:

- `generated/typescript/types.ts` for workspace consumers.
- `sdks/typescript/src/generated/types.ts` for the SDK.

Commit both copies with the Rust DTO change. The generator uses `temp_generated/` for staging.

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
4. **Run DTO tests**: Execute local verification checks to assert serialization consistency against golden snapshots:
   ```bash
   cargo test -p brain-integrations
   ```
5. **Inspect Changes**: Review the contract changes in Git:
   ```bash
   git diff generated/typescript/types.ts sdks/typescript/src/generated/types.ts
   ```
6. **Verify the artifacts**: Stage both intentional generated changes and run `cargo xtask verify-contracts`. The command expects no unstaged generated-file drift.

---

## 3. Pipeline Invariants

The contract generation pipeline enforces several safety properties:

* **Staging**: The generator verifies that the staged output is nonempty and includes the required envelope before copying it into the two tracked paths.
* **Generation and freshness**: Use the generator for all edits. Verification generates twice in memory to check determinism, compares both tracked copies to that output, and checks for unstaged generated-file drift.
* **Golden Snapshots**: Predefined test payloads are serialized and compared byte-for-byte against static snapshots in `crates/brain-integrations/tests/golden/`. Any change in field order, spelling, or value structures will fail the build until snapshots are intentionally updated.
