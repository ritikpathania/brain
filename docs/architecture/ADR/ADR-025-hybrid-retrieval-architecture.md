# ADR-025: Hybrid Retrieval Architecture: Independent Channels and RRF Fusion

## Status
Proposed (Pending approval for Milestone 3.3/3.4 implementation)

## Context
In Milestone 3, we are designing a scale-safe, high-precision hybrid retrieval engine. We need to define the architectural boundary between our lexical retrieval (BM25 via SQLite FTS5) and semantic retrieval (vector similarity search via IVF partitioning). 

Specifically, we must decide whether:
1. **Option A (Independent Channels + RRF Fusion)**: Vector search operates as an independent retrieval channel, loading candidate subsets directly from the database and fusing them later with lexical matches using Reciprocal Rank Fusion (RRF).
2. **Option B (Lexical-Only Candidates)**: Vector search operates strictly as a secondary reranking step over the subset of candidate nodes already retrieved by the lexical search engine.

## Tradeoffs

### Option A: Independent Channels and RRF Fusion (Two-Stage Hybrid Search)
In this model, the retrieval service queries the lexical source (BM25) and the semantic source (Vector Index) independently, and then fuses the ranked outputs using RRF.

*   **Pros**:
    *   **Maximum Semantic Recall**: Semantic search can retrieve nodes that have a high conceptual overlap with the query but share zero literal keyword overlap. Option A completely avoids the "vocabulary mismatch" bottleneck.
    *   **Orthogonal Complementarity**: BM25 excels at finding exact terms, names, code identifiers, and specific UUIDs. Semantic search excels at conceptual meaning, synonyms, and intent. Fusing independent streams captures both strengths.
    *   **Scale-Safety**: IVF vector indexing (`find_by_centroids`) keeps semantic database reads highly constrained by filtering partitions at the SQL layer, preventing flat-scan bottlenecks at scale.
*   **Cons**:
    *   Requires executing two independent database reads (one FTS5 query, one partitioned embeddings query). However, SQLite FTS and partitioned reads are extremely fast (median latencies under ~1.5 ms), making this overhead negligible.

### Option B: Vector Search over Lexical Candidates Only
In this model, the lexical search first narrows down the candidate space to matching nodes, and the vector search is performed only on these lexical candidates.

*   **Pros**:
    *   Avoids executing a separate semantic database query, since embeddings are only loaded for the lexical matches.
*   **Cons**:
    *   **Lexical Mismatch Bottleneck**: If a relevant memory node does not contain any of the literal words in the user query, the lexical step will not retrieve it. Consequently, the semantic step will never see it, and it can never be retrieved. This fundamentally breaks the core value proposition of semantic search.
    *   **Strict Coupling**: Semantic retrieval quality is strictly bounded by the recall of the lexical engine.

## Decision
We will implement **Option A: Independent Channels and RRF Fusion**. 

The retrieval architecture will separate candidate generation into independent memory sources (`StmMemorySource`, `LtmMemorySource` with lexical FTS5, and `SemanticMemorySource` with IVF vector indexing), orchestrate candidate collection via the `RetrievalPipeline`, and fuse/rank them at the final stage using a dedicated Reciprocal Rank Fusion (`RrfRanking`) strategy.

This ensures maximum recall, zero vocabulary mismatch constraints, and clean, decoupled retrieval components that conform to our Domain-Driven Design (DDD) boundaries.
