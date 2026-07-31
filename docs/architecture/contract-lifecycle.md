---
status: active
owner: architecture
canonical: true
review_cycle: quarterly
last_reviewed: 2026-07-30
applies_to: v0.8+
---

# Contract Lifecycle

This document explains the lifecycle of Application Interface contracts (DTOs) and how they evolve, generate, and synchronize across client SDKs.

---

## The Mental Model

```text
Edit DTO (Rust)
       │
       ▼
Register Contract (xtask)
       │
       ▼
Generate (Specta/xtask)
       │
       ▼
Verify (Golden & Freshness checks)
       │
       ▼
Commit (Generated SDK types to Git)
       │
       ▼
Consume (TypeScript & Python clients)
```

### 1. Edit DTO
Any change to the boundary contract starts inside the production `brain-integrations` crate (e.g. inside `events.rs`, `envelope.rs`, or `identity.rs`). Developers define or modify idiomatic Rust structures. Domain entities are never modified here; DTOs and domain models evolve independently.

### 2. Register Contract
Register the new or modified types inside the explicit registry inside `xtask/src/main.rs`. This ensures only intended public structures are exposed to client SDKs.

### 3. Generate
The developer runs the generator task:
```bash
cargo xtask generate-contracts
```
This derives TypeScript definitions and intermediate contract representations. Output files are written to a temporary directory and moved atomically to the workspace root `generated/` directory if successful.

### 4. Verify
Local and CI verification gates assert contract correctness:
* **Golden Tests**: Asserts that Predefined DTO states serialize to deterministic byte-for-byte identical snapshots.
* **Freshness Check**: Verifies that committed artifacts on disk match the current Rust definitions (using SHA256 validation).
* **Determinism Check**: Guarantees that compilation output order and headers are fully reproducible across builds.

### 5. Commit
Generated client SDK type definitions (under `generated/typescript/`) are checked into source control so that SDK clients can import them natively. Temporary intermediate contract representations under `generated/contracts/` are ignored.

### 6. Consume
Client libraries (like the TypeScript SDK) import the types from the `generated/` folder and wrap them in client-side helpers to call the stable application interface.
