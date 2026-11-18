use std::process::Command;

use brain_core::errors::BrainError;

use super::driver::{EnforcementMode, SandboxActivationReport, SandboxDriver};
use super::policy::SandboxPolicy;

/// Process boundary isolation driver providing process-level defense-in-depth
/// when platform kernel LSMs (Seatbelt/Landlock) are unavailable or restricted.
#[derive(Debug, Clone, Default)]
pub struct ProcessBoundaryDriver;

impl ProcessBoundaryDriver {
    pub fn new() -> Self {
        Self
    }
}

impl SandboxDriver for ProcessBoundaryDriver {
    fn name(&self) -> &'static str {
        "ProcessBoundary"
    }

    fn is_supported(&self) -> bool {
        cfg!(unix)
    }

    fn prepare_command(
        &self,
        program: &str,
        args: &[&str],
        policy: &SandboxPolicy,
    ) -> Result<(Command, SandboxActivationReport), BrainError> {
        // Under Enforced mode, fail closed: ProcessBoundary cannot provide kernel confinement
        if policy.enforcement_mode == EnforcementMode::Enforced {
            return Err(BrainError::Authorization {
                message: "ProcessBoundaryDriver provides advisory process isolation only and cannot provide kernel-enforced confinement. Execution denied in Enforced mode (fail-closed).".to_string(),
            });
        }

        let mut cmd = Command::new(program);
        cmd.args(args);
        policy.env_policy.sanitize_command(&mut cmd);

        let mut warnings = vec![
            "Process-boundary isolation active: credentials scrubbed, file descriptors closed. Kernel-level filesystem confinement is NOT active on this host."
                .to_string(),
        ];

        // Advisory boundary inspection
        for arg in args {
            if let Err(violation) = policy.check_command_boundaries(arg) {
                warnings.push(format!("Advisory boundary violation detected: {violation}"));
            }
        }

        // Close inherited file descriptors > 2 in child pre_exec
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            unsafe {
                cmd.pre_exec(|| {
                    let max_fd = libc::sysconf(libc::_SC_OPEN_MAX).clamp(256, 65536) as i32;
                    for fd in 3..max_fd {
                        libc::close(fd);
                    }
                    Ok(())
                });
            }
        }

        let report = SandboxActivationReport {
            driver_name: self.name(),
            activated: false, // Kernel-level confinement is not active; process boundary only
            mode: policy.enforcement_mode,
            workspace_root: policy.workspace_root.clone(),
            applied_rules_count: policy.fs_rules.len(),
            warnings,
        };

        Ok((cmd, report))
    }

    fn apply_to_command(
        &self,
        cmd: &mut Command,
        policy: &SandboxPolicy,
    ) -> Result<SandboxActivationReport, BrainError> {
        // Under Enforced mode, fail closed: ProcessBoundary cannot provide kernel confinement
        if policy.enforcement_mode == EnforcementMode::Enforced {
            return Err(BrainError::Authorization {
                message: "ProcessBoundaryDriver provides advisory process isolation only and cannot provide kernel-enforced confinement. Execution denied in Enforced mode (fail-closed).".to_string(),
            });
        }

        // 1. Sanitize environment variables: strip all API tokens and cloud credentials
        policy.env_policy.sanitize_command(cmd);

        let mut warnings = vec![
            "Process-boundary isolation active: credentials scrubbed, file descriptors closed. Kernel-level filesystem confinement is NOT active on this host."
                .to_string(),
        ];

        // 2. Advisory boundary inspection
        for arg in cmd.get_args() {
            let s = arg.to_string_lossy();
            if let Err(violation) = policy.check_command_boundaries(&s) {
                warnings.push(format!("Advisory boundary violation detected: {violation}"));
            }
        }

        // 3. Close inherited file descriptors above stderr in child pre_exec
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            unsafe {
                cmd.pre_exec(|| {
                    let max_fd = libc::sysconf(libc::_SC_OPEN_MAX).clamp(256, 65536) as i32;
                    for fd in 3..max_fd {
                        libc::close(fd);
                    }
                    Ok(())
                });
            }
        }

        Ok(SandboxActivationReport {
            driver_name: self.name(),
            activated: false, // Truthful reporting: kernel LSM is not active
            mode: policy.enforcement_mode,
            workspace_root: policy.workspace_root.clone(),
            applied_rules_count: policy.fs_rules.len(),
            warnings,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_process_boundary_driver_fails_closed_in_enforced_mode() {
        let driver = ProcessBoundaryDriver::new();
        let ws = PathBuf::from("/tmp/test_ws");
        let policy = SandboxPolicy::builder(&ws)
            .with_mode(EnforcementMode::Enforced)
            .build();
        let mut cmd = Command::new("/bin/echo");

        let res = driver.apply_to_command(&mut cmd, &policy);
        assert!(res.is_err());
        assert!(matches!(res.unwrap_err(), BrainError::Authorization { .. }));

        let prep_res = driver.prepare_command("/bin/echo", &["test"], &policy);
        assert!(prep_res.is_err());
        assert!(matches!(
            prep_res.unwrap_err(),
            BrainError::Authorization { .. }
        ));
    }

    #[test]
    fn test_process_boundary_driver_permits_execution_in_advisory_mode() {
        let driver = ProcessBoundaryDriver::new();
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
