use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList, PyTuple};
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use crate::plugins::registry::PluginRegistry;
use crate::plugins::traits::{
    CliPlugin, EmbeddingProvider, Exporter, LlmProvider, MemoryExtractor, RankingStrategy,
    RetrievalAlgorithm, StorageBackend,
};
use crate::stm::{STMIndex, TempNode};
use crate::storage::{ExtractedEdge, ExtractedGraph, ExtractedNode};

// ==========================================
// Python Runtime Adapters (GIL & PyO3)
// ==========================================

pub struct PythonEmbeddingProvider {
    pub name: String,
    pub py_obj: PyObject,
}
impl EmbeddingProvider for PythonEmbeddingProvider {
    fn name(&self) -> &str {
        &self.name
    }
    fn embed(&self, text: &str) -> Result<Vec<f32>, String> {
        Python::with_gil(|py| {
            let res = self
                .py_obj
                .call_method1(py, "embed", (text,))
                .map_err(|e| e.to_string())?;
            let vec: Vec<f32> = res.extract(py).map_err(|e| e.to_string())?;
            Ok(vec)
        })
    }
}

pub struct PythonLlmProvider {
    pub name: String,
    pub py_obj: PyObject,
}
impl LlmProvider for PythonLlmProvider {
    fn name(&self) -> &str {
        &self.name
    }
    fn generate(&self, prompt: &str) -> Result<String, String> {
        Python::with_gil(|py| {
            let res = self
                .py_obj
                .call_method1(py, "generate", (prompt,))
                .map_err(|e| e.to_string())?;
            let response: String = res.extract(py).map_err(|e| e.to_string())?;
            Ok(response)
        })
    }
}

pub struct PythonRetrievalAlgorithm {
    pub name: String,
    pub py_obj: PyObject,
}
impl RetrievalAlgorithm for PythonRetrievalAlgorithm {
    fn name(&self) -> &str {
        &self.name
    }
    fn retrieve(
        &self,
        query: &str,
        _index: &STMIndex,
        window: &[TempNode],
    ) -> Result<Vec<(TempNode, i64)>, String> {
        Python::with_gil(|py| {
            let py_window = PyList::empty_bound(py);
            for node in window {
                let dict = PyDict::new_bound(py);
                dict.set_item("id", &node.id).map_err(|e| e.to_string())?;
                dict.set_item("epoch", node.epoch)
                    .map_err(|e| e.to_string())?;
                dict.set_item("content", &node.content)
                    .map_err(|e| e.to_string())?;
                dict.set_item("timestamp", node.timestamp)
                    .map_err(|e| e.to_string())?;
                py_window.append(dict).map_err(|e| e.to_string())?;
            }
            let res = self
                .py_obj
                .call_method1(py, "retrieve", (query, py_window))
                .map_err(|e| e.to_string())?;
            let scored_list: Vec<(String, i64)> = res.extract(py).map_err(|e| e.to_string())?;
            let mut results = Vec::new();
            for (id, score) in scored_list {
                if let Some(node) = window.iter().find(|n| n.id == id) {
                    results.push((node.clone(), score));
                }
            }
            Ok(results)
        })
    }
}

pub struct PythonRankingStrategy {
    pub name: String,
    pub py_obj: PyObject,
}
impl RankingStrategy for PythonRankingStrategy {
    fn name(&self) -> &str {
        &self.name
    }
    fn rank(&self, query: &str, candidates: &mut Vec<(TempNode, i64)>) -> Result<(), String> {
        Python::with_gil(|py| {
            let py_candidates = PyList::empty_bound(py);
            for (node, score) in candidates.iter() {
                let dict = PyDict::new_bound(py);
                dict.set_item("id", &node.id).map_err(|e| e.to_string())?;
                dict.set_item("epoch", node.epoch)
                    .map_err(|e| e.to_string())?;
                dict.set_item("content", &node.content)
                    .map_err(|e| e.to_string())?;
                dict.set_item("timestamp", node.timestamp)
                    .map_err(|e| e.to_string())?;
                let pair = PyTuple::new_bound(
                    py,
                    &[dict.into_any(), score.into_py(py).into_bound(py).into_any()],
                );
                py_candidates.append(pair).map_err(|e| e.to_string())?;
            }
            let res = self
                .py_obj
                .call_method1(py, "rank", (query, py_candidates))
                .map_err(|e| e.to_string())?;
            let sorted_ids: Vec<String> = res.extract(py).map_err(|e| e.to_string())?;
            let mut candidates_map: HashMap<String, (TempNode, i64)> = candidates
                .drain(..)
                .map(|(node, score)| (node.id.clone(), (node, score)))
                .collect();
            for id in sorted_ids {
                if let Some(pair) = candidates_map.remove(&id) {
                    candidates.push(pair);
                }
            }
            for (_, pair) in candidates_map {
                candidates.push(pair);
            }
            Ok(())
        })
    }
}

pub struct PythonStorageBackend {
    pub name: String,
    pub py_obj: PyObject,
}
impl StorageBackend for PythonStorageBackend {
    fn name(&self) -> &str {
        &self.name
    }
    fn write_graph(&self, nodes: &[ExtractedNode], edges: &[ExtractedEdge]) -> Result<(), String> {
        Python::with_gil(|py| {
            let nodes_json = serde_json::to_string(nodes).map_err(|e| e.to_string())?;
            let edges_json = serde_json::to_string(edges).map_err(|e| e.to_string())?;
            self.py_obj
                .call_method1(py, "write_graph", (nodes_json, edges_json))
                .map_err(|e| e.to_string())?;
            Ok(())
        })
    }
    fn query_graph(&self, query: &str) -> Result<Vec<(ExtractedNode, Vec<ExtractedEdge>)>, String> {
        Python::with_gil(|py| {
            let res = self
                .py_obj
                .call_method1(py, "query_graph", (query,))
                .map_err(|e| e.to_string())?;
            let json_output: String = res.extract(py).map_err(|e| e.to_string())?;
            let result: Vec<(ExtractedNode, Vec<ExtractedEdge>)> =
                serde_json::from_str(&json_output).map_err(|e| e.to_string())?;
            Ok(result)
        })
    }
    fn get_updates_since(
        &self,
        timestamp: i64,
    ) -> Result<(Vec<ExtractedNode>, Vec<ExtractedEdge>, i64), String> {
        Python::with_gil(|py| {
            let res = self
                .py_obj
                .call_method1(py, "get_updates_since", (timestamp,))
                .map_err(|e| e.to_string())?;
            let json_output: String = res.extract(py).map_err(|e| e.to_string())?;
            let result: (Vec<ExtractedNode>, Vec<ExtractedEdge>, i64) =
                serde_json::from_str(&json_output).map_err(|e| e.to_string())?;
            Ok(result)
        })
    }
    fn decay_weights(&self, half_life_secs: f64, threshold: f64) -> Result<(), String> {
        Python::with_gil(|py| {
            let bound_obj = self.py_obj.bind(py);
            if bound_obj.hasattr("decay_weights").unwrap_or(false) {
                bound_obj
                    .call_method1("decay_weights", (half_life_secs, threshold))
                    .map_err(|e| e.to_string())?;
            }
            Ok(())
        })
    }
    fn write_embeddings(&self, embeddings: &[(String, Vec<f32>)]) -> Result<(), String> {
        Python::with_gil(|py| {
            let bound_obj = self.py_obj.bind(py);
            if bound_obj.hasattr("write_embeddings").unwrap_or(false) {
                let py_embeddings = PyList::empty_bound(py);
                for (id, emb) in embeddings {
                    let py_emb = PyList::empty_bound(py);
                    for &val in emb {
                        py_emb.append(val).map_err(|e| e.to_string())?;
                    }
                    let pair = PyTuple::new_bound(
                        py,
                        &[id.into_py(py).into_bound(py).into_any(), py_emb.into_any()],
                    );
                    py_embeddings.append(pair).map_err(|e| e.to_string())?;
                }
                bound_obj
                    .call_method1("write_embeddings", (py_embeddings,))
                    .map_err(|e| e.to_string())?;
            }
            Ok(())
        })
    }
    fn query_nearest_neighbors(
        &self,
        query_embedding: &[f32],
        limit: usize,
    ) -> Result<Vec<(String, f32)>, String> {
        Python::with_gil(|py| {
            let bound_obj = self.py_obj.bind(py);
            if bound_obj
                .hasattr("query_nearest_neighbors")
                .unwrap_or(false)
            {
                let py_emb = PyList::empty_bound(py);
                for &val in query_embedding {
                    py_emb.append(val).map_err(|e| e.to_string())?;
                }
                let res = bound_obj
                    .call_method1("query_nearest_neighbors", (py_emb, limit))
                    .map_err(|e| e.to_string())?;
                let results: Vec<(String, f32)> = res.extract().map_err(|e| e.to_string())?;
                Ok(results)
            } else {
                Ok(Vec::new())
            }
        })
    }
    fn get_connections(&self, node_ids: &[String]) -> Result<Vec<ExtractedEdge>, String> {
        Python::with_gil(|py| {
            let bound_obj = self.py_obj.bind(py);
            if bound_obj.hasattr("get_connections").unwrap_or(false) {
                let py_ids = PyList::empty_bound(py);
                for id in node_ids {
                    py_ids.append(id).map_err(|e| e.to_string())?;
                }
                let res = bound_obj
                    .call_method1("get_connections", (py_ids,))
                    .map_err(|e| e.to_string())?;
                let json_output: String = res.extract().map_err(|e| e.to_string())?;
                let edges: Vec<ExtractedEdge> =
                    serde_json::from_str(&json_output).map_err(|e| e.to_string())?;
                Ok(edges)
            } else {
                Ok(Vec::new())
            }
        })
    }
    fn get_nodes_by_ids(&self, ids: &[String]) -> Result<Vec<ExtractedNode>, String> {
        Python::with_gil(|py| {
            let bound_obj = self.py_obj.bind(py);
            if bound_obj.hasattr("get_nodes_by_ids").unwrap_or(false) {
                let py_ids = PyList::empty_bound(py);
                for id in ids {
                    py_ids.append(id).map_err(|e| e.to_string())?;
                }
                let res = bound_obj
                    .call_method1("get_nodes_by_ids", (py_ids,))
                    .map_err(|e| e.to_string())?;
                let json_output: String = res.extract().map_err(|e| e.to_string())?;
                let nodes: Vec<ExtractedNode> =
                    serde_json::from_str(&json_output).map_err(|e| e.to_string())?;
                Ok(nodes)
            } else {
                Ok(Vec::new())
            }
        })
    }
}

pub struct PythonMemoryExtractor {
    pub name: String,
    pub py_obj: PyObject,
}
impl MemoryExtractor for PythonMemoryExtractor {
    fn name(&self) -> &str {
        &self.name
    }
    fn extract(&self, stm_nodes: &[TempNode]) -> Result<ExtractedGraph, String> {
        Python::with_gil(|py| {
            let json_input = serde_json::to_string(stm_nodes).map_err(|e| e.to_string())?;
            let res = self
                .py_obj
                .call_method1(py, "extract", (json_input,))
                .map_err(|e| e.to_string())?;
            let json_output: String = res.extract(py).map_err(|e| e.to_string())?;
            let graph: ExtractedGraph =
                serde_json::from_str(&json_output).map_err(|e| e.to_string())?;
            Ok(graph)
        })
    }
}

pub struct PythonExporter {
    pub name: String,
    pub py_obj: PyObject,
}
impl Exporter for PythonExporter {
    fn name(&self) -> &str {
        &self.name
    }
    fn export(&self, backend: &dyn StorageBackend) -> Result<(), String> {
        Python::with_gil(|py| {
            let last_sync: i64 = self
                .py_obj
                .call_method0(py, "get_last_sync_timestamp")
                .and_then(|r| r.extract(py))
                .unwrap_or(0);
            let (nodes, edges, max_ts) = backend.get_updates_since(last_sync)?;
            if !nodes.is_empty() || !edges.is_empty() {
                let nodes_json = serde_json::to_string(&nodes).map_err(|e| e.to_string())?;
                let edges_json = serde_json::to_string(&edges).map_err(|e| e.to_string())?;
                self.py_obj
                    .call_method1(py, "export_updates", (nodes_json, edges_json, max_ts))
                    .map_err(|e| e.to_string())?;
            }
            Ok(())
        })
    }
}

pub struct PythonCliPlugin {
    pub name: String,
    pub subcommand: String,
    pub description: String,
    pub py_obj: PyObject,
}
impl CliPlugin for PythonCliPlugin {
    fn name(&self) -> &str {
        &self.name
    }
    fn get_subcommand_name(&self) -> &str {
        &self.subcommand
    }
    fn get_subcommand_description(&self) -> &str {
        &self.description
    }
    fn handle_command(&self, args: &[String]) -> Result<(), String> {
        Python::with_gil(|py| {
            let py_args = PyList::empty_bound(py);
            for arg in args {
                py_args.append(arg).map_err(|e| e.to_string())?;
            }
            self.py_obj
                .call_method1(py, "handle_command", (py_args,))
                .map_err(|e| e.to_string())?;
            Ok(())
        })
    }
}

// Built-in python extractor that wraps embedded code
pub struct BuiltinPythonExtractor {
    pub code: String,
}
impl BuiltinPythonExtractor {
    pub fn new(code: String) -> Self {
        Self { code }
    }
}
impl MemoryExtractor for BuiltinPythonExtractor {
    fn name(&self) -> &str {
        "python-default"
    }
    fn extract(&self, stm_nodes: &[TempNode]) -> Result<ExtractedGraph, String> {
        let json_payload = serde_json::to_string(stm_nodes).map_err(|e| e.to_string())?;
        let ffi_result = Python::with_gil(|py| -> PyResult<String> {
            let extractor_module =
                PyModule::from_code_bound(py, &self.code, "extractor.py", "extractor")?;
            let extract_fn = extractor_module.getattr("extract_semantic_nodes")?;
            let res: String = extract_fn.call1((json_payload,))?.extract()?;
            Ok(res)
        });
        match ffi_result {
            Ok(json_response) => {
                let graph: ExtractedGraph =
                    serde_json::from_str(&json_response).map_err(|e| e.to_string())?;
                Ok(graph)
            }
            Err(py_err) => Err(py_err.to_string()),
        }
    }
}

// Dynamic Python Plugin Loader
pub fn load_python_plugins(
    registry: &mut PluginRegistry,
    plugins_dir: &Path,
) -> Result<(), String> {
    if !plugins_dir.exists() {
        let _ = std::fs::create_dir_all(plugins_dir);
        return Ok(());
    }
    let dir_entries = std::fs::read_dir(plugins_dir).map_err(|e| e.to_string())?;
    Python::with_gil(|py| -> PyResult<()> {
        let sys = py.import_bound("sys")?;
        let path_list = sys.getattr("path")?;
        let plugins_dir_str = plugins_dir.to_str().ok_or_else(|| {
            pyo3::exceptions::PyValueError::new_err("Invalid plugins directory path")
        })?;
        path_list.call_method1("append", (plugins_dir_str,))?;

        for entry in dir_entries {
            let entry = match entry {
                Ok(e) => e,
                Err(_) => continue,
            };
            let file_path = entry.path();
            if file_path.extension().and_then(|s| s.to_str()) == Some("py") {
                let file_stem = file_path.file_stem().and_then(|s| s.to_str()).unwrap();
                let module = py.import_bound(file_stem)?;
                if module.hasattr("register_plugins")? {
                    let register_fn = module.getattr("register_plugins")?;
                    let dict: Bound<'_, PyDict> = register_fn.call0()?.downcast_into()?;

                    // Validate compatibility metadata if present
                    let api_version: Option<String> = dict
                        .get_item("api_version")?
                        .map(|v| v.extract())
                        .transpose()?;
                    let minimum_brain_version: Option<String> = dict
                        .get_item("minimum_brain_version")?
                        .map(|v| v.extract())
                        .transpose()?;
                    let plugin_version: Option<String> = dict
                        .get_item("plugin_version")?
                        .map(|v| v.extract())
                        .transpose()?;

                    if let Some(min_ver) = &minimum_brain_version {
                        // Current brain version is "0.1.0"
                        if min_ver.as_str() > "0.1.0" {
                            tracing::warn!(
                                "Skipping plugin from file '{:?}' because it requires brain version >= {}, but current version is 0.1.0",
                                file_path,
                                min_ver
                            );
                            continue;
                        }
                    }

                    if let Some(api_ver) = &api_version {
                        // We only support "1.0" API version currently
                        if api_ver != "1.0" {
                            tracing::warn!(
                                "Skipping plugin from file '{:?}' due to unsupported api_version: '{}' (supported: '1.0')",
                                file_path,
                                api_ver
                            );
                            continue;
                        }
                    }

                    tracing::info!(
                        "Loaded versioned plugin metadata for '{:?}': api_version={:?}, minimum_brain_version={:?}, plugin_version={:?}",
                        file_path.file_name().unwrap_or_default(),
                        api_version,
                        minimum_brain_version,
                        plugin_version
                    );

                    if let Some(list) = dict.get_item("embedding_providers")? {
                        let list: Bound<'_, PyList> = list.downcast_into()?;
                        for obj in list.iter() {
                            let name: String = obj.call_method0("name")?.extract()?;
                            registry.embedding_providers.insert(
                                name.clone(),
                                Arc::new(PythonEmbeddingProvider {
                                    name,
                                    py_obj: obj.to_object(py),
                                }),
                            );
                        }
                    }
                    if let Some(list) = dict.get_item("llm_providers")? {
                        let list: Bound<'_, PyList> = list.downcast_into()?;
                        for obj in list.iter() {
                            let name: String = obj.call_method0("name")?.extract()?;
                            registry.llm_providers.insert(
                                name.clone(),
                                Arc::new(PythonLlmProvider {
                                    name,
                                    py_obj: obj.to_object(py),
                                }),
                            );
                        }
                    }
                    if let Some(list) = dict.get_item("retrieval_algorithms")? {
                        let list: Bound<'_, PyList> = list.downcast_into()?;
                        for obj in list.iter() {
                            let name: String = obj.call_method0("name")?.extract()?;
                            registry.retrieval_algorithms.insert(
                                name.clone(),
                                Arc::new(PythonRetrievalAlgorithm {
                                    name,
                                    py_obj: obj.to_object(py),
                                }),
                            );
                        }
                    }
                    if let Some(list) = dict.get_item("ranking_strategies")? {
                        let list: Bound<'_, PyList> = list.downcast_into()?;
                        for obj in list.iter() {
                            let name: String = obj.call_method0("name")?.extract()?;
                            registry.ranking_strategies.insert(
                                name.clone(),
                                Arc::new(PythonRankingStrategy {
                                    name,
                                    py_obj: obj.to_object(py),
                                }),
                            );
                        }
                    }
                    if let Some(list) = dict.get_item("storage_backends")? {
                        let list: Bound<'_, PyList> = list.downcast_into()?;
                        for obj in list.iter() {
                            let name: String = obj.call_method0("name")?.extract()?;
                            registry.storage_backends.insert(
                                name.clone(),
                                Arc::new(PythonStorageBackend {
                                    name,
                                    py_obj: obj.to_object(py),
                                }),
                            );
                        }
                    }
                    if let Some(list) = dict.get_item("memory_extractors")? {
                        let list: Bound<'_, PyList> = list.downcast_into()?;
                        for obj in list.iter() {
                            let name: String = obj.call_method0("name")?.extract()?;
                            registry.memory_extractors.insert(
                                name.clone(),
                                Arc::new(PythonMemoryExtractor {
                                    name,
                                    py_obj: obj.to_object(py),
                                }),
                            );
                        }
                    }
                    if let Some(list) = dict.get_item("exporters")? {
                        let list: Bound<'_, PyList> = list.downcast_into()?;
                        for obj in list.iter() {
                            let name: String = obj.call_method0("name")?.extract()?;
                            registry.exporters.insert(
                                name.clone(),
                                Arc::new(PythonExporter {
                                    name,
                                    py_obj: obj.to_object(py),
                                }),
                            );
                        }
                    }
                    if let Some(list) = dict.get_item("cli_plugins")? {
                        let list: Bound<'_, PyList> = list.downcast_into()?;
                        for obj in list.iter() {
                            let name: String = obj.call_method0("name")?.extract()?;
                            let subcommand: String =
                                obj.call_method0("get_subcommand_name")?.extract()?;
                            let description: String =
                                obj.call_method0("get_subcommand_description")?.extract()?;
                            registry.cli_plugins.insert(
                                subcommand.clone(),
                                Arc::new(PythonCliPlugin {
                                    name,
                                    subcommand,
                                    description,
                                    py_obj: obj.to_object(py),
                                }),
                            );
                        }
                    }
                }
            }
        }
        Ok(())
    })
    .map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::PluginConfig;
    use std::fs;

    #[test]
    fn test_python_plugin_loading() {
        let temp_dir =
            std::env::temp_dir().join(format!("brain_test_plugins_{}", std::process::id()));
        fs::create_dir_all(&temp_dir).unwrap();

        let plugin_code = r#"
class MockLlm:
    def name(self) -> str:
        return "mock-llm"
    def generate(self, prompt: str) -> str:
        return "Mock response to: " + prompt

def register_plugins():
    return {
        "api_version": "1.0",
        "minimum_brain_version": "0.1.0",
        "plugin_version": "1.0.0",
        "llm_providers": [MockLlm()]
    }
"#;
        let file_path = temp_dir.join("mock_plugin.py");
        fs::write(&file_path, plugin_code).unwrap();

        pyo3::prepare_freethreaded_python();

        let config = PluginConfig {
            active_llm_provider: "mock-llm".to_string(),
            ..Default::default()
        };
        let mut registry = PluginRegistry::new(config);

        load_python_plugins(&mut registry, &temp_dir).unwrap();

        let active_llm = registry.get_llm().unwrap();
        assert_eq!(active_llm.name(), "mock-llm");
        assert_eq!(
            active_llm.generate("test").unwrap(),
            "Mock response to: test"
        );

        let _ = fs::remove_dir_all(temp_dir);
    }
}
