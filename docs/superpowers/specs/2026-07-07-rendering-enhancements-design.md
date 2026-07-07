# Design Specification: Rendering Enhancements & Semantic Pipeline

This document defines the architectural redesign of the markdown and tool execution rendering subsystems in `crates/brain-tui/` into a decoupled, semantic rendering pipeline.

---

## 1. Pipeline Stages

The rendering engine is organized as a unidirectional, multi-stage pipeline. Each stage has a single, isolated responsibility:

```text
Markdown Text / State
        │
        ▼ (Stage 1)
Markdown Parser ────────► Stable BlockIds / Normalization
        │
        ▼
Semantic Document (DocumentBlock / InlineNode)
        │
        ▼ (Stage 2)
Layout Engine ──────────► Token & Syntax Lexers (LanguageId)
        │
        ▼
Layout Tree (Geometry) ◄─ Dynamic Expansion State
        │
        ├──────────────────────────┐
        ▼ (Stage 3)                ▼ (Stage 4)
Navigation Solver           Renderer & Link/Border Resolvers
        │                          │
        ▼                          ▼
Flat Navigation Index       Terminal Spans & Buffer Writes
```

---

## 2. Component Design & Structures

### Stage 1: Width-Agnostic Parser & Semantic Document

The parser parses raw text into semantic structures. It has no awareness of terminal dimensions, wrapping constraints, or style tokens.

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BlockId(pub u64);

// Block IDs are stable within a single content_revision of a message, not globally.

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct LinkTarget(Box<str>);

impl LinkTarget {
    pub fn new(url: impl Into<Box<str>>) -> Self {
        Self(url.into())
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CitationId(pub Box<str>);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InlineNode {
    Text(String),
    Strong(Vec<InlineNode>),
    Emphasis(Vec<InlineNode>),
    Code(String),
    Link {
        children: Vec<InlineNode>,
        url: LinkTarget,
    },
    Citation(CitationId),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableCell {
    pub content: Vec<InlineNode>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableNode {
    pub headers: Vec<TableCell>,
    pub rows: Vec<Vec<TableCell>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ListKind {
    Unordered,
    Ordered,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DocumentBlock {
    Paragraph(Vec<InlineNode>),
    Heading {
        level: u8,
        content: Vec<InlineNode>,
    },
    CodeBlock {
        language: LanguageId,
        lines: Vec<String>,
    },
    Table(TableNode),
    List {
        kind: ListKind,
        items: Vec<Vec<InlineNode>>,
    },
    BlockQuote(Vec<DocumentBlock>),
    HorizontalRule,
}
```

#### Syntax Lexer Integration
`LanguageId` identifies fenced markdown language names. The registry maps `LanguageId` to highlighters (e.g. normalizing `rs` to `Rust`).
```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LanguageId {
    PlainText,
    Rust,
    Python,
    Json,
    Shell,
    Unknown(Box<str>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenKind {
    Keyword,
    String,
    Number,
    Comment,
    Operator,
    Type,
    Function,
    Identifier,
    Plain,
}

pub struct HighlightSpan<'a> {
    pub kind: TokenKind,
    pub text: &'a str,
}

pub trait LanguageHighlighter: Send + Sync {
    fn language_id(&self) -> LanguageId;
    fn highlight<'a>(&self, line: &'a str) -> Box<dyn Iterator<Item = HighlightSpan<'a>> + 'a>;
}
```

---

### Stage 2: Width-Aware Layout Engine

The layout engine consumes structural `DocumentBlock`s and resolves wrapping, column measurements, list indentation, and visual styles.

```rust
pub struct LayoutTree {
    blocks: Vec<LayoutBlock>,
}

impl LayoutTree {
    pub fn new(blocks: Vec<LayoutBlock>) -> Self {
        Self { blocks }
    }
    
    pub fn blocks(&self) -> &[LayoutBlock] {
        &self.blocks
    }
}

pub struct LayoutBlock {
    pub id: BlockId,
    pub kind: VisualBlockKind,
    pub lines: Vec<VisualLine>,
}

pub enum VisualBlockKind {
    Paragraph,
    Heading(u8),
    CodeBlock,
    Table {
        col_widths: Vec<usize>,
    },
    List,
    BlockQuote,
    HorizontalRule,
}

pub struct VisualLine {
    pub spans: Vec<VisualSpan>,
}

pub struct VisualSpan {
    pub text: Cow<'static, str>,
    pub style: VisualStyle,
    pub token_kind: Option<TokenKind>,
    pub action: SpanAction,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VisualStyle {
    Normal,
    Heading1, Heading2, Heading3,
    Bold, Italic, InlineCode,
    BlockQuote, ListBullet,
    TableHeader, TableCell, TableBorder,
    HorizontalRule,
}
```

---

### Stage 3: View State & Navigation Index

#### Ephemeral View State
```rust
pub struct ConversationViewState {
    pub selected_node: Option<ConversationNodeId>,
    pub scroll_offset: usize,
    pub viewport_height: usize,
    pub expanded_tool_sections: HashMap<ToolCallId, HashSet<ToolSection>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ToolSection {
    Input,
    Output,
    Logs,
    Metadata,
}
```

#### Selection & Layout Navigation Nodes
`MessageId` is imported directly from the domain module (`brain_domain::MessageId`) to preserve unified message identifiers.
```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BlockId(pub u64);

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum InteractiveNodeId {
    ToolSection(ToolSection),
    Citation(CitationId),
    Hyperlink(LinkTarget),
    CodeBlock,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ConversationNodeId {
    Message(MessageId),
    Block {
        message: MessageId,
        block: BlockId,
    },
    Interactive {
        message: MessageId,
        block: BlockId,
        node: InteractiveNodeId,
    },
}
```

#### Pure Navigation Solver
The `NavigationSolver` is side-effect free. Given a `LayoutTree` and `ConversationViewState`, it resolves a flat list of focusable coordinates:
```rust
pub struct NavigationIndex {
    pub nodes: Vec<ConversationNodeId>,
}

pub struct NavigationSolver;

impl NavigationSolver {
    pub fn solve(layout: &LayoutTree, view_state: &ConversationViewState) -> NavigationIndex {
        // Collects message IDs, active tool sections (if expanded), and hyperlinks/citations in layout order
        let mut nodes = Vec::new();
        // ... traversal ...
        NavigationIndex { nodes }
    }
}
```

---

### Stage 4: Capability-Driven Renderer

The presentation layer consumes the layout tree and maps tokens and spans into crossterm/ratatui buffer cells.

```rust
pub struct BorderGlyphs {
    pub top_left: &'static str, pub top_mid: &'static str, pub top_right: &'static str,
    pub mid_left: &'static str, pub mid_mid: &'static str, pub mid_right: &'static str,
    pub bottom_left: &'static str, pub bottom_mid: &'static str, pub bottom_right: &'static str,
    pub vertical: &'static str, pub horizontal: &'static str,
}

pub struct BorderResolver;

impl BorderResolver {
    pub fn resolve(caps: &EffectiveCapabilities) -> BorderGlyphs {
        if caps.unicode == UnicodeSupport::Full {
            BorderGlyphs {
                top_left: "┌", top_mid: "┬", top_right: "┐",
                mid_left: "├", mid_mid: "┼", mid_right: "┤",
                bottom_left: "└", bottom_mid: "┴", bottom_right: "┘",
                vertical: "│", horizontal: "─",
            }
        } else {
            BorderGlyphs {
                top_left: "+", top_mid: "+", top_right: "+",
                mid_left: "+", mid_mid: "+", mid_right: "+",
                bottom_left: "+", bottom_mid: "+", bottom_right: "+",
                vertical: "|", horizontal: "-",
            }
        }
    }
}
```

#### LinkRenderer (Terminal Span Mapping)
```rust
pub enum TerminalSpan<'a> {
    Plain(&'a str),
    Hyperlink {
        text: &'a str,
        url: &'a str,
    },
}

pub struct LinkRenderer;

impl LinkRenderer {
    pub fn render<'a>(
        span: &'a VisualSpan,
        caps: &EffectiveCapabilities,
    ) -> TerminalSpan<'a> {
        match (&span.action, caps.osc8) {
            (SpanAction::Hyperlink(url), true) => {
                TerminalSpan::Hyperlink {
                    text: &span.text,
                    url: url.as_str(),
                }
            }
            (SpanAction::Hyperlink(url), false) => {
                // Return plain text fallback representation
                // Implementation will format as: text (url)
                TerminalSpan::Plain(...) 
            }
            _ => TerminalSpan::Plain(&span.text),
        }
    }
}
```

---

## 3. Caching Design

We split view-based indices and geometry measurements into separate cache blocks:

```rust
pub struct LayoutCacheKey {
    pub message_id: MessageId,
    pub content_revision: MessageRevision,
    pub width: usize,
}

pub struct LayoutCache {
    /// Stores the geometry blocks. Refreshed on content change or window resize.
    pub trees: HashMap<LayoutCacheKey, LayoutTree>,
}

pub struct NavigationCacheKey {
    pub message_id: MessageId,
    pub content_revision: MessageRevision,
    pub width: usize,
    /// Hash/State representation of expanded tool blocks inside this message
    pub expansion_hash: u64,
}

pub struct NavigationCache {
    /// Stores flat interactive indexes.
    pub indexes: HashMap<NavigationCacheKey, NavigationIndex>,
}
```

---

## 4. Verification & Testing Strategy

Each stage of the rendering pipeline is verified using isolated unit testing, followed by overall golden snapshot tests:

1. **Parser Tests**: Validate raw markdown inputs convert to precise `DocumentBlock` structures (verifying recursion of nested bold/italic/links and table content).
2. **Lexer Tests**: Check programming language lexical scanners correctly extract tokens with zero panics on malformed code.
3. **Layout Tests**: Verify text wrapping, alignment, and column width distribution of tables.
4. **Navigation Tests**: Assert tree traversal invariants (e.g. Arrow keys, Right/Left collapses, selection node validation).
5. **Renderer Tests**: Check capability border glyphes and link-to-buffer outputs.
6. **Snapshot Tests**: Validate visual appearance under standard and ASCII fallback profiles.
7. **Property-Based Invariant Tests**:
   - **Layout Determinism**: Assert that given identical content, width, and capabilities, the layout compiler produces a byte-for-byte identical `LayoutTree`.
