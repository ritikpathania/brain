use brain_tui::client::Confidence;
use brain_tui::ui::search::ranking::RankingEngine;
use brain_tui::ui::search::types::{SearchResult, SearchResultAction, SearchResultKind};

fn make_dummy_result(title: &str, kind: SearchResultKind, score: i32) -> SearchResult {
    SearchResult {
        entity_id: String::new(),
        title: Some(title.to_string()),
        subtitle: Some("dummy description".to_string()),
        kind,
        provider_score: score,
        confidence: Confidence::Medium,
        action: SearchResultAction::InvokeCommand(brain_tui::ui::command::CommandId("dummy")),
    }
}

#[test]
fn test_ranking_determinism_and_stable_sort() {
    let engine = RankingEngine::default();

    // Test exact alphabetical sorting fallback when scores and boosts are identical
    let results = vec![
        make_dummy_result("Z Session", SearchResultKind::Session, 5),
        make_dummy_result("A Session", SearchResultKind::Session, 5),
        make_dummy_result("M Session", SearchResultKind::Session, 5),
    ];

    let ranked = engine.rank("session", results.clone());
    assert_eq!(ranked.len(), 3);
    assert_eq!(ranked[0].title, Some("A Session".to_string()));
    assert_eq!(ranked[1].title, Some("M Session".to_string()));
    assert_eq!(ranked[2].title, Some("Z Session".to_string()));

    // Determinism: calling it again on the same input yields identical order
    let ranked_second = engine.rank("session", results);
    assert_eq!(ranked, ranked_second);
}

#[test]
fn test_ranking_idempotency() {
    let engine = RankingEngine::default();
    let results = vec![
        make_dummy_result("banana", SearchResultKind::Message, 2),
        make_dummy_result("apple", SearchResultKind::Message, 10),
        make_dummy_result("cherry", SearchResultKind::Message, 5),
    ];

    let pass1 = engine.rank("a", results);
    let pass2 = engine.rank("a", pass1.clone());
    assert_eq!(pass1, pass2);
}

#[test]
fn test_ranking_independence_of_irrelevant_alternatives() {
    let engine = RankingEngine::default();
    let high1 = make_dummy_result("Perfect match", SearchResultKind::Command, 100);
    let high2 = make_dummy_result("Good match", SearchResultKind::Command, 50);
    // Note: score -100 is well below minimum_score=6 threshold for non-empty queries,
    // so "Irrelevant" is filtered out when ranking with the query "match".
    let low = make_dummy_result("Irrelevant", SearchResultKind::Message, -100);

    let results_initial = vec![high1.clone(), high2.clone()];
    let ranked_initial = engine.rank("match", results_initial);

    // Relative order of high1 and high2 is:
    assert_eq!(ranked_initial[0].title, Some("Perfect match".to_string()));
    assert_eq!(ranked_initial[1].title, Some("Good match".to_string()));

    // Add the low-scoring result: it should be filtered by minimum_score
    let results_extended = vec![high2, low, high1];
    let ranked_extended = engine.rank("match", results_extended);

    // The low result is filtered — only the two high-scoring results remain
    assert_eq!(
        ranked_extended.len(),
        2,
        "Low-score result should be filtered by minimum_score"
    );
    assert_eq!(ranked_extended[0].title, Some("Perfect match".to_string()));
    assert_eq!(ranked_extended[1].title, Some("Good match".to_string()));
}

#[test]
fn test_ranking_boosts_additive() {
    let engine = RankingEngine::default();

    // We expect:
    // Prefix boost: +100
    // Word boundary boost: +50
    // Kind boosts: Session = +10, Command = +5

    // "ren" matches "Rename Session" (starts with query -> +100)
    // "ren" matches "Trigger Rename" (has space + query -> +50)
    let res1 = make_dummy_result("Trigger Rename", SearchResultKind::Session, 10); // score = 10 + 50 + 10 = 70
    let res2 = make_dummy_result("Rename Session", SearchResultKind::Session, 5); // score = 5 + 100 + 10 = 115
    let res3 = make_dummy_result("Renegade", SearchResultKind::Command, 5); // score = 5 + 100 + 5 = 110

    let results = vec![res1, res2, res3];
    let ranked = engine.rank("Ren", results);

    assert_eq!(ranked[0].title, Some("Rename Session".to_string())); // score 115
    assert_eq!(ranked[1].title, Some("Renegade".to_string())); // score 110
    assert_eq!(ranked[2].title, Some("Trigger Rename".to_string())); // score 70
}
