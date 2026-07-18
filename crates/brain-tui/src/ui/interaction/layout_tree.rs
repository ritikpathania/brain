//! Width-aware layout tree definitions and compilers.

use crate::ui::interaction::ast::{
    BlockId, CitationId, DocumentBlock, InlineNode, LinkTarget, ListKind, TableNode,
};
use crate::ui::interaction::lexer::{SyntaxHighlighterRegistry, TokenKind};
use std::borrow::Cow;

/// Semantic style categories resolved by the presentation theme.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VisualStyle {
    /// Regular default text.
    Normal,
    /// Heading level 1 style.
    Heading1,
    /// Heading level 2 style.
    Heading2,
    /// Heading level 3 style.
    Heading3,
    /// Bold emphasis style.
    Bold,
    /// Italic emphasis style.
    Italic,
    /// Inline monospaced code.
    InlineCode,
    /// Blockquote left border or margin style.
    BlockQuote,
    /// List bullet or ordered index prefix.
    ListBullet,
    /// Table header row style.
    TableHeader,
    /// Table regular row cell style.
    TableCell,
    /// Table border grid style.
    TableBorder,
    /// Horizontal rule divider.
    HorizontalRule,
}

/// Interactive actions mapped directly inside visual spans.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SpanAction {
    /// No interactive action.
    None,
    /// Interactive hyperlink target.
    Hyperlink(LinkTarget),
    /// Footnote or citation target key.
    CitationTarget(CitationId),
}

/// UI-agnostic visual span representing styled text.
pub struct VisualSpan {
    /// Text segment string content.
    pub text: Cow<'static, str>,
    /// Semantic styling category.
    pub style: VisualStyle,
    /// Syntax highlighting token classification.
    pub token_kind: Option<TokenKind>,
    /// Interactive action descriptor.
    pub action: SpanAction,
}

impl VisualSpan {
    /// Instantiates a new VisualSpan.
    pub fn new<T: Into<Cow<'static, str>>>(
        text: T,
        style: VisualStyle,
        token_kind: Option<TokenKind>,
        action: SpanAction,
    ) -> Self {
        Self {
            text: text.into(),
            style,
            token_kind,
            action,
        }
    }
}

/// A formatted visual line containing styled text segments.
pub struct VisualLine {
    /// Sequenced visual spans.
    pub spans: Vec<VisualSpan>,
}

/// Structured geometry block within the laid out document.
pub struct LayoutBlock {
    /// Parent block ID.
    pub id: BlockId,
    /// Block classification type.
    pub kind: VisualBlockKind,
    /// Formatted visual lines.
    pub lines: Vec<VisualLine>,
}

/// Classification of layout blocks.
pub enum VisualBlockKind {
    /// Text flow paragraph block.
    Paragraph,
    /// Section title heading with level.
    Heading(u8),
    /// Syntax-highlighted code block.
    CodeBlock,
    /// Aligned table grid block.
    Table {
        /// Assigned column widths for cell rendering.
        col_widths: Vec<usize>,
    },
    /// Indented list block.
    List,
    /// Left-bordered blockquote.
    BlockQuote,
    /// Horizontal line divider.
    HorizontalRule,
}

/// Root LayoutTree holding compiled layout blocks.
pub struct LayoutTree {
    blocks: Vec<LayoutBlock>,
}

impl LayoutTree {
    /// Creates a new LayoutTree containing blocks.
    pub fn new(blocks: Vec<LayoutBlock>) -> Self {
        Self { blocks }
    }

    /// Access read-only layout blocks.
    pub fn blocks(&self) -> &[LayoutBlock] {
        &self.blocks
    }
}

/// Layout Compiler.
pub struct LayoutEngine;

impl LayoutEngine {
    /// Compiles a list of semantic blocks into a layout tree matching the maximum width boundary.
    pub fn compile(blocks: &[DocumentBlock], width: usize) -> LayoutTree {
        let max_width = if width < 5 { 5 } else { width };
        let mut layout_blocks = Vec::new();

        for (idx, block) in blocks.iter().enumerate() {
            let id = BlockId::generate(&format!("{:?}", block), idx);
            match block {
                DocumentBlock::Heading { level, content } => {
                    let style = match level {
                        1 => VisualStyle::Heading1,
                        2 => VisualStyle::Heading2,
                        _ => VisualStyle::Heading3,
                    };
                    let spans = compile_inlines(content, style);
                    let wrapped = wrap_spans(spans, max_width);
                    let lines = wrapped
                        .into_iter()
                        .map(|line| VisualLine { spans: line })
                        .collect();
                    layout_blocks.push(LayoutBlock {
                        id,
                        kind: VisualBlockKind::Heading(*level),
                        lines,
                    });
                }
                DocumentBlock::Paragraph(inlines) => {
                    let spans = compile_inlines(inlines, VisualStyle::Normal);
                    let wrapped = wrap_spans(spans, max_width);
                    let lines = wrapped
                        .into_iter()
                        .map(|line| VisualLine { spans: line })
                        .collect();
                    layout_blocks.push(LayoutBlock {
                        id,
                        kind: VisualBlockKind::Paragraph,
                        lines,
                    });
                }
                DocumentBlock::CodeBlock {
                    language,
                    lines: code_lines,
                } => {
                    let mut lines = Vec::new();
                    for line in code_lines {
                        let highlighted: Vec<_> =
                            SyntaxHighlighterRegistry::highlight(*language, line)
                                .map(|span| {
                                    VisualSpan::new(
                                        span.text.to_string(),
                                        VisualStyle::Normal,
                                        Some(span.kind),
                                        SpanAction::None,
                                    )
                                })
                                .collect();

                        let wrapped = wrap_spans(highlighted, max_width);
                        for line_spans in wrapped {
                            lines.push(VisualLine { spans: line_spans });
                        }
                    }
                    layout_blocks.push(LayoutBlock {
                        id,
                        kind: VisualBlockKind::CodeBlock,
                        lines,
                    });
                }
                DocumentBlock::List { kind, items } => {
                    let mut lines = Vec::new();
                    let bullet = match kind {
                        ListKind::Unordered => "• ".to_string(),
                        ListKind::Ordered => "1. ".to_string(),
                    };

                    for item in items {
                        let bullet_span = VisualSpan::new(
                            bullet.clone(),
                            VisualStyle::ListBullet,
                            None,
                            SpanAction::None,
                        );
                        let mut item_spans = vec![bullet_span];
                        item_spans.extend(compile_inlines(item, VisualStyle::Normal));

                        // Lists are indented by bullet width
                        let wrapped = wrap_spans(item_spans, max_width.saturating_sub(2));
                        for (i, line_spans) in wrapped.into_iter().enumerate() {
                            let mut line = Vec::new();
                            if i > 0 {
                                // Add indentation padding spaces
                                line.push(VisualSpan::new(
                                    "  ",
                                    VisualStyle::Normal,
                                    None,
                                    SpanAction::None,
                                ));
                            }
                            line.extend(line_spans);
                            lines.push(VisualLine { spans: line });
                        }
                    }

                    layout_blocks.push(LayoutBlock {
                        id,
                        kind: VisualBlockKind::List,
                        lines,
                    });
                }
                DocumentBlock::BlockQuote(nested) => {
                    let nested_tree = Self::compile(nested, max_width.saturating_sub(2));
                    let mut lines = Vec::new();
                    for b in nested_tree.blocks() {
                        for line in &b.lines {
                            let mut quoted_line = vec![VisualSpan::new(
                                "│ ",
                                VisualStyle::BlockQuote,
                                None,
                                SpanAction::None,
                            )];
                            quoted_line.extend(line.spans.iter().map(|s| VisualSpan {
                                text: s.text.clone(),
                                style: s.style,
                                token_kind: s.token_kind,
                                action: s.action.clone(),
                            }));
                            lines.push(VisualLine { spans: quoted_line });
                        }
                    }
                    layout_blocks.push(LayoutBlock {
                        id,
                        kind: VisualBlockKind::BlockQuote,
                        lines,
                    });
                }
                DocumentBlock::HorizontalRule => {
                    let hr_str = "─".repeat(max_width);
                    let line = VisualLine {
                        spans: vec![VisualSpan::new(
                            hr_str,
                            VisualStyle::HorizontalRule,
                            None,
                            SpanAction::None,
                        )],
                    };
                    layout_blocks.push(LayoutBlock {
                        id,
                        kind: VisualBlockKind::HorizontalRule,
                        lines: vec![line],
                    });
                }
                DocumentBlock::Table(table) => {
                    let layout_lines = compile_table_layout(table, max_width);
                    layout_blocks.push(LayoutBlock {
                        id,
                        kind: VisualBlockKind::Table { col_widths: vec![] }, // Re-evaluated during render
                        lines: layout_lines,
                    });
                }
            }
        }

        LayoutTree::new(layout_blocks)
    }
}

fn compile_inlines(inlines: &[InlineNode], default_style: VisualStyle) -> Vec<VisualSpan> {
    let mut spans = Vec::new();
    for node in inlines {
        match node {
            InlineNode::Text(t) => {
                spans.push(VisualSpan::new(
                    t.clone(),
                    default_style,
                    None,
                    SpanAction::None,
                ));
            }
            InlineNode::Code(t) => {
                spans.push(VisualSpan::new(
                    t.clone(),
                    VisualStyle::InlineCode,
                    None,
                    SpanAction::None,
                ));
            }
            InlineNode::Strong(children) => {
                spans.extend(compile_inlines(children, VisualStyle::Bold));
            }
            InlineNode::Emphasis(children) => {
                spans.extend(compile_inlines(children, VisualStyle::Italic));
            }
            InlineNode::Link { children, url } => {
                let inner = compile_inlines(children, default_style);
                for mut s in inner {
                    s.action = SpanAction::Hyperlink(url.clone());
                    spans.push(s);
                }
            }
            InlineNode::Citation(id) => {
                spans.push(VisualSpan::new(
                    format!("[{}]", id.0),
                    VisualStyle::TableCell,
                    None,
                    SpanAction::CitationTarget(id.clone()),
                ));
            }
        }
    }
    spans
}

fn wrap_spans(spans: Vec<VisualSpan>, max_width: usize) -> Vec<Vec<VisualSpan>> {
    let mut lines = Vec::new();
    let mut current_line = Vec::new();
    let mut current_width = 0;

    for span in spans {
        let text = &span.text;
        let style = span.style;
        let token_kind = span.token_kind;
        let action = span.action.clone();

        let words = text.split_inclusive(char::is_whitespace);

        for word in words {
            let word_len = word.chars().count();
            if current_width + word_len > max_width {
                if !current_line.is_empty() {
                    lines.push(current_line);
                    current_line = Vec::new();
                    current_width = 0;
                }
                if word_len > max_width {
                    let word_chars: Vec<char> = word.chars().collect();
                    for chunk in word_chars.chunks(max_width) {
                        let chunk_str: String = chunk.iter().collect();
                        lines.push(vec![VisualSpan {
                            text: Cow::Owned(chunk_str),
                            style,
                            token_kind,
                            action: action.clone(),
                        }]);
                    }
                    continue;
                }
            }
            current_line.push(VisualSpan {
                text: Cow::Owned(word.to_string()),
                style,
                token_kind,
                action: action.clone(),
            });
            current_width += word_len;
        }
    }
    if !current_line.is_empty() {
        lines.push(current_line);
    }
    lines
}

fn compile_table_layout(table: &TableNode, max_width: usize) -> Vec<VisualLine> {
    let col_count = std::cmp::max(
        table.headers.len(),
        table.rows.iter().map(|r| r.len()).max().unwrap_or(0),
    );
    if col_count == 0 {
        return Vec::new();
    }

    let mut col_widths = vec![3; col_count];
    // Simple width allocation: divide max_width evenly
    let cell_width = max_width.saturating_sub(col_count * 3 + 1) / col_count;
    let cell_width = if cell_width < 2 { 2 } else { cell_width };
    col_widths.fill(cell_width);

    let mut lines = Vec::new();

    // Table Header line
    let mut header_spans = Vec::new();
    header_spans.push(VisualSpan::new(
        "│ ",
        VisualStyle::TableBorder,
        None,
        SpanAction::None,
    ));
    for (i, cell) in table.headers.iter().enumerate() {
        let width = col_widths[i];
        let spans = compile_inlines(&cell.content, VisualStyle::TableHeader);
        let text_repr = spans
            .iter()
            .map(|s| s.text.to_string())
            .collect::<Vec<_>>()
            .join("");
        header_spans.push(VisualSpan::new(
            truncate_or_pad(&text_repr, width),
            VisualStyle::TableHeader,
            None,
            SpanAction::None,
        ));
        header_spans.push(VisualSpan::new(
            " │ ",
            VisualStyle::TableBorder,
            None,
            SpanAction::None,
        ));
    }
    lines.push(VisualLine {
        spans: header_spans,
    });

    // Table Rows
    for row in &table.rows {
        let mut row_spans = Vec::new();
        row_spans.push(VisualSpan::new(
            "│ ",
            VisualStyle::TableBorder,
            None,
            SpanAction::None,
        ));
        for (i, width) in col_widths.iter().enumerate().take(col_count) {
            let cell = row.get(i);
            let cell_text = if let Some(c) = cell {
                let spans = compile_inlines(&c.content, VisualStyle::TableCell);
                spans
                    .iter()
                    .map(|s| s.text.to_string())
                    .collect::<Vec<_>>()
                    .join("")
            } else {
                "".to_string()
            };
            row_spans.push(VisualSpan::new(
                truncate_or_pad(&cell_text, *width),
                VisualStyle::TableCell,
                None,
                SpanAction::None,
            ));
            row_spans.push(VisualSpan::new(
                " │ ",
                VisualStyle::TableBorder,
                None,
                SpanAction::None,
            ));
        }
        lines.push(VisualLine { spans: row_spans });
    }

    lines
}

fn truncate_or_pad(text: &str, width: usize) -> String {
    let count = text.chars().count();
    if count > width {
        let chars: Vec<char> = text.chars().collect();
        chars[..width].iter().collect()
    } else {
        let padding = width - count;
        format!("{}{}", text, " ".repeat(padding))
    }
}
