use std::ffi::CString;
use std::path::{Path, PathBuf};
use std::process::Command;

use brain_core::errors::BrainError;

use super::driver::{EnforcementMode, SandboxActivationReport, SandboxDriver};
use super::policy::{FsAccess, SandboxPolicy};

#[cfg(all(unix, target_os = "macos"))]
extern "C" {
    fn sandbox_init(
        profile: *const libc::c_char,
        flags: u64,
        errorbuf: *mut *mut libc::c_char,
    ) -> libc::c_int;
    fn sandbox_free_error(errorbuf: *mut libc::c_char);
}

static SEATBELT_USABLE: std::sync::OnceLock<bool> = std::sync::OnceLock::new();

fn check_seatbelt_usable() -> bool {
    #[cfg(target_os = "macos")]
    {
        *SEATBELT_USABLE.get_or_init(|| {
            if !Path::new("/usr/bin/sandbox-exec").exists() {
                return false;
            }
            let output = Command::new("/usr/bin/sandbox-exec")
                .arg("-p")
                .arg("(version 1)(allow default)")
                .arg("/usr/bin/true")
                .output();
            match output {
                Ok(out) => out.status.success(),
                Err(_) => false,
            }
        })
    }
    #[cfg(not(target_os = "macos"))]
    {
        false
    }
}

/// macOS Seatbelt sandbox enforcement driver using Apple's kernel sandbox architecture.
#[derive(Debug, Clone)]
pub struct SeatbeltSandboxDriver {
    sandbox_exec_path: PathBuf,
}

impl Default for SeatbeltSandboxDriver {
    fn default() -> Self {
        Self::new()
    }
}

impl SeatbeltSandboxDriver {
    /// Creates a new SeatbeltSandboxDriver pointing to standard `/usr/bin/sandbox-exec`.
    pub fn new() -> Self {
        Self {
            sandbox_exec_path: PathBuf::from("/usr/bin/sandbox-exec"),
        }
    }

    /// Path to the sandbox-exec binary.
    pub fn sandbox_exec_path(&self) -> &Path {
        &self.sandbox_exec_path
    }

    /// Generates a deny-by-default Seatbelt S-expression profile string from a [`SandboxPolicy`].
    pub fn generate_profile(&self, policy: &SandboxPolicy) -> String {
        let mut p = String::new();
        p.push_str(";; Apple Seatbelt Sandbox Profile (Brain SEC-03 Native Enforcement)\n");
        p.push_str("(version 1)\n");
        p.push_str("(deny default)\n\n");

        // 1. Process lifecycle & management
        p.push_str(";; Process execution and signaling\n");
        p.push_str("(allow process-fork)\n");
        p.push_str("(allow sysctl-read)\n");
        p.push_str("(allow signal (target self))\n");

        // Allow process execution of binaries from standard system and user path locations
        p.push_str("(allow process-exec\n");
        for bin_dir in &[
            "/bin",
            "/sbin",
            "/usr/bin",
            "/usr/sbin",
            "/usr/libexec",
            "/usr/local/bin",
            "/opt/homebrew/bin",
            "/Library/Developer",
        ] {
            if Path::new(bin_dir).exists() {
                p.push_str(&format!("    (subpath \"{bin_dir}\")\n"));
            }
        }
        // Allow executing binaries inside workspace
        let ws_str = policy.workspace_root.to_string_lossy();
        let canon_ws_opt = policy
            .workspace_root
            .canonicalize()
            .ok()
            .map(|c| c.to_string_lossy().to_string());
        p.push_str(&format!("    (subpath \"{ws_str}\")\n"));
        p.push_str(")\n\n");

        // Helper to resolve ~ and canonical path
        let resolve_rule_str = |path: &Path| -> String {
            let p_str = path.to_string_lossy();
            if p_str.starts_with("~/") {
                if let Some(home) = std::env::var_os("HOME") {
                    PathBuf::from(home)
                        .join(p_str.trim_start_matches("~/"))
                        .to_string_lossy()
                        .to_string()
                } else {
                    p_str.to_string()
                }
            } else if p_str == "~" {
                if let Some(home) = std::env::var_os("HOME") {
                    PathBuf::from(home).to_string_lossy().to_string()
                } else {
                    p_str.to_string()
                }
            } else {
                p_str.to_string()
            }
        };

        // 2. File read allowances (Default allow system-wide reads for dyld, libc, and workspace)
        p.push_str(";; Authorized filesystem reads\n");
        p.push_str("(allow file-read*)\n");
        p.push_str("(allow file-read*\n");
        p.push_str(&format!("    (subpath \"{ws_str}\")\n"));
        if let Some(ref canon_str) = canon_ws_opt {
            if canon_str != &ws_str {
                p.push_str(&format!("    (subpath \"{canon_str}\")\n"));
            }
        }
        for rule in &policy.fs_rules {
            if rule.access == FsAccess::Read || rule.access == FsAccess::ReadWrite {
                let path_str = resolve_rule_str(&rule.path);
                if rule.recursive {
                    p.push_str(&format!("    (subpath \"{path_str}\")\n"));
                } else {
                    p.push_str(&format!("    (literal \"{path_str}\")\n"));
                }
            }
        }
        p.push_str(")\n\n");

        // 3. Strict sensitive file READ denials (Deny rules override Allow in Seatbelt)
        p.push_str(";; Explicit denial of sensitive files & credentials (overrides allow)\n");
        p.push_str("(deny file-read*\n");
        // In-workspace secret environment variables & configs
        p.push_str(&format!("    (subpath \"{ws_str}/.env\")\n"));
        if let Some(ref canon_str) = canon_ws_opt {
            if canon_str != &ws_str {
                p.push_str(&format!("    (subpath \"{canon_str}/.env\")\n"));
            }
        }
        p.push_str("    (regex #\"/\\.env($|[\\.\\-_/].*|rc$)\")\n");
        p.push_str("    (regex #\"/\\.git/config$\")\n");

        if let Some(home) = std::env::var_os("HOME") {
            let home_path = PathBuf::from(home);
            let home_str = home_path.to_string_lossy().to_string();
            let canon_home = home_path
                .canonicalize()
                .ok()
                .map(|c| c.to_string_lossy().to_string());
            for dir in &[
                ".ssh",
                ".aws",
                ".azure",
                ".config/gcloud",
                ".kube",
                ".gnupg",
            ] {
                p.push_str(&format!("    (subpath \"{home_str}/{dir}\")\n"));
                if let Some(ref ch) = canon_home {
                    if ch != &home_str {
                        p.push_str(&format!("    (subpath \"{ch}/{dir}\")\n"));
                    }
                }
            }
        }

        p.push_str("    (literal \"/etc/passwd\")\n");
        p.push_str("    (literal \"/etc/shadow\")\n");
        p.push_str("    (literal \"/etc/sudoers\")\n");
        p.push_str("    (literal \"/etc/master.passwd\")\n");
        p.push_str("    (literal \"/private/etc/passwd\")\n");
        p.push_str("    (literal \"/private/etc/shadow\")\n");
        p.push_str("    (literal \"/private/etc/sudoers\")\n");
        p.push_str("    (literal \"/private/etc/master.passwd\")\n");
        p.push_str(")\n\n");

        // 4. File write allowances (Workspace root + safe standard targets)
        p.push_str(";; Authorized filesystem writes\n");
        p.push_str("(allow file-write*\n");
        p.push_str(&format!("    (subpath \"{ws_str}\")\n"));
        if let Some(ref canon_str) = canon_ws_opt {
            if canon_str != &ws_str {
                p.push_str(&format!("    (subpath \"{canon_str}\")\n"));
            }
        }
        for rule in &policy.fs_rules {
            if rule.access == FsAccess::Write || rule.access == FsAccess::ReadWrite {
                let path_str = resolve_rule_str(&rule.path);
                if rule.recursive {
                    p.push_str(&format!("    (subpath \"{path_str}\")\n"));
                } else {
                    p.push_str(&format!("    (literal \"{path_str}\")\n"));
                }
            }
        }
        p.push_str(")\n\n");

        // 5. Strict sensitive file WRITE denials (Symmetric with READ denials)
        p.push_str(";; Explicit denial of sensitive file writes\n");
        p.push_str("(deny file-write*\n");
        p.push_str(&format!("    (subpath \"{ws_str}/.env\")\n"));
        if let Some(ref canon_str) = canon_ws_opt {
            if canon_str != &ws_str {
                p.push_str(&format!("    (subpath \"{canon_str}/.env\")\n"));
            }
        }
        p.push_str("    (regex #\"/\\.env($|[\\.\\-_/].*|rc$)\")\n");
        p.push_str("    (regex #\"/\\.git/config$\")\n");
        if let Some(home) = std::env::var_os("HOME") {
            let home_path = PathBuf::from(home);
            let home_str = home_path.to_string_lossy().to_string();
            let canon_home = home_path
                .canonicalize()
                .ok()
                .map(|c| c.to_string_lossy().to_string());
            for dir in &[
                ".ssh",
                ".aws",
                ".azure",
                ".config/gcloud",
                ".kube",
                ".gnupg",
            ] {
                p.push_str(&format!("    (subpath \"{home_str}/{dir}\")\n"));
                if let Some(ref ch) = canon_home {
                    if ch != &home_str {
                        p.push_str(&format!("    (subpath \"{ch}/{dir}\")\n"));
                    }
                }
            }
        }
        p.push_str("    (literal \"/etc/passwd\")\n");
        p.push_str("    (literal \"/etc/shadow\")\n");
        p.push_str("    (literal \"/etc/sudoers\")\n");
        p.push_str("    (literal \"/etc/master.passwd\")\n");
        p.push_str("    (literal \"/private/etc/passwd\")\n");
        p.push_str("    (literal \"/private/etc/shadow\")\n");
        p.push_str("    (literal \"/private/etc/sudoers\")\n");
        p.push_str("    (literal \"/private/etc/master.passwd\")\n");
        p.push_str(")\n\n");

        // 6. Network confinement
        if !policy.allow_network {
            p.push_str(";; Network access prohibited\n");
            p.push_str("(deny network*)\n\n");
        }

        p
    }
}

impl SandboxDriver for SeatbeltSandboxDriver {
    fn name(&self) -> &'static str {
        "Seatbelt"
    }

    fn is_supported(&self) -> bool {
        check_seatbelt_usable()
    }

    fn prepare_command(
        &self,
        program: &str,
        args: &[&str],
        policy: &SandboxPolicy,
    ) -> Result<(Command, SandboxActivationReport), BrainError> {
        // In Advisory or Disabled mode, execute without kernel confinement while sanitizing env & logging warnings
        if policy.enforcement_mode != EnforcementMode::Enforced {
            let mut cmd = Command::new(program);
            cmd.args(args);
            policy.env_policy.sanitize_command(&mut cmd);

            let mut warnings = vec![
                "Seatbelt sandbox inactive: Advisory or Disabled enforcement mode.".to_string(),
            ];
            for arg in args {
                if let Err(violation) = policy.check_command_boundaries(arg) {
                    warnings.push(format!("Advisory boundary violation detected: {violation}"));
                }
            }

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

            return Ok((
                cmd,
                SandboxActivationReport {
                    driver_name: self.name(),
                    activated: false,
                    mode: policy.enforcement_mode,
                    workspace_root: policy.workspace_root.clone(),
                    applied_rules_count: policy.fs_rules.len(),
                    warnings,
                },
            ));
        }

        if !self.is_supported() {
            return Err(BrainError::Authorization {
                message: "macOS Seatbelt sandboxing is unavailable on this host platform. Execution denied (fail-closed)."
                    .to_string(),
            });
        }

        let profile_str = self.generate_profile(policy);

        // Wrap execution using /usr/bin/sandbox-exec
        let mut cmd = if self.sandbox_exec_path.exists() {
            let mut c = Command::new(&self.sandbox_exec_path);
            c.arg("-p").arg(&profile_str).arg(program).args(args);
            c
        } else {
            if policy.enforcement_mode == EnforcementMode::Enforced {
                return Err(BrainError::Authorization {
                    message: format!(
                        "macOS sandbox-exec binary not found at {}. Execution denied (fail-closed).",
                        self.sandbox_exec_path.display()
                    ),
                });
            }
            let mut c = Command::new(program);
            c.args(args);
            c
        };

        policy.env_policy.sanitize_command(&mut cmd);

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

        Ok((
            cmd,
            SandboxActivationReport {
                driver_name: self.name(),
                activated: true,
                mode: policy.enforcement_mode,
                workspace_root: policy.workspace_root.clone(),
                applied_rules_count: policy.fs_rules.len(),
                warnings: vec![
                    "Seatbelt is deprecated by Apple in macOS >= 27.0; subject to host entitlement constraints.".to_string(),
                ],
            },
        ))
    }

    fn apply_to_command(
        &self,
        cmd: &mut Command,
        policy: &SandboxPolicy,
    ) -> Result<SandboxActivationReport, BrainError> {
        // 1. Sanitize environment variables
        policy.env_policy.sanitize_command(cmd);

        if policy.enforcement_mode != EnforcementMode::Enforced {
            let warnings = vec![
                "Seatbelt sandbox inactive: Advisory or Disabled enforcement mode.".to_string(),
            ];
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
            return Ok(SandboxActivationReport {
                driver_name: self.name(),
                activated: false,
                mode: policy.enforcement_mode,
                workspace_root: policy.workspace_root.clone(),
                applied_rules_count: policy.fs_rules.len(),
                warnings,
            });
        }

        // 2. Check platform capability
        if !self.is_supported() {
            return Err(BrainError::Authorization {
                message: "macOS Seatbelt sandboxing is unavailable on this host platform. Execution denied (fail-closed)."
                    .to_string(),
            });
        }

        // 3. Generate policy profile
        let profile_str = self.generate_profile(policy);

        #[cfg(all(unix, target_os = "macos"))]
        {
            use std::os::unix::process::CommandExt;

            let profile_c = match CString::new(profile_str) {
                Ok(c) => c,
                Err(e) => {
                    return Err(BrainError::Authorization {
                        message: format!("Seatbelt profile contains invalid NUL byte: {e}"),
                    });
                }
            };

            // In child pre_exec: close non-standard FDs and invoke sandbox_init
            unsafe {
                cmd.pre_exec(move || {
                    let max_fd = libc::sysconf(libc::_SC_OPEN_MAX).clamp(256, 65536) as i32;
                    for fd in 3..max_fd {
                        libc::close(fd);
                    }

                    let mut err_ptr: *mut libc::c_char = std::ptr::null_mut();
                    let ret = sandbox_init(profile_c.as_ptr(), 0, &mut err_ptr);
                    if ret != 0 {
                        let err_msg = if !err_ptr.is_null() {
                            let msg = std::ffi::CStr::from_ptr(err_ptr)
                                .to_string_lossy()
                                .into_owned();
                            sandbox_free_error(err_ptr);
                            msg
                        } else {
                            "Seatbelt sandbox_init activation rejected by macOS kernel".to_string()
                        };
                        return Err(std::io::Error::new(
                            std::io::ErrorKind::PermissionDenied,
                            format!("Seatbelt activation failed: {err_msg}"),
                        ));
                    }
                    Ok(())
                });
            }
        }

        Ok(SandboxActivationReport {
            driver_name: self.name(),
            activated: true,
            mode: policy.enforcement_mode,
            workspace_root: policy.workspace_root.clone(),
            applied_rules_count: policy.fs_rules.len(),
            warnings: vec![
                "Seatbelt is deprecated by Apple in macOS >= 27.0; subject to host entitlement constraints.".to_string(),
            ],
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_seatbelt_profile_generation_deny_default_and_sensitive_blocks() {
        let driver = SeatbeltSandboxDriver::new();
        let ws = PathBuf::from("/Users/testuser/project");
        let policy = SandboxPolicy::builder(&ws)
            .with_system_defaults()
            .with_sensitive_defaults()
            .allow_read("~/custom_config", true)
            .build();

        let profile = driver.generate_profile(&policy);
        assert!(profile.contains("(version 1)"));
        assert!(profile.contains("(deny default)"));
        assert!(profile.contains("(allow process-fork)"));
        assert!(profile.contains("(allow sysctl-read)"));

        // Authorized workspace
        assert!(profile.contains(&format!("(subpath \"{}\")", ws.to_string_lossy())));

        // Explicit sensitive denials for read AND write
        assert!(profile.contains("(deny file-read*"));
        assert!(profile.contains("(deny file-write*"));
        assert!(profile.contains("/\\.env($|[\\.\\-_/].*|rc$)"));
        assert!(profile.contains("/\\.git/config$"));
        assert!(profile.contains("/.ssh"));
        assert!(profile.contains("/.azure"));
        assert!(profile.contains("/etc/passwd"));
        assert!(profile.contains("/private/etc/shadow"));

        // Tilde expansion in rules
        if let Some(home) = std::env::var_os("HOME") {
            let home_str = PathBuf::from(home).to_string_lossy().to_string();
            assert!(profile.contains(&format!("{home_str}/custom_config")));
            assert!(!profile.contains("(subpath \"~/"));
        }

        // No NUL bytes
        assert!(!profile.contains('\0'));
    }
}
