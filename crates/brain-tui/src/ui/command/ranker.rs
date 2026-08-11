//! Deterministic ranking and tie-breaking engine for command candidate matches.

use crate::ui::command::matcher::CandidateMatch;
use std::cmp::Ordering;

/// Ranks candidate matches deterministically.
pub struct CommandRanker;

impl CommandRanker {
    /// Sorts candidate matches in-place according to weighted score and stable tie-breaking rules:
    /// 1. score DESC
    /// 2. category ASC
    /// 3. name ASC
    pub fn rank(candidates: &mut [CandidateMatch<'_>]) {
        candidates.sort_by(|a, b| {
            let score_a = a.factors.score();
            let score_b = b.factors.score();

            let cmp_score = score_b.cmp(&score_a); // score DESC
            if cmp_score != Ordering::Equal {
                return cmp_score;
            }

            let cmp_priority = b.metadata.priority.cmp(&a.metadata.priority); // priority DESC
            if cmp_priority != Ordering::Equal {
                return cmp_priority;
            }

            let cmp_category = (a.metadata.category as u8).cmp(&(b.metadata.category as u8));
            if cmp_category != Ordering::Equal {
                return cmp_category;
            }

            a.metadata.name.cmp(b.metadata.name)
        });
    }
}
