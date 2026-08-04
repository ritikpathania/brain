//! Encapsulated navigation state for the sidebar session list.
//!
//! ## Architectural decision
//!
//! `SessionNavigator` owns selection and scroll state, keeping the renderer
//! stateless. At render time, the widget derives a `ratatui::widgets::ListState`
//! from `scroll_offset()` and `selected()`:
//!
//! ```text
//! SessionNavigator
//!         │
//!         ▼
//! Renderer (derives ListState each frame)
//!         │
//!         ▼
//! ratatui::ListState
//! ```
//!
//! This prevents Ratatui-specific state from leaking into the interaction model.
//! Future features (search, filter, multi-select, pinned sessions) extend this
//! type rather than scattering state across widgets or `AppState`.
//!
//! ## Selection identity
//!
//! The navigator stores selection as a `SessionId` (stable identity), not an
//! index. This prevents the classic bug where a list refresh changes which
//! session sits at index N, silently changing the selected session.

use brain_domain::SessionId;

/// A snapshot item used by the navigator to track what is currently visible.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionListItem {
    /// Stable session identity. Used for selection preservation across refreshes.
    pub id: SessionId,
}

/// Encapsulated navigation state for the sidebar session list.
///
/// ## Invariants
///
/// - `selected` is always `< items.len()` when `items` is non-empty.
/// - `selected` is `None` when `items` is empty.
/// - `scroll_offset` is always `<= selected.unwrap_or(0)` and keeps the
///   selected item within `[scroll_offset, scroll_offset + viewport_height)`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionNavigator {
    /// Ordered snapshot of visible session list items.
    items: Vec<SessionListItem>,
    /// Currently selected position by `SessionId`. Stored as an id, not an index,
    /// to survive list refreshes where the ordering may change.
    selected_id: Option<SessionId>,
    /// Index of the top-most visible item in the viewport.
    scroll_offset: usize,
    /// Number of rows available in the rendered viewport. Updated each render frame
    /// via `set_viewport_height`. Defaults conservatively to 10.
    viewport_height: usize,
}

impl Default for SessionNavigator {
    fn default() -> Self {
        Self::new()
    }
}

impl SessionNavigator {
    /// Creates a new empty `SessionNavigator`.
    pub fn new() -> Self {
        Self {
            items: Vec::new(),
            selected_id: None,
            scroll_offset: 0,
            viewport_height: 10,
        }
    }

    // ── Read accessors ────────────────────────────────────────────────────────

    /// Returns the index of the currently selected item, if any.
    pub fn selected_index(&self) -> Option<usize> {
        let id = self.selected_id?;
        self.items.iter().position(|item| item.id == id)
    }

    /// Returns the `SessionId` of the currently selected item, if any.
    pub fn selected_id(&self) -> Option<SessionId> {
        self.selected_id
    }

    /// Returns the current scroll offset (index of the topmost visible row).
    pub fn scroll_offset(&self) -> usize {
        self.scroll_offset
    }

    /// Returns the number of items in the current snapshot.
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Returns `true` if the item list is empty.
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Returns a reference to the current item snapshot.
    pub fn items(&self) -> &[SessionListItem] {
        &self.items
    }

    // ── Viewport ──────────────────────────────────────────────────────────────

    /// Updates the viewport height. Should be called by the renderer each frame
    /// before reading `scroll_offset()`.
    pub fn set_viewport_height(&mut self, height: usize) {
        if height > 0 {
            self.viewport_height = height;
            self.clamp_scroll();
        }
    }

    // ── Navigation ────────────────────────────────────────────────────────────

    /// Moves the selection one item towards the start of the list.
    /// Clamps at the first item (does not wrap).
    pub fn select_prev(&mut self) {
        if self.items.is_empty() {
            self.selected_id = None;
            return;
        }
        let current = self.selected_index().unwrap_or(0);
        let new_idx = current.saturating_sub(1);
        self.set_index(new_idx);
    }

    /// Moves the selection one item towards the end of the list.
    /// Clamps at the last item (does not wrap).
    pub fn select_next(&mut self) {
        if self.items.is_empty() {
            self.selected_id = None;
            return;
        }
        let current = self.selected_index().unwrap_or(0);
        let new_idx = (current + 1).min(self.items.len() - 1);
        self.set_index(new_idx);
    }

    /// Moves the selection to the first item.
    pub fn select_first(&mut self) {
        if self.items.is_empty() {
            self.selected_id = None;
        } else {
            self.set_index(0);
        }
    }

    /// Moves the selection to the last item.
    pub fn select_last(&mut self) {
        if self.items.is_empty() {
            self.selected_id = None;
        } else {
            self.set_index(self.items.len() - 1);
        }
    }

    // ── List refresh ──────────────────────────────────────────────────────────

    /// Replaces the item snapshot.
    ///
    /// When `preserve_selection` is `true` (the common case for live refreshes),
    /// the navigator tries to keep the same `SessionId` selected. If the
    /// previously-selected ID is not present in `new_items`, the selection falls
    /// back to the first item.
    ///
    /// When `preserve_selection` is `false`, the selection always resets to the
    /// first item.
    pub fn update_items(&mut self, new_items: Vec<SessionListItem>, preserve_selection: bool) {
        let previous_id = self.selected_id;
        self.items = new_items;

        if self.items.is_empty() {
            self.selected_id = None;
            self.scroll_offset = 0;
            return;
        }

        if preserve_selection {
            if let Some(pid) = previous_id {
                if self.items.iter().any(|item| item.id == pid) {
                    // Previous selection still exists — preserve it.
                    self.selected_id = Some(pid);
                    self.clamp_scroll();
                    return;
                }
            }
        }

        // Default: select first item.
        self.selected_id = Some(self.items[0].id);
        self.scroll_offset = 0;
    }

    // ── Internal helpers ──────────────────────────────────────────────────────

    /// Sets the selected item by index and updates `scroll_offset` so the item
    /// is visible within the viewport.
    fn set_index(&mut self, idx: usize) {
        if idx >= self.items.len() {
            return;
        }
        self.selected_id = Some(self.items[idx].id);
        self.scroll_into_view(idx);
    }

    /// Adjusts `scroll_offset` so that `idx` is within the visible viewport
    /// `[scroll_offset, scroll_offset + viewport_height)`.
    fn scroll_into_view(&mut self, idx: usize) {
        if idx < self.scroll_offset {
            // Selection moved above the viewport top — scroll up.
            self.scroll_offset = idx;
        } else if idx >= self.scroll_offset + self.viewport_height {
            // Selection moved below the viewport bottom — scroll down.
            self.scroll_offset = idx + 1 - self.viewport_height;
        }
    }

    /// Clamps `scroll_offset` so that it does not exceed the maximum valid
    /// value given the current item count and viewport height.
    fn clamp_scroll(&mut self) {
        if self.items.is_empty() {
            self.scroll_offset = 0;
            return;
        }
        let max_offset = self.items.len().saturating_sub(self.viewport_height);
        self.scroll_offset = self.scroll_offset.min(max_offset);

        // Also scroll the current selection into view after clamping.
        if let Some(idx) = self.selected_index() {
            self.scroll_into_view(idx);
        }
    }
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// Generates a deterministic pool of 256 unique session IDs.
    /// Test helpers address them by u8 index — same index always returns the
    /// same `SessionId` within a single test binary run.
    fn id_pool() -> Vec<SessionId> {
        // Generate 256 IDs upfront. Each call to SessionId::new() returns a
        // time-based ULID; since ULID includes random bits the IDs are unique.
        // We cache them in a thread-local to keep the same mapping across calls.
        use std::cell::RefCell;
        thread_local! {
            static POOL: RefCell<Option<Vec<SessionId>>> = const { RefCell::new(None) };
        }
        POOL.with(|cell| {
            let mut borrow = cell.borrow_mut();
            if borrow.is_none() {
                *borrow = Some((0..=255).map(|_| SessionId::new()).collect());
            }
            borrow.as_ref().unwrap().clone()
        })
    }

    fn sid(n: u8) -> SessionId {
        id_pool()[n as usize]
    }

    fn items(ids: &[u8]) -> Vec<SessionListItem> {
        ids.iter()
            .map(|&n| SessionListItem { id: sid(n) })
            .collect()
    }

    fn navigator(ids: &[u8]) -> SessionNavigator {
        let mut nav = SessionNavigator::new();
        nav.update_items(items(ids), false);
        nav.set_viewport_height(5);
        nav
    }

    // ── Basic selection ───────────────────────────────────────────────────────

    #[test]
    fn test_empty_list_has_no_selection() {
        let nav = SessionNavigator::new();
        assert_eq!(nav.selected_index(), None);
        assert_eq!(nav.selected_id(), None);
        assert!(nav.is_empty());
    }

    #[test]
    fn test_non_empty_list_starts_at_first_item() {
        let nav = navigator(&[1, 2, 3]);
        assert_eq!(nav.selected_index(), Some(0));
        assert_eq!(nav.selected_id(), Some(sid(1)));
    }

    #[test]
    fn test_select_next_advances_selection() {
        let mut nav = navigator(&[1, 2, 3]);
        nav.select_next();
        assert_eq!(nav.selected_index(), Some(1));
        assert_eq!(nav.selected_id(), Some(sid(2)));
    }

    #[test]
    fn test_select_prev_retreats_selection() {
        let mut nav = navigator(&[1, 2, 3]);
        nav.select_next();
        nav.select_next();
        nav.select_prev();
        assert_eq!(nav.selected_index(), Some(1));
    }

    #[test]
    fn test_select_next_clamps_at_last_item() {
        let mut nav = navigator(&[1, 2, 3]);
        nav.select_last();
        let before = nav.selected_index();
        nav.select_next();
        assert_eq!(
            nav.selected_index(),
            before,
            "Should not advance past last item"
        );
    }

    #[test]
    fn test_select_prev_clamps_at_first_item() {
        let mut nav = navigator(&[1, 2, 3]);
        let before = nav.selected_index();
        nav.select_prev();
        assert_eq!(
            nav.selected_index(),
            before,
            "Should not retreat before first item"
        );
    }

    #[test]
    fn test_select_first_and_last() {
        let mut nav = navigator(&[10, 20, 30, 40, 50]);
        nav.select_last();
        assert_eq!(nav.selected_id(), Some(sid(50)));
        nav.select_first();
        assert_eq!(nav.selected_id(), Some(sid(10)));
    }

    // ── Scroll-into-view ──────────────────────────────────────────────────────

    #[test]
    fn test_select_next_scrolls_viewport_when_selection_leaves_bottom() {
        let mut nav = SessionNavigator::new();
        nav.update_items(items(&[1, 2, 3, 4, 5, 6, 7, 8, 9, 10]), false);
        nav.set_viewport_height(3); // viewport shows rows [offset, offset+3)

        // Navigate from 0 → 1 → 2 (still in viewport [0..3))
        nav.select_next();
        nav.select_next();
        assert_eq!(nav.scroll_offset(), 0);

        // Navigate to index 3 — leaves bottom of viewport [0..3)
        nav.select_next();
        assert_eq!(nav.selected_index(), Some(3));
        assert!(
            nav.scroll_offset() > 0,
            "scroll_offset should advance when selection leaves viewport bottom"
        );
        // Index 3 must be visible: scroll_offset <= 3 < scroll_offset + 3
        let off = nav.scroll_offset();
        assert!(off <= 3 && 3 < off + 3);
    }

    #[test]
    fn test_select_prev_scrolls_viewport_when_selection_leaves_top() {
        let mut nav = SessionNavigator::new();
        nav.update_items(items(&[1, 2, 3, 4, 5, 6, 7, 8, 9, 10]), false);
        nav.set_viewport_height(3);

        // Jump to near the end
        nav.select_last();
        let last_offset = nav.scroll_offset();
        assert!(
            last_offset > 0,
            "Should have scrolled down to reach last item"
        );

        // Navigate back up past the top of the viewport
        for _ in 0..last_offset + 2 {
            nav.select_prev();
        }
        // After scrolling all the way back, offset should have retreated
        assert!(
            nav.scroll_offset() < last_offset,
            "scroll_offset should retreat when selection leaves viewport top"
        );
    }

    // ── Identity-based selection preservation ─────────────────────────────────

    #[test]
    fn test_update_items_preserves_selection_by_id() {
        let mut nav = navigator(&[1, 2, 3]);
        nav.select_next(); // selects id=2 at index 1
        assert_eq!(nav.selected_id(), Some(sid(2)));

        // Refresh list with same items in a different order
        nav.update_items(items(&[3, 2, 1]), true);

        // id=2 is now at index 1 in the new order — selection should track the ID
        assert_eq!(nav.selected_id(), Some(sid(2)));
        assert_eq!(nav.selected_index(), Some(1));
    }

    #[test]
    fn test_update_items_resets_to_first_when_selected_id_disappears() {
        let mut nav = navigator(&[1, 2, 3]);
        nav.select_next(); // selects id=2

        // Refresh list without id=2
        nav.update_items(items(&[10, 20, 30]), true);

        assert_eq!(nav.selected_id(), Some(sid(10)));
        assert_eq!(nav.selected_index(), Some(0));
    }

    #[test]
    fn test_update_items_with_preserve_false_always_resets() {
        let mut nav = navigator(&[1, 2, 3]);
        nav.select_last();

        nav.update_items(items(&[1, 2, 3]), false);
        assert_eq!(
            nav.selected_index(),
            Some(0),
            "preserve=false should always reset to first"
        );
    }

    #[test]
    fn test_update_items_with_empty_list_clears_selection() {
        let mut nav = navigator(&[1, 2, 3]);
        nav.select_next();
        nav.update_items(vec![], true);

        assert_eq!(nav.selected_id(), None);
        assert_eq!(nav.scroll_offset(), 0);
    }

    // ── Invariants ────────────────────────────────────────────────────────────

    #[test]
    fn test_len_and_is_empty() {
        let mut nav = SessionNavigator::new();
        assert!(nav.is_empty());
        assert_eq!(nav.len(), 0);

        nav.update_items(items(&[1, 2, 3]), false);
        assert!(!nav.is_empty());
        assert_eq!(nav.len(), 3);
    }

    #[test]
    fn test_scroll_offset_never_exceeds_max_valid_value() {
        let mut nav = SessionNavigator::new();
        nav.update_items(items(&[1, 2, 3, 4, 5]), false);
        nav.set_viewport_height(3);

        nav.select_last();
        // scroll_offset must be <= len - viewport_height = 5 - 3 = 2
        assert!(
            nav.scroll_offset() <= 2,
            "scroll_offset {} exceeds maximum 2",
            nav.scroll_offset()
        );
    }

    #[test]
    fn test_viewport_height_zero_does_not_update() {
        let mut nav = navigator(&[1, 2, 3]);
        let before = nav.scroll_offset();
        nav.set_viewport_height(0); // should be ignored
        assert_eq!(nav.scroll_offset(), before);
    }
}
