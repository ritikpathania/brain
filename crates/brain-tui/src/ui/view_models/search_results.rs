//! Encapsulated presentation view model for interactive search results.

use std::ops::Range;

/// Computes case-insensitive matching character index ranges of `query` inside `text`.
pub fn compute_highlight_ranges(text: &str, query: &str) -> Vec<Range<usize>> {
    let trimmed_query = query.trim();
    if trimmed_query.is_empty() || text.is_empty() {
        return Vec::new();
    }

    let text_lower = text.to_lowercase();
    let query_lower = trimmed_query.to_lowercase();
    let mut ranges = Vec::new();
    let mut start_search = 0;

    while let Some(idx) = text_lower[start_search..].find(&query_lower) {
        let match_start = start_search + idx;
        let match_end = match_start + query_lower.len();
        ranges.push(match_start..match_end);
        start_search = match_end;
    }

    ranges
}

/// Presentation view model for an individual search result item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchResultViewModel {
    /// Unique entity identifier.
    pub id: String,
    /// Display name / entity label string.
    pub display_name: String,
    /// Content preview snippet.
    pub snippet: String,
    /// Relevance / quality score.
    pub score: i64,
    /// Originating source system label.
    pub source_kind: String,
    /// Query term match ranges inside `display_name`.
    pub display_name_highlights: Vec<Range<usize>>,
    /// Query term match ranges inside `snippet`.
    pub snippet_highlights: Vec<Range<usize>>,
}

impl SearchResultViewModel {
    /// Constructs a `SearchResultViewModel` computing highlight ranges automatically.
    pub fn new(
        id: String,
        display_name: String,
        snippet: String,
        score: i64,
        source_kind: String,
        query: &str,
    ) -> Self {
        let display_name_highlights = compute_highlight_ranges(&display_name, query);
        let snippet_highlights = compute_highlight_ranges(&snippet, query);

        Self {
            id,
            display_name,
            snippet,
            score,
            source_kind,
            display_name_highlights,
            snippet_highlights,
        }
    }
}

/// Encapsulated presentation view model for an interactive search collection.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SearchResultsViewModel {
    /// Active search query string.
    query: String,
    /// Ordered search result item view models.
    items: Vec<SearchResultViewModel>,
    /// Encapsulated private selection index.
    selected: Option<usize>,
    /// Whether search selection focus is currently active.
    active: bool,
}

impl SearchResultsViewModel {
    /// Instantiates a new `SearchResultsViewModel`.
    pub fn new(query: String, items: Vec<SearchResultViewModel>) -> Self {
        let selected = if items.is_empty() { None } else { Some(0) };
        Self {
            query,
            items,
            selected,
            active: false,
        }
    }

    /// Returns the active query string.
    pub fn query(&self) -> &str {
        &self.query
    }

    /// Returns a slice of current item view models.
    pub fn items(&self) -> &[SearchResultViewModel] {
        &self.items
    }

    /// Returns true if search result selection focus is currently active.
    pub fn is_active(&self) -> bool {
        self.active
    }

    /// Sets whether search result selection focus is active.
    pub fn set_active(&mut self, active: bool) {
        self.active = active;
        if active && self.selected.is_none() && !self.items.is_empty() {
            self.selected = Some(0);
        }
    }

    /// Returns the currently selected item view model, if any.
    pub fn selected_item(&self) -> Option<&SearchResultViewModel> {
        self.selected.and_then(|idx| self.items.get(idx))
    }

    /// Returns the active selection index.
    pub fn selected_index(&self) -> Option<usize> {
        self.selected
    }

    /// Navigates selection to the next item.
    pub fn select_next(&mut self) {
        if self.items.is_empty() {
            self.selected = None;
            return;
        }

        self.selected = match self.selected {
            Some(curr) => {
                if curr + 1 < self.items.len() {
                    Some(curr + 1)
                } else {
                    Some(curr)
                }
            }
            None => Some(0),
        };
    }

    /// Navigates selection to the previous item.
    pub fn select_prev(&mut self) {
        if self.items.is_empty() {
            self.selected = None;
            return;
        }

        self.selected = match self.selected {
            Some(curr) => Some(curr.saturating_sub(1)),
            None => Some(0),
        };
    }

    /// Navigates selection to the first item.
    pub fn select_first(&mut self) {
        if self.items.is_empty() {
            self.selected = None;
        } else {
            self.selected = Some(0);
        }
    }

    /// Navigates selection to the last item.
    pub fn select_last(&mut self) {
        if self.items.is_empty() {
            self.selected = None;
        } else {
            self.selected = Some(self.items.len() - 1);
        }
    }

    /// Clears active selection and deactivates focus.
    pub fn clear_selection(&mut self) {
        self.selected = None;
        self.active = false;
    }

    /// Updates search results preserving selection stability if the selected entity still exists.
    pub fn update_results(&mut self, new_query: String, new_items: Vec<SearchResultViewModel>) {
        let currently_selected_id = self.selected_item().map(|item| item.id.clone());

        self.query = new_query;
        self.items = new_items;

        if self.items.is_empty() {
            self.selected = None;
            return;
        }

        // Selection stability: Preserve selection if currently selected ID exists in new results
        if let Some(target_id) = currently_selected_id {
            if let Some(new_idx) = self.items.iter().position(|item| item.id == target_id) {
                self.selected = Some(new_idx);
                return;
            }
        }

        // Fallback: Default to first item if previous selection disappeared
        self.selected = Some(0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_highlight_ranges() {
        let text = "Rust Memory Model & Ownership in Rust";
        let query = "rust";
        let ranges = compute_highlight_ranges(text, query);
        assert_eq!(ranges, vec![0..4, 33..37]);
    }

    #[test]
    fn test_navigation_lifecycle() {
        let items = vec![
            SearchResultViewModel::new(
                "node_1".to_string(),
                "Title 1".to_string(),
                "Snippet 1".to_string(),
                100,
                "Graph".to_string(),
                "Title",
            ),
            SearchResultViewModel::new(
                "node_2".to_string(),
                "Title 2".to_string(),
                "Snippet 2".to_string(),
                90,
                "Graph".to_string(),
                "Title",
            ),
            SearchResultViewModel::new(
                "node_3".to_string(),
                "Title 3".to_string(),
                "Snippet 3".to_string(),
                80,
                "Graph".to_string(),
                "Title",
            ),
        ];

        let mut vm = SearchResultsViewModel::new("Title".to_string(), items);
        assert_eq!(vm.selected_index(), Some(0));

        vm.select_next();
        assert_eq!(vm.selected_index(), Some(1));
        assert_eq!(vm.selected_item().unwrap().id, "node_2");

        vm.select_next();
        assert_eq!(vm.selected_index(), Some(2));

        vm.select_next();
        assert_eq!(vm.selected_index(), Some(2));

        vm.select_prev();
        assert_eq!(vm.selected_index(), Some(1));

        vm.select_last();
        assert_eq!(vm.selected_index(), Some(2));

        vm.select_first();
        assert_eq!(vm.selected_index(), Some(0));
    }

    #[test]
    fn test_selection_stability_on_update() {
        let items_initial = vec![
            SearchResultViewModel::new(
                "id_a".to_string(),
                "A".to_string(),
                "".to_string(),
                10,
                "G".to_string(),
                "",
            ),
            SearchResultViewModel::new(
                "id_b".to_string(),
                "B".to_string(),
                "".to_string(),
                20,
                "G".to_string(),
                "",
            ),
            SearchResultViewModel::new(
                "id_c".to_string(),
                "C".to_string(),
                "".to_string(),
                30,
                "G".to_string(),
                "",
            ),
        ];

        let mut vm = SearchResultsViewModel::new("query".to_string(), items_initial);
        vm.select_next(); // selects id_b (index 1)
        assert_eq!(vm.selected_item().unwrap().id, "id_b");

        let items_updated = vec![
            SearchResultViewModel::new(
                "id_b".to_string(),
                "B".to_string(),
                "".to_string(),
                20,
                "G".to_string(),
                "",
            ),
            SearchResultViewModel::new(
                "id_a".to_string(),
                "A".to_string(),
                "".to_string(),
                10,
                "G".to_string(),
                "",
            ),
        ];

        vm.update_results("query".to_string(), items_updated);
        assert_eq!(vm.selected_index(), Some(0));
        assert_eq!(vm.selected_item().unwrap().id, "id_b");

        let items_removed = vec![SearchResultViewModel::new(
            "id_x".to_string(),
            "X".to_string(),
            "".to_string(),
            5,
            "G".to_string(),
            "",
        )];

        vm.update_results("query".to_string(), items_removed);
        assert_eq!(vm.selected_index(), Some(0));
        assert_eq!(vm.selected_item().unwrap().id, "id_x");
    }

    #[test]
    fn test_invariant_10_deterministic_view_model_projections() {
        // Invariant 10: Given identical inputs, Projection Layer MUST produce byte-for-byte identical ViewModels.
        let loading1 = LoadingViewModel::project("rust", 3);
        let loading2 = LoadingViewModel::project("rust", 3);
        assert_eq!(loading1, loading2);

        let empty1 = EmptyViewModel::project("nonexistent");
        let empty2 = EmptyViewModel::project("nonexistent");
        assert_eq!(empty1, empty2);

        let err1 = ErrorViewModel::project("test", "Connection timeout");
        let err2 = ErrorViewModel::project("test", "Connection timeout");
        assert_eq!(err1, err2);
    }
}

// ─── State Projections (ADR-027 Step 2) ────────────────────────────────────

/// Immutable presentation View Model for searching / loading states.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoadingViewModel {
    /// Active query text.
    pub query: String,
    /// Pre-formatted loading status string.
    pub status_text: String,
    /// Animated frame indicator symbol.
    pub spinner_frame: &'static str,
}

impl LoadingViewModel {
    /// Projection constructor mapping query text and frame counter into a deterministic `LoadingViewModel`.
    pub fn project(query: impl Into<String>, frame_tick: usize) -> Self {
        let spinners = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
        let spinner_frame = spinners[frame_tick % spinners.len()];
        let q = query.into();
        let status_text = if q.is_empty() {
            "Searching knowledge graph...".to_string()
        } else {
            format!("Searching knowledge graph for \"{}\"...", q)
        };

        Self {
            query: q,
            status_text,
            spinner_frame,
        }
    }
}

/// Immutable presentation View Model for empty search results states.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmptyViewModel {
    /// Active query text.
    pub query: String,
    /// Primary diagnostic status message.
    pub headline: String,
    /// Actionable search refinement suggestions.
    pub suggestions: Vec<String>,
}

impl EmptyViewModel {
    /// Projection constructor mapping query text into a deterministic `EmptyViewModel`.
    pub fn project(query: impl Into<String>) -> Self {
        let q = query.into();
        let headline = if q.is_empty() {
            "No active query".to_string()
        } else {
            format!("No matching knowledge found for \"{}\"", q)
        };

        let suggestions = vec![
            "Check for typos or broader keywords".to_string(),
            "Try searching by concept entity type (e.g. System, Language)".to_string(),
            "Use `/ingest` to add new knowledge to memory".to_string(),
        ];

        Self {
            query: q,
            headline,
            suggestions,
        }
    }
}

/// Immutable presentation View Model for error search states.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ErrorViewModel {
    /// Active query text.
    pub query: String,
    /// Pre-formatted user-facing error message.
    pub error_message: String,
    /// Actionable recovery guidance hint.
    pub recovery_hint: String,
}

impl ErrorViewModel {
    /// Projection constructor mapping query text and raw error into a deterministic `ErrorViewModel`.
    pub fn project(query: impl Into<String>, raw_error: &str) -> Self {
        let q = query.into();
        let error_message = format!("Search Failed: {}", raw_error);
        let recovery_hint =
            "Press Esc to clear or check if brain-daemon is running (`brain health`)".to_string();

        Self {
            query: q,
            error_message,
            recovery_hint,
        }
    }
}
