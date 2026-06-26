use brain_core::*;
use std::error::Error;

#[test]
fn test_brain_error_display() {
    let err = BrainError::Configuration {
        message: "Invalid version".to_string(),
    };
    assert_eq!(format!("{}", err), "Configuration Error: Invalid version");
}

#[test]
fn test_brain_error_source_chaining() {
    let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
    let err = BrainError::Storage {
        message: "DB open failed".to_string(),
        source: Some(Box::new(io_err)),
    };

    assert_eq!(format!("{}", err), "Storage Error: DB open failed");
    assert!(err.source().is_some());
    assert_eq!(format!("{}", err.source().unwrap()), "file not found");
}

struct MockTool {
    metadata: ToolMetadata,
}

impl Tool for MockTool {
    fn metadata(&self) -> &ToolMetadata {
        &self.metadata
    }

    fn execute(
        &self,
        _arguments: &std::collections::HashMap<String, serde_json::Value>,
    ) -> Result<serde_json::Value, BrainError> {
        Ok(serde_json::Value::Null)
    }
}

#[test]
fn test_trait_object_compilation() {
    let metadata = ToolMetadata {
        name: "mock_tool".to_string(),
        description: "A testing tool".to_string(),
        usage: "mock_tool <arg>".to_string(),
        version: "0.1.0".to_string(),
        author: "Agent".to_string(),
        required_permissions: vec!["fs:read".to_string()],
        timeout_ms: 500,
        supports_streaming: false,
        is_idempotent: true,
        causes_side_effects: false,
    };

    let tool: Box<dyn Tool> = Box::new(MockTool { metadata });
    assert_eq!(tool.metadata().name, "mock_tool");
    assert_eq!(tool.metadata().required_permissions[0], "fs:read");
}

#[test]
fn test_retrieval_abstractions() {
    let session_id = brain_domain::SessionId::new();
    let request = RetrievalRequest {
        session_id,
        query: "test query".to_string(),
        limit: 10,
        exclude_ids: std::collections::HashSet::new(),
        deadline: None,
    };

    let node = brain_domain::Node::new(
        brain_domain::NodeId::new(),
        "test_label".to_string(),
        brain_domain::NodeType::Concept,
    );

    let ranking = IdentityRanking;
    let ranked_nodes = ranking.rank(&request, vec![node.clone()]).unwrap();
    assert_eq!(ranked_nodes.len(), 1);
    assert_eq!(ranked_nodes[0].id, node.id);

    let metadata = SourceMetadata {
        source_name: "test_source",
    };
    let result = MemorySourceResult {
        nodes: vec![node],
        metadata,
    };
    assert_eq!(result.metadata.source_name, "test_source");
}
