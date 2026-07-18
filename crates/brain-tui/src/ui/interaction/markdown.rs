//! Incremental markdown AST definitions, layout engines, and render caches.

use std::borrow::Cow;

pub use crate::ui::interaction::{ast, parser};

/// Immutable raw markdown document content container.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarkdownDocument {
    raw: String,
}

impl MarkdownDocument {
    /// Instantiates a new empty MarkdownDocument.
    pub fn new() -> Self {
        Self { raw: String::new() }
    }

    /// Access the underlying raw text slice.
    pub fn raw(&self) -> &str {
        &self.raw
    }

    /// Appends a new string chunk to the raw document content.
    pub fn append(&mut self, text: &str) {
        self.raw.push_str(text);
    }

    /// Clears the document raw content.
    pub fn clear(&mut self) {
        self.raw.clear();
    }
}

impl Default for MarkdownDocument {
    fn default() -> Self {
        Self::new()
    }
}

/// Logical AST block elements representing document structure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MarkdownBlock {
    /// Heading with level (1 to 6) and raw text content.
    Heading {
        /// Heading level (1 to 6).
        level: u8,
        /// Raw heading content text.
        text: String,
    },
    /// Paragraph grouped into inline style nodes.
    Paragraph(Vec<InlineNode>),
    /// Fenced pre-formatted block.
    CodeBlock {
        /// Programming language identifier if present.
        language: Option<String>,
        /// Inner lines of code.
        lines: Vec<String>,
    },
    /// Aligned table grid block.
    Table(TableNode),
    /// Footnote citation reference definition.
    Citation(CitationNode),
}

/// Logical inline nodes representing styled text spans.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InlineNode {
    /// Unstyled plain text.
    Text(String),
    /// Bold formatted segment.
    Bold(String),
    /// Italic formatted segment.
    Italic(String),
    /// Monospaced code segment.
    InlineCode(String),
    /// Footnote or literature citation reference token.
    Citation(String),
    /// Interactive graph node reference.
    EntityReference {
        /// Display label of the entity link.
        label: String,
        /// The referenced NodeId.
        node_id: brain_domain::NodeId,
    },
}

/// Aligned table data structure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableNode {
    /// Header row labels.
    pub headers: Vec<String>,
    /// Cell rows containing columns.
    pub rows: Vec<Vec<String>>,
}

/// Footnote citation structure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CitationNode {
    /// Readable tag label (e.g. "1" or "^ref").
    pub label: String,
    /// Absolute sequential index.
    pub index: usize,
}

/// Classification of mapped layout lines.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VisualLineKind {
    /// Heading line with level.
    Heading(u8),
    /// Regular body text flow line.
    Text,
    /// Code block line.
    Code,
    /// Aligned table cell/separator line.
    Table,
}

/// Semantic style categories resolved by the presentation theme.
///
/// **VisualStyle is a semantic contract, not a rendering contract.**
/// Existing variants are stable and should not be renamed. Future additions
/// must follow an append-only pattern, keeping color/mode specific variations
/// isolated entirely within the presentation-layer theme resolvers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum VisualStyle {
    /// Regular default style.
    #[default]
    Normal,
    /// H1 heading style.
    Heading1,
    /// H2 heading style.
    Heading2,
    /// H3 heading style.
    Heading3,
    /// Bold emphasis style.
    Bold,
    /// Italic emphasis style.
    Italic,
    /// Inline monospaced code style.
    InlineCode,
    /// Code syntax keyword style.
    CodeKeyword,
    /// Code comments style.
    CodeComment,
    /// Table header row style.
    TableHeader,
    /// Table regular cell row style.
    TableCell,
    /// Citation token/footnote style.
    Citation,
    /// Highlighted selected text style.
    Selected,
    /// Traversable entity reference link style.
    EntityReference(brain_domain::NodeId),
}

/// UI-agnostic visual span representing styled text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VisualSpan {
    /// Text content.
    pub text: Cow<'static, str>,
    /// Associated theme-agnostic semantic style category.
    pub style: VisualStyle,
}

impl VisualSpan {
    /// Instantiates a new VisualSpan.
    pub fn new<T: Into<Cow<'static, str>>>(text: T, style: VisualStyle) -> Self {
        Self {
            text: text.into(),
            style,
        }
    }
}

/// Mapped visual line comprising layout spans.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VisualLine {
    /// Line category.
    pub kind: VisualLineKind,
    /// Sequential visual spans.
    pub spans: Vec<VisualSpan>,
}

/// Trait abstracting replaceable code syntax highlighting strategies.
pub trait SyntaxHighlighter {
    /// Converts a code block line into styled spans.
    fn highlight(&self, language: &Option<String>, line: &str) -> Vec<VisualSpan>;
}

/// Default syntax highlighter highlighting common keyword tokens.
#[derive(Default)]
pub struct KeywordSyntaxHighlighter;

impl KeywordSyntaxHighlighter {
    /// Instantiates a new KeywordSyntaxHighlighter.
    pub fn new() -> Self {
        Self
    }
}

impl SyntaxHighlighter for KeywordSyntaxHighlighter {
    fn highlight(&self, _language: &Option<String>, line: &str) -> Vec<VisualSpan> {
        let trimmed = line.trim_start();
        if trimmed.starts_with("//") {
            return vec![VisualSpan::new(line.to_string(), VisualStyle::CodeComment)];
        }

        let mut spans = Vec::new();
        let words = line.split_inclusive(|c: char| !c.is_alphanumeric() && c != '_');

        for word in words {
            let trimmed_word = word.trim_end_matches(|c: char| !c.is_alphanumeric() && c != '_');
            let is_keyword = matches!(
                trimmed_word,
                "fn" | "let"
                    | "pub"
                    | "struct"
                    | "impl"
                    | "match"
                    | "return"
                    | "mut"
                    | "use"
                    | "mod"
                    | "crate"
                    | "enum"
                    | "self"
                    | "if"
                    | "else"
                    | "for"
                    | "in"
                    | "loop"
                    | "while"
                    | "as"
                    | "static"
                    | "const"
                    | "dyn"
                    | "trait"
                    | "async"
                    | "await"
                    | "type"
            );

            if is_keyword {
                let spacing = &word[trimmed_word.len()..];
                spans.push(VisualSpan::new(
                    trimmed_word.to_string(),
                    VisualStyle::CodeKeyword,
                ));
                if !spacing.is_empty() {
                    spans.push(VisualSpan::new(spacing.to_string(), VisualStyle::Normal));
                }
            } else {
                spans.push(VisualSpan::new(word.to_string(), VisualStyle::Normal));
            }
        }

        spans
    }
}

/// UI presentation-layer selection state tracking focused coordinates.
#[derive(Debug, Clone, Default)]
pub struct SelectionState {
    /// Selected range bounds: (start_index, end_index) inclusive.
    pub range: Option<(usize, usize)>,
}

impl SelectionState {
    /// Instantiates a new empty SelectionState.
    pub fn new() -> Self {
        Self { range: None }
    }

    /// Focuses selection bounds.
    pub fn select(&mut self, start: usize, end: usize) {
        self.range = Some((start, end));
    }

    /// Resets active selection.
    pub fn clear(&mut self) {
        self.range = None;
    }

    /// Verifies if index resides within the active selected bounds.
    pub fn is_selected(&self, idx: usize) -> bool {
        if let Some((start, end)) = self.range {
            let (min, max) = if start <= end {
                (start, end)
            } else {
                (end, start)
            };
            idx >= min && idx <= max
        } else {
            false
        }
    }
}

// NOTE:
// This cache is intentionally ephemeral.
// Destroying and rebuilding it must never change rendered output.
/// Ephemeral visual lines rendering cache owned exclusively by widgets.
#[derive(Debug, Clone, Default)]
pub struct MarkdownRenderState {
    visual_lines: Vec<VisualLine>,
}

impl MarkdownRenderState {
    /// Instantiates a new empty MarkdownRenderState.
    pub fn new() -> Self {
        Self {
            visual_lines: Vec::new(),
        }
    }

    /// Clears visual lines cache.
    pub fn clear(&mut self) {
        self.visual_lines.clear();
    }

    /// Exposes read-only parsed visual lines.
    pub fn visual_lines(&self) -> &[VisualLine] {
        &self.visual_lines
    }

    /// Modifies cached parsed visual lines.
    pub fn set_visual_lines(&mut self, lines: Vec<VisualLine>) {
        self.visual_lines = lines;
    }
}

/// Canonical document AST container.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MarkdownAst {
    /// Sequenced document block elements.
    pub blocks: Vec<MarkdownBlock>,
}

/// Decoupled markdown document parser.
pub struct MarkdownParser;

impl MarkdownParser {
    /// Parses raw markdown text into structural AST blocks.
    pub fn parse(text: &str) -> MarkdownAst {
        let mut blocks = Vec::new();
        let mut lines = text.lines().peekable();

        while let Some(&line) = lines.peek() {
            let trimmed = line.trim();

            if trimmed.is_empty() {
                lines.next();
                continue;
            }

            // 1. Heading Check
            if trimmed.starts_with('#') {
                let text_line = lines.next().unwrap();
                let heading_trimmed = text_line.trim_start_matches('#');
                let level = (text_line.len() - heading_trimmed.len()) as u8;
                blocks.push(MarkdownBlock::Heading {
                    level,
                    text: heading_trimmed.trim().to_string(),
                });
                continue;
            }

            // 2. Code Block Check
            if trimmed.starts_with("```") {
                let start_fence = lines.next().unwrap().trim();
                let language = if start_fence.len() > 3 {
                    Some(start_fence[3..].trim().to_string())
                } else {
                    None
                };

                let mut code_lines = Vec::new();
                while let Some(&inner) = lines.peek() {
                    let inner_trimmed = inner.trim();
                    if inner_trimmed.starts_with("```") {
                        lines.next();
                        break;
                    } else {
                        code_lines.push(lines.next().unwrap().to_string());
                    }
                }

                blocks.push(MarkdownBlock::CodeBlock {
                    language,
                    lines: code_lines,
                });
                continue;
            }

            // 3. Table Check
            if trimmed.starts_with('|') && trimmed.ends_with('|') {
                let header_line = lines.next().unwrap();
                let headers = parse_table_row(header_line);

                // Consume separator row if present
                if let Some(&sep_line) = lines.peek() {
                    let sep_trimmed = sep_line.trim();
                    if sep_trimmed.starts_with('|')
                        && sep_trimmed
                            .chars()
                            .all(|c| c == '|' || c == '-' || c == ':' || c.is_whitespace())
                    {
                        lines.next();
                    }
                }

                let mut rows = Vec::new();
                while let Some(&row_line) = lines.peek() {
                    let row_trimmed = row_line.trim();
                    if row_trimmed.starts_with('|') && row_trimmed.ends_with('|') {
                        rows.push(parse_table_row(lines.next().unwrap()));
                    } else {
                        break;
                    }
                }

                blocks.push(MarkdownBlock::Table(TableNode { headers, rows }));
                continue;
            }

            // 4. Citation Definition Check
            if trimmed.starts_with('[') && trimmed.contains("]:") {
                let citation_line = lines.next().unwrap().trim();
                if let Some(col_idx) = citation_line.find("]:") {
                    let label = citation_line[1..col_idx].trim().to_string();
                    let index_str = label.trim_start_matches('^');
                    let index = index_str.parse::<usize>().unwrap_or(0);
                    blocks.push(MarkdownBlock::Citation(CitationNode { label, index }));
                    continue;
                }
            }

            // 5. Paragraph text
            let p_line = lines.next().unwrap();
            let inlines = parse_inline(p_line);
            blocks.push(MarkdownBlock::Paragraph(inlines));
        }

        MarkdownAst { blocks }
    }
}

fn parse_table_row(row: &str) -> Vec<String> {
    let trimmed = row.trim();
    if trimmed.is_empty() {
        return Vec::new();
    }
    let content = &trimmed[1..trimmed.len() - 1];
    content.split('|').map(|s| s.trim().to_string()).collect()
}

fn parse_inline(text: &str) -> Vec<InlineNode> {
    let mut nodes = Vec::new();
    let chars = text.chars().collect::<Vec<_>>();
    let mut idx = 0;
    let mut current_text = String::new();

    macro_rules! flush_text {
        () => {
            if !current_text.is_empty() {
                nodes.push(InlineNode::Text(current_text.clone()));
                current_text.clear();
            }
        };
    }

    while idx < chars.len() {
        // Bold: **
        if idx + 1 < chars.len() && chars[idx] == '*' && chars[idx + 1] == '*' {
            flush_text!();
            idx += 2;
            let mut bold_content = String::new();
            let mut closed = false;
            while idx < chars.len() {
                if idx + 1 < chars.len() && chars[idx] == '*' && chars[idx + 1] == '*' {
                    idx += 2;
                    closed = true;
                    break;
                } else {
                    bold_content.push(chars[idx]);
                    idx += 1;
                }
            }
            if closed {
                nodes.push(InlineNode::Bold(bold_content));
            } else {
                current_text.push_str("**");
                current_text.push_str(&bold_content);
            }
            continue;
        }

        // Inline Code: `
        if chars[idx] == '`' {
            flush_text!();
            idx += 1;
            let mut code_content = String::new();
            let mut closed = false;
            while idx < chars.len() {
                if chars[idx] == '`' {
                    idx += 1;
                    closed = true;
                    break;
                } else {
                    code_content.push(chars[idx]);
                    idx += 1;
                }
            }
            if closed {
                nodes.push(InlineNode::InlineCode(code_content));
            } else {
                current_text.push('`');
                current_text.push_str(&code_content);
            }
            continue;
        }

        // Italic: * or _
        if chars[idx] == '*' || chars[idx] == '_' {
            let delim = chars[idx];
            flush_text!();
            idx += 1;
            let mut italic_content = String::new();
            let mut closed = false;
            while idx < chars.len() {
                if chars[idx] == delim {
                    idx += 1;
                    closed = true;
                    break;
                } else {
                    italic_content.push(chars[idx]);
                    idx += 1;
                }
            }
            if closed {
                nodes.push(InlineNode::Italic(italic_content));
            } else {
                current_text.push(delim);
                current_text.push_str(&italic_content);
            }
            continue;
        }

        // Citation: [1] or [^ref]
        if chars[idx] == '[' {
            flush_text!();
            idx += 1;
            let mut cit_content = String::new();
            let mut closed = false;
            while idx < chars.len() {
                if chars[idx] == ']' {
                    idx += 1;
                    closed = true;
                    break;
                } else {
                    cit_content.push(chars[idx]);
                    idx += 1;
                }
            }
            if closed {
                let mut is_link = false;
                if idx < chars.len() && chars[idx] == '(' {
                    let start_paren = idx;
                    idx += 1;
                    let mut target = String::new();
                    let mut paren_closed = false;
                    while idx < chars.len() {
                        if chars[idx] == ')' {
                            idx += 1;
                            paren_closed = true;
                            break;
                        } else {
                            target.push(chars[idx]);
                            idx += 1;
                        }
                    }
                    if paren_closed && target.starts_with("node:") {
                        let uuid_str = target.trim_start_matches("node:").trim();
                        if let Ok(parsed_uuid) = uuid::Uuid::parse_str(uuid_str) {
                            nodes.push(InlineNode::EntityReference {
                                label: cit_content.clone(),
                                node_id: brain_domain::NodeId(parsed_uuid),
                            });
                            is_link = true;
                        }
                    }
                    if !is_link {
                        idx = start_paren;
                    }
                }

                if !is_link {
                    if cit_content.chars().all(|c| c.is_numeric()) || cit_content.starts_with('^') {
                        nodes.push(InlineNode::Citation(cit_content));
                    } else {
                        current_text.push('[');
                        current_text.push_str(&cit_content);
                        current_text.push(']');
                    }
                }
            } else {
                current_text.push('[');
                current_text.push_str(&cit_content);
            }
            continue;
        }

        current_text.push(chars[idx]);
        idx += 1;
    }

    flush_text!();
    nodes
}

/// Markdown layout wrapper handling column constraints.
pub struct MarkdownLayout;

impl MarkdownLayout {
    /// Maps AST blocks into visual formatted lines based on column width boundary.
    pub fn layout(
        ast: &MarkdownAst,
        max_width: usize,
        highlighter: &dyn SyntaxHighlighter,
    ) -> Vec<VisualLine> {
        let mut visual_lines = Vec::new();
        let max_width = if max_width < 5 { 5 } else { max_width };

        for block in &ast.blocks {
            match block {
                MarkdownBlock::Heading { level, text } => {
                    let style = match level {
                        1 => VisualStyle::Heading1,
                        2 => VisualStyle::Heading2,
                        _ => VisualStyle::Heading3,
                    };
                    let wrapped = wrap_text(text, max_width);
                    for line in wrapped {
                        visual_lines.push(VisualLine {
                            kind: VisualLineKind::Heading(*level),
                            spans: vec![VisualSpan::new(line, style)],
                        });
                    }
                }
                MarkdownBlock::Paragraph(inlines) => {
                    let mut spans = Vec::new();
                    for node in inlines {
                        let (text, style) = match node {
                            InlineNode::Text(t) => (t.clone(), VisualStyle::Normal),
                            InlineNode::Bold(t) => (t.clone(), VisualStyle::Bold),
                            InlineNode::Italic(t) => (t.clone(), VisualStyle::Italic),
                            InlineNode::InlineCode(t) => (t.clone(), VisualStyle::InlineCode),
                            InlineNode::Citation(t) => (format!("[{}]", t), VisualStyle::Citation),
                            InlineNode::EntityReference { label, node_id } => {
                                (label.clone(), VisualStyle::EntityReference(*node_id))
                            }
                        };
                        spans.push(VisualSpan::new(text, style));
                    }

                    let paragraph_lines = wrap_spans(spans, max_width);
                    for line_spans in paragraph_lines {
                        visual_lines.push(VisualLine {
                            kind: VisualLineKind::Text,
                            spans: line_spans,
                        });
                    }
                }
                MarkdownBlock::CodeBlock { language, lines } => {
                    for line in lines {
                        let highlighted = highlighter.highlight(language, line);
                        let code_lines = wrap_spans(highlighted, max_width);
                        for line_spans in code_lines {
                            visual_lines.push(VisualLine {
                                kind: VisualLineKind::Code,
                                spans: line_spans,
                            });
                        }
                    }
                }
                MarkdownBlock::Table(table) => {
                    let table_lines = layout_table(table, max_width);
                    for spans in table_lines {
                        visual_lines.push(VisualLine {
                            kind: VisualLineKind::Table,
                            spans,
                        });
                    }
                }
                MarkdownBlock::Citation(node) => {
                    let formatted = format!("[{}] reference index", node.label);
                    let wrapped = wrap_text(&formatted, max_width);
                    for line in wrapped {
                        visual_lines.push(VisualLine {
                            kind: VisualLineKind::Text,
                            spans: vec![VisualSpan::new(line, VisualStyle::Citation)],
                        });
                    }
                }
            }
        }

        visual_lines
    }
}

fn wrap_spans(spans: Vec<VisualSpan>, max_width: usize) -> Vec<Vec<VisualSpan>> {
    let mut lines = Vec::new();
    let mut current_line = Vec::new();
    let mut current_width = 0;

    for span in spans {
        let text = &span.text;
        let style = span.style;
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
                        lines.push(vec![VisualSpan::new(chunk_str, style)]);
                    }
                    continue;
                }
            }
            current_line.push(VisualSpan::new(word.to_string(), style));
            current_width += word_len;
        }
    }
    if !current_line.is_empty() {
        lines.push(current_line);
    }
    lines
}

fn wrap_text(text: &str, max_width: usize) -> Vec<String> {
    let spans = vec![VisualSpan::new(text.to_string(), VisualStyle::Normal)];
    let wrapped = wrap_spans(spans, max_width);
    wrapped
        .into_iter()
        .map(|line_spans| {
            line_spans
                .into_iter()
                .map(|s| s.text.to_string())
                .collect::<Vec<_>>()
                .join("")
        })
        .collect()
}

fn layout_table(table: &TableNode, max_width: usize) -> Vec<Vec<VisualSpan>> {
    let col_count = std::cmp::max(
        table.headers.len(),
        table.rows.iter().map(|r| r.len()).max().unwrap_or(0),
    );
    if col_count == 0 {
        return Vec::new();
    }

    let mut col_widths = vec![0; col_count];
    for (i, h) in table.headers.iter().enumerate() {
        col_widths[i] = std::cmp::max(col_widths[i], h.chars().count());
    }
    for row in &table.rows {
        for (i, cell) in row.iter().enumerate() {
            if i < col_count {
                col_widths[i] = std::cmp::max(col_widths[i], cell.chars().count());
            }
        }
    }

    let total_padding = 3 * col_count + 1;
    let sum_widths: usize = col_widths.iter().sum();

    if sum_widths + total_padding > max_width {
        let available_width = max_width.saturating_sub(total_padding);
        let available_width = if available_width < col_count {
            col_count
        } else {
            available_width
        };
        for w in &mut col_widths {
            if let Some(val) = (*w * available_width).checked_div(sum_widths) {
                *w = val;
                if *w == 0 {
                    *w = 1;
                }
            }
        }
    }

    let mut lines = Vec::new();

    let mut header_spans = Vec::new();
    header_spans.push(VisualSpan::new("| ", VisualStyle::TableHeader));
    for (i, h) in table.headers.iter().enumerate() {
        let width = col_widths[i];
        let cell_text = truncate_or_pad(h, width);
        header_spans.push(VisualSpan::new(cell_text, VisualStyle::TableHeader));
        header_spans.push(VisualSpan::new(" | ", VisualStyle::TableHeader));
    }
    lines.push(header_spans);

    for row in &table.rows {
        let mut row_spans = Vec::new();
        row_spans.push(VisualSpan::new("| ", VisualStyle::TableCell));
        for (i, &width) in col_widths.iter().enumerate().take(col_count) {
            let cell_val = row.get(i).map(|s| s.as_str()).unwrap_or("");
            let cell_text = truncate_or_pad(cell_val, width);
            row_spans.push(VisualSpan::new(cell_text, VisualStyle::TableCell));
            row_spans.push(VisualSpan::new(" | ", VisualStyle::TableCell));
        }
        lines.push(row_spans);
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

/// Monotonic revision sequence wrapper tracking paragraph content changes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct MessageRevision(pub u64);

/// Cached compilation result of laid out lines for a single message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CachedMessageLayout {
    /// Revision of the message when this layout was generated.
    pub revision: MessageRevision,
    /// Cached layout visual lines.
    pub visual_lines: Vec<VisualLine>,
    /// Precalculated visual line height of the message layout.
    pub height: usize,
}

/// Cumulative line heights index supporting O(log n) viewport visibility offset lookups.
#[derive(Debug, Clone, Default)]
pub struct ViewportIndex {
    /// Cumulative sum of line heights per message index (cumulative_heights[i] is sum of heights of messages 0..=i).
    pub cumulative_heights: Vec<u32>,
}

impl ViewportIndex {
    /// Rebuilds index boundaries from layout height slices.
    pub fn rebuild(heights: &[usize]) -> Self {
        let mut cumulative_heights = Vec::with_capacity(heights.len());
        let mut total = 0u32;
        for &h in heights {
            total = total.saturating_add(h as u32);
            cumulative_heights.push(total);
        }
        Self { cumulative_heights }
    }

    /// Binary searches the cumulative index to locate the message index and the local line offset.
    pub fn find_offset(&self, global_offset: u32) -> Option<(usize, u32)> {
        if self.cumulative_heights.is_empty() {
            return None;
        }

        let idx = match self.cumulative_heights.binary_search_by(|&val| {
            if val <= global_offset {
                std::cmp::Ordering::Less
            } else {
                std::cmp::Ordering::Greater
            }
        }) {
            Ok(i) => i,
            Err(i) => i,
        };

        if idx < self.cumulative_heights.len() {
            let previous_sum = if idx > 0 {
                self.cumulative_heights[idx - 1]
            } else {
                0
            };
            let local_offset = global_offset - previous_sum;
            Some((idx, local_offset))
        } else {
            None
        }
    }
}
