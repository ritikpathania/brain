use serde::{Deserialize, Serialize};

pub mod duckdb;
pub mod sqlite;

pub use duckdb::AnalyticsDatabase;
pub use sqlite::LtmDatabase;

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
pub struct ExtractedNode {
    pub id: String,
    pub label: String,
    #[serde(rename = "type")]
    pub node_type: String,
    pub attributes: serde_json::Value,
}

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
pub struct ExtractedEdge {
    pub source: String,
    pub target: String,
    pub relation: String,
}

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
pub struct ExtractedGraph {
    pub nodes: Vec<ExtractedNode>,
    pub edges: Vec<ExtractedEdge>,
}
