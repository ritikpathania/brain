//! Canonical session identity validation for the external UDS protocol.
//!
//! Brain issues session IDs as canonical ULID strings. External callers must
//! return that exact representation; the protocol never derives an identity
//! from arbitrary input.

use brain_domain::SessionId;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CanonicalSessionIdError {
    Missing,
    Invalid,
}

impl CanonicalSessionIdError {
    pub(crate) const fn code(self) -> &'static str {
        match self {
            Self::Missing => "missing_session_id",
            Self::Invalid => "invalid_session_id",
        }
    }

    pub(crate) const fn message(self) -> &'static str {
        match self {
            Self::Missing => "session_id is required",
            Self::Invalid => "session_id must be a canonical Brain-issued ULID",
        }
    }
}

/// Parses the only session identity format accepted by canonical UDS actions.
///
/// The empty string is an omitted required field. Whitespace-only, lower-case,
/// malformed, oversized, or control-containing strings are invalid instead of
/// being transformed into a different identity.
pub(crate) fn parse_canonical_session_id(raw: &str) -> Result<SessionId, CanonicalSessionIdError> {
    if raw.is_empty() {
        return Err(CanonicalSessionIdError::Missing);
    }

    let session_id = raw
        .parse::<SessionId>()
        .map_err(|_| CanonicalSessionIdError::Invalid)?;

    (session_id.to_string() == raw)
        .then_some(session_id)
        .ok_or(CanonicalSessionIdError::Invalid)
}

#[cfg(test)]
mod tests {
    use super::{parse_canonical_session_id, CanonicalSessionIdError};

    #[test]
    fn accepts_the_exact_brain_issued_ulid_representation() {
        let id = brain_domain::SessionId::new();
        assert_eq!(parse_canonical_session_id(&id.to_string()), Ok(id));
    }

    #[test]
    fn distinguishes_missing_from_invalid_session_ids() {
        assert_eq!(
            parse_canonical_session_id(""),
            Err(CanonicalSessionIdError::Missing)
        );
        for raw in [
            "   ",
            "not-a-ulid",
            "../../session",
            "01arz3ndektsv4rrffq69g5fav",
        ] {
            assert_eq!(
                parse_canonical_session_id(raw),
                Err(CanonicalSessionIdError::Invalid),
                "{raw:?} must not become an alias"
            );
        }
    }
}
