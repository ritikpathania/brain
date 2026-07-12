mod common;

use std::borrow::Cow;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Style, Modifier, Color};
use brain_tui::ui::theme::{dark_theme, ActiveTheme};
use brain_tui::ui::render::{RenderContext, IconSet};
use brain_tui::ui::interaction::markdown::{
    MarkdownParser, MarkdownLayout, KeywordSyntaxHighlighter,
    SelectionState, VisualSpan, VisualStyle, VisualLine, VisualLineKind,
    MarkdownBlock, InlineNode, TableNode, CitationNode
};

fn map_span_for_test(span: &VisualSpan, theme: &Style, is_selected: bool) -> ratatui::text::Span<'static> {
    let mut style = Style::default();

    match span.style {
        VisualStyle::Heading1 => {
            style = style.fg(Color::Cyan).add_modifier(Modifier::BOLD);
        }
        VisualStyle::Heading2 => {
            style = style.fg(Color::Cyan).add_modifier(Modifier::BOLD);
        }
        VisualStyle::Heading3 => {
            style = style.fg(Color::Cyan).add_modifier(Modifier::BOLD);
        }
        VisualStyle::Bold => {
            style = style.add_modifier(Modifier::BOLD);
        }
        VisualStyle::Italic => {
            style = style.add_modifier(Modifier::ITALIC);
        }
        VisualStyle::InlineCode => {
            style = style.fg(Color::Yellow).bg(Color::DarkGray);
        }
        VisualStyle::CodeKeyword => {
            style = style.fg(Color::Magenta).add_modifier(Modifier::BOLD);
        }
        VisualStyle::CodeComment => {
            style = style.fg(Color::Gray);
        }
        VisualStyle::TableHeader => {
            style = style.fg(Color::Green).add_modifier(Modifier::BOLD);
        }
        VisualStyle::TableCell => {
            style = style.fg(Color::White);
        }
        VisualStyle::Citation => {
            style = style.fg(Color::Blue).add_modifier(Modifier::UNDERLINED);
        }
        VisualStyle::Selected => {
            style = style.bg(Color::LightBlue).fg(Color::Black);
        }
        VisualStyle::Normal => {
            style = *theme;
        }
    }

    if is_selected {
        style = style.bg(Color::LightBlue).fg(Color::Black);
    }

    ratatui::text::Span::styled(span.text.to_string(), style)
}

fn render_visual_lines(
    buf: &mut Buffer,
    area: Rect,
    lines: &[VisualLine],
    theme: &Style,
    selection: &SelectionState,
) {
    let mut y = area.y;
    for (idx, visual_line) in lines.iter().enumerate() {
        if y >= area.y + area.height {
            break;
        }
        let is_sel = selection.is_selected(idx);
        let mut x = area.x;
        for span in &visual_line.spans {
            let mapped = map_span_for_test(span, theme, is_sel);
            let text = &mapped.content;
            let style = mapped.style;
            let chars_count = text.chars().count();
            if x + chars_count as u16 > area.x + area.width {
                break;
            }
            buf.set_string(x, y, text.as_ref(), style);
            x += chars_count as u16;
        }
        y += 1;
    }
}

#[test]
fn test_markdown_ast_parser() {
    // 1. Happy Path Parsing
    let text = "# H1 Heading
## H2 Heading

Some **bold** and *italic* text with `code`.

```rust
fn test() {}
```

| H1 | H2 |
|---|---|
| C1 | C2 |

[1]: reference content";
    let ast = MarkdownParser::parse(text);

    assert_eq!(ast.blocks.len(), 6);
    assert!(matches!(ast.blocks[0], MarkdownBlock::Heading { level: 1, .. }));
    assert!(matches!(ast.blocks[1], MarkdownBlock::Heading { level: 2, .. }));
    assert!(matches!(ast.blocks[2], MarkdownBlock::Paragraph(_)));
    assert!(matches!(ast.blocks[3], MarkdownBlock::CodeBlock { .. }));
    assert!(matches!(ast.blocks[4], MarkdownBlock::Table(_)));
    assert!(matches!(ast.blocks[5], MarkdownBlock::Citation(_)));

    // 2. Parser Robustness / Malformed Cases
    let malformed = "Some **unclosed bold and `unclosed code block

| Col1 | Col2
|---|";
    let ast_malformed = MarkdownParser::parse(malformed);
    assert!(!ast_malformed.blocks.is_empty());
}

#[test]
fn test_markdown_layout_width() {
    let text = "This is a very long paragraph that should be wrapped across multiple lines under small width constraints.";
    let ast = MarkdownParser::parse(text);
    let highlighter = KeywordSyntaxHighlighter::new();

    // Narrow layout wrapping
    let narrow = MarkdownLayout::layout(&ast, 20, &highlighter);
    assert!(narrow.len() > 3);

    // Wide layout wrapping
    let wide = MarkdownLayout::layout(&ast, 100, &highlighter);
    assert_eq!(wide.len(), 2);
}

#[test]
fn test_markdown_table_alignment() {
    let text = "| Name | Description |
|---|---|
| Rust | Multi-paradigm language |
| Go | Concurrent language |";
    let ast = MarkdownParser::parse(text);
    let highlighter = KeywordSyntaxHighlighter::new();

    let layout = MarkdownLayout::layout(&ast, 80, &highlighter);
    assert!(layout.len() >= 3);
    for line in &layout {
        assert_eq!(line.kind, VisualLineKind::Table);
    }
}

#[test]
fn test_selection_cooperative() {
    let text = "Line 1
Line 2
Line 3";
    let ast = MarkdownParser::parse(text);
    let highlighter = KeywordSyntaxHighlighter::new();
    let layout = MarkdownLayout::layout(&ast, 80, &highlighter);

    let mut selection = SelectionState::new();
    selection.select(1, 2);

    assert!(!selection.is_selected(0));
    assert!(selection.is_selected(1));
    assert!(selection.is_selected(2));
}

#[test]
fn test_rich_rendering_snapshots() {
    let theme = dark_theme();
    let icons = IconSet::new(true);
    let capabilities = common::mock_capabilities();
    let ctx = RenderContext { theme: theme.clone(), icons: &icons, capabilities, tick: 0 };
    let text_style = theme.text;
    let highlighter = KeywordSyntaxHighlighter::new();
    let selection_none = SelectionState::new();

    let test_cases = vec![
        ("rich_markdown_headers", "# Heading 1
## Heading 2
### Heading 3", 80, &selection_none),
        ("rich_markdown_code_block", "```rust
fn main() {
    let value = 42;
    return value;
}
```", 80, &selection_none),
        ("rich_markdown_table", "| Item | Qty | Cost |
|---|---|---|
| Apple | 10 | $5 |
| Orange | 5 | $3 |", 80, &selection_none),
        ("rich_markdown_inline_styles", "This is **bold** text, *italic* text, and `inline code` spans.", 80, &selection_none),
        ("rich_markdown_nested_formatting", "Text containing [1] citation and `code` keywords like fn and let.", 80, &selection_none),
        ("rich_markdown_malformed", "Unterminated **bold format `code fence
| Unfinished Table", 80, &selection_none),
        ("rich_markdown_wrapped_heading", "# A very long heading that must wrap under narrow width limits", 30, &selection_none),
        ("rich_markdown_resize", "Responsive text resizing verification.", 15, &selection_none),
    ];

    for (name, content, width, selection) in test_cases {
        let ast = MarkdownParser::parse(content);
        let layout = MarkdownLayout::layout(&ast, width, &highlighter);

        let area = Rect::new(0, 0, width as u16, layout.len() as u16);
        let mut buf = Buffer::empty(area);
        render_visual_lines(&mut buf, area, &layout, &text_style, selection);
        common::assert_snapshot(&buf, &ctx, &format!("screens/chat/{}", name));
    }

    // Snapshot: rich_markdown_selected
    {
        let ast = MarkdownParser::parse("Line 1 text
Line 2 text
Line 3 text");
        let layout = MarkdownLayout::layout(&ast, 80, &highlighter);
        let mut selection = SelectionState::new();
        selection.select(0, 1);

        let area = Rect::new(0, 0, 80, layout.len() as u16);
        let mut buf = Buffer::empty(area);
        render_visual_lines(&mut buf, area, &layout, &text_style, &selection);
        common::assert_snapshot(&buf, &ctx, "screens/chat/rich_markdown_selected");
    }
}
