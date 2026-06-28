# Storage Backend Subsystem

The storage subsystem separates Transaction Processing (OLTP) from Analytical Querying (OLAP) to achieve sub-millisecond query latencies.

## 1. SQLite OLTP Store (`src/storage/sqlite/`)

SQLite stores nodes and relationship edges under Write-Ahead Logging (WAL) mode.

### Schema
* **`nodes`**: `id (TEXT PRIMARY KEY)`, `label (TEXT)`, `type (TEXT)`, `properties (TEXT JSON)`, `updated_at (INTEGER)`
* **`edges`**: `source (TEXT)`, `target (TEXT)`, `relation (TEXT)`, `weight (REAL DEFAULT 1.0)`, `updated_at (INTEGER)`, referencing nodes.

### Mathematical Decay & Reinforcement
When edge updates occur, the edge weight is reinforced by adding $+0.5$ (capped at $2.0$). 
During background consolidation passes, relationship weights are dampended using half-life decay:

\[W_{\text{new}} = W_{\text{old}} \times e^{-\lambda \Delta t}\]

where \(\lambda = \frac{\ln(2)}{T_{1/2}}\). Edges dropping below the threshold weight (default: $0.1$) are pruned.

---

## 2. DuckDB OLAP Store (`src/storage/duckdb/`)

DuckDB manages diagnostic logs and analytical graph queries (node type counts, degree centralities, similarities, and p50/p95/p99 query latency percentiles).

Data is synchronized from SQLite incrementally using watermark timestamps tracked in DuckDB's `sync_metadata`.
