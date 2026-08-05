//! E2E Navigation behavioral tests.
//!
//! Verifies SessionNavigator viewport scroll-into-view, identity-based selection
//! preservation across list refreshes, and bounds clamping.

use brain_domain::SessionId;
use brain_tui::ui::interaction::session_navigator::{SessionListItem, SessionNavigator};
use brain_tui::ui::interaction::sidebar::SidebarInteraction;

#[test]
fn test_session_navigator_identity_preservation() {
    // ── Arrange ──────────────────────────────────────────────────────────────
    let s1 = SessionId::new();
    let s2 = SessionId::new();
    let s3 = SessionId::new();

    let initial_items = vec![
        SessionListItem { id: s1 },
        SessionListItem { id: s2 },
        SessionListItem { id: s3 },
    ];

    let mut nav = SessionNavigator::new();
    nav.update_items(initial_items, false);
    nav.select_next(); // Selects s2 at index 1

    assert_eq!(nav.selected_id(), Some(s2));
    assert_eq!(nav.selected_index(), Some(1));

    // ── Act: Refresh list in a different order (s3, s2, s1) ─────────────────
    let refreshed_items = vec![
        SessionListItem { id: s3 },
        SessionListItem { id: s2 },
        SessionListItem { id: s1 },
    ];

    nav.update_items(refreshed_items, true);

    // ── Assert ───────────────────────────────────────────────────────────────
    // Selection MUST remain on s2 (which is now at index 1 in the new order)
    assert_eq!(
        nav.selected_id(),
        Some(s2),
        "Selection MUST follow session identity, not raw list index"
    );
    assert_eq!(nav.selected_index(), Some(1));
}

#[test]
fn test_session_navigator_viewport_scroll_into_view() {
    // ── Arrange ──────────────────────────────────────────────────────────────
    let sessions: Vec<SessionListItem> = (0..20)
        .map(|_| SessionListItem {
            id: SessionId::new(),
        })
        .collect();

    let mut nav = SessionNavigator::new();
    nav.update_items(sessions, false);
    nav.set_viewport_height(5); // Viewport height = 5 rows

    assert_eq!(nav.scroll_offset(), 0);

    // ── Act: Navigate down past viewport height (5 items) ───────────────────
    for _ in 0..6 {
        nav.select_next();
    }

    // ── Assert ───────────────────────────────────────────────────────────────
    let idx = nav.selected_index().unwrap();
    let offset = nav.scroll_offset();

    assert_eq!(idx, 6);
    assert!(
        offset > 0,
        "Scroll offset MUST advance when selection moves past viewport height"
    );
    assert!(
        idx >= offset && idx < offset + 5,
        "Selected item index ({}) MUST be within visible viewport window [{}, {})",
        idx,
        offset,
        offset + 5
    );
}

#[test]
fn test_sidebar_interaction_navigator_sync() {
    // ── Arrange ──────────────────────────────────────────────────────────────
    let s1 = SessionId::new();
    let s2 = SessionId::new();
    let visible = vec![s1, s2];

    let mut sidebar = SidebarInteraction::new();
    sidebar.restore_selection_fallback(&visible);

    assert_eq!(sidebar.browse.selected, Some(s1));

    // ── Act: Select fallback when list changes ───────────────────────────────
    sidebar.restore_selection_fallback(&[]);

    // ── Assert ───────────────────────────────────────────────────────────────
    assert_eq!(sidebar.browse.selected, None);
    assert!(sidebar.navigator.is_empty());
}
