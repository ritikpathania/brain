use crate::bkf::document::BKFDocument;

/// Trait for importing document representations from external formats.
pub trait KnowledgeFormatImporter {
    /// External source representation type.
    type Source;
    /// Conversion error type.
    type Error;

    /// Translates an external representation into a canonical `BKFDocument`.
    fn import_document(&self, source: Self::Source) -> Result<BKFDocument, Self::Error>;
}

/// Trait for exporting canonical BKF Documents to other format variants.
pub trait KnowledgeFormatExporter {
    /// Target representation type.
    type Target;
    /// Conversion error type.
    type Error;

    /// Translates the canonical `BKFDocument` into the target layout.
    fn export_document(&self, document: &BKFDocument) -> Result<Self::Target, Self::Error>;
}

/// Normalization boundary trait separating raw parsing ASTs from the standard `BKFDocument`.
pub trait IngestionNormalizer {
    /// Raw source AST representation.
    type SourceAST;
    /// Normalization error type.
    type Error;

    /// Normalizes raw parsed structures into a clean canonical `BKFDocument`.
    fn normalize(&self, raw: Self::SourceAST) -> Result<BKFDocument, Self::Error>;
}
