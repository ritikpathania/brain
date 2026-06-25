# Relational Memory Engine Plugin Architecture

`brain` features a modular, plugin-based extensibility architecture. You can customize the engine's core pipelines (from storage and LLMs to custom CLI subcommands) by registering plugins either statically in Rust or dynamically at runtime using Python scripts.

---

## 1. Plugin Configuration (`config.json`)

The active plugin for each component is resolved at runtime using the `~/.brain/config.json` file. If the file does not exist, the daemon automatically creates it with the default built-in configurations.

### Configuration Schema

```json
{
  "active_embedding_provider": "noop",
  "active_llm_provider": "noop",
  "active_retrieval_algorithm": "fuzzy",
  "active_ranking_strategy": "default",
  "active_storage_backend": "sqlite",
  "active_memory_extractor": "python-default",
  "active_exporter": "duckdb"
}
```

---

## 2. Core Traits Specification

All plugins map to one of the following 8 stable interfaces.

| Interface / Trait | Responsibility | Built-in Defaults |
|---|---|---|
| `EmbeddingProvider` | Vector embedding generation | `noop` |
| `LlmProvider` | Text generation & prompts completion | `noop` |
| `RetrievalAlgorithm` | Match candidates in short-term memory | `fuzzy` |
| `RankingStrategy` | Re-rank match candidates | `default` |
| `StorageBackend` | Core transactional graph storage read/write | `sqlite` |
| `MemoryExtractor` | Consolidate text chunks into graphs | `python-default` |
| `Exporter` | Incremental analytical syncer / exporter | `duckdb` |
| `CliPlugin` | Add custom command-line endpoints | *(None)* |

---

## 3. Creating Dynamic Python Plugins

Dynamic Python plugins are loaded at startup. You can place Python files in `~/.brain/plugins/` (e.g. `~/.brain/plugins/my_custom_plugin.py`). The daemon scans this directory, imports each module, calls its `register_plugins()` entrypoint, and registers the returned objects.

### Interface Mapping for Python

Every Python class representing a plugin must implement the expected method signatures. The data exchanged at the boundary is serialized using JSON to ensure high safety.

#### 1. LlmProvider
* **Python API**:
  ```python
  class MyLlm:
      def name(self) -> str:
          return "my-custom-llm"
      def generate(self, prompt: str) -> str:
          return "Response..."
  ```

#### 2. EmbeddingProvider
* **Python API**:
  ```python
  class MyEmbedder:
      def name(self) -> str:
          return "my-custom-embedder"
      def embed(self, text: str) -> list[float]:
          return [0.1, 0.2, 0.3]
  ```

#### 3. RetrievalAlgorithm
* **Python API**:
  ```python
  class MyRetrieval:
      def name(self) -> str:
          return "my-retriever"
      def retrieve(self, query: str, window: list[dict]) -> list[tuple[str, int]]:
          # Returns a list of (node_id, score) pairs
          return [(node["id"], 100) for node in window if query in node["content"]]
  ```

#### 4. RankingStrategy
* **Python API**:
  ```python
  class MyRanking:
      def name(self) -> str:
          return "my-ranker"
      def rank(self, query: str, candidates: list[tuple[dict, int]]) -> list[str]:
          # Re-orders candidates and returns a list of sorted Node IDs
          sorted_candidates = sorted(candidates, key=lambda x: x[1], reverse=True)
          return [pair[0]["id"] for pair in sorted_candidates]
  ```

#### 5. StorageBackend
* **Python API**:
  ```python
  class MyStorage:
      def name(self) -> str:
          return "my-storage"
      def write_graph(self, nodes_json: str, edges_json: str) -> None:
          # Write graph to destination
          pass
      def query_graph(self, query: str) -> str:
          # Returns JSON representing list of (Node, list of Edges)
          return "[]"
      def get_updates_since(self, timestamp: int) -> str:
          # Returns JSON representing (nodes, edges, max_timestamp)
          return "([], [], 0)"
  ```

#### 6. MemoryExtractor
* **Python API**:
  ```python
  class MyExtractor:
      def name(self) -> str:
          return "my-extractor"
      def extract(self, stm_nodes_json: str) -> str:
          # Processes raw STM nodes and returns JSON matching graph schema:
          # {"nodes": [...], "edges": [...]}
          return '{"nodes": [], "edges": []}'
  ```

#### 7. Exporter
* **Python API**:
  ```python
  class MyExporter:
      def name(self) -> str:
          return "my-exporter"
      def get_last_sync_timestamp(self) -> int:
          return 0
      def export_updates(self, nodes_json: str, edges_json: str, max_timestamp: int) -> None:
          # Sync updates to destination
          pass
  ```

#### 8. CliPlugin
* **Python API**:
  ```python
  class MyCliCommand:
      def name(self) -> str:
          return "my-cli-command"
      def get_subcommand_name(self) -> str:
          return "custom-cmd"
      def get_subcommand_description(self) -> str:
          return "Runs my custom subcommand"
      def handle_command(self, args: list[str]) -> None:
          print(f"Executing with arguments: {args}")
  ```

---

## 4. Built-in Example LLM Plugins (`llm_plugins.py`)

A generalized plugin `llm_plugins.py` is included, exposing ready-to-use LLM providers:
- `OllamaLlmProvider`
- `OpenAiLlmProvider` (uses `OPENAI_API_KEY` env var)
- `AnthropicLlmProvider` (uses `ANTHROPIC_API_KEY` env var)

It also includes `LlmMemoryExtractor` which can wrap any of the above to perform LLM-based graph extraction.

To use OpenAI for context graph extraction:
1. Copy `llm_plugins.py` to `~/.brain/plugins/`.
2. Export your API key: `export OPENAI_API_KEY="your-key"`.
3. Set `"active_memory_extractor": "openai-extractor"` in `~/.brain/config.json`.

---

## 5. Engineering Safety: Preventing Blocking in Hot Paths

To ensure sub-millisecond latencies on core UDS operations:
1. Any Dynamic Python plugin hook execution automatically acquires the CPython GIL.
2. The dispatcher wraps Python and database transactions inside `tokio::task::spawn_blocking` pools. This offloads CPU-heavy NLP, FFI boundaries, and external HTTP network requests completely off the core async socket thread pool, preventing pipeline starvation.
