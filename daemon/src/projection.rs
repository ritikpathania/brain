use brain_domain::{validation::ValidationReport, KnowledgeGraph};
use duckdb::{params, Connection};

/// Relational representation of a graph Node for analytical projection.
pub struct TableNode {
    /// Unique identifier of the node.
    pub id: String,
    /// Canonical label of the node.
    pub label: String,
    /// Stringified node type/kind.
    pub node_type: String,
    /// Serialized JSON properties map.
    pub properties_json: String,
    /// Unix timestamp when the node was updated.
    pub updated_at: i64,
}

/// Relational representation of a graph Edge for analytical projection.
pub struct TableEdge {
    /// Source node identifier.
    pub source: String,
    /// Target node identifier.
    pub target: String,
    /// Edge relationship relation type.
    pub relation: String,
    /// Strength/confidence weight of the edge.
    pub weight: f64,
    /// Stringified source provenance.
    pub source_provenance: String,
    /// Unix timestamp when the edge was updated.
    pub updated_at: i64,
}

/// Relational representation of derived edge explanations.
pub struct TableDerivation {
    /// Source node identifier.
    pub source: String,
    /// Target node identifier.
    pub target: String,
    /// Edge relationship relation type.
    pub relation: String,
    /// The rule applied during derivation.
    pub rule: String,
    /// Serialized list of supporting Edge identifiers.
    pub supporting_edges_json: String,
}

/// Relational representation of a validation run snapshot.
pub struct TableValidationRun {
    /// Unique run identifier.
    pub run_id: String,
    /// Total count of errors.
    pub total_errors: i64,
    /// Total count of warnings.
    pub total_warnings: i64,
    /// Whether the graph was valid.
    pub is_valid: bool,
}

/// Relational representation of a validation diagnostic.
pub struct TableValidationDiagnostic {
    /// Run identifier referencing the validation run.
    pub run_id: String,
    /// Machine-readable code.
    pub code: String,
    /// Diagnostic severity string.
    pub severity: String,
    /// Diagnostic category string.
    pub category: String,
    /// Human-readable message.
    pub message: String,
    /// Serialized JSON array of affected elements.
    pub affected_elements_json: String,
}

/// The intermediate decoupled relational projection model of a graph.
pub struct ProjectionModel {
    /// List of projected nodes.
    pub nodes: Vec<TableNode>,
    /// List of projected edges.
    pub edges: Vec<TableEdge>,
    /// List of projected derivation links.
    pub derivations: Vec<TableDerivation>,
    /// Projected validation run details.
    pub validation_run: Option<TableValidationRun>,
    /// List of projected validation diagnostics.
    pub validation_diagnostics: Vec<TableValidationDiagnostic>,
}

/// Builder responsible for mapping domain graphs into relational projection models.
pub struct ProjectionBuilder;

impl ProjectionBuilder {
    /// Translates a KnowledgeGraph and ValidationReport into a ProjectionModel.
    pub fn build(graph: &KnowledgeGraph, report: &ValidationReport) -> ProjectionModel {
        let run_id = uuid::Uuid::new_v4().to_string();

        let mut nodes = Vec::new();
        for node in graph.nodes.values() {
            nodes.push(TableNode {
                id: node.id.to_string(),
                label: node.label.clone(),
                node_type: format!("{:?}", node.node_type),
                properties_json: serde_json::to_string(&node.properties)
                    .unwrap_or_else(|_| "{}".to_string()),
                updated_at: node.updated_at as i64,
            });
        }

        let mut edges = Vec::new();
        let mut derivations = Vec::new();
        for edge in graph.edges.values() {
            edges.push(TableEdge {
                source: edge.source.to_string(),
                target: edge.target.to_string(),
                relation: edge.relation.to_string(),
                weight: edge.weight,
                source_provenance: format!("{:?}", edge.provenance.source),
                updated_at: edge.updated_at as i64,
            });

            if let Some(ref deriv) = edge.derivation {
                let supporting_ids: Vec<String> = deriv
                    .supporting_edges
                    .iter()
                    .map(|id| format!("{}-{}-{}", id.source, id.target, id.relation))
                    .collect();

                derivations.push(TableDerivation {
                    source: edge.source.to_string(),
                    target: edge.target.to_string(),
                    relation: edge.relation.to_string(),
                    rule: format!("{:?}", deriv.rule),
                    supporting_edges_json: serde_json::to_string(&supporting_ids)
                        .unwrap_or_else(|_| "[]".to_string()),
                });
            }
        }

        let validation_run = Some(TableValidationRun {
            run_id: run_id.clone(),
            total_errors: report.summary.total_errors as i64,
            total_warnings: report.summary.total_warnings as i64,
            is_valid: report.summary.is_valid,
        });

        let mut validation_diagnostics = Vec::new();
        for diag in &report.diagnostics {
            let affected_strings: Vec<String> = diag
                .affected
                .iter()
                .map(|elem| match elem {
                    brain_domain::validation::AffectedElement::Node(n) => format!("Node({})", n),
                    brain_domain::validation::AffectedElement::Edge(e) => {
                        format!("Edge({}-{}-{})", e.source, e.target, e.relation)
                    }
                    brain_domain::validation::AffectedElement::Relation(r) => {
                        format!("Relation({})", r)
                    }
                })
                .collect();

            validation_diagnostics.push(TableValidationDiagnostic {
                run_id: run_id.clone(),
                code: diag.code.clone(),
                severity: format!("{:?}", diag.severity),
                category: format!("{:?}", diag.category),
                message: diag.message.clone(),
                affected_elements_json: serde_json::to_string(&affected_strings)
                    .unwrap_or_else(|_| "[]".to_string()),
            });
        }

        ProjectionModel {
            nodes,
            edges,
            derivations,
            validation_run,
            validation_diagnostics,
        }
    }
}

/// Writer for syncing projected models into DuckDB.
pub struct DuckDBWriter;

impl DuckDBWriter {
    /// Writes the given projection model into a DuckDB database connection.
    pub fn write(conn: &mut Connection, model: &ProjectionModel) -> Result<(), duckdb::Error> {
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS analytics_projection_metadata (
                projection_version INTEGER,
                schema_version INTEGER,
                generated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
            );
            CREATE TABLE IF NOT EXISTS analytics_nodes (
                id VARCHAR PRIMARY KEY,
                label VARCHAR,
                type VARCHAR,
                properties VARCHAR,
                updated_at BIGINT,
                exported_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
            );
            CREATE TABLE IF NOT EXISTS analytics_edges (
                source VARCHAR,
                target VARCHAR,
                relation VARCHAR,
                weight DOUBLE,
                source_provenance VARCHAR,
                updated_at BIGINT,
                exported_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                PRIMARY KEY (source, target, relation)
            );
            CREATE TABLE IF NOT EXISTS analytics_derivations (
                source VARCHAR,
                target VARCHAR,
                relation VARCHAR,
                rule VARCHAR,
                supporting_edges VARCHAR,
                PRIMARY KEY (source, target, relation)
            );
            CREATE TABLE IF NOT EXISTS analytics_validation_runs (
                run_id VARCHAR PRIMARY KEY,
                total_errors INTEGER,
                total_warnings INTEGER,
                is_valid BOOLEAN,
                run_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
            );
            CREATE TABLE IF NOT EXISTS analytics_validation_diagnostics (
                run_id VARCHAR,
                code VARCHAR,
                severity VARCHAR,
                category VARCHAR,
                message VARCHAR,
                affected_elements VARCHAR,
                PRIMARY KEY (run_id, code, affected_elements)
            );",
        )?;

        let tx = conn.transaction()?;
        {
            tx.execute(
                "INSERT INTO analytics_projection_metadata (projection_version, schema_version) VALUES (?, ?)",
                params![1, 1],
            )?;

            let mut insert_node = tx.prepare(
                "INSERT INTO analytics_nodes (id, label, type, properties, updated_at)
                 VALUES (?, ?, ?, ?, ?)
                 ON CONFLICT(id) DO UPDATE SET
                     label = excluded.label,
                     type = excluded.type,
                     properties = excluded.properties,
                     updated_at = excluded.updated_at",
            )?;

            for node in &model.nodes {
                insert_node.execute(params![
                    node.id,
                    node.label,
                    node.node_type,
                    node.properties_json,
                    node.updated_at
                ])?;
            }
        }

        {
            let mut insert_edge = tx.prepare(
                "INSERT INTO analytics_edges (source, target, relation, weight, source_provenance, updated_at)
                 VALUES (?, ?, ?, ?, ?, ?)
                 ON CONFLICT(source, target, relation) DO UPDATE SET
                     weight = excluded.weight,
                     source_provenance = excluded.source_provenance,
                     updated_at = excluded.updated_at"
            )?;

            for edge in &model.edges {
                insert_edge.execute(params![
                    edge.source,
                    edge.target,
                    edge.relation,
                    edge.weight,
                    edge.source_provenance,
                    edge.updated_at
                ])?;
            }
        }

        {
            let mut insert_deriv = tx.prepare(
                "INSERT INTO analytics_derivations (source, target, relation, rule, supporting_edges)
                 VALUES (?, ?, ?, ?, ?)
                 ON CONFLICT(source, target, relation) DO UPDATE SET
                     rule = excluded.rule,
                     supporting_edges = excluded.supporting_edges"
            )?;

            for deriv in &model.derivations {
                insert_deriv.execute(params![
                    deriv.source,
                    deriv.target,
                    deriv.relation,
                    deriv.rule,
                    deriv.supporting_edges_json
                ])?;
            }
        }

        if let Some(ref run) = model.validation_run {
            tx.execute(
                "INSERT INTO analytics_validation_runs (run_id, total_errors, total_warnings, is_valid)
                 VALUES (?, ?, ?, ?)",
                params![run.run_id, run.total_errors, run.total_warnings, run.is_valid],
            )?;

            let mut insert_diag = tx.prepare(
                "INSERT INTO analytics_validation_diagnostics (run_id, code, severity, category, message, affected_elements)
                 VALUES (?, ?, ?, ?, ?, ?)"
            )?;

            for diag in &model.validation_diagnostics {
                insert_diag.execute(params![
                    diag.run_id,
                    diag.code,
                    diag.severity,
                    diag.category,
                    diag.message,
                    diag.affected_elements_json
                ])?;
            }
        }

        tx.commit()?;
        Ok(())
    }
}

/// Query record representing aggregate counts over validation runs.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct ValidationTrendRecord {
    /// Validation execution run identifier.
    pub run_id: String,
    /// Total count of validation errors.
    pub total_errors: usize,
    /// Total count of validation warnings.
    pub total_warnings: usize,
    /// Overall validity state.
    pub is_valid: bool,
}

/// Engine executing relational SQL analytical operations over DuckDB projection tables.
pub struct DuckDBAnalyticsEngine;

impl DuckDBAnalyticsEngine {
    /// Calculates aggregate edge count statistics grouped by edge provenance.
    pub fn get_provenance_stats(
        conn: &Connection,
    ) -> Result<std::collections::HashMap<String, usize>, duckdb::Error> {
        let mut stmt = conn.prepare(
            "SELECT source_provenance, COUNT(*) FROM analytics_edges GROUP BY source_provenance",
        )?;
        let mut rows = stmt.query([])?;
        let mut stats = std::collections::HashMap::new();
        while let Some(row) = rows.next()? {
            let prov: String = row.get(0)?;
            let count: i64 = row.get(1)?;
            stats.insert(prov, count as usize);
        }
        Ok(stats)
    }

    /// Retrieves chronological historical validation run summary trends.
    pub fn get_validation_trends(
        conn: &Connection,
    ) -> Result<Vec<ValidationTrendRecord>, duckdb::Error> {
        let mut stmt = conn.prepare(
            "SELECT run_id, total_errors, total_warnings, is_valid FROM analytics_validation_runs ORDER BY run_at ASC"
        )?;
        let mut rows = stmt.query([])?;
        let mut trends = Vec::new();
        while let Some(row) = rows.next()? {
            trends.push(ValidationTrendRecord {
                run_id: row.get(0)?,
                total_errors: row.get(1).map(|v: i64| v as usize)?,
                total_warnings: row.get(2).map(|v: i64| v as usize)?,
                is_valid: row.get(3)?,
            });
        }
        Ok(trends)
    }

    /// Evaluates degree centrality over projected edges returning the top central nodes in sorted order.
    pub fn get_top_central_nodes(
        conn: &Connection,
        limit: usize,
    ) -> Result<Vec<(String, usize)>, duckdb::Error> {
        let mut stmt = conn.prepare(
            "SELECT node, COUNT(*) as degree FROM (
                SELECT source as node FROM analytics_edges
                UNION ALL
                SELECT target as node FROM analytics_edges
            ) GROUP BY node ORDER BY degree DESC, node ASC LIMIT ?",
        )?;
        let mut rows = stmt.query(params![limit as i64])?;
        let mut results = Vec::new();
        while let Some(row) = rows.next()? {
            let node: String = row.get(0)?;
            let degree: i64 = row.get(1)?;
            results.push((node, degree as usize));
        }
        Ok(results)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use brain_domain::{
        Derivation, Edge, EdgeId, KnowledgeGraph, Node, NodeId, NodeType, ProvenanceSource,
        RelationKind, RelationRegistry, RuleId,
    };

    #[test]
    fn test_projection_builder_completeness_and_duckdb_writer() {
        let mut graph = KnowledgeGraph::new();

        let node_a = NodeId::new();
        let node_b = NodeId::new();
        let node_c = NodeId::new();

        graph.add_node(Node::new(node_a, "NodeA".to_string(), NodeType::Concept));
        graph.add_node(Node::new(node_b, "NodeB".to_string(), NodeType::Concept));
        graph.add_node(Node::new(node_c, "NodeC".to_string(), NodeType::Concept));

        let edge1 = Edge::new(node_a, node_b, RelationKind::Uses, 0.9);
        let edge1_id = EdgeId::new(node_a, node_b, RelationKind::Uses.id());

        let mut edge2 = Edge::new(node_b, node_c, RelationKind::Uses, 0.8);
        edge2.provenance.source = ProvenanceSource::Inferred;
        edge2.derivation = Some(Derivation {
            rule: RuleId::Transitive,
            supporting_edges: vec![edge1_id.clone()],
        });

        graph.add_edge(edge1).unwrap();
        graph.add_edge(edge2).unwrap();

        // 1. Build projection
        let registry = RelationRegistry::default_embedded();
        let report = brain_domain::validation::GraphValidator::validate(&graph, &registry);
        let model = ProjectionBuilder::build(&graph, &report);

        // Invariant: Projection Completeness
        assert_eq!(model.nodes.len(), graph.nodes.len());
        assert_eq!(model.edges.len(), graph.edges.len());
        assert_eq!(model.derivations.len(), 1); // Only edge2 has derivation

        // 2. Write to DuckDB
        let mut conn = Connection::open_in_memory().unwrap();
        DuckDBWriter::write(&mut conn, &model).unwrap();

        // Verify Node count in DuckDB
        let mut stmt = conn
            .prepare("SELECT COUNT(*) FROM analytics_nodes")
            .unwrap();
        let mut rows = stmt.query([]).unwrap();
        let count: i64 = rows.next().unwrap().unwrap().get(0).unwrap();
        assert_eq!(count, 3);

        // Verify Edge count in DuckDB
        let mut stmt = conn
            .prepare("SELECT COUNT(*) FROM analytics_edges")
            .unwrap();
        let mut rows = stmt.query([]).unwrap();
        let count: i64 = rows.next().unwrap().unwrap().get(0).unwrap();
        assert_eq!(count, 2);

        // Verify Derivation count in DuckDB
        let mut stmt = conn
            .prepare("SELECT COUNT(*) FROM analytics_derivations")
            .unwrap();
        let mut rows = stmt.query([]).unwrap();
        let count: i64 = rows.next().unwrap().unwrap().get(0).unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn test_duckdb_analytics_queries() {
        let registry = RelationRegistry::default_embedded();
        let mut graph = KnowledgeGraph::new();

        let node_a = NodeId::new();
        let node_b = NodeId::new();
        let node_c = NodeId::new();

        graph.add_node(Node::new(node_a, "NodeA".to_string(), NodeType::Concept));
        graph.add_node(Node::new(node_b, "NodeB".to_string(), NodeType::Concept));
        graph.add_node(Node::new(node_c, "NodeC".to_string(), NodeType::Concept));

        // Create 2 edges: A -> B, B -> C
        graph
            .add_edge(Edge::new(node_a, node_b, RelationKind::Uses, 0.9))
            .unwrap();
        graph
            .add_edge(Edge::new(node_b, node_c, RelationKind::Uses, 0.8))
            .unwrap();

        let report = brain_domain::validation::GraphValidator::validate(&graph, &registry);
        let model1 = ProjectionBuilder::build(&graph, &report);

        let mut conn = Connection::open_in_memory().unwrap();

        // Write run 1
        DuckDBWriter::write(&mut conn, &model1).unwrap();

        // Verify metadata schema versioning
        let mut stmt = conn
            .prepare("SELECT projection_version, schema_version FROM analytics_projection_metadata")
            .unwrap();
        let mut rows = stmt.query([]).unwrap();
        let row = rows.next().unwrap().unwrap();
        let proj_v: i32 = row.get(0).unwrap();
        let schema_v: i32 = row.get(1).unwrap();
        assert_eq!(proj_v, 1);
        assert_eq!(schema_v, 1);

        // Write run 2 (append-only historical snapshots)
        let model2 = ProjectionBuilder::build(&graph, &report);
        DuckDBWriter::write(&mut conn, &model2).unwrap();

        // 1. Validation Trends (should have exactly 2 runs recorded, not overwritten)
        let trends = DuckDBAnalyticsEngine::get_validation_trends(&conn).unwrap();
        assert_eq!(trends.len(), 2);
        assert_eq!(trends[0].total_errors, 0);
        assert!(trends[0].is_valid);

        // 2. Provenance Stats (both edges are Extracted)
        let stats = DuckDBAnalyticsEngine::get_provenance_stats(&conn).unwrap();
        assert_eq!(stats.get("Extracted").cloned().unwrap_or(0), 2);

        // 3. Centrality Ranking (node_b degree = 2, others = 1)
        let centrality = DuckDBAnalyticsEngine::get_top_central_nodes(&conn, 3).unwrap();
        assert_eq!(centrality.len(), 3);
        assert_eq!(centrality[0].0, node_b.to_string());
        assert_eq!(centrality[0].1, 2);
    }
}
