use serde::{Deserialize, Serialize};

/// Strongly-typed classification for active LLM providers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LlmProvider {
    /// Anthropic Claude API.
    Anthropic,
    /// OpenAI GPT API.
    Openai,
    /// Google Gemini API.
    Gemini,
    /// Local Ollama service.
    Ollama,
    /// An extensible custom provider.
    #[serde(untagged)]
    Custom(String),
}

/// Strongly-typed classification for active semantic text embedding agents.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EmbeddingProvider {
    /// Anthropic Claude embeddings.
    Anthropic,
    /// OpenAI embeddings.
    Openai,
    /// Google Gemini embeddings.
    Gemini,
    /// Local Ollama service embeddings.
    Ollama,
    /// An extensible custom provider.
    #[serde(untagged)]
    Custom(String),
}

/// Trait defining combining/layering partial configuration fragments.
pub trait Merge {
    /// Merges fields from another partial struct into self, prioritizing the other struct's values.
    fn merge(&mut self, other: Self);
}

/// Database settings configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseSettings {
    /// Local file path for the SQLite database.
    pub(crate) path: String,
    /// Maximum connection pool size.
    pub(crate) pool_size: u32,
    /// Toggle WAL (Write-Ahead Logging) database mode.
    pub(crate) enable_wal: bool,
}

impl DatabaseSettings {
    /// Returns the local file path for the SQLite database.
    pub fn path(&self) -> &str {
        &self.path
    }

    /// Returns the maximum connection pool size.
    pub fn pool_size(&self) -> u32 {
        self.pool_size
    }

    /// Returns whether WAL mode is enabled.
    pub fn enable_wal(&self) -> bool {
        self.enable_wal
    }
}

/// Partial database settings where all fields are optional.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct PartialDatabaseSettings {
    /// Optional local file path for SQLite.
    pub path: Option<String>,
    /// Optional connection pool size.
    pub pool_size: Option<u32>,
    /// Optional WAL mode toggle.
    pub enable_wal: Option<bool>,
}

impl Merge for PartialDatabaseSettings {
    fn merge(&mut self, other: Self) {
        if let Some(path) = other.path {
            self.path = Some(path);
        }
        if let Some(pool_size) = other.pool_size {
            self.pool_size = Some(pool_size);
        }
        if let Some(enable_wal) = other.enable_wal {
            self.enable_wal = Some(enable_wal);
        }
    }
}

/// AI Model settings configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelSettings {
    /// Active LLM provider enum.
    pub(crate) active_llm_provider: LlmProvider,
    /// Active text embedding provider enum.
    pub(crate) active_embedding_provider: EmbeddingProvider,
    /// Dimensions of the generated vectors.
    pub(crate) embedding_dimension: usize,
}

impl ModelSettings {
    /// Returns the active LLM provider.
    pub fn active_llm_provider(&self) -> &LlmProvider {
        &self.active_llm_provider
    }

    /// Returns the active text embedding provider.
    pub fn active_embedding_provider(&self) -> &EmbeddingProvider {
        &self.active_embedding_provider
    }

    /// Returns the dimension of the generated vectors.
    pub fn embedding_dimension(&self) -> usize {
        self.embedding_dimension
    }
}

/// Partial AI Model settings where all fields are optional.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct PartialModelSettings {
    /// Optional active LLM provider.
    pub active_llm_provider: Option<LlmProvider>,
    /// Optional active embedding provider.
    pub active_embedding_provider: Option<EmbeddingProvider>,
    /// Optional embedding dimensions.
    pub embedding_dimension: Option<usize>,
}

impl Merge for PartialModelSettings {
    fn merge(&mut self, other: Self) {
        if let Some(llm) = other.active_llm_provider {
            self.active_llm_provider = Some(llm);
        }
        if let Some(emb) = other.active_embedding_provider {
            self.active_embedding_provider = Some(emb);
        }
        if let Some(dim) = other.embedding_dimension {
            self.embedding_dimension = Some(dim);
        }
    }
}

/// Session settings configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionSettings {
    /// Short-term memory (STM) TTL in seconds.
    pub(crate) volatile_ttl_secs: u64,
    /// Maximum size of the sliding context window.
    pub(crate) max_sliding_window_size: usize,
}

impl SessionSettings {
    /// Returns the volatile TTL in seconds.
    pub fn volatile_ttl_secs(&self) -> u64 {
        self.volatile_ttl_secs
    }

    /// Returns the maximum size of the sliding context window.
    pub fn max_sliding_window_size(&self) -> usize {
        self.max_sliding_window_size
    }
}

/// Partial session settings where all fields are optional.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct PartialSessionSettings {
    /// Optional STM volatile TTL in seconds.
    pub volatile_ttl_secs: Option<u64>,
    /// Optional sliding window context size.
    pub max_sliding_window_size: Option<usize>,
}

impl Merge for PartialSessionSettings {
    fn merge(&mut self, other: Self) {
        if let Some(ttl) = other.volatile_ttl_secs {
            self.volatile_ttl_secs = Some(ttl);
        }
        if let Some(w) = other.max_sliding_window_size {
            self.max_sliding_window_size = Some(w);
        }
    }
}

/// Contextual ranking policy settings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RankingPolicy {
    /// Default Reciprocal Rank Fusion of BM25, Semantic, and Graph.
    DefaultRrf,
    /// Learned scoring models (Linear or LambdaMART) resolved via ModelLoader.
    LearnedModel,
}

/// Retrieval settings configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetrievalSettings {
    /// The active ranking policy to execute.
    pub(crate) ranking_policy: RankingPolicy,
    /// Path to the serialized model file.
    pub(crate) model_path: Option<String>,
}

impl RetrievalSettings {
    /// Creates a new RetrievalSettings.
    pub fn new(ranking_policy: RankingPolicy, model_path: Option<String>) -> Self {
        Self { ranking_policy, model_path }
    }

    /// Returns the active ranking policy.
    pub fn ranking_policy(&self) -> RankingPolicy {
        self.ranking_policy
    }

    /// Returns the path to the serialized model file, if configured.
    pub fn model_path(&self) -> Option<&str> {
        self.model_path.as_deref()
    }
}

/// Partial retrieval settings.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct PartialRetrievalSettings {
    /// Optional ranking policy.
    pub ranking_policy: Option<RankingPolicy>,
    /// Optional model path.
    pub model_path: Option<String>,
}

impl Merge for PartialRetrievalSettings {
    fn merge(&mut self, other: Self) {
        if let Some(policy) = other.ranking_policy {
            self.ranking_policy = Some(policy);
        }
        if let Some(path) = other.model_path {
            self.model_path = Some(path);
        }
    }
}

/// Root application configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrainSettings {
    /// Schema format version.
    pub(crate) version: u32,
    /// Nested database settings.
    pub(crate) database: DatabaseSettings,
    /// Nested AI model settings.
    pub(crate) models: ModelSettings,
    /// Nested session settings.
    pub(crate) sessions: SessionSettings,
    /// Nested retrieval settings.
    pub(crate) retrieval: RetrievalSettings,
    /// Plugins installation directory path.
    pub(crate) plugins_directory: String,
}

impl BrainSettings {
    /// Returns the schema format version.
    pub fn version(&self) -> u32 {
        self.version
    }

    /// Returns the nested database settings.
    pub fn database(&self) -> &DatabaseSettings {
        &self.database
    }

    /// Returns the nested AI model settings.
    pub fn models(&self) -> &ModelSettings {
        &self.models
    }

    /// Returns the nested session settings.
    pub fn sessions(&self) -> &SessionSettings {
        &self.sessions
    }

    /// Returns the nested retrieval settings.
    pub fn retrieval(&self) -> &RetrievalSettings {
        &self.retrieval
    }

    /// Returns the plugins installation directory path.
    pub fn plugins_directory(&self) -> &str {
        &self.plugins_directory
    }

    /// Replaces the retrieval settings (useful for testing / runtime reconfiguration).
    pub fn with_retrieval(mut self, retrieval: RetrievalSettings) -> Self {
        self.retrieval = retrieval;
        self
    }
}

/// Partial root settings where all nested modules and fields are optional.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct PartialBrainSettings {
    /// Optional schema format version.
    pub version: Option<u32>,
    /// Optional database settings block.
    pub database: Option<PartialDatabaseSettings>,
    /// Optional AI model settings block.
    pub models: Option<PartialModelSettings>,
    /// Optional session settings block.
    pub sessions: Option<PartialSessionSettings>,
    /// Optional retrieval settings block.
    pub retrieval: Option<PartialRetrievalSettings>,
    /// Optional plugins directory path.
    pub plugins_directory: Option<String>,
}

impl Merge for PartialBrainSettings {
    fn merge(&mut self, other: Self) {
        if let Some(v) = other.version {
            self.version = Some(v);
        }
        if let Some(plugins_dir) = other.plugins_directory {
            self.plugins_directory = Some(plugins_dir);
        }

        // Merge database
        if let Some(other_db) = other.database {
            let mut db = self.database.take().unwrap_or_default();
            db.merge(other_db);
            self.database = Some(db);
        }

        // Merge models
        if let Some(other_models) = other.models {
            let mut models = self.models.take().unwrap_or_default();
            models.merge(other_models);
            self.models = Some(models);
        }

        // Merge sessions
        if let Some(other_sessions) = other.sessions {
            let mut sessions = self.sessions.take().unwrap_or_default();
            sessions.merge(other_sessions);
            self.sessions = Some(sessions);
        }

        // Merge retrieval
        if let Some(other_retrieval) = other.retrieval {
            let mut retrieval = self.retrieval.take().unwrap_or_default();
            retrieval.merge(other_retrieval);
            self.retrieval = Some(retrieval);
        }
    }
}
