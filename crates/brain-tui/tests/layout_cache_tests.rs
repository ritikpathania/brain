use brain_tui::ui::interaction::ast::{DocumentBlock, InlineNode};
use brain_tui::ui::interaction::layout_tree::LayoutEngine;

#[test]
fn test_layout_compilation_determinism() {
    let blocks = vec![
        DocumentBlock::Heading {
            level: 1,
            content: vec![InlineNode::Text("Determinism Test".to_string())],
        },
        DocumentBlock::Paragraph(vec![
            InlineNode::Text("This is a simple paragraph to verify that compilation is deterministic.".to_string())
        ])
    ];

    let tree_1 = LayoutEngine::compile(&blocks, 40);
    let tree_2 = LayoutEngine::compile(&blocks, 40);

    let blocks_1 = tree_1.blocks();
    let blocks_2 = tree_2.blocks();

    assert_eq!(blocks_1.len(), blocks_2.len());
    for i in 0..blocks_1.len() {
        assert_eq!(blocks_1[i].lines.len(), blocks_2[i].lines.len());
        for j in 0..blocks_1[i].lines.len() {
            let line_1 = &blocks_1[i].lines[j];
            let line_2 = &blocks_2[i].lines[j];
            assert_eq!(line_1.spans.len(), line_2.spans.len());
            for k in 0..line_1.spans.len() {
                assert_eq!(line_1.spans[k].text, line_2.spans[k].text);
                assert_eq!(line_1.spans[k].style, line_2.spans[k].style);
                assert_eq!(line_1.spans[k].token_kind, line_2.spans[k].token_kind);
                assert_eq!(line_1.spans[k].action, line_2.spans[k].action);
            }
        }
    }
}
