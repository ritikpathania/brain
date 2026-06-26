use crate::schema::BrainSettings;
use brain_core::BrainError;

/// Performs semantic validation checks on the resolved final BrainSettings.
pub fn validate(settings: &BrainSettings) -> Result<(), BrainError> {
    if settings.database.pool_size == 0 {
        return Err(BrainError::Configuration {
            message: "database.pool_size must be greater than 0".to_string(),
        });
    }

    if settings.sessions.volatile_ttl_secs == 0 {
        return Err(BrainError::Configuration {
            message: "sessions.volatile_ttl_secs must be at least 1 second".to_string(),
        });
    }

    if settings.models.embedding_dimension == 0 {
        return Err(BrainError::Configuration {
            message: "models.embedding_dimension must be greater than 0".to_string(),
        });
    }

    Ok(())
}
