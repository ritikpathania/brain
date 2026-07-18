import json
import os

import pytest
from jsonschema import Draft7Validator, RefResolver, ValidationError

SCHEMA_PATH = os.path.abspath(
    os.path.join(os.path.dirname(__file__), "../../protocol/uds_ipc.schema.json")
)


@pytest.fixture(scope="module")
def response_validator():
    with open(SCHEMA_PATH) as f:
        schema = json.load(f)
    resolver = RefResolver.from_schema(schema)
    # Draft7Validator compiled against the specific Response definition
    validator = Draft7Validator(schema["$defs"]["Response"], resolver=resolver)
    return validator


# --- Positive Conformance Cases ---


def test_legacy_response_success(response_validator):
    resp = {"status": "ok", "message": "Ingested successfully"}
    response_validator.validate(resp)


def test_legacy_error_response_success(response_validator):
    resp = {"status": "error", "message": "Malformed request"}
    response_validator.validate(resp)


def test_versioned_response_success(response_validator):
    resp = {
        "version": "1.0",
        "type": "Response",
        "id": 42,
        "status": "success",
        "body": "Found 1 matches",
    }
    response_validator.validate(resp)


def test_versioned_response_with_extensions(response_validator):
    resp = {
        "version": "1.0",
        "type": "Response",
        "id": 43,
        "status": "success",
        "body": "Found matches",
        "ext": {"execution_mode": "embedded"},
    }
    response_validator.validate(resp)


def test_versioned_error_success(response_validator):
    resp = {
        "version": "1.0",
        "type": "Error",
        "id": 44,
        "status": "error",
        "body": "Database lock timeout",
    }
    response_validator.validate(resp)


def test_versioned_notification_success(response_validator):
    resp = {
        "version": "1.0",
        "type": "Notification",
        "notification_type": "sync_complete",
        "message": "Analytics database synced successfully",
    }
    response_validator.validate(resp)


def test_versioned_event_success(response_validator):
    resp = {
        "version": "1.0",
        "type": "Event",
        "event_name": "epoch_rotated",
        "payload": {"epoch": 5},
    }
    response_validator.validate(resp)


# --- Negative Conformance Cases ---


def test_legacy_response_missing_required(response_validator):
    # Missing 'message'
    resp = {"status": "ok"}
    with pytest.raises(ValidationError):
        response_validator.validate(resp)


def test_legacy_response_extraneous_properties(response_validator):
    # extra field not allowed
    resp = {"status": "ok", "message": "Ingested", "extraneous": True}
    with pytest.raises(ValidationError):
        response_validator.validate(resp)


def test_versioned_response_missing_required(response_validator):
    # Missing 'status'
    resp = {"version": "1.0", "type": "Response", "id": 45, "body": "No status"}
    with pytest.raises(ValidationError):
        response_validator.validate(resp)


def test_versioned_response_invalid_version(response_validator):
    resp = {
        "version": "1.1",
        "type": "Response",
        "id": 46,
        "status": "success",
        "body": "Invalid version",
    }
    with pytest.raises(ValidationError):
        response_validator.validate(resp)


def test_versioned_response_invalid_status_enum(response_validator):
    # status must be success/error, not "ok"
    resp = {
        "version": "1.0",
        "type": "Response",
        "id": 47,
        "status": "ok",
        "body": "Invalid status",
    }
    with pytest.raises(ValidationError):
        response_validator.validate(resp)


def test_versioned_error_invalid_status(response_validator):
    # For Error type, status must be exactly "error"
    resp = {
        "version": "1.0",
        "type": "Error",
        "id": 48,
        "status": "success",
        "body": "Invalid status for Error",
    }
    with pytest.raises(ValidationError):
        response_validator.validate(resp)


def test_versioned_notification_missing_required(response_validator):
    # Missing 'notification_type'
    resp = {"version": "1.0", "type": "Notification", "message": "Syncing..."}
    with pytest.raises(ValidationError):
        response_validator.validate(resp)


def test_versioned_event_missing_required(response_validator):
    # Missing 'payload'
    resp = {"version": "1.0", "type": "Event", "event_name": "epoch_rotated"}
    with pytest.raises(ValidationError):
        response_validator.validate(resp)


def test_versioned_response_invalid_field_type(response_validator):
    # id is string instead of integer
    resp = {
        "version": "1.0",
        "type": "Response",
        "id": "49",
        "status": "success",
        "body": "Invalid ID type",
    }
    with pytest.raises(ValidationError):
        response_validator.validate(resp)
