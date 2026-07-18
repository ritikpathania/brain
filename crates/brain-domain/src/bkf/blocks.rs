use crate::bkf::ids::BkfBlockId;
use crate::bkf::provenance::Provenance;
use serde::{Deserialize, Serialize};

/// Categorized kinds of structural document blocks.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", content = "content", rename_all = "snake_case")]
pub enum BlockContent {
    /// A paragraph of text.
    Paragraph(String),
    /// A section heading.
    Heading {
        /// Text content of heading.
        text: String,
        /// Level of heading.
        level: u32,
    },
    /// Source code block.
    Code {
        /// Raw code string.
        code: String,
        /// Optional language syntax identifier.
        language: Option<String>,
    },
    /// Structured table.
    Table {
        /// Header cells.
        headers: Vec<String>,
        /// Row cells.
        rows: Vec<Vec<String>>,
    },
    /// Ordered or unordered list.
    List {
        /// Items within the list.
        items: Vec<String>,
        /// True if ordered, false if unordered.
        ordered: bool,
    },
    /// A blockquote.
    Quote(String),
    /// Image reference block.
    Image {
        /// URL/path to the image.
        url: String,
        /// Alt text description.
        alt_text: Option<String>,
    },
    /// Extensible custom block kind.
    Custom {
        /// Custom kind name.
        custom_kind: String,
        /// Arbitrary custom block data.
        data: serde_json::Value,
    },
}

/// A sequential block that makes up a document or section.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Block {
    /// Block ID.
    pub id: BkfBlockId,
    /// Block type and data content.
    pub content: BlockContent,
    /// History of provenance sources for this block.
    pub provenance: Vec<Provenance>,
    /// Classifications/tags applied to the block.
    pub tags: Vec<String>,
}
