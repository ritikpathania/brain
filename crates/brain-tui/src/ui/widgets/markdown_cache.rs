//! Incremental markdown block cache enforcing zero-flicker block immutability.
//!
//! Invariant: Finalized blocks (`Block 0 .. N-1`) retain their cached representation and geometry.
//! Only the active streaming block (`Block N`) is parsed and reflowed incrementally.

/// Individual cached markdown block representation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CachedBlock {
    /// Monotonic block index within the document stream.
    pub index: usize,
    /// Raw unparsed text content of the block.
    pub raw_text: String,
    /// Rendered line strings for the block.
    pub rendered_lines: Vec<String>,
    /// Whether this block has been finalized (frozen).
    pub is_finalized: bool,
}

/// Cache managing incremental markdown block updates without invalidating finalized blocks.
#[derive(Debug, Clone, Default)]
pub struct MarkdownBlockCache {
    blocks: Vec<CachedBlock>,
    active_text: String,
}

impl MarkdownBlockCache {
    /// Creates a new empty `MarkdownBlockCache`.
    pub fn new() -> Self {
        Self {
            blocks: Vec::new(),
            active_text: String::new(),
        }
    }

    /// Appends new streaming text chunks into the active block buffer.
    pub fn append_chunk(&mut self, chunk: &str) {
        self.active_text.push_str(chunk);
        self.update_active_block();
    }

    /// Returns all cached blocks. Finalized blocks (`0..N-1`) remain unchanged.
    pub fn blocks(&self) -> &[CachedBlock] {
        &self.blocks
    }

    /// Returns the number of finalized blocks in the cache.
    pub fn finalized_count(&self) -> usize {
        self.blocks.iter().filter(|b| b.is_finalized).count()
    }

    /// Clears the block cache.
    pub fn clear(&mut self) {
        self.blocks.clear();
        self.active_text.clear();
    }

    /// Updates or splits active text into finalized blocks and an active block.
    fn update_active_block(&mut self) {
        // Split on double-newlines (markdown paragraph boundaries)
        let parts: Vec<&str> = self.active_text.split("\n\n").collect();

        if parts.is_empty() {
            return;
        }

        // Finalize all preceding parts except the last active part
        for (idx, part) in parts.iter().enumerate().take(parts.len().saturating_sub(1)) {
            if idx < self.blocks.len() {
                if !self.blocks[idx].is_finalized {
                    self.blocks[idx].raw_text = part.to_string();
                    self.blocks[idx].rendered_lines = vec![part.to_string()];
                    self.blocks[idx].is_finalized = true;
                }
            } else {
                self.blocks.push(CachedBlock {
                    index: idx,
                    raw_text: part.to_string(),
                    rendered_lines: vec![part.to_string()],
                    is_finalized: true,
                });
            }
        }

        // Update active trailing block (Block N)
        if let Some(&last_part) = parts.last() {
            let active_idx = parts.len() - 1;
            let active_block = CachedBlock {
                index: active_idx,
                raw_text: last_part.to_string(),
                rendered_lines: vec![last_part.to_string()],
                is_finalized: false,
            };

            if active_idx < self.blocks.len() {
                if !self.blocks[active_idx].is_finalized {
                    self.blocks[active_idx] = active_block;
                }
            } else {
                self.blocks.push(active_block);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_markdown_block_cache_immutability() {
        let mut cache = MarkdownBlockCache::new();

        // 1. Stream paragraph 1
        cache.append_chunk("First paragraph.\n\nSecond paragraph ");
        assert_eq!(cache.blocks().len(), 2);
        assert!(cache.blocks()[0].is_finalized);
        assert!(!cache.blocks()[1].is_finalized);
        assert_eq!(cache.blocks()[0].raw_text, "First paragraph.");

        // Record identity of block 0
        let block_0_frozen = cache.blocks()[0].clone();

        // 2. Stream tokens into paragraph 2
        cache.append_chunk("continued text.\n\nThird paragraph");
        assert_eq!(cache.blocks().len(), 3);
        assert!(cache.blocks()[0].is_finalized);
        assert!(cache.blocks()[1].is_finalized);
        assert!(!cache.blocks()[2].is_finalized);

        // Assert Block 0 was NOT modified or re-parsed
        assert_eq!(cache.blocks()[0], block_0_frozen);
    }
}
