//! Volatile session context and short-term memory (STM) cache.
//!
//! This crate manages the active user session context, sliding windows of
//! short-term memory interactions, token indexing, and cache invalidation
//! lifecycles in memory before they are consolidated to long-term storage.

#![deny(missing_docs)]

use brain_domain::{Node, NodeId, SessionId};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::{Arc, RwLock};

/// Strongly-typed identifier for active chronological epochs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct EpochId(pub u64);

/// A short-term memory node containing the domain node and its insertion epoch.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StmNode {
    /// The underlying domain node.
    pub node: Node,
    /// The active epoch when this node was ingested.
    pub epoch: EpochId,
}

impl PartialEq for StmNode {
    fn eq(&self, other: &Self) -> bool {
        self.node.id == other.node.id && self.epoch == other.epoch
    }
}

/// Tokenize and normalize text by removing punctuation, lowercasing, and skipping stop-words.
fn tokenize(text: &str) -> HashSet<String> {
    let stop_words: HashSet<&str> = [
        "a", "an", "the", "and", "or", "but", "is", "are", "was", "were", "to", "of", "in", "on",
        "at", "for", "with", "by", "about", "as", "this", "that", "these", "those", "it", "its",
        "you", "your", "my", "up", "down", "out", "off",
    ]
    .iter()
    .cloned()
    .collect();

    text.to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|s| !s.is_empty() && s.len() > 1 && !stop_words.contains(s))
        .map(|s| s.to_string())
        .collect()
}

/// A volatile token-based inverted index for fast keyword searches on active STM nodes.
#[derive(Debug, Clone, Default)]
pub struct STMIndex {
    inverted_index: HashMap<String, Vec<NodeId>>,
    insertion_order: Vec<NodeId>,
}

impl STMIndex {
    /// Creates a new empty `STMIndex`.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a node to the index by tokenizing its label and string properties.
    pub fn insert(&mut self, stm_node: &StmNode) {
        let node_id = stm_node.node.id;

        // Tokenize label
        let mut tokens = tokenize(&stm_node.node.label);

        // Tokenize string properties
        for val in stm_node.node.properties.values() {
            if let serde_json::Value::String(s) = val {
                tokens.extend(tokenize(s));
            }
        }

        for token in tokens {
            self.inverted_index.entry(token).or_default().push(node_id);
        }

        if !self.insertion_order.contains(&node_id) {
            self.insertion_order.push(node_id);
        }
    }

    /// Clears all token mappings and insertion order history.
    pub fn clear(&mut self) {
        self.inverted_index.clear();
        self.insertion_order.clear();
    }

    /// Search the index for exact token matches, returning matching NodeIds in deterministic insertion order.
    pub fn search(&self, query: &str) -> Vec<NodeId> {
        let query_tokens = tokenize(query);
        if query_tokens.is_empty() {
            return Vec::new();
        }

        let mut matches = HashSet::new();
        for token in &query_tokens {
            if let Some(ids) = self.inverted_index.get(token) {
                for id in ids {
                    matches.insert(*id);
                }
            }
        }

        // Return in deterministic insertion order
        self.insertion_order
            .iter()
            .filter(|id| matches.contains(id))
            .copied()
            .collect()
    }
}

/// Manages short-term memory (STM) interactions, index updates, and epoch rotations.
pub struct SessionContext {
    session_id: SessionId,
    current_epoch: EpochId,
    interaction_sliding_window: VecDeque<StmNode>,
    index: STMIndex,
}

impl SessionContext {
    /// Creates a new `SessionContext` for the given session.
    pub fn new(session_id: SessionId) -> Self {
        Self {
            session_id,
            current_epoch: EpochId(0),
            interaction_sliding_window: VecDeque::new(),
            index: STMIndex::new(),
        }
    }

    /// Returns the session identifier.
    pub fn session_id(&self) -> SessionId {
        self.session_id
    }

    /// Returns the current active epoch.
    pub fn current_epoch(&self) -> EpochId {
        self.current_epoch
    }

    /// Returns the number of nodes in the sliding window.
    pub fn len(&self) -> usize {
        self.interaction_sliding_window.len()
    }

    /// Returns true if the sliding window is empty.
    pub fn is_empty(&self) -> bool {
        self.interaction_sliding_window.is_empty()
    }

    /// Returns an iterator over the active STM nodes in the sliding window.
    pub fn iter(&self) -> impl Iterator<Item = &StmNode> {
        self.interaction_sliding_window.iter()
    }

    /// Ingests a new node into the active epoch, indexes it, and appends it to the sliding window.
    pub fn ingest(&mut self, node: Node) -> StmNode {
        let stm_node = StmNode {
            node,
            epoch: self.current_epoch,
        };

        self.index.insert(&stm_node);
        self.interaction_sliding_window.push_back(stm_node.clone());

        stm_node
    }

    /// Rotates the active epoch to the next value and returns the previous epoch ID.
    pub fn rotate_epoch(&mut self) -> EpochId {
        let old_epoch = self.current_epoch;
        self.current_epoch = EpochId(old_epoch.0 + 1);
        old_epoch
    }

    /// Evicts and returns all nodes belonging to the specified epoch or earlier, and rebuilds the index.
    pub fn drain_epoch(&mut self, epoch_id: EpochId) -> Vec<StmNode> {
        let mut drained = Vec::new();
        let mut remaining = VecDeque::new();

        for item in self.interaction_sliding_window.drain(..) {
            if item.epoch <= epoch_id {
                drained.push(item);
            } else {
                remaining.push_back(item);
            }
        }

        self.interaction_sliding_window = remaining;

        // Rebuild index
        self.index.clear();
        for item in &self.interaction_sliding_window {
            self.index.insert(item);
        }

        drained
    }

    /// Queries the STM cache using token matching, returning matched StmNodes in sliding window order.
    pub fn query(&self, query_text: &str) -> Vec<StmNode> {
        let matched_ids = self.index.search(query_text);
        if matched_ids.is_empty() {
            return Vec::new();
        }

        let matched_set: HashSet<NodeId> = matched_ids.into_iter().collect();

        self.interaction_sliding_window
            .iter()
            .filter(|item| matched_set.contains(&item.node.id))
            .cloned()
            .collect()
    }
}

/// Thread-safe manager for mapping active SessionIds to their respective SessionContexts.
pub struct SessionCacheManager {
    contexts: RwLock<HashMap<SessionId, Arc<RwLock<SessionContext>>>>,
}

impl SessionCacheManager {
    /// Creates a new `SessionCacheManager`.
    pub fn new() -> Self {
        Self {
            contexts: RwLock::new(HashMap::new()),
        }
    }

    /// Retrieves an existing session's context or creates a new one.
    pub fn get_or_create(&self, session_id: SessionId) -> Arc<RwLock<SessionContext>> {
        if let Some(context) = self.contexts.read().unwrap().get(&session_id) {
            return Arc::clone(context);
        }

        let mut w_lock = self.contexts.write().unwrap();
        Arc::clone(
            w_lock
                .entry(session_id)
                .or_insert_with(|| Arc::new(RwLock::new(SessionContext::new(session_id)))),
        )
    }

    /// Removes a session's context from active tracking.
    pub fn remove(&self, session_id: &SessionId) {
        let mut w_lock = self.contexts.write().unwrap();
        w_lock.remove(session_id);
    }
}

impl Default for SessionCacheManager {
    fn default() -> Self {
        Self::new()
    }
}
