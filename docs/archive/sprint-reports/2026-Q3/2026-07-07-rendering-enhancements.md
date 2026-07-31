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
- Keep the `interaction/` directory modular:
  ```text
  interaction/
      ast.rs
      parser.rs
      lexer.rs
      layout_tree.rs
      navigation.rs
      markdown.rs   // façade re-exporting the API
  ```

---

### Task 1: Semantic Document AST & Width-Agnostic Parser

**Files:**
- Create: `crates/brain-tui/src/ui/interaction/ast.rs`
- Create: `crates/brain-tui/src/ui/interaction/parser.rs`
- Modify: `crates/brain-tui/src/ui/interaction/markdown.rs` (turns into façade re-exporting the submodules)
- Create: `crates/brain-tui/tests/markdown_ast_tests.rs`

**Interfaces:**
- Consumes: None
- Produces: `DocumentBlock`, `InlineNode`, `LanguageId`, `LinkTarget`, `TableCell`, `TableNode`, `ListKind`, and `MarkdownParser::parse_to_blocks(text: &str) -> Vec<DocumentBlock>`
- **Resilience Invariant**: Unknown/unsupported markdown elements must degrade gracefully to plain text paragraphs rather than failing or panicking.

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

Create `crates/brain-tui/src/ui/interaction/ast.rs` defining the normalized enum blocks and recursive inline nodes, using typed `BlockId` and `LinkTarget` identifiers.
Create `crates/brain-tui/src/ui/interaction/parser.rs` implementing width-agnostic parsing, converting inputs into `DocumentBlock` collections.
Rewrite `crates/brain-tui/src/ui/interaction/markdown.rs` to serve as the façade module.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --test markdown_ast_tests`
Expected: PASS

- [ ] **Step 5: Commit**

Run: `git add crates/brain-tui/src/ui/interaction/ast.rs crates/brain-tui/src/ui/interaction/parser.rs crates/brain-tui/src/ui/interaction/markdown.rs crates/brain-tui/tests/markdown_ast_tests.rs && git commit -m "feat: add width-agnostic markdown parser and AST structures"`

---

### Task 2: Language Lexer Framework & Highlighters

**Files:**
- Create: `crates/brain-tui/src/ui/interaction/lexer.rs`
- Create: `crates/brain-tui/tests/lexer_tests.rs`

**Interfaces:**
- Consumes: `LanguageId`
- Produces: `TokenKind`, `HighlightSpan`, `LanguageHighlighter` trait, and `SyntaxHighlighterRegistry::highlight(lang: LanguageId, line: &str) -> Box<dyn Iterator<Item = HighlightSpan<'static>>>`

- [ ] **Step 1: Write the failing lexer test**

Create `crates/brain-tui/tests/lexer_tests.rs` including alias normalization checks:
```rust
use brain_tui::ui::interaction::lexer::{TokenKind, SyntaxHighlighterRegistry, normalize_language};
use brain_tui::ui::interaction::ast::LanguageId;

#[test]
fn test_rust_keyword_tokenization() {
    let line = "pub fn main() {}";
    let spans: Vec<_> = SyntaxHighlighterRegistry::highlight(LanguageId::Rust, line).collect();
    assert_eq!(spans[0].kind, TokenKind::Keyword);
    assert_eq!(spans[0].text, "pub");
}

#[test]
fn test_alias_normalization() {
    assert_eq!(normalize_language("rs"), LanguageId::Rust);
    assert_eq!(normalize_language("python"), LanguageId::Python);
    assert_eq!(normalize_language("py"), LanguageId::Python);
    assert_eq!(normalize_language("sh"), LanguageId::Shell);
    assert_eq!(normalize_language("bash"), LanguageId::Shell);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --test lexer_tests`
Expected: FAIL

- [ ] **Step 3: Lexer Framework Implementation**

Create `crates/brain-tui/src/ui/interaction/lexer.rs` implementing `LanguageHighlighter` (returning lazy iterators to avoid allocations) and highlighters for Rust, Python, Json, Shell, and PlainText.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --test lexer_tests`
Expected: PASS

- [ ] **Step 5: Commit**

Run: `git add crates/brain-tui/src/ui/interaction/lexer.rs crates/brain-tui/tests/lexer_tests.rs && git commit -m "feat: implement syntax highlighting lexer framework"`

---

### Task 3: Width-Aware Layout Tree Compiler

**Files:**
- Create: `crates/brain-tui/src/ui/interaction/layout_tree.rs`
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

- [ ] **Step 3: Phase A Implementation (Paragraph Wrapping, Headings, and Inline Spans)**

Create `crates/brain-tui/src/ui/interaction/layout_tree.rs` and implement text/heading layout compiler logic. Validate basic layout.

- [ ] **Step 4: Phase B Implementation (Tables, Lists, and Blockquotes)**

Implement structured layout for tables (sizing/alignment), lists, and blockquotes inside `layout_tree.rs`.

- [ ] **Step 5: Run all layout tests to verify it passes**

Run: `cargo test --test layout_tree_tests`
Expected: PASS

- [ ] **Step 6: Commit**

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

Create `crates/brain-tui/src/ui/render/resolver.rs`. Integrate `LinkRenderer` and `BorderResolver` inside `crates/brain-tui/src/ui/renderer.rs` to output final cells.

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

- [ ] **Step 1: Write the navigation solver property tests**

Create `crates/brain-tui/tests/navigation_solver_tests.rs`:
```rust
use brain_tui::ui::interaction::navigation::{NavigationSolver, ConversationViewState, ConversationNodeId};
use brain_tui::ui::interaction::layout_tree::{LayoutTree, LayoutBlock, VisualBlockKind};
use brain_tui::ui::interaction::ast::BlockId;
use brain_domain::MessageId;

#[test]
fn test_navigation_solver_uniqueness_and_selection_existence() {
    let blocks = vec![
        LayoutBlock { id: BlockId(1), kind: VisualBlockKind::Paragraph, lines: vec![] }
    ];
    let tree = LayoutTree::new(blocks);
    let mut view_state = ConversationViewState::default();
    let selected = ConversationNodeId::Message(MessageId(42));
    view_state.selected_node = Some(selected.clone());
    
    let index = NavigationSolver::solve(&tree, &view_state);
    
    // Invariant: every selected node must exist in the resulting NavigationIndex, and all nodes must be unique
    assert!(index.nodes.contains(&selected));
    let mut unique = index.nodes.clone();
    unique.sort_by_key(|n| format!("{:?}", n));
    unique.dedup();
    assert_eq!(unique.len(), index.nodes.len());
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --test navigation_solver_tests`
Expected: FAIL

- [ ] **Step 3: ViewState & Solver Implementation**

Create `crates/brain-tui/src/ui/interaction/navigation.rs` implementing `ConversationViewState`, `ConversationNodeId`, and the side-effect free `NavigationSolver::solve`.

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

Create `crates/brain-tui/tests/lazy_timeline_tests.rs` verifying logs are only constructed inside layout blocks when expanded.

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --test lazy_timeline_tests`
Expected: FAIL

- [ ] **Step 3: Lazy Tool Execution Blocks Implementation**

Update `crates/brain-tui/src/ui/renderer.rs` and `chat_screen.rs` to support collapsible tool blocks, lazily adding children blocks to the LayoutTree.

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
- Consumes: `LayoutCacheKey`, `LayoutCache`, `NavigationCache`
- Produces: Cached LayoutTrees and layout determinism properties

- [ ] **Step 1: Write the layout determinism test**

Create `crates/brain-tui/tests/layout_cache_tests.rs` asserting layout tree compilation determinism (excluding memory pointers/timestamps/hash order) and layout cache invalidations:

| Change             | Layout Cache | Navigation Cache |
| ------------------ | ------------ | ---------------- |
| Width              | Invalidate   | Invalidate       |
| Message revision   | Invalidate   | Invalidate       |
| Theme              | Keep         | Keep             |
| Unicode capability | Keep         | Keep             |
| Expanded sections  | Keep         | Invalidate       |

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --test layout_cache_tests`
Expected: FAIL

- [ ] **Step 3: Layout Caching Implementation**

Modify `crates/brain-tui/src/ui/renderer.rs` to cache `LayoutTree` geometry and `NavigationIndex` separately.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --test layout_cache_tests`
Expected: PASS

- [ ] **Step 5: Commit**

Run: `git add crates/brain-tui/src/ui/renderer.rs crates/brain-tui/tests/layout_cache_tests.rs && git commit -m "feat: integrate layout tree cache and enforce determinism invariants"`
