//! Pre-tokenized Markdown AST renderer.

use crate::ui::render::context::RenderContext;
use crate::ui::theme::{ActiveTheme, ThemeToken};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

/// Pre-tokenized AST structure node.
pub enum MarkdownNode<'a> {
    /// Regular block of text.
    Text(&'a str),
    /// A header block with level metadata.
    Heading {
        /// Header hierarchy level.
        level: u8,
        /// Raw header label.
        text: &'a str,
    },
    /// A code segment block.
    CodeBlock {
        /// Highlight code language.
        lang: &'a str,
        /// Source code content.
        code: &'a str,
    },
    /// Blockquote citation block.
    Quote(&'a str),
    /// A bullet or numbered list. Borrowed slice prevents allocations.
    List(&'a [&'a str]),
    /// Horizontal separation rule block.
    Rule,
}

/// Renderer component processing pre-tokenized markdown segments.
pub struct MarkdownRenderer<'a> {
    /// List of pre-parsed markdown nodes to render.
    pub nodes: &'a [MarkdownNode<'a>],
}

impl<'a> MarkdownRenderer<'a> {
    /// Draws the first markdown node directly to the screen buffer.
    pub fn draw<T: ActiveTheme>(&self, area: Rect, buf: &mut Buffer, ctx: &RenderContext<'_, T>) {
        let style = ctx.theme.style(ThemeToken::Muted);
        if let Some(node) = self.nodes.first() {
            let text = match node {
                MarkdownNode::Text(t) => t,
                MarkdownNode::Heading { text: t, .. } => t,
                MarkdownNode::CodeBlock { code: c, .. } => c,
                MarkdownNode::Quote(t) => t,
                MarkdownNode::List(items) => items.first().copied().unwrap_or(""),
                MarkdownNode::Rule => "---",
            };
            buf.set_stringn(area.x, area.y, text, area.width as usize, style);
        }
    }
}
