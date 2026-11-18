use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use brain_core::errors::BrainError;
use serde::{Deserialize, Serialize};

use super::policy::SandboxPolicy;

/// Active enforcement mode determining runtime confinement behavior.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum EnforcementMode {
    /// Sandbox confinement MUST be enforced by kernel LSM or OS sandbox.
    /// If enforcement fails or is unavailable on the platform, execution fails closed.
    #[default]
    Enforced,
    /// Advisory inspection only: warnings are logged/returned but execution is not blocked.
    Advisory,
    /// Sandbox confinement is explicitly disabled (e.g. debugging).
    Disabled,
}

impl EnforcementMode {
    /// Determines active mode from the environment variable `BRAIN_SANDBOX_MODE`.
    /// Defaults to `Enforced` for fail-closed security.
    pub fn from_env() -> Self {
        match std::env::var("BRAIN_SANDBOX_MODE").ok().as_deref() {
            Some("advisory") => Self::Advisory,
            Some("disabled") => Self::Disabled,
            _ => Self::Enforced,
        }
    }
}

/// Verification report returned when a sandbox driver evaluates or applies confinement.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SandboxActivationReport {
    /// Identifier name of the driver applying the restrictions.
    pub driver_name: &'static str,
    /// Whether kernel-level or process-level confinement was successfully installed.
    pub activated: bool,
    /// Enforcement mode under which the driver ran.
    pub mode: EnforcementMode,
    /// Authorized workspace directory.
    pub workspace_root: PathBuf,
    /// Total count of filesystem rules applied.
    pub applied_rules_count: usize,
    /// Diagnostic warnings or compatibility notes.
    pub warnings: Vec<String>,
}

/// Common abstraction for platform-specific sandbox isolation drivers.
pub trait SandboxDriver: Send + Sync {
    /// Name of this sandbox driver (e.g., "Seatbelt", "Landlock", "Mock").
    fn name(&self) -> &'static str;

    /// Checks if this sandbox driver is supported and operational on the host kernel.
    fn is_supported(&self) -> bool;

    /// Applies sandbox policy confinement to a subprocess `Command`.
    ///
    /// When `policy.enforcement_mode` is `Enforced`, this method MUST fail closed
    /// and return an `Err(BrainError::Authorization)` if confinement cannot be guaranteed.
    fn apply_to_command(
        &self,
        cmd: &mut Command,
        policy: &SandboxPolicy,
    ) -> Result<SandboxActivationReport, BrainError>;

    /// Prepares a command wrapped or configured with sandbox policy confinement.
    /// Default implementation constructs `Command::new(program)`, adds `args`, and calls `apply_to_command`.
    fn prepare_command(
        &self,
        program: &str,
        args: &[&str],
        policy: &SandboxPolicy,
    ) -> Result<(Command, SandboxActivationReport), BrainError> {
        let mut cmd = Command::new(program);
        cmd.args(args);
        let report = self.apply_to_command(&mut cmd, policy)?;
        Ok((cmd, report))
    }
}

/// No-op sandbox driver used as fallback or baseline.
/// Fails closed when `Enforced`, passes when `Disabled` or `Advisory`.
#[derive(Debug, Default)]
pub struct NoopSandboxDriver;

impl NoopSandboxDriver {
    pub fn new() -> Self {
        Self
    }
}

impl SandboxDriver for NoopSandboxDriver {
    fn name(&self) -> &'static str {
        "Noop"
    }

    fn is_supported(&self) -> bool {
        false
    }

    fn apply_to_command(
        &self,
        cmd: &mut Command,
        policy: &SandboxPolicy,
    ) -> Result<SandboxActivationReport, BrainError> {
        policy.env_policy.sanitize_command(cmd);

        match policy.enforcement_mode {
            EnforcementMode::Enforced => Err(BrainError::Authorization {
                message: "No enforceable kernel sandbox driver available on this platform. Execution denied (fail-closed).".to_string(),
            }),
            EnforcementMode::Advisory => Ok(SandboxActivationReport {
                driver_name: self.name(),
                activated: false,
                mode: policy.enforcement_mode,
                workspace_root: policy.workspace_root.clone(),
                applied_rules_count: policy.fs_rules.len(),
                warnings: vec!["Running with advisory sandbox only: no kernel enforcement active.".to_string()],
            }),
            EnforcementMode::Disabled => Ok(SandboxActivationReport {
                driver_name: self.name(),
                activated: false,
                mode: policy.enforcement_mode,
                workspace_root: policy.workspace_root.clone(),
                applied_rules_count: 0,
                warnings: vec!["Sandbox is explicitly disabled.".to_string()],
            }),
        }
    }
}

/// Mock sandbox driver for deterministic adversarial and unit testing.
#[derive(Debug, Clone)]
pub struct MockSandboxDriver {
    name: &'static str,
    supported: bool,
    force_failure: Arc<AtomicBool>,
    simulated_denial_paths: Arc<std::sync::RwLock<Vec<PathBuf>>>,
    simulated_runtime_denial_paths: Arc<std::sync::RwLock<Vec<PathBuf>>>,
}

impl Default for MockSandboxDriver {
    fn default() -> Self {
        Self::new()
    }
}

impl MockSandboxDriver {
    pub fn new() -> Self {
        Self {
            name: "MockSandbox",
            supported: true,
            force_failure: Arc::new(AtomicBool::new(false)),
            simulated_denial_paths: Arc::new(std::sync::RwLock::new(Vec::new())),
            simulated_runtime_denial_paths: Arc::new(std::sync::RwLock::new(Vec::new())),
        }
    }

    pub fn with_unsupported() -> Self {
        Self {
            name: "MockUnsupported",
            supported: false,
            force_failure: Arc::new(AtomicBool::new(false)),
            simulated_denial_paths: Arc::new(std::sync::RwLock::new(Vec::new())),
            simulated_runtime_denial_paths: Arc::new(std::sync::RwLock::new(Vec::new())),
        }
    }

    pub fn set_force_failure(&self, fail: bool) {
        self.force_failure.store(fail, Ordering::SeqCst);
    }

    pub fn add_simulated_denial(&self, path: impl Into<PathBuf>) {
        self.simulated_denial_paths
            .write()
            .unwrap()
            .push(path.into());
    }

    /// Simulates a kernel-enforced sandbox where the subprocess actually starts,
    /// but the kernel denies unauthorized filesystem access at runtime with EPERM.
    pub fn add_simulated_runtime_denial(&self, path: impl Into<PathBuf>) {
        self.simulated_runtime_denial_paths
            .write()
            .unwrap()
            .push(path.into());
    }
}

impl SandboxDriver for MockSandboxDriver {
    fn name(&self) -> &'static str {
        self.name
    }

    fn is_supported(&self) -> bool {
        self.supported
    }

    fn apply_to_command(
        &self,
        cmd: &mut Command,
        policy: &SandboxPolicy,
    ) -> Result<SandboxActivationReport, BrainError> {
        policy.env_policy.sanitize_command(cmd);

        if self.force_failure.load(Ordering::SeqCst) {
            return Err(BrainError::Authorization {
                message: "Mock sandbox installation failed deliberately for testing".to_string(),
            });
        }

        if !self.supported && policy.enforcement_mode == EnforcementMode::Enforced {
            return Err(BrainError::Authorization {
                message: "Platform lacks kernel sandbox support. Fail-closed.".to_string(),
            });
        }

        // Check simulated pre-flight denials
        let denials = self.simulated_denial_paths.read().unwrap();
        if !denials.is_empty() {
            for arg in cmd.get_args() {
                let s = arg.to_string_lossy();
                for denied in denials.iter() {
                    let d_str = denied.to_string_lossy();
                    if s.contains(&*d_str) {
                        return Err(BrainError::Authorization {
                            message: format!(
                                "Mock kernel sandbox denied access to path: {}",
                                denied.display()
                            ),
                        });
                    }
                }
            }
        }

        // Check simulated runtime kernel denials (subprocess starts, OS denies access at runtime)
        let runtime_denials = self.simulated_runtime_denial_paths.read().unwrap();
        if !runtime_denials.is_empty() {
            let mut matched_denial: Option<PathBuf> = None;
            for arg in cmd.get_args() {
                let s = arg.to_string_lossy();
                for denied in runtime_denials.iter() {
                    let d_str = denied.to_string_lossy();
                    if s.contains(&*d_str) {
                        matched_denial = Some(denied.clone());
                        break;
                    }
                }
                if matched_denial.is_some() {
                    break;
                }
            }
            if let Some(denied) = matched_denial {
                let denied_display = denied.display().to_string();
                let shim =
                    format!("echo 'cat: {denied_display}: Operation not permitted' 1>&2; exit 1");
                *cmd = Command::new("/bin/bash");
                cmd.args(["-c", shim.as_str()]);
            }
        }

        // Set an environment marker indicating mock confinement was installed
        cmd.env("BRAIN_SANDBOX_MOCK_ACTIVE", "1");

        Ok(SandboxActivationReport {
            driver_name: self.name(),
            activated: self.supported,
            mode: policy.enforcement_mode,
            workspace_root: policy.workspace_root.clone(),
            applied_rules_count: policy.fs_rules.len(),
            warnings: Vec::new(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mock_driver_succeeds_and_marks_activation() {
        let driver = MockSandboxDriver::new();
        let ws = PathBuf::from("/tmp/test_ws");
        let policy = SandboxPolicy::builder(&ws).build();
        let mut cmd = Command::new("/bin/echo");

        let report = driver.apply_to_command(&mut cmd, &policy).unwrap();
        assert_eq!(report.driver_name, "MockSandbox");
        assert!(report.activated);
    }

    #[test]
    fn test_mock_driver_fails_closed_when_forced() {
        let driver = MockSandboxDriver::new();
        driver.set_force_failure(true);
        let ws = PathBuf::from("/tmp/test_ws");
        let policy = SandboxPolicy::builder(&ws).build();
        let mut cmd = Command::new("/bin/echo");

        let res = driver.apply_to_command(&mut cmd, &policy);
        assert!(res.is_err());
        assert!(matches!(res.unwrap_err(), BrainError::Authorization { .. }));
    }

    #[test]
    fn test_noop_driver_fails_closed_when_enforced() {
        let driver = NoopSandboxDriver::new();
        let ws = PathBuf::from("/tmp/test_ws");
        let policy = SandboxPolicy::builder(&ws)
            .with_mode(EnforcementMode::Enforced)
            .build();
        let mut cmd = Command::new("/bin/echo");

        let res = driver.apply_to_command(&mut cmd, &policy);
        assert!(res.is_err());
    }

    #[test]
    fn test_noop_driver_passes_when_advisory() {
        let driver = NoopSandboxDriver::new();
        let ws = PathBuf::from("/tmp/test_ws");
        let policy = SandboxPolicy::builder(&ws)
            .with_mode(EnforcementMode::Advisory)
            .build();
        let mut cmd = Command::new("/bin/echo");

        let res = driver.apply_to_command(&mut cmd, &policy);
        assert!(res.is_ok());
        let report = res.unwrap();
        assert!(!report.activated);
        assert_eq!(report.mode, EnforcementMode::Advisory);
    }
}
