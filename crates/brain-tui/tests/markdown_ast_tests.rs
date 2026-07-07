use brain_tui::ui::interaction::ast::{DocumentBlock, InlineNode, LanguageId};
use brain_tui::ui::interaction::markdown::parser::MarkdownParser;

#[test]
fn test_recursive_inline_parsing() {
    let input = "**hello `code` world**";
    let blocks = MarkdownParser::parse_to_blocks(input);
    assert_eq!(blocks.len(), 1);
    match &blocks[0] {
        DocumentBlock::Paragraph(inlines) => {
            assert_eq!(inlines.len(), 1);
            match &inlines[0] {
                InlineNode::Strong(children) => {
                    assert_eq!(children.len(), 3);
                    assert!(matches!(children[0], InlineNode::Text(ref s) if s == "hello "));
                    assert!(matches!(children[1], InlineNode::Code(ref s) if s == "code"));
                    assert!(matches!(children[2], InlineNode::Text(ref s) if s == " world"));
                }
                _ => panic!("Expected Strong node"),
            }
        }
        _ => panic!("Expected Paragraph block"),
    }
}

#[test]
fn test_blockquote_recursive_parsing() {
    let input = "> # Heading\n> \n> Plain text paragraph.";
    let blocks = MarkdownParser::parse_to_blocks(input);
    assert_eq!(blocks.len(), 1);
    match &blocks[0] {
        DocumentBlock::BlockQuote(nested) => {
            assert_eq!(nested.len(), 2);
            assert!(matches!(nested[0], DocumentBlock::Heading { level: 1, .. }));
            assert!(matches!(nested[1], DocumentBlock::Paragraph(_)));
        }
        _ => panic!("Expected BlockQuote block"),
    }
}

#[test]
fn test_code_block_language_parsing() {
    let input = "```rs\nfn main() {}\n```";
    let blocks = MarkdownParser::parse_to_blocks(input);
    assert_eq!(blocks.len(), 1);
    match &blocks[0] {
        DocumentBlock::CodeBlock { language, lines } => {
            assert_eq!(*language, LanguageId::Rust);
            assert_eq!(lines.len(), 1);
            assert_eq!(lines[0], "fn main() {}");
        }
        _ => panic!("Expected CodeBlock"),
    }
}
