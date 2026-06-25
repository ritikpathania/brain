import json
import os
import urllib.error
import urllib.request


class OllamaLlmProvider:
    def __init__(self, model: str = "llama3", base_url: str = "http://localhost:11434"):
        self.model = model
        self.base_url = base_url

    def name(self) -> str:
        return f"ollama-{self.model}"

    def generate(self, prompt: str) -> str:
        url = f"{self.base_url}/api/generate"
        data = json.dumps(
            {"model": self.model, "prompt": prompt, "stream": False}
        ).encode("utf-8")

        req = urllib.request.Request(
            url, data=data, headers={"Content-Type": "application/json"}
        )

        try:
            with urllib.request.urlopen(req, timeout=10) as response:
                resp_data = json.loads(response.read().decode("utf-8"))
                return resp_data.get("response", "")
        except Exception as e:
            return f"Ollama error: {e}"


class OpenAiLlmProvider:
    def __init__(self, model: str = "gpt-4o-mini"):
        self.model = model

    def name(self) -> str:
        return f"openai-{self.model}"

    def generate(self, prompt: str) -> str:
        api_key = os.environ.get("OPENAI_API_KEY")
        if not api_key:
            return "Error: OPENAI_API_KEY environment variable not set."

        url = "https://api.openai.com/v1/chat/completions"
        data = json.dumps(
            {
                "model": self.model,
                "messages": [{"role": "user", "content": prompt}],
                "temperature": 0.2,
            }
        ).encode("utf-8")

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
                return resp_data["choices"][0]["message"]["content"]
        except Exception as e:
            return f"OpenAI API error: {e}"


class AnthropicLlmProvider:
    def __init__(self, model: str = "claude-3-5-sonnet-20240620"):
        self.model = model

    def name(self) -> str:
        return f"anthropic-{self.model}"

    def generate(self, prompt: str) -> str:
        api_key = os.environ.get("ANTHROPIC_API_KEY")
        if not api_key:
            return "Error: ANTHROPIC_API_KEY environment variable not set."

        url = "https://api.anthropic.com/v1/messages"
        data = json.dumps(
            {
                "model": self.model,
                "max_tokens": 4096,
                "messages": [{"role": "user", "content": prompt}],
                "temperature": 0.2,
            }
        ).encode("utf-8")

        req = urllib.request.Request(
            url,
            data=data,
            headers={
                "content-type": "application/json",
                "x-api-key": api_key,
                "anthropic-version": "2023-06-01",
            },
        )

        try:
            with urllib.request.urlopen(req, timeout=20) as response:
                resp_data = json.loads(response.read().decode("utf-8"))
                return resp_data["content"][0]["text"]
        except Exception as e:
            return f"Anthropic API error: {e}"
