# Brain Reference Specification

> **Status**: Versioned Specification  
> **Authority**: Stable Interface Contracts

---

## 1. Overview

This document specifies the stable contracts, traits, and intermediate representations that realize the principles set forth in [CONSTITUTION.md](file:///Users/ritikpathania/Developer/PyCharm/brain/docs/architecture/CONSTITUTION.md).

---

## 2. Compiler Contracts

### A. The Compiler Interface
The `KnowledgeCompiler` operates with a pure input/output contract:

```rust
pub trait KnowledgeCompiler: Send + Sync {
    fn compile(
        &self,
        ctx: &CompilerContext,
        request: MutationRequest,
    ) -> Result<CompilerResult, CompilerError>;
}
```

### B. `CompilerResult` & `GraphDelta`
```rust
pub struct CompilerResult {
    pub graph_delta: GraphDelta,
    pub events: Vec<RuntimeEvent>,
    pub diagnostics: Vec<Diagnostic>,
}

pub struct GraphDelta {
    pub added_nodes: Vec<Node>,
    pub updated_nodes: Vec<Node>,
    pub removed_nodes: Vec<NodeId>,
    pub added_edges: Vec<Edge>,
    pub updated_edges: Vec<Edge>,
    pub removed_edges: Vec<EdgeId>,
}
```

---

## 3. Compiler Pass Classification

Compiler passes are organized into three sequential tiers:

1. **Front End Passes**:
   * `ParsePass`: Deserializes and validates raw payload structure.
   * `ValidationPass`: Asserts schema and domain invariants.
   * `NormalizePass`: Standardizes timestamps and strings into `Observation IR`.

2. **Middle End Passes**:
   * `CanonicalizePass`: Maps raw observations to candidate entity IDs (`Knowledge IR`).
   * `ConflictResolutionPass`: Applies deterministic tie-breaking policies.
   * `ReflectionPass`: Evaluates graph reflection rules and generates findings.

3. **Back End Passes**:
   * `ProvenancePass`: Stamps origin, pass metadata, and execution timestamps.
   * `PersistencePass`: Applies `GraphDelta` to `KnowledgeRepository`.
   * `EventPass`: Emits internal `RuntimeEvent`s for stateless adapters.

---

## 4. ReadProjection Contract

Read-only projections MUST implement the `ReadProjection` trait:

```rust
pub trait ReadProjection: Send + Sync {
    type Input;
    type Output;

    fn project(
        &self,
        input: &Self::Input,
    ) -> Result<Self::Output, ProjectionError>;
}
```

---

## 5. Storage Repository Interfaces

Runtime operations interact exclusively through repository trait boundaries:

```rust
pub trait KnowledgeRepository: Send + Sync {
    fn apply_delta(&self, delta: &GraphDelta) -> Result<(), StorageError>;
    fn query_nodes(&self, params: &QueryParams) -> Result<Vec<Node>, StorageError>;
}

pub trait SecretRepository: Send + Sync {
    fn get_secret(&self, key: &str) -> Result<Option<SecretValue>, StorageError>;
    fn set_secret(&self, key: &str, value: &SecretValue) -> Result<(), StorageError>;
}

pub trait BlobRepository: Send + Sync {
    fn read_blob(&self, id: &BlobId) -> Result<Vec<u8>, StorageError>;
    fn write_blob(&self, id: &BlobId, data: &[u8]) -> Result<(), StorageError>;
}
```

---

## 6. Capability Registry Interface

```rust
pub struct CapabilityRegistry {
    pub projection: Arc<dyn ProjectionCapability>,
    pub query: Arc<dyn QueryCapability>,
    pub storage: Arc<dyn StorageCapability>,
    extensions: std::collections::HashMap<std::any::TypeId, Box<dyn std::any::Any + Send + Sync>>,
}
```
