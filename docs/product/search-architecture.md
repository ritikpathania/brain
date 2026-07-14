# Search & Retrieval Engine: Product & Pipeline Design

This document defines the product requirements, pipeline architecture, and evaluation framework for the Retrieval Engine (Phase P1) and Ranking Engine (Phase P2).

---

## 1. Quality Strategy: Recall vs. Precision

Retrieval and ranking are optimized as distinct stages with different mathematical goals:

```text
       Candidate Generation                   Multi-Stage Ranking
                │                                      │
                ▼                                      ▼
         Maximize RECALL                       Maximize PRECISION
  (Gather all possibly relevant nodes)      (Ensure the best order of context)
```

1.  **Candidate Generation (Recall-Focused)**: The search channels act as independent, parallel *competitors*. FTS, Semantic, and Metadata retrieval operate in isolation and do not share state. The union phase collects candidates to avoid missing any relevant context.
2.  **Ranking (Precision-Focused)**: Sorts and filters the consolidated union of candidates using a feature-based scoring engine to place the highest-utility memories at the top.

---

## 2. Candidate Provenance & Feature Separation

To keep candidate generation decoupled from ranking, each retrieved candidate carries a stable **Candidate Provenance** structure. This describes *how* the candidate was reached and its raw scores *before* feature extraction.

```text
Candidate Provenance
├── NodeId
├── RetrievalSources (Flags: FTS, Semantic, Metadata)
├── RawScores
│   ├── Lexical (Raw BM25 score)
│   ├── Semantic (Raw cosine similarity)
│   └── Metadata (Category or tag matching score)
└── FeatureVector (Immutable; populated during ranking)
```

This ensures retrieval-specific metadata (provenance) is tracked at the retrieval boundary, making explainability straightforward without requiring re-computation during ranking.

---

## 3. The Retrieval, Ranking & Context Pipeline

The retrieval and ranking flow runs sequentially through the following pipeline:

```mermaid
graph TD
    Query[User Query] --> Intent[Intent Extraction]
    Intent --> Gen[Candidate Retrieval]
    
    subgraph Candidate Retrieval (Recall)
        Gen --> FTS[FTS / BM25 Channel]
        Gen --> Vector[Semantic Embedding Channel]
        Gen --> Meta[Metadata Channel]
    end
    
    FTS --> Union[Candidate Union & Deduplication]
    Vector --> Union
    Meta --> Union
    
    Union --> Expand[Candidate Expansion]
    Expand --> Feature[Feature Extraction]
    
    subgraph Multi-Stage Ranking (Precision)
        Feature --> Rank[Feature-Based Ranking Engine]
    end
    
    Rank --> Assembly[Context Assembly]
    
    subgraph Context Assembly & Enrichment
        Assembly --> Dedupe[Redundancy / Chunk Fusion]
        Dedupe --> Reflect[Reflection / Neighbor Expansion]
    end
    
    Reflect --> Final[Final Context Output + Diagnostics]
```

### Stage 1: Intent Extraction
Parses user queries for implicit parameters (temporal constraints, tags, categories, target limits).

### Stage 2: Candidate Retrieval (Recall)
Parallel, competing search channels:
*   **Lexical (FTS/BM25)**: Querying text index structures.
*   **Semantic (Embeddings)**: Computing nearest neighbors in vector space.
*   **Metadata**: Extracting specific node categories, tags, or properties.

### Stage 3: Candidate Union & Deduplication
Merges candidate sets from all channels, tracks **Candidate Provenance**, and deduplicates based on `NodeId`.

### Stage 4: Candidate Expansion
Inspects immediate topological neighbors of candidate nodes in the graph to add potential relational context before scoring.

### Stage 5: Feature Extraction & Ranking (Precision)
The ranking engine decouples the definition of features from the scoring logic itself. Feature extraction produces an **immutable Feature Vector** for each candidate node.

#### Immutable Feature Vector Attributes:
*   **Semantic Similarity**: Cosine similarity to the query vector.
*   **Lexical Similarity**: Normalized BM25 matching score.
*   **Recency (Temporal)**: Time elapsed since creation or last access.
*   **Importance / Pinning**: Manual weight indicating fixed priority.
*   **Provenance Confidence**: Reliability score of the origin source (e.g. user input vs. automated web scraper).
*   **Graph Centrality**: Number of active edges (relationships) connected to this node.
*   **Relationship Distance**: Shortest path to current active focus nodes.
*   **Access Frequency**: Historical frequency of retrieval.

Once computed, the feature vector is passed as an immutable input to the ranking algorithm. This ensures that the ranking heuristics or models can be modified, A/B tested, or replaced without altering the feature extraction logic.

### Stage 6: Context Assembly & Enrichment (Reflection)
*   **Deduplication & Fusion**: Condenses similar or overlapping memory text.
*   **Reflection (Context Enrichment)**: Expands the context around top-ranked nodes by pulling in immediate graph neighbors. *Note: Reflection does not alter the relative order of the top-ranked nodes; it merely enriches the context to improve final output comprehension.*
*   **Diagnostics Assembly**: Formulates metadata detailing the match reasons and latency timings.

---

## 4. Explainability

To build trust, every retrieved memory returned by the engine includes a structured explainability block stating why it was matched.

```json
{
  "node_id": "mem_9083a21",
  "text": "Using UdsTransport for typescript clients...",
  "match_explanation": {
    "reasons": [
      "High semantic similarity (92%) to query",
      "Manually pinned by user",
      "Connected to active node: typescript_sdk"
    ]
  }
}
```

---

## 5. Diagnostics & Latency Budgets

We explicitly separate runtime diagnostics (produced per query for debugging/production monitoring) from evaluation metrics (used strictly during benchmarks).

### Runtime Diagnostics (Per-Query)
*   **Latency Breakdown**: Milliseconds spent in retrieval, ranking, and context assembly.
*   **Source Metrics**: Candidates returned per search channel.
*   **Union Size**: Size of the unique candidate pool.
*   **Explanation**: Reasons explaining why the node was selected.

### Performance & Latency Targets
Targets are measured under a database size of $10,000$ memories:

| Percentile | Candidate Retrieval | Ranking & Scoring | Assembly & Enrichment | Total End-to-End |
| :--- | :--- | :--- | :--- | :--- |
| **P50 (Median)** | $\le 8\text{ ms}$ | $\le 4\text{ ms}$ | $\le 2\text{ ms}$ | $\le 14\text{ ms}$ |
| **P95 (Tail)** | $\le 15\text{ ms}$ | $\le 10\text{ ms}$ | $\le 5\text{ ms}$ | $\le 30\text{ ms}$ |
| **P99 (Max Jitter)**| $\le 25\text{ ms}$ | $\le 18\text{ ms}$ | $\le 10\text{ ms}$ | $\le 53\text{ ms}$ |

---

## 6. Retrieval Evaluation Framework

Used strictly during benchmark runs to determine if changes to search logic improve retrieval quality.

### Evaluation Directory Structure
```text
crates/brain-services/tests/evaluation/
    ├── queries.json         # Evaluation queries and intent expectations
    └── ground_truth.json    # Target memories expected per query (gold standard)
```

### Evaluation Metrics
*   **Recall@K**: The proportion of relevant memories retrieved within the top $K$ results.
*   **Precision@K**: The proportion of retrieved memories that are relevant.
*   **Mean Reciprocal Rank (MRR)**: Average reciprocal rank of the first relevant memory.
*   **Normalized Discounted Cumulative Gain (nDCG)**: Measures ranking quality by discounting relevance based on position.

### Benchmark Corpus Diversity
The `queries.json` corpus is kept intentionally small (20-30 carefully curated queries), focusing on quality and covering key challenging operational edge cases:
1.  **Happy Path**: Direct query match.
2.  **Typo Tolerance**: Query contains spelling errors.
3.  **Partial Match**: Only fragment of node names or properties provided.
4.  **Ambiguous Concepts**: Broad terms requiring semantic resolution.
5.  **Broad vs. Narrow Queries**: Assessing limits and expansion parameters.
6.  **Temporal Queries**: Filters using relative time descriptors ("yesterday", "last week").
7.  **Entity-Only**: Querying directly for tag names or structural connections.

---

## 7. Implementation Milestones & Boundaries

### 🚀 Milestone P1.1: Lexical Search & Evaluation Foundation

#### Scope Boundaries (What NOT to build yet)
*   No embeddings or vector database queries.
*   No graph traversal or node connection parsing.
*   No multi-stage reranking or scoring models.
*   No temporal scoring or decay.
*   No reflection/neighbor expansion.

#### Small Implementation Steps:
1.  **Step 1: Evaluation Harness**: Establish `queries.json` and `ground_truth.json` files, implement Recall@K and MRR computation, and build a deterministic test runner. No retrieval code changes.
2.  **Step 2: Pure FTS Retrieval**: Implement isolated SQLite FTS retrieval returning a raw candidate list with no ranking. Verify harness metrics.
3.  **Step 3: Runtime Diagnostics**: Add query metrics (latency, candidate count, query plan, source, timing) recorded on every query.
4.  **Step 4: Benchmark Suite**: Connect the evaluation corpus and FTS query executor to output a deterministic baseline report.

#### Success Criteria
*   The benchmark corpus exists and is version-controlled.
*   Every benchmark produces deterministic Recall@K and MRR.
*   Every query emits runtime diagnostics.
*   FTS retrieval stays within the documented latency budget.
*   Future retrieval implementations can be compared against this baseline without changing the evaluation framework.

---

### 🚀 Milestone P1.2: Semantic Integration & Candidate Union
*   Implement isolated semantic embedding search channel.
*   Build the thread-safe `Candidate Union & Deduplication` stage, tracking `Candidate Provenance`.
*   Evaluate combined lexical/semantic recall improvements against the evaluation dataset.

### 🚀 Milestone P1.3: Feature Extraction & Ranking Engine
*   Implement the feature extraction layer producing an immutable Feature Vector.
*   Build the modular ranking engine consuming these features.
*   Verify precision gains using MRR and nDCG changes from the evaluation test suite.

### 🚀 Milestone P1.4: Context Enrichment & Reflection
*   Implement redundancy deduplication and chunk fusion.
*   Build reflection-based context enrichment (topological neighbor expansion).
*   Add explainability outputs detailing match reasons.
*   Validate end-to-end latency, variance (P50/P95/P99), and precision boundaries.
