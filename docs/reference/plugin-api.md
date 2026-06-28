# Plugin API Specification

The daemon supports dynamic plugin extensions written in Python or compiled in Rust.

## 1. Rust Trait Interfaces

Defined in `daemon/src/plugins/traits.rs`. Key traits include:

```rust
pub trait LlmProvider: Send + Sync {
    fn name(&self) -> &str;
    fn generate(&self, prompt: &str) -> Result<String, String>;
}

pub trait StorageBackend: Send + Sync {
    fn name(&self) -> &str;
    fn write_graph(&self, nodes: &[ExtractedNode], edges: &[ExtractedEdge]) -> Result<(), String>;
    fn query_graph(&self, query: &str) -> Result<Vec<(ExtractedNode, Vec<ExtractedEdge>)>, String>;
    fn decay_weights(&self, half_life_secs: f64, threshold: f64) -> Result<(), String>;
}
```

## 2. Python Plugin Interface

Python plugins should be saved under `~/.brain/plugins/` (e.g. `my_plugin.py`) and export a `register_plugins()` function.

### Interface Example:
```python
class CustomLlmProvider:
    def name(self) -> str:
        return "custom-llm"

    def generate(self, prompt: str) -> str:
        # custom generation logic
        return f"Response to: {prompt}"

def register_plugins():
    return {
        "llm_providers": [CustomLlmProvider()]
    }
```

Registered components are automatically discovered and loaded into the `PluginRegistry` on daemon startup.
