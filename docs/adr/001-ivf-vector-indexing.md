# ADR 001: IVF Vector Indexing in SQLite

## Status
Proposed (Milestone 2.2)

## Context
High-dimensional semantic embeddings (384 dimensions) are currently stored in the SQLite `embeddings` table. Querying nearest neighbors requires a brute-force flat scan (computing cosine similarity between the query vector and all records in the database). While this is highly accurate (100% recall), its latency and CPU utilization scale as $O(N)$ where $N$ is the number of active node embeddings. For large knowledge graphs, this becomes a performance bottleneck.

We need a low-risk, embedded-friendly, approximate nearest neighbor (ANN) search optimization that can be implemented cleanly within our pure Rust/SQLite architecture without relying on heavy external vector database engines or unstable FFI integrations.

## Decision
We will implement an Inverted File (IVF) index directly in SQLite using a **deterministic predefined centroid strategy**.

### 1. Predefined Centroids
Instead of running a heavy dynamic K-Means clustering algorithm on the database, we partition the 384-dimensional unit hypersphere using **8 predefined orthogonal-like centroids** generated deterministically at runtime.

The centroid vectors are generated using varying frequency sinusoidal wave patterns:
\[v_c[i] = \sin\left(\frac{2\pi \cdot (i+1) \cdot (c+1)}{384}\right) \quad \text{for } c \in [0, 7], \, i \in [0, 383]\]
Each centroid vector is then normalized to unit length:
\[\hat{v}_c = \frac{v_c}{\|v_c\|}\]

This ensures that:
- Centroid vectors are 100% deterministic and reproducible across compiles, restarts, and systems.
- No database table is needed to persist the centroid vectors themselves.

### 2. Schema Adjustments
We add a `centroid_id` column to the `embeddings` table:
```sql
ALTER TABLE embeddings ADD COLUMN centroid_id INTEGER;
CREATE INDEX IF NOT EXISTS idx_embeddings_centroid ON embeddings(centroid_id);
```

### 3. Clustering & Routing Logic
- **Write Path (Indexing)**:
  When an embedding is inserted/updated, we compute the dot product (cosine similarity) between the normalized embedding vector and all 8 predefined centroids. The ID of the closest centroid ($c$ with the highest dot product) is stored in the `centroid_id` column.
- **Read Path (Query Probing)**:
  1. Count the total number of records in the `embeddings` table.
  2. If the count is **less than 50**, bypass indexing and perform a flat scan across all vectors to guarantee 100% recall.
  3. If the count is **50 or more**, compute the cosine similarity between the query vector and all 8 centroids.
  4. Select the top **$P = 2$ closest centroids** (probe size).
  5. Query only the subset of embeddings where `centroid_id` matches one of the top 2 centroids:
     ```sql
     SELECT node_id, vector FROM embeddings WHERE centroid_id IN (?1, ?2)
     ```
  6. Compute final cosine similarities and rank candidates only within this partitioned subset.

## Consequences & Trade-Offs

### Pros
- **Significant Performance Gains**: Reduces the similarity scan space by 75% for large datasets, directly translating to proportional CPU and latency reductions.
- **Zero Heavy External Dependencies**: Runs entirely inside vanilla SQLite and Rust standard library, maintaining the zero FFI-coupling invariant.
- **Low Memory Overhead**: Precomputed centroids are small (8 vectors of 384 floats = 12 KB).
- **Graceful Degradation**: Small datasets retain 100% recall via the scale-activation threshold ($\ge 50$).

### Cons
- **Approximate Search (Recall Loss)**: Because the partitioning is fixed, vectors near cluster boundaries might be missed if their closest centroid is not probed. Selecting a probe size of $P = 2$ (25% probe space) balances this trade-off to keep recall $\ge 90\%$ for most semantic scopes.
- **Centroid Balance**: Sinusoidal centroids assume a relatively uniform distribution of embeddings. Highly clustered domain data could partition unevenly, though the small dataset activation limit mitigates this.

## Future Evolution
1. **Dynamic K-Means**: If datasets exceed 10,000 nodes, transition to a background task that periodically runs K-Means clustering on the active dataset to compute and save optimized, data-adapted centroids.
2. **`sqlite-vec` Integration**: Once the native `sqlite-vec` extension stabilizes its compilation toolchain for cross-platform Rust builds, migrate the underlying indexing to its virtual tables.
