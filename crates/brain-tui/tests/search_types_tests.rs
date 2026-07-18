use brain_tui::ui::search::types::{SearchGeneration, SearchQuery, PROVIDER_COMMANDS};

#[test]
fn test_type_construction() {
    let gen = SearchGeneration(1);
    let query = SearchQuery {
        generation: gen,
        text: "hello".to_string(),
    };
    assert_eq!(query.text, "hello");
    assert_eq!(PROVIDER_COMMANDS.as_str(), "commands");
}
