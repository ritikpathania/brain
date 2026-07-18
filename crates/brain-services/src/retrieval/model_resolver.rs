use brain_core::errors::BrainError;
use brain_domain::retrieval::models::{DecisionTreeDefinition, DecisionTreeRankingModel};
use brain_domain::retrieval::models::{
    LinearRankingModel, RankingModel, RankingModelVersion, WeightSnapshot,
};

/// Service converting snapshots into executable ranking models.
pub struct ModelDeserializer;

impl ModelDeserializer {
    /// Deserializes a model loudly or returns a custom error.
    pub fn resolve(snapshot: &WeightSnapshot) -> Result<Box<dyn RankingModel>, BrainError> {
        let version = snapshot
            .metadata
            .calibration_metadata
            .model_version()
            .unwrap_or(RankingModelVersion::V1Linear);

        match version {
            RankingModelVersion::V2DecisionTree => {
                let json_str = snapshot
                    .metadata
                    .calibration_metadata
                    .parameters()
                    .ok_or_else(|| BrainError::Internal {
                        message: "Missing parameters for V2DecisionTree model".to_string(),
                    })?;
                let def =
                    serde_json::from_str::<DecisionTreeDefinition>(json_str).map_err(|e| {
                        BrainError::Internal {
                            message: format!("DecisionTree parsing failed: {:?}", e),
                        }
                    })?;
                Ok(Box::new(DecisionTreeRankingModel::new(def)))
            }
            RankingModelVersion::V1Linear => {
                Ok(Box::new(LinearRankingModel::new(snapshot.weights.clone())))
            }
        }
    }
}
