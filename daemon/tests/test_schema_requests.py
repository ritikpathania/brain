import json
import os

import pytest
from jsonschema import Draft7Validator, RefResolver, ValidationError

SCHEMA_PATH = os.path.abspath(
    os.path.join(os.path.dirname(__file__), "../../protocol/uds_ipc.schema.json")
)


@pytest.fixture(scope="module")
def request_validator():
    with open(SCHEMA_PATH) as f:
        schema = json.load(f)
    resolver = RefResolver.from_schema(schema)
    # Draft7Validator compiled against the specific Request definition
    validator = Draft7Validator(schema["$defs"]["Request"], resolver=resolver)
    return validator


# --- Positive Conformance Cases ---


def test_versioned_request_success(request_validator):
    req = {
        "version": "1.0",
        "type": "Request",
        "id": 123,
        "action": "query",
        "body": "sqlite",
    }
    request_validator.validate(req)


def test_versioned_request_with_extensions(request_validator):
    req = {
        "version": "1.0",
        "type": "Request",
        "id": 124,
        "action": "ingest",
        "body": "learning rust",
        "ext": {"custom_flag": True, "priority": "high"},
    }
    request_validator.validate(req)


def test_legacy_request_success(request_validator):
    req = {"action": "query", "payload": "sqlite database"}
    request_validator.validate(req)


# --- Negative Conformance Cases ---


def test_versioned_request_missing_required(request_validator):
    # Missing 'body'
    req = {"version": "1.0", "type": "Request", "id": 125, "action": "query"}
    with pytest.raises(ValidationError):
        request_validator.validate(req)


def test_versioned_request_invalid_version(request_validator):
    # version is "2.0" instead of "1.0"
    req = {
        "version": "2.0",
        "type": "Request",
        "id": 126,
        "action": "query",
        "body": "sqlite",
    }
    with pytest.raises(ValidationError):
        request_validator.validate(req)


def test_versioned_request_invalid_type_field(request_validator):
    # type is "Req" instead of "Request"
    req = {
        "version": "1.0",
        "type": "Req",
        "id": 127,
        "action": "query",
        "body": "sqlite",
    }
    with pytest.raises(ValidationError):
        request_validator.validate(req)


def test_versioned_request_invalid_action(request_validator):
    # action is "delete" which is not in query/ingest enum
    req = {
        "version": "1.0",
        "type": "Request",
        "id": 128,
        "action": "delete",
        "body": "sqlite",
    }
    with pytest.raises(ValidationError):
        request_validator.validate(req)


def test_versioned_request_invalid_field_type(request_validator):
    # body is an integer instead of string
    req = {
        "version": "1.0",
        "type": "Request",
        "id": 129,
        "action": "query",
        "body": 42,
    }
    with pytest.raises(ValidationError):
        request_validator.validate(req)


def test_versioned_request_extraneous_properties(request_validator):
    # 'extra_field' is present with additionalProperties: false
    req = {
        "version": "1.0",
        "type": "Request",
        "id": 130,
        "action": "query",
        "body": "sqlite",
        "extra_field": "unallowed",
    }
    with pytest.raises(ValidationError):
        request_validator.validate(req)


def test_legacy_request_missing_required(request_validator):
    # Missing 'payload'
    req = {"action": "query"}
    with pytest.raises(ValidationError):
        request_validator.validate(req)


def test_legacy_request_extraneous_properties(request_validator):
    # 'extra' field on legacy request
    req = {"action": "query", "payload": "sqlite", "extra": "unallowed"}
    with pytest.raises(ValidationError):
        request_validator.validate(req)
