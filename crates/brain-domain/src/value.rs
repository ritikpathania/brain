//! Pure domain data representation decoupled from transport and serialization formats.

use std::collections::BTreeMap;
use std::fmt;

/// Domain-wide canonical structured value type reusable across execution, memory, inspection, and reasoning.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize)]
pub enum StructuredValue {
    /// Null / empty value representation.
    Null,
    /// Boolean value.
    Bool(bool),
    /// 64-bit signed integer value.
    Integer(i64),
    /// String value.
    String(String),
    /// Ordered list of structured values.
    List(Vec<StructuredValue>),
    /// Deterministically ordered map of key-value pairs.
    Object(BTreeMap<String, StructuredValue>),
}

impl StructuredValue {
    /// Returns true if the value is Null.
    pub fn is_null(&self) -> bool {
        matches!(self, Self::Null)
    }

    /// Returns a reference to the inner string if this is a String.
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::String(s) => Some(s.as_str()),
            _ => None,
        }
    }

    /// Returns a reference to the inner object map if this is an Object.
    pub fn as_object(&self) -> Option<&BTreeMap<String, StructuredValue>> {
        match self {
            Self::Object(map) => Some(map),
            _ => None,
        }
    }
}

impl Default for StructuredValue {
    fn default() -> Self {
        Self::Null
    }
}

impl fmt::Display for StructuredValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Null => write!(f, "null"),
            Self::Bool(b) => write!(f, "{}", b),
            Self::Integer(i) => write!(f, "{}", i),
            Self::String(s) => write!(f, "\"{}\"", s),
            Self::List(items) => {
                write!(f, "[")?;
                for (idx, item) in items.iter().enumerate() {
                    if idx > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", item)?;
                }
                write!(f, "]")
            }
            Self::Object(map) => {
                write!(f, "{{")?;
                for (idx, (k, v)) in map.iter().enumerate() {
                    if idx > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "\"{}\": {}", k, v)?;
                }
                write!(f, "}}")
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_structured_value_formatting_and_ordering() {
        let mut map = BTreeMap::new();
        map.insert("key1".to_string(), StructuredValue::String("val1".to_string()));
        map.insert("key2".to_string(), StructuredValue::Integer(42));

        let val = StructuredValue::Object(map);
        assert_eq!(format!("{}", val), "{\"key1\": \"val1\", \"key2\": 42}");
    }
}
