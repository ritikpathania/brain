# Data Flow and Sequences

This document provides a canonical reference for the internal data flow, layers, components, and execution sequences in the Brain Relational Memory Engine as of v0.8.

---

## 1. Architectural Layers and Data Flow

The system is designed around a clean Domain-Driven Design (DDD) model, separating protocols, services, and pure domain rules. Data flows top-down, and dependencies point strictly inward.

```mermaid
graph TD
    subgraph Layer1 [Adapters & UI Layer]
        TUI[Ratatui Console]
        MCP[MCP Server]
        UDS[UDS JSON-IPC Listener]
    end

    subgraph Layer2 [Orchestration Layer]
        App[BrainRuntime / Application Service]
    end

    subgraph Layer3 [Domain Services Layer]
        Pipeline[Retrieval Pipeline]
        Expander[Relationship Expander]
        Projectors[Graph & List Projectors]
    end

    subgraph Layer4 [Infrastructure & Storage Layer]
        Sqlite[SqliteStorage / Transactions]
        Cache[SessionCacheManager]
    end

    subgraph Layer5 [Core Domain Model]
        DomainEntities[Node / Edge / KnowledgeGraph]
        DomainInvariants[Taxonomy / TemporalValidity]
    end

    %% Flow Directions
    TUI --> App
    MCP --> App
    UDS --> App
    
    App --> Pipeline
    App --> Expander
    App --> Projectors
    
    Pipeline --> Sqlite
    Pipeline --> Cache
    Projectors --> Sqlite
    
    Sqlite --> DomainEntities
    Pipeline --> DomainEntities
    Projectors --> DomainEntities
    DomainEntities --> DomainInvariants
```

---

## 2. Pipeline Component Topology

The retrieval engine processes search requests through a modular multi-stage pipeline. The stage boundaries are strictly enforced:

```mermaid
flowchart LR
    Request([RetrievalRequest]) --> Stage1[Stage 1: Normalization]
    Stage1 --> Stage2[Stage 2: Candidate Retrieval]
    
    subgraph Sources [Memory Sources]
        Stm[StmMemorySource]
        Ltm[LtmMemorySource]
    end
    
    Stage2 --- Sources
    Sources --> Accumulator[PipelineAccumulator]
    Accumulator --> Stage3[Stage 3: Fusion / Ranking]
    
    subgraph Fuser [Ranking / Fusion Strategies]
        RRF[RrfFusionStrategy]
        Decay[Temporal Decay / Ranker]
    end
    
    Stage3 --- Fuser
    Fuser --> Stage4[Stage 4: Truncation]
    Stage4 --> Stage5([RetrievalResponse])
    
    Stage5 --> Expander[Relationship Expander (Opt-In)]
    Expander --> Projector[Projectors (Outside Pipeline)]
```

---

## 3. Sequence Diagrams

### A. Standard Retrieval Sequence

Below is the execution flow for a standard lexical and vector query:

```mermaid
sequenceDiagram
    autonumber
    participant App as Application Service
    participant Pipe as RetrievalPipeline
    participant STM as StmMemorySource
    participant LTM as LtmMemorySource
    participant Accum as PipelineAccumulator
    participant RRF as RrfFusionStrategy
    participant Store as SqliteStorage

    App->>Pipe: execute(RetrievalRequest)
    activate Pipe
    
    %% Stage 1 & 2
    Pipe->>STM: retrieve_candidates(query)
    activate STM
    STM-->>Pipe: stm_candidates
    deactivate STM

    Pipe->>LTM: retrieve_candidates(query)
    activate LTM
    LTM->>Store: Query lexical nodes (BM25)
    Store-->>LTM: matching_nodes
    LTM->>Store: Query vector embeddings (IVF partitions)
    Store-->>LTM: matching_embeddings
    LTM-->>Pipe: ltm_candidates
    deactivate LTM
    
    %% Accumulation
    Pipe->>Accum: new()
    Pipe->>Accum: add(stm_candidates)
    Pipe->>Accum: add(ltm_candidates)
    Accum-->>Pipe: unique_candidates
    
    %% Stage 3 & 4
    Pipe->>RRF: rank(unique_candidates)
    activate RRF
    RRF-->>Pipe: fused_ranked_list (deterministic tie-break)
    deactivate RRF
    
    Pipe->>Pipe: Truncate list to RetrievalRequest.limit
    
    Pipe-->>App: RetrievalResponse (ranked nodes + explanations)
    deactivate Pipe
```

---

### B. Graph-Aware Retrieval Sequence (N-Hop Traversal)

When `graph_depth` is set to `Some(n)`, memory sources perform a BFS graph traversal starting from the direct query matches to include neighbor context:

```mermaid
sequenceDiagram
    autonumber
    participant Pipe as RetrievalPipeline
    participant Source as GraphAwareMemorySource / LTM
    participant Store as SqliteStorage
    participant Budget as TraversalBudget

    Pipe->>Source: retrieve_candidates(request)
    activate Source
    
    Source->>Store: Get flat matching nodes (lexical/semantic)
    Store-->>Source: flat_results

    Source->>Budget: new(max_depth, limit)
    
    Note over Source, Budget: Traversal Loop (Up to max_depth)
    loop for depth = 1 to max_depth
        Source->>Store: Get adjacent edges for current frontier
        Store-->>Source: adjacent_edges
        Source->>Budget: accumulate(adjacent_edges)
        Budget-->>Source: updated_frontier / continue
    end

    Source-->>Pipe: merged_graph_candidates (monotonic expansion)
    deactivate Source
```

---

### C. Relationship Expansion Sequence

When `expand_relations` is set to `true`, a post-pipeline processor maps candidate nodes to their first-order edges:

```mermaid
sequenceDiagram
    autonumber
    participant App as Application Service
    participant Exp as RelationshipExpander
    participant Store as SqliteStorage

    App->>App: Execute standard RetrievalPipeline
    Note over App: Obtains list of ranked Node entries

    App->>Exp: expand_relationships(ranked_nodes)
    activate Exp
    
    loop for each node in ranked_nodes
        Exp->>Store: get_connections(node_id)
        Store-->>Exp: list_of_edges (Edge)
        Exp->>Exp: Map to RelationshipExpansionDTO (Incoming / Outgoing EdgeDTO sets)
    end
    
    Exp-->>App: Vec<RelationshipExpansionDTO>
    deactivate Exp
    
    App-->>App: Attach DTO list to RetrievalResponse.relationships
```

---

### D. Graph Projection Sequence

Read models evaluate projections on-demand outside of the retrieval pipeline:

```mermaid
sequenceDiagram
    autonumber
    participant App as Application Service
    participant Proj as Neighborhood / Path / Cluster Projector
    participant Context as ProjectionContext
    participant Graph as KnowledgeGraph

    App->>Context: new(graph, query, epoch, correlation_id)
    App->>Proj: project(context)
    activate Proj
    
    alt NeighborhoodProjection
        Proj->>Graph: Traverse BFS from focal node up to depth N
        Graph-->>Proj: neighbors & edges
    else PathProjection
        Proj->>Graph: Run shortest path BFS(source, target)
        Graph-->>Proj: path_edges & path_nodes
    else ClusterProjection
        Proj->>Graph: Run BFS partition for connected components
        Graph-->>Proj: partitioned_clusters (filtered by min_size)
    end
    
    Proj-->>App: Projected output representation
    deactivate Proj
```
