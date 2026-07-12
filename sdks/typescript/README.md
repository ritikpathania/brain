# Brain TypeScript Client SDK

A lightweight, transport-neutral TypeScript Client SDK for the `brain` Relational Memory Engine.

---

## SDK Philosophy & Responsibilities

The SDK acts strictly as a typed interface to the stable application boundary. It does not carry database engines or runtimes.

* **In Scope**:
  * Request/response DTO serialization & deserialization.
  * Byte-level transport interfaces (`Transport`).
  * Protocol framing (newline delimited codec).
  * Ergonomic, generic interface mapping.
* **Out of Scope** (Deferred to consuming applications or wrappers):
  * Retries, reconnection, or backpressure policies.
  * In-memory cache models or caching layers.
  * Business workflows or orchestration logic.

---

## Architecture & Surface Area

The package exposes a minimal, future-proof public API:

```typescript
import {
    BrainClient,
    BrainError,
    IngestionEnvelope
} from "@brain/sdk";
```

All transport implementation detail (`internal/`) and compiled code contract models (`generated/`) are hidden from the public exports list in `index.ts`.

---

## Compatibility Policy

The client SDK aligns with the application interface lifecycle guarantees:

```text
SDK MAJOR VERSION === Application Interface MAJOR VERSION
```

Backward-compatibility constraints and deprecation schemas are governed by the stable application interface policies (refer to `STABILITY.md`).

---

## Build and Developer Workflow

The TypeScript SDK targets ES2020. 

### Local Verification Test
Run the mock transport test runner to check integration typing and serialization compatibility:
```bash
# Install dependencies
npm install

# Compile the library and the example app
npm run build
npx tsc -p example/tsconfig.json --noEmit

# Execute validation program
node dist/example/index.js
```
