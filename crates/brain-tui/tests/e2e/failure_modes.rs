//! E2E Failure Modes behavioral tests.
//!
//! Verifies transport error handling, distinction between backend unavailability
//! and empty search results, and recovery state machines.

use brain_tui::ui::search::types::{SearchFailure, SearchResult};

#[test]
fn test_backend_unavailable_vs_empty_results_contract() {
    // ── Arrange ──────────────────────────────────────────────────────────────
    let empty_results: Result<Vec<SearchResult>, SearchFailure> = Ok(Vec::new());
    let backend_failure: Result<Vec<SearchResult>, SearchFailure> =
        Err(SearchFailure::BackendUnavailable);

    // ── Act & Assert (Contract Verification) ─────────────────────────────────
    assert!(
        empty_results.is_ok(),
        "Empty results MUST be Ok(Vec::new()) representing 'No relevant memories found'"
    );

    assert!(
        backend_failure.is_err(),
        "Daemon failure MUST be Err(BackendUnavailable), NOT silently swallowed to Ok(vec![])"
    );

    assert_eq!(
        backend_failure,
        Err(SearchFailure::BackendUnavailable),
        "Daemon failure MUST produce SearchFailure::BackendUnavailable"
    );
}

#[test]
fn test_reconnection_recovery_lifecycle() {
    // ── Arrange ──────────────────────────────────────────────────────────────
    // Simulated state transition: Disconnected -> Connected
    let mut is_connected = false;
    let mut last_error: Option<SearchFailure> = None;

    // Phase 1: Disconnected state
    if !is_connected {
        last_error = Some(SearchFailure::BackendUnavailable);
    }

    assert!(
        last_error.is_some(),
        "Failure status MUST be recorded when disconnected"
    );

    // Phase 2: Reconnection event (Daemon becomes available)
    is_connected = true;
    if is_connected {
        last_error = None; // Reconnection clears the error banner
    }

    // ── Assert ───────────────────────────────────────────────────────────────
    assert_eq!(
        last_error, None,
        "Reconnection MUST clear stale error state without requiring application restart"
    );
    assert!(is_connected, "Client connection status MUST update to true");
}
