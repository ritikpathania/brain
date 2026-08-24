use crate::identifiers::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

/// Returns the current Unix timestamp in seconds.
fn current_unix_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// The role of a chat message sender.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MessageRole {
    /// Message sent by the human user.
    User,
    /// Message sent by the AI assistant.
    Assistant,
    /// Message containing system prompt instructions.
    System,
    /// Agentic-loop tool outcome persisted as part of the transcript (Inc 8).
    Tool,
}

impl std::fmt::Display for MessageRole {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::User => write!(f, "user"),
            Self::Assistant => write!(f, "assistant"),
            Self::System => write!(f, "system"),
            Self::Tool => write!(f, "tool"),
        }
    }
}

impl std::str::FromStr for MessageRole {
    type Err = std::convert::Infallible;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let role = match s.to_lowercase().as_str() {
            "assistant" => Self::Assistant,
            "system" => Self::System,
            "tool" => Self::Tool,
            _ => Self::User,
        };
        Ok(role)
    }
}

/// The type classification of a knowledge graph node.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum NodeKind {
    /// A person entity.
    Person,
    /// A project workspace entity.
    Project,
    /// An organization entity.
    Organization,
    /// A technology or language entity.
    Technology,
    /// A database entity.
    Database,
    /// A file path or source code entity.
    File,
    /// A credential entity.
    Credential,
    /// An abstract concept or topic entity.
    Concept,
    /// An agent-callable tool entity.
    Tool,
    /// A background service entity.
    Service,
    /// Fallback for unknown/unrecognized variants.
    #[serde(other)]
    Unknown,
}

/// Type alias for backward compatibility.
pub type NodeType = NodeKind;

impl std::fmt::Display for NodeKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Self::Person => "person",
            Self::Project => "project",
            Self::Organization => "organization",
            Self::Technology => "technology",
            Self::Database => "database",
            Self::File => "file",
            Self::Credential => "credential",
            Self::Concept => "concept",
            Self::Tool => "tool",
            Self::Service => "service",
            Self::Unknown => "unknown",
        };
        write!(f, "{}", s)
    }
}

impl std::str::FromStr for NodeKind {
    type Err = std::convert::Infallible;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let kind = match s.to_lowercase().as_str() {
            "person" => Self::Person,
            "project" => Self::Project,
            "organization" => Self::Organization,
            "technology" => Self::Technology,
            "database" => Self::Database,
            "file" => Self::File,
            "credential" => Self::Credential,
            "concept" => Self::Concept,
            "tool" => Self::Tool,
            "service" => Self::Service,
            _ => Self::Unknown,
        };
        Ok(kind)
    }
}

/// The kind classification of a relationship edge.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RelationKind {
    /// Indicates usage.
    Uses,
    /// Indicates dependency.
    DependsOn,
    /// Indicates platform execution.
    RunsOn,
    /// Indicates development ownership.
    Develops,
    /// Indicates storage location.
    StoredIn,
    /// Indicates configuration coupling.
    Configures,
    /// Indicates protocol/network communications channel.
    CommunicatesVia,
    /// Indicates generic association.
    AssociatedWith,
    /// Fallback for unknown/unrecognized variants.
    #[serde(other)]
    Unknown,
}

impl RelationKind {
    /// Returns the canonical stable identifier for the relation type.
    pub fn id(self) -> crate::identifiers::RelationId {
        let s = match self {
            Self::Uses => "uses",
            Self::DependsOn => "depends_on",
            Self::RunsOn => "runs_on",
            Self::Develops => "develops",
            Self::StoredIn => "stored_in",
            Self::Configures => "configures",
            Self::CommunicatesVia => "communicates_via",
            Self::AssociatedWith => "associated_with",
            Self::Unknown => "unknown",
        };
        crate::identifiers::RelationId::new(s)
    }

    /// Static slice containing all valid relation variants (excluding Unknown).
    pub const ALL: &'static [Self] = &[
        Self::Uses,
        Self::DependsOn,
        Self::RunsOn,
        Self::Develops,
        Self::StoredIn,
        Self::Configures,
        Self::CommunicatesVia,
        Self::AssociatedWith,
    ];
}

impl std::fmt::Display for RelationKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.id())
    }
}

impl std::str::FromStr for RelationKind {
    type Err = std::convert::Infallible;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let kind = match s.to_lowercase().replace("-", "_").as_str() {
            "uses" => Self::Uses,
            "depends_on" => Self::DependsOn,
            "runs_on" => Self::RunsOn,
            "develops" => Self::Develops,
            "stored_in" => Self::StoredIn,
            "configures" => Self::Configures,
            "communicates_via" => Self::CommunicatesVia,
            "associated_with" => Self::AssociatedWith,
            _ => Self::Unknown,
        };
        Ok(kind)
    }
}

/// Version wrapper for the knowledge graph protocol.
/// Intended to enforce explicit migrations and serialization compatibility.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct GraphVersion(u32);

impl GraphVersion {
    /// Version 1 of the graph protocol.
    pub const V1: Self = Self(1);

    /// Returns the raw u32 representation of the version.
    pub fn value(self) -> u32 {
        self.0
    }
}

/// Classification of the semantic origin/source of a graph element.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum ProvenanceSource {
    /// Extracted via NLP/Heuristics/LLM from user interactions.
    #[default]
    Extracted,
    /// Inferred via rule-based or transitive deduction.
    Inferred,
    /// Imported from external static configurations or repositories.
    Imported,
    /// Manually authored or updated directly by the user.
    UserAuthored,
}

/// Intrinsic provenance indicating the origin, author, and context of graph elements.
/// Intrinsic fields are strictly immutable once persisted.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GraphProvenance {
    /// The conversation ID from which this was extracted.
    pub source_conversation: Option<String>,
    /// The message ID from which this was extracted.
    pub source_message: Option<String>,
    /// Timestamp (Unix seconds) when the extraction occurred.
    pub extracted_at: u64,
    /// Version of the extractor that parsed this entity.
    pub extractor_version: String,
    /// Confidence score assigned by the extractor.
    pub confidence: f32,
    /// Exact text span from which the extraction was parsed.
    pub text_span: Option<String>,
    /// Classification of the origin of this graph element.
    #[serde(default)]
    pub source: ProvenanceSource,
}

impl Default for GraphProvenance {
    fn default() -> Self {
        Self {
            source_conversation: None,
            source_message: None,
            extracted_at: current_unix_timestamp(),
            extractor_version: "v1.0.0".to_string(),
            confidence: 1.0,
            text_span: None,
            source: ProvenanceSource::Extracted,
        }
    }
}

/// The lifecycle states of a system extension plugin.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PluginState {
    /// Plugin located on disk but not yet processed.
    Discovered,
    /// Dependencies specified in the manifest have been resolved.
    DependenciesResolved,
    /// Manifest schema, signatures, and capabilities validated.
    Validated,
    /// Crate assemblies or scripts loaded into the execution context.
    Loaded,
    /// Handlers and settings initialized.
    Initialized,
    /// Dynamic tools and handlers actively running.
    Active,
    /// Temporarily paused or restricted.
    Suspended,
    /// Explicitly disabled.
    Disabled,
    /// Cleaned up and unloaded from runtime memory.
    Unloaded,
}

/// Represents a knowledge graph node containing semantic properties.
///
/// Invariants:
/// - Node ID is strictly immutable once persisted.
/// - Provenance information is intrinsic and immutable.
#[non_exhaustive]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Node {
    /// The unique identifier of the node.
    pub id: NodeId,
    /// The primary label/name of the node.
    pub label: String,
    /// The category or type classification of the node.
    pub node_type: NodeType,
    /// Extensible metadata and key-value properties.
    pub properties: HashMap<String, serde_json::Value>,
    /// Intrinsic immutable provenance of the node.
    pub provenance: GraphProvenance,
    /// Unix timestamp when the node was last updated.
    pub updated_at: u64,
}

impl Node {
    /// Creates a new `Node` with default empty properties and current timestamp.
    pub fn new(id: NodeId, label: String, node_type: NodeType) -> Self {
        Self {
            id,
            label,
            node_type,
            properties: HashMap::new(),
            provenance: GraphProvenance::default(),
            updated_at: current_unix_timestamp(),
        }
    }

    /// Builder method to specify properties on the node.
    pub fn with_properties(mut self, properties: HashMap<String, serde_json::Value>) -> Self {
        self.properties = properties;
        self
    }

    /// Builder method to specify updated_at timestamp.
    pub fn with_updated_at(mut self, updated_at: u64) -> Self {
        self.updated_at = updated_at;
        self
    }

    /// Builder method to specify label on the node.
    pub fn with_label(mut self, label: String) -> Self {
        self.label = label;
        self
    }

    /// Merges another node's properties and provenance into this node.
    /// Follows the Provenance Monotonicity Rule: aggregates source context in an append-only format.
    pub fn merge_with(&mut self, other: &Self) {
        // 1. Merge properties
        for (k, v) in &other.properties {
            self.properties
                .entry(k.clone())
                .or_insert_with(|| v.clone());
        }

        // 2. Merge provenance monotonically
        let mut convs = std::collections::BTreeSet::new();
        if let Some(c) = &self.provenance.source_conversation {
            for part in c.split(',').map(|s| s.trim()).filter(|s| !s.is_empty()) {
                convs.insert(part.to_string());
            }
        }
        if let Some(c) = &other.provenance.source_conversation {
            for part in c.split(',').map(|s| s.trim()).filter(|s| !s.is_empty()) {
                convs.insert(part.to_string());
            }
        }
        self.provenance.source_conversation = if convs.is_empty() {
            None
        } else {
            Some(convs.into_iter().collect::<Vec<_>>().join(", "))
        };

        let mut msgs = std::collections::BTreeSet::new();
        if let Some(m) = &self.provenance.source_message {
            for part in m.split(',').map(|s| s.trim()).filter(|s| !s.is_empty()) {
                msgs.insert(part.to_string());
            }
        }
        if let Some(m) = &other.provenance.source_message {
            for part in m.split(',').map(|s| s.trim()).filter(|s| !s.is_empty()) {
                msgs.insert(part.to_string());
            }
        }
        self.provenance.source_message = if msgs.is_empty() {
            None
        } else {
            Some(msgs.into_iter().collect::<Vec<_>>().join(", "))
        };

        // Combine text spans
        let mut spans = Vec::new();
        if let Some(s) = &self.provenance.text_span {
            spans.push(s.clone());
        }
        if let Some(s) = &other.provenance.text_span {
            if !spans.contains(s) {
                spans.push(s.clone());
            }
        }
        self.provenance.text_span = if spans.is_empty() {
            None
        } else {
            Some(spans.join(" | "))
        };

        // Keep the oldest extraction timestamp to preserve historical provenance start time
        self.provenance.extracted_at = self
            .provenance
            .extracted_at
            .min(other.provenance.extracted_at);

        // Keep maximum confidence
        self.provenance.confidence = self.provenance.confidence.max(other.provenance.confidence);

        self.updated_at = current_unix_timestamp();
    }
}

/// Typed identifier representing an inference rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuleId {
    /// Inverse relation rule (e.g. A develops B -> B stored_in A).
    Inverse,
    /// Transitive path closure rule (e.g. A uses B uses C -> A uses C).
    Transitive,
}

/// Description of the reasoning/inference details that produced an edge.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Derivation {
    /// Typed rule identifier.
    pub rule: RuleId,
    /// Identifiers of the supporting source/inferred edges that produced this edge.
    pub supporting_edges: Vec<EdgeId>,
}

/// Represents a directed relationship edge between two nodes in the knowledge graph.
///
/// Invariants:
/// - Edge ID (source, target, relation) is strictly immutable once persisted.
/// - Provenance information is intrinsic and immutable.
#[non_exhaustive]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Edge {
    /// The source node identifier.
    pub source: NodeId,
    /// The target node identifier.
    pub target: NodeId,
    /// The relation label/kind.
    pub relation: RelationKind,
    /// The weight or confidence score of the relationship.
    pub weight: f64,
    /// Intrinsic immutable provenance of the edge.
    pub provenance: GraphProvenance,
    /// Optional explanation of how this edge was derived (None for extracted/imported/user-authored edges).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub derivation: Option<Derivation>,
    /// Unix timestamp when the edge was last updated.
    pub updated_at: u64,
}

impl Edge {
    /// Creates a new `Edge` with current timestamp.
    pub fn new(source: NodeId, target: NodeId, relation: RelationKind, weight: f64) -> Self {
        Self {
            source,
            target,
            relation,
            weight,
            provenance: GraphProvenance::default(),
            derivation: None,
            updated_at: current_unix_timestamp(),
        }
    }

    /// Creates a new derived/inferred `Edge` with a derivation history.
    pub fn new_derived(
        source: NodeId,
        target: NodeId,
        relation: RelationKind,
        weight: f64,
        derivation: Derivation,
    ) -> Self {
        Self {
            source,
            target,
            relation,
            weight,
            provenance: GraphProvenance::default(),
            derivation: Some(derivation),
            updated_at: current_unix_timestamp(),
        }
    }

    /// Strengthens the relationship weight by 0.1, capped at 1.0.
    pub fn strengthen(&mut self) -> Result<crate::events::DomainEvent, crate::errors::DomainError> {
        if !(0.0..=1.0).contains(&self.weight) {
            return Err(crate::errors::DomainError::InvalidEdgeWeight(
                self.weight.to_string(),
            ));
        }
        self.weight = (self.weight + 0.1).min(1.0);
        self.updated_at = current_unix_timestamp();
        Ok(crate::events::DomainEvent::RelationshipStrengthened {
            source: self.source.to_string(),
            target: self.target.to_string(),
            relation: self.relation.to_string(),
            new_weight: self.weight,
        })
    }

    /// Strengthens the relationship weight using a [`crate::relations::ConfidenceStrategy`] and an
    /// explicit evidence weight, capped at 1.0.
    ///
    /// This is the preferred path for runtime-driven strengthening. The reflection engine consults
    /// the ontology (`RelationDefinition.confidence_strategy`) and passes the policy and evidence
    /// weight here — keeping confidence calculations inside the domain model, not in the engine.
    ///
    /// The reflection engine must call this method. It must never assign to `edge.weight` directly.
    pub fn strengthen_with_evidence(
        &mut self,
        new_evidence_weight: f64,
        strategy: crate::relations::ConfidenceStrategy,
    ) -> Result<crate::events::DomainEvent, crate::errors::DomainError> {
        if !(0.0..=1.0).contains(&self.weight) {
            return Err(crate::errors::DomainError::InvalidEdgeWeight(
                self.weight.to_string(),
            ));
        }
        if !(0.0..=1.0).contains(&new_evidence_weight) {
            return Err(crate::errors::DomainError::InvalidEdgeWeight(format!(
                "new_evidence_weight={}",
                new_evidence_weight
            )));
        }
        self.weight = strategy.combine(self.weight, new_evidence_weight).min(1.0);
        self.updated_at = current_unix_timestamp();
        Ok(crate::events::DomainEvent::RelationshipStrengthened {
            source: self.source.to_string(),
            target: self.target.to_string(),
            relation: self.relation.to_string(),
            new_weight: self.weight,
        })
    }

    /// Decays the relationship weight exponentially.
    pub fn decay(
        &mut self,
        half_life_secs: f64,
        delta_t_secs: f64,
    ) -> Result<(), crate::errors::DomainError> {
        if half_life_secs <= 0.0 {
            return Err(crate::errors::DomainError::InvalidEdgeWeight(format!(
                "half_life_secs={}",
                half_life_secs
            )));
        }
        if delta_t_secs < 0.0 {
            return Err(crate::errors::DomainError::InvalidEdgeWeight(format!(
                "delta_t_secs={}",
                delta_t_secs
            )));
        }
        let lambda = 2.0f64.ln() / half_life_secs;
        self.weight *= (-lambda * delta_t_secs).exp();
        self.updated_at = current_unix_timestamp();
        Ok(())
    }
}

/// Represents a high-dimensional vector embedding associated with a graph node.
#[non_exhaustive]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Embedding {
    /// The node identifier associated with this embedding.
    pub node_id: NodeId,
    /// The vector representation.
    pub vector: Vec<f32>,
    /// The length / dimensions of the vector.
    pub dimension: usize,
}

impl Embedding {
    /// Creates a new `Embedding` and calculates its dimension.
    pub fn new(node_id: NodeId, vector: Vec<f32>) -> Self {
        let dimension = vector.len();
        Self {
            node_id,
            vector,
            dimension,
        }
    }
}

/// Represents an individual chat message in a conversation thread.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Message {
    /// The unique identifier of the message.
    pub id: MessageId,
    /// The role of the sender ("user" | "assistant" | "system").
    pub role: MessageRole,
    /// The text content of the message.
    pub content: String,
    /// Unix timestamp when the message was created.
    pub timestamp: u64,
}

impl Message {
    /// Creates a new `Message` with the current timestamp.
    pub fn new(id: MessageId, role: MessageRole, content: String) -> Self {
        Self {
            id,
            role,
            content,
            timestamp: current_unix_timestamp(),
        }
    }
}

/// Value object representing a session goal with identity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Goal {
    /// Goal ID.
    pub id: GoalId,
    /// Goal text.
    pub text: String,
}

/// Represents a planned tool invocation request formulated by a planning agent.
#[non_exhaustive]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    /// The call identifier (correlation ID).
    pub call_id: String,
    /// The registered tool name to be invoked.
    pub tool_name: String,
    /// Input arguments for the tool execution.
    pub arguments: HashMap<String, serde_json::Value>,
}

impl ToolCall {
    /// Creates a new `ToolCall`.
    pub fn new(
        call_id: String,
        tool_name: String,
        arguments: HashMap<String, serde_json::Value>,
    ) -> Self {
        Self {
            call_id,
            tool_name,
            arguments,
        }
    }
}

/// Represents a KnowledgeGraph boundary protecting structural and referential invariants.
/// This acts as an aggregate root for in-memory graph operations.
#[non_exhaustive]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgeGraph {
    /// In-memory map of node ID to Node.
    pub nodes: HashMap<NodeId, Node>,
    /// In-memory map of Edge ID to Edge.
    pub edges: HashMap<EdgeId, Edge>,
}

impl KnowledgeGraph {
    /// Creates a new empty `KnowledgeGraph`.
    pub fn new() -> Self {
        Self {
            nodes: HashMap::new(),
            edges: HashMap::new(),
        }
    }

    /// Adds a node to the knowledge graph.
    pub fn add_node(&mut self, node: Node) {
        self.nodes.insert(node.id, node);
    }

    /// Adds a relationship edge to the knowledge graph.
    /// Validates referential integrity: the source and target nodes must exist.
    pub fn add_edge(&mut self, edge: Edge) -> Result<(), crate::errors::DomainError> {
        if !self.nodes.contains_key(&edge.source) {
            return Err(crate::errors::DomainError::MissingSourceNode(
                edge.source.to_string(),
            ));
        }
        if !self.nodes.contains_key(&edge.target) {
            return Err(crate::errors::DomainError::MissingTargetNode(
                edge.target.to_string(),
            ));
        }
        let edge_id = EdgeId::new(edge.source, edge.target, edge.relation.id());
        if self.edges.contains_key(&edge_id) {
            return Err(crate::errors::DomainError::EdgeAlreadyExists {
                source_node: edge.source.to_string(),
                target_node: edge.target.to_string(),
                relation: edge.relation.to_string(),
            });
        }
        self.edges.insert(edge_id, edge);
        Ok(())
    }

    /// Strengthens an existing edge within the graph.
    pub fn strengthen_relationship(
        &mut self,
        source: NodeId,
        target: NodeId,
        relation: crate::identifiers::RelationId,
    ) -> Result<crate::events::DomainEvent, crate::errors::DomainError> {
        let edge_id = EdgeId::new(source, target, relation);
        if let Some(edge) = self.edges.get_mut(&edge_id) {
            edge.strengthen()
        } else {
            Err(crate::errors::DomainError::MissingSourceNode(format!(
                "Edge {} -> {} [{}] not found",
                source, target, edge_id.relation
            )))
        }
    }

    /// Recursively builds the explanation chain (derivation tree) explaining why an edge exists.
    pub fn explain_edge(&self, edge_id: &EdgeId) -> Option<ExplanationChain> {
        self.explain_edge_recursive(edge_id, &mut std::collections::HashSet::new())
    }

    fn explain_edge_recursive(
        &self,
        edge_id: &EdgeId,
        visited: &mut std::collections::HashSet<EdgeId>,
    ) -> Option<ExplanationChain> {
        if visited.contains(edge_id) {
            return None; // Prevent cycles
        }
        visited.insert(edge_id.clone());

        let edge = self.edges.get(edge_id)?.clone();
        let mut supporting_chains = Vec::new();

        let rule = if let Some(ref derivation) = edge.derivation {
            for sub_id in &derivation.supporting_edges {
                if let Some(sub_chain) = self.explain_edge_recursive(sub_id, visited) {
                    supporting_chains.push(sub_chain);
                }
            }
            Some(derivation.rule)
        } else {
            None
        };

        visited.remove(edge_id);

        Some(ExplanationChain {
            edge,
            rule,
            supporting_chains,
        })
    }
}

/// Recursive reasoning chain explaining why an edge exists in the knowledge graph.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExplanationChain {
    /// The edge itself.
    pub edge: Edge,
    /// The rule that derived the edge, or None if it is a source edge (extracted, imported, or user-authored).
    pub rule: Option<RuleId>,
    /// Supporting reasoning chains for the edges that contributed to this derivation.
    pub supporting_chains: Vec<ExplanationChain>,
}

impl Default for KnowledgeGraph {
    fn default() -> Self {
        Self::new()
    }
}

/// Represents an active user session in the brain system.
/// This acts as an aggregate root for session-related data, history, and goal tracking.
#[non_exhaustive]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    /// The unique identifier of the session.
    pub id: SessionId,
    /// The title of the session.
    #[serde(default)]
    pub title: SessionTitle,
    /// Whether the session is archived.
    #[serde(default)]
    pub archived: bool,
    /// Whether the session is pinned.
    #[serde(default)]
    pub pinned: bool,
    /// The ordered list of messages in this session.
    #[serde(default)]
    pub messages: Vec<Message>,
    /// The targets / goals tracked within this session.
    #[serde(default)]
    pub goals: Vec<Goal>,
    /// Value timestamp when the session was last updated.
    #[serde(default)]
    pub updated_at: SessionTimestamp,
    /// Staged domain events.
    #[serde(skip)]
    pub staged_events: Vec<crate::events::DomainEvent>,
}

impl Session {
    /// Creates a new empty `Session` (primarily for testing and empty initializations).
    pub fn new_empty() -> Self {
        Self {
            id: SessionId::new(),
            title: SessionTitle("Empty Session".to_string()),
            archived: false,
            pinned: false,
            messages: Vec::new(),
            goals: Vec::new(),
            updated_at: SessionTimestamp(0),
            staged_events: Vec::new(),
        }
    }

    /// Creates a new `Session`.
    pub fn new(id: SessionId, title: SessionTitle, timestamp: SessionTimestamp) -> Self {
        let mut session = Self {
            id,
            title: title.clone(),
            archived: false,
            pinned: false,
            messages: Vec::new(),
            goals: Vec::new(),
            updated_at: timestamp,
            staged_events: Vec::new(),
        };
        session
            .staged_events
            .push(crate::events::DomainEvent::SessionCreated {
                session_id: id,
                title,
                created_at: timestamp,
            });
        session
    }

    /// Builder/creation helper for creating a new session from a given ID without staging events.
    /// This is useful when reconstructing the aggregate from stored state without spawning new events.
    pub fn reconstruct(
        id: SessionId,
        title: SessionTitle,
        archived: bool,
        pinned: bool,
        messages: Vec<Message>,
        goals: Vec<Goal>,
        updated_at: SessionTimestamp,
    ) -> Self {
        Self {
            id,
            title,
            archived,
            pinned,
            messages,
            goals,
            updated_at,
            staged_events: Vec::new(),
        }
    }

    /// Renames a session.
    pub fn rename(&mut self, title: SessionTitle, timestamp: SessionTimestamp) {
        self.title = title.clone();
        self.updated_at = timestamp;
        self.staged_events
            .push(crate::events::DomainEvent::SessionRenamed {
                session_id: self.id,
                title,
                updated_at: timestamp,
            });
    }

    /// Archives a session.
    pub fn archive(
        &mut self,
        timestamp: SessionTimestamp,
    ) -> Result<(), crate::errors::DomainError> {
        if self.archived {
            return Err(crate::errors::DomainError::SessionArchived(
                self.id.to_string(),
            ));
        }
        self.archived = true;
        self.updated_at = timestamp;
        self.staged_events
            .push(crate::events::DomainEvent::SessionArchived {
                session_id: self.id,
                updated_at: timestamp,
            });
        Ok(())
    }

    /// Sets the pinned status.
    pub fn set_pinned(&mut self, pinned: bool, timestamp: SessionTimestamp) {
        self.pinned = pinned;
        self.updated_at = timestamp;
        self.staged_events
            .push(crate::events::DomainEvent::SessionPinnedChanged {
                session_id: self.id,
                pinned,
                updated_at: timestamp,
            });
    }

    /// Restores an archived session.
    pub fn restore(
        &mut self,
        timestamp: SessionTimestamp,
    ) -> Result<(), crate::errors::DomainError> {
        if !self.archived {
            return Ok(());
        }
        self.archived = false;
        self.updated_at = timestamp;
        self.staged_events
            .push(crate::events::DomainEvent::SessionRestored {
                session_id: self.id,
                updated_at: timestamp,
            });
        Ok(())
    }

    /// Staged event for deletion (this is processed by the projection as a deletion policy).
    pub fn delete(&mut self) {
        self.staged_events
            .push(crate::events::DomainEvent::SessionDeleted {
                session_id: self.id,
            });
    }

    /// Adds a chat message.
    pub fn add_message(&mut self, message: Message) -> Result<(), crate::errors::DomainError> {
        if self.archived {
            return Err(crate::errors::DomainError::SessionArchived(
                self.id.to_string(),
            ));
        }
        let snapshot = MessageSnapshot {
            id: message.id,
            role: message.role,
            content: message.content.clone(),
            timestamp: MessageTimestamp(message.timestamp),
        };
        self.messages.push(message);
        self.updated_at = SessionTimestamp(current_unix_timestamp());
        self.staged_events
            .push(crate::events::DomainEvent::MessageAdded {
                session_id: self.id,
                message: snapshot,
            });
        Ok(())
    }

    /// Adds a goal to the session.
    pub fn add_goal(&mut self, goal: Goal) -> Result<(), crate::errors::DomainError> {
        let trimmed = goal.text.trim();
        if trimmed.is_empty() {
            return Err(crate::errors::DomainError::DuplicateGoal(
                "Goal cannot be empty".to_string(),
            ));
        }
        if self.goals.iter().any(|g| g.text == trimmed) {
            return Err(crate::errors::DomainError::DuplicateGoal(
                trimmed.to_string(),
            ));
        }
        self.goals.push(goal);
        self.updated_at = SessionTimestamp(current_unix_timestamp());
        Ok(())
    }

    /// Removes a goal from the session.
    pub fn remove_goal(&mut self, goal_id: &GoalId) -> Result<(), crate::errors::DomainError> {
        if let Some(pos) = self.goals.iter().position(|g| g.id == *goal_id) {
            self.goals.remove(pos);
            self.updated_at = SessionTimestamp(current_unix_timestamp());
            Ok(())
        } else {
            Err(crate::errors::DomainError::GoalNotFound(
                goal_id.to_string(),
            ))
        }
    }

    /// Drains all staged domain events.
    pub fn drain_events(&mut self) -> std::vec::Drain<'_, crate::events::DomainEvent> {
        self.staged_events.drain(..)
    }
}

/// A builder for constructing knowledge graphs while validating invariants against a RelationRegistry.
pub struct GraphBuilder<'a> {
    registry: &'a crate::relations::RelationRegistry,
    graph: KnowledgeGraph,
    canonicalizer: Option<crate::canonical::EntityCanonicalizer>,
    node_redirection: HashMap<NodeId, NodeId>,
    label_to_node: HashMap<String, NodeId>,
}

impl<'a> GraphBuilder<'a> {
    /// Creates a new `GraphBuilder` with the specified registry.
    pub fn new(registry: &'a crate::relations::RelationRegistry) -> Self {
        Self {
            registry,
            graph: KnowledgeGraph::new(),
            canonicalizer: None,
            node_redirection: HashMap::new(),
            label_to_node: HashMap::new(),
        }
    }

    /// Sets an optional EntityCanonicalizer for this builder session.
    pub fn with_canonicalizer(
        mut self,
        canonicalizer: crate::canonical::EntityCanonicalizer,
    ) -> Self {
        self.canonicalizer = Some(canonicalizer);
        self
    }

    /// Adds a node to the graph, applying lexical normalization and alias resolution.
    pub fn add_node(mut self, mut node: Node) -> Self {
        let normalized = crate::canonical::Normalizer::normalize(&node.label);

        let resolved_id = self
            .canonicalizer
            .as_ref()
            .and_then(|c| c.canonicalize(&node.label).1)
            .unwrap_or(node.id);

        let final_id = if let Some(&existing_id) = self.label_to_node.get(&normalized) {
            existing_id
        } else {
            self.label_to_node.insert(normalized.clone(), resolved_id);
            resolved_id
        };

        let orig_id = node.id;

        if let Some(existing_node) = self.graph.nodes.get_mut(&final_id) {
            let merge_policy = crate::canonical::MergePolicy::for_node_type(&node.node_type);
            match merge_policy {
                crate::canonical::MergePolicy::TakeFirst => {}
                crate::canonical::MergePolicy::MergeProperties => {
                    for (k, v) in node.properties {
                        existing_node.properties.insert(k, v);
                    }
                }
                crate::canonical::MergePolicy::SumWeight => {}
            }
        } else {
            node.id = final_id;
            node.label = normalized;
            self.graph.add_node(node);
        }

        self.node_redirection.insert(orig_id, final_id);
        self
    }

    /// Adds an edge to the graph after validating the relation against the registry.
    pub fn add_edge(
        mut self,
        source: NodeId,
        target: NodeId,
        relation: RelationKind,
        weight: f64,
    ) -> Result<Self, crate::errors::DomainError> {
        // Validate that relation exists in the registry (excluding Unknown)
        if relation == RelationKind::Unknown || !self.registry.contains_kind(relation) {
            return Err(crate::errors::DomainError::UnregisteredRelation(format!(
                "RelationKind {:?} is not registered in the RelationRegistry",
                relation
            )));
        }

        let canon_source = self
            .node_redirection
            .get(&source)
            .copied()
            .unwrap_or(source);
        let canon_target = self
            .node_redirection
            .get(&target)
            .copied()
            .unwrap_or(target);

        // Skip self-loops
        if canon_source == canon_target {
            return Ok(self);
        }

        let edge_id = EdgeId::new(canon_source, canon_target, relation.id());
        if self.graph.edges.contains_key(&edge_id) {
            if let Some(existing_edge) = self.graph.edges.get_mut(&edge_id) {
                if weight > existing_edge.weight {
                    existing_edge.weight = weight;
                }
            }
        } else {
            let edge = Edge::new(canon_source, canon_target, relation, weight);
            self.graph.add_edge(edge)?;
        }

        Ok(self)
    }

    /// Returns the built KnowledgeGraph.
    pub fn build(self) -> KnowledgeGraph {
        self.graph
    }
}

/// The kind classification of a search document.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SearchDocumentKind {
    /// Active or archived chat session.
    Session,
    /// Chat message in a session thread.
    Message,
    /// Configured goal or objective.
    Goal,
    /// Background job execution trace.
    Job,
    /// Retrieval telemetry/provenance reference.
    Retrieval,
}

/// Structured search document metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SearchMetadata {
    /// Metadata specific to a session document.
    Session {
        /// Whether the session is archived.
        archived: bool,
        /// Whether the session is pinned.
        pinned: bool,
    },
    /// Metadata specific to a message document.
    Message {
        /// Parent session identifier.
        session_id: SessionId,
        /// Sender role (e.g. user, assistant, system).
        role: MessageRole,
    },
}

/// Value object representing a single message snapshot staged in domain events.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MessageSnapshot {
    /// Message ID.
    pub id: MessageId,
    /// Sender role.
    pub role: MessageRole,
    /// Content string.
    pub content: String,
    /// Timestamp of when the message was sent.
    pub timestamp: MessageTimestamp,
}

/// Represents an immutable indexed unit stored in the full-text search projection index.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SearchDocument {
    /// Unique search document ID (e.g. session:id or message:id).
    pub id: SearchDocumentId,
    /// Kind classification.
    pub kind: SearchDocumentKind,
    /// Title field (indexed).
    pub title: String,
    /// Body field (indexed).
    pub body: String,
    /// Structured metadata (unindexed JSON).
    pub metadata: SearchMetadata,
}

impl SearchDocument {
    /// Creates a new immutable `SearchDocument`.
    pub fn new(
        id: SearchDocumentId,
        kind: SearchDocumentKind,
        title: String,
        body: String,
        metadata: SearchMetadata,
    ) -> Self {
        Self {
            id,
            kind,
            title,
            body,
            metadata,
        }
    }
}
