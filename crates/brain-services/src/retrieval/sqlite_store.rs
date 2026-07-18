use brain_domain::retrieval::{
    CacheStore, CompiledQueryCacheKey, LogicalPlanCacheKey, PhysicalPlanCacheKey, ResultCacheKey,
    SnapshotCacheStore, SnapshotId,
};
use rusqlite::Connection;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// Configuration settings for SQLiteStore caching.
#[derive(Debug, Clone)]
pub struct SQLiteConfig {
    /// File path to the SQLite database.
    pub path: PathBuf,
    /// Whether WAL (Write-Ahead Logging) mode is enabled.
    pub wal_enabled: bool,
    /// Connection busy timeout duration.
    pub busy_timeout: Duration,
}

/// A durable, transaction-backed caching layer using SQLite.
pub struct SQLiteStore<K, V> {
    conn: Arc<Mutex<Connection>>,
    table_name: String,
    _marker: std::marker::PhantomData<(K, V)>,
}

/// Helper trait to extract the SnapshotId from cache keys at the storage level.
pub trait ExtractSnapshotId {
    /// Extracts the SnapshotId.
    fn extract_snapshot_id(&self) -> SnapshotId;
}

impl ExtractSnapshotId for CompiledQueryCacheKey {
    fn extract_snapshot_id(&self) -> SnapshotId {
        self.snapshot_id
    }
}

impl ExtractSnapshotId for LogicalPlanCacheKey {
    fn extract_snapshot_id(&self) -> SnapshotId {
        self.snapshot_id
    }
}

impl ExtractSnapshotId for PhysicalPlanCacheKey {
    fn extract_snapshot_id(&self) -> SnapshotId {
        self.snapshot_id
    }
}

impl ExtractSnapshotId for ResultCacheKey {
    fn extract_snapshot_id(&self) -> SnapshotId {
        self.snapshot_id
    }
}

/// SQL Column Type options.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SqlType {
    /// Integer numeric values.
    Integer,
    /// UTF-8 Text strings.
    Text,
    /// Binary raw data blobs.
    Blob,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ExpectedColumn {
    name: &'static str,
    ty: SqlType,
    nullable: bool,
    primary_key_position: Option<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ExpectedIndex {
    name: String,
    unique: bool,
    columns: &'static [&'static str],
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ExpectedTable {
    name: String,
    columns: &'static [ExpectedColumn],
    indexes: Vec<ExpectedIndex>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ExpectedSchema {
    tables: Vec<ExpectedTable>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ObservedColumn {
    name: &'static str, // Static mapping to matching expected column names for comparison
    ty: SqlType,
    nullable: bool,
    primary_key_position: Option<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ObservedIndex {
    name: String,
    unique: bool,
    columns: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ObservedTable {
    name: String,
    columns: Vec<ObservedColumn>,
    indexes: Vec<ObservedIndex>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ObservedSchema {
    tables: Vec<ObservedTable>,
}

/// Schema verification and validation errors.
#[derive(Debug, thiserror::Error)]
pub enum SchemaVerificationError {
    /// Target table is missing.
    #[error("Table '{0}' is missing")]
    MissingTable(String),
    /// Target column is missing.
    #[error("Column '{col}' in table '{table}' is missing")]
    MissingColumn {
        /// Table name.
        table: String,
        /// Column name.
        col: String,
    },
    /// Column has unexpected SQLite type.
    #[error("Column '{col}' in table '{table}' expected type {expected:?}, got type {actual:?}")]
    UnexpectedColumnType {
        /// Table name.
        table: String,
        /// Column name.
        col: String,
        /// Expected type.
        expected: SqlType,
        /// Actual observed type.
        actual: SqlType,
    },
    /// Nullable property mismatch.
    #[error("Column '{col}' in table '{table}' expected nullable={expected}, got {actual}")]
    NullableMismatch {
        /// Table name.
        table: String,
        /// Column name.
        col: String,
        /// Expected nullable state.
        expected: bool,
        /// Actual observed nullable state.
        actual: bool,
    },
    /// Primary key sequence mismatch.
    #[error("Column '{col}' in table '{table}' expected PK position {expected:?}, got {actual:?}")]
    InvalidPrimaryKey {
        /// Table name.
        table: String,
        /// Column name.
        col: String,
        /// Expected PK position.
        expected: Option<u8>,
        /// Actual observed PK position.
        actual: Option<u8>,
    },
    /// Target index is missing.
    #[error("Index '{0}' is missing")]
    MissingIndex(String),
    /// Index uniqueness property mismatch.
    #[error("Index '{name}' expected unique={expected}, got {actual}")]
    IndexUniquenessMismatch {
        /// Index name.
        name: String,
        /// Expected unique constraint state.
        expected: bool,
        /// Actual observed unique state.
        actual: bool,
    },
    /// Index column configuration mismatch.
    #[error("Index '{name}' expected columns {expected:?}, got {actual:?}")]
    IndexColumnsMismatch {
        /// Index name.
        name: String,
        /// Expected columns.
        expected: Vec<String>,
        /// Actual columns.
        actual: Vec<String>,
    },
    /// Encountered column type not supported by schema definition.
    #[error("Unsupported SQL type '{sqlite_type}' for column '{column}' in table '{table}'")]
    UnsupportedSqlType {
        /// Table name.
        table: String,
        /// Column name.
        column: String,
        /// Invalid SQLite type string.
        sqlite_type: String,
    },
    /// Underlying sqlite error.
    #[error("Database error: {0}")]
    DatabaseError(#[from] rusqlite::Error),
}

fn parse_sql_type(ty_str: &str) -> Result<SqlType, String> {
    let normalized = ty_str.to_uppercase();
    if normalized.contains("INT") {
        Ok(SqlType::Integer)
    } else if normalized.contains("CHAR")
        || normalized.contains("TEXT")
        || normalized.contains("CLOB")
    {
        Ok(SqlType::Text)
    } else if normalized.contains("BLOB") {
        Ok(SqlType::Blob)
    } else {
        Err(ty_str.to_string())
    }
}

fn expected_table(table_name: &str) -> ExpectedTable {
    ExpectedTable {
        name: table_name.to_string(),
        columns: &[
            ExpectedColumn {
                name: "snapshot_id",
                ty: SqlType::Integer,
                nullable: false,
                primary_key_position: None,
            },
            ExpectedColumn {
                name: "key_hash",
                ty: SqlType::Text,
                nullable: false,
                primary_key_position: Some(1),
            },
            ExpectedColumn {
                name: "key_blob",
                ty: SqlType::Text,
                nullable: false,
                primary_key_position: Some(2),
            },
            ExpectedColumn {
                name: "value_blob",
                ty: SqlType::Text,
                nullable: false,
                primary_key_position: None,
            },
        ],
        indexes: vec![ExpectedIndex {
            name: format!("idx_{}_snapshot", table_name),
            unique: false,
            columns: &["snapshot_id"],
        }],
    }
}

fn observe_table(
    conn: &Connection,
    table_name: &str,
) -> Result<Option<ObservedTable>, SchemaVerificationError> {
    let mut stmt = conn.prepare("SELECT 1 FROM sqlite_master WHERE type='table' AND name=?")?;
    let exists = stmt.exists([table_name])?;
    if !exists {
        return Ok(None);
    }

    let mut columns = Vec::new();
    let mut col_stmt = conn.prepare(&format!("PRAGMA table_info({})", table_name))?;
    let mut rows = col_stmt.query([])?;
    while let Some(row) = rows.next()? {
        let name: String = row.get(1)?;
        let ty_str: String = row.get(2)?;
        let notnull: i32 = row.get(3)?;
        let pk: i32 = row.get(5)?;

        let ty = match parse_sql_type(&ty_str) {
            Ok(t) => t,
            Err(e) => {
                return Err(SchemaVerificationError::UnsupportedSqlType {
                    table: table_name.to_string(),
                    column: name,
                    sqlite_type: e,
                })
            }
        };

        // Standardize Column Names to static lifetimes matching expectations if equal
        let static_name = match name.as_str() {
            "snapshot_id" => "snapshot_id",
            "key_hash" => "key_hash",
            "key_blob" => "key_blob",
            "value_blob" => "value_blob",
            _ => Box::leak(name.into_boxed_str()), // Safe leak for custom test schemas
        };

        columns.push(ObservedColumn {
            name: static_name,
            ty,
            nullable: notnull == 0,
            primary_key_position: if pk > 0 { Some(pk as u8) } else { None },
        });
    }

    let mut indexes = Vec::new();
    let mut idx_stmt = conn.prepare(&format!("PRAGMA index_list({})", table_name))?;
    let mut idx_rows = idx_stmt.query([])?;
    while let Some(idx_row) = idx_rows.next()? {
        let name: String = idx_row.get(1)?;
        let unique_int: i32 = idx_row.get(2)?;

        let mut idx_cols = Vec::new();
        let mut info_stmt = conn.prepare(&format!("PRAGMA index_info({})", name))?;
        let mut info_rows = info_stmt.query([])?;
        while let Some(info_row) = info_rows.next()? {
            let col_name: Option<String> = info_row.get(2)?;
            if let Some(c) = col_name {
                idx_cols.push(c);
            }
        }

        indexes.push(ObservedIndex {
            name,
            unique: unique_int != 0,
            columns: idx_cols,
        });
    }

    columns.sort_by_key(|c| c.name);
    indexes.sort_by(|a, b| a.name.cmp(&b.name));

    Ok(Some(ObservedTable {
        name: table_name.to_string(),
        columns,
        indexes,
    }))
}

impl ObservedSchema {
    fn from_connection(
        conn: &Connection,
        table_names: &[String],
    ) -> Result<Self, SchemaVerificationError> {
        let mut tables = Vec::new();
        for table_name in table_names {
            if let Some(table) = observe_table(conn, table_name)? {
                tables.push(table);
            } else {
                return Err(SchemaVerificationError::MissingTable(table_name.clone()));
            }
        }
        Ok(Self { tables })
    }
}

fn verify_schema(
    expected: &ExpectedSchema,
    observed: &ObservedSchema,
) -> Result<(), SchemaVerificationError> {
    for expected_table in &expected.tables {
        let observed_table = observed
            .tables
            .iter()
            .find(|t| t.name == expected_table.name)
            .ok_or_else(|| SchemaVerificationError::MissingTable(expected_table.name.clone()))?;

        let mut expected_cols = expected_table.columns.to_vec();
        expected_cols.sort_by_key(|c| c.name);

        if expected_cols.len() != observed_table.columns.len() {
            for ec in &expected_cols {
                if !observed_table.columns.iter().any(|oc| oc.name == ec.name) {
                    return Err(SchemaVerificationError::MissingColumn {
                        table: expected_table.name.clone(),
                        col: ec.name.to_string(),
                    });
                }
            }
            return Err(SchemaVerificationError::MissingColumn {
                table: expected_table.name.clone(),
                col: "unknown_length_mismatch".to_string(),
            });
        }

        for (ec, oc) in expected_cols.iter().zip(observed_table.columns.iter()) {
            if ec.name != oc.name {
                return Err(SchemaVerificationError::MissingColumn {
                    table: expected_table.name.clone(),
                    col: ec.name.to_string(),
                });
            }
            if ec.ty != oc.ty {
                return Err(SchemaVerificationError::UnexpectedColumnType {
                    table: expected_table.name.clone(),
                    col: ec.name.to_string(),
                    expected: ec.ty,
                    actual: oc.ty,
                });
            }
            if ec.nullable != oc.nullable {
                return Err(SchemaVerificationError::NullableMismatch {
                    table: expected_table.name.clone(),
                    col: ec.name.to_string(),
                    expected: ec.nullable,
                    actual: oc.nullable,
                });
            }
            if ec.primary_key_position != oc.primary_key_position {
                return Err(SchemaVerificationError::InvalidPrimaryKey {
                    table: expected_table.name.clone(),
                    col: ec.name.to_string(),
                    expected: ec.primary_key_position,
                    actual: oc.primary_key_position,
                });
            }
        }

        let mut expected_indexes = expected_table.indexes.clone();
        expected_indexes.sort_by(|a, b| a.name.cmp(&b.name));

        for ei in &expected_indexes {
            let oi = observed_table
                .indexes
                .iter()
                .find(|i| i.name == ei.name)
                .ok_or_else(|| SchemaVerificationError::MissingIndex(ei.name.clone()))?;

            if ei.unique != oi.unique {
                return Err(SchemaVerificationError::IndexUniquenessMismatch {
                    name: ei.name.clone(),
                    expected: ei.unique,
                    actual: oi.unique,
                });
            }

            let expected_cols: Vec<String> = ei.columns.iter().map(|s| s.to_string()).collect();
            if expected_cols != oi.columns {
                return Err(SchemaVerificationError::IndexColumnsMismatch {
                    name: ei.name.clone(),
                    expected: expected_cols,
                    actual: oi.columns.clone(),
                });
            }
        }
    }

    Ok(())
}

fn migrate_schema(conn: &Connection, table_name: &str) -> rusqlite::Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS schema_versions (
            table_name TEXT PRIMARY KEY,
            version INTEGER NOT NULL
        );",
    )?;

    let mut current_version: i32 = conn
        .query_row(
            "SELECT version FROM schema_versions WHERE table_name = ?",
            [table_name],
            |row| row.get(0),
        )
        .unwrap_or(-1);

    if current_version == -1 {
        let table_exists = conn
            .prepare("SELECT 1 FROM sqlite_master WHERE type='table' AND name=?")?
            .exists([table_name])?;
        if table_exists {
            current_version = conn.query_row("PRAGMA user_version", [], |row| row.get(0))?;
        } else {
            current_version = 0;
        }
    }

    let mut version = current_version;

    if version < 1 {
        // Step 0 -> 1: legacy table layout without snapshot_id index
        conn.execute_batch(&format!(
            "CREATE TABLE {} (
                key_hash TEXT NOT NULL,
                key_blob TEXT NOT NULL,
                value_blob TEXT NOT NULL,
                PRIMARY KEY (key_hash, key_blob)
            );",
            table_name
        ))?;
        version = 1;
        conn.execute(
            "INSERT OR REPLACE INTO schema_versions (table_name, version) VALUES (?, ?)",
            rusqlite::params![table_name, 1],
        )?;
    }

    if version < 2 {
        // Step 1 -> 2: add snapshot_id column and index
        conn.execute_batch(&format!(
            "ALTER TABLE {} ADD COLUMN snapshot_id INTEGER NOT NULL DEFAULT 0;
            CREATE INDEX idx_{}_snapshot ON {}(snapshot_id);",
            table_name, table_name, table_name
        ))?;
        conn.execute(
            "INSERT OR REPLACE INTO schema_versions (table_name, version) VALUES (?, ?)",
            rusqlite::params![table_name, 2],
        )?;
    }

    Ok(())
}

fn fnv1a_hash(data: &str) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in data.as_bytes() {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(0x100000001b3u64);
    }
    hash
}

fn compute_key_hash(key_str: &str) -> String {
    format!("{:016x}", fnv1a_hash(key_str))
}

impl<K, V> SQLiteStore<K, V> {
    /// Creates a new SQLiteStore with config and table_name.
    pub fn new(config: SQLiteConfig, table_name: &str) -> Result<Self, SchemaVerificationError> {
        let conn = Connection::open(&config.path)?;

        conn.busy_timeout(config.busy_timeout)?;
        if config.wal_enabled {
            let _: String = conn.query_row("PRAGMA journal_mode = WAL", [], |r| r.get(0))?;
        }

        migrate_schema(&conn, table_name)?;

        let expected = ExpectedSchema {
            tables: vec![expected_table(table_name)],
        };
        let observed = ObservedSchema::from_connection(&conn, &[table_name.to_string()])?;
        verify_schema(&expected, &observed)?;

        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
            table_name: table_name.to_string(),
            _marker: std::marker::PhantomData,
        })
    }
}

impl<K, V> CacheStore<K, V> for SQLiteStore<K, V>
where
    K: serde::Serialize + serde::de::DeserializeOwned + ExtractSnapshotId + Send + Sync + 'static,
    V: serde::Serialize + serde::de::DeserializeOwned + Send + Sync + 'static,
{
    fn get(&self, key: &K) -> Option<V> {
        let key_str = serde_json::to_string(key).ok()?;
        let hash = compute_key_hash(&key_str);

        let conn = self.conn.lock().unwrap();
        let query = format!(
            "SELECT value_blob FROM {} WHERE key_hash = ? AND key_blob = ?",
            self.table_name
        );
        let val_str: String = conn
            .query_row(&query, [&hash, &key_str], |row| row.get(0))
            .ok()?;

        serde_json::from_str(&val_str).ok()
    }

    fn insert(&self, key: K, value: V) {
        let key_str = match serde_json::to_string(&key) {
            Ok(s) => s,
            Err(_) => return,
        };
        let val_str = match serde_json::to_string(&value) {
            Ok(s) => s,
            Err(_) => return,
        };
        let hash = compute_key_hash(&key_str);
        let snapshot_id = key.extract_snapshot_id().as_u64();

        let mut conn = self.conn.lock().unwrap();
        let tx = match conn.transaction() {
            Ok(t) => t,
            Err(_) => return,
        };

        let query = format!(
            "INSERT OR REPLACE INTO {} (snapshot_id, key_hash, key_blob, value_blob) VALUES (?, ?, ?, ?)",
            self.table_name
        );

        if tx
            .execute(
                &query,
                rusqlite::params![snapshot_id, hash, key_str, val_str],
            )
            .is_err()
        {
            return;
        }

        let _ = tx.commit();
    }

    fn remove(&self, key: &K) -> Option<V> {
        let key_str = serde_json::to_string(key).ok()?;
        let hash = compute_key_hash(&key_str);

        let mut conn = self.conn.lock().unwrap();
        let tx = match conn.transaction() {
            Ok(t) => t,
            Err(_) => return None,
        };

        let select_query = format!(
            "SELECT value_blob FROM {} WHERE key_hash = ? AND key_blob = ?",
            self.table_name
        );
        let val_str: String = match tx.query_row(&select_query, [&hash, &key_str], |row| row.get(0))
        {
            Ok(s) => s,
            Err(_) => return None,
        };

        let delete_query = format!(
            "DELETE FROM {} WHERE key_hash = ? AND key_blob = ?",
            self.table_name
        );
        if tx.execute(&delete_query, [&hash, &key_str]).is_err() {
            return None;
        }

        if tx.commit().is_err() {
            return None;
        }

        serde_json::from_str(&val_str).ok()
    }

    fn clear(&self) {
        let mut conn = self.conn.lock().unwrap();
        let tx = match conn.transaction() {
            Ok(t) => t,
            Err(_) => return,
        };

        let query = format!("DELETE FROM {}", self.table_name);
        if tx.execute(&query, []).is_err() {
            return;
        }

        let _ = tx.commit();
    }
}

impl<K, V> SnapshotCacheStore<K, V> for SQLiteStore<K, V>
where
    K: serde::Serialize + serde::de::DeserializeOwned + ExtractSnapshotId + Send + Sync + 'static,
    V: serde::Serialize + serde::de::DeserializeOwned + Send + Sync + 'static,
{
    fn invalidate_snapshot(&self, snapshot_id: SnapshotId) {
        let mut conn = self.conn.lock().unwrap();
        let tx = match conn.transaction() {
            Ok(t) => t,
            Err(_) => return,
        };

        let query = format!("DELETE FROM {} WHERE snapshot_id = ?", self.table_name);
        if tx.execute(&query, [snapshot_id.as_u64()]).is_err() {
            return;
        }

        let _ = tx.commit();
    }
}
