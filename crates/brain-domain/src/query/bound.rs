//! Semantic bound query representation with numerical SlotId indexing.

use crate::query::ast::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Strongly typed variable slot index offset.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct SlotId(pub usize);

/// Schema mapping variables to slot offsets with bi-directional lookup.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct BindingSchema {
    /// Map of QueryVar to SlotId offset.
    pub var_to_slot: HashMap<QueryVar, SlotId>,
    /// O(1) reverse lookup vector mapping SlotId to QueryVar.
    pub slot_to_var: Vec<QueryVar>,
}

impl BindingSchema {
    /// Creates a new empty schema.
    pub fn new() -> Self {
        Self::default()
    }

    /// Allocates or retrieves a SlotId for a QueryVar.
    pub fn get_or_create_slot(&mut self, var: &QueryVar) -> SlotId {
        if let Some(&slot) = self.var_to_slot.get(var) {
            slot
        } else {
            let slot = SlotId(self.slot_to_var.len());
            self.var_to_slot.insert(var.clone(), slot);
            self.slot_to_var.push(var.clone());
            slot
        }
    }

    /// Looks up QueryVar for a given SlotId.
    pub fn get_var(&self, slot: SlotId) -> Option<&QueryVar> {
        self.slot_to_var.get(slot.0)
    }

    /// Returns total number of allocated variable slots.
    pub fn slot_count(&self) -> usize {
        self.slot_to_var.len()
    }
}

/// Validated bound query with scope resolution and slot mappings.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BoundQuery {
    /// Inner AST.
    pub ast: Query,
    /// Schema mapping variables to slot IDs.
    pub schema: BindingSchema,
}
