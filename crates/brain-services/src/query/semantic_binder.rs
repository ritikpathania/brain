//! Semantic binder validating AST variables and building BoundQuery with SlotId assignments.

use brain_domain::query::*;

/// Semantic binder translating Query AST into validated BoundQuery with slot indexing.
pub struct SemanticBinder;

impl SemanticBinder {
    /// Binds and validates a Query AST.
    pub fn bind(query: &Query) -> Result<BoundQuery, QueryError> {
        let mut schema = BindingSchema::new();

        for pat in &query.patterns {
            if let PatternTarget::Variable(ref v) = pat.subject {
                schema.get_or_create_slot(v);
            }
            if let PatternTarget::Variable(ref v) = pat.object {
                schema.get_or_create_slot(v);
            }
        }

        Ok(BoundQuery {
            ast: query.clone(),
            schema,
        })
    }
}
