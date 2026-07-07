use brain_tui::ui::interaction::navigation::{NavigationSolver, ConversationViewState, ConversationNodeId};
use brain_tui::ui::interaction::layout_tree::{LayoutTree, LayoutBlock, VisualBlockKind};
use brain_tui::ui::interaction::ast::BlockId;
use brain_tui::ui::interaction::MessageId;

#[test]
fn test_navigation_solver_uniqueness_and_selection_existence() {
    let blocks = vec![
        LayoutBlock { id: BlockId(1), kind: VisualBlockKind::Paragraph, lines: vec![] }
    ];
    let tree = LayoutTree::new(blocks);
    
    let mut view_state = ConversationViewState::default();
    let selected = ConversationNodeId::Message(MessageId(42));
    view_state.selected_node = Some(selected.clone());
    
    let layouts = vec![(MessageId(10), &tree)];
    let index = NavigationSolver::solve(&layouts, &view_state);
    
    // Invariant 1: every selected node must exist in the resulting NavigationIndex
    assert!(index.nodes.contains(&selected));
    
    // Invariant 2: all nodes in the index must be unique
    let mut unique = index.nodes.clone();
    unique.sort_by_key(|n| format!("{:?}", n));
    let orig_len = unique.len();
    unique.dedup();
    assert_eq!(unique.len(), orig_len);
}
