use crate::schema::{
    BrainSettings, DatabaseSettings, EmbeddingProvider, LlmProvider, Merge, ModelSettings,
    PartialBrainSettings, PartialDatabaseSettings, PartialModelSettings, PartialRetrievalSettings,
    PartialSessionSettings, RankingPolicy, RetrievalSettings, SessionSettings, PartialTemporalRankingSettings,
    PartialReflectionSettings, ReflectionSettings,
};
use brain_core::BrainError;
use std::convert::TryFrom;
use std::path::PathBuf;

/// Trait defining a configuration loader source.
/// All implementations must be thread-safe and sendable.
pub trait ConfigSource: Send + Sync {
    /// Loads a partial configuration from the source.
    fn load(&self) -> Result<PartialBrainSettings, BrainError>;
}

/// Hardcoded defaults source.
pub struct DefaultsSource;

impl ConfigSource for DefaultsSource {
    fn load(&self) -> Result<PartialBrainSettings, BrainError> {
        Ok(PartialBrainSettings {
            version: Some(1),
            plugins_directory: Some("~/.local/share/brain/plugins".to_string()),
            database: Some(PartialDatabaseSettings {
                path: Some("brain.db".to_string()),
                pool_size: Some(5),
                enable_wal: Some(true),
            }),
            models: Some(PartialModelSettings {
                active_llm_provider: Some(LlmProvider::Anthropic),
                active_embedding_provider: Some(EmbeddingProvider::Anthropic),
                embedding_dimension: Some(1536),
            }),
            sessions: Some(PartialSessionSettings {
                volatile_ttl_secs: Some(3600),
                max_sliding_window_size: Some(100),
            }),
            retrieval: Some(PartialRetrievalSettings {
                ranking_policy: Some(RankingPolicy::DefaultRrf),
                model_path: None,
                temporal_ranking: Some(PartialTemporalRankingSettings {
                    enabled: Some(false),
                    model: Some(brain_core::retrieval::DecayModel::Uniform),
                    half_life_seconds: Some(86400),
                    scaling_factor: Some(1.0),
                }),
            }),
            reflection: Some(PartialReflectionSettings {
                duplicate_confidence_threshold: Some(0.92),
                link_suggestion_confidence_threshold: Some(0.85),
                auto_approve_merges: Some(false),
            }),
        })
    }
}

/// TOML file configuration source.
pub struct TomlSource {
    path: PathBuf,
}

impl TomlSource {
    /// Creates a new `TomlSource` referencing a TOML configuration file path.
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }
}

impl ConfigSource for TomlSource {
    fn load(&self) -> Result<PartialBrainSettings, BrainError> {
        if !self.path.exists() {
            // Missing configuration files are handled as empty layers, not errors.
            return Ok(PartialBrainSettings::default());
        }

        let content =
            std::fs::read_to_string(&self.path).map_err(|e| BrainError::Configuration {
                message: format!("Failed to read config file {}: {}", self.path.display(), e),
            })?;

        let partial: PartialBrainSettings =
            toml::from_str(&content).map_err(|e| BrainError::Configuration {
                message: format!("Failed to parse TOML config {}: {}", self.path.display(), e),
            })?;

        Ok(partial)
    }
}

/// Environment variable configuration source.
pub struct EnvironmentSource;

impl ConfigSource for EnvironmentSource {
    fn load(&self) -> Result<PartialBrainSettings, BrainError> {
        let mut settings = PartialBrainSettings::default();

        // Centralized metadata variable table mapping keys to setting update closures.
        #[allow(clippy::type_complexity)]
        let mappings: &[(&str, &dyn Fn(&mut PartialBrainSettings, String))] = &[
            ("BRAIN_VERSION", &|s, v| {
                if let Ok(val) = v.parse() {
                    s.version = Some(val);
                }
            }),
            ("BRAIN_PLUGINS_DIR", &|s, v| {
                s.plugins_directory = Some(v);
            }),
            ("BRAIN_DATABASE_PATH", &|s, v| {
                let mut db = s.database.take().unwrap_or_default();
                db.path = Some(v);
                s.database = Some(db);
            }),
            ("BRAIN_DATABASE_POOL_SIZE", &|s, v| {
                if let Ok(val) = v.parse() {
                    let mut db = s.database.take().unwrap_or_default();
                    db.pool_size = Some(val);
                    s.database = Some(db);
                }
            }),
            ("BRAIN_DATABASE_ENABLE_WAL", &|s, v| {
                if let Ok(val) = v.parse() {
                    let mut db = s.database.take().unwrap_or_default();
                    db.enable_wal = Some(val);
                    s.database = Some(db);
                }
            }),
            ("BRAIN_MODEL_LLM_PROVIDER", &|s, v| {
                let mut m = s.models.take().unwrap_or_default();
                let val = match v.to_lowercase().as_str() {
                    "anthropic" => LlmProvider::Anthropic,
                    "openai" => LlmProvider::Openai,
                    "gemini" => LlmProvider::Gemini,
                    "ollama" => LlmProvider::Ollama,
                    other => LlmProvider::Custom(other.to_string()),
                };
                m.active_llm_provider = Some(val);
                s.models = Some(m);
            }),
            ("BRAIN_MODEL_EMBEDDING_PROVIDER", &|s, v| {
                let mut m = s.models.take().unwrap_or_default();
                let val = match v.to_lowercase().as_str() {
                    "anthropic" => EmbeddingProvider::Anthropic,
                    "openai" => EmbeddingProvider::Openai,
                    "gemini" => EmbeddingProvider::Gemini,
                    "ollama" => EmbeddingProvider::Ollama,
                    other => EmbeddingProvider::Custom(other.to_string()),
                };
                m.active_embedding_provider = Some(val);
                s.models = Some(m);
            }),
            ("BRAIN_MODEL_EMBEDDING_DIMENSION", &|s, v| {
                if let Ok(val) = v.parse() {
                    let mut m = s.models.take().unwrap_or_default();
                    m.embedding_dimension = Some(val);
                    s.models = Some(m);
                }
            }),
            ("BRAIN_SESSION_TTL", &|s, v| {
                if let Ok(val) = v.parse() {
                    let mut ss = s.sessions.take().unwrap_or_default();
                    ss.volatile_ttl_secs = Some(val);
                    s.sessions = Some(ss);
                }
            }),
            ("BRAIN_SESSION_WINDOW_SIZE", &|s, v| {
                if let Ok(val) = v.parse() {
                    let mut ss = s.sessions.take().unwrap_or_default();
                    ss.max_sliding_window_size = Some(val);
                    s.sessions = Some(ss);
                }
            }),
            ("BRAIN_REFLECTION_DUPLICATE_THRESHOLD", &|s, v| {
                if let Ok(val) = v.parse() {
                    let mut rf = s.reflection.take().unwrap_or_default();
                    rf.duplicate_confidence_threshold = Some(val);
                    s.reflection = Some(rf);
                }
            }),
            ("BRAIN_REFLECTION_LINK_THRESHOLD", &|s, v| {
                if let Ok(val) = v.parse() {
                    let mut rf = s.reflection.take().unwrap_or_default();
                    rf.link_suggestion_confidence_threshold = Some(val);
                    s.reflection = Some(rf);
                }
            }),
            ("BRAIN_REFLECTION_AUTO_APPROVE", &|s, v| {
                if let Ok(val) = v.parse() {
                    let mut rf = s.reflection.take().unwrap_or_default();
                    rf.auto_approve_merges = Some(val);
                    s.reflection = Some(rf);
                }
            }),
        ];

        for &(key, apply_fn) in mappings {
            if let Ok(val) = std::env::var(key) {
                apply_fn(&mut settings, val);
            }
        }

        Ok(settings)
    }
}

/// In-memory override configuration source.
pub struct OverrideSource {
    settings: PartialBrainSettings,
}

impl OverrideSource {
    /// Creates a new `OverrideSource` wrapping a custom `PartialBrainSettings`.
    pub fn new(settings: PartialBrainSettings) -> Self {
        Self { settings }
    }
}

impl ConfigSource for OverrideSource {
    fn load(&self) -> Result<PartialBrainSettings, BrainError> {
        Ok(self.settings.clone())
    }
}

impl TryFrom<PartialBrainSettings> for BrainSettings {
    type Error = BrainError;

    fn try_from(partial: PartialBrainSettings) -> Result<Self, Self::Error> {
        // Structural Validation (checks for required fields and layouts)
        let version = partial.version.ok_or_else(|| BrainError::Configuration {
            message: "version is missing".to_string(),
        })?;
        let plugins_directory =
            partial
                .plugins_directory
                .ok_or_else(|| BrainError::Configuration {
                    message: "plugins_directory is missing".to_string(),
                })?;

        let db = partial.database.ok_or_else(|| BrainError::Configuration {
            message: "database section is missing".to_string(),
        })?;
        let database = DatabaseSettings {
            path: db.path.ok_or_else(|| BrainError::Configuration {
                message: "database.path is missing".to_string(),
            })?,
            pool_size: db.pool_size.ok_or_else(|| BrainError::Configuration {
                message: "database.pool_size is missing".to_string(),
            })?,
            enable_wal: db.enable_wal.ok_or_else(|| BrainError::Configuration {
                message: "database.enable_wal is missing".to_string(),
            })?,
        };

        let md = partial.models.ok_or_else(|| BrainError::Configuration {
            message: "models section is missing".to_string(),
        })?;
        let models = ModelSettings {
            active_llm_provider: md.active_llm_provider.ok_or_else(|| {
                BrainError::Configuration {
                    message: "models.active_llm_provider is missing".to_string(),
                }
            })?,
            active_embedding_provider: md.active_embedding_provider.ok_or_else(|| {
                BrainError::Configuration {
                    message: "models.active_embedding_provider is missing".to_string(),
                }
            })?,
            embedding_dimension: md.embedding_dimension.ok_or_else(|| {
                BrainError::Configuration {
                    message: "models.embedding_dimension is missing".to_string(),
                }
            })?,
        };

        let ss = partial.sessions.ok_or_else(|| BrainError::Configuration {
            message: "sessions section is missing".to_string(),
        })?;
        let sessions = SessionSettings {
            volatile_ttl_secs: ss
                .volatile_ttl_secs
                .ok_or_else(|| BrainError::Configuration {
                    message: "sessions.volatile_ttl_secs is missing".to_string(),
                })?,
            max_sliding_window_size: ss.max_sliding_window_size.ok_or_else(|| {
                BrainError::Configuration {
                    message: "sessions.max_sliding_window_size is missing".to_string(),
                }
            })?,
        };

        let ret = partial.retrieval.ok_or_else(|| BrainError::Configuration {
            message: "retrieval section is missing".to_string(),
        })?;
        let temp_opt = ret.temporal_ranking.unwrap_or_default();
        let temporal_ranking = brain_core::retrieval::TemporalRankingSettings {
            enabled: temp_opt.enabled.unwrap_or(false),
            model: temp_opt.model.unwrap_or(brain_core::retrieval::DecayModel::Uniform),
            half_life_seconds: temp_opt.half_life_seconds.unwrap_or(86400),
            scaling_factor: temp_opt.scaling_factor.unwrap_or(1.0),
        };
        let retrieval = RetrievalSettings {
            ranking_policy: ret
                .ranking_policy
                .ok_or_else(|| BrainError::Configuration {
                    message: "retrieval.ranking_policy is missing".to_string(),
                })?,
            model_path: ret.model_path,
            temporal_ranking,
        };

        let ref_partial = partial.reflection.unwrap_or_default();
        let reflection = ReflectionSettings {
            duplicate_confidence_threshold: ref_partial.duplicate_confidence_threshold.unwrap_or(0.92),
            link_suggestion_confidence_threshold: ref_partial.link_suggestion_confidence_threshold.unwrap_or(0.85),
            auto_approve_merges: ref_partial.auto_approve_merges.unwrap_or(false),
        };

        Ok(BrainSettings {
            version,
            database,
            models,
            sessions,
            retrieval,
            reflection,
            plugins_directory,
        })
    }
}

/// Resolves a final configuration from an ordered list of ConfigSource trait objects.
/// Runs structural validation during conversion, then semantic validation.
pub fn resolve(sources: &[Box<dyn ConfigSource>]) -> Result<BrainSettings, BrainError> {
    let mut merged = PartialBrainSettings::default();
    for source in sources {
        let partial = source.load()?;
        merged.merge(partial);
    }
    let final_settings = BrainSettings::try_from(merged)?;
    crate::validation::validate(&final_settings)?;
    Ok(final_settings)
}
