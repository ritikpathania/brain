//! Query AST builder and pattern models.

use crate::bkf::*;
use crate::query::filters::*;
use serde::{Deserialize, Serialize};

/// Query variable identifier.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct QueryVar(pub String);

impl QueryVar {
    /// Creates a new QueryVar.
    pub fn new(name: impl Into<String>) -> Self {
        Self(name.into())
    }

    /// Returns reference to internal string representation.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Pattern triple subject/object target.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum PatternTarget {
    /// Variable binding.
    Variable(QueryVar),
    /// Fixed entity ID.
    Entity(KnowledgeEntityId),
    /// Literal scalar value.
    Value(LiteralValue),
}

/// Graph triple pattern.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Pattern {
    /// Subject target.
    pub subject: PatternTarget,
    /// Predicate name.
    pub predicate: PredicateName,
    /// Object target.
    pub object: PatternTarget,
}

impl Pattern {
    /// Creates a new triple pattern.
    pub fn triple(subject: QueryVar, predicate: PredicateName, object: QueryVar) -> Self {
        Self {
            subject: PatternTarget::Variable(subject),
            predicate,
            object: PatternTarget::Variable(object),
        }
    }
}

/// Declarative query AST.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Query {
    /// Pattern rules.
    pub patterns: Vec<Pattern>,
    /// Filter expressions.
    pub filters: Vec<QueryFilter>,
    /// Optional limit.
    pub limit: Option<usize>,
    /// Optional offset.
    pub offset: Option<usize>,
}

impl Query {
    /// Creates a new query builder.
    pub fn builder() -> QueryBuilder {
        QueryBuilder::default()
    }
}

/// Builder for Query AST.
#[derive(Debug, Clone, Default)]
pub struct QueryBuilder {
    query: Query,
}

impl QueryBuilder {
    /// Adds a pattern.
    pub fn pattern(mut self, pattern: Pattern) -> Self {
        self.query.patterns.push(pattern);
        self
    }

    /// Adds a filter.
    pub fn filter(mut self, filter: QueryFilter) -> Self {
        self.query.filters.push(filter);
        self
    }

    /// Sets limit.
    pub fn limit(mut self, limit: usize) -> Self {
        self.query.limit = Some(limit);
        self
    }

    /// Sets offset.
    pub fn offset(mut self, offset: usize) -> Self {
        self.query.offset = Some(offset);
        self
    }

    /// Builds the query.
    pub fn build(self) -> Query {
        self.query
    }
}
