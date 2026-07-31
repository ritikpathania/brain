//! Half-open temporal validity interval filtering primitive.

use brain_domain::bkf::Timestamp;

/// Evaluates if a half-open validity interval `[valid_from, valid_until)` satisfies a target `Timestamp` query.
/// Inclusive lower bound (`valid_from <= target_time`), exclusive upper bound (`valid_until > target_time`), `None` = unbounded future.
pub fn is_valid_at(
    valid_from: Timestamp,
    valid_until: Option<Timestamp>,
    target_time: Timestamp,
) -> bool {
    if valid_from > target_time {
        return false;
    }
    match valid_until {
        None => true,
        Some(until) => until > target_time,
    }
}
