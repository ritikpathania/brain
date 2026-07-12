import json
import os
from pathlib import Path

import pytest
from jsonschema import Draft7Validator, RefResolver, ValidationError

from brain_sdk.models import IngestionEnvelope


@pytest.fixture
def schema_validator():
    schema_dir = Path(__file__).resolve().parents[3] / "protocol" / "schema" / "v1"
    envelope_schema_path = schema_dir / "ingestion_envelope.schema.json"

    with open(envelope_schema_path) as f:
        envelope_schema = json.load(f)

    base_uri = envelope_schema_path.as_uri()
    resolver = RefResolver(base_uri=base_uri, referrer=envelope_schema)
    return Draft7Validator(envelope_schema, resolver=resolver)

def test_compat_positive_fixtures(schema_validator):
    root_dir = Path(__file__).resolve().parents[3]
    valid_dir = root_dir / "protocol" / "fixtures" / "v1" / "valid"
    
    for filename in os.listdir(valid_dir):
        if not filename.endswith(".json"):
            continue
        filepath = valid_dir / filename
        with open(filepath) as f:
            fixture_str = f.read().strip()
        fixture_val = json.loads(fixture_str)

        # 1. Pre-validate against JSON schema
        schema_validator.validate(fixture_val)

        # 2. Deserialize in SDK
        envelope = IngestionEnvelope.from_dict(fixture_val)

        # 3. Canonical Re-serialize
        serialized_str = json.dumps(
            envelope.to_dict(), sort_keys=True, separators=(",", ":")
        )

        # 4. Assert byte-for-byte identical
        assert serialized_str == fixture_str, f"Byte-for-byte mismatch on {filename}"

def test_compat_negative_fixtures(schema_validator):
    root_dir = Path(__file__).resolve().parents[3]
    invalid_dir = root_dir / "protocol" / "fixtures" / "v1" / "invalid"
    
    for filename in os.listdir(invalid_dir):
        if not filename.endswith(".json"):
            continue
        filepath = invalid_dir / filename
        with open(filepath) as f:
            fixture_str = f.read()
        fixture_val = json.loads(fixture_str)

        # 1. Assert it fails JSON schema validation
        with pytest.raises(ValidationError):
            schema_validator.validate(fixture_val)

        # 2. Assert it fails SDK deserialization
        with pytest.raises((KeyError, TypeError, ValueError)):
            IngestionEnvelope.from_dict(fixture_val)

def test_compat_forward_compatibility(schema_validator):
    root_dir = Path(__file__).resolve().parents[3]
    fixtures_dir = root_dir / "protocol" / "fixtures" / "v1" / "forward"
    
    # 1. unknown_fields.json
    unknown_fields_path = fixtures_dir / "unknown_fields.json"
    with open(unknown_fields_path) as f:
        fixture_str = f.read()
    fixture_val = json.loads(fixture_str)

    # Schema validation must pass
    schema_validator.validate(fixture_val)

    # SDK Deserialization must pass and ignore unknown fields
    envelope = IngestionEnvelope.from_dict(fixture_val)
    
    # Re-serialization must compile cleanly and match schema
    serialized_dict = envelope.to_dict()
    schema_validator.validate(serialized_dict)

    # 2. unknown_event_type.json
    unknown_event_path = fixtures_dir / "unknown_event_type.json"
    with open(unknown_event_path) as f:
        fixture_str = f.read()
    fixture_val = json.loads(fixture_str)

    # Schema validation must pass
    schema_validator.validate(fixture_val)

    # SDK deserialization: since Python SDK models do not type-restrict
    # `event` fields (they are represented as JSONDict/dict), the Python SDK
    # will deserialize it successfully. Re-serialization is also valid.
    envelope2 = IngestionEnvelope.from_dict(fixture_val)
    schema_validator.validate(envelope2.to_dict())
