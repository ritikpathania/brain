//! Deterministic sorting primitive with tie-breaking.

use crate::query::models::{EntityMatch, QueryOrdering, SortDirection, SortField};

/// Performs in-place stable sorting over candidates with deterministic EntityId ASC tie-breaking.
pub fn sort_matches(candidates: &mut [EntityMatch], ordering: Option<&QueryOrdering>) {
    let ordering = match ordering {
        Some(ord) => ord,
        None => return,
    };

    candidates.sort_by(|a, b| {
        let primary_cmp = match ordering.field {
            SortField::Confidence => {
                let a_val = a.average_confidence.value();
                let b_val = b.average_confidence.value();
                a_val.total_cmp(&b_val)
            }
            SortField::Degree => {
                let a_deg = a
                    .graph_metadata
                    .as_ref()
                    .map_or(0, |g| g.in_degree + g.out_degree);
                let b_deg = b
                    .graph_metadata
                    .as_ref()
                    .map_or(0, |g| g.in_degree + g.out_degree);
                a_deg.cmp(&b_deg)
            }
            SortField::Recency => {
                // Placeholder: Recency ordering will be populated in 5.3.2 once evaluator metadata exposes timestamps.
                std::cmp::Ordering::Equal
            }
        };

        let primary_cmp = match ordering.direction {
            SortDirection::Ascending => primary_cmp,
            SortDirection::Descending => primary_cmp.reverse(),
        };

        if primary_cmp != std::cmp::Ordering::Equal {
            primary_cmp
        } else {
            // Secondary tie-breaker: KnowledgeEntityId ASC
            a.entity_id.0.cmp(&b.entity_id.0)
        }
    });
}
