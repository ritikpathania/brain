use brain_config::*;
use std::convert::TryFrom;
use std::path::PathBuf;
use std::sync::Mutex;

// Global lock to prevent environment variable test interference
static ENV_MUTEX: Mutex<()> = Mutex::new(());

#[test]
fn test_defaults_source() {
    let source = DefaultsSource;
    let partial = source.load().unwrap();

    assert_eq!(partial.version, Some(1));
    assert_eq!(
        partial.database.as_ref().unwrap().path,
        Some("brain.db".to_string())
    );
    assert_eq!(
        partial.models.as_ref().unwrap().active_llm_provider,
        Some(LlmProvider::Anthropic)
    );
}

#[test]
fn test_toml_source_missing() {
    // Missing config file is not an error and returns an empty partial
    let source = TomlSource::new(PathBuf::from("nonexistent_config.toml"));
    let partial = source.load().unwrap();
    assert_eq!(partial, PartialBrainSettings::default());
}

#[test]
fn test_toml_source_parsing() {
    let dir = std::env::temp_dir();
    let file_path = dir.join("test_config.toml");

    let toml_content = r#"
        version = 2
        plugins_directory = "/custom/plugins"

        [database]
        path = "custom.db"
        pool_size = 10
        enable_wal = false

        [models]
        active_llm_provider = "openai"
        active_embedding_provider = "ollama"
        embedding_dimension = 768

        [sessions]
        volatile_ttl_secs = 600
        max_sliding_window_size = 50
    "#;

    std::fs::write(&file_path, toml_content).unwrap();

    let source = TomlSource::new(file_path.clone());
    let partial = source.load().unwrap();

    assert_eq!(partial.version, Some(2));
    assert_eq!(
        partial.plugins_directory,
        Some("/custom/plugins".to_string())
    );
    assert_eq!(
        partial.database.as_ref().unwrap().path,
        Some("custom.db".to_string())
    );
    assert_eq!(
        partial.models.as_ref().unwrap().active_llm_provider,
        Some(LlmProvider::Openai)
    );
    assert_eq!(
        partial.models.as_ref().unwrap().active_embedding_provider,
        Some(EmbeddingProvider::Ollama)
    );

    let _ = std::fs::remove_file(file_path);
}

#[test]
fn test_environment_source() {
    let _lock = ENV_MUTEX.lock().unwrap();

    // Set test env variables
    std::env::set_var("BRAIN_VERSION", "3");
    std::env::set_var("BRAIN_DATABASE_PATH", "env.db");
    std::env::set_var("BRAIN_DATABASE_POOL_SIZE", "15");
    std::env::set_var("BRAIN_MODEL_LLM_PROVIDER", "gemini");
    std::env::set_var("BRAIN_MODEL_EMBEDDING_PROVIDER", "custom_provider");

    let source = EnvironmentSource;
    let partial = source.load().unwrap();

    assert_eq!(partial.version, Some(3));
    assert_eq!(
        partial.database.as_ref().unwrap().path,
        Some("env.db".to_string())
    );
    assert_eq!(partial.database.as_ref().unwrap().pool_size, Some(15));
    assert_eq!(
        partial.models.as_ref().unwrap().active_llm_provider,
        Some(LlmProvider::Gemini)
    );
    assert_eq!(
        partial.models.as_ref().unwrap().active_embedding_provider,
        Some(EmbeddingProvider::Custom("custom_provider".to_string()))
    );

    // Clean up env
    std::env::remove_var("BRAIN_VERSION");
    std::env::remove_var("BRAIN_DATABASE_PATH");
    std::env::remove_var("BRAIN_DATABASE_POOL_SIZE");
    std::env::remove_var("BRAIN_MODEL_LLM_PROVIDER");
    std::env::remove_var("BRAIN_MODEL_EMBEDDING_PROVIDER");
}

#[test]
fn test_override_source() {
    let custom = PartialBrainSettings {
        version: Some(9),
        plugins_directory: Some("/override/plugins".to_string()),
        ..Default::default()
    };

    let source = OverrideSource::new(custom);
    let partial = source.load().unwrap();

    assert_eq!(partial.version, Some(9));
    assert_eq!(
        partial.plugins_directory,
        Some("/override/plugins".to_string())
    );
}

#[test]
fn test_merge_precedence() {
    let mut defaults = PartialBrainSettings {
        version: Some(1),
        plugins_directory: Some("defaults_dir".to_string()),
        ..Default::default()
    };

    let override_layer = PartialBrainSettings {
        version: Some(2),
        ..Default::default()
    };

    // Prioritize values from override_layer
    defaults.merge(override_layer);

    assert_eq!(defaults.version, Some(2));
    assert_eq!(defaults.plugins_directory, Some("defaults_dir".to_string()));
}

#[test]
fn test_merge_associative() {
    let a = PartialBrainSettings {
        version: Some(1),
        plugins_directory: Some("dir_a".to_string()),
        ..Default::default()
    };

    let b = PartialBrainSettings {
        version: Some(2),
        database: Some(PartialDatabaseSettings {
            path: Some("b.db".to_string()),
            ..Default::default()
        }),
        ..Default::default()
    };

    let c = PartialBrainSettings {
        plugins_directory: Some("dir_c".to_string()),
        database: Some(PartialDatabaseSettings {
            pool_size: Some(10),
            ..Default::default()
        }),
        ..Default::default()
    };

    // Test (A merge B) merge C
    let mut ab = a.clone();
    ab.merge(b.clone());
    let mut ab_c = ab;
    ab_c.merge(c.clone());

    // Test A merge (B merge C)
    let mut bc = b.clone();
    bc.merge(c.clone());
    let mut a_bc = a.clone();
    a_bc.merge(bc);

    // Assert associative continuity
    assert_eq!(ab_c, a_bc);
}

#[test]
fn test_resolution_idempotent() {
    let sources: Vec<Box<dyn ConfigSource>> = vec![
        Box::new(DefaultsSource),
        Box::new(OverrideSource::new(PartialBrainSettings {
            plugins_directory: Some("/final/path".to_string()),
            ..Default::default()
        })),
    ];

    let res1 = resolve(&sources).unwrap();
    let res2 = resolve(&sources).unwrap();

    assert_eq!(res1.version(), res2.version());
    assert_eq!(res1.plugins_directory(), res2.plugins_directory());
    assert_eq!(res1.database().path(), res2.database().path());
}

#[test]
fn test_validation_semantic_rules() {
    // 1. Invalid Database Pool Size
    let bad_db = PartialBrainSettings {
        version: Some(1),
        plugins_directory: Some("/plugins".to_string()),
        database: Some(PartialDatabaseSettings {
            path: Some("test.db".to_string()),
            pool_size: Some(0), // Fails validation: pool size must be > 0
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
        }),
    };

    let settings = BrainSettings::try_from(bad_db).unwrap();
    let res = validate(&settings);
    assert!(res.is_err());
    assert!(format!("{}", res.unwrap_err()).contains("database.pool_size"));

    // 2. Invalid Session TTL
    let bad_ttl = PartialBrainSettings {
        version: Some(1),
        plugins_directory: Some("/plugins".to_string()),
        database: Some(PartialDatabaseSettings {
            path: Some("test.db".to_string()),
            pool_size: Some(5),
            enable_wal: Some(true),
        }),
        models: Some(PartialModelSettings {
            active_llm_provider: Some(LlmProvider::Anthropic),
            active_embedding_provider: Some(EmbeddingProvider::Anthropic),
            embedding_dimension: Some(1536),
        }),
        sessions: Some(PartialSessionSettings {
            volatile_ttl_secs: Some(0), // Fails validation: TTL must be >= 1
            max_sliding_window_size: Some(100),
        }),
        retrieval: Some(PartialRetrievalSettings {
            ranking_policy: Some(RankingPolicy::DefaultRrf),
            model_path: None,
        }),
    };

    let settings = BrainSettings::try_from(bad_ttl).unwrap();
    let res = validate(&settings);
    assert!(res.is_err());
    assert!(format!("{}", res.unwrap_err()).contains("sessions.volatile_ttl_secs"));
}
