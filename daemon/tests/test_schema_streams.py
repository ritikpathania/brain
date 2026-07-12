import json
import os
import pytest
from jsonschema import RefResolver, Draft7Validator, ValidationError

SCHEMA_PATH = os.path.abspath(os.path.join(os.path.dirname(__file__), "../../protocol/uds_ipc.schema.json"))

@pytest.fixture(scope="module")
def stream_validator():
    with open(SCHEMA_PATH) as f:
        schema = json.load(f)
    resolver = RefResolver.from_schema(schema)
    # Draft7Validator compiled against the specific StreamEvent definition
    validator = Draft7Validator(schema["$defs"]["StreamEvent"], resolver=resolver)
    return validator

# --- Positive Conformance Cases ---

def test_stream_start_success(stream_validator):
    event = {
        "type": "stream_start",
        "streamId": "stream-123"
    }
    stream_validator.validate(event)

def test_stream_start_with_metadata_and_extensions(stream_validator):
    event = {
        "type": "stream_start",
        "streamId": "stream-123",
        "metadata": {
            "client_timestamp": 123456789,
            "latency_ms": 1.5
        },
        "ext": {
            "custom_theme": "dark"
        }
    }
    stream_validator.validate(event)

def test_stream_progress_success(stream_validator):
    event = {
        "type": "stream_progress",
        "streamId": "stream-123",
        "sequence": 1,
        "progress": 0.42,
        "message": "indexing..."
    }
    stream_validator.validate(event)

def test_stream_chunk_success(stream_validator):
    event = {
        "type": "stream_chunk",
        "streamId": "stream-123",
        "sequence": 2,
        "content": "rust node matched"
    }
    stream_validator.validate(event)

def test_stream_end_success(stream_validator):
    event = {
        "type": "stream_end",
        "streamId": "stream-123",
        "sequence": 3
    }
    stream_validator.validate(event)

def test_stream_cancelled_success(stream_validator):
    event = {
        "type": "stream_cancelled",
        "streamId": "stream-123",
        "sequence": 3
    }
    stream_validator.validate(event)

# --- Negative Conformance Cases ---

def test_stream_event_missing_stream_id(stream_validator):
    event = {
        "type": "stream_start"
    }
    with pytest.raises(ValidationError):
        stream_validator.validate(event)

def test_stream_progress_missing_required(stream_validator):
    # Missing 'progress' and 'message'
    event = {
        "type": "stream_progress",
        "streamId": "stream-123",
        "sequence": 1
    }
    with pytest.raises(ValidationError):
        stream_validator.validate(event)

def test_stream_progress_invalid_bounds(stream_validator):
    # progress is > 1.0
    event = {
        "type": "stream_progress",
        "streamId": "stream-123",
        "sequence": 1,
        "progress": 1.5,
        "message": "indexing..."
    }
    with pytest.raises(ValidationError):
        stream_validator.validate(event)

def test_stream_progress_invalid_negative_bounds(stream_validator):
    # progress is < 0.0
    event = {
        "type": "stream_progress",
        "streamId": "stream-123",
        "sequence": 1,
        "progress": -0.1,
        "message": "indexing..."
    }
    with pytest.raises(ValidationError):
        stream_validator.validate(event)

def test_stream_chunk_invalid_sequence_type(stream_validator):
    # sequence is float instead of integer
    event = {
        "type": "stream_chunk",
        "streamId": "stream-123",
        "sequence": 2.5,
        "content": "rust node matched"
    }
    with pytest.raises(ValidationError):
        stream_validator.validate(event)

def test_stream_chunk_extraneous_properties(stream_validator):
    # extraneous field on stream_chunk
    event = {
        "type": "stream_chunk",
        "streamId": "stream-123",
        "sequence": 2,
        "content": "rust node matched",
        "invalid_field": 123
    }
    with pytest.raises(ValidationError):
        stream_validator.validate(event)

def test_stream_invalid_type_enum(stream_validator):
    # type field has invalid value
    event = {
        "type": "stream_unknown",
        "streamId": "stream-123"
    }
    with pytest.raises(ValidationError):
        stream_validator.validate(event)
