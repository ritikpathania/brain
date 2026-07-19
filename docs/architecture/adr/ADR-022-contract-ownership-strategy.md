# ADR-022: Contract Ownership & DTO Generation Strategy

## Status
Proposed

## Context
In [ADR-021 (Stable Application Interface)](file:///Users/ritikpathania/Developer/PyCharm/brain/docs/architecture/adr/ADR-021-stable-application-interface.md), we established the transport-neutral contract boundary governing interactions with the Brain Runtime. Now, we must make a foundational engineering strategy decision: **what owns the contract?**

This decision determines how Data Transfer Objects (DTOs) and client SDK types are defined, generated, and verified across languages (primarily Rust, TypeScript, and Python) without manual typing drift or runtime API desynchronization.

## Goals
* **Automate Schema Generation**: Automatically derive language-neutral schemas and downstream client types.
* **Drift Prevention**: Ensure that Rust backend DTOs, TypeScript SDK types, and Python SDK types never drift out of sync.
* **Ergonomic Workflows**: Maximize developer productivity, maintain idiomatic constructs in each language, and ensure compiler/IDE assistance works natively.
* **Backward Compatibility**: Establish a clear workflow for evaluating contract modifications before they are shipped.
* **Minimize Build Complexity**: Avoid introducing heavy, non-native build-tooling chains or complex runtime configurations.

---

## Evaluation Criteria
1. **Rust Ergonomics**: How natural and idiomatic is it to define, modify, and serialize models in the primary codebase?
2. **Multi-language / SDK Support**: How cleanly does the strategy generate high-quality types and builders for TypeScript and Python?
3. **Versioning & Schema Evolution**: How well does the system support additive fields, backward compatibility checks, and schema version tagging?
4. **Developer Experience**: Ergonomics of modifying a DTO, speed of the feedback loop, clarity of generation/compilation error messages, and ease of onboarding.
5. **Incremental Evolution**: Ability to modify or add one DTO without regenerating the entire workspace or introducing massive git diff churn.
6. **Build & Release Complexity**: Impact on local cargo builds, continuous integration (CI) time, and release automation.
7. **Ecosystem Maturity**: Quality and stability of the underlying tooling and generator libraries.

---

## Candidate Analysis

### Option 1: Rust-First (Code-First)
The contract is defined in idiomatic Rust structs. Downstream schemas and multi-language SDK types are automatically generated from these structures.

```
Rust Types (Canonical Source)
      │
      ├─► [Tooling compiles types] ──► Intermediate Contract Representation
      │                                       │
      ▼                                       ▼
Rust Runtime Serializers             Generated TS/Python Types
```

* **Observed Outcome**: Highly ergonomic for the core team. Rust remains the absolute source of truth. Types map cleanly to Serde serialization macros.
* **Tradeoff**: Downstream type generators depend on Rust compilation steps, requiring a Cargo invocation or build script to output the intermediate representation.
* **Impact on Brain**: Keeps `brain-domain` and `brain-integrations` lightweight, utilizing standard Rust macros to derive capabilities.

### Option 2: JSON Schema-First (Schema-First)
The contract is defined directly in language-neutral JSON Schema files. Both Rust structures and SDK clients are generated from these raw schemas.

```
JSON Schema Files (Canonical Source)
      │
      ├─► [Generator] ──► Generated Rust Structs
      │
      └─► [Generator] ──► Generated TS/Python Types
```

* **Observed Outcome**: Language-neutral representation is highly decoupled. However, generated Rust code is often bloated, uses non-idiomatic structures (e.g. nested Option wrappers, generic names), and lacks direct integration with domain validation traits.
* **Tradeoff**: Increases developer friction during Rust refactoring, as developers must edit JSON files and run external build steps before receiving compiler feedback.
* **Impact on Brain**: Impedes developer ergonomics inside the core crates.

### Option 3: IDL-First (Interface Definition Language)
The contract is defined using a dedicated IDL (e.g. Protobuf/gRPC, AWS Smithy, or CUE). Code-generators emit Rust, TS, and Python models.

```
IDL Files (Canonical Source)
      │
      ├─► [IDL Compiler] ──► Generated Rust Structs
      │
      └─► [IDL Compiler] ──► Generated TS/Python SDKs
```

* **Observed Outcome**: Strongest type-safety and backward compatibility guarantees. However, it introduces steep build-system complexity (e.g., protocol compilers, code generation build steps) and runtime serialization overhead (e.g., binary Protobuf to internal JSON-IPC mapping).
* **Tradeoff**: Significant tooling overhead that is disproportionate to a local-first, terminal-centric single-user runtime.
* **Impact on Brain**: High maintenance and contributor onboarding burden.

---

## Comparison Matrix

| Evaluation Criterion | Option 1: Rust-First | Option 2: Schema-First | Option 3: IDL-First |
| :--- | :---: | :---: | :---: |
| **Rust Ergonomics** | Excellent | Poor (non-idiomatic) | Moderate |
| **Multi-language Support**| Good | Excellent | Excellent |
| **Versioning & Evolution**| Good | Good | Excellent |
| **Developer Experience** | Excellent | Moderate | Poor (tooling overhead) |
| **Incremental Evolution** | Excellent | Moderate | Good |
| **Build & Release Complexity**| Low | Moderate | High (extra compilers) |
| **Ecosystem Maturity** | Excellent (Serde-based) | Good | Excellent |

---

## Decision Requirements

### 1. What is the canonical source of truth?
* The **Rust type definitions** located inside the application interface boundary act as the canonical source of truth.

### 2. What artifacts are generated?
* The intermediate **contract representation files** (e.g. schema definitions or serialization payloads).
* The **TypeScript interface definitions** (generated from the intermediate representation).
* The **Python SDK dataclasses/models** (if automated downstream type checking is introduced).

### 3. What artifacts are handwritten?
* The **Rust DTO structs** residing within the application interface boundary.
* The translation logic mapping DTOs to internal runtime domain entities.
* Domain entities themselves are **never** exported directly; application DTOs are the **only** exported contract, allowing domain models and DTOs to evolve independently.

### 4. What artifacts are forbidden to edit manually?
* The generated intermediate representation files (e.g., `protocol/*.schema.json` or exported TS files).
* The generated TypeScript interfaces and SDK types (`sdk/typescript/*.ts`).
* Any generated Python SDK files.

### 5. Who owns versioning?
* The **Application Interface Layer** owns versioning. Any breaking semantic change or DTO mutation triggers a compatibility version bump in the contract, independent of individual client or runtime release versions.

### 6. Who owns compatibility?
* **Application Interface Layer**: The Application Interface layer owns compatibility.
* **CI Build Verification**: The compilation and CI pipeline **enforces** compatibility. The pipeline verifies that the generated schemas/artifacts match the compiled Rust types and that backward compatibility tests pass successfully.

---

## Selected Strategy
We select **Option 1: Rust-First (Code-First)**. 

Phase 4C.1 empirically validated Specta as the preferred TypeScript generation backend. This decision does not establish Specta as the permanent intermediate contract representation for future languages. Additional generation backends may be introduced as future requirements emerge.

### Rationale
Because Brain is implemented as a native Rust runtime, keeping Rust types as the canonical source of truth maximizes developer experience, type safety, and IDE ergonomics. Tooling in the Rust ecosystem allows automated generation of downstream type definitions directly or via intermediate contract representations. This approach avoids non-idiomatic generated Rust code and minimizes build toolchain dependencies for local developers.

---

## Impact Areas
* **Maintenance Overhead**: Lowered. Changing a contract requires modifying only a Rust struct.
* **Generated Code Quality**: Excellent. TypeScript definitions represent clean, idiomatic interfaces.
* **Testing & Mocking**: Simplified. We can run standard unit tests on Rust DTOs and assert round-tripping.
* **CI Pipeline**: Evaluates the compiled Rust types, auto-generates the contract representation, and compares them with the checked-in artifacts to block PRs on un-staged drift.

---

## Migration Strategy
1. **Define DTOs in Rust**: Establish the initial Rust DTO types representing request/response envelopes.
2. **Export Intermediate Contract / TS representation**: Implement the export harness using the chosen code generation tooling.
3. **Establish Golden Serialization Tests**: Write tests that serialize sample Rust DTO structs and verify their binary/text formats match established "golden" snapshots.
4. **Deprecate Hand-written structs**: Systematically replace manually updated DTO definitions in `brain-integrations` with the new generated contract structures.

---

## Reconsideration Criteria
We will revisit this contract strategy decision if:
* The core Brain engine is rewritten in another language, or the runtime becomes language-neutral (eliminating Rust's role as the primary implementation language).
* The chosen generator libraries are deprecated or fail to support emerging platform standards.
* The multi-language SDK matrix expands significantly to languages where Rust-first schema generation becomes unmaintainable.

---

## Appendix: Possible Implementations & Concrete Tooling
* **Rust Intermediate Contract / Downstream Type Generation**:
  * `specta`: Directly generates TypeScript types from Rust types without intermediate JSON Schema steps.
  * `schemars`: Generates JSON Schema documents from Rust types using Serde.
* **Client SDK Code Generation**:
  * `quicktype-core` / `quicktype`: Generates TypeScript, Python, and other languages from JSON Schema.
  * Custom AST Translator: A simple translator script to map intermediate schemas to clean target types.
