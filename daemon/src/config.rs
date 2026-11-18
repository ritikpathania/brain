/// Daemon-local name for the shared runtime path model.
pub type BrainPaths = brain_config::RuntimePaths;

pub fn resolve_paths() -> BrainPaths {
    brain_config::resolve_runtime_paths()
}
