use rusqlite::{params, Error};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::retrieval::fuzzy::tokenize;
use crate::storage::sqlite::LtmDatabase;
use crate::storage::{ExtractedEdge, ExtractedNode};

impl LtmDatabase {
    /// Transactionally upsert extracted semantic nodes and relationship edges.
    pub fn upsert_nodes_and_edges(
        &self,
        nodes: &[ExtractedNode],
        edges: &[ExtractedEdge],
    ) -> Result<(), Error> {
        let mut conn_guard = self.conn.lock().unwrap();
        let tx = conn_guard.transaction()?;

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        // 1. Transactionally Upsert all Nodes first
        {
            let mut select_stmt = tx.prepare("SELECT type, properties FROM nodes WHERE id = ?1")?;
            let mut update_stmt = tx.prepare(
                "UPDATE nodes SET label = ?1, type = ?2, properties = ?3, updated_at = ?4 WHERE id = ?5"
            )?;
            let mut insert_stmt = tx.prepare(
                "INSERT INTO nodes (id, label, type, properties, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
            )?;

            for node in nodes {
                let mut rows = select_stmt.query(params![node.id])?;
                if let Some(row) = rows.next()? {
                    let existing_type: String = row.get(0)?;
                    let existing_properties_str: String = row.get(1)?;

                    let mut merged_props =
                        match serde_json::from_str::<serde_json::Value>(&existing_properties_str) {
                            Ok(serde_json::Value::Object(map)) => map,
                            _ => serde_json::Map::new(),
                        };

                    if let serde_json::Value::Object(incoming_map) = &node.attributes {
                        for (k, v) in incoming_map {
                            merged_props.insert(k.clone(), v.clone());
                        }
                    }

                    let final_type = if existing_type == "stub" {
                        &node.node_type
                    } else {
                        &existing_type
                    };

                    let props_str =
                        serde_json::to_string(&merged_props).unwrap_or_else(|_| "{}".to_string());
                    update_stmt
                        .execute(params![node.label, final_type, props_str, now, node.id])?;
                } else {
                    let props_str = serde_json::to_string(&node.attributes)
                        .unwrap_or_else(|_| "{}".to_string());
                    insert_stmt.execute(params![
                        node.id,
                        node.label,
                        node.node_type,
                        props_str,
                        now
                    ])?;
                }
            }
        }

        // 2. Transactionally Upsert all Edges second, inserting stubs if nodes are missing
        {
            let mut stmt = tx.prepare(
                "INSERT INTO edges (source, target, relation, weight, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5)
                 ON CONFLICT(source, target, relation) DO UPDATE SET
                    weight = excluded.weight,
                    updated_at = excluded.updated_at",
            )?;

            for edge in edges {
                // Ensure foreign key references exist (insert stub nodes if references are omitted in this batch)
                tx.execute(
                    "INSERT OR IGNORE INTO nodes (id, label, type, properties, updated_at)
                     VALUES (?1, ?1, 'stub', '{}', ?2)",
                    params![edge.source, now],
                )?;
                tx.execute(
                    "INSERT OR IGNORE INTO nodes (id, label, type, properties, updated_at)
                     VALUES (?1, ?1, 'stub', '{}', ?2)",
                    params![edge.target, now],
                )?;

                // Query existing edge for decay calculation
                let mut existing_query = tx.prepare(
                    "SELECT weight, updated_at FROM edges WHERE source = ?1 AND target = ?2 AND relation = ?3"
                )?;
                let mut rows =
                    existing_query.query(params![edge.source, edge.target, edge.relation])?;
                let (weight, last_updated) = if let Some(row) = rows.next()? {
                    (row.get::<_, f64>(0)?, row.get::<_, i64>(1)?)
                } else {
                    (1.0, now as i64)
                };

                let elapsed = (now as i64 - last_updated) as f64;
                let decay_const = std::f64::consts::LN_2 / 604800.0; // 7 days half-life default
                let decayed_weight = weight * (-decay_const * elapsed).exp();
                let new_weight = (decayed_weight + 0.5).min(2.0);

                stmt.execute(params![
                    edge.source,
                    edge.target,
                    edge.relation,
                    new_weight,
                    now
                ])?;
            }
        }

        tx.commit()?;
        Ok(())
    }

    /// Query long-term memory for nodes matching a keyword (case-insensitive)
    /// and retrieve their 1-hop adjacency list.
    pub fn query_ltm(
        &self,
        keyword: &str,
    ) -> Result<Vec<(ExtractedNode, Vec<ExtractedEdge>)>, Error> {
        let conn_guard = self.conn.lock().unwrap();

        let tokens = tokenize(keyword);
        if tokens.is_empty() {
            return Ok(Vec::new());
        }

        // Build dynamic query matching all tokens
        let mut query_str = "SELECT id, label, type, properties FROM nodes WHERE ".to_string();
        let mut clauses = Vec::new();
        for i in 0..tokens.len() {
            clauses.push(format!(
                "(LOWER(id) LIKE ?{} OR LOWER(label) LIKE ?{} OR LOWER(properties) LIKE ?{})",
                i + 1,
                i + 1,
                i + 1
            ));
        }
        query_str.push_str(&clauses.join(" AND "));

        let mut stmt = conn_guard.prepare(&query_str)?;

        let params_vec: Vec<String> = tokens.iter().map(|t| format!("%{}%", t)).collect();

        let node_iter = stmt.query_map(rusqlite::params_from_iter(params_vec.iter()), |row| {
            let props_str: String = row.get(3)?;
            let attributes = serde_json::from_str(&props_str)
                .unwrap_or_else(|_| serde_json::Value::Object(serde_json::Map::new()));

            Ok(ExtractedNode {
                id: row.get(0)?,
                label: row.get(1)?,
                node_type: row.get(2)?,
                attributes,
            })
        })?;

        let mut matched_subgraphs = Vec::new();

        for node_res in node_iter {
            let node = node_res?;

            // Query outbound and inbound relationship edges (1-hop connections)
            let mut edge_stmt = conn_guard.prepare(
                "SELECT source, target, relation FROM edges 
                 WHERE source = ?1 OR target = ?1",
            )?;

            let edge_iter = edge_stmt.query_map(params![node.id], |row| {
                Ok(ExtractedEdge {
                    source: row.get(0)?,
                    target: row.get(1)?,
                    relation: row.get(2)?,
                })
            })?;

            let mut neighbors = Vec::new();
            for edge in edge_iter {
                neighbors.push(edge?);
            }

            matched_subgraphs.push((node, neighbors));
        }

        Ok(matched_subgraphs)
    }

    pub fn decay_relationships(&self, half_life_secs: f64, threshold: f64) -> Result<(), Error> {
        let mut conn_guard = self.conn.lock().unwrap();
        let tx = conn_guard.transaction()?;
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

        let decay_const = std::f64::consts::LN_2 / half_life_secs;

        let mut updates = Vec::new();
        let mut deletes = Vec::new();

        {
            let mut stmt =
                tx.prepare("SELECT source, target, relation, weight, updated_at FROM edges")?;
            let edge_iter = stmt.query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, f64>(3)?,
                    row.get::<_, i64>(4)?,
                ))
            })?;

            for edge_res in edge_iter {
                let (source, target, relation, weight, updated_at) = edge_res?;
                let delta_t = (now - updated_at) as f64;
                if delta_t <= 0.0 {
                    continue;
                }
                let new_weight = weight * (-decay_const * delta_t).exp();
                if new_weight < threshold {
                    deletes.push((source, target, relation));
                } else {
                    updates.push((new_weight, now, source, target, relation));
                }
            }
        }

        {
            let mut update_stmt = tx.prepare(
                "UPDATE edges SET weight = ?1, updated_at = ?2 WHERE source = ?3 AND target = ?4 AND relation = ?5"
            )?;
            for (w, ts, src, tgt, rel) in updates {
                update_stmt.execute(params![w, ts, src, tgt, rel])?;
            }
        }

        {
            let mut delete_stmt = tx
                .prepare("DELETE FROM edges WHERE source = ?1 AND target = ?2 AND relation = ?3")?;
            for (src, tgt, rel) in deletes {
                delete_stmt.execute(params![src, tgt, rel])?;
            }
        }

        tx.commit()?;
        Ok(())
    }
}
