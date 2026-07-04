# Graph Protocol Specification

This document defines the formal protocol contracts and constraints governing the Knowledge Graph model, extraction layers, repository storage engine, and query traversals in the Brain system.

---

## 1. Protocol Schemas & Enums

To ensure schema stability and prevent drift over time, all schema types are defined as frozen domain-level contracts.

### NodeKind
The `NodeKind` enum classifies entities in the graph. External plugins/extractors that need custom semantics must store them in the node's properties or labels rather than attempting to introduce raw ad-hoc variants.
- **Variants**:
  - `Person`
  - `Project`
  - `Organization`
  - `Technology`
  - `Database`
  - `File`
  - `Credential`
  - `Concept`
  - `Tool`
  - `Service`
  - `Unknown` (Fallback variant. All unsupported or unrecognized strings during deserialization resolve to `Unknown`).

### RelationKind
The `RelationKind` enum classifies structural edges/connections between nodes.
* **Semantic Authority**: The semantic definitions, directional properties, and validations for relation kinds are governed by the declarative relation registry (see [relations.md](file:///Users/ritikpathania/Developer/PyCharm/brain/docs/architecture/relations.md) for full taxonomy specification).
- **Variants**:
  - `Uses`
  - `DependsOn`
  - `RunsOn`
  - `Develops`
  - `StoredIn`
  - `Configures`
  - `CommunicatesVia`
  - `AssociatedWith`
  - `Unknown` (Fallback variant).

### GraphVersion
Encapsulates the schema version to ensure explicit versioned handling and future migration paths.
- **Type**: `pub struct GraphVersion(u32);`
- **Associated Constants**: `GraphVersion::V1`
- **Serialization**: Serializes/deserializes directly as a raw `u32` integer value to remain backward compatible.

### GraphProvenance
Represents the origin context metadata for nodes and edges.
- **Fields**:
  - `source_conversation`: Option<ConversationId>
  - `source_message`: Option<MessageId>
  - `extracted_at`: u64 (Unix timestamp)
  - `extractor_version`: String
  - `confidence`: f32
  - `text_span`: Option<String> (Specific text fragment from which this fact was extracted)
- **Invariants**:
  - **Intrinsic Immutability**: All fields of `GraphProvenance` are strictly immutable once persisted. Derived runtime metadata (e.g. retrieval count, reinforcement score, access timestamps) must be tracked in separate models.

---

## 2. Boundary Contracts

### MemoryExtractor
A pure, persistence-agnostic interface for semantic memory extraction.
```rust
pub struct ExtractionRequest {
    pub raw_content: String,
    pub context_metadata: HashMap<String, String>,
}

pub struct ExtractionResult {
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
    pub provenance: GraphProvenance,
    pub graph_version: GraphVersion,
}

pub trait MemoryExtractor: Send + Sync {
    fn extract(&self, request: ExtractionRequest) -> Result<ExtractionResult, BrainError>;
}
```
- **Invariants**:
  - **Read-Oriented Only**: Extractors must perform no direct database writes or repository mutations. They simply transform text into candidate nodes and edges.

---

## 3. Storage & Repository Invariants

Every repository implementation (e.g. SQLite storage engine) must guarantee the following safety invariants:

1. **ID Immutability**: Node and Edge IDs are strictly immutable. Any conflict resolution must preserve the existing ID.
2. **Provenance Safety**: Repository implementations must never overwrite or mutate existing `GraphProvenance` records during conflict resolution unless explicitly requested.
3. **No Silent Deletions**: Ingress conflicts must be resolved via deterministic properties merging (retaining legacy fields and updating matching ones) instead of silently deleting or overwriting metadata.
4. **Idempotence**: Redundant writes of identical elements must have no side effects.

---

## 4. Traversal & Budgets

To protect retrieval sources from path explosions, cycles, and unbounded latency, all graph traversals are bound by `TraversalBudget`.

```rust
pub struct TraversalBudget {
    pub max_depth: usize,
    pub max_nodes: usize,
    pub max_edges: usize,
    pub prevent_cycles: bool,
    pub deadline: Option<Instant>,
}
```
- **Invariants**:
  - **Cycle Prevention**: If `prevent_cycles` is enabled, traversal paths must detect and prune loops.
  - **Deadline Enforcement**: Soft deadlines must be checked at each traversal iteration to prevent hung queries.
  - **Read-Only**: `Graph` must never perform database writes.

---

## 5. Protocol Evolution Rules

To maintain long-term system stability, all updates to this protocol must strictly adhere to the following constitution:

1. **Additive Changes Only**: New enum variants for protocol types (e.g., `NodeKind`, `RelationKind`) are strictly additive. Unused or deprecated variants may be retired but must remain as valid schema definitions to avoid breaking legacy databases.
2. **Immutable Semantics**: The semantics and meaning of an existing enum variant cannot be modified.
3. **GraphVersion Increments**: The `GraphVersion` sequence increments if and only if an incompatible, breaking schema change is introduced.
4. **Backward Compatible Invariants**: All database/repository safety invariants must be kept fully backward compatible with older version transactions.
5. **Deterministic Migrations**: Any schema migrations or translation pipelines must run in a 100% deterministic manner.
6. **Mandatory Golden Tests**: Every active or retired protocol version must be accompanied by mandatory, automated golden tests (in the storage test suite) to prevent regression against legacy serialized payloads.
7. **Forward Extensibility Preservation**: Protocol implementations must preserve forward extensibility. Specifically, unknown fields should be ignored where appropriate, older clients should fail predictably when encountering unsupported protocol versions, and newer implementations should avoid assuming exhaustive knowledge of future metadata.
