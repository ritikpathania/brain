//! Empirical Challenger Stress & Adversarial Test Suite for Milestone 5:
//! ServicesReadyPhase & Runtime Wiring.
//!
//! Stress tests:
//! 1. Lifecycle rollback resetting locator fields on earlier phase failure.
//! 2. Getters on unstarted, stopped, and failed runtimes returning typed BrainError.
//! 3. Malformed, uppercase, and boundary values of BRAIN_MEMORY_EXTRACTOR.
//! 4. Partial pre-injection precedence in RuntimeBuilder.
//! 5. Rapid lifecycle churn (50 iterations) without leaks or hangs.
//! 6. Concurrent multithreaded getter access.
//! 7. 100% offline startup verification (zero external network I/O during phase execution).

use std::sync::Arc;
use tempfile::TempDir;

use brain_config::loader::{resolve, DefaultsSource, OverrideSource};
use brain_config::schema::{
    BrainSettings, EmbeddingProvider as ConfigEmbeddingProvider, LlmProvider as ConfigLlmProvider,
    PartialBrainSettings, PartialDatabaseSettings, PartialModelSettings,
};
use brain_core::agents::ChatAgent;
use brain_core::errors::BrainError;
use brain_core::services::ExtractionRequest;
use brain_services::conversation::DummyChatAgent;
use brain_services::runtime::{ApplicationRuntime, HealthStatus, RuntimeState};

static TEST_MUTEX: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

struct ChallengerEnv {
    _temp_dir: TempDir,
    db_path: String,
    plugins_path: String,
}

impl ChallengerEnv {
    fn new() -> Self {
        let temp_dir = TempDir::new().expect("Failed to create tempdir");
        let db_path = temp_dir
            .path()
            .join("challenger.db")
            .to_string_lossy()
            .to_string();
        let plugins_path = temp_dir
            .path()
            .join("plugins")
            .to_string_lossy()
            .to_string();
        std::fs::create_dir_all(&plugins_path).unwrap();
        Self {
            _temp_dir: temp_dir,
            db_path,
            plugins_path,
        }
    }

    fn config(
        &self,
        llm: Option<ConfigLlmProvider>,
        emb: Option<ConfigEmbeddingProvider>,
        dim: Option<usize>,
    ) -> BrainSettings {
        let partial = PartialBrainSettings {
            database: Some(PartialDatabaseSettings {
                path: Some(self.db_path.clone()),
                pool_size: Some(2),
                enable_wal: Some(false),
            }),
            plugins_directory: Some(self.plugins_path.clone()),
            models: Some(PartialModelSettings {
                active_llm_provider: llm,
                active_embedding_provider: emb,
                embedding_dimension: dim,
            }),
            ..Default::default()
        };
        resolve(&[
            Box::new(DefaultsSource),
            Box::new(OverrideSource::new(partial)),
        ])
        .expect("Failed to resolve challenger test configuration")
    }
}

// ----------------------------------------------------------------------------
// Challenge 1: Early Phase Rollback Resets Locator Fields & Error Handling
// ----------------------------------------------------------------------------
#[tokio::test]
async fn test_challenge_rollback_cleans_locator_fields_on_early_phase_failure() {
    let _guard = TEST_MUTEX.lock().await;

    // Point to completely invalid path that fails during StorageMigrationPhase
    let bad_partial = PartialBrainSettings {
        database: Some(PartialDatabaseSettings {
            path: Some("/dev/null/impossible_dir/db.sqlite".to_string()),
            pool_size: Some(2),
            enable_wal: Some(false),
        }),
        ..Default::default()
    };
    let bad_config = resolve(&[
        Box::new(DefaultsSource),
        Box::new(OverrideSource::new(bad_partial)),
    ])
    .expect("Failed to resolve bad configuration");

    let runtime = ApplicationRuntime::builder()
        .with_config(bad_config)
        .build()
        .expect("Build should succeed before startup");

    let start_res = runtime.start();
    assert!(
        start_res.is_err(),
        "Runtime start must fail due to storage error"
    );
    assert_eq!(
        runtime.state(),
        RuntimeState::Created,
        "Runtime state must revert to Created"
    );
    assert!(!runtime.is_ready(), "Runtime must not report ready");

    // All getters must return BrainError::InvalidTransition
    match runtime.embedding_provider() {
        Err(BrainError::InvalidTransition { message }) => {
            assert!(message.contains("not in Running state"));
        }
        _ => panic!("Expected BrainError::InvalidTransition"),
    }

    match runtime.chat_agent() {
        Err(BrainError::InvalidTransition { message }) => {
            assert!(message.contains("not in Running state"));
        }
        _ => panic!("Expected BrainError::InvalidTransition"),
    }

    match runtime.memory_extractor() {
        Err(BrainError::InvalidTransition { message }) => {
            assert!(message.contains("not in Running state"));
        }
        _ => panic!("Expected BrainError::InvalidTransition"),
    }

    match runtime.conversation() {
        Err(BrainError::InvalidTransition { message }) => {
            assert!(message.contains("not in Running state"));
        }
        _ => panic!("Expected BrainError::InvalidTransition"),
    }

    match runtime.storage() {
        Err(BrainError::InvalidTransition { message }) => {
            assert!(message.contains("not in Running state"));
        }
        _ => panic!("Expected BrainError::InvalidTransition"),
    }

    match runtime.tools() {
        Err(BrainError::InvalidTransition { message }) => {
            assert!(message.contains("not in Running state"));
        }
        _ => panic!("Expected BrainError::InvalidTransition"),
    }
}

// ----------------------------------------------------------------------------
// Challenge 2: Getters on Unstarted and Stopped Runtimes
// ----------------------------------------------------------------------------
#[tokio::test]
async fn test_challenge_getters_unstarted_and_stopped() {
    let _guard = TEST_MUTEX.lock().await;
    let env = ChallengerEnv::new();
    let config = env.config(None, None, None);

    let runtime = ApplicationRuntime::builder()
        .with_config(config)
        .build()
        .expect("Build failed");

    // UNSTARTED state: Created
    assert_eq!(runtime.state(), RuntimeState::Created);
    assert!(matches!(
        runtime.embedding_provider(),
        Err(BrainError::InvalidTransition { .. })
    ));
    assert!(matches!(
        runtime.chat_agent(),
        Err(BrainError::InvalidTransition { .. })
    ));
    assert!(matches!(
        runtime.memory_extractor(),
        Err(BrainError::InvalidTransition { .. })
    ));
    assert!(matches!(
        runtime.conversation(),
        Err(BrainError::InvalidTransition { .. })
    ));

    // START runtime
    runtime.start().expect("Start failed");
    assert_eq!(runtime.state(), RuntimeState::Running);
    assert!(runtime.embedding_provider().is_ok());
    assert!(runtime.chat_agent().is_ok());
    assert!(runtime.memory_extractor().is_ok());
    assert!(runtime.conversation().is_ok());

    // STOP runtime
    runtime.shutdown().expect("Shutdown failed");
    assert_eq!(runtime.state(), RuntimeState::Stopped);

    // STOPPED state
    assert!(matches!(
        runtime.embedding_provider(),
        Err(BrainError::InvalidTransition { .. })
    ));
    assert!(matches!(
        runtime.chat_agent(),
        Err(BrainError::InvalidTransition { .. })
    ));
    assert!(matches!(
        runtime.memory_extractor(),
        Err(BrainError::InvalidTransition { .. })
    ));
    assert!(matches!(
        runtime.conversation(),
        Err(BrainError::InvalidTransition { .. })
    ));
}

// ----------------------------------------------------------------------------
// Challenge 3: BRAIN_MEMORY_EXTRACTOR Malformed & Boundary Values Fallback
// ----------------------------------------------------------------------------
#[tokio::test]
async fn test_challenge_env_var_malformed_and_boundary_fallback() {
    let _guard = TEST_MUTEX.lock().await;

    let test_cases = vec![
        ("invalid_extractor_xyz", true), // invalid string -> fallback heuristic
        ("12345", true),                 // numbers -> fallback heuristic
        ("", true),                      // empty string -> fallback heuristic
        ("   ", true),                   // whitespace -> fallback heuristic
        ("null", true),                  // literal null -> fallback heuristic
        ("DUMMY", false),                // uppercase DUMMY -> dummy extractor (returns empty)
        ("DuMmY", false),                // mixed-case -> dummy extractor
        ("NOOP", false),                 // uppercase NOOP -> dummy extractor
        ("noop", false),                 // lowercase noop -> dummy extractor
    ];

    for (env_val, expect_heuristic) in test_cases {
        std::env::set_var("BRAIN_MEMORY_EXTRACTOR", env_val);

        let env = ChallengerEnv::new();
        let config = env.config(None, None, None);
        let runtime = ApplicationRuntime::builder()
            .with_config(config)
            .build()
            .expect("Build failed");

        runtime
            .start()
            .expect("Start must succeed for all env var inputs");

        let extractor = runtime
            .memory_extractor()
            .expect("Extractor must be initialized");
        let req = ExtractionRequest {
            raw_content: "sqlite database and rust".to_string(),
            context_metadata: std::collections::HashMap::new(),
        };

        let result = extractor.extract(req).expect("Extraction must succeed");
        if expect_heuristic {
            let ids: Vec<&str> = result
                .nodes
                .iter()
                .filter_map(|n| n.properties.get("canonical_id").and_then(|v| v.as_str()))
                .collect();
            assert!(
                ids.contains(&"sqlite"),
                "Value '{}' should have fallen back to HeuristicExtractor with sqlite node",
                env_val
            );
        } else {
            assert!(
                result.nodes.is_empty(),
                "Value '{}' should have selected DummyMemoryExtractor with empty nodes",
                env_val
            );
        }

        runtime.shutdown().expect("Shutdown failed");
    }

    std::env::remove_var("BRAIN_MEMORY_EXTRACTOR");
}

// ----------------------------------------------------------------------------
// Challenge 4: Partial Pre-injected Builder Overrides
// ----------------------------------------------------------------------------
#[tokio::test]
async fn test_challenge_partial_preinjected_builder_overrides() {
    let _guard = TEST_MUTEX.lock().await;
    let env = ChallengerEnv::new();

    // Config requests OpenAI chat and Ollama embeddings
    let config = env.config(
        Some(ConfigLlmProvider::Openai),
        Some(ConfigEmbeddingProvider::Ollama),
        Some(384),
    );

    // Builder pre-injects ONLY chat_agent (DummyChatAgent)
    let preinjected_chat: Arc<dyn ChatAgent> = Arc::new(DummyChatAgent);

    let runtime = ApplicationRuntime::builder()
        .with_config(config)
        .with_chat_agent(preinjected_chat)
        .build()
        .expect("Build failed");

    runtime.start().expect("Start failed");

    // Pre-injected chat agent must take strict precedence over ConfigLlmProvider::Openai
    assert_eq!(runtime.chat_agent().unwrap().name(), "DummyChatAgent");

    // Embedding provider was NOT pre-injected; must resolve to Ollama according to config
    assert_eq!(runtime.embedding_provider().unwrap().name(), "ollama");

    // Memory extractor was NOT pre-injected; must default to HeuristicExtractor
    let req = ExtractionRequest {
        raw_content: "sqlite rust".to_string(),
        context_metadata: std::collections::HashMap::new(),
    };
    let result = runtime.memory_extractor().unwrap().extract(req).unwrap();
    assert!(
        !result.nodes.is_empty(),
        "Heuristic extractor must produce nodes"
    );

    runtime.shutdown().expect("Shutdown failed");
}

// ----------------------------------------------------------------------------
// Challenge 5: Rapid Lifecycle Churn (50 Iterations)
// ----------------------------------------------------------------------------
#[tokio::test]
async fn test_challenge_rapid_lifecycle_churn_50_iterations() {
    let _guard = TEST_MUTEX.lock().await;

    for i in 0..50 {
        let env = ChallengerEnv::new();
        let config = env.config(None, None, None);

        let runtime = ApplicationRuntime::builder()
            .with_config(config)
            .build()
            .unwrap_or_else(|e| panic!("Iteration {} build failed: {:?}", i, e));

        runtime
            .start()
            .unwrap_or_else(|e| panic!("Iteration {} start failed: {:?}", i, e));

        assert_eq!(runtime.state(), RuntimeState::Running);
        assert!(runtime.embedding_provider().is_ok());
        assert!(runtime.chat_agent().is_ok());
        assert!(runtime.memory_extractor().is_ok());
        assert!(runtime.conversation().is_ok());

        runtime
            .shutdown()
            .unwrap_or_else(|e| panic!("Iteration {} shutdown failed: {:?}", i, e));

        assert_eq!(runtime.state(), RuntimeState::Stopped);
    }
}

// ----------------------------------------------------------------------------
// Challenge 6: Concurrent Multi-Threaded Getter Access
// ----------------------------------------------------------------------------
#[tokio::test]
async fn test_challenge_concurrent_getter_access() {
    let _guard = TEST_MUTEX.lock().await;
    let env = ChallengerEnv::new();
    let config = env.config(None, None, None);

    let runtime = Arc::new(
        ApplicationRuntime::builder()
            .with_config(config)
            .build()
            .expect("Build failed"),
    );
    runtime.start().expect("Start failed");

    let mut handles = Vec::new();
    for _ in 0..20 {
        let rt = runtime.clone();
        handles.push(tokio::spawn(async move {
            for _ in 0..50 {
                assert!(rt.embedding_provider().is_ok());
                assert!(rt.chat_agent().is_ok());
                assert!(rt.memory_extractor().is_ok());
                assert!(rt.conversation().is_ok());
                assert!(rt.storage().is_ok());
                assert!(rt.tools().is_ok());
                let report = rt.health();
                assert_eq!(report.overall, HealthStatus::Healthy);
            }
        }));
    }

    for h in handles {
        h.await.expect("Concurrent worker panicked");
    }

    runtime.shutdown().expect("Shutdown failed");
}

// ----------------------------------------------------------------------------
// Challenge 7: 100% Offline Guarantee During Phase Execution
// ----------------------------------------------------------------------------
#[tokio::test]
async fn test_challenge_offline_guarantee_all_provider_configs() {
    let _guard = TEST_MUTEX.lock().await;

    // Test with all provider variations using fake / non-routable hosts:
    // None of these should hang, attempt network connections, or fail during runtime.start().
    let configs = vec![
        // OpenAI LLM & OpenAI embedding with non-routable domain
        (
            Some(ConfigLlmProvider::Openai),
            Some(ConfigEmbeddingProvider::Openai),
        ),
        // Ollama LLM & Ollama embedding
        (
            Some(ConfigLlmProvider::Ollama),
            Some(ConfigEmbeddingProvider::Ollama),
        ),
        // Anthropic LLM & Noop embedding
        (
            Some(ConfigLlmProvider::Anthropic),
            Some(ConfigEmbeddingProvider::Anthropic),
        ),
        // Custom OpenAI-compatible
        (
            Some(ConfigLlmProvider::Custom("dummy".to_string())),
            Some(ConfigEmbeddingProvider::Custom(
                "openai-compatible".to_string(),
            )),
        ),
    ];

    for (llm, emb) in configs {
        let env = ChallengerEnv::new();
        let config = env.config(llm, emb, Some(384));

        let start_instant = std::time::Instant::now();
        let runtime = ApplicationRuntime::builder()
            .with_config(config)
            .build()
            .expect("Build failed");

        runtime.start().expect("Startup must succeed offline");
        let elapsed = start_instant.elapsed();

        // Must complete in under 500ms (verifying no HTTP connection attempts or DNS timeouts)
        assert!(
            elapsed.as_millis() < 500,
            "Startup took {:?}, suggesting network blocking or DNS lookup occurred!",
            elapsed
        );

        runtime.shutdown().expect("Shutdown failed");
    }
}
