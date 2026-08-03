//! Pure presentation view model for entity inspection overlays.

/// Stable structural identity for entity inspector sections.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EntitySectionId {
    /// Identity metadata section.
    Identity,
    /// Originating source provenance section.
    Source,
    /// Attributes and key-value metadata section.
    Metadata,
    /// Graph relationships and neighbor links section.
    Relationships,
    /// Retrieval explanation section.
    RetrievalExplanation,
    /// Recent stewardship activity section.
    ActivityFeed,
}

/// Qualitative confidence classification for retrieval explanation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RetrievalConfidence {
    /// High confidence match.
    High,
    /// Medium confidence match.
    Medium,
    /// Low confidence match.
    Low,
}

impl RetrievalConfidence {
    /// Returns user-facing confidence badge text.
    pub fn badge_text(&self) -> &'static str {
        match self {
            Self::High => "High",
            Self::Medium => "Medium",
            Self::Low => "Low",
        }
    }
}

/// Descriptive reason classification for why an entity matched during retrieval.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MatchReason {
    /// Matched via entity label / name string.
    EntityLabel,
    /// Matched via attribute or key-value metadata.
    Metadata,
    /// Matched via graph connection or relationship context.
    Relationship,
    /// Matched via vector or semantic similarity.
    SemanticSimilarity,
}

impl MatchReason {
    /// Returns user-facing label text for this match reason.
    pub fn label(&self) -> &'static str {
        match self {
            Self::EntityLabel => "Entity label",
            Self::Metadata => "Metadata",
            Self::Relationship => "Relationship",
            Self::SemanticSimilarity => "Semantic similarity",
        }
    }
}

/// First-class presentation view model for retrieval explanation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RetrievalExplanationViewModel {
    /// Qualitative confidence rating.
    pub confidence: RetrievalConfidence,
    /// Matched element reason tags.
    pub matched_elements: Vec<MatchReason>,
}

/// Structured composable entity presentation sections.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EntitySection {
    /// Identity metadata.
    Identity {
        /// Entity ID.
        id: String,
        /// Entity display name label.
        display_name: String,
        /// Entity node type classification.
        node_type: String,
    },
    /// Originating source kind and location provenance.
    Source {
        /// System or file source kind.
        kind: String,
        /// Producing subsystem / engine.
        producer: String,
        /// Source file path or URI location.
        location: String,
        /// Creation or ingestion timestamp.
        timestamp: i64,
        /// Active workspace context.
        workspace: String,
    },
    /// Attributes and key-value metadata.
    Metadata {
        /// Attribute key-value pairs.
        attributes: Vec<(String, String)>,
    },
    /// Graph connections and neighbor links.
    Relationships {
        /// Neighbor relationship connection view models.
        connections: Vec<RelationshipViewModel>,
    },
    /// First-class retrieval explanation section.
    RetrievalExplanation {
        /// Encapsulated retrieval explanation view model.
        explanation: RetrievalExplanationViewModel,
    },
    /// Stewardship activity history log feed.
    ActivityFeed {
        /// Structured activity log entries.
        entries: Vec<brain_domain::query::inspector::ActivityLogEntry>,
    },
}

impl EntitySection {
    /// Returns the stable `EntitySectionId` for this section.
    pub fn id(&self) -> EntitySectionId {
        match self {
            Self::Identity { .. } => EntitySectionId::Identity,
            Self::Source { .. } => EntitySectionId::Source,
            Self::Metadata { .. } => EntitySectionId::Metadata,
            Self::Relationships { .. } => EntitySectionId::Relationships,
            Self::RetrievalExplanation { .. } => EntitySectionId::RetrievalExplanation,
            Self::ActivityFeed { .. } => EntitySectionId::ActivityFeed,
        }
    }

    /// Returns the user-facing section header title.
    pub fn heading(&self) -> &'static str {
        match self {
            Self::Identity { .. } => "Identity",
            Self::Source { .. } => "Source & Provenance",
            Self::Metadata { .. } => "Properties & Metadata",
            Self::Relationships { .. } => "Relationships & Adjacency",
            Self::RetrievalExplanation { .. } => "Retrieval Explanation",
            Self::ActivityFeed { .. } => "Recent Stewardship Activity",
        }
    }
}

/// Presentation model for a graph relationship link.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelationshipViewModel {
    /// Target entity identifier.
    pub target_id: String,
    /// Target entity display label.
    pub target_label: String,
    /// Edge relationship type or label.
    pub relation_kind: String,
}

/// Pure presentation view model for an inspection target entity.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct InspectorViewModel {
    /// Unique entity identifier.
    pub entity_id: String,
    /// Human-friendly display label.
    pub display_name: String,
    /// Composable sections.
    pub sections: Vec<EntitySection>,
}

impl InspectorViewModel {
    /// Constructs a pure `InspectorViewModel` from raw entity model fields.
    pub fn new(entity_id: String, display_name: String, sections: Vec<EntitySection>) -> Self {
        Self {
            entity_id,
            display_name,
            sections,
        }
    }

    /// Maps a domain `InspectorModel` into an enriched presentation `InspectorViewModel`.
    pub fn from_domain(model: &brain_domain::query::inspector::InspectorModel) -> Self {
        let entity_id = model.entity.id.clone();
        let display_name = model.entity.label.clone();
        let mut sections = Vec::new();

        // 1. Identity Section
        sections.push(EntitySection::Identity {
            id: model.entity.id.clone(),
            display_name: model.entity.label.clone(),
            node_type: model.entity.node_type.to_string(),
        });

        // 2. Source & Provenance Section
        sections.push(EntitySection::Source {
            kind: model.provenance.source.clone(),
            producer: "Knowledge Graph Engine".to_string(),
            location: model.provenance.location.clone(),
            timestamp: model.provenance.timestamp as i64,
            workspace: "Default".to_string(),
        });

        // 3. Retrieval Explanation (if present)
        if let Some(ref expr) = model.retrieval_explanation {
            let confidence = if expr.score >= 0.8 {
                RetrievalConfidence::High
            } else if expr.score >= 0.5 {
                RetrievalConfidence::Medium
            } else {
                RetrievalConfidence::Low
            };

            let mut matched_elements = Vec::new();
            if !expr.keyword_boosts.is_empty() {
                matched_elements.push(MatchReason::EntityLabel);
            }
            if expr.semantic_distance > 0.0 {
                matched_elements.push(MatchReason::SemanticSimilarity);
            }
            matched_elements.push(MatchReason::Metadata);
            matched_elements.push(MatchReason::Relationship);

            sections.push(EntitySection::RetrievalExplanation {
                explanation: RetrievalExplanationViewModel {
                    confidence,
                    matched_elements,
                },
            });
        }

        // 4. Recent Stewardship Activity Feed (bounded to last 20 events)
        if !model.recent_activity.is_empty() {
            let entries = model.recent_activity.iter().take(20).cloned().collect();
            sections.push(EntitySection::ActivityFeed { entries });
        }

        // 4. Metadata Section
        let mut attributes = Vec::new();
        if let serde_json::Value::Object(map) = &model.entity.attributes {
            for (k, v) in map {
                let val_str = match v {
                    serde_json::Value::String(s) => s.clone(),
                    other => other.to_string(),
                };
                attributes.push((k.clone(), val_str));
            }
        }
        for (k, v) in &model.metadata {
            if k != "id" {
                attributes.push((k.clone(), v.clone()));
            }
        }
        sections.push(EntitySection::Metadata { attributes });

        // 5. Relationships Section
        let connections = model
            .relationships
            .iter()
            .map(|rel| RelationshipViewModel {
                target_id: rel.target_id.clone(),
                target_label: rel.target_label.clone(),
                relation_kind: rel.relation.clone(),
            })
            .collect();
        sections.push(EntitySection::Relationships { connections });

        Self {
            entity_id,
            display_name,
            sections,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_activity_feed_retention_bounding_and_chronological_ordering() {
        let mut model = brain_domain::query::inspector::InspectorModel {
            entity: brain_domain::dtos::NodeDTO::new(
                "mem_bound".to_string(),
                "Bound Test".to_string(),
                "Memory".to_string(),
                serde_json::Value::Null,
            ),
            metadata: std::collections::HashMap::new(),
            relationships: vec![],
            provenance: brain_domain::query::inspector::ProvenanceDTO {
                source: "Test Engine".to_string(),
                location: "Store".to_string(),
                timestamp: 100,
                extra_info: std::collections::HashMap::new(),
            },
            retrieval_explanation: None,
            recent_activity: vec![],
        };

        // Populate 25 activity entries
        for i in 0..25 {
            model
                .recent_activity
                .push(brain_domain::query::inspector::ActivityLogEntry {
                    timestamp: 100 + i as u64,
                    action: format!("Mutation_{}", i),
                    details: format!("Activity event detail #{}", i),
                });
        }

        let vm = InspectorViewModel::from_domain(&model);
        let activity_section = vm
            .sections
            .iter()
            .find(|s| s.id() == EntitySectionId::ActivityFeed)
            .unwrap();

        if let EntitySection::ActivityFeed { entries } = activity_section {
            // Retention is strictly bounded to the 20 most recent entries
            assert_eq!(entries.len(), 20);
            assert_eq!(entries[0].action, "Mutation_0");
            assert_eq!(entries[19].action, "Mutation_19");
        } else {
            panic!("Expected ActivityFeed section");
        }
    }
}
