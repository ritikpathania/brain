use rusqlite::Connection;
use std::sync::{Arc, Mutex};

use crate::storage::sqlite::LtmDatabase;

impl LtmDatabase {
    /// Open in-memory connection for unit testing and benchmarking
    pub fn new_in_memory() -> Result<Self, rusqlite::Error> {
        let conn = Connection::open_in_memory()?;

        conn.pragma_update(None, "foreign_keys", "ON")?;

        super::schema::initialize_schema(&conn)?;

        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }
}
