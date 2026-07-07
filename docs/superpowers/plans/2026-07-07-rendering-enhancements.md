# Rendering Enhancements & Semantic Pipeline Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Redesign the markdown parser and TUI viewport rendering into a decoupled, semantic rendering pipeline supporting rich table/list layouts, custom lexer syntax highlighting, interactive OSC 8 links, and hierarchical tree navigation for tool blocks.

**Architecture:** Split parsing (width-agnostic) from layout compiling (width-aware). Flatten layout boundaries into a layout tree, query it with a pure navigation solver, and resolve glyphs/escape sequences at the renderer boundary.

**Tech Stack:** Rust, Ratatui, Crossterm.

## Global Constraints
- Avoid introducing heavyweight external parsing/highlighting dependencies (e.g. syntect, pulldown-cmark).
- Maintain zero external subsystem dependencies on `brain-domain`.
- Handle Unicode and ASCII terminal capability levels gracefully at the renderer boundary.
- Do not let escape sequences affect visual text wrapping math.

---

### Task 1: Semantic Document AST & Width-Agnostic Parser

**Files:**
- Create: `crates/brain-tui/src/ui/interaction/ast.rs`
- Modify: `crates/brain-tui/src/ui/interaction/markdown.rs`
- Create: `crates/brain-tui/tests/markdown_ast_tests.rs`

**Interfaces:**
- Consumes: None
- Produces: `DocumentBlock`, `InlineNode`, `LanguageId`, `LinkTarget`, `TableCell`, `TableNode`, `ListKind`, and `MarkdownParser::parse_to_blocks(text: &str) -> Vec<DocumentBlock>`

- [ ] **Step 1: Write the failing parser test**

Create `crates/brain-tui/tests/markdown_ast_tests.rs`:
```rust
use brain_tui::ui::interaction::ast::{DocumentBlock, InlineNode, LanguageId, LinkTarget, ListKind};
use brain_tui::ui::interaction::markdown::MarkdownParser;

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
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --test markdown_ast_tests`
Expected: FAIL with compilation error (modules/types not defined)

- [ ] **Step 3: Scaffolding and Parser Implementation**

Create `crates/brain-tui/src/ui/interaction/ast.rs`:
```rust
use brain_domain::MessageId;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BlockId(pub u64);

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ListKind {
    Unordered,
    Ordered,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LanguageId {
    PlainText,
    Rust,
    Python,
    Json,
    Shell,
    Unknown,
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

Update `crates/brain-tui/src/ui/interaction/markdown.rs` to expose parser methods mapping Markdown strings recursively to `DocumentBlock`s. Integrate a hash-based generator to produce stable `BlockId`s for parsed blocks.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --test markdown_ast_tests`
Expected: PASS

- [ ] **Step 5: Commit**

Run: `git add crates/brain-tui/src/ui/interaction/ast.rs crates/brain-tui/src/ui/interaction/markdown.rs crates/brain-tui/tests/markdown_ast_tests.rs && git commit -m "feat: add width-agnostic markdown parser and AST structures"`

---

### Task 2: Language Lexer Framework & Highlighters

**Files:**
- Create: `crates/brain-tui/src/ui/interaction/lexer.rs`
- Create: `crates/brain-tui/tests/lexer_tests.rs`

**Interfaces:**
- Consumes: `LanguageId`
- Produces: `TokenKind`, `HighlightSpan`, `LanguageHighlighter` trait, and `SyntaxHighlighterRegistry::highlight(lang: LanguageId, line: &str) -> Box<dyn Iterator<Item = HighlightSpan<'static>>>`

- [ ] **Step 1: Write the failing lexer test**

Create `crates/brain-tui/tests/lexer_tests.rs`:
```rust
use brain_tui::ui::interaction::lexer::{TokenKind, SyntaxHighlighterRegistry};
use brain_tui::ui::interaction::ast::LanguageId;

#[test]
fn test_rust_keyword_tokenization() {
    let line = "pub fn main() {}";
    let spans: Vec<_> = SyntaxHighlighterRegistry::highlight(LanguageId::Rust, line).collect();
    assert_eq!(spans[0].kind, TokenKind::Keyword);
    assert_eq!(spans[0].text, "pub");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --test lexer_tests`
Expected: FAIL

- [ ] **Step 3: Lexer Framework Implementation**

Create `crates/brain-tui/src/ui/interaction/lexer.rs`:
```rust
use crate::ui::interaction::ast::LanguageId;

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HighlightSpan<'a> {
    pub kind: TokenKind,
    pub text: &'a str,
}

pub trait LanguageHighlighter: Send + Sync {
    fn language_id(&self) -> LanguageId;
    fn highlight<'a>(&self, line: &'a str) -> Box<dyn Iterator<Item = HighlightSpan<'a>> + 'a>;
}

pub struct SyntaxHighlighterRegistry;

impl SyntaxHighlighterRegistry {
    pub fn highlight<'a>(lang: LanguageId, line: &'a str) -> Box<dyn Iterator<Item = HighlightSpan<'a>> + 'a> {
        // Match lang against Rust, Python, Json, Shell highlighters or fallback to PlainText
        // Return boxed iterator yielding HighlightSpans
    }
}
```
Implement simple, non-allocating string-scanners for:
- `RustHighlighter` (matches standard keywords, double-slash comments, and strings).
- `JsonHighlighter` (scans key string labels and numbers).
- `PythonHighlighter` (scans python keyword tokens and comments).
- `ShellHighlighter` (scans bash scripts).

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --test lexer_tests`
Expected: PASS

- [ ] **Step 5: Commit**

Run: `git add crates/brain-tui/src/ui/interaction/lexer.rs crates/brain-tui/tests/lexer_tests.rs && git commit -m "feat: implement syntax highlighting lexer framework"`

---

### Task 3: Width-Aware Layout Tree Compiler

**Files:**
- Create: `crates/brain-tui/src/ui/interaction/layout_tree.rs`
- Modify: `crates/brain-tui/src/ui/interaction/markdown.rs`
- Create: `crates/brain-tui/tests/layout_tree_tests.rs`

**Interfaces:**
- Consumes: `DocumentBlock`, `TokenKind`
- Produces: `LayoutTree`, `LayoutBlock`, `VisualLine`, `VisualSpan`, `VisualStyle`, `SpanAction`, and `LayoutEngine::compile(blocks: &[DocumentBlock], width: usize) -> LayoutTree`

- [ ] **Step 1: Write the failing layout test**

Create `crates/brain-tui/tests/layout_tree_tests.rs`:
```rust
use brain_tui::ui::interaction::ast::{DocumentBlock, InlineNode};
use brain_tui::ui::interaction::layout_tree::{LayoutEngine, VisualStyle};

#[test]
fn test_heading_layout_wrapping() {
    let heading = DocumentBlock::Heading {
        level: 1,
        content: vec![InlineNode::Text("A very long title heading text that wraps".to_string())],
    };
    let tree = LayoutEngine::compile(&[heading], 15);
    let blocks = tree.blocks();
    assert!(blocks[0].lines.len() > 1);
    assert_eq!(blocks[0].lines[0].spans[0].style, VisualStyle::Heading1);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --test layout_tree_tests`
Expected: FAIL

- [ ] **Step 3: Layout Engine Implementation**

Create `crates/brain-tui/src/ui/interaction/layout_tree.rs` carrying definitions for `LayoutTree`, `LayoutBlock`, `VisualLine`, `VisualSpan`, `VisualStyle`, and `SpanAction`.
Implement `LayoutEngine::compile(blocks, width)` to:
- Resolve indentation for blockquotes and lists.
- Dynamically wrap nested inline formatting spans cleanly at word boundaries.
- Calculate table column widths (balancing available space) and format cells into structured columns.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --test layout_tree_tests`
Expected: PASS

- [ ] **Step 5: Commit**

Run: `git add crates/brain-tui/src/ui/interaction/layout_tree.rs crates/brain-tui/tests/layout_tree_tests.rs && git commit -m "feat: compile semantic layout tree with width awareness"`

---

### Task 4: Border & Link Presentation Resolvers

**Files:**
- Create: `crates/brain-tui/src/ui/render/resolver.rs`
- Modify: `crates/brain-tui/src/ui/renderer.rs`
- Create: `crates/brain-tui/tests/presentation_resolver_tests.rs`

**Interfaces:**
- Consumes: `EffectiveCapabilities`, `VisualSpan`
- Produces: `BorderResolver::resolve(caps: &EffectiveCapabilities) -> BorderGlyphs`, `LinkRenderer::render(span: &VisualSpan, caps: &EffectiveCapabilities) -> TerminalSpan`, and `draw_span` wrapper

- [ ] **Step 1: Write the failing border resolver test**

Create `crates/brain-tui/tests/presentation_resolver_tests.rs`:
```rust
use brain_tui::ui::render::resolver::{BorderResolver, BorderGlyphs};
use brain_tui::ui::render::context::{EffectiveCapabilities, UnicodeSupport};

#[test]
fn test_border_glyph_resolution() {
    let mut caps = EffectiveCapabilities::default();
    caps.unicode = UnicodeSupport::Full;
    let glyphs = BorderResolver::resolve(&caps);
    assert_eq!(glyphs.top_left, "┌");

    caps.unicode = UnicodeSupport::AsciiOnly;
    let glyphs = BorderResolver::resolve(&caps);
    assert_eq!(glyphs.top_left, "+");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --test presentation_resolver_tests`
Expected: FAIL

- [ ] **Step 3: Border & Link Resolvers Implementation**

Create `crates/brain-tui/src/ui/render/resolver.rs`:
- Define `BorderGlyphs`, `BorderResolver`.
- Define `TerminalSpan`, `LinkRenderer::render`.
- Implement OSC 8 escape code generator for `TerminalSpan::Hyperlink` when OSC 8 is enabled; otherwise format link as plain text: `text (url)`.
Integrate `LinkRenderer` and `BorderResolver` inside `crates/brain-tui/src/ui/renderer.rs` to output final cells.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --test presentation_resolver_tests`
Expected: PASS

- [ ] **Step 5: Commit**

Run: `git add crates/brain-tui/src/ui/render/resolver.rs crates/brain-tui/tests/presentation_resolver_tests.rs && git commit -m "feat: add capability-driven border and link resolvers"`

---

### Task 5: Navigation Solver & Hierarchical Selection

**Files:**
- Modify: `crates/brain-tui/src/ui/state.rs`
- Create: `crates/brain-tui/src/ui/interaction/navigation.rs`
- Create: `crates/brain-tui/tests/navigation_solver_tests.rs`

**Interfaces:**
- Consumes: `LayoutTree`, `ConversationViewState`
- Produces: `ConversationViewState`, `ConversationNodeId`, `NavigationIndex`, `NavigationSolver::solve(layout: &LayoutTree, view_state: &ConversationViewState) -> NavigationIndex`

- [ ] **Step 1: Write the failing navigation test**

Create `crates/brain-tui/tests/navigation_solver_tests.rs`:
```rust
use brain_tui::ui::interaction::navigation::{NavigationSolver, ConversationViewState, ConversationNodeId};
use brain_tui::ui::interaction::layout_tree::{LayoutTree, LayoutBlock, VisualBlockKind};
use brain_tui::ui::interaction::ast::BlockId;
use brain_domain::MessageId;

#[test]
fn test_navigation_solver_traversal() {
    let blocks = vec![
        LayoutBlock { id: BlockId(1), kind: VisualBlockKind::Paragraph, lines: vec![] }
    ];
    let tree = LayoutTree::new(blocks);
    let view_state = ConversationViewState::default();
    let index = NavigationSolver::solve(&tree, &view_state);
    assert_eq!(index.nodes.len(), 1);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --test navigation_solver_tests`
Expected: FAIL

- [ ] **Step 3: ViewState & Solver Implementation**

Create `crates/brain-tui/src/ui/interaction/navigation.rs`:
- Define `ConversationViewState` (holds scroll offset, selection, and a set of expanded tool call headers).
- Define `ConversationNodeId` enum hierarchy.
- Implement pure `NavigationSolver::solve` that traverses the layout tree and view state to assemble the flat `NavigationIndex`.
- Integrate cursoring keys (Up/Down targets indices, Right/Left expands/collapses, Enter toggles).

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --test navigation_solver_tests`
Expected: PASS

- [ ] **Step 5: Commit**

Run: `git add crates/brain-tui/src/ui/interaction/navigation.rs crates/brain-tui/tests/navigation_solver_tests.rs && git commit -m "feat: implement hierarchical document cursor navigation solver"`

---

### Task 6: Lazy Tool Timelines & Integration

**Files:**
- Modify: `crates/brain-tui/src/ui/renderer.rs`
- Modify: `crates/brain-tui/src/ui/widgets/chat_screen.rs`
- Create: `crates/brain-tui/tests/lazy_timeline_tests.rs`

**Interfaces:**
- Consumes: `ConversationViewState`, `LayoutTree`
- Produces: Collapsible cards with lazy logs rendering

- [ ] **Step 1: Write the failing lazy timeline test**

Create `crates/brain-tui/tests/lazy_timeline_tests.rs`:
- Setup a tool block in state.
- Assert that when `Logs` section is collapsed, log visual lines are NOT compiled into the layout tree.
- Assert that when `Logs` is expanded, log lines are compiled.

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --test lazy_timeline_tests`
Expected: FAIL

- [ ] **Step 3: Lazy Tool Execution Blocks Implementation**

Update `crates/brain-tui/src/ui/renderer.rs` and `chat_screen.rs` to:
- Render each completed tool execution inside a framed card.
- Read `ConversationViewState` expanded tool call keys.
- Lazily generate visual layout lines for logs/JSON outputs only when expanded.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --test lazy_timeline_tests`
Expected: PASS

- [ ] **Step 5: Commit**

Run: `git add crates/brain-tui/src/ui/renderer.rs crates/brain-tui/tests/lazy_timeline_tests.rs && git commit -m "feat: add lazy rendering of expandable tool execution logs"`

---

### Task 7: Layout Cache & Determinism Invariant

**Files:**
- Modify: `crates/brain-tui/src/ui/renderer.rs`
- Create: `crates/brain-tui/tests/layout_cache_tests.rs`

**Interfaces:**
- Consumes: `LayoutCacheKey`, `LayoutCache`
- Produces: Cached LayoutTrees and layout determinism properties

- [ ] **Step 1: Write the layout determinism test**

Create `crates/brain-tui/tests/layout_cache_tests.rs` containing a property test checking that two compilation runs on identical text, width, and capabilities yield identical byte-for-byte `LayoutTree` geometry output.

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --test layout_cache_tests`
Expected: FAIL

- [ ] **Step 3: Layout Caching Implementation**

Modify `crates/brain-tui/src/ui/renderer.rs` to cache `LayoutTree` geometry blocks using `LayoutCacheKey` (MessageId, content_revision, and width), bypassing layout rebuilding when only viewport focus, selection, or theme styles change.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --test layout_cache_tests`
Expected: PASS

- [ ] **Step 5: Commit**

Run: `git add crates/brain-tui/src/ui/renderer.rs crates/brain-tui/tests/layout_cache_tests.rs && git commit -m "feat: integrate layout tree cache and enforce determinism invariants"`
