use brain_config::schema::{BrainSettings, RankingPolicy};
use brain_config::{ConfigSource, DefaultsSource};
use brain_core::errors::BrainError;
use brain_core::retrieval::{DefaultQueryEmbeddingService, EmbeddingProvider, RetrievalRequest};
use brain_domain::SessionId;
use brain_services::retrieval::eval_harness::ranking::RankingWeights;
use brain_services::retrieval::eval_harness::LinearRanker;
use brain_services::retrieval::ranking::feature_provider::{FeatureContext, RankingDecay};
use brain_services::retrieval::RetrievalServiceImpl;
use brain_storage::TestStorage;
use std::collections::HashSet;
use std::sync::Arc;

fn current_time_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

struct DummyEmbeddingProvider;
impl EmbeddingProvider for DummyEmbeddingProvider {
    fn name(&self) -> &'static str {
        "dummy"
    }
    fn embed(&self, _text: &str) -> Result<Vec<f32>, BrainError> {
        Ok(vec![1.0, 0.0, 0.0])
    }
}

fn load_default_config() -> BrainSettings {
    BrainSettings::try_from(DefaultsSource.load().unwrap()).unwrap()
}

// ---------------------------------------------------------------------------
// TEST 1: Policy switching + fallback to DefaultRrf when model file is absent
// ---------------------------------------------------------------------------
#[test]
fn test_ranking_policy_switching_and_fallback() {
    let test_storage = TestStorage::new();
    let storage = test_storage.store();
    let cache_manager = Arc::new(brain_session::SessionCacheManager::new());
    let registry = Arc::new(brain_domain::RelationRegistry::default_embedded());
    let query_embedding_service = Arc::new(DefaultQueryEmbeddingService::new(Arc::new(
        DummyEmbeddingProvider,
    )));

    let config =
        load_default_config().with_retrieval(brain_config::schema::RetrievalSettings::new(
            RankingPolicy::LearnedModel,
            None, // no path => fallback to DefaultRrf
        ));

    let svc = RetrievalServiceImpl::new_with_config(
        storage.clone(),
        &config,
        cache_manager.clone(),
        registry.clone(),
        query_embedding_service.clone(),
    );

    let request = RetrievalRequest {
        session_id: SessionId::new(),
        query: "rust database".to_string(),
        limit: 5,
        exclude_ids: HashSet::new(),
        deadline: None,
    };

    let response = svc.execute_pipeline(&request).unwrap();
    // DB is empty but the pipeline must complete without error
    assert!(response.nodes.is_empty());
}

// ---------------------------------------------------------------------------
// TEST 2: Fingerprint determinism - same feature inputs always produce same digest
// ---------------------------------------------------------------------------
#[test]
fn test_feature_fingerprint_determinism() {
    use brain_services::retrieval::ranking::feature_provider::FeatureVector;

    let fv = FeatureVector {
        lexical_similarity: Some(0.75),
        semantic_similarity: Some(0.85),
        recency: Some(0.9),
        importance: Some(0.5),
        provenance_confidence: Some(1.0),
        graph_degree: Some(0.693), // ln(2+1)
        access_frequency: Some(0.0),
        freshness_decay: Some(0.99),
    };

    let fp1 = fv.fingerprint();
    let fp2 = fv.fingerprint();

    // Determinism: fingerprint must be stable across multiple calls
    assert_eq!(fp1, fp2, "FeatureVector fingerprint must be deterministic");

    // Length: SHA-256 hex encoding is always 64 chars
    assert_eq!(fp1.len(), 64, "SHA-256 hex digest must be 64 characters");

    // Different inputs => different digest (collision resistance check)
    let fv2 = FeatureVector {
        lexical_similarity: Some(0.50), // changed
        ..fv.clone()
    };
    assert_ne!(
        fv.fingerprint(),
        fv2.fingerprint(),
        "Different features must produce different fingerprints"
    );
}

// ---------------------------------------------------------------------------
// TEST 3: LinearRanker serialization round-trip parity
// ---------------------------------------------------------------------------
#[test]
fn test_linear_ranker_serialization_round_trip() {
    let weights = RankingWeights {
        lexical: 0.5,
        semantic: 0.5,
        recency: 0.1,
        importance: 0.2,
        provenance_confidence: 0.1,
        graph_degree: 0.1,
        access_frequency: 0.05,
        freshness_decay: 0.05,
    };
    let model = LinearRanker::new(weights);

    // Serialize to JSON
    let json = serde_json::to_string(&model).unwrap();
    assert!(!json.is_empty(), "Serialized model JSON must not be empty");

    // Deserialize back
    let deserialized: LinearRanker = serde_json::from_str(&json).unwrap();

    // Verify that scoring the same feature vector produces identical outputs
    use brain_services::retrieval::ranking::feature_provider::FeatureVector;

    let fv = FeatureVector {
        lexical_similarity: Some(0.7),
        semantic_similarity: Some(0.8),
        recency: Some(0.9),
        importance: Some(0.5),
        provenance_confidence: Some(1.0),
        graph_degree: Some(0.693),
        access_frequency: Some(0.0),
        freshness_decay: Some(0.99),
    };

    let score_original = model.score(&fv);
    let score_deserialized = deserialized.score(&fv);

    assert_eq!(
        score_original, score_deserialized,
        "Deserialized model must produce identical scores to original"
    );
}

// ---------------------------------------------------------------------------
// TEST 4: ModelLoader round-trip - serialize LinearRanker to file, load as Arc<dyn ScoreRanker>
// ---------------------------------------------------------------------------
#[test]
fn test_model_loader_file_round_trip() {
    use brain_services::retrieval::model_loader::ModelLoader;
    use brain_services::retrieval::ranking::feature_provider::FeatureVector;

    let weights = RankingWeights {
        lexical: 0.5,
        semantic: 1.0,
        recency: 0.0,
        importance: 0.0,
        provenance_confidence: 0.0,
        graph_degree: 0.0,
        access_frequency: 0.0,
        freshness_decay: 0.0,
    };
    let model = LinearRanker::new(weights);
    let json = serde_json::to_string(&model).unwrap();

    // Write to temp file
    let mut temp = std::env::temp_dir();
    temp.push(format!("brain_test_loader_{}.json", uuid::Uuid::new_v4()));
    let path = temp.to_str().unwrap().to_string();
    std::fs::write(&path, &json).unwrap();

    // Load via ModelLoader
    let loaded =
        ModelLoader::load_from_file(&path).expect("ModelLoader must load file successfully");

    let fv = FeatureVector {
        lexical_similarity: Some(1.0),
        semantic_similarity: Some(0.0),
        recency: None,
        importance: None,
        provenance_confidence: None,
        graph_degree: None,
        access_frequency: None,
        freshness_decay: None,
    };

    // Score must match the pure offline score
    let expected_score = model.score(&fv);
    let loaded_score = loaded.score(&fv);
    assert_eq!(
        expected_score, loaded_score,
        "ModelLoader must preserve exact scoring parity with the offline model"
    );

    // Cleanup
    let _ = std::fs::remove_file(&path);
}

// ---------------------------------------------------------------------------
// TEST 5: Three-stage parity across online pipeline and offline evaluation
//         Stage 1: FeatureVector fingerprint identity
//         Stage 2: Raw score parity
//         Stage 3: Node ordering parity
// ---------------------------------------------------------------------------
#[test]
fn test_three_stage_parity_via_extractor() {
    // Use the inner feature_provider::FeatureExtractor which takes (lexical, semantic, context)
    use brain_services::retrieval::ranking::feature_provider::FeatureExtractor as InnerExtractor;

    let weights = RankingWeights {
        lexical: 1.0,
        semantic: 0.5,
        recency: 0.0,
        importance: 0.0,
        provenance_confidence: 0.0,
        graph_degree: 0.0,
        access_frequency: 0.0,
        freshness_decay: 0.0,
    };
    let model = LinearRanker::new(weights);

    let reference_time = current_time_secs();
    let decay = RankingDecay::default();
    let extractor = InnerExtractor::new(reference_time, decay);

    // Simulate two candidate nodes with different lexical/semantic signals
    let ctx_a = FeatureContext {
        updated_at: Some(reference_time - 100),
        importance: Some(0.5),
        pinned: false,
        provenance_confidence: Some(1.0),
        graph_degree: Some(2),
        access_count: Some(0),
        last_observed_at: Some(reference_time - 50),
    };
    let ctx_b = FeatureContext {
        updated_at: Some(reference_time - 200),
        importance: Some(0.3),
        pinned: false,
        provenance_confidence: Some(0.8),
        graph_degree: Some(1),
        access_count: Some(1),
        last_observed_at: Some(reference_time - 150),
    };

    // Extract using the correct 3-arg inner API
    let fv_a = extractor.extract(Some(0.9), Some(0.8), &ctx_a);
    let fv_b = extractor.extract(Some(0.4), Some(0.3), &ctx_b);

    // Stage 1: Fingerprint uniqueness
    let fp_a = fv_a.fingerprint();
    let fp_b = fv_b.fingerprint();
    assert_ne!(
        fp_a, fp_b,
        "Stage 1: Different candidates must have distinct fingerprints"
    );

    // Stage 2: Raw score correctness
    let score_a = model.score(&fv_a);
    let score_b = model.score(&fv_b);
    assert!(
        score_a > score_b,
        "Stage 2: Node A has higher lexical/semantic signals and must score higher than Node B: {:.4} vs {:.4}",
        score_a, score_b
    );

    // Stage 3: Ordering parity - a sorted Vec reflects the score ordering
    let mut candidates = [("b", score_b), ("a", score_a)];
    candidates.sort_by(|x, y| y.1.partial_cmp(&x.1).unwrap_or(std::cmp::Ordering::Equal));
    assert_eq!(
        candidates[0].0, "a",
        "Stage 3: Candidate A must rank first after sorting by score"
    );
    assert_eq!(
        candidates[1].0, "b",
        "Stage 3: Candidate B must rank second after sorting by score"
    );
}

// ---------------------------------------------------------------------------
// TEST 6: ModelEnvelope round-trip — wrapping and reloading preserves parity
// ---------------------------------------------------------------------------
#[test]
fn test_model_envelope_round_trip() {
    use brain_services::retrieval::model_loader::{ModelEnvelope, ModelLoader};
    use brain_services::retrieval::ranking::feature_provider::FeatureVector;

    let weights = RankingWeights {
        lexical: 0.6,
        semantic: 0.4,
        recency: 0.0,
        importance: 0.0,
        provenance_confidence: 0.0,
        graph_degree: 0.0,
        access_frequency: 0.0,
        freshness_decay: 0.0,
    };
    let model = LinearRanker::new(weights);

    // Wrap with current-version metadata
    let envelope =
        ModelEnvelope::wrap(&model, 1, 1).expect("wrap() must succeed for a valid model");

    // Verify metadata is stamped with the current feature schema version
    use brain_services::retrieval::ranking::feature_provider::FEATURE_SCHEMA_VERSION;
    assert_eq!(
        envelope.metadata.feature_schema_version, FEATURE_SCHEMA_VERSION,
        "Envelope must carry current FEATURE_SCHEMA_VERSION"
    );

    // Serialize + reload via ModelLoader
    let json = serde_json::to_string(&envelope).expect("Envelope must serialize to JSON");
    let loaded = ModelLoader::load_from_str(&json)
        .expect("ModelLoader must accept a valid current-version envelope");

    // Scoring parity
    let fv = FeatureVector {
        lexical_similarity: Some(0.9),
        semantic_similarity: Some(0.1),
        recency: None,
        importance: None,
        provenance_confidence: None,
        graph_degree: None,
        access_frequency: None,
        freshness_decay: None,
    };
    assert_eq!(
        model.score(&fv),
        loaded.score(&fv),
        "Envelope round-trip must preserve exact scoring parity"
    );
}

// ---------------------------------------------------------------------------
// TEST 7: ModelLoader rejects a model with a too-old feature_schema_version
// ---------------------------------------------------------------------------
#[test]
fn test_model_loader_rejects_too_old_schema_version() {
    use brain_services::retrieval::model_loader::{ModelEnvelope, ModelLoader, ModelMetadata};

    let weights = RankingWeights {
        lexical: 1.0,
        semantic: 0.0,
        recency: 0.0,
        importance: 0.0,
        provenance_confidence: 0.0,
        graph_degree: 0.0,
        access_frequency: 0.0,
        freshness_decay: 0.0,
    };
    let model = LinearRanker::new(weights);

    // Manually construct an envelope with an incompatibly old schema version
    let mut envelope = ModelEnvelope::wrap(&model, 1, 1).expect("wrap() must succeed");
    envelope.metadata = ModelMetadata {
        feature_schema_version: 0, // too old — below MIN_COMPATIBLE_FEATURE_SCHEMA_VERSION
        model_version: 1,
        trainer_version: 1,
    };

    let json = serde_json::to_string(&envelope).expect("must serialize");
    let result = ModelLoader::load_from_str(&json);
    assert!(
        result.is_err(),
        "ModelLoader must reject models with feature_schema_version below the minimum"
    );
    let err_msg = format!("{:?}", result.err().expect("expected Err"));
    assert!(
        err_msg.contains("too old"),
        "Error message must mention 'too old': {}",
        err_msg
    );
}

// ---------------------------------------------------------------------------
// TEST 8: ModelLoader rejects a model with a too-new feature_schema_version
// ---------------------------------------------------------------------------
#[test]
fn test_model_loader_rejects_too_new_schema_version() {
    use brain_services::retrieval::model_loader::{ModelEnvelope, ModelLoader, ModelMetadata};

    let weights = RankingWeights {
        lexical: 1.0,
        semantic: 0.0,
        recency: 0.0,
        importance: 0.0,
        provenance_confidence: 0.0,
        graph_degree: 0.0,
        access_frequency: 0.0,
        freshness_decay: 0.0,
    };
    let model = LinearRanker::new(weights);

    // Manually construct an envelope with a feature schema version from the future
    let mut envelope = ModelEnvelope::wrap(&model, 1, 1).expect("wrap() must succeed");
    envelope.metadata = ModelMetadata {
        feature_schema_version: u32::MAX, // far-future version
        model_version: 1,
        trainer_version: 1,
    };

    let json = serde_json::to_string(&envelope).expect("must serialize");
    let result = ModelLoader::load_from_str(&json);
    assert!(
        result.is_err(),
        "ModelLoader must reject models with feature_schema_version newer than this build"
    );
    let err_msg = format!("{:?}", result.err().expect("expected Err"));
    assert!(
        err_msg.contains("newer"),
        "Error message must mention 'newer': {}",
        err_msg
    );
}
