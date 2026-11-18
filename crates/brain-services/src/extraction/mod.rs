//! Knowledge graph entity and relation extraction modules.

/// Native heuristic extraction engine with deterministic behavioral parity to Python oracle.
pub mod heuristics;
pub use heuristics::HeuristicExtractor;

/// LLM-driven knowledge graph entity and relationship extractor.
pub mod llm;
pub use llm::LlmMemoryExtractor;
