use std::time::{SystemTime, UNIX_EPOCH};

/// The Specification pattern trait.
pub trait Specification<T> {
    /// Determines whether the given value satisfies this specification.
    fn is_satisfied_by(&self, value: &T) -> bool;

    /// Combines this specification with another using logical AND.
    fn and<S>(self, other: S) -> AndSpecification<Self, S>
    where
        Self: Sized,
    {
        AndSpecification { left: self, right: other }
    }

    /// Combines this specification with another using logical OR.
    fn or<S>(self, other: S) -> OrSpecification<Self, S>
    where
        Self: Sized,
    {
        OrSpecification { left: self, right: other }
    }

    /// Negates this specification.
    fn not(self) -> NotSpecification<Self>
    where
        Self: Sized,
    {
        NotSpecification { spec: self }
    }
}

/// A specification that requires both sub-specifications to be satisfied.
pub struct AndSpecification<L, R> {
    /// The left-hand specification.
    pub left: L,
    /// The right-hand specification.
    pub right: R,
}

impl<T, L, R> Specification<T> for AndSpecification<L, R>
where
    L: Specification<T>,
    R: Specification<T>,
{
    fn is_satisfied_by(&self, value: &T) -> bool {
        self.left.is_satisfied_by(value) && self.right.is_satisfied_by(value)
    }
}

/// A specification that requires at least one sub-specification to be satisfied.
pub struct OrSpecification<L, R> {
    /// The left-hand specification.
    pub left: L,
    /// The right-hand specification.
    pub right: R,
}

impl<T, L, R> Specification<T> for OrSpecification<L, R>
where
    L: Specification<T>,
    R: Specification<T>,
{
    fn is_satisfied_by(&self, value: &T) -> bool {
        self.left.is_satisfied_by(value) || self.right.is_satisfied_by(value)
    }
}

/// A specification that negates the inner specification.
pub struct NotSpecification<S> {
    /// The inner specification.
    pub spec: S,
}

impl<T, S> Specification<T> for NotSpecification<S>
where
    S: Specification<T>,
{
    fn is_satisfied_by(&self, value: &T) -> bool {
        !self.spec.is_satisfied_by(value)
    }
}

/// A specification that checks if an Edge has expired based on its updated_at timestamp.
pub struct IsExpired {
    ttl_seconds: u64,
    current_time: u64,
}

impl IsExpired {
    /// Creates a new `IsExpired` specification with given TTL and custom current timestamp.
    pub fn new(ttl_seconds: u64, current_time: u64) -> Self {
        Self { ttl_seconds, current_time }
    }

    /// Creates a new `IsExpired` specification with given TTL and system current timestamp.
    pub fn now(ttl_seconds: u64) -> Self {
        let current_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        Self { ttl_seconds, current_time }
    }
}

impl crate::entities::Edge {
    /// Checks if the edge has expired relative to a current timestamp and TTL.
    pub fn is_expired(&self, ttl_seconds: u64, current_time: u64) -> bool {
        current_time.saturating_sub(self.updated_at) > ttl_seconds
    }
}

impl Specification<crate::entities::Edge> for IsExpired {
    fn is_satisfied_by(&self, edge: &crate::entities::Edge) -> bool {
        edge.is_expired(self.ttl_seconds, self.current_time)
    }
}

/// A specification that checks if a Node is pinned.
pub struct IsPinned;

impl Specification<crate::entities::Node> for IsPinned {
    fn is_satisfied_by(&self, node: &crate::entities::Node) -> bool {
        node.properties
            .get("pinned")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
    }
}
