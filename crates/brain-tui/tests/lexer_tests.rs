use brain_tui::ui::interaction::lexer::{TokenKind, SyntaxHighlighterRegistry, normalize_language};
use brain_tui::ui::interaction::ast::LanguageId;

#[test]
fn test_rust_keyword_tokenization() {
    let line = "pub fn main() {}";
    let spans: Vec<_> = SyntaxHighlighterRegistry::highlight(LanguageId::Rust, line).collect();
    assert_eq!(spans[0].kind, TokenKind::Keyword);
    assert_eq!(spans[0].text, "pub");
    assert_eq!(spans[2].kind, TokenKind::Keyword);
    assert_eq!(spans[2].text, "fn");
}

#[test]
fn test_alias_normalization() {
    assert_eq!(normalize_language("rs"), LanguageId::Rust);
    assert_eq!(normalize_language("python"), LanguageId::Python);
    assert_eq!(normalize_language("py"), LanguageId::Python);
    assert_eq!(normalize_language("sh"), LanguageId::Shell);
    assert_eq!(normalize_language("bash"), LanguageId::Shell);
    assert_eq!(normalize_language("json"), LanguageId::Json);
    assert_eq!(normalize_language("RUST"), LanguageId::Rust);
}

#[test]
fn test_json_highlighting() {
    let line = r#"  "status": "ok", "code": 200 "#;
    let spans: Vec<_> = SyntaxHighlighterRegistry::highlight(LanguageId::Json, line).collect();
    
    // Check if the strings and numbers are identified
    let strings: Vec<_> = spans.iter().filter(|s| s.kind == TokenKind::String).collect();
    let numbers: Vec<_> = spans.iter().filter(|s| s.kind == TokenKind::Number).collect();
    
    assert_eq!(strings.len(), 3);
    assert_eq!(strings[0].text, r#""status""#);
    assert_eq!(strings[1].text, r#""ok""#);
    assert_eq!(strings[2].text, r#""code""#);
    
    assert_eq!(numbers.len(), 1);
    assert_eq!(numbers[0].text, "200");
}
