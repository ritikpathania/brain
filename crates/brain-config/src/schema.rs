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
    /// Settings for temporal reranking.
    pub(crate) temporal_ranking: brain_core::retrieval::TemporalRankingSettings,
}

impl RetrievalSettings {
    /// Creates a new RetrievalSettings.
    pub fn new(
        ranking_policy: RankingPolicy,
        model_path: Option<String>,
        temporal_ranking: brain_core::retrieval::TemporalRankingSettings,
    ) -> Self {
        Self {
            ranking_policy,
            model_path,
            temporal_ranking,
        }
    }

    /// Returns the active ranking policy.
    pub fn ranking_policy(&self) -> RankingPolicy {
        self.ranking_policy
    }

    /// Returns the path to the serialized model file, if configured.
    pub fn model_path(&self) -> Option<&str> {
        self.model_path.as_deref()
    }

    /// Returns the active temporal ranking settings.
    pub fn temporal_ranking(&self) -> &brain_core::retrieval::TemporalRankingSettings {
        &self.temporal_ranking
    }
}

/// Partial settings for temporal ranking.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct PartialTemporalRankingSettings {
    /// Flag indicating if temporal reranking is active.
    pub enabled: Option<bool>,
    /// The decay model function to apply.
    pub model: Option<brain_core::retrieval::DecayModel>,
    /// Parameter half-life duration in seconds.
    pub half_life_seconds: Option<u64>,
    /// General scaling factor applied to decay scores.
    pub scaling_factor: Option<f64>,
}

impl Merge for PartialTemporalRankingSettings {
    fn merge(&mut self, other: Self) {
        if let Some(enabled) = other.enabled {
            self.enabled = Some(enabled);
        }
        if let Some(model) = other.model {
            self.model = Some(model);
        }
        if let Some(half_life) = other.half_life_seconds {
            self.half_life_seconds = Some(half_life);
        }
        if let Some(scaling_factor) = other.scaling_factor {
            self.scaling_factor = Some(scaling_factor);
        }
    }
}

/// Partial retrieval settings.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct PartialRetrievalSettings {
    /// Optional ranking policy.
    pub ranking_policy: Option<RankingPolicy>,
    /// Optional model path.
    pub model_path: Option<String>,
    /// Optional settings for temporal ranking.
    pub temporal_ranking: Option<PartialTemporalRankingSettings>,
}

impl Merge for PartialRetrievalSettings {
    fn merge(&mut self, other: Self) {
        if let Some(policy) = other.ranking_policy {
            self.ranking_policy = Some(policy);
        }
        if let Some(path) = other.model_path {
            self.model_path = Some(path);
        }
        if let Some(other_temporal) = other.temporal_ranking {
            let mut temp = self.temporal_ranking.take().unwrap_or_default();
            temp.merge(other_temporal);
            self.temporal_ranking = Some(temp);
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
    /// Nested reflection settings.
    pub(crate) reflection: ReflectionSettings,
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

    /// Returns the nested reflection settings.
    pub fn reflection(&self) -> &ReflectionSettings {
        &self.reflection
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
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
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
    /// Optional reflection settings block.
    pub reflection: Option<PartialReflectionSettings>,
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

        // Merge reflection
        if let Some(other_reflection) = other.reflection {
            let mut reflection = self.reflection.take().unwrap_or_default();
            reflection.merge(other_reflection);
            self.reflection = Some(reflection);
        }
    }
}

/// Reflection settings configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReflectionSettings {
    /// Threshold confidence score for merging duplicate concept nodes.
    pub(crate) duplicate_confidence_threshold: f64,
    /// Threshold confidence score for suggesting inferred transitive links.
    pub(crate) link_suggestion_confidence_threshold: f64,
    /// Enable auto-approval and automatic execution of reflection merge commands.
    pub(crate) auto_approve_merges: bool,
    /// Enable periodic background reflection scheduler thread.
    pub(crate) background_enabled: bool,
    /// Interval in seconds between background reflection ticks.
    pub(crate) interval_secs: u64,
    /// Minimum new WAL events required before triggering a background cycle.
    pub(crate) min_events_trigger: u64,
    /// Maximum concept nodes evaluated per reflection cycle.
    pub(crate) max_nodes_per_cycle: usize,
    /// Time budget in milliseconds per reflection cycle before cancelling.
    pub(crate) cycle_time_budget_ms: u64,
}

impl ReflectionSettings {
    /// Returns the duplicate merge confidence threshold.
    pub fn duplicate_confidence_threshold(&self) -> f64 {
        self.duplicate_confidence_threshold
    }

    /// Returns the link suggestion confidence threshold.
    pub fn link_suggestion_confidence_threshold(&self) -> f64 {
        self.link_suggestion_confidence_threshold
    }

    /// Returns whether auto-approve merges is enabled.
    pub fn auto_approve_merges(&self) -> bool {
        self.auto_approve_merges
    }

    /// Returns whether background reflection scheduling is enabled.
    pub fn background_enabled(&self) -> bool {
        self.background_enabled
    }

    /// Returns interval in seconds between background ticks.
    pub fn interval_secs(&self) -> u64 {
        self.interval_secs
    }

    /// Returns minimum WAL event delta required to trigger a cycle.
    pub fn min_events_trigger(&self) -> u64 {
        self.min_events_trigger
    }

    /// Returns maximum node evaluation limit per cycle.
    pub fn max_nodes_per_cycle(&self) -> usize {
        self.max_nodes_per_cycle
    }

    /// Returns time budget in milliseconds per cycle.
    pub fn cycle_time_budget_ms(&self) -> u64 {
        self.cycle_time_budget_ms
    }
}

/// Partial reflection settings where all fields are optional.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct PartialReflectionSettings {
    /// Optional duplicate merge confidence threshold.
    pub duplicate_confidence_threshold: Option<f64>,
    /// Optional link suggestion confidence threshold.
    pub link_suggestion_confidence_threshold: Option<f64>,
    /// Optional auto-approve merges toggle.
    pub auto_approve_merges: Option<bool>,
    /// Optional background scheduler toggle.
    pub background_enabled: Option<bool>,
    /// Optional interval in seconds between background ticks.
    pub interval_secs: Option<u64>,
    /// Optional minimum WAL event delta trigger.
    pub min_events_trigger: Option<u64>,
    /// Optional maximum nodes cap per cycle.
    pub max_nodes_per_cycle: Option<usize>,
    /// Optional time budget in ms per cycle.
    pub cycle_time_budget_ms: Option<u64>,
}

impl Merge for PartialReflectionSettings {
    fn merge(&mut self, other: Self) {
        if let Some(t) = other.duplicate_confidence_threshold {
            self.duplicate_confidence_threshold = Some(t);
        }
        if let Some(t) = other.link_suggestion_confidence_threshold {
            self.link_suggestion_confidence_threshold = Some(t);
        }
        if let Some(a) = other.auto_approve_merges {
            self.auto_approve_merges = Some(a);
        }
        if let Some(bg) = other.background_enabled {
            self.background_enabled = Some(bg);
        }
        if let Some(inv) = other.interval_secs {
            self.interval_secs = Some(inv);
        }
        if let Some(min_ev) = other.min_events_trigger {
            self.min_events_trigger = Some(min_ev);
        }
        if let Some(max_n) = other.max_nodes_per_cycle {
            self.max_nodes_per_cycle = Some(max_n);
        }
        if let Some(budget) = other.cycle_time_budget_ms {
            self.cycle_time_budget_ms = Some(budget);
        }
    }
}
