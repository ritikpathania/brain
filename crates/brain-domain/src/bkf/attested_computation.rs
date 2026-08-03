use crate::bkf::ids::*;
use crate::bkf::provenance::Provenance;
use serde::{Deserialize, Serialize};

/// Parameter definition for an attested computation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ComputationParameter {
    /// Name of the parameter.
    pub name: String,
    /// Data type of the parameter (e.g. "string", "integer", "date").
    pub parameter_type: String,
    /// Description of the parameter purpose.
    pub description: Option<String>,
}

/// Verification / attestation rule mechanics.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AttesterRules {
    /// Deterministic script code or reference.
    pub verification_script: String,
    /// Expected result fields required in computation receipt.
    pub expected_receipt_fields: Vec<String>,
}

/// An attested computation definition (OKF v0.2 spec).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AttestedComputation {
    /// Computation unique identifier.
    pub id: BkfComputationId,
    /// Computation target runtime environment (e.g. bigquery, postgres, python).
    pub runtime: String,
    /// Sanctioned logic / code script that can never be modified.
    pub computation_logic: String,
    /// Authorized parameters allowed to be provided.
    pub parameters: Vec<ComputationParameter>,
    /// Executor config/run instructions.
    pub executor_instructions: Option<String>,
    /// Rules used to mechanically verify the result receipt.
    pub attester: AttesterRules,
    /// Lineage tracking.
    pub provenance: Vec<Provenance>,
}
