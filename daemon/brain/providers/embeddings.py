import json
import os
import urllib.error
import urllib.request


class OllamaEmbeddingProvider:
    def __init__(
        self, model: str = "nomic-embed-text", base_url: str = "http://localhost:11434"
    ):
        self.model = model
        self.base_url = base_url.rstrip("/")

    def name(self) -> str:
        return f"ollama-{self.model}"

    def embed(self, text: str) -> list[float]:
        # Try /api/embed first
        url = f"{self.base_url}/api/embed"
        data = json.dumps({"model": self.model, "input": text}).encode("utf-8")
        req = urllib.request.Request(
            url, data=data, headers={"Content-Type": "application/json"}
        )
        try:
            with urllib.request.urlopen(req, timeout=10) as response:
                resp = json.loads(response.read().decode("utf-8"))
                if "embeddings" in resp and len(resp["embeddings"]) > 0:
                    return resp["embeddings"][0]
        except Exception:
            pass

        # Fallback to /api/embeddings
        url = f"{self.base_url}/api/embeddings"
        data = json.dumps({"model": self.model, "prompt": text}).encode("utf-8")
        req = urllib.request.Request(
            url, data=data, headers={"Content-Type": "application/json"}
        )
        try:
            with urllib.request.urlopen(req, timeout=10) as response:
                resp = json.loads(response.read().decode("utf-8"))
                return resp.get("embedding", [0.0] * 384)
        except Exception as e:
            print(f"Ollama embedding error: {e}")
            return [0.0] * 384


class OpenAiEmbeddingProvider:
    def __init__(self, model: str = "text-embedding-3-small"):
        self.model = model

    def name(self) -> str:
        return f"openai-{self.model}"

    def embed(self, text: str) -> list[float]:
        api_key = os.environ.get("OPENAI_API_KEY")
        if not api_key:
            print("Error: OPENAI_API_KEY environment variable not set.")
            return [0.0] * 384

        url = "https://api.openai.com/v1/embeddings"
        data = json.dumps({"input": text, "model": self.model}).encode("utf-8")

        req = urllib.request.Request(
            url,
            data=data,
            headers={
                "Content-Type": "application/json",
                "Authorization": f"Bearer {api_key}",
            },
        )

        try:
            with urllib.request.urlopen(req, timeout=15) as response:
                resp_data = json.loads(response.read().decode("utf-8"))
                return resp_data["data"][0]["embedding"]
        except Exception as e:
            print(f"OpenAI embedding error: {e}")
            return [0.0] * 384


class LocalEmbeddingProvider:
    def __init__(self, model_name: str = "all-MiniLM-L6-v2"):
        self.model_name = model_name
        self._model = None

    def name(self) -> str:
        return f"local-{self.model_name}"

    def embed(self, text: str) -> list[float]:
        if self._model is None:
            try:
                from sentence_transformers import SentenceTransformer  # type: ignore

                self._model = SentenceTransformer(self.model_name)
            except ImportError:
                print(
                    "sentence-transformers package not installed. "
                    "Run 'pip install sentence-transformers'."
                )
                return [0.0] * 384

        try:
            emb = self._model.encode(text)
            return emb.tolist()
        except Exception as e:
            print(f"Local embedding error: {e}")
            return [0.0] * 384


class OpenAiCompatibleEmbeddingProvider:
    def __init__(
        self,
        model: str = "custom-model",
        base_url: str = "http://localhost:8000/v1",
        api_key: str | None = None,
    ):
        self.model = model
        self.base_url = base_url.rstrip("/")
        self.api_key = api_key

    def name(self) -> str:
        return f"custom-{self.model}"

    def embed(self, text: str) -> list[float]:
        api_key = self.api_key or os.environ.get("CUSTOM_EMBEDDING_API_KEY", "")
        url = f"{self.base_url}/embeddings"
        data = json.dumps({"input": text, "model": self.model}).encode("utf-8")

        headers = {
            "Content-Type": "application/json",
        }
        if api_key:
            headers["Authorization"] = f"Bearer {api_key}"

        req = urllib.request.Request(url, data=data, headers=headers)

        try:
            with urllib.request.urlopen(req, timeout=15) as response:
                resp_data = json.loads(response.read().decode("utf-8"))
                return resp_data["data"][0]["embedding"]
        except Exception as e:
            print(f"OpenAI-compatible embedding error: {e}")
            return [0.0] * 384
