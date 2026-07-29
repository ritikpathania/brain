//! Knowledge Reasoning Engine (Phase 5 Milestone 5.2).
//!
//! Provides a domain-oriented Intermediate Representation (`InferenceGraph`), monotonic inference passes (`InferencePass`),
//! composable `InferencePipeline`, presentation `ResponseSynthesizer`, and orchestration root `KnowledgeReasoningEngine`.

pub mod engine;
pub mod models;
pub mod pass;
pub mod pipeline;
pub mod synthesizer;

pub use engine::KnowledgeReasoningEngine;
pub use models::{
    ConfidenceMetrics, EvidenceRef, InferenceEdge, InferenceGraph, InferenceKind, InferenceNode,
    InferenceNodeId, KnowledgeResponse, Proposition, ReasoningTraceStep,
};
pub use pass::{
    CausalInferencePass, ContradictionResolutionPass, InferencePass, TemporalInferencePass,
};
pub use pipeline::InferencePipeline;
pub use synthesizer::ResponseSynthesizer;
