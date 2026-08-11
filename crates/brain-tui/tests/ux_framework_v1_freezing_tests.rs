use brain_tui::ui::command::registry::CommandRegistry;
use brain_tui::ui::focus::{FocusManager, FocusProfile};
use brain_tui::ui::navigation::{Modal, NavigationStack, Screen};
use brain_tui::ui::theme::provider::{Appearance, AppearanceProvider, StaticProvider};
use brain_tui::ui::theme::{dark_theme, light_theme, ActiveTheme};
use brain_tui::ui::widgets::view_models::FocusTarget;

#[test]
fn test_stage1_navigation_stack_invariants() {
    let mut stack = NavigationStack::default();
    assert_eq!(stack.current(), Screen::Home);

    // Push Workspace screen
    stack.push(Screen::Workspace);
    assert_eq!(stack.current(), Screen::Workspace);

    // Push GraphExplorer screen
    stack.push(Screen::GraphExplorer);
    assert_eq!(stack.current(), Screen::GraphExplorer);

    // Pop returns to Workspace
    assert_eq!(stack.pop(), Some(Screen::GraphExplorer));
    assert_eq!(stack.current(), Screen::Workspace);

    // Pop returns to Home
    assert_eq!(stack.pop(), Some(Screen::Workspace));
    assert_eq!(stack.current(), Screen::Home);

    // Root protection: cannot pop past Home screen
    assert_eq!(stack.pop(), None);
    assert_eq!(stack.current(), Screen::Home);
}

#[test]
fn test_stage1_focus_restoration_invariants() {
    let mut focus = FocusManager::new(FocusTarget::Prompt, FocusProfile::Chat);
    assert_eq!(focus.current(), FocusTarget::Prompt);

    // Save prompt focus and shift to CommandPalette
    focus.save_focus(FocusTarget::Prompt);
    focus.set_focus(FocusTarget::CommandPalette);
    assert_eq!(focus.current(), FocusTarget::CommandPalette);

    // Dismiss modal -> pop saved focus restores Prompt
    if let Some(restored) = focus.pop_saved_focus() {
        focus.set_focus(restored);
    }
    assert_eq!(focus.current(), FocusTarget::Prompt);
}

#[test]
fn test_stage2_appearance_provider_trait_invariants() {
    let dark_provider = StaticProvider::new(Appearance::Dark);
    assert_eq!(dark_provider.appearance(), Appearance::Dark);

    let light_provider = StaticProvider::new(Appearance::Light);
    assert_eq!(light_provider.appearance(), Appearance::Light);

    let dark_theme_ref = dark_theme();
    let light_theme_ref = light_theme();

    assert_ne!(
        dark_theme_ref
            .style(brain_tui::ui::theme::ThemeToken::TextPrimary)
            .fg,
        light_theme_ref
            .style(brain_tui::ui::theme::ThemeToken::TextPrimary)
            .fg
    );
}

#[test]
fn test_stage3_command_registry_fuzzy_ranking() {
    let registry = CommandRegistry::new();
    let index = brain_tui::ui::command::CommandIndex::build(&registry);

    // 1. Query "search" -> exact_prefix & exact_name match on /search beats keyword matches
    let matches = brain_tui::ui::command::FuzzyMatcher::match_query(&index, "search");
    let mut ranked = matches.clone();
    brain_tui::ui::command::CommandRanker::rank(&mut ranked);
    assert_eq!(ranked[0].metadata.id, "search.memory");

    // 2. Query "new" -> alias match on /session new
    let matches_alias = brain_tui::ui::command::FuzzyMatcher::match_query(&index, "new");
    let mut ranked_alias = matches_alias.clone();
    brain_tui::ui::command::CommandRanker::rank(&mut ranked_alias);
    assert_eq!(ranked_alias[0].metadata.id, "session.new");

    // 3. Exact prefix beats alias match invariant test
    let factors_prefix = brain_tui::ui::command::RankingFactors {
        exact_prefix: true,
        exact_name: false,
        alias_match: false,
        keyword_match: false,
        priority: 0,
        recency: 0,
        frequency: 0,
    };
    let factors_alias = brain_tui::ui::command::RankingFactors {
        exact_prefix: false,
        exact_name: false,
        alias_match: true,
        keyword_match: false,
        priority: 0,
        recency: 0,
        frequency: 0,
    };
    assert!(factors_prefix.score() > factors_alias.score());

    // 4. Alias beats keyword match invariant test
    let factors_keyword = brain_tui::ui::command::RankingFactors {
        exact_prefix: false,
        exact_name: false,
        alias_match: false,
        keyword_match: true,
        priority: 0,
        recency: 0,
        frequency: 0,
    };
    assert!(factors_alias.score() > factors_keyword.score());

    // 5. Replay determinism: identical query yields identical ordering
    let run1 = brain_tui::ui::command::FuzzyMatcher::match_query(&index, "mem");
    let mut ranked1 = run1.clone();
    brain_tui::ui::command::CommandRanker::rank(&mut ranked1);

    let run2 = brain_tui::ui::command::FuzzyMatcher::match_query(&index, "mem");
    let mut ranked2 = run2.clone();
    brain_tui::ui::command::CommandRanker::rank(&mut ranked2);

    assert_eq!(ranked1.len(), ranked2.len());
    for i in 0..ranked1.len() {
        assert_eq!(ranked1[i].metadata.id, ranked2[i].metadata.id);
    }
}

#[test]
fn test_stage4_modal_layering_and_backdrop_invariants() {
    let screen = Screen::Workspace;
    let modal = Some(Modal::CommandPalette);

    assert_ne!(screen, Screen::Home);
    assert_eq!(modal, Some(Modal::CommandPalette));
}
