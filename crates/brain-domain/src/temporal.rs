//! First-class temporal domain models and abstractions for time-aware relational reasoning.

use std::time::Duration;
use serde::{Serialize, Deserialize};
use crate::entities::Edge;

/// A strongly-typed, opaque wrapper around a Unix timestamp in seconds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct TimePoint(u64);

impl TimePoint {
    /// Creates a new `TimePoint` from a Unix timestamp in seconds.
    pub fn from_unix_seconds(secs: u64) -> Self {
        Self(secs)
    }

    /// Access the underlying Unix timestamp in seconds.
    pub fn unix_seconds(&self) -> u64 {
        self.0
    }

    /// Checked addition. Computes `self + duration`, returning `None` if overflow occurred.
    pub fn checked_add(&self, duration: Duration) -> Option<Self> {
        self.0.checked_add(duration.as_secs()).map(Self)
    }

    /// Checked subtraction. Computes `self - duration`, returning `None` if overflow occurred.
    pub fn checked_sub(&self, duration: Duration) -> Option<Self> {
        self.0.checked_sub(duration.as_secs()).map(Self)
    }
}

/// A half-open bounding time interval `[start, end)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TimeInterval {
    start: TimePoint,
    end: Option<TimePoint>,
}

/// Domain-specific errors for temporal interval validations.
#[derive(Debug, Clone, thiserror::Error, PartialEq, Eq)]
pub enum TimeIntervalError {
    /// Error returned when trying to construct an interval where start > end.
    #[error("Start time {start:?} cannot be after end time {end:?}")]
    StartAfterEnd {
        /// The invalid start point.
        start: TimePoint,
        /// The invalid end point.
        end: TimePoint,
    },
}

impl TimeInterval {
    /// Creates a new `TimeInterval` and validates that `start <= end`.
    pub fn new(start: TimePoint, end: Option<TimePoint>) -> Result<Self, TimeIntervalError> {
        if let Some(e) = end {
            if start > e {
                return Err(TimeIntervalError::StartAfterEnd { start, end: e });
            }
        }
        Ok(Self { start, end })
    }

    /// Accessor for the start point of the interval.
    pub fn start(&self) -> TimePoint {
        self.start
    }

    /// Accessor for the end point of the interval, if one exists.
    pub fn end(&self) -> Option<TimePoint> {
        self.end
    }

    /// Checks if a given `TimePoint` lies within this interval `[start, end)`.
    pub fn contains(&self, point: TimePoint) -> bool {
        point >= self.start && self.end.map_or(true, |e| point < e)
    }

    /// Checks if this interval overlaps with another interval.
    pub fn overlaps(&self, other: &Self) -> bool {
        let cond1 = other.end.map_or(true, |e2| self.start < e2);
        let cond2 = self.end.map_or(true, |e1| other.start < e1);
        cond1 && cond2
    }

    /// Returns the intersection of this interval with another interval, if any.
    pub fn intersect(&self, other: &Self) -> Option<Self> {
        if !self.overlaps(other) {
            return None;
        }
        let start = std::cmp::max(self.start, other.start);
        let end = match (self.end, other.end) {
            (Some(e1), Some(e2)) => Some(std::cmp::min(e1, e2)),
            (Some(e1), None) => Some(e1),
            (None, Some(e2)) => Some(e2),
            (None, None) => None,
        };
        Some(Self { start, end })
    }
}

/// Aggregation of validity intervals representing when a graph element is active or authoritative.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TemporalValidity {
    intervals: Vec<TimeInterval>,
}

impl TemporalValidity {
    /// Creates a new `TemporalValidity` with the given intervals.
    pub fn new(intervals: Vec<TimeInterval>) -> Self {
        Self { intervals }
    }

    /// Accessor to the underlying intervals list.
    pub fn intervals(&self) -> &[TimeInterval] {
        &self.intervals
    }

    /// Checks if the validity ranges cover the given `TimePoint`.
    pub fn is_valid_at(&self, time: TimePoint) -> bool {
        self.intervals.iter().any(|interval| interval.contains(time))
    }

    /// Checks if the validity ranges overlap with a target `TimeInterval`.
    pub fn intersects_interval(&self, target: &TimeInterval) -> bool {
        self.intervals.iter().any(|interval| interval.overlaps(target))
    }
}

/// A trait representing a clock source for querying time.
pub trait Clock {
    /// Returns the current `TimePoint`.
    fn now(&self) -> TimePoint;
}

/// A system clock implementation using standard library system time.
pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> TimePoint {
        use std::time::{SystemTime, UNIX_EPOCH};
        let secs = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        TimePoint(secs)
    }
}

/// A clock source that returns a fixed constant `TimePoint`.
pub struct FixedClock {
    time: TimePoint,
}

impl FixedClock {
    /// Creates a new `FixedClock` at the target `TimePoint`.
    pub fn new(time: TimePoint) -> Self {
        Self { time }
    }
}

impl Clock for FixedClock {
    fn now(&self) -> TimePoint {
        self.time
    }
}

/// A mockable clock source that can be advanced manually for test verification.
pub struct TestClock {
    time: std::cell::Cell<u64>,
}

impl TestClock {
    /// Creates a new `TestClock` starting at `initial_secs`.
    pub fn new(initial_secs: u64) -> Self {
        Self {
            time: std::cell::Cell::new(initial_secs),
        }
    }

    /// Advances the clock time forward by `secs` seconds.
    pub fn advance(&self, secs: u64) {
        self.time.set(self.time.get() + secs);
    }
}

impl Clock for TestClock {
    fn now(&self) -> TimePoint {
        TimePoint(self.time.get())
    }
}

/// Policy configurations for calculating the decay of edge weight over time.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum RecencyPolicy {
    /// Exponential decay defined by a half-life duration in seconds.
    Exponential {
        /// The half-life duration in seconds.
        half_life_secs: f64,
    },
    /// Linear decay dropping down to 0 at a horizon limit.
    Linear {
        /// The horizon threshold in seconds.
        horizon_secs: f64,
    },
    /// No weight decay applied.
    None,
}

impl std::hash::Hash for RecencyPolicy {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        match self {
            Self::Exponential { half_life_secs } => {
                state.write_u8(0);
                let bits = if half_life_secs.is_nan() {
                    f64::NAN.to_bits()
                } else if *half_life_secs == 0.0 {
                    0.0f64.to_bits()
                } else {
                    half_life_secs.to_bits()
                };
                bits.hash(state);
            }
            Self::Linear { horizon_secs } => {
                state.write_u8(1);
                let bits = if horizon_secs.is_nan() {
                    f64::NAN.to_bits()
                } else if *horizon_secs == 0.0 {
                    0.0f64.to_bits()
                } else {
                    horizon_secs.to_bits()
                };
                bits.hash(state);
            }
            Self::None => {
                state.write_u8(2);
            }
        }
    }
}

impl RecencyPolicy {
    /// Computes the decayed weight of an element given its base weight, observation time, and a reference query time.
    pub fn compute_weight(&self, base_weight: f64, observation_time: TimePoint, reference_time: TimePoint) -> f64 {
        if observation_time > reference_time {
            return base_weight;
        }
        let elapsed = (reference_time.unix_seconds() - observation_time.unix_seconds()) as f64;
        match self {
            Self::Exponential { half_life_secs } => {
                if *half_life_secs <= 0.0 {
                    base_weight
                } else {
                    let lambda = 2.0f64.ln() / *half_life_secs;
                    base_weight * (-lambda * elapsed).exp()
                }
            }
            Self::Linear { horizon_secs } => {
                if *horizon_secs <= 0.0 {
                    0.0
                } else if elapsed >= *horizon_secs {
                    0.0
                } else {
                    base_weight * (1.0 - elapsed / *horizon_secs)
                }
            }
            Self::None => base_weight,
        }
    }
}

/// Visibility semantic scope for temporal query projections.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TemporalVisibility {
    /// Facts valid exactly at the reference time.
    Current,
    /// Facts ever valid before or at the reference time.
    Historical,
    /// Facts whose validity interval intersects the target interval.
    Interval(TimeInterval),
}

/// Context encapsulating reference parameters for query evaluations.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct TemporalQuery {
    /// Reference evaluation timestamp.
    pub reference_time: TimePoint,
    /// Semantic visibility mode.
    pub visibility: TemporalVisibility,
    /// Recency policy configuration.
    pub recency_policy: RecencyPolicy,
}

/// An edge decorated with first-class temporal validation and observation traits.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemporalEdge {
    /// The underlying graph relationship edge.
    pub edge: Edge,
    /// Validity ranges when this relationship is active.
    pub validity: TemporalValidity,
    /// When the relationship was observed by the system.
    pub observed_at: TimePoint,
}

/// A projected view of valid knowledge graph relationships at a target `TimePoint`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TemporalSnapshot {
    /// The reference projection point.
    pub time_point: TimePoint,
    /// All active, visible edge IDs at the projection point.
    pub active_edge_ids: std::collections::HashSet<crate::identifiers::EdgeId>,
}

impl TemporalSnapshot {
    /// Projects active relationships out of a dataset matching the `TemporalQuery` criteria.
    pub fn project(edges: &[TemporalEdge], query: &TemporalQuery) -> Self {
        let reference = query.reference_time;
        let mut active_edge_ids = std::collections::HashSet::new();

        for te in edges {
            if te.observed_at > reference {
                continue;
            }

            let is_visible = match query.visibility {
                TemporalVisibility::Current => {
                    te.validity.is_valid_at(reference)
                }
                TemporalVisibility::Historical => {
                    te.validity.intervals().iter().any(|interval| interval.start() <= reference)
                }
                TemporalVisibility::Interval(target_interval) => {
                    if let Ok(ref_limit) = TimeInterval::new(TimePoint::from_unix_seconds(0), Some(reference)) {
                        if let Some(valid_limit) = target_interval.intersect(&ref_limit) {
                            te.validity.intersects_interval(&valid_limit)
                        } else {
                            false
                        }
                    } else {
                        false
                    }
                }
            };

            if is_visible {
                let edge_id = crate::identifiers::EdgeId::new(te.edge.source, te.edge.target, te.edge.relation.id());
                active_edge_ids.insert(edge_id);
            }
        }

        Self {
            time_point: reference,
            active_edge_ids,
        }
    }
}

/// Pure helper structure for deterministic projection evaluation.
pub struct TemporalProjector;

impl TemporalProjector {
    /// Deterministically projects a slice of `TemporalEdge` relationships under the visibility constraints of `TemporalQuery`.
    pub fn project(edges: &[TemporalEdge], query: &TemporalQuery) -> TemporalSnapshot {
        TemporalSnapshot::project(edges, query)
    }
}
