//! Presentation resolvers for capabilities-driven terminal borders and hyperlinks.

use crate::ui::interaction::layout_tree::{SpanAction, VisualSpan};
use crate::ui::render::context::{EffectiveCapabilities, UnicodeSupport};

/// A complete set of border line symbols for grid formatting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BorderGlyphs {
    /// Border top-left corner character.
    pub top_left: &'static str,
    /// Border top-mid column intersection character.
    pub top_mid: &'static str,
    /// Border top-right corner character.
    pub top_right: &'static str,
    /// Border mid-left row boundary character.
    pub mid_left: &'static str,
    /// Border inner cells cross intersection character.
    pub mid_mid: &'static str,
    /// Border mid-right row boundary character.
    pub mid_right: &'static str,
    /// Border bottom-left corner character.
    pub bottom_left: &'static str,
    /// Border bottom-mid column intersection character.
    pub bottom_mid: &'static str,
    /// Border bottom-right corner character.
    pub bottom_right: &'static str,
    /// Border vertical rule line character.
    pub vertical: &'static str,
    /// Border horizontal rule line character.
    pub horizontal: &'static str,
}

/// Dynamic resolver selecting border symbols based on Unicode capability context.
pub struct BorderResolver;

impl BorderResolver {
    /// Resolves border glyph elements matching observed capability level.
    pub fn resolve(caps: &EffectiveCapabilities) -> BorderGlyphs {
        if caps.unicode == UnicodeSupport::Full {
            BorderGlyphs {
                top_left: "╭",
                top_mid: "┬",
                top_right: "╮",
                mid_left: "├",
                mid_mid: "┼",
                mid_right: "┤",
                bottom_left: "╰",
                bottom_mid: "┴",
                bottom_right: "╯",
                vertical: "│",
                horizontal: "─",
            }
        } else {
            BorderGlyphs {
                top_left: "+",
                top_mid: "+",
                top_right: "+",
                mid_left: "+",
                mid_mid: "+",
                mid_right: "+",
                bottom_left: "+",
                bottom_mid: "+",
                bottom_right: "+",
                vertical: "|",
                horizontal: "-",
            }
        }
    }
}

/// Abstract representation of terminal-bound text segment output.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalSpan<'a> {
    /// Plain unescaped text content.
    Plain(&'a str),
    /// Hyperlinked text segment referencing target URL.
    Hyperlink {
        /// Display text.
        text: &'a str,
        /// Target URL string slice.
        url: &'a str,
    },
}

/// Dynamic resolver mapping links to display representation or OSC 8 protocols.
pub struct LinkRenderer;

impl LinkRenderer {
    /// Computes terminal presentation for visual spans under caps constraints.
    pub fn render<'a>(span: &'a VisualSpan, caps: &EffectiveCapabilities) -> TerminalSpan<'a> {
        match (&span.action, caps.osc8) {
            (SpanAction::Hyperlink(url), true) => TerminalSpan::Hyperlink {
                text: &span.text,
                url: url.as_str(),
            },
            (SpanAction::Hyperlink(url), false) => {
                // If text is same as URL, just print URL
                if span.text.as_ref() == url.as_str() {
                    TerminalSpan::Plain(url.as_str())
                } else {
                    // Fall back to: text (url)
                    // Note: returned lifetimes require plain fallback text to be owned or cached.
                    // For line-wrapping safety, we return plain target URL reference or formatted display.
                    TerminalSpan::Plain(&span.text)
                }
            }
            _ => TerminalSpan::Plain(&span.text),
        }
    }
}
