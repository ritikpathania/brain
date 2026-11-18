//! LLM-driven knowledge graph entity and relationship extractor.

use std::collections::HashMap;
use std::sync::Arc;

use brain_core::agents::{ChatAgent, ExtractionAgent};
use brain_core::errors::BrainError;
use brain_core::services::{ExtractionRequest, ExtractionResult, MemoryExtractor};
use brain_domain::{
    Edge, EdgeDTO, GraphProvenance, GraphVersion, Node, NodeDTO, NodeId, NodeType, RelationKind,
    SessionId,
};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use uuid::Uuid;

/// Deterministic UUID namespace for memory node identification.
const EXTRACTOR_NAMESPACE: Uuid = Uuid::from_bytes([
    0x6b, 0xa7, 0xb8, 0x10, 0x9d, 0xad, 0x11, 0xd1, 0x80, 0xb4, 0x00, 0xc0, 0x4f, 0xd4, 0x30, 0xc8,
]);

/// Deterministically generates a RFC 4122 v5-compliant UUID from a canonical slug without external network dependencies.
fn deterministic_uuid(slug: &str) -> Uuid {
    let mut hasher = Sha256::new();
    hasher.update(EXTRACTOR_NAMESPACE.as_bytes());
    hasher.update(slug.as_bytes());
    let hash = hasher.finalize();

    let mut bytes = [0u8; 16];
    bytes.copy_from_slice(&hash[..16]);
    bytes[6] = (bytes[6] & 0x0f) | 0x50; // Version 5
    bytes[8] = (bytes[8] & 0x3f) | 0x80; // Variant RFC 4122
    Uuid::from_bytes(bytes)
}

/// Memory extractor utilizing an underlying LLM `ChatAgent` provider.
#[derive(Clone)]
pub struct LlmMemoryExtractor {
    provider: Arc<dyn ChatAgent>,
    name: String,
}

impl LlmMemoryExtractor {
    /// Creates a new `LlmMemoryExtractor` wrapping an LLM provider.
    pub fn new(provider: Arc<dyn ChatAgent>, name: impl Into<String>) -> Self {
        Self {
            provider,
            name: name.into(),
        }
    }

    /// Returns the descriptive name of the extractor.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Strips markdown code fence wrappers (```json ... ```) from LLM output.
    pub fn strip_code_fences(input: &str) -> &str {
        let trimmed = input.trim();
        if let Some(start_pos) = trimmed.find("```") {
            let after_fence = &trimmed[start_pos + 3..];
            let after_tag = if let Some(rest) = after_fence.strip_prefix("json") {
                rest
            } else if let Some(rest) = after_fence.strip_prefix("JSON") {
                rest
            } else {
                after_fence
            };
            let content_start = after_tag.trim_start();
            let content = if let Some(end_pos) = content_start.rfind("```") {
                &content_start[..end_pos]
            } else {
                content_start
            };
            let trimmed_content = content.trim();
            if let (Some(s), Some(e)) = (trimmed_content.find('{'), trimmed_content.rfind('}')) {
                if s <= e {
                    return &trimmed_content[s..=e];
                }
            }
            return trimmed_content;
        }

        if let (Some(s), Some(e)) = (trimmed.find('{'), trimmed.rfind('}')) {
            if s <= e {
                return &trimmed[s..=e];
            }
        }

        trimmed
    }

    /// Parses raw LLM JSON text into strongly-typed `NodeDTO` and `EdgeDTO` structures.
    pub fn parse_llm_json(raw: &str) -> Result<(Vec<NodeDTO>, Vec<EdgeDTO>), BrainError> {
        let cleaned = Self::strip_code_fences(raw);
        let parsed: RawLlmOutput =
            serde_json::from_str(cleaned).map_err(|e| BrainError::Validation {
                message: format!(
                    "Failed to parse LLM graph output JSON: {}. Raw response: {}",
                    e, raw
                ),
            })?;

        let nodes = parsed
            .nodes
            .into_iter()
            .map(|n| NodeDTO::new(n.id, n.label, n.node_type, n.attributes))
            .collect();

        let edges = parsed
            .edges
            .into_iter()
            .map(|e| EdgeDTO::new(e.source, e.target, e.relation, e.weight.unwrap_or(1.0)))
            .collect();

        Ok((nodes, edges))
    }

    /// Replicates Python `LlmMemoryExtractor.extract(stm_nodes_json)` for JSON-over-FFI callers.
    pub fn extract_semantic_nodes_json(&self, json_input: &str) -> Result<String, BrainError> {
        let stm_nodes: serde_json::Value = match serde_json::from_str(json_input) {
            Ok(val) => val,
            Err(e) => {
                let err_resp = serde_json::json!({
                    "status": "error",
                    "message": format!("Failed to parse input STM nodes: {}", e),
                    "nodes": [],
                    "edges": []
                });
                return Ok(err_resp.to_string());
            }
        };

        let prompt = format!(
            "You are a knowledge graph extractor. Extract semantic entities (nodes) and their relations (edges) from the following short-term memory transcripts. Return ONLY a valid JSON object matching this schema:\n\
            {{\"nodes\": [{{\"id\": \"entity-id\", \"label\": \"Entity Label\", \"type\": \"entity-type\", \"attributes\": {{}}}}], \"edges\": [{{\"source\": \"entity-id\", \"target\": \"entity-id\", \"relation\": \"relationship_name\"}}]}}\n\n\
            Transcripts:\n{}\n\n\
            JSON output:",
            serde_json::to_string_pretty(&stm_nodes).unwrap_or_default()
        );

        let llm_response = match self.provider.chat(SessionId::new(), &prompt) {
            Ok(resp) => resp,
            Err(e) => {
                let err_resp = serde_json::json!({
                    "status": "error",
                    "message": format!("LLM generation failed: {}", e),
                    "nodes": [],
                    "edges": []
                });
                return Ok(err_resp.to_string());
            }
        };

        match Self::parse_llm_json(&llm_response) {
            Ok((nodes, edges)) => {
                let success_resp = serde_json::json!({
                    "status": "success",
                    "nodes": nodes,
                    "edges": edges
                });
                Ok(success_resp.to_string())
            }
            Err(e) => {
                let err_resp = serde_json::json!({
                    "status": "error",
                    "message": format!("LLM parsing failed: {}. Raw response: {}", e, llm_response),
                    "nodes": [],
                    "edges": []
                });
                Ok(err_resp.to_string())
            }
        }
    }
}

impl ExtractionAgent for LlmMemoryExtractor {
    fn name(&self) -> &str {
        &self.name
    }

    fn extract_graph(&self, text: &str) -> Result<(Vec<NodeDTO>, Vec<EdgeDTO>), BrainError> {
        let prompt = format!(
            "You are a knowledge graph extractor. Extract semantic entities (nodes) and their relations (edges) from the following text. Return ONLY a valid JSON object matching this schema:\n\
            {{\"nodes\": [{{\"id\": \"entity-id\", \"label\": \"Entity Label\", \"type\": \"entity-type\", \"attributes\": {{}}}}], \"edges\": [{{\"source\": \"entity-id\", \"target\": \"entity-id\", \"relation\": \"relationship_name\"}}]}}\n\n\
            Text:\n{}\n\n\
            JSON output:",
            text
        );
        let response = self.provider.chat(SessionId::new(), &prompt)?;
        Self::parse_llm_json(&response)
    }
}

impl MemoryExtractor for LlmMemoryExtractor {
    fn extract(&self, request: ExtractionRequest) -> Result<ExtractionResult, BrainError> {
        let (node_dtos, edge_dtos) = self.extract_graph(&request.raw_content)?;

        let mut id_map: HashMap<String, NodeId> = HashMap::new();
        let mut nodes: Vec<Node> = Vec::with_capacity(node_dtos.len());

        for dto in node_dtos {
            let node_uuid = deterministic_uuid(&dto.id);
            let node_id = NodeId(node_uuid);
            id_map.insert(dto.id.clone(), node_id);

            let node_type = match dto.node_type.to_lowercase().as_str() {
                "technology" | "language" | "runtime" | "library" => NodeType::Technology,
                "database" => NodeType::Database,
                "project" => NodeType::Project,
                "person" => NodeType::Person,
                "organization" => NodeType::Organization,
                "credential" => NodeType::Credential,
                "file" => NodeType::File,
                "service" => NodeType::Service,
                "tool" => NodeType::Tool,
                _ => NodeType::Concept,
            };

            let mut properties: HashMap<String, serde_json::Value> = match dto.attributes {
                serde_json::Value::Object(map) => map.into_iter().collect(),
                _ => HashMap::new(),
            };
            properties.insert("canonical_id".to_string(), serde_json::json!(dto.id));

            let mut provenance = GraphProvenance::default();
            if let Some(conv_id) = request.context_metadata.get("conversation_id") {
                provenance.source_conversation = Some(conv_id.clone());
            }
            if let Some(msg_id) = request.context_metadata.get("message_id") {
                provenance.source_message = Some(msg_id.clone());
            }
            provenance.extractor_version = "llm-v1".to_string();

            let mut node = Node::new(node_id, dto.label, node_type).with_properties(properties);
            node.provenance = provenance;
            nodes.push(node);
        }

        let mut edges: Vec<Edge> = Vec::with_capacity(edge_dtos.len());
        for dto in edge_dtos {
            let (Some(&src_id), Some(&tgt_id)) = (id_map.get(&dto.source), id_map.get(&dto.target))
            else {
                continue;
            };

            let relation = dto
                .relation
                .parse::<RelationKind>()
                .unwrap_or(RelationKind::AssociatedWith);

            let mut edge = Edge::new(src_id, tgt_id, relation, dto.weight);
            let mut provenance = GraphProvenance::default();
            if let Some(conv_id) = request.context_metadata.get("conversation_id") {
                provenance.source_conversation = Some(conv_id.clone());
            }
            if let Some(msg_id) = request.context_metadata.get("message_id") {
                provenance.source_message = Some(msg_id.clone());
            }
            provenance.extractor_version = "llm-v1".to_string();
            edge.provenance = provenance;
            edges.push(edge);
        }

        Ok(ExtractionResult {
            nodes,
            edges,
            provenance: GraphProvenance::default(),
            graph_version: GraphVersion::V1,
        })
    }
}

#[derive(Debug, Deserialize)]
struct RawLlmOutput {
    #[serde(default)]
    nodes: Vec<RawLlmNode>,
    #[serde(default)]
    edges: Vec<RawLlmEdge>,
}

#[derive(Debug, Deserialize)]
struct RawLlmNode {
    id: String,
    label: String,
    #[serde(alias = "type", default = "default_node_type")]
    node_type: String,
    #[serde(default = "default_attributes")]
    attributes: serde_json::Value,
}

#[derive(Debug, Deserialize)]
struct RawLlmEdge {
    source: String,
    target: String,
    relation: String,
    #[serde(default = "default_weight")]
    weight: Option<f64>,
}

fn default_node_type() -> String {
    "concept".to_string()
}

fn default_attributes() -> serde_json::Value {
    serde_json::json!({})
}

fn default_weight() -> Option<f64> {
    Some(1.0)
}
