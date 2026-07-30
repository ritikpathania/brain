# Design Specification: Graph Adjacency Projection (Phase 4 — Sub-Project 1)

## 1. Executive Summary & Goals

The **Graph Adjacency Projection** is a pure, event-driven domain read model that maintains high-performance graph topology and directional adjacency indices (`out_edges`, `in_edges`, normalized `edges` payload table, `node_degrees`) over domain events (`FactEvent`).

### Architectural Invariants & Core Rules
- **Domain/Service Separation**: `GraphAdjacencyState`, `NodeId`, `EdgeId`, `EdgeRecord`, and `GraphAdjacencyReducer` live in `brain-domain` with zero external dependencies.
- **Normalized Dual-Index Topology**: `out_edges` and `in_edges` map `NodeId -> Vec<EdgeId>`, while edge payloads exist once in `edges: HashMap<EdgeId, EdgeRecord>`.
- **$O(1)$ Directional Traversals**: Both incoming and outgoing neighbor lookups execute in $O(1)$ hash table time.
- **Deterministic Adjacency Ordering**: The order of `EdgeId`s in every adjacency list is strictly deterministic, determined solely by canonical event sequence.
- **Idempotent Duplicate Insertion**: Re-recording an existing `EdgeId` is idempotently ignored.
- **Empty Key Pruning**: Deleting the final edge targeting or originating from a node removes the empty `NodeId` map key to keep state canonical.
- **Encapsulated Read APIs**: Access to adjacency data is encapsulated through domain query methods (`neighbors_out`, `neighbors_in`, `degree`, `edge`).
- **Pure Replay Transparent Reducer**: `GraphAdjacencyReducer` implements `ProjectionReducer` with zero knowledge of storage, scheduling, or replay mode.

---

## 2. Architecture & Data Structures

```text
                           FactEvent Stream
                                  │
                                  ▼
                       GraphAdjacencyReducer
                                  │
                                  ▼
                       GraphAdjacencyState
         ┌────────────────────────┼────────────────────────┐
         ▼                        ▼                        ▼
     out_edges                in_edges                   edges
HashMap<NodeId, Vec<EdgeId>>  HashMap<NodeId, Vec<EdgeId>>  HashMap<EdgeId, EdgeRecord>
```

### Data Models (`crates/brain-domain/src/projection/graph_adjacency/`)

```rust
/// Node identifier wrapper.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NodeId(pub EntityId);

/// Edge identifier wrapper.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct EdgeId(pub FactVersionId);

/// Edge record containing normalized edge payload and metadata.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EdgeRecord {
    pub id: EdgeId,
    pub source: NodeId,
    pub target: NodeId,
    pub predicate: PredicateId,
    pub confidence: Confidence,
    pub temporal: TemporalWindow,
}

/// Cached degree stats per node.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct NodeDegree {
    pub in_degree: usize,
    pub out_degree: usize,
}

/// In-memory graph adjacency state.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct GraphAdjacencyState {
    out_edges: HashMap<NodeId, Vec<EdgeId>>,
    in_edges: HashMap<NodeId, Vec<EdgeId>>,
    edges: HashMap<EdgeId, EdgeRecord>,
    degrees: HashMap<NodeId, NodeDegree>,
}

impl GraphAdjacencyState {
    /// Returns outgoing edge IDs for node.
    pub fn neighbors_out(&self, node: &NodeId) -> &[EdgeId] {
        self.out_edges.get(node).map(|v| v.as_slice()).unwrap_or(&[])
    }

    /// Returns incoming edge IDs for node.
    pub fn neighbors_in(&self, node: &NodeId) -> &[EdgeId] {
        self.in_edges.get(node).map(|v| v.as_slice()).unwrap_or(&[])
    }

    /// Returns node degree stats.
    pub fn degree(&self, node: &NodeId) -> NodeDegree {
        self.degrees.get(node).copied().unwrap_or_default()
    }

    /// Returns edge record by EdgeId.
    pub fn edge(&self, id: &EdgeId) -> Option<&EdgeRecord> {
        self.edges.get(id)
    }

    /// Internal helper inserting edge record atomically.
    pub fn insert_edge(&mut self, record: EdgeRecord) {
        if self.edges.contains_key(&record.id) {
            return; // Idempotent ignore
        }
        let edge_id = record.id.clone();
        let source = record.source.clone();
        let target = record.target.clone();

        self.edges.insert(edge_id.clone(), record);
        self.out_edges.entry(source.clone()).or_default().push(edge_id.clone());
        self.in_edges.entry(target.clone()).or_default().push(edge_id);

        self.degrees.entry(source).or_default().out_degree += 1;
        self.degrees.entry(target).or_default().in_degree += 1;
    }

    /// Internal helper removing edge record atomically with empty key pruning.
    pub fn remove_edge(&mut self, edge_id: &EdgeId) {
        if let Some(record) = self.edges.remove(edge_id) {
            if let Some(out_list) = self.out_edges.get_mut(&record.source) {
                out_list.retain(|id| id != edge_id);
                if out_list.is_empty() {
                    self.out_edges.remove(&record.source);
                }
            }
            if let Some(in_list) = self.in_edges.get_mut(&record.target) {
                in_list.retain(|id| id != edge_id);
                if in_list.is_empty() {
                    self.in_edges.remove(&record.target);
                }
            }

            if let Some(deg) = self.degrees.get_mut(&record.source) {
                deg.out_degree = deg.out_degree.saturating_sub(1);
                if deg.out_degree == 0 && deg.in_degree == 0 {
                    self.degrees.remove(&record.source);
                }
            }
            if let Some(deg) = self.degrees.get_mut(&record.target) {
                deg.in_degree = deg.in_degree.saturating_sub(1);
                if deg.out_degree == 0 && deg.in_degree == 0 {
                    self.degrees.remove(&record.target);
                }
            }
        }
    }
}
```

---

## 3. Reducer Behavior & Event Handling

```rust
pub struct GraphAdjacencyReducer {
    id: ProjectionId,
    version: ProjectionVersion,
    state: GraphAdjacencyState,
}

impl ProjectionReducer for GraphAdjacencyReducer {
    fn id(&self) -> ProjectionId { self.id.clone() }
    fn version(&self) -> ProjectionVersion { self.version }

    fn apply_event(&mut self, event: &FactEvent) -> Result<(), ProjectionError> {
        match event {
            FactEvent::FactRecorded { fact } => {
                let edge_id = EdgeId(fact.id.clone());
                let source = NodeId(fact.entity_id.clone());
                if let Some(target_entity) = fact.value.as_entity_id() {
                    let target = NodeId(target_entity.clone());
                    let record = EdgeRecord {
                        id: edge_id,
                        source,
                        target,
                        predicate: fact.predicate_id.clone(),
                        confidence: fact.confidence,
                        temporal: fact.validity,
                    };
                    self.state.insert_edge(record);
                }
            }
            FactEvent::FactSuperseded { old_fact_id, .. } | FactEvent::FactArchived { fact_id: old_fact_id, .. } => {
                let edge_id = EdgeId(old_fact_id.clone());
                self.state.remove_edge(&edge_id);
            }
        }
        Ok(())
    }

    fn reset(&mut self) -> Result<(), ProjectionError> {
        self.state.out_edges.clear();
        self.state.in_edges.clear();
        self.state.edges.clear();
        self.state.degrees.clear();
        Ok(())
    }
}
```

---

## 4. Verification & Testing Strategy

1. **Unit & Invariant Tests (`crates/brain-domain/tests/graph_adjacency_tests.rs`)**:
   - `test_graph_adjacency_insert_and_directional_lookup`: Verifies $O(1)$ outgoing/incoming neighbor queries and read APIs (`neighbors_out`, `neighbors_in`, `degree`, `edge`).
   - `test_graph_adjacency_supersede_and_archive_removal`: Verifies correct edge removal, degree decrements, and empty map key pruning.
   - `test_graph_adjacency_idempotent_duplicate_insertion`: Verifies duplicate insertion is safely ignored.
   - `test_graph_adjacency_reset_flushes_all_state`: Verifies complete state reset.
2. **Projection Runtime Integration (`crates/brain-services/tests/graph_adjacency_runtime_tests.rs`)**:
   - Integrates `GraphAdjacencyReducer` with `ProjectionRuntime` and `ReplayEngine`.
   - Verifies live dispatch vs catch-up replay produces identical `GraphAdjacencyState`.
