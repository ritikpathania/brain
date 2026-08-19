# Plugin Manager Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement the generic, composed `PluginManager`, `PluginScanner`, and registry layer in `brain-plugins`, adapt the existing `brain-python` plugin loader to conform to the new architecture, and verify transactional RCU-style reloads and lock-free event dispatches.

**Architecture:** We will refine `brain-core` to establish composed traits (`PluginMetadata`, `PluginLifecycle`, `PluginCapabilities`, `PluginEventHandler`) and the `HostContext` wrapper. We will implement `PluginScanner` and `PluginManager` in `brain-plugins` using `parking_lot` locks and transactional RCU-style reload swaps. Finally, we will adapt the Python plugin loader to these traits.

**Tech Stack:** Rust, `parking_lot`, `semver`, PyO3.

## Global Constraints
- Every task must compile cleanly under `cargo check` and `cargo clippy --all-targets -- -D warnings`.
- Code must follow `cargo fmt` formatting.
- PluginManager must remain 100% runtime-agnostic and never perform Python executions or acquire the GIL.
- All dependencies must utilize workspace versions.

---

### Task 1: PR-011A — Core Abstraction Contracts

**Files:**
- Modify: `crates/brain-domain/src/entities.rs`
- Modify: `Cargo.toml`
- Modify: `crates/brain-core/Cargo.toml`
- Modify: `crates/brain-core/src/extensibility.rs`

**Interfaces:**
- Produces: `PluginState::Disabled`
- Produces: `ApiVersion` enum
- Produces: `PluginManifest` struct
- Produces: `HostContext` trait
- Produces: `PluginContext` struct
- Produces: `PluginEventKind` & `PluginEvent` structs
- Produces: `PluginCapability` & `CapabilityDescriptor` structs
- Produces: `Plugin` trait composing `PluginMetadata`, `PluginLifecycle`, `PluginCapabilities`, and `PluginEventHandler`.

- [ ] **Step 1: Update `PluginState` to include `Disabled` variant**
  Modify [entities.rs](../../../../crates/brain-domain/src/entities.rs#L47-L64):
  ```rust
  pub enum PluginState {
      Discovered,
      DependenciesResolved,
      Validated,
      Loaded,
      Initialized,
      Active,
      Suspended,
      Disabled,
      Unloaded,
  }
  ```

- [ ] **Step 2: Add `semver` dependency to workspace and `brain-core`**
  Modify root [Cargo.toml](../../../../Cargo.toml):
  ```toml
  [workspace.dependencies]
  semver = "1.0"
  ```
  Modify [Cargo.toml](../../../../crates/brain-core/Cargo.toml):
  ```toml
  [dependencies]
  semver = { version = "1.0", features = ["serde"] }
  ```

- [ ] **Step 3: Refine extensibility types and traits in `brain-core`**
  Overwrite [extensibility.rs](../../../../crates/brain-core/src/extensibility.rs) keeping existing tool primitives intact:
  ```rust
  use crate::errors::BrainError;
  use brain_domain::{PluginId, PluginState, SessionId, Node, ChatAgent, PlannerAgent, EmbeddingAgent, ExtractionAgent};
  use serde::{Deserialize, Serialize};
  use std::collections::BTreeSet;
  use std::path::{Path, PathBuf};
  use std::sync::Arc;

  #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
  #[serde(rename_all = "snake_case")]
  pub enum Permission {
      FilesystemRead,
      FilesystemWrite,
      Shell,
      Git,
      Network,
      Clipboard,
      Storage,
      Llm,
  }

  #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
  pub enum ApiVersion {
      V1,
  }

  #[derive(Debug, Clone, Serialize, Deserialize)]
  pub struct PluginManifest {
      id: PluginId,
      version: semver::Version,
      api_version: ApiVersion,
      entrypoint: PathBuf,
      required_permissions: BTreeSet<Permission>,
  }

  impl PluginManifest {
      pub fn new(
          id: PluginId,
          version: semver::Version,
          api_version: ApiVersion,
          entrypoint: PathBuf,
          required_permissions: BTreeSet<Permission>,
      ) -> Self {
          Self {
              id,
              version,
              api_version,
              entrypoint,
              required_permissions,
          }
      }

      pub fn from_path(path: &Path) -> Result<Self, BrainError> {
          let content = std::fs::read_to_string(path).map_err(|e| BrainError::Storage {
              message: format!("Failed to read plugin manifest: {}", e),
              source: Some(Box::new(e)),
          })?;
          toml::from_str(&content).map_err(|e| BrainError::Validation {
              message: format!("Failed to parse plugin manifest: {}", e),
          })
      }

      pub fn id(&self) -> PluginId {
          self.id
      }

      pub fn version(&self) -> &semver::Version {
          &self.version
      }

      pub fn api_version(&self) -> ApiVersion {
          self.api_version
      }

      pub fn entrypoint(&self) -> &Path {
          &self.entrypoint
      }

      pub fn required_permissions(&self) -> &BTreeSet<Permission> {
          &self.required_permissions
      }
  }

  pub trait HostContext: Send + Sync {
      fn retrieve(&self, session_id: &SessionId, query: &str, limit: usize) -> Result<Vec<Node>, BrainError>;
      fn execute_tool(
          &self,
          session_id: &SessionId,
          tool_name: &str,
          arguments: &std::collections::HashMap<String, serde_json::Value>,
      ) -> Result<ExecutionResult, BrainError>;
  }

  pub struct PluginContext<'a> {
      pub host: &'a dyn HostContext,
      pub session_id: Option<SessionId>,
  }

  #[derive(Debug, Clone, Copy, PartialEq, Eq)]
  pub enum PluginEventKind {
      Load,
      Unload,
      SessionStart,
      SessionEnd,
  }

  pub struct PluginEvent<'a> {
      pub kind: PluginEventKind,
      pub context: &'a PluginContext<'a>,
  }

  #[derive(Clone)]
  pub enum PluginCapability {
      Chat(Arc<dyn ChatAgent>),
      Planner(Arc<dyn PlannerAgent>),
      Embedding(Arc<dyn EmbeddingAgent>),
      Extraction(Arc<dyn ExtractionAgent>),
  }

  pub struct CapabilityDescriptor {
      pub name: &'static str,
      pub capability: PluginCapability,
  }

  pub trait PluginMetadata: Send + Sync {
      fn manifest(&self) -> &PluginManifest;
  }

  pub trait PluginLifecycle: Send + Sync {
      fn state(&self) -> PluginState;
      fn load(&mut self) -> Result<(), BrainError>;
      fn initialize(&mut self) -> Result<(), BrainError>;
      fn activate(&mut self) -> Result<(), BrainError>;
      fn suspend(&mut self) -> Result<(), BrainError>;
      fn resume(&mut self) -> Result<(), BrainError>;
      fn unload(&mut self) -> Result<(), BrainError>;
  }

  pub trait PluginCapabilities: Send + Sync {
      fn capabilities(&self) -> &[CapabilityDescriptor];
  }

  pub trait PluginEventHandler: Send + Sync {
      fn dispatch(&self, event: &PluginEvent<'_>) -> Result<(), BrainError>;
  }

  pub trait Plugin:
      PluginMetadata
      + PluginLifecycle
      + PluginCapabilities
      + PluginEventHandler
      + Send
      + Sync
  {}
  ```

- [ ] **Step 4: Verify compilation of `brain-core`**
  Run: `cargo check -p brain-core`
  Expected: PASS

- [ ] **Step 5: Commit changes**
  Run: `git add crates/brain-domain/ crates/brain-core/ Cargo.toml`
  Run: `git commit -m "feat(brain-core): define composed Plugin traits and PluginManifest"`
  Expected: Commit successfully.

---

### Task 2: PR-011B — Registry, Scanner & PluginHandle

**Files:**
- Modify: `crates/brain-plugins/Cargo.toml`
- Overwrite: `crates/brain-plugins/src/lib.rs`

**Interfaces:**
- Produces: `LoaderKind` enum
- Produces: `InstalledPlugin` struct
- Produces: `PluginLoader` trait
- Produces: `PluginHandle` struct
- Produces: `ManagedPlugin` struct
- Produces: `PluginSummary` struct
- Produces: `PluginRegistry` struct
- Produces: `PluginScanner` struct

- [ ] **Step 1: Add dependencies to `crates/brain-plugins/Cargo.toml`**
  Modify [Cargo.toml](../../../../crates/brain-plugins/Cargo.toml):
  ```toml
  [dependencies]
  brain-core = { path = "../brain-core" }
  brain-domain = { path = "../brain-domain" }
  semver = { version = "1.0", features = ["serde"] }
  parking_lot = "0.12"
  toml = "0.8"
  tracing = "0.1"
  ```

- [ ] **Step 2: Add Registry, Loader & Scanner to `crates/brain-plugins/src/lib.rs`**
  ```rust
  use std::collections::{BTreeMap, HashMap};
  use std::path::{Path, PathBuf};
  use std::sync::Arc;
  use parking_lot::RwLock;

  use brain_core::errors::BrainError;
  use brain_core::extensibility::{Plugin, PluginManifest, ApiVersion, Permission, PluginEventKind, PluginEvent, PluginContext};
  use brain_domain::{PluginId, PluginState};

  #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
  pub enum LoaderKind {
      Python,
      Wasm,
      Native,
  }

  #[derive(Debug, Clone)]
  pub struct InstalledPlugin {
      pub manifest: PluginManifest,
      pub path: PathBuf,
      pub loader_kind: LoaderKind,
  }

  pub trait PluginLoader: Send + Sync {
      fn kind(&self) -> LoaderKind;
      fn supports(&self, path: &Path) -> bool;
      fn load(&self, descriptor: &InstalledPlugin) -> Result<Box<dyn Plugin>, BrainError>;
  }

  #[derive(Clone)]
  pub struct PluginHandle {
      pub inner: Arc<RwLock<dyn Plugin>>,
  }

  pub struct ManagedPlugin {
      pub installed: InstalledPlugin,
      pub active_instance: Option<PluginHandle>,
  }

  #[derive(Debug, Clone)]
  pub struct PluginSummary {
      pub id: PluginId,
      pub version: semver::Version,
      pub api_version: ApiVersion,
      pub state: PluginState,
      pub path: PathBuf,
      pub loader_kind: LoaderKind,
  }

  pub struct PluginRegistry {
      pub plugins: BTreeMap<PluginId, ManagedPlugin>,
  }

  impl PluginRegistry {
      pub fn new() -> Self {
          Self { plugins: BTreeMap::new() }
      }

      pub fn register(&mut self, installed: InstalledPlugin) -> Result<(), BrainError> {
          let id = installed.manifest.id();
          if self.plugins.contains_key(&id) {
              return Err(BrainError::InvalidTransition {
                  message: format!("Plugin ID {} is already registered", id),
              });
          }
          self.plugins.insert(id, ManagedPlugin {
              installed,
              active_instance: None,
          });
          Ok(())
      }

      pub fn get(&self, id: &PluginId) -> Option<&ManagedPlugin> {
          self.plugins.get(id)
      }

      pub fn get_mut(&mut self, id: &PluginId) -> Option<&mut ManagedPlugin> {
          self.plugins.get_mut(id)
      }

      pub fn list(&self) -> Vec<PluginSummary> {
          self.plugins.values().map(|mp| {
              let state = mp.active_instance.as_ref()
                  .map(|p| p.inner.read().state())
                  .unwrap_or(PluginState::Discovered);
              PluginSummary {
                  id: mp.installed.manifest.id(),
                  version: mp.installed.manifest.version().clone(),
                  api_version: mp.installed.manifest.api_version(),
                  state,
                  path: mp.installed.path.clone(),
                  loader_kind: mp.installed.loader_kind,
              }
          }).collect()
      }
  }

  pub struct PluginScanner {
      loaders: Vec<Box<dyn PluginLoader>>,
  }

  impl PluginScanner {
      pub fn new(loaders: Vec<Box<dyn PluginLoader>>) -> Self {
          Self { loaders }
      }

      pub fn scan_directory(&self, plugins_dir: &Path) -> Result<Vec<InstalledPlugin>, BrainError> {
          let mut discovered = Vec::new();
          let read_dir = std::fs::read_dir(plugins_dir).map_err(|e| BrainError::Storage {
              message: format!("Failed to read plugins directory: {}", e),
              source: Some(Box::new(e)),
          })?;

          for entry in read_dir.flatten() {
              let path = entry.path();
              if path.is_dir() {
                  for loader in &self.loaders {
                      if loader.supports(&path) {
                          let manifest_path = path.join("plugin.toml");
                          if manifest_path.exists() {
                              if let Ok(manifest) = PluginManifest::from_path(&manifest_path) {
                                  discovered.push(InstalledPlugin {
                                      manifest,
                                      path: path.clone(),
                                      loader_kind: loader.kind(),
                                  });
                                  break;
                              }
                          }
                      }
                  }
              }
          }
          Ok(discovered)
      }
  }
  ```

- [ ] **Step 3: Verify registry and scanner compile**
  Run: `cargo check -p brain-plugins`
  Expected: PASS

- [ ] **Step 4: Commit registry and scanner**
  Run: `git add crates/brain-plugins/`
  Run: `git commit -m "feat(brain-plugins): implement PluginRegistry and PluginScanner"`
  Expected: Commit successfully.

---

### Task 3: PR-011C — PluginManager & Lifecycle Orchestration

**Files:**
- Modify: `crates/brain-plugins/src/lib.rs`
- Create: `crates/brain-plugins/tests/manager_tests.rs`

**Interfaces:**
- Produces: `PluginManager`
- Produces: `PluginDispatchReport`

- [ ] **Step 1: Append `PluginManager` to `crates/brain-plugins/src/lib.rs`**
  Add the `PluginManager` implementation, including RCU reload and lock-free event dispatching.

- [ ] **Step 2: Create unit tests in `crates/brain-plugins/tests/manager_tests.rs`**
  Write tests covering:
  - Discovered plugin registration.
  - State machine transitions (`load()`, `initialize()`, `activate()`).
  - Lock-free hook dispatches (`PluginDispatchReport`).
  - Transactional reload (swap pointer and best-effort unload).

- [ ] **Step 3: Verify tests compile and pass**
  Run: `cargo test -p brain-plugins`
  Expected: PASS

- [ ] **Step 4: Commit PluginManager**
  Run: `git add crates/brain-plugins/`
  Run: `git commit -m "feat(brain-plugins): implement PluginManager lifecycle orchestration"`
  Expected: Commit successfully.

---

### Task 4: PR-011D — Adapt `brain-python` Loader & Adapters

**Files:**
- Modify: `crates/brain-python/Cargo.toml`
- Modify: `crates/brain-python/src/loader.rs`
- Modify: `crates/brain-python/tests/python_tests.rs`

**Interfaces:**
- Consumes: Composed `Plugin` traits from `brain-core`
- Consumes: `PluginLoader` trait from `brain-plugins`

- [ ] **Step 1: Add dependencies to `crates/brain-python/Cargo.toml`**
  Modify [Cargo.toml](../../../../crates/brain-python/Cargo.toml):
  ```toml
  [dependencies]
  brain-plugins = { path = "../brain-plugins" }
  parking_lot = "0.12"
  semver = "1.0"
  ```

- [ ] **Step 2: Adapt `crates/brain-python/src/loader.rs` to generic `Plugin` traits**
  Refactor `LoadedPlugin` to implement `Plugin`, `PluginMetadata`, `PluginLifecycle`, `PluginCapabilities`, and `PluginEventHandler`. Adapt `PythonPluginLoader` to implement `PluginLoader`.

- [ ] **Step 3: Update `crates/brain-python/tests/python_tests.rs` to compile**
  Update the tests to compile against the updated `Plugin` traits and descriptors.

- [ ] **Step 4: Verify Python tests pass**
  Run: `PYO3_PYTHON=/Users/ritikpathania/.local/share/uv/python/cpython-3.12-macos-aarch64-none/bin/python3.12 cargo test -p brain-python`
  Expected: PASS

- [ ] **Step 5: Commit changes**
  Run: `git add crates/brain-python/`
  Run: `git commit -m "feat(brain-python): adapt python loader to composed plugin traits"`
  Expected: Commit successfully.

---

### Task 5: PR-011E — Concurrency Stress Test & Walkthrough

**Files:**
- Modify: `crates/brain-plugins/tests/manager_tests.rs`
- Modify: `crates/brain-python/tests/python_tests.rs`
- Modify: `walkthrough.md`

- [ ] **Step 1: Write multi-threaded concurrency stress test in `crates/brain-plugins/tests/manager_tests.rs`**
  Write a test spawning 8 threads executing 10,000 cycles:
  - Thread 1 & 2: `reload(id)`
  - Thread 3 & 4: `dispatch_event(&event)`
  - Thread 5: `list()`
  - Thread 6: `load(id)`
  - Thread 7: `unload(id)`
  - Thread 8: `suspend(id)` / `resume(id)`
  Assert zero deadlocks or panics.

- [ ] **Step 2: Check workspace compile and clippy warnings**
  Run: `PYO3_PYTHON=/Users/ritikpathania/.local/share/uv/python/cpython-3.12-macos-aarch64-none/bin/python3.12 cargo clippy --all-targets -- -D warnings`
  Expected: PASS with zero warnings

- [ ] **Step 3: Run all workspace tests**
  Run: `PYO3_PYTHON=/Users/ritikpathania/.local/share/uv/python/cpython-3.12-macos-aarch64-none/bin/python3.12 cargo test`
  Expected: PASS

- [ ] **Step 4: Update `walkthrough.md`**
  Extend [walkthrough.md](artifact://walkthrough.md) with details for PR-011.

- [ ] **Step 5: Commit final changes**
  Run: `git status`
  Run: `git commit -am "test(brain-plugins): add multi-threaded stress test and update docs"`
  Expected: Commit successfully.
