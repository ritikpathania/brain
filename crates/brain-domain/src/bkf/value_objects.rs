//! Domain value objects for Reflection Engine v2 and BKF representations.

use serde::{Deserialize, Serialize};
use std::time::SystemTime;

/// Immutable strongly-typed timestamp wrapper.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Timestamp(pub SystemTime);

impl Timestamp {
    /// Creates a new timestamp set to current system time.
    pub fn now() -> Self {
        Self(SystemTime::now())
    }
}

/// Normalized confidence score strictly bounded to [0.0, 1.0].
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct Confidence(f32);

impl Confidence {
    /// Validates and constructs a Confidence score. Returns error if value < 0.0 or > 1.0.
    pub fn new(value: f32) -> Result<Self, String> {
        if (0.0..=1.0).contains(&value) {
            Ok(Self(value))
        } else {
            Err(format!(
                "Confidence value {} must be between 0.0 and 1.0",
                value
            ))
        }
    }

    /// Returns the underlying f32 float value.
    pub fn value(&self) -> f32 {
        self.0
    }
}

/// Entity Name value object with canonical normalization.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct EntityName(String);

impl EntityName {
    /// Validates and constructs an EntityName. Trims whitespace and errors if empty.
    pub fn new(name: &str) -> Result<Self, String> {
        let trimmed = name.trim();
        if trimmed.is_empty() {
            return Err("EntityName cannot be empty".to_string());
        }
        Ok(Self(trimmed.to_string()))
    }

    /// Returns a string slice of the entity name.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Predicate Name value object with canonical normalization.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PredicateName(String);

impl PredicateName {
    /// Validates and constructs a PredicateName. Trims whitespace and errors if empty.
    pub fn new(name: &str) -> Result<Self, String> {
        let trimmed = name.trim();
        if trimmed.is_empty() {
            return Err("PredicateName cannot be empty".to_string());
        }
        Ok(Self(trimmed.to_string()))
    }

    /// Returns a string slice of the predicate name.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Category of predicate cardinality constraints.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PredicateCardinality {
    /// Subject can have at most 1 active value for this predicate (e.g. LivesIn).
    Exclusive,
    /// Subject can have multiple active values for this predicate (e.g. Knows).
    MultiValued,
}

/// Rich literal values for non-entity assertion targets.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "value", rename_all = "snake_case")]
pub enum LiteralValue {
    /// Text literal.
    String(String),
    /// Integer literal.
    Integer(i64),
    /// Floating point literal.
    Float(f64),
    /// Boolean literal.
    Boolean(bool),
    /// Timestamp literal.
    Timestamp(Timestamp),
}

/// Strongly-typed identifier for reflection passes.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct PassId(pub String);

impl PassId {
    /// Constructs a PassId from a string-like identifier.
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// Returns a string slice of the pass ID.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
