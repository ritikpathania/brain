use brain_tui::ui::search::ranking::RankingEngine;
use brain_tui::ui::search::types::{SearchResult, SearchResultAction, SearchResultKind};

fn make_dummy_result(title: &str, kind: SearchResultKind, score: i32) -> SearchResult {
    SearchResult {
        title: title.to_string(),
        subtitle: "dummy description".to_string(),
        kind,
        provider_score: score,
        action: SearchResultAction::InvokeCommand(brain_tui::ui::command::CommandId("dummy")),
    }
}

#[test]
fn test_ranking_determinism_and_stable_sort() {
    let engine = RankingEngine;

    // Test exact alphabetical sorting fallback when scores and boosts are identical
    let results = vec![
        make_dummy_result("Z Session", SearchResultKind::Session, 5),
        make_dummy_result("A Session", SearchResultKind::Session, 5),
        make_dummy_result("M Session", SearchResultKind::Session, 5),
    ];

    let ranked = engine.rank("session", results.clone());
    assert_eq!(ranked.len(), 3);
    assert_eq!(ranked[0].title, "A Session");
    assert_eq!(ranked[1].title, "M Session");
    assert_eq!(ranked[2].title, "Z Session");

    // Determinism: calling it again on the same input yields identical order
    let ranked_second = engine.rank("session", results);
    assert_eq!(ranked, ranked_second);
}

#[test]
fn test_ranking_idempotency() {
    let engine = RankingEngine;
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
    let engine = RankingEngine;
    let high1 = make_dummy_result("Perfect match", SearchResultKind::Command, 100);
    let high2 = make_dummy_result("Good match", SearchResultKind::Command, 50);
    let low = make_dummy_result("Irrelevant", SearchResultKind::Message, -100);

    let results_initial = vec![high1.clone(), high2.clone()];
    let ranked_initial = engine.rank("match", results_initial);

    // Relative order of high1 and high2 is:
    assert_eq!(ranked_initial[0].title, "Perfect match");
    assert_eq!(ranked_initial[1].title, "Good match");

    // Add the low-scoring result
    let results_extended = vec![high2, low, high1];
    let ranked_extended = engine.rank("match", results_extended);

    // Perfect match and Good match relative order should be preserved at the top
    assert_eq!(ranked_extended[0].title, "Perfect match");
    assert_eq!(ranked_extended[1].title, "Good match");
    assert_eq!(ranked_extended[2].title, "Irrelevant");
}

#[test]
fn test_ranking_boosts_additive() {
    let engine = RankingEngine;

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

    assert_eq!(ranked[0].title, "Rename Session"); // score 115
    assert_eq!(ranked[1].title, "Renegade"); // score 110
    assert_eq!(ranked[2].title, "Trigger Rename"); // score 70
}
