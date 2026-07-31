//! Confidence threshold filtering primitive.

use crate::query::models::{ConfidenceFilter, EntityMatch};

/// Filters candidates against a ConfidenceFilter threshold, preserving relative candidate ordering.
pub fn filter_by_confidence(candidates: &mut Vec<EntityMatch>, filter: Option<&ConfidenceFilter>) {
    if let Some(conf_filter) = filter {
        let min_val = conf_filter.min_confidence.value();
        candidates.retain(|item| item.average_confidence.value() >= min_val);
    }
}
