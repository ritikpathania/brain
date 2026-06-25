use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::retrieval::fuzzy::{tokenize, FuzzyRetrieval};
use crate::retrieval::pipeline::run_retrieval_pipeline;
use crate::retrieval::reranker::DefaultRanking;

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
pub struct TempNode {
    pub id: String,
    pub epoch: u64,
    pub content: String,
    pub timestamp: u64,
}

#[derive(Clone, Default)]
pub struct STMIndex {
    // Maps normalized word tokens to a list of matching Node IDs
    pub inverted_index: HashMap<String, Vec<String>>,
    // List of (Node ID, Raw Content) for scanning with SkimMatcherV2
    pub searchable_strings: Vec<(String, String)>,
}

impl STMIndex {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a node to the index by tokenizing its content and storing mappings.
    pub fn add(&mut self, node_id: String, content: &str) {
        let tokens = tokenize(content);
        for token in tokens {
            self.inverted_index
                .entry(token)
                .or_default()
                .push(node_id.clone());
        }
        self.searchable_strings.push((node_id, content.to_string()));
    }

    /// Rebuild the entire index from a queue of active nodes.
    pub fn rebuild(&mut self, nodes: &VecDeque<TempNode>) {
        self.inverted_index.clear();
        self.searchable_strings.clear();
        for node in nodes {
            self.add(node.id.clone(), &node.content);
        }
    }
}

#[derive(Default)]
pub struct SessionContext {
    pub current_epoch: u64,
    pub interaction_sliding_window: VecDeque<TempNode>,
    pub index: STMIndex,
    pub node_counter: u64,
}

impl SessionContext {
    pub fn new() -> Self {
        Self::default()
    }

    /// Ingest new text into the Short-Term Memory, index it, and return the created TempNode.
    pub fn ingest(&mut self, content: String) -> TempNode {
        self.node_counter += 1;
        let id = format!("node-{}", self.node_counter);

        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let node = TempNode {
            id: id.clone(),
            epoch: self.current_epoch,
            content: content.clone(),
            timestamp,
        };

        self.interaction_sliding_window.push_back(node.clone());
        self.index.add(id, &content);

        node
    }

    /// Perform a hybrid query on the active STM.
    /// Matches exact tokens and runs the SkimMatcherV2 fuzzy search over raw strings.
    pub fn query(&self, query_text: &str) -> Vec<(TempNode, i64)> {
        let retrieval = FuzzyRetrieval;
        let ranking = DefaultRanking;
        let window_vec: Vec<TempNode> = self.interaction_sliding_window.iter().cloned().collect();
        if let Ok(candidates) = run_retrieval_pipeline(
            query_text,
            &self.index,
            &window_vec,
            &retrieval,
            &ranking,
            None,
            None,
        ) {
            candidates
                .into_iter()
                .map(|n| {
                    (
                        TempNode {
                            id: n.id,
                            epoch: n
                                .attributes
                                .get("epoch")
                                .and_then(|e| e.as_u64())
                                .unwrap_or(0),
                            content: n.content,
                            timestamp: n
                                .attributes
                                .get("timestamp")
                                .and_then(|t| t.as_u64())
                                .unwrap_or(0),
                        },
                        n.score,
                    )
                })
                .collect()
        } else {
            Vec::new()
        }
    }

    /// Increment the active epoch and return the previous epoch number.
    pub fn rotate_epoch(&mut self) -> u64 {
        let old_epoch = self.current_epoch;
        self.current_epoch += 1;
        old_epoch
    }

    /// Evict and return all nodes belonging to epoch_id or earlier.
    /// Rebuilds the index for remaining items.
    pub fn drain_epoch(&mut self, epoch_id: u64) -> Vec<TempNode> {
        let mut drained = Vec::new();
        let mut remaining = VecDeque::new();

        for node in self.interaction_sliding_window.drain(..) {
            if node.epoch <= epoch_id {
                drained.push(node);
            } else {
                remaining.push_back(node);
            }
        }

        self.interaction_sliding_window = remaining;
        self.index.rebuild(&self.interaction_sliding_window);

        drained
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_ingestion_and_exact_match() {
        let mut session = SessionContext::new();
        let node1 = session.ingest("API key is stored in environment variables".to_string());
        let _node2 = session.ingest("set up database configuration in sqlite".to_string());

        assert_eq!(session.interaction_sliding_window.len(), 2);

        let results = session.query("api key");
        assert!(!results.is_empty());
        assert_eq!(results[0].0.id, node1.id);
        assert!(results[0].1 > 50); // Exact token match bonus
    }

    #[test]
    fn test_fuzzy_abbrev_match() {
        let mut session = SessionContext::new();
        let _node1 = session.ingest("API key is stored in environment variables".to_string());
        let node2 = session.ingest("set up database configuration in sqlite".to_string());

        let results = session.query("db config");
        assert!(!results.is_empty());
        assert_eq!(results[0].0.id, node2.id);
    }

    #[test]
    fn test_epoch_rotation_and_draining() {
        let mut session = SessionContext::new();

        // Ingest into Epoch 0
        let node1 = session.ingest("First event".to_string());
        let node2 = session.ingest("Second event".to_string());

        // Rotate to Epoch 1
        let old_epoch = session.rotate_epoch();
        assert_eq!(old_epoch, 0);
        assert_eq!(session.current_epoch, 1);

        // Ingest into Epoch 1
        let node3 = session.ingest("Third event".to_string());

        assert_eq!(session.interaction_sliding_window.len(), 3);

        // Drain Epoch 0 (node1 & node2)
        let drained = session.drain_epoch(0);
        assert_eq!(drained.len(), 2);
        assert!(drained.contains(&node1));
        assert!(drained.contains(&node2));

        // Only node3 remains in sliding window
        assert_eq!(session.interaction_sliding_window.len(), 1);
        assert_eq!(session.interaction_sliding_window[0].id, node3.id);

        // Index is rebuilt: searching for "first" yields nothing, "third" yields results
        assert!(session.query("first").is_empty());
        assert!(!session.query("third").is_empty());
    }

    proptest::proptest! {
        #[test]
        fn test_ingest_and_query_does_not_panic(
            ingests in proptest::collection::vec(proptest::prelude::any::<String>(), 1..10),
            query in proptest::prelude::any::<String>()
        ) {
            let mut session = SessionContext::new();
            for item in ingests {
                session.ingest(item);
            }
            let _results = session.query(&query);
        }
    }
}
