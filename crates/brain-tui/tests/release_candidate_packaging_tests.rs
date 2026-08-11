use brain_tui::state::UiState;
use brain_tui::ui::navigation::Screen;

#[test]
fn test_release_candidate_packaging_version_and_defaults() {
    let state = UiState::new();
    assert_eq!(state.screen, Screen::Home);

    // Verify workspace crate versions
    let tui_pkg_version = env!("CARGO_PKG_VERSION");
    assert!(
        !tui_pkg_version.is_empty(),
        "Package version should not be empty"
    );
}
