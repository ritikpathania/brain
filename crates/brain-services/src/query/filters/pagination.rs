//! Safe offset/limit candidate slicing primitive.

use crate::query::models::{EntityMatch, PaginationParams};

/// Applies limit and offset slicing to candidates, returning the paginated items and total matched count.
pub fn paginate_matches(
    candidates: &[EntityMatch],
    pagination: &PaginationParams,
) -> (Vec<EntityMatch>, usize) {
    let total_matched = candidates.len();
    if pagination.limit == 0 || pagination.offset >= total_matched {
        return (vec![], total_matched);
    }

    let end = pagination
        .offset
        .saturating_add(pagination.limit)
        .min(total_matched);
    let paginated = candidates[pagination.offset..end].to_vec();
    (paginated, total_matched)
}
