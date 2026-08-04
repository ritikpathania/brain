use brain_tui::ui::search::aggregator::SearchAggregator;
use brain_tui::ui::search::types::{
    ProviderId, ProviderStatus, SearchEvent, SearchFailure, SearchGeneration, SearchResult,
    SearchResultAction, SearchResultKind, PROVIDER_LOCAL_MESSAGES, PROVIDER_REMOTE_MESSAGES,
};
use std::collections::HashMap;

fn make_dummy_result(title: &str, score: i32) -> SearchResult {
    SearchResult {
        entity_id: String::new(), // Commands/sessions have no knowledge-graph ID
        title: Some(title.to_string()),
        subtitle: Some("description".to_string()),
        kind: SearchResultKind::Session,
        provider_score: score,
        confidence: brain_tui::client::Confidence::Medium,
        action: SearchResultAction::InvokeCommand(brain_tui::ui::command::CommandId("dummy")),
    }
}

#[test]
fn test_aggregator_generation_filtering() {
    let p_local = PROVIDER_LOCAL_MESSAGES;
    let p_remote = PROVIDER_REMOTE_MESSAGES;
    let mut aggregator = SearchAggregator::new(vec![p_local, p_remote]);

    // Set generation to 2 by starting local search for generation 2
    aggregator.handle_event(SearchEvent::Started {
        generation: SearchGeneration(2),
        provider: p_local,
    });

    // Older event (generation 1) should be ignored
    aggregator.handle_event(SearchEvent::Results {
        generation: SearchGeneration(1),
        provider: p_remote,
        results: vec![make_dummy_result("Stale result", 100)],
    });

    let state = aggregator.view_state();
    assert_eq!(state.results().len(), 0); // Discarded

    // Newer event (generation 2) should be accepted
    aggregator.handle_event(SearchEvent::Results {
        generation: SearchGeneration(2),
        provider: p_local,
        results: vec![make_dummy_result("Fresh local result", 50)],
    });

    let state2 = aggregator.view_state();
    assert_eq!(state2.results().len(), 1);
    assert_eq!(
        state2.results()[0].title,
        Some("Fresh local result".to_string())
    );
}

#[test]
fn test_aggregator_duplicate_results_replace() {
    let p_local = PROVIDER_LOCAL_MESSAGES;
    let mut aggregator = SearchAggregator::new(vec![p_local]);

    aggregator.handle_event(SearchEvent::Started {
        generation: SearchGeneration(1),
        provider: p_local,
    });

    // Send first batch
    aggregator.handle_event(SearchEvent::Results {
        generation: SearchGeneration(1),
        provider: p_local,
        results: vec![make_dummy_result("Result A", 10)],
    });

    // Send second batch from same provider in same generation (should replace first batch)
    aggregator.handle_event(SearchEvent::Results {
        generation: SearchGeneration(1),
        provider: p_local,
        results: vec![make_dummy_result("Result B", 20)],
    });

    let state = aggregator.view_state();
    assert_eq!(state.results().len(), 1);
    assert_eq!(state.results()[0].title, Some("Result B".to_string()));
}

#[test]
fn test_aggregator_out_of_order_finished_and_completeness() {
    let p_local = PROVIDER_LOCAL_MESSAGES;
    let p_remote = PROVIDER_REMOTE_MESSAGES;
    let mut aggregator = SearchAggregator::new(vec![p_local, p_remote]);

    // Send Finished before Started (should be processed without panic and update status)
    aggregator.handle_event(SearchEvent::Finished {
        generation: SearchGeneration(1),
        provider: p_local,
    });

    let state = aggregator.view_state();
    let statuses: HashMap<ProviderId, ProviderStatus> =
        state.statuses().map(|(&k, &v)| (k, v)).collect();
    assert_eq!(statuses.get(&p_local), Some(&ProviderStatus::Completed));
    assert_eq!(statuses.get(&p_remote), Some(&ProviderStatus::Idle));
    assert!(!aggregator.is_complete()); // remote is still Idle

    // Mark remote as failed
    aggregator.handle_event(SearchEvent::Failed {
        generation: SearchGeneration(1),
        provider: p_remote,
        reason: SearchFailure::Timeout,
    });

    let state2 = aggregator.view_state();
    let statuses2: HashMap<ProviderId, ProviderStatus> =
        state2.statuses().map(|(&k, &v)| (k, v)).collect();
    assert_eq!(
        statuses2.get(&p_remote),
        Some(&ProviderStatus::Failed(SearchFailure::Timeout))
    );
    assert!(aggregator.is_complete()); // Both completed/failed
}

#[test]
fn test_aggregator_view_state_idempotence() {
    let p_local = PROVIDER_LOCAL_MESSAGES;
    let mut aggregator = SearchAggregator::new(vec![p_local]);

    aggregator.handle_event(SearchEvent::Results {
        generation: SearchGeneration(1),
        provider: p_local,
        results: vec![make_dummy_result("Constant", 10)],
    });

    let state_a = aggregator.view_state();
    let state_b = aggregator.view_state();

    assert_eq!(state_a.generation(), state_b.generation());
    assert_eq!(state_a.query(), state_b.query());
    assert_eq!(state_a.results(), state_b.results());
}
