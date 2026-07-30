//! Opaque vectorized batch structure for binding rows.

use brain_domain::query::*;

/// In-memory vectorized batch container for binding rows.
#[derive(Debug, Clone)]
pub struct BindingBatch {
    capacity: usize,
    rows: Vec<BindingRow>,
}

impl BindingBatch {
    /// Creates a new BindingBatch with capacity.
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity,
            rows: Vec::with_capacity(capacity),
        }
    }

    /// Appends a row to the batch.
    pub fn append(&mut self, row: BindingRow) {
        if self.rows.len() < self.capacity {
            self.rows.push(row);
        }
    }

    /// Clears all rows in batch.
    pub fn clear(&mut self) {
        self.rows.clear();
    }

    /// Truncates batch length.
    pub fn truncate(&mut self, len: usize) {
        self.rows.truncate(len);
    }

    /// Returns row count.
    pub fn len(&self) -> usize {
        self.rows.len()
    }

    /// Returns true if batch is empty.
    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    /// Returns capacity.
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Returns reference to internal binding rows slice.
    pub fn rows(&self) -> &[BindingRow] {
        &self.rows
    }
}
