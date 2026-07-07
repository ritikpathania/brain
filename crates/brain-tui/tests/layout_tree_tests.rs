use brain_tui::ui::interaction::ast::{DocumentBlock, InlineNode, TableCell, TableNode};
use brain_tui::ui::interaction::layout_tree::{LayoutEngine, VisualStyle};

#[test]
fn test_heading_layout_wrapping() {
    let heading = DocumentBlock::Heading {
        level: 1,
        content: vec![InlineNode::Text("A very long title heading text that wraps".to_string())],
    };
    let tree = LayoutEngine::compile(&[heading], 15);
    let blocks = tree.blocks();
    assert_eq!(blocks.len(), 1);
    assert!(blocks[0].lines.len() > 1);
    assert_eq!(blocks[0].lines[0].spans[0].style, VisualStyle::Heading1);
}

#[test]
fn test_paragraph_and_inline_code_layout() {
    let p = DocumentBlock::Paragraph(vec![
        InlineNode::Text("This is ".to_string()),
        InlineNode::Code("code".to_string()),
        InlineNode::Text(" text.".to_string()),
    ]);
    let tree = LayoutEngine::compile(&[p], 80);
    let blocks = tree.blocks();
    assert_eq!(blocks.len(), 1);
    
    let line = &blocks[0].lines[0];
    // Check if Code style is assigned correctly
    let code_spans: Vec<_> = line.spans.iter().filter(|s| s.style == VisualStyle::InlineCode).collect();
    assert_eq!(code_spans.len(), 1);
    assert_eq!(code_spans[0].text, "code");
}

#[test]
fn test_table_layout_formatting() {
    let table = DocumentBlock::Table(TableNode {
        headers: vec![
            TableCell { content: vec![InlineNode::Text("Key".to_string())] },
            TableCell { content: vec![InlineNode::Text("Value".to_string())] },
        ],
        rows: vec![
            vec![
                TableCell { content: vec![InlineNode::Text("Age".to_string())] },
                TableCell { content: vec![InlineNode::Text("42".to_string())] },
            ]
        ],
    });
    
    let tree = LayoutEngine::compile(&[table], 20);
    let blocks = tree.blocks();
    assert_eq!(blocks.len(), 1);
    
    // Check headers and rows exist and contain TableBorder cells
    assert_eq!(blocks[0].lines.len(), 2);
    let border_spans: Vec<_> = blocks[0].lines[0].spans.iter().filter(|s| s.style == VisualStyle::TableBorder).collect();
    assert!(border_spans.len() >= 2);
}
