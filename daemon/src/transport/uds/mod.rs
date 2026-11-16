/// Canonical frontend compatibility handshake for the versioned UDS transport.
pub mod frontend_handshake;
/// UDS connection handlers managing active Unix Domain Socket streams.
pub mod handlers;
/// Property-recovery helpers for memory/search (relations, excerpt, scope).
pub mod memory_relations;
/// UDS protocol router mapping wire protocol actions to application request variants.
pub mod router;
/// Canonical external session-ID validation for UDS actions.
pub(crate) mod session_id;
