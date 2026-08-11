use brain_tui::state::{ConnectionMode, UiState};

#[test]
fn test_release_candidate_daemon_connection_lifecycle_states() {
    let mut state = UiState::new();

    // Default startup connection state
    assert_eq!(state.connection_mode, ConnectionMode::Disconnected);

    // Toggle connection states
    state.connection_mode = ConnectionMode::Connecting;
    assert_eq!(state.connection_mode, ConnectionMode::Connecting);

    state.connection_mode = ConnectionMode::Daemon;
    assert_eq!(state.connection_mode, ConnectionMode::Daemon);
}
