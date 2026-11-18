"""Versioned UDS integration coverage against an isolated real daemon."""


def test_versioned_status_uses_an_isolated_daemon(uds_runtime):
    response = uds_runtime.call("v1/status", "")

    assert response["version"] == "1.0"
    assert response["type"] == "Response"
    assert response["status"] == "success"


def test_unknown_versioned_action_returns_a_typed_error(uds_runtime):
    response = uds_runtime.call("v1/not-a-real-action", "")

    assert response["version"] == "1.0"
    assert response["type"] == "Error"
    assert response["status"] == "error"
