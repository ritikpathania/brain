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


def test_classification_and_property_extraction():
    payload = json.dumps(
        [
            {
                "id": "node-3",
                "epoch": 0,
                "content": (
                    "Brain is an AI agent engine. DuckDB is a SQL database engine."
                ),
                "timestamp": 1002,
            }
        ]
    )
    res = json.loads(extract_semantic_nodes(payload))
    assert res["status"] == "success"

    nodes_dict = {n["id"]: n for n in res["nodes"]}
    assert "brain" in nodes_dict
    assert "duckdb" in nodes_dict

    # Verify attributes (category, type)
    assert nodes_dict["brain"]["attributes"].get("category") == "AI agent engine"
    assert nodes_dict["brain"]["attributes"].get("type") == "AI agent engine"
    assert nodes_dict["duckdb"]["attributes"].get("category") == "SQL database engine"
    assert nodes_dict["duckdb"]["attributes"].get("type") == "SQL database engine"

    # Verify concept nodes were extracted
    assert "ai-agent-engine" in nodes_dict
    assert "sql-database-engine" in nodes_dict

    # Verify edge was created
    edges = [(e["source"], e["target"], e["relation"]) for e in res["edges"]]
    assert ("brain", "ai-agent-engine", "associated_with") in edges
    assert ("duckdb", "sql-database-engine", "associated_with") in edges


def test_relation_sentence_parsing():
    payload = json.dumps(
        [
            {
                "id": "node-4",
                "epoch": 0,
                "content": (
                    "ritikpathania develops Brain. SQLite is stored in config.toml. "
                    "Brain runs on Docker. Brain depends on Python."
                ),
                "timestamp": 1003,
            }
        ]
    )
    res = json.loads(extract_semantic_nodes(payload))
    assert res["status"] == "success"

    edges = [(e["source"], e["target"], e["relation"]) for e in res["edges"]]

    # Check relation: develops (ritikpathania -> brain)
    assert ("ritikpathania", "brain", "develops") in edges

    # Check relation: stored_in (sqlite -> config.toml)
    assert ("sqlite", "config.toml", "stored_in") in edges

    # Check relation: runs_on (brain -> docker)
    assert ("brain", "docker", "runs_on") in edges

    # Check relation: depends_on (brain -> python)
    assert ("brain", "python", "depends_on") in edges


def test_synonym_normalization_and_merging():
    payload = json.dumps(
        [
            {
                "id": "node-5",
                "epoch": 0,
                "content": (
                    "We are using Postgres on the server. "
                    "John was configuring PostgreSQL for the app."
                ),
                "timestamp": 1004,
            }
        ]
    )
    res = json.loads(extract_semantic_nodes(payload))
    assert res["status"] == "success"

    # Verify that both 'Postgres' and 'PostgreSQL' normalise to 'postgresql'
    nodes_dict = {n["id"]: n for n in res["nodes"]}
    assert "postgresql" in nodes_dict
    assert nodes_dict["postgresql"]["label"] == "PostgreSQL"
    assert nodes_dict["postgresql"]["type"] == "technology"

    # Should only have one 'postgresql' node (merged)
    postgres_nodes = [n for n in res["nodes"] if n["id"] == "postgresql"]
    assert len(postgres_nodes) == 1


def test_sequential_associated_with_reproduction():
    payload = json.dumps(
        [
            {
                "id": "node-reprod-1",
                "epoch": 0,
                "content": "We are migrating our services from Python to Rust.",
                "timestamp": 1005,
            }
        ]
    )
    res = json.loads(extract_semantic_nodes(payload))
    edges = [(e["source"], e["target"], e["relation"]) for e in res["edges"]]
    assert ("python", "rust", "associated_with") in edges
    # No self-loops
    assert ("python", "python", "associated_with") not in edges
    assert ("rust", "rust", "associated_with") not in edges


def test_sequential_associated_with_multiple():
    payload = json.dumps(
        [
            {
                "id": "node-reprod-2",
                "epoch": 0,
                "content": "Python Rust Go",
                "timestamp": 1006,
            }
        ]
    )
    res = json.loads(extract_semantic_nodes(payload))
    edges = [(e["source"], e["target"], e["relation"]) for e in res["edges"]]
    assert ("python", "rust", "associated_with") in edges
    assert ("rust", "go", "associated_with") in edges
    assert len(edges) == 2


def test_sequential_associated_with_single():
    payload = json.dumps(
        [
            {
                "id": "node-reprod-3",
                "epoch": 0,
                "content": "Rust",
                "timestamp": 1007,
            }
        ]
    )
    res = json.loads(extract_semantic_nodes(payload))
    edges = [(e["source"], e["target"], e["relation"]) for e in res["edges"]]
    assert len(edges) == 0


def test_sequential_associated_with_duplicate_cycle():
    payload = json.dumps(
        [
            {
                "id": "node-reprod-4",
                "epoch": 0,
                "content": "Rust Python Rust",
                "timestamp": 1008,
            }
        ]
    )
    res = json.loads(extract_semantic_nodes(payload))
    edges = [(e["source"], e["target"], e["relation"]) for e in res["edges"]]
    assert ("rust", "python", "associated_with") in edges
    assert ("python", "rust", "associated_with") in edges
    assert ("rust", "rust", "associated_with") not in edges
    assert ("python", "python", "associated_with") not in edges


def test_predefined_relation_precedence():
    payload = json.dumps(
        [
            {
                "id": "node-reprod-5",
                "epoch": 0,
                "content": (
                    "The Rust application communicates via a Unix Domain Socket."
                ),
                "timestamp": 1009,
            }
        ]
    )
    res = json.loads(extract_semantic_nodes(payload))
    edges = [(e["source"], e["target"], e["relation"]) for e in res["edges"]]
    assert ("rust", "uds", "communicates_via") in edges
    assert ("rust", "uds", "associated_with") not in edges


def test_multi_word_proper_noun_regression():
    payload = json.dumps(
        [
            {
                "id": "node-reprod-6",
                "epoch": 0,
                "content": "Visual Studio Code Rust",
                "timestamp": 1010,
            }
        ]
    )
    res = json.loads(extract_semantic_nodes(payload))
    nodes_dict = {n["id"]: n for n in res["nodes"]}
    assert "visual-studio-code" in nodes_dict
    assert "rust" in nodes_dict

    edges = [(e["source"], e["target"], e["relation"]) for e in res["edges"]]
    assert ("visual-studio-code", "rust", "associated_with") in edges


def test_multi_word_proper_noun_regression_vsc():
    payload = json.dumps(
        [
            {
                "id": "node-reprod-7",
                "epoch": 0,
                "content": "The project uses Visual Studio Code.",
                "timestamp": 1011,
            }
        ]
    )
    res = json.loads(extract_semantic_nodes(payload))
    nodes_dict = {n["id"]: n for n in res["nodes"]}

    # Visual Studio Code should be extracted exactly as a single proper noun concept
    assert "visual-studio-code" in nodes_dict
    assert nodes_dict["visual-studio-code"]["label"] == "Visual Studio Code"
    assert nodes_dict["visual-studio-code"]["type"] == "concept"

    # Ensure no duplicate nodes or fragment concepts were emitted
    assert "visual" not in nodes_dict
    assert "studio" not in nodes_dict
    assert "code" not in nodes_dict


def test_predefined_relation_dominance_suppresses_fallback():
    payload = json.dumps(
        [
            {
                "id": "node-reprod-8",
                "epoch": 0,
                "content": (
                    "The Rust application communicates via a Unix Domain Socket. "
                    "The Unix Domain Socket Rust is fast."
                ),
                "timestamp": 1012,
            }
        ]
    )
    res = json.loads(extract_semantic_nodes(payload))
    edges = [(e["source"], e["target"], e["relation"]) for e in res["edges"]]

    # We should have the predefined communicates_via edge
    assert ("rust", "uds", "communicates_via") in edges
    # We should NOT have a redundant fallback associated_with edge in the
    # opposite direction
    assert ("uds", "rust", "associated_with") not in edges
    # Exactly one edge should be present between this pair
    assert len(edges) == 1
