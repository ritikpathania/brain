import json

from brain.providers.embeddings import (
    LocalEmbeddingProvider,
    OllamaEmbeddingProvider,
    OpenAiCompatibleEmbeddingProvider,
    OpenAiEmbeddingProvider,
)
from brain.providers.llm import (
    AnthropicLlmProvider,
    OllamaLlmProvider,
    OpenAiLlmProvider,
)


class LlmMemoryExtractor:
    def __init__(self, provider, name: str):
        self.provider = provider
        self.extractor_name = name

    def name(self) -> str:
        return self.extractor_name

    def extract(self, stm_nodes_json: str) -> str:
        try:
            stm_nodes = json.loads(stm_nodes_json)
        except Exception as e:
            return json.dumps(
                {
                    "status": "error",
                    "message": f"Failed to parse input STM nodes: {e}",
                    "nodes": [],
                    "edges": [],
                }
            )

        prompt = (
            "You are a knowledge graph extractor. Extract semantic entities "
            "(nodes) and their relations (edges) from the following "
            "short-term memory transcripts. Return ONLY a valid JSON object "
            "matching this schema:\n"
            '{"nodes": [{"id": "entity-id", "label": "Entity Label", '
            '"type": "entity-type", "attributes": {}}], '
            '"edges": [{"source": "entity-id", "target": "entity-id", '
            '"relation": "relationship_name"}]}\n\n'
            f"Transcripts:\n{json.dumps(stm_nodes, indent=2)}\n\n"
            "JSON output:"
        )

        llm_response = self.provider.generate(prompt)

        # Basic JSON block cleaning/extraction helper
        try:
            # Strip markdown block wrappers if LLM returned them
            cleaned = llm_response.strip()
            if cleaned.startswith("```json"):
                cleaned = cleaned[7:]
            elif cleaned.startswith("```"):
                cleaned = cleaned[3:]
            if cleaned.endswith("```"):
                cleaned = cleaned[:-3]
            cleaned = cleaned.strip()

            graph = json.loads(cleaned)
            return json.dumps(
                {
                    "status": "success",
                    "nodes": graph.get("nodes", []),
                    "edges": graph.get("edges", []),
                }
            )
        except Exception as e:
            return json.dumps(
                {
                    "status": "error",
                    "message": f"LLM parsing failed: {e}. Raw response: {llm_response}",
                    "nodes": [],
                    "edges": [],
                }
            )


def register_plugins():
    ollama = OllamaLlmProvider()
    openai = OpenAiLlmProvider()
    anthropic = AnthropicLlmProvider()

    ollama_emb = OllamaEmbeddingProvider()
    openai_emb = OpenAiEmbeddingProvider()
    local_emb = LocalEmbeddingProvider()
    custom_emb = OpenAiCompatibleEmbeddingProvider()

    return {
        "api_version": "1.0",
        "minimum_brain_version": "0.1.0",
        "plugin_version": "1.0.0",
        "llm_providers": [ollama, openai, anthropic],
        "embedding_providers": [ollama_emb, openai_emb, local_emb, custom_emb],
        "memory_extractors": [
            LlmMemoryExtractor(ollama, name="ollama-extractor"),
            LlmMemoryExtractor(openai, name="openai-extractor"),
            LlmMemoryExtractor(anthropic, name="anthropic-extractor"),
        ],
    }
