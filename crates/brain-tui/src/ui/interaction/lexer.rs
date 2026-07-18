//! Syntax highlight lexers for programming languages.

use crate::ui::interaction::ast::LanguageId;

/// Classification of parsed tokens for styling.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenKind {
    /// Reserved programming language keywords.
    Keyword,
    /// Literal strings.
    String,
    /// Numerical constants.
    Number,
    /// Comment lines or inline comments.
    Comment,
    /// Operators (e.g. +, -, =).
    Operator,
    /// Type/Struct/Class definitions.
    Type,
    /// Function names.
    Function,
    /// Regular identifiers.
    Identifier,
    /// Unstyled content.
    Plain,
}

/// A parsed token span containing semantic category and reference text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HighlightSpan<'a> {
    /// Token category.
    pub kind: TokenKind,
    /// Token text reference.
    pub text: &'a str,
}

/// Normalized language identifier mapping helper.
pub fn normalize_language(name: &str) -> LanguageId {
    match name.to_lowercase().as_str() {
        "rust" | "rs" => LanguageId::Rust,
        "python" | "py" => LanguageId::Python,
        "json" => LanguageId::Json,
        "shell" | "sh" | "bash" => LanguageId::Shell,
        "" => LanguageId::PlainText,
        _ => LanguageId::Unknown,
    }
}

/// Trait abstracting programming language syntax lexers.
pub trait LanguageHighlighter: Send + Sync {
    /// Normalized language identifier matched by this highlighter.
    fn language_id(&self) -> LanguageId;

    /// Returns a lazy iterator highlighting line elements.
    fn highlight<'a>(&self, line: &'a str) -> Box<dyn Iterator<Item = HighlightSpan<'a>> + 'a>;
}

/// Highlight lexer for Rust source code.
pub struct RustHighlighter;

impl LanguageHighlighter for RustHighlighter {
    fn language_id(&self) -> LanguageId {
        LanguageId::Rust
    }

    fn highlight<'a>(&self, line: &'a str) -> Box<dyn Iterator<Item = HighlightSpan<'a>> + 'a> {
        let trimmed = line.trim_start();
        if trimmed.starts_with("//") {
            return Box::new(std::iter::once(HighlightSpan {
                kind: TokenKind::Comment,
                text: line,
            }));
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
                spans.push(HighlightSpan {
                    kind: TokenKind::Keyword,
                    text: trimmed_word,
                });
                if !spacing.is_empty() {
                    spans.push(HighlightSpan {
                        kind: TokenKind::Plain,
                        text: spacing,
                    });
                }
            } else {
                spans.push(HighlightSpan {
                    kind: TokenKind::Plain,
                    text: word,
                });
            }
        }

        Box::new(spans.into_iter())
    }
}

/// Highlight lexer for Python source code.
pub struct PythonHighlighter;

impl LanguageHighlighter for PythonHighlighter {
    fn language_id(&self) -> LanguageId {
        LanguageId::Python
    }

    fn highlight<'a>(&self, line: &'a str) -> Box<dyn Iterator<Item = HighlightSpan<'a>> + 'a> {
        let trimmed = line.trim_start();
        if trimmed.starts_with('#') {
            return Box::new(std::iter::once(HighlightSpan {
                kind: TokenKind::Comment,
                text: line,
            }));
        }

        let mut spans = Vec::new();
        let words = line.split_inclusive(|c: char| !c.is_alphanumeric() && c != '_');

        for word in words {
            let trimmed_word = word.trim_end_matches(|c: char| !c.is_alphanumeric() && c != '_');
            let is_keyword = matches!(
                trimmed_word,
                "def"
                    | "class"
                    | "import"
                    | "from"
                    | "if"
                    | "else"
                    | "elif"
                    | "return"
                    | "print"
                    | "for"
                    | "in"
                    | "while"
                    | "try"
                    | "except"
                    | "with"
                    | "as"
                    | "pass"
                    | "None"
                    | "True"
                    | "False"
            );

            if is_keyword {
                let spacing = &word[trimmed_word.len()..];
                spans.push(HighlightSpan {
                    kind: TokenKind::Keyword,
                    text: trimmed_word,
                });
                if !spacing.is_empty() {
                    spans.push(HighlightSpan {
                        kind: TokenKind::Plain,
                        text: spacing,
                    });
                }
            } else {
                spans.push(HighlightSpan {
                    kind: TokenKind::Plain,
                    text: word,
                });
            }
        }

        Box::new(spans.into_iter())
    }
}

/// Highlight lexer for JSON text.
pub struct JsonHighlighter;

impl LanguageHighlighter for JsonHighlighter {
    fn language_id(&self) -> LanguageId {
        LanguageId::Json
    }

    fn highlight<'a>(&self, line: &'a str) -> Box<dyn Iterator<Item = HighlightSpan<'a>> + 'a> {
        let mut spans = Vec::new();
        let mut chars = line.char_indices().peekable();
        let mut last_idx = 0;

        while let Some(&(idx, c)) = chars.peek() {
            if c == '"' {
                // Yield pre-string plain text
                if idx > last_idx {
                    spans.push(HighlightSpan {
                        kind: TokenKind::Plain,
                        text: &line[last_idx..idx],
                    });
                }
                chars.next();
                let start = idx;
                let mut escaped = false;
                let mut end = start;

                while let Some(&(c_idx, c_char)) = chars.peek() {
                    end = c_idx;
                    chars.next();
                    if c_char == '"' && !escaped {
                        break;
                    }
                    escaped = c_char == '\\' && !escaped;
                }
                spans.push(HighlightSpan {
                    kind: TokenKind::String,
                    text: &line[start..=end],
                });
                last_idx = end + 1;
            } else if c.is_numeric() || c == '-' {
                if idx > last_idx {
                    spans.push(HighlightSpan {
                        kind: TokenKind::Plain,
                        text: &line[last_idx..idx],
                    });
                }
                let start = idx;
                let mut end = start;
                while let Some(&(c_idx, c_char)) = chars.peek() {
                    if c_char.is_numeric()
                        || c_char == '.'
                        || c_char == 'e'
                        || c_char == 'E'
                        || c_char == '-'
                        || c_char == '+'
                    {
                        end = c_idx;
                        chars.next();
                    } else {
                        break;
                    }
                }
                spans.push(HighlightSpan {
                    kind: TokenKind::Number,
                    text: &line[start..=end],
                });
                last_idx = end + 1;
            } else {
                chars.next();
            }
        }

        if last_idx < line.len() {
            spans.push(HighlightSpan {
                kind: TokenKind::Plain,
                text: &line[last_idx..],
            });
        }

        Box::new(spans.into_iter())
    }
}

/// Highlight lexer for Shell command lines.
pub struct ShellHighlighter;

impl LanguageHighlighter for ShellHighlighter {
    fn language_id(&self) -> LanguageId {
        LanguageId::Shell
    }

    fn highlight<'a>(&self, line: &'a str) -> Box<dyn Iterator<Item = HighlightSpan<'a>> + 'a> {
        let trimmed = line.trim_start();
        if trimmed.starts_with('#') {
            return Box::new(std::iter::once(HighlightSpan {
                kind: TokenKind::Comment,
                text: line,
            }));
        }

        let mut spans = Vec::new();
        let words = line.split_inclusive(|c: char| !c.is_alphanumeric() && c != '_');

        for word in words {
            let trimmed_word = word.trim_end_matches(|c: char| !c.is_alphanumeric() && c != '_');
            let is_keyword = matches!(
                trimmed_word,
                "echo"
                    | "cd"
                    | "ls"
                    | "mkdir"
                    | "git"
                    | "cargo"
                    | "python"
                    | "pip"
                    | "npm"
                    | "node"
                    | "sudo"
                    | "curl"
                    | "wget"
            );

            if is_keyword {
                let spacing = &word[trimmed_word.len()..];
                spans.push(HighlightSpan {
                    kind: TokenKind::Keyword,
                    text: trimmed_word,
                });
                if !spacing.is_empty() {
                    spans.push(HighlightSpan {
                        kind: TokenKind::Plain,
                        text: spacing,
                    });
                }
            } else {
                spans.push(HighlightSpan {
                    kind: TokenKind::Plain,
                    text: word,
                });
            }
        }

        Box::new(spans.into_iter())
    }
}

/// Highlight lexer for Plain text.
pub struct PlainHighlighter;

impl LanguageHighlighter for PlainHighlighter {
    fn language_id(&self) -> LanguageId {
        LanguageId::PlainText
    }

    fn highlight<'a>(&self, line: &'a str) -> Box<dyn Iterator<Item = HighlightSpan<'a>> + 'a> {
        Box::new(std::iter::once(HighlightSpan {
            kind: TokenKind::Plain,
            text: line,
        }))
    }
}

/// Registry dispatching syntax highlighter requests.
pub struct SyntaxHighlighterRegistry;

impl SyntaxHighlighterRegistry {
    /// Maps a language identifier to the target lexer and tokenizes the input line.
    pub fn highlight<'a>(
        lang: LanguageId,
        line: &'a str,
    ) -> Box<dyn Iterator<Item = HighlightSpan<'a>> + 'a> {
        match lang {
            LanguageId::Rust => Box::new(RustHighlighter.highlight(line)),
            LanguageId::Python => Box::new(PythonHighlighter.highlight(line)),
            LanguageId::Json => Box::new(JsonHighlighter.highlight(line)),
            LanguageId::Shell => Box::new(ShellHighlighter.highlight(line)),
            _ => Box::new(PlainHighlighter.highlight(line)),
        }
    }
}
