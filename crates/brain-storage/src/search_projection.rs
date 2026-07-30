//! SQLite-backed FTS5 search index projection.

use brain_core::errors::BrainError;
use brain_domain::{
    SearchDocument, SearchDocumentId, SearchDocumentKind, SearchMetadata, SessionId,
};
use rusqlite::params;
use serde_json;

/// Query processing evaluation mode.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum SearchMode {
    /// Sanitize natural language input and punctuation into safe FTS5 terms.
    #[default]
    NaturalLanguage,
    /// Pass raw FTS boolean expression directly without sanitization.
    FtsExpression,
}

/// Sanitizes natural language query text into safe SQLite FTS5 search terms.
pub fn sanitize_fts5_query(query: &str) -> String {
    let mut words = Vec::new();
    for token in query.split_whitespace() {
        let cleaned: String = token
            .chars()
            .filter(|c| {
                !matches!(
                    c,
                    '?' | '*' | ':' | '"' | '^' | '(' | ')' | '{' | '}' | '+' | '-'
                )
            })
            .collect();
        if !cleaned.is_empty() {
            words.push(format!("\"{}\"", cleaned));
        }
    }
    words.join(" ")
}

/// Structured search query.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SearchQuery {
    /// The query matching text.
    pub text: String,
    /// Query evaluation mode (defaults to NaturalLanguage).
    pub mode: SearchMode,
    /// Optional filter by document kinds.
    pub kinds: Option<Vec<SearchDocumentKind>>,
    /// Pagination limit.
    pub limit: Option<usize>,
    /// Pagination offset.
    pub offset: Option<usize>,
}

/// SQLite repository interface for mutating and querying search projections.
pub struct SqliteSearchRepository {
    pool: r2d2::Pool<crate::connection::SqliteConnectionManager>,
}

impl SqliteSearchRepository {
    /// Creates a new `SqliteSearchRepository` instance.
    pub fn new(pool: r2d2::Pool<crate::connection::SqliteConnectionManager>) -> Self {
        if let Ok(mut conn) = pool.get() {
            let _ = crate::migrations::MigrationRunner::run_migrations(&mut conn);
        }
        Self { pool }
    }

    /// Finds an indexed search document by its unique ID.
    pub fn find_by_id(&self, id: &SearchDocumentId) -> Result<Option<SearchDocument>, BrainError> {
        let conn = self.pool.get().map_err(|e| BrainError::Storage {
            message: format!("Failed to get connection: {}", e),
            source: Some(Box::new(e)),
        })?;
        self.find_by_id_conn(&conn, id)
    }

    /// Finds an indexed search document by its unique ID using a shared transaction connection.
    pub fn find_by_id_conn(
        &self,
        conn: &rusqlite::Connection,
        id: &SearchDocumentId,
    ) -> Result<Option<SearchDocument>, BrainError> {
        let mut stmt = conn
            .prepare("SELECT id, kind, title, body, metadata FROM search_projection WHERE id = ?1")
            .map_err(|e| BrainError::Storage {
                message: format!("Failed to prepare select statement: {}", e),
                source: Some(Box::new(e)),
            })?;

        let mut rows = stmt
            .query_map(params![id.as_str()], |row| {
                let id_str: String = row.get(0)?;
                let kind_str: String = row.get(1)?;
                let title: String = row.get(2)?;
                let body: String = row.get(3)?;
                let metadata_str: String = row.get(4)?;

                let id = SearchDocumentId::new(id_str);
                let quoted_kind = format!("\"{}\"", kind_str);
                let kind: SearchDocumentKind = serde_json::from_str(&quoted_kind).map_err(|e| {
                    rusqlite::Error::FromSqlConversionFailure(
                        1,
                        rusqlite::types::Type::Text,
                        Box::new(e),
                    )
                })?;

                let metadata: SearchMetadata =
                    serde_json::from_str(&metadata_str).map_err(|e| {
                        rusqlite::Error::FromSqlConversionFailure(
                            4,
                            rusqlite::types::Type::Text,
                            Box::new(e),
                        )
                    })?;

                Ok(SearchDocument::new(id, kind, title, body, metadata))
            })
            .map_err(|e| BrainError::Storage {
                message: format!("Failed to query search document: {}", e),
                source: Some(Box::new(e)),
            })?;

        if let Some(row_res) = rows.next() {
            let doc = row_res.map_err(|e| BrainError::Storage {
                message: format!("Failed to parse search row: {}", e),
                source: Some(Box::new(e)),
            })?;
            Ok(Some(doc))
        } else {
            Ok(None)
        }
    }

    /// Saves a search document (inserts or updates the FTS5 virtual table).
    pub fn save(&self, doc: &SearchDocument, sequence: u64) -> Result<(), BrainError> {
        let conn = self.pool.get().map_err(|e| BrainError::Storage {
            message: format!("Failed to get connection: {}", e),
            source: Some(Box::new(e)),
        })?;
        self.save_conn(&conn, doc, sequence)
    }

    /// Saves a search document using a shared transaction connection.
    pub fn save_conn(
        &self,
        conn: &rusqlite::Connection,
        doc: &SearchDocument,
        sequence: u64,
    ) -> Result<(), BrainError> {
        let id_str = doc.id.as_str();

        // Delete existing document first to ensure we replace it and avoid duplicates in FTS5
        conn.execute(
            "DELETE FROM search_projection WHERE id = ?1",
            params![id_str],
        )
        .map_err(|e| BrainError::Storage {
            message: format!("Failed to delete existing search document: {}", e),
            source: Some(Box::new(e)),
        })?;

        let kind_str = serde_json::to_string(&doc.kind)
            .unwrap_or_default()
            .trim_matches('"')
            .to_string();
        let metadata_str =
            serde_json::to_string(&doc.metadata).map_err(|e| BrainError::Storage {
                message: format!("Failed to serialize search metadata: {}", e),
                source: Some(Box::new(e)),
            })?;

        let session_id_opt = match &doc.metadata {
            brain_domain::SearchMetadata::Message { session_id, .. } => {
                Some(session_id.0.to_string())
            }
            _ => None,
        };

        conn.execute(
            "INSERT INTO search_projection (id, kind, title, body, metadata, updated_sequence, session_id) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![id_str, kind_str, doc.title, doc.body, metadata_str, sequence, session_id_opt],
        )
        .map_err(|e| BrainError::Storage {
            message: format!("Failed to save search document: {}", e),
            source: Some(Box::new(e)),
        })?;

        Ok(())
    }

    /// Deletes a search document from the FTS5 virtual table.
    pub fn delete(&self, id: &SearchDocumentId) -> Result<(), BrainError> {
        let conn = self.pool.get().map_err(|e| BrainError::Storage {
            message: format!("Failed to get connection: {}", e),
            source: Some(Box::new(e)),
        })?;
        self.delete_conn(&conn, id)
    }

    /// Deletes a search document from the FTS5 virtual table using a shared transaction connection.
    pub fn delete_conn(
        &self,
        conn: &rusqlite::Connection,
        id: &SearchDocumentId,
    ) -> Result<(), BrainError> {
        conn.execute(
            "DELETE FROM search_projection WHERE id = ?1",
            params![id.as_str()],
        )
        .map_err(|e| BrainError::Storage {
            message: format!("Failed to delete search document: {}", e),
            source: Some(Box::new(e)),
        })?;

        Ok(())
    }

    /// Searches matching candidate documents from FTS5 index.
    pub fn search(&self, query: &SearchQuery) -> Result<Vec<SearchDocument>, BrainError> {
        let conn = self.pool.get().map_err(|e| BrainError::Storage {
            message: format!("Failed to get connection: {}", e),
            source: Some(Box::new(e)),
        })?;

        let search_text = match query.mode {
            SearchMode::NaturalLanguage => sanitize_fts5_query(&query.text),
            SearchMode::FtsExpression => query.text.clone(),
        };

        if search_text.trim().is_empty() {
            return Ok(Vec::new());
        }

        let mut sql = "SELECT id, kind, title, body, metadata FROM search_projection WHERE search_projection MATCH ?1".to_string();
        let mut params_vec: Vec<rusqlite::types::Value> =
            vec![rusqlite::types::Value::Text(search_text)];

        let mut param_index = 2;
        if let Some(ref kinds) = query.kinds {
            if !kinds.is_empty() {
                sql.push_str(" AND kind IN (");
                let placeholders: Vec<String> = kinds
                    .iter()
                    .map(|k| {
                        let k_str = serde_json::to_string(k)
                            .unwrap_or_default()
                            .trim_matches('"')
                            .to_string();
                        params_vec.push(rusqlite::types::Value::Text(k_str));
                        let ph = format!("?{}", param_index);
                        param_index += 1;
                        ph
                    })
                    .collect();
                sql.push_str(&placeholders.join(", "));
                sql.push(')');
            }
        }

        if let Some(limit) = query.limit {
            sql.push_str(&format!(" LIMIT ?{}", param_index));
            params_vec.push(rusqlite::types::Value::Integer(limit as i64));
            param_index += 1;
        }

        if let Some(offset) = query.offset {
            sql.push_str(&format!(" OFFSET ?{}", param_index));
            params_vec.push(rusqlite::types::Value::Integer(offset as i64));
        }

        let mut stmt = conn.prepare(&sql).map_err(|e| BrainError::Storage {
            message: format!("Failed to prepare search statement: {}", e),
            source: Some(Box::new(e)),
        })?;

        let params_ref: Vec<&dyn rusqlite::ToSql> = params_vec
            .iter()
            .map(|v| v as &dyn rusqlite::ToSql)
            .collect();

        let rows = stmt
            .query_map(&params_ref[..], |row| {
                let id_str: String = row.get(0)?;
                let kind_str: String = row.get(1)?;
                let title: String = row.get(2)?;
                let body: String = row.get(3)?;
                let metadata_str: String = row.get(4)?;

                let id = SearchDocumentId::new(id_str);
                let quoted_kind = format!("\"{}\"", kind_str);
                let kind: SearchDocumentKind = serde_json::from_str(&quoted_kind).map_err(|e| {
                    rusqlite::Error::FromSqlConversionFailure(
                        1,
                        rusqlite::types::Type::Text,
                        Box::new(e),
                    )
                })?;

                let metadata: SearchMetadata =
                    serde_json::from_str(&metadata_str).map_err(|e| {
                        rusqlite::Error::FromSqlConversionFailure(
                            4,
                            rusqlite::types::Type::Text,
                            Box::new(e),
                        )
                    })?;

                Ok(SearchDocument::new(id, kind, title, body, metadata))
            })
            .map_err(|e| BrainError::Storage {
                message: format!("Failed to execute search query: {}", e),
                source: Some(Box::new(e)),
            })?;

        let mut docs = Vec::new();
        for row_res in rows {
            docs.push(row_res.map_err(|e| BrainError::Storage {
                message: format!("Failed to parse search row: {}", e),
                source: Some(Box::new(e)),
            })?);
        }

        Ok(docs)
    }

    /// Clears all entries in the search projection.
    pub fn clear_all(&self) -> Result<(), BrainError> {
        let conn = self.pool.get().map_err(|e| BrainError::Storage {
            message: format!("Failed to get connection: {}", e),
            source: Some(Box::new(e)),
        })?;
        self.clear_all_conn(&conn)
    }

    /// Clears all entries in the search projection using a shared transaction connection.
    pub fn clear_all_conn(&self, conn: &rusqlite::Connection) -> Result<(), BrainError> {
        conn.execute("DELETE FROM search_projection", [])
            .map_err(|e| BrainError::Storage {
                message: format!("Failed to clear search projection: {}", e),
                source: Some(Box::new(e)),
            })?;
        Ok(())
    }

    /// Deletes all indexed messages that belong to a specific session.
    pub fn delete_by_session_id(&self, session_id: &SessionId) -> Result<(), BrainError> {
        let conn = self.pool.get().map_err(|e| BrainError::Storage {
            message: format!("Failed to get connection: {}", e),
            source: Some(Box::new(e)),
        })?;
        self.delete_by_session_id_conn(&conn, session_id)
    }

    /// Deletes all indexed messages that belong to a specific session using a shared transaction connection.
    pub fn delete_by_session_id_conn(
        &self,
        conn: &rusqlite::Connection,
        session_id: &SessionId,
    ) -> Result<(), BrainError> {
        let sess_str = session_id.0.to_string();
        let like_pattern = format!("%\"session_id\":\"{}\"%", sess_str);
        conn.execute(
            "DELETE FROM search_projection WHERE kind = 'message' AND (session_id = ?1 OR (session_id IS NULL AND metadata LIKE ?2))",
            params![sess_str, like_pattern],
        )
        .map_err(|e| BrainError::Storage {
            message: format!("Failed to delete messages by session: {}", e),
            source: Some(Box::new(e)),
        })?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_fts5_query_matrix() {
        // Natural question with ?
        assert_eq!(
            sanitize_fts5_query("What database do I use?"),
            "\"What\" \"database\" \"do\" \"I\" \"use\""
        );

        // FTS operators
        assert_eq!(sanitize_fts5_query("NOT AND OR"), "\"NOT\" \"AND\" \"OR\"");

        // Special punctuation (?, *, :, ", (), {}, +, -)
        assert_eq!(
            sanitize_fts5_query("server: \"Ubuntu\" * (test)+"),
            "\"server\" \"Ubuntu\" \"test\""
        );

        // Unicode and Emojis
        assert_eq!(
            sanitize_fts5_query("PostgreSQL 🐘 🚀"),
            "\"PostgreSQL\" \"🐘\" \"🚀\""
        );

        // Empty / Whitespace
        assert_eq!(sanitize_fts5_query("   "), "");
    }
}
