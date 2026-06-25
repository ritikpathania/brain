import json

import pytest
from hypothesis import given
from hypothesis import strategies as st

from brain.extraction.heuristics import extract_semantic_nodes


@pytest.fixture
def empty_log_payload():
    return "[]"


@pytest.fixture
def tech_log_payload():
    return json.dumps(
        [
            {
                "id": "node-1",
                "epoch": 0,
                "content": "setting up sqlite database configuration",
                "timestamp": 1000,
            }
        ]
    )


@pytest.fixture
def credential_log_payload():
    return json.dumps(
        [
            {
                "id": "node-2",
                "epoch": 0,
                "content": "the API keys should be stored in environment variables",
                "timestamp": 1001,
            }
        ]
    )


def test_empty_input(empty_log_payload):
    res = json.loads(extract_semantic_nodes(empty_log_payload))
    assert res["status"] == "success"
    assert len(res["nodes"]) == 0
    assert len(res["edges"]) == 0


def test_tech_and_database_config_extraction(tech_log_payload):
    res = json.loads(extract_semantic_nodes(tech_log_payload))
    assert res["status"] == "success"
    node_ids = [n["id"] for n in res["nodes"]]
    assert "sqlite" in node_ids
    assert "db-config" in node_ids
    assert len(res["edges"]) == 1
    assert res["edges"][0]["source"] == "db-config"
    assert res["edges"][0]["target"] == "sqlite"
    assert res["edges"][0]["relation"] == "configures"


def test_credential_extraction(credential_log_payload):
    res = json.loads(extract_semantic_nodes(credential_log_payload))
    assert res["status"] == "success"
    node_ids = [n["id"] for n in res["nodes"]]
    assert "api-key" in node_ids
    assert "env-vars" in node_ids
    assert len(res["edges"]) == 1
    assert res["edges"][0]["source"] == "api-key"
    assert res["edges"][0]["target"] == "env-vars"
    assert res["edges"][0]["relation"] == "stored_in"


@given(st.text())
def test_extractor_robustness_random_strings(input_str):
    # Fuzzing arbitrary string input - it should either parse or return a
    # graceful failure/empty result without throwing unhandled exceptions
    try:
        res_str = extract_semantic_nodes(input_str)
        res = json.loads(res_str)
        assert "status" in res
    except (json.JSONDecodeError, ValueError, TypeError):
        pass


@given(
    st.lists(
        st.fixed_dictionaries(
            {
                "id": st.text(),
                "epoch": st.integers(min_value=0, max_value=10000),
                "content": st.text(),
                "timestamp": st.integers(min_value=0),
            }
        )
    )
)
def test_extractor_robustness_valid_json_structure(log_list):
    input_data = json.dumps(log_list)
    res_str = extract_semantic_nodes(input_data)
    res = json.loads(res_str)
    assert res["status"] == "success"
    assert "nodes" in res
    assert "edges" in res


def test_embedding_providers_instantiation():
    from brain.providers.embeddings import (
        LocalEmbeddingProvider,
        OllamaEmbeddingProvider,
        OpenAiCompatibleEmbeddingProvider,
        OpenAiEmbeddingProvider,
    )

    # Test Ollama
    ollama = OllamaEmbeddingProvider()
    assert ollama.name() == "ollama-nomic-embed-text"
    # Ensure fallback works on connection failure
    assert len(ollama.embed("test")) == 384

    # Test OpenAI
    openai = OpenAiEmbeddingProvider()
    assert openai.name() == "openai-text-embedding-3-small"
    # Env var is not set, should return default fallback vector
    assert len(openai.embed("test")) == 384

    # Test Local MiniLM
    local = LocalEmbeddingProvider()
    assert local.name() == "local-all-MiniLM-L6-v2"
    assert len(local.embed("test")) == 384

    # Test Custom OpenAI-Compatible
    custom = OpenAiCompatibleEmbeddingProvider()
    assert custom.name() == "custom-custom-model"
    assert len(custom.embed("test")) == 384


def test_plugin_registration_contains_embeddings():
    from brain.plugins.llm_plugins import register_plugins

    plugins = register_plugins()
    assert "embedding_providers" in plugins
    assert len(plugins["embedding_providers"]) == 4
    names = [p.name() for p in plugins["embedding_providers"]]
    assert "ollama-nomic-embed-text" in names
    assert "openai-text-embedding-3-small" in names
    assert "local-all-MiniLM-L6-v2" in names
    assert "custom-custom-model" in names
