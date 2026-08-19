use crate::retrieval::eval_harness::{RetrievalResult, Retriever};
use brain_core::errors::BrainError;
use brain_domain::NodeId;
use brain_storage::connection::SqliteConnectionManager;
use brain_storage::r2d2::Pool;

/// A SQLite FTS5 lexical retriever querying node_search virtual table.
pub struct FtsRetriever {
    pool: Pool<SqliteConnectionManager>,
}

impl FtsRetriever {
    /// Creates a new FtsRetriever instance.
    pub fn new(pool: Pool<SqliteConnectionManager>) -> Self {
        Self { pool }
    }
}

fn sanitize_query(query: &str) -> String {
    let terms: Vec<String> = query
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c.is_whitespace() {
                c
            } else {
                ' '
            }
        })
        .collect::<String>()
        .split_whitespace()
        .map(|s| s.to_string())
        .collect();

    terms.join(" OR ")
}

impl Retriever for FtsRetriever {
    fn retrieve(&self, query: &str) -> Result<Vec<RetrievalResult>, BrainError> {
        let conn = self.pool.get().map_err(|e| BrainError::Storage {
            message: format!("Failed to get DB connection: {}", e),
            source: Some(Box::new(e)),
        })?;

        let sanitized = sanitize_query(query);

        // If the query is empty, SQLite FTS MATCH will throw syntax error, so return empty list.
        if sanitized.is_empty() {
            return Ok(Vec::new());
        }

        // Explicit ORDER BY bm25(node_search) ASC to order by relevance.
        // We join with the standard nodes table using rowid matching.
        let mut stmt = conn
            .prepare(
                "SELECT n.id, bm25(node_search) \
                 FROM nodes n \
                 JOIN node_search ns ON n.rowid = ns.rowid \
                 WHERE node_search MATCH ?1 \
                 ORDER BY bm25(node_search) ASC",
            )
            .map_err(|e| BrainError::Storage {
                message: format!("Failed to prepare FTS search statement: {}", e),
                source: Some(Box::new(e)),
            })?;

        let rows = stmt
            .query_map([&sanitized], |row| {
                let uuid_str: String = row.get(0)?;
                let bm25_score: f64 = row.get(1)?;
                Ok((uuid_str, bm25_score))
            })
            .map_err(|e| BrainError::Storage {
                message: format!("Failed to execute FTS query: {}", e),
                source: Some(Box::new(e)),
            })?;

        let mut results = Vec::new();
        for row_res in rows {
            let (uuid_str, bm25_score) = row_res.map_err(|e| BrainError::Storage {
                message: format!("Failed to parse FTS row: {}", e),
                source: Some(Box::new(e)),
            })?;

            let uuid = uuid::Uuid::parse_str(&uuid_str).map_err(|e| BrainError::Storage {
                message: format!("Invalid UUID string in DB nodes: {}", e),
                source: Some(Box::new(e)),
            })?;

            // Negate the bm25 score so that higher scores represent better matches.
            // Under FTS5, bm25 returns a lower (more negative) value for better matches.
            // So negating it preserves this as a traditional positive relevance score.
            results.push(RetrievalResult {
                node_id: NodeId(uuid),
                channel_scores: std::collections::HashMap::from([(
                    crate::retrieval::eval_harness::RetrievalChannel::Fts,
                    -bm25_score,
                )]),
                ranking_score: None,
            });
        }

        Ok(results)
    }

    fn normalize_query(&self, query: &str) -> Option<String> {
        let cleaned = query
            .chars()
            .map(|c| {
                if c.is_alphanumeric() || c.is_whitespace() {
                    c
                } else {
                    ' '
                }
            })
            .collect::<String>()
            .to_lowercase();
        let tokens: Vec<&str> = cleaned.split_whitespace().collect();
        if tokens.is_empty() {
            None
        } else {
            Some(tokens.join(" "))
        }
    }

    fn executed_query(&self, query: &str) -> Option<String> {
        let cleaned = query
            .chars()
            .map(|c| {
                if c.is_alphanumeric() || c.is_whitespace() {
                    c
                } else {
                    ' '
                }
            })
            .collect::<String>()
            .to_lowercase();
        let tokens: Vec<&str> = cleaned.split_whitespace().collect();
        if tokens.is_empty() {
            None
        } else {
            Some(tokens.join(" OR "))
        }
    }
}
