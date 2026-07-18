use crate::query::dto::SearchSummary;
use crate::query::filters::SearchQuery;
use crate::query::traits::SearchQueryService;
use brain_core::errors::BrainError;
use brain_domain::SearchDocument;
use brain_storage::{SearchQuery as StorageSearchQuery, SqliteSearchRepository};
use std::sync::Arc;

/// Concrete implementation of `SearchQueryService` backing by Sqlite FTS5 search index.
pub struct SqliteSearchQueryService {
    repo: Arc<SqliteSearchRepository>,
}

impl SqliteSearchQueryService {
    /// Creates a new `SqliteSearchQueryService` instance.
    pub fn new(repo: Arc<SqliteSearchRepository>) -> Self {
        Self { repo }
    }
}

// Module-local mapper functions to map database projection models to Query DTOs.
fn map_to_summary(doc: SearchDocument) -> SearchSummary {
    SearchSummary {
        id: doc.id,
        kind: doc.kind,
        title: doc.title,
        body: doc.body,
        metadata: doc.metadata,
    }
}

impl SearchQueryService for SqliteSearchQueryService {
    fn search(&self, query: SearchQuery) -> Result<Vec<SearchSummary>, BrainError> {
        let storage_query = StorageSearchQuery {
            text: query.text,
            kinds: query.kinds,
            limit: query.pagination.as_ref().and_then(|p| p.limit),
            offset: query.pagination.as_ref().and_then(|p| p.offset),
        };
        let docs = self.repo.search(&storage_query)?;

        let mut summaries = Vec::new();
        for doc in docs {
            summaries.push(map_to_summary(doc));
        }

        Ok(summaries)
    }
}
