use brain_core::BrainError;

/// Trait defining a configuration schema migration.
/// Handles migrations on TOML representations directly to preserve structure.
pub trait ConfigMigration: Send + Sync {
    /// Returns the target schema format version of this migration.
    fn version(&self) -> u32;

    /// Migrates configuration values from a previous schema version representation.
    ///
    /// Note: To preserve forward compatibility and keep unknown fields across
    /// migration steps, implementations should deserialize fields into mapping structures
    /// and merge them back rather than discarding unrecognized keys.
    fn migrate(&self, config: toml::Value) -> Result<toml::Value, BrainError>;
}
