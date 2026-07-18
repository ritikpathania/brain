use brain_domain::bkf::*;
use brain_storage::TestStorage;
use std::collections::HashMap;

#[test]
fn test_sqlite_projection_delta_application() {
    let test_store = TestStorage::new();
    let store = test_store.storage();

    // 1. Prepare raw observation and compile
    let raw_text = r#"
        entity: SQLite [Database]
        entity: PostgreSQL [Database]
        relation: SQLite -> PostgreSQL [depends_on]
    "#;

    let obs = Observation::Conversation(ConversationObservation {
        conversation_id: "conv-123".to_string(),
        session_id: "sess-456".to_string(),
        prompt: raw_text.to_string(),
        response: None,
    });

    let obs_ir = ObservationIR::parse("obs-1".to_string(), 1700000000, obs, HashMap::new());
    let compiler = KnowledgeCompiler::new_default();
    let compile_res = compiler.compile(&obs_ir).unwrap();
    let optimizer = KnowledgeOptimizer::new_default();
    let opt_res = optimizer.optimize(compile_res.output).unwrap();
    let compiled = opt_res.output;

    // Calculate projection delta on empty old graph
    let sqlite_projection = SqliteProjection;
    let ops = sqlite_projection.calculate_delta(None, &compiled).unwrap();

    // Apply projection to database
    store.apply_kpp_ops(&ops).unwrap();

    // Verify written values using a connection from pool
    let (_id1, _id2) = {
        let conn = store.pool().get().unwrap();
        let mut stmt = conn.prepare("SELECT id, label, node_type, lifecycle, validity, version_state FROM nodes ORDER BY label").unwrap();
        let mut rows = stmt.query([]).unwrap();

        // SQLite node
        let row1 = rows.next().unwrap().unwrap();
        let id1: String = row1.get(0).unwrap();
        let label1: String = row1.get(1).unwrap();
        let nt1: String = row1.get(2).unwrap();
        let lc1: String = row1.get(3).unwrap();
        let val1: String = row1.get(4).unwrap();
        let vs1: String = row1.get(5).unwrap();

        assert_eq!(label1, "PostgreSQL");
        assert_eq!(nt1, "Database");
        assert_eq!(lc1, "Observed");
        assert_eq!(val1, "Unverified");
        assert_eq!(vs1, "Current");

        // SQLite node 2
        let row2 = rows.next().unwrap().unwrap();
        let id2: String = row2.get(0).unwrap();
        let label2: String = row2.get(1).unwrap();
        let nt2: String = row2.get(2).unwrap();

        assert_eq!(label2, "SQLite");
        assert_eq!(nt2, "Database");

        // Edge check
        let mut stmt_edges = conn
            .prepare("SELECT source, target, relation, weight, lifecycle, version_state FROM edges")
            .unwrap();
        let mut edge_rows = stmt_edges.query([]).unwrap();
        let edge_row = edge_rows.next().unwrap().unwrap();
        let src: String = edge_row.get(0).unwrap();
        let dst: String = edge_row.get(1).unwrap();
        let rel: String = edge_row.get(2).unwrap();
        let wt: f64 = edge_row.get(3).unwrap();
        let elc: String = edge_row.get(4).unwrap();
        let evs: String = edge_row.get(5).unwrap();

        assert_eq!(src, id2); // node-sqlite
        assert_eq!(dst, id1); // node-postgresql
        assert_eq!(rel, "depends_on");
        assert_eq!(wt, 1.0);
        assert_eq!(elc, "Observed");
        assert_eq!(evs, "Current");

        (id1, id2)
    };

    // 2. Perform delta update
    // Define a new graph with SQLite removed and a new node "DuckDB" added
    let raw_text_v2 = r#"
        entity: PostgreSQL [Database]
        entity: DuckDB [Database]
        relation: DuckDB -> PostgreSQL [depends_on]
    "#;

    let obs_v2 = Observation::Conversation(ConversationObservation {
        conversation_id: "conv-123".to_string(),
        session_id: "sess-456".to_string(),
        prompt: raw_text_v2.to_string(),
        response: None,
    });

    let obs_ir_v2 = ObservationIR::parse("obs-2".to_string(), 1700000001, obs_v2, HashMap::new());
    let compile_res_v2 = compiler.compile(&obs_ir_v2).unwrap();
    let opt_res_v2 = optimizer.optimize(compile_res_v2.output).unwrap();
    let compiled_v2 = opt_res_v2.output;

    // Calculate delta from compiled to compiled_v2
    let ops_v2 = sqlite_projection
        .calculate_delta(Some(&compiled), &compiled_v2)
        .unwrap();

    println!("Compiled V1 nodes: {:?}", compiled.nodes);
    println!("Compiled V2 nodes: {:?}", compiled_v2.nodes);
    println!("Ops V2: {:?}", ops_v2);

    // Apply delta update
    store.apply_kpp_ops(&ops_v2).unwrap();

    // Verify: node-sqlite should be deleted, node-duckdb inserted, and PostgreSQL remains
    {
        let conn = store.pool().get().unwrap();
        let mut stmt_v2 = conn
            .prepare("SELECT label FROM nodes ORDER BY label")
            .unwrap();
        let mut rows_v2 = stmt_v2.query([]).unwrap();

        let mut labels = Vec::new();
        while let Some(row) = rows_v2.next().unwrap() {
            let l: String = row.get(0).unwrap();
            labels.push(l);
        }
        println!("DB Node Labels in Table: {:?}", labels);

        assert_eq!(labels.len(), 2);
        assert!(labels.contains(&"DuckDB".to_string()));
        assert!(labels.contains(&"PostgreSQL".to_string()));
    }
}
