use brain_tui::ui::render::resolver::{BorderResolver, LinkRenderer, TerminalSpan};
use brain_tui::ui::render::context::{RenderCapabilities, CapabilityPolicy, CapabilityResolver, UnicodeSupport};
use brain_tui::ui::interaction::layout_tree::{VisualSpan, VisualStyle, SpanAction};
use brain_tui::ui::interaction::ast::LinkTarget;

#[test]
fn test_border_glyph_resolution() {
    let caps = RenderCapabilities::detect();
    let policy = CapabilityPolicy::default();
    let mut capabilities = CapabilityResolver::resolve(&caps, &policy);
    
    capabilities.unicode = UnicodeSupport::Full;
    let glyphs = BorderResolver::resolve(&capabilities);
    assert_eq!(glyphs.top_left, "┌");
    assert_eq!(glyphs.top_mid, "┬");
    assert_eq!(glyphs.vertical, "│");

    capabilities.unicode = UnicodeSupport::AsciiOnly;
    let glyphs = BorderResolver::resolve(&capabilities);
    assert_eq!(glyphs.top_left, "+");
    assert_eq!(glyphs.top_mid, "+");
    assert_eq!(glyphs.vertical, "|");
}

#[test]
fn test_link_renderer_osc8_enabled() {
    let caps = RenderCapabilities::detect();
    let policy = CapabilityPolicy::default();
    let mut capabilities = CapabilityResolver::resolve(&caps, &policy);
    capabilities.osc8 = true;

    let target = LinkTarget::new("https://google.com");
    let span = VisualSpan::new(
        "Google",
        VisualStyle::Normal,
        None,
        SpanAction::Hyperlink(target),
    );

    let term_span = LinkRenderer::render(&span, &capabilities);
    assert!(matches!(
        term_span,
        TerminalSpan::Hyperlink { text: "Google", url: "https://google.com" }
    ));
}

#[test]
fn test_link_renderer_osc8_disabled() {
    let caps = RenderCapabilities::detect();
    let policy = CapabilityPolicy::default();
    let mut capabilities = CapabilityResolver::resolve(&caps, &policy);
    capabilities.osc8 = false;

    let target = LinkTarget::new("https://google.com");
    let span = VisualSpan::new(
        "Google",
        VisualStyle::Normal,
        None,
        SpanAction::Hyperlink(target),
    );

    let term_span = LinkRenderer::render(&span, &capabilities);
    assert!(matches!(term_span, TerminalSpan::Plain("Google")));
}
