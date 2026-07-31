# Phase 5.3.3 — Lexical Search Evaluator Design Specification

**Status:** Approved  
**Author:** AI Pair Programmer & User  
**Date:** 2026-07-31  
**Crate Target:** `crates/brain-services` (`src/query/evaluators/search.rs`)

---

## 1. Executive Summary & Semantic Contract

Phase 5.3.3 implements the **`SearchEvaluator`** (`brain-services::query::evaluators::search`). The evaluator tokenizes query text using domain `SearchToken::tokenize`, deduplicates query tokens, performs posting list lookups against `snapshot.search()`, computes per-entity matched tokens and initial lexical relevance scores (`matched_tokens.len() as f64`), enriches candidates with `SearchMetadata`, `GraphMetadata`, and entity statistics, and passes candidates through our canonical query processing pipeline.

### Core Architectural Invariants:
1. **Tokenization & Immediate Deduplication**: Tokenization uses domain `SearchToken::tokenize(&query.query_text)` and deduplicates query tokens immediately before posting lookups.
2. **Per-Entity Matched Tokens**: `SearchMetadata.matched_tokens` records only the unique query tokens that *actually matched* that specific entity, rather than echoing the full query.
3. **Initial Lexical Scoring Heuristic**: Candidate `SearchMetadata.score` is calculated as `matched_tokens.len() as f64`, providing a transparent lexical relevance score.
4. **Idempotent Empty Query Handling**: Empty, whitespace-only, or punctuation-only queries producing zero valid tokens return `Ok(QueryFacadeResult)` with `matches = []` and `total_matched = 0`.
5. **Deterministic Fallback Invariant**: If entity statistics are absent, `active_facts_count` defaults to `0` and `average_confidence` defaults to `Confidence::new(0.0).unwrap()`.
6. **Canonical Pipeline Execution**:
   ```text
   Lexical Posting Retrieval ──► Per-Entity Match Enrichment ──► Temporal Filter ──► Confidence Filter ──► Deterministic Sort ──► Pagination
   ```

---

## 2. Evaluation Algorithm & Flow

```rust
use crate::query::errors::*;
use crate::query::filters::*;
use crate::query::models::*;
use crate::query::snapshot::ProjectionSnapshot;
use brain_domain::bkf::*;
use brain_domain::projection::graph_adjacency::GraphNodeId;
use brain_domain::projection::search_index::SearchToken;
use brain_domain::EntityId;
use std::collections::HashSet;

pub struct SearchEvaluator;

impl SearchEvaluator {
    /// Evaluates lexical search query against projection snapshot.
    pub fn evaluate(
        snapshot: &ProjectionSnapshot,
        query: &LexicalSearchQuery,
    ) -> Result<QueryFacadeResult, QueryError> {
        let mut tokens = SearchToken::tokenize(&query.query_text);
        if tokens.is_empty() {
            return Ok(QueryFacadeResult {
                matches: vec![],
                total_matched: 0,
                metadata: QueryResponseMetadata {
                    execution_duration_us: 0,
                    snapshot_watermark: snapshot.watermark().0,
                },
            });
        }

        // Deduplicate query tokens
        let mut seen_tokens = HashSet::new();
        tokens.retain(|t| seen_tokens.insert(t.as_str().to_string()));

        // 1. Lexical Candidate Retrieval
        let discovered_entities = snapshot.search().search_entities(&tokens);
        if discovered_entities.is_empty() {
            return Ok(QueryFacadeResult {
                matches: vec![],
                total_matched: 0,
                metadata: QueryResponseMetadata {
                    execution_duration_us: 0,
                    snapshot_watermark: snapshot.watermark().0,
                },
            });
        }

        // 2. Metadata Enrichment & Per-Entity Match Calculation
        let mut candidates = Vec::with_capacity(discovered_entities.len());
        for entity_id in discovered_entities {
            let node_id = GraphNodeId(EntityId(entity_id.0));
            let degree = snapshot.graph().degree(&node_id);
            let stats = snapshot.statistics().get(&entity_id);

            let active_facts_count = stats.map_or(0, |s| s.active_facts_count);
            let average_confidence = stats.map_or(
                Confidence::new(0.0).unwrap(),
                |s| Confidence::new(s.average_confidence()).unwrap_or_else(|_| Confidence::new(0.0).unwrap()),
            );

            let satisfies_temporal = match query.temporal_mode {
                TemporalMode::CurrentActive => active_facts_count > 0 || stats.is_none(),
                TemporalMode::ValidAt(at_ts) => !snapshot.temporal().facts_at(&entity_id, at_ts).is_empty(),
                TemporalMode::AllHistorical => true,
            };

            if satisfies_temporal {
                // Find actual matching tokens for this entity
                let mut matched_tokens = Vec::new();
                for token in &tokens {
                    let token_entities = snapshot.search().search_entities(&[token.clone()]);
                    if token_entities.contains(&entity_id) {
                        matched_tokens.push(token.as_str().to_string());
                    }
                }

                let score = matched_tokens.len() as f64;

                candidates.push(EntityMatch {
                    entity_id,
                    active_facts_count,
                    average_confidence,
                    graph_metadata: Some(GraphMetadata {
                        in_degree: degree.in_degree,
                        out_degree: degree.out_degree,
                    }),
                    search_metadata: Some(SearchMetadata {
                        matched_tokens,
                        score,
                    }),
                });
            }
        }

        // 3. Pipeline Execution (Confidence Filter -> Ordering -> Paginate)
        filter_by_confidence(&mut candidates, query.confidence_filter.as_ref());

        let ordering = query.ordering.clone().unwrap_or_else(|| QueryOrdering {
            field: SortField::Confidence,
            direction: SortDirection::Descending,
        });
        sort_matches(&mut candidates, Some(&ordering));
        let (paginated, total_matched) = paginate_matches(&candidates, &query.pagination);

        Ok(QueryFacadeResult {
            matches: paginated,
            total_matched,
            metadata: QueryResponseMetadata {
                execution_duration_us: 0,
                snapshot_watermark: snapshot.watermark().0,
            },
        })
    }
}
```

---

## 3. Verification & Testing Strategy

1. **Unit & Contract Tests (`crates/brain-services/tests/search_evaluator_tests.rs`)**:
   - `test_search_empty_and_whitespace_query`: Verifies empty or whitespace query returns empty matches without error.
   - `test_search_single_and_multi_token_match`: Verifies lexical token matching and `SearchMetadata` score computation.
   - `test_search_duplicate_query_tokens`: Verifies `"rust rust rust"` is deduplicated before posting lookup.
   - `test_search_partial_token_matches`: Verifies partial vs full matching tokens generate distinct `matched_tokens` and `score` values.
   - `test_search_no_statistics_fallback`: Verifies indexed entity with missing statistics receives `active_facts_count = 0` and `average_confidence = 0.0`.
   - `test_search_temporal_and_confidence_filter`: Verifies candidate temporal filtering and confidence thresholding.
   - `test_search_ordering_and_pagination`: Verifies candidate ordering and pagination slicing.
