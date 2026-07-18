//! Width-agnostic markdown parser implementation.

use crate::ui::interaction::ast::{
    CitationId, DocumentBlock, InlineNode, LanguageId, LinkTarget, ListKind, TableCell, TableNode,
};

/// Width-agnostic markdown document parser.
pub struct MarkdownParser;

impl MarkdownParser {
    /// Parses raw markdown text into structural AST blocks.
    pub fn parse_to_blocks(text: &str) -> Vec<DocumentBlock> {
        let mut blocks = Vec::new();
        let mut lines = text.lines().peekable();

        while let Some(&line) = lines.peek() {
            let trimmed = line.trim();

            if trimmed.is_empty() {
                lines.next();
                continue;
            }

            // 1. Horizontal Rule Check
            if trimmed == "---" || trimmed == "***" || trimmed == "___" {
                lines.next();
                blocks.push(DocumentBlock::HorizontalRule);
                continue;
            }

            // 2. Heading Check
            if trimmed.starts_with('#') {
                let text_line = lines.next().unwrap();
                let heading_trimmed = text_line.trim_start_matches('#');
                let level = (text_line.len() - heading_trimmed.len()) as u8;
                if level >= 1
                    && level <= 6
                    && (heading_trimmed.starts_with(' ') || heading_trimmed.is_empty())
                {
                    let content = parse_inline(heading_trimmed.trim());
                    blocks.push(DocumentBlock::Heading { level, content });
                    continue;
                }
            }

            // 3. Fenced Code Block Check
            if trimmed.starts_with("```") {
                let start_fence = lines.next().unwrap().trim();
                let lang_str = if start_fence.len() > 3 {
                    start_fence[3..].trim()
                } else {
                    ""
                };
                let language = match lang_str.to_lowercase().as_str() {
                    "" => LanguageId::PlainText,
                    "rust" | "rs" => LanguageId::Rust,
                    "python" | "py" => LanguageId::Python,
                    "json" => LanguageId::Json,
                    "shell" | "sh" | "bash" => LanguageId::Shell,
                    _other => LanguageId::Unknown,
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

                blocks.push(DocumentBlock::CodeBlock {
                    language,
                    lines: code_lines,
                });
                continue;
            }

            // 4. Blockquote Check
            if trimmed.starts_with('>') {
                let mut quote_content = Vec::new();
                while let Some(&quote_line) = lines.peek() {
                    let quote_trimmed = quote_line.trim();
                    if quote_trimmed.starts_with('>') {
                        let content = quote_line.strip_prefix('>').unwrap_or(quote_line);
                        let content = if content.starts_with(' ') {
                            &content[1..]
                        } else {
                            content
                        };
                        quote_content.push(content.to_string());
                        lines.next();
                    } else {
                        break;
                    }
                }
                let quote_text = quote_content.join("\n");
                let nested_blocks = Self::parse_to_blocks(&quote_text);
                blocks.push(DocumentBlock::BlockQuote(nested_blocks));
                continue;
            }

            // 5. Unordered and Ordered Lists Check
            if trimmed.starts_with("- ")
                || trimmed.starts_with("* ")
                || (trimmed.chars().next().unwrap_or(' ').is_numeric() && trimmed.contains(". "))
            {
                let is_ordered = !trimmed.starts_with("- ") && !trimmed.starts_with("* ");
                let kind = if is_ordered {
                    ListKind::Ordered
                } else {
                    ListKind::Unordered
                };
                let mut items = Vec::new();

                while let Some(&list_line) = lines.peek() {
                    let list_trimmed = list_line.trim();
                    if list_trimmed.is_empty() {
                        break;
                    }

                    if list_trimmed.starts_with("- ") || list_trimmed.starts_with("* ") {
                        if is_ordered {
                            break;
                        }
                        let item_text = &list_trimmed[2..];
                        items.push(parse_inline(item_text));
                        lines.next();
                    } else if let Some(dot_idx) = list_trimmed.find(". ") {
                        let prefix = &list_trimmed[..dot_idx];
                        if prefix.chars().all(|c| c.is_numeric()) {
                            if !is_ordered {
                                break;
                            }
                            let item_text = &list_trimmed[dot_idx + 2..];
                            items.push(parse_inline(item_text));
                            lines.next();
                        } else {
                            break;
                        }
                    } else {
                        break;
                    }
                }

                blocks.push(DocumentBlock::List { kind, items });
                continue;
            }

            // 6. Table Check
            if trimmed.starts_with('|') && trimmed.ends_with('|') {
                let header_line = lines.next().unwrap();
                let raw_headers = parse_table_row(header_line);
                let headers: Vec<TableCell> = raw_headers
                    .into_iter()
                    .map(|h| TableCell {
                        content: parse_inline(&h),
                    })
                    .collect();

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
                        let raw_cells = parse_table_row(lines.next().unwrap());
                        let row_cells: Vec<TableCell> = raw_cells
                            .into_iter()
                            .map(|c| TableCell {
                                content: parse_inline(&c),
                            })
                            .collect();
                        rows.push(row_cells);
                    } else {
                        break;
                    }
                }

                blocks.push(DocumentBlock::Table(TableNode { headers, rows }));
                continue;
            }

            // 7. Regular Paragraph Text Flow (with link & citation fallback)
            let p_line = lines.next().unwrap();
            let inlines = parse_inline(p_line);
            blocks.push(DocumentBlock::Paragraph(inlines));
        }

        blocks
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

/// Helper parsing text into recursive InlineNode sequences.
pub fn parse_inline(text: &str) -> Vec<InlineNode> {
    let chars = text.chars().collect::<Vec<_>>();
    parse_inline_recursive(&chars)
}

fn parse_inline_recursive(chars: &[char]) -> Vec<InlineNode> {
    let mut nodes = Vec::new();
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
            let mut content_chars = Vec::new();
            let mut closed = false;
            while idx < chars.len() {
                if idx + 1 < chars.len() && chars[idx] == '*' && chars[idx + 1] == '*' {
                    idx += 2;
                    closed = true;
                    break;
                } else {
                    content_chars.push(chars[idx]);
                    idx += 1;
                }
            }
            if closed {
                nodes.push(InlineNode::Strong(parse_inline_recursive(&content_chars)));
            } else {
                current_text.push_str("**");
                for c in content_chars {
                    current_text.push(c);
                }
            }
            continue;
        }

        // Italic: * or _
        if chars[idx] == '*' || chars[idx] == '_' {
            let delim = chars[idx];
            flush_text!();
            idx += 1;
            let mut content_chars = Vec::new();
            let mut closed = false;
            while idx < chars.len() {
                if chars[idx] == delim {
                    idx += 1;
                    closed = true;
                    break;
                } else {
                    content_chars.push(chars[idx]);
                    idx += 1;
                }
            }
            if closed {
                nodes.push(InlineNode::Emphasis(parse_inline_recursive(&content_chars)));
            } else {
                current_text.push(delim);
                for c in content_chars {
                    current_text.push(c);
                }
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
                nodes.push(InlineNode::Code(code_content));
            } else {
                current_text.push('`');
                current_text.push_str(&code_content);
            }
            continue;
        }

        // Link: [label](url)
        if chars[idx] == '[' {
            flush_text!();
            idx += 1;
            let mut label_chars = Vec::new();
            let mut closed_bracket = false;
            while idx < chars.len() {
                if chars[idx] == ']' {
                    idx += 1;
                    closed_bracket = true;
                    break;
                } else {
                    label_chars.push(chars[idx]);
                    idx += 1;
                }
            }

            if closed_bracket && idx < chars.len() && chars[idx] == '(' {
                idx += 1;
                let mut url_str = String::new();
                let mut closed_paren = false;
                while idx < chars.len() {
                    if chars[idx] == ')' {
                        idx += 1;
                        closed_paren = true;
                        break;
                    } else {
                        url_str.push(chars[idx]);
                        idx += 1;
                    }
                }

                if closed_paren {
                    let children = parse_inline_recursive(&label_chars);
                    nodes.push(InlineNode::Link {
                        children,
                        url: LinkTarget::new(url_str),
                    });
                    continue;
                } else {
                    current_text.push('[');
                    for c in &label_chars {
                        current_text.push(*c);
                    }
                    current_text.push(']');
                    current_text.push('(');
                    current_text.push_str(&url_str);
                    continue;
                }
            } else {
                let label_str: String = label_chars.iter().collect();
                if closed_bracket
                    && (label_str.chars().all(|c| c.is_numeric()) || label_str.starts_with('^'))
                {
                    nodes.push(InlineNode::Citation(CitationId(label_str.into_boxed_str())));
                } else {
                    current_text.push('[');
                    for c in label_chars {
                        current_text.push(c);
                    }
                    if closed_bracket {
                        current_text.push(']');
                    }
                }
                continue;
            }
        }

        current_text.push(chars[idx]);
        idx += 1;
    }

    flush_text!();
    nodes
}
