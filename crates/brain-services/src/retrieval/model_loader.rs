//! Model loader with metadata validation.
//!
//! Models are stored on disk as a [`ModelEnvelope`] — a JSON object containing
//! [`ModelMetadata`] and the raw model payload. The loader validates
//! compatibility before deserializing the inner model, so incompatible models
//! are rejected **before** inference begins.

use std::sync::Arc;
use serde::{Serialize, Deserialize};
use brain_core::errors::BrainError;
use crate::retrieval::ranking::score_ranker::ScoreRanker;
use crate::retrieval::ranking::feature_provider::FEATURE_SCHEMA_VERSION;
use crate::retrieval::eval_harness::{LambdaMartModel, LinearRanker};

// ---------------------------------------------------------------------------
// Versioning constants
// ---------------------------------------------------------------------------

/// The minimum feature schema version this build can accept.
/// Increment [`FEATURE_SCHEMA_VERSION`] in `feature_provider.rs` when the
/// schema changes; update `MIN_COMPATIBLE_FEATURE_SCHEMA_VERSION` only when
/// older models must be rejected entirely.
pub const MIN_COMPATIBLE_FEATURE_SCHEMA_VERSION: u32 = 1;

// ---------------------------------------------------------------------------
// ModelMetadata
// ---------------------------------------------------------------------------

/// Compatibility metadata embedded in every serialized model file.
///
/// Before any weights are loaded, [`ModelLoader`] validates these fields
/// against the running build's constants. An incompatible model is rejected
/// with a descriptive [`BrainError::Configuration`] before inference begins.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelMetadata {
    /// Version of the feature schema the model was trained on.
    ///
    /// Must equal [`FEATURE_SCHEMA_VERSION`] (or fall within the accepted
    /// range) for the model to be loaded.
    pub feature_schema_version: u32,

    /// Monotonically increasing model iteration version.
    ///
    /// Informational only — used for logging and operational tracing.
    pub model_version: u32,

    /// Version string of the trainer / eval harness that produced the model.
    ///
    /// Informational only — used for logging and operational tracing.
    pub trainer_version: u32,

    // -----------------------------------------------------------------------
    // Future: artifact provenance fields (deferred — no correctness impact)
    //
    // The following fields are planned additions that make it much easier to
    // investigate production behaviour months after a model was trained.
    // They should be added once the training pipeline is more stable:
    //
    //   corpus_version: String,       — hash or tag of the training corpus
    //   git_commit: String,           — VCS ref of the training codebase
    //   training_timestamp: u64,      — Unix seconds when training completed
    //
    // When added, increment `feature_schema_version` / `model_version` and
    // use `#[serde(default)]` on each field so that existing envelope files
    // deserialize without error (forwards compatibility).
    // -----------------------------------------------------------------------
}

impl ModelMetadata {
    /// Creates metadata for the current feature schema version.
    pub fn current(model_version: u32, trainer_version: u32) -> Self {
        Self {
            feature_schema_version: FEATURE_SCHEMA_VERSION,
            model_version,
            trainer_version,
        }
    }

    /// Validates that this metadata is compatible with the running build.
    ///
    /// Returns `Ok(())` when the feature schema version falls within the
    /// accepted range; otherwise returns a descriptive `BrainError`.
    pub fn validate(&self) -> Result<(), BrainError> {
        if self.feature_schema_version < MIN_COMPATIBLE_FEATURE_SCHEMA_VERSION {
            return Err(BrainError::Configuration {
                message: format!(
                    "Model feature_schema_version {} is too old (minimum accepted: {}). \
                     Re-train the model against the current feature schema.",
                    self.feature_schema_version, MIN_COMPATIBLE_FEATURE_SCHEMA_VERSION,
                ),
            });
        }
        if self.feature_schema_version > FEATURE_SCHEMA_VERSION {
            return Err(BrainError::Configuration {
                message: format!(
                    "Model feature_schema_version {} is newer than this build ({}) — \
                     upgrade brain-services or use a compatible model.",
                    self.feature_schema_version, FEATURE_SCHEMA_VERSION,
                ),
            });
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// ModelEnvelope
// ---------------------------------------------------------------------------

/// On-disk format for serialized ranking models.
///
/// ```json
/// {
///   "metadata": { "feature_schema_version": 1, "model_version": 1, "trainer_version": 1 },
///   "model": { /* LinearRanker or LambdaMartModel payload */ }
/// }
/// ```
///
/// The `model` field is stored as a raw [`serde_json::Value`] so that
/// validation and dispatch can happen before committing to a concrete type.
#[derive(Debug, Serialize, Deserialize)]
pub struct ModelEnvelope {
    /// Compatibility metadata validated before loading.
    pub metadata: ModelMetadata,
    /// Raw model payload — dispatched to the correct concrete type after
    /// metadata validation passes.
    pub model: serde_json::Value,
}

impl ModelEnvelope {
    /// Wraps a serializable model with current-version metadata.
    ///
    /// # Errors
    /// Returns an error if the model cannot be serialized to JSON.
    pub fn wrap<M: Serialize>(
        model: &M,
        model_version: u32,
        trainer_version: u32,
    ) -> Result<Self, BrainError> {
        let model_value = serde_json::to_value(model).map_err(|e| BrainError::Configuration {
            message: format!("Failed to serialize model payload: {}", e),
        })?;
        Ok(Self {
            metadata: ModelMetadata::current(model_version, trainer_version),
            model: model_value,
        })
    }
}

// ---------------------------------------------------------------------------
// ModelLoader
// ---------------------------------------------------------------------------

/// Dynamic loader for resolving `ScoreRanker` models from envelope files.
///
/// ## Loading sequence
/// 1. Parse the JSON as a [`ModelEnvelope`].
/// 2. Call [`ModelMetadata::validate`] — reject incompatible models immediately.
/// 3. Attempt to deserialize the inner `model` payload as `LambdaMartModel`,
///    then fall back to `LinearRanker`.
///
/// ## Legacy / bare-JSON fallback
/// If the JSON does not contain a `metadata` key (models written before R6),
/// the loader falls back to the old behaviour for backwards compatibility,
/// emitting a deprecation notice in the error context.
pub struct ModelLoader;

impl ModelLoader {
    /// Deserializes a model from a JSON string, validating metadata first.
    pub fn load_from_str(model_json: &str) -> Result<Arc<dyn ScoreRanker>, BrainError> {
        // Attempt envelope parsing first.
        let maybe_envelope: Result<ModelEnvelope, _> = serde_json::from_str(model_json);
        match maybe_envelope {
            Ok(envelope) => {
                // Validate compatibility before touching the weights.
                envelope.metadata.validate()?;
                Self::dispatch_model(envelope.model)
            }
            Err(_) => {
                // Legacy path: bare model JSON without an envelope.
                // Preserved for backwards compatibility with models written before R6.
                Self::load_bare_str(model_json)
            }
        }
    }

    /// Loads a `ScoreRanker` model from a JSON file.
    pub fn load_from_file(path: &str) -> Result<Arc<dyn ScoreRanker>, BrainError> {
        let content = std::fs::read_to_string(path).map_err(|e| BrainError::Configuration {
            message: format!("Failed to read model file '{}': {}", path, e),
        })?;
        Self::load_from_str(&content)
    }

    // -----------------------------------------------------------------------
    // Private helpers
    // -----------------------------------------------------------------------

    /// Dispatches a validated raw `serde_json::Value` to the correct concrete type.
    fn dispatch_model(model_value: serde_json::Value) -> Result<Arc<dyn ScoreRanker>, BrainError> {
        if let Ok(model) = serde_json::from_value::<LambdaMartModel>(model_value.clone()) {
            return Ok(Arc::new(model));
        }
        if let Ok(model) = serde_json::from_value::<LinearRanker>(model_value) {
            return Ok(Arc::new(model));
        }
        Err(BrainError::Configuration {
            message: "Model payload could not be deserialized as LambdaMartModel or LinearRanker \
                      after metadata validation passed."
                .to_string(),
        })
    }

    /// Legacy bare-JSON loading path (no envelope).
    fn load_bare_str(model_json: &str) -> Result<Arc<dyn ScoreRanker>, BrainError> {
        if let Ok(model) = serde_json::from_str::<LambdaMartModel>(model_json) {
            return Ok(Arc::new(model));
        }
        if let Ok(model) = serde_json::from_str::<LinearRanker>(model_json) {
            return Ok(Arc::new(model));
        }
        Err(BrainError::Configuration {
            message: "Failed to deserialize model as LambdaMartModel or LinearRanker \
                      (bare JSON — no metadata envelope found)."
                .to_string(),
        })
    }
}
