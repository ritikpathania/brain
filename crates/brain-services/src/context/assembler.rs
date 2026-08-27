//! Default deterministic context assembler with strict budget clamping and whole-item preservation.

use brain_core::context::{
    AssembledMemory, ContextAssembler, ContextSnapshot, MemoryProvenance, RankedMemory,
    RetrievalChannel,
};
use std::collections::HashSet;

/// Default implementation of `ContextAssembler`.
#[derive(Debug, Clone, Default)]
pub struct DefaultContextAssembler;

impl DefaultContextAssembler {
    /// Creates a new `DefaultContextAssembler`.
    pub fn new() -> Self {
        Self
    }

    /// Estimates token count of a given string (approx 4 chars per token).
    pub fn estimate_tokens(text: &str) -> usize {
        text.len().div_ceil(4)
    }

    /// Escapes XML special characters in untrusted memory data to defend against prompt injection.
    pub fn escape_xml(input: &str) -> String {
        let mut escaped = String::with_capacity(input.len() + 16);
        for c in input.chars() {
            match c {
                '&' => escaped.push_str("&amp;"),
                '<' => escaped.push_str("&lt;"),
                '>' => escaped.push_str("&gt;"),
                '"' => escaped.push_str("&quot;"),
                '\'' => escaped.push_str("&apos;"),
                _ => escaped.push(c),
            }
        }
        escaped
    }

    /// Formats a single memory item for inclusion in serialized context as structured untrusted data.
    fn format_item(item: &RankedMemory) -> String {
        let escaped_label = Self::escape_xml(&item.label);
        let escaped_content = Self::escape_xml(&item.content);
        format!(
            "  <memory id=\"{}\" score=\"{:.2}\" scope=\"{}\">\n    <label>{}</label>\n    <content>{}</content>\n  </memory>\n",
            item.node_id, item.final_score, item.scope, escaped_label, escaped_content
        )
    }

    /// Formats header and footer framing for context block.
    fn framing_header(epoch_id: &str) -> String {
        format!("<brain_memory_context epoch=\"{}\">\n", epoch_id)
    }

    fn framing_footer() -> &'static str {
        "</brain_memory_context>\n"
    }
}

impl ContextAssembler for DefaultContextAssembler {
    fn assemble(
        &self,
        ranked: &[RankedMemory],
        max_tokens: usize,
        max_items: usize,
        epoch_id: &str,
    ) -> ContextSnapshot {
        if ranked.is_empty() || max_tokens == 0 || max_items == 0 {
            return ContextSnapshot {
                epoch_id: epoch_id.to_string(),
                items: Vec::new(),
                token_count: 0,
                provenance: MemoryProvenance {
                    count: 0,
                    sources: Vec::new(),
                    channels: Vec::new(),
                    min_score: 0.0,
                    max_score: 0.0,
                    epoch_id: epoch_id.to_string(),
                },
                serialized_context: String::new(),
            };
        }

        let header = Self::framing_header(epoch_id);
        let footer = Self::framing_footer();
        let framing_tokens = Self::estimate_tokens(&header) + Self::estimate_tokens(footer);

        if framing_tokens >= max_tokens {
            // Budget cannot even fit framing
            return ContextSnapshot {
                epoch_id: epoch_id.to_string(),
                items: Vec::new(),
                token_count: 0,
                provenance: MemoryProvenance {
                    count: 0,
                    sources: Vec::new(),
                    channels: Vec::new(),
                    min_score: 0.0,
                    max_score: 0.0,
                    epoch_id: epoch_id.to_string(),
                },
                serialized_context: String::new(),
            };
        }

        let mut available_tokens = max_tokens - framing_tokens;
        let mut selected_items: Vec<AssembledMemory> = Vec::new();
        let mut formatted_body = String::new();

        let mut distinct_sources = HashSet::new();
        let mut distinct_channels = HashSet::new();
        let mut min_score = f64::MAX;
        let mut max_score = f64::MIN;

        for item in ranked.iter().take(max_items) {
            let item_str = Self::format_item(item);
            let item_tokens = Self::estimate_tokens(&item_str);

            if item_tokens <= available_tokens {
                available_tokens -= item_tokens;
                formatted_body.push_str(&item_str);

                distinct_sources.insert(item.node_id.to_string());
                distinct_channels.insert(item.channel);
                if item.final_score < min_score {
                    min_score = item.final_score;
                }
                if item.final_score > max_score {
                    max_score = item.final_score;
                }

                selected_items.push(AssembledMemory {
                    node_id: item.node_id,
                    label: item.label.clone(),
                    content: item.content.clone(),
                    channel: item.channel,
                    score: item.final_score,
                    timestamp: item.timestamp,
                    scope: item.scope.clone(),
                });
            } else {
                // Whole-item preservation: do not truncate item mid-sentence; stop accumulation.
                break;
            }
        }

        if selected_items.is_empty() {
            return ContextSnapshot {
                epoch_id: epoch_id.to_string(),
                items: Vec::new(),
                token_count: 0,
                provenance: MemoryProvenance {
                    count: 0,
                    sources: Vec::new(),
                    channels: Vec::new(),
                    min_score: 0.0,
                    max_score: 0.0,
                    epoch_id: epoch_id.to_string(),
                },
                serialized_context: String::new(),
            };
        }

        let mut serialized_context =
            String::with_capacity(header.len() + formatted_body.len() + footer.len());
        serialized_context.push_str(&header);
        serialized_context.push_str(&formatted_body);
        serialized_context.push_str(footer);

        let total_token_count = Self::estimate_tokens(&serialized_context);

        let mut sources_vec: Vec<String> = distinct_sources.into_iter().collect();
        sources_vec.sort();

        let mut channels_vec: Vec<RetrievalChannel> = distinct_channels.into_iter().collect();
        channels_vec.sort_by_key(|c| format!("{:?}", c));

        ContextSnapshot {
            epoch_id: epoch_id.to_string(),
            items: selected_items,
            token_count: total_token_count,
            provenance: MemoryProvenance {
                count: sources_vec.len(),
                sources: sources_vec,
                channels: channels_vec,
                min_score,
                max_score,
                epoch_id: epoch_id.to_string(),
            },
            serialized_context,
        }
    }
}
