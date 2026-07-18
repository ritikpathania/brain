use brain_integrations::IngestionEnvelope;
use std::path::Path;

#[test]
fn test_protocol_compatibility() {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let schema_dir = manifest_dir.join("../../protocol/schema/v1");

    let identity_schema_str =
        std::fs::read_to_string(schema_dir.join("event_identity.schema.json")).unwrap();
    let envelope_schema_str =
        std::fs::read_to_string(schema_dir.join("ingestion_envelope.schema.json")).unwrap();

    let identity_val: serde_json::Value = serde_json::from_str(&identity_schema_str).unwrap();
    let envelope_val: serde_json::Value = serde_json::from_str(&envelope_schema_str).unwrap();

    let validator = jsonschema::JSONSchema::options()
        .with_document(
            "json-schema:///event_identity.schema.json".to_string(),
            identity_val,
        )
        .compile(&envelope_val)
        .expect("Failed to compile JSON Schema");

    // 1. Positive Fixtures
    let valid_dir = manifest_dir.join("../../protocol/fixtures/v1/valid");
    for entry in std::fs::read_dir(valid_dir).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        if path.extension().is_some_and(|ext| ext == "json") {
            let fixture_str = std::fs::read_to_string(&path).unwrap();
            let fixture_val: serde_json::Value = serde_json::from_str(&fixture_str).unwrap();

            // Schema pre-validation
            if let Err(errors) = validator.validate(&fixture_val) {
                for error in errors {
                    println!("Validation error on {:?}: {}", path, error);
                }
                panic!("Fixture {:?} failed schema validation", path);
            }

            // SDK Deserialization
            let envelope: IngestionEnvelope =
                serde_json::from_str(&fixture_str).unwrap_or_else(|e| {
                    panic!("Failed to deserialize valid fixture {:?}: {}", path, e)
                });

            // SDK Canonical Re-serialization
            let serialized_canonical = brain_integrations::to_canonical_json(&envelope).unwrap();

            // Byte-for-byte identical
            assert_eq!(
                serialized_canonical.trim(),
                fixture_str.trim(),
                "Byte-for-byte serialization mismatch on {:?}",
                path
            );
        }
    }

    // 2. Negative Fixtures
    let invalid_dir = manifest_dir.join("../../protocol/fixtures/v1/invalid");
    for entry in std::fs::read_dir(invalid_dir).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        if path.extension().is_some_and(|ext| ext == "json") {
            let fixture_str = std::fs::read_to_string(&path).unwrap();
            let fixture_val: serde_json::Value = serde_json::from_str(&fixture_str).unwrap();

            // Schema pre-validation (must fail)
            assert!(
                !validator.is_valid(&fixture_val),
                "Invalid fixture {:?} unexpectedly passed schema validation",
                path
            );

            // SDK Deserialization (must fail cleanly)
            let res: Result<IngestionEnvelope, _> = serde_json::from_str(&fixture_str);
            assert!(
                res.is_err(),
                "Invalid fixture {:?} unexpectedly deserialized successfully in SDK",
                path
            );
        }
    }

    // 3. Forward Compatibility - Unknown Fields
    let unknown_fields_path =
        manifest_dir.join("../../protocol/fixtures/v1/forward/unknown_fields.json");
    let unknown_fields_str = std::fs::read_to_string(&unknown_fields_path).unwrap();
    let unknown_fields_val: serde_json::Value = serde_json::from_str(&unknown_fields_str).unwrap();

    // Schema validation (must pass because schemas allow additional properties)
    assert!(
        validator.is_valid(&unknown_fields_val),
        "unknown_fields.json failed schema validation"
    );

    // SDK Deserialization (must pass and ignore unknown fields)
    let envelope: IngestionEnvelope = serde_json::from_str(&unknown_fields_str)
        .expect("SDK failed to deserialize unknown_fields.json");

    // SDK Canonical Re-serialization must compile cleanly
    let serialized = brain_integrations::to_canonical_json(&envelope).unwrap();
    let serialized_val: serde_json::Value = serde_json::from_str(&serialized).unwrap();
    assert!(validator.is_valid(&serialized_val));

    // 4. Forward Compatibility - Unknown Event Type
    let unknown_event_path =
        manifest_dir.join("../../protocol/fixtures/v1/forward/unknown_event_type.json");
    let unknown_event_str = std::fs::read_to_string(&unknown_event_path).unwrap();
    let unknown_event_val: serde_json::Value = serde_json::from_str(&unknown_event_str).unwrap();

    // Schema validation (must pass)
    assert!(
        validator.is_valid(&unknown_event_val),
        "unknown_event_type.json failed schema validation"
    );

    // SDK Deserialization (fails cleanly / returns error rather than panicking/crashing)
    let res: Result<IngestionEnvelope, _> = serde_json::from_str(&unknown_event_str);
    assert!(
        res.is_err(),
        "unknown_event_type.json should fail typed deserialization cleanly"
    );
}
