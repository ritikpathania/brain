//! AST nodes for width-agnostic markdown representation.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

/// Stable block identifier within a message content revision.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BlockId(pub u64);

impl BlockId {
    /// Generates a stable BlockId based on content and position/index
    pub fn generate(content: &str, index: usize) -> Self {
        let mut hasher = DefaultHasher::new();
        content.hash(&mut hasher);
        index.hash(&mut hasher);
        Self(hasher.finish())
    }
}

/// Typed opaque wrapper for a hyperlink target URL.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct LinkTarget(Box<str>);

impl LinkTarget {
    /// Instantiates a new LinkTarget from a URL representation.
    pub fn new(url: impl Into<Box<str>>) -> Self {
        Self(url.into())
    }
    /// Exposes read-only target URL slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Typed opaque wrapper for a citation/footnote label.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CitationId(pub Box<str>);

/// Recursive inline markdown node representing formatted segments.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InlineNode {
    /// Unstyled plain text.
    Text(String),
    /// Bold formatted children.
    Strong(Vec<InlineNode>),
    /// Italic formatted children.
    Emphasis(Vec<InlineNode>),
    /// Inline code segment.
    Code(String),
    /// Interactive hyperlink block.
    Link {
        /// Children inside link tag.
        children: Vec<InlineNode>,
        /// Opaque URL target.
        url: LinkTarget,
    },
    /// Citation reference badge.
    Citation(CitationId),
}

/// A cell in a layout table grid.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableCell {
    /// Text or inline nodes within cell.
    pub content: Vec<InlineNode>,
}

/// Represents a parsed markdown table node.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableNode {
    /// Header row cells.
    pub headers: Vec<TableCell>,
    /// Table body cell grid.
    pub rows: Vec<Vec<TableCell>>,
}

/// Classification of lists.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ListKind {
    /// Bullet items list.
    Unordered,
    /// Numbered items list.
    Ordered,
}

/// Normalized identifiers for syntax highlighting lexers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LanguageId {
    /// Plain text fallback.
    PlainText,
    /// Rust source code.
    Rust,
    /// Python source code.
    Python,
    /// JSON structured text.
    Json,
    /// Shell scripting or bash code.
    Shell,
    /// Unsupported or unrecognized syntax language.
    Unknown,
}

/// Logical AST blocks forming the document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DocumentBlock {
    /// Text flow paragraph.
    Paragraph(Vec<InlineNode>),
    /// Section heading with level and styled content.
    Heading {
        /// Heading depth level (1 to 6).
        level: u8,
        /// Styled inline elements.
        content: Vec<InlineNode>,
    },
    /// Fenced pre-formatted code block.
    CodeBlock {
        /// Normalized programming language.
        language: LanguageId,
        /// Inner lines.
        lines: Vec<String>,
    },
    /// Aligned table grid block.
    Table(TableNode),
    /// Ordered or unordered list of items.
    List {
        /// Unordered/ordered categorization.
        kind: ListKind,
        /// List items, each represented by inline nodes.
        items: Vec<Vec<InlineNode>>,
    },
    /// Nested blockquote section.
    BlockQuote(Vec<DocumentBlock>),
    /// Horizontal rule divider.
    HorizontalRule,
}
