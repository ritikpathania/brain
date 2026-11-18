use std::path::PathBuf;
use std::process::Command;

use brain_core::errors::BrainError;

use super::driver::{EnforcementMode, SandboxActivationReport, SandboxDriver};
use super::policy::{FsAccess, SandboxPolicy};

// Landlock syscall constants and bitmasks (Linux kernel 5.13+)
pub const LANDLOCK_CREATE_RULESET_VERSION: u32 = 1 << 0;
pub const LANDLOCK_RULE_PATH_BENEATH: u32 = 1;

// Access flags ABI 1
pub const LANDLOCK_ACCESS_FS_EXECUTE: u64 = 1 << 0;
pub const LANDLOCK_ACCESS_FS_WRITE_FILE: u64 = 1 << 1;
pub const LANDLOCK_ACCESS_FS_READ_FILE: u64 = 1 << 2;
pub const LANDLOCK_ACCESS_FS_READ_DIR: u64 = 1 << 3;
pub const LANDLOCK_ACCESS_FS_REMOVE_DIR: u64 = 1 << 4;
pub const LANDLOCK_ACCESS_FS_REMOVE_FILE: u64 = 1 << 5;
pub const LANDLOCK_ACCESS_FS_MAKE_CHAR: u64 = 1 << 6;
pub const LANDLOCK_ACCESS_FS_MAKE_DIR: u64 = 1 << 7;
pub const LANDLOCK_ACCESS_FS_MAKE_REG: u64 = 1 << 8;
pub const LANDLOCK_ACCESS_FS_MAKE_SOCK: u64 = 1 << 9;
pub const LANDLOCK_ACCESS_FS_MAKE_FIFO: u64 = 1 << 10;
pub const LANDLOCK_ACCESS_FS_MAKE_BLOCK: u64 = 1 << 11;
pub const LANDLOCK_ACCESS_FS_MAKE_SYM: u64 = 1 << 12;
// Access flags ABI 2+
pub const LANDLOCK_ACCESS_FS_REFER: u64 = 1 << 13;
// Access flags ABI 3+
pub const LANDLOCK_ACCESS_FS_TRUNCATE: u64 = 1 << 14;

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct LandlockRulesetAttr {
    pub handled_access_fs: u64,
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct LandlockPathBeneathAttr {
    pub allowed_access: u64,
    pub parent_fd: i32,
}

/// Linux Landlock LSM sandbox driver using unprivileged kernel filesystem confinement.
#[derive(Debug, Clone, Default)]
pub struct LandlockSandboxDriver;

impl LandlockSandboxDriver {
    pub fn new() -> Self {
        Self
    }

    /// Queries the kernel Landlock ABI version if running on Linux.
    /// Returns 0 or negative if Landlock is unavailable.
    pub fn query_abi_version(&self) -> i32 {
        #[cfg(target_os = "linux")]
        {
            // Syscall 444 is landlock_create_ruleset on Linux x86_64 and arm64
            let res = unsafe {
                libc::syscall(
                    444,
                    std::ptr::null::<LandlockRulesetAttr>(),
                    0,
                    LANDLOCK_CREATE_RULESET_VERSION,
                )
            };
            res as i32
        }
        #[cfg(not(target_os = "linux"))]
        {
            -1
        }
    }

    /// Computes handled filesystem access bitmask based on detected ABI version.
    pub fn compute_handled_access_mask(&self, abi: i32) -> u64 {
        let mut mask = LANDLOCK_ACCESS_FS_EXECUTE
            | LANDLOCK_ACCESS_FS_WRITE_FILE
            | LANDLOCK_ACCESS_FS_READ_FILE
            | LANDLOCK_ACCESS_FS_READ_DIR
            | LANDLOCK_ACCESS_FS_REMOVE_DIR
            | LANDLOCK_ACCESS_FS_REMOVE_FILE
            | LANDLOCK_ACCESS_FS_MAKE_CHAR
            | LANDLOCK_ACCESS_FS_MAKE_DIR
            | LANDLOCK_ACCESS_FS_MAKE_REG
            | LANDLOCK_ACCESS_FS_MAKE_SOCK
            | LANDLOCK_ACCESS_FS_MAKE_FIFO
            | LANDLOCK_ACCESS_FS_MAKE_BLOCK
            | LANDLOCK_ACCESS_FS_MAKE_SYM;

        if abi >= 2 {
            mask |= LANDLOCK_ACCESS_FS_REFER;
        }
        if abi >= 3 {
            mask |= LANDLOCK_ACCESS_FS_TRUNCATE;
        }
        mask
    }
}

impl SandboxDriver for LandlockSandboxDriver {
    fn name(&self) -> &'static str {
        "Landlock"
    }

    fn is_supported(&self) -> bool {
        self.query_abi_version() >= 1
    }

    fn apply_to_command(
        &self,
        cmd: &mut Command,
        policy: &SandboxPolicy,
    ) -> Result<SandboxActivationReport, BrainError> {
        // 1. Sanitize environment variables
        policy.env_policy.sanitize_command(cmd);

        let abi = self.query_abi_version();
        let is_avail = abi >= 1;

        if !is_avail {
            if policy.enforcement_mode == EnforcementMode::Enforced {
                return Err(BrainError::Authorization {
                    message: format!(
                        "Linux Landlock kernel LSM is not available or disabled (ABI query returned {abi}). Execution denied (fail-closed)."
                    ),
                });
            } else {
                return Ok(SandboxActivationReport {
                    driver_name: self.name(),
                    activated: false,
                    mode: policy.enforcement_mode,
                    workspace_root: policy.workspace_root.clone(),
                    applied_rules_count: 0,
                    warnings: vec!["Landlock not supported on this kernel".to_string()],
                });
            }
        }

        let handled_mask = self.compute_handled_access_mask(abi);

        // Collect allowed rules for child pre_exec installation
        let mut child_rules: Vec<(PathBuf, u64)> = Vec::new();

        // Authorized workspace root: full read/write and execute
        let mut ws_access = LANDLOCK_ACCESS_FS_READ_FILE
            | LANDLOCK_ACCESS_FS_READ_DIR
            | LANDLOCK_ACCESS_FS_WRITE_FILE
            | LANDLOCK_ACCESS_FS_REMOVE_FILE
            | LANDLOCK_ACCESS_FS_REMOVE_DIR
            | LANDLOCK_ACCESS_FS_MAKE_DIR
            | LANDLOCK_ACCESS_FS_MAKE_REG
            | LANDLOCK_ACCESS_FS_MAKE_SYM
            | LANDLOCK_ACCESS_FS_EXECUTE;
        if abi >= 2 {
            ws_access |= LANDLOCK_ACCESS_FS_REFER;
        }
        let has_workspace_secrets = policy.sensitive_rules.iter().any(|r| r.is_in_workspace);
        if has_workspace_secrets && policy.workspace_root.is_dir() {
            // Workspace root gets directory operations and execute, but not blanket file read or write
            let base_ws_access = (ws_access
                & !(LANDLOCK_ACCESS_FS_READ_FILE | LANDLOCK_ACCESS_FS_WRITE_FILE))
                & handled_mask;
            child_rules.push((policy.workspace_root.clone(), base_ws_access));

            // Grant read and write access specifically to non-sensitive entries in workspace root
            if let Ok(entries) = std::fs::read_dir(&policy.workspace_root) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if !policy.is_sensitive_target(&path) {
                        child_rules.push((
                            path,
                            (LANDLOCK_ACCESS_FS_READ_FILE
                                | LANDLOCK_ACCESS_FS_WRITE_FILE
                                | LANDLOCK_ACCESS_FS_READ_DIR
                                | LANDLOCK_ACCESS_FS_EXECUTE)
                                & handled_mask,
                        ));
                    }
                }
            }
        } else {
            child_rules.push((policy.workspace_root.clone(), ws_access & handled_mask));
        }

        for rule in &policy.fs_rules {
            if rule.access == FsAccess::Deny {
                continue;
            }
            let resolved_path = if rule.path.starts_with("~/") {
                if let Some(home) = std::env::var_os("HOME") {
                    PathBuf::from(home).join(rule.path.to_string_lossy().trim_start_matches("~/"))
                } else {
                    rule.path.clone()
                }
            } else {
                rule.path.clone()
            };
            let mut access = 0u64;
            match rule.access {
                FsAccess::Read => {
                    access |= LANDLOCK_ACCESS_FS_READ_FILE
                        | LANDLOCK_ACCESS_FS_READ_DIR
                        | LANDLOCK_ACCESS_FS_EXECUTE;
                }
                FsAccess::Write => {
                    access |= LANDLOCK_ACCESS_FS_WRITE_FILE
                        | LANDLOCK_ACCESS_FS_REMOVE_FILE
                        | LANDLOCK_ACCESS_FS_REMOVE_DIR
                        | LANDLOCK_ACCESS_FS_MAKE_DIR
                        | LANDLOCK_ACCESS_FS_MAKE_REG;
                    if abi >= 3 {
                        access |= LANDLOCK_ACCESS_FS_TRUNCATE;
                    }
                }
                FsAccess::ReadWrite => {
                    access |= LANDLOCK_ACCESS_FS_READ_FILE
                        | LANDLOCK_ACCESS_FS_READ_DIR
                        | LANDLOCK_ACCESS_FS_WRITE_FILE
                        | LANDLOCK_ACCESS_FS_REMOVE_FILE
                        | LANDLOCK_ACCESS_FS_REMOVE_DIR
                        | LANDLOCK_ACCESS_FS_MAKE_DIR
                        | LANDLOCK_ACCESS_FS_MAKE_REG
                        | LANDLOCK_ACCESS_FS_EXECUTE;
                    if abi >= 2 {
                        access |= LANDLOCK_ACCESS_FS_REFER;
                    }
                    if abi >= 3 {
                        access |= LANDLOCK_ACCESS_FS_TRUNCATE;
                    }
                }
                FsAccess::Execute => {
                    access |= LANDLOCK_ACCESS_FS_EXECUTE | LANDLOCK_ACCESS_FS_READ_FILE;
                }
                FsAccess::Deny => {}
            }
            if access != 0 && resolved_path.exists() {
                child_rules.push((resolved_path, access & handled_mask));
            }
        }

        #[cfg(all(unix, target_os = "linux"))]
        {
            use std::ffi::CString;
            use std::os::unix::process::CommandExt;

            let rules_to_apply = child_rules.clone();

            unsafe {
                cmd.pre_exec(move || {
                    let max_fd = libc::sysconf(libc::_SC_OPEN_MAX).clamp(256, 65536) as i32;
                    for fd in 3..max_fd {
                        libc::close(fd);
                    }

                    // 1. Landlock requires PR_SET_NO_NEW_PRIVS before restricting without CAP_SYS_ADMIN
                    if libc::prctl(libc::PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0) != 0 {
                        return Err(std::io::Error::last_os_error());
                    }

                    // 2. Create the ruleset
                    let attr = LandlockRulesetAttr {
                        handled_access_fs: handled_mask,
                    };
                    let ruleset_fd = libc::syscall(
                        444, // SYS_landlock_create_ruleset
                        &attr as *const LandlockRulesetAttr,
                        std::mem::size_of::<LandlockRulesetAttr>(),
                        0,
                    ) as i32;

                    if ruleset_fd < 0 {
                        return Err(std::io::Error::last_os_error());
                    }

                    // 3. Add rules for each allowed path beneath
                    for (path, access) in &rules_to_apply {
                        if let Ok(c_path) = CString::new(path.to_string_lossy().as_bytes()) {
                            let mut is_dir = true;
                            let mut dir_fd = libc::open(
                                c_path.as_ptr(),
                                libc::O_PATH | libc::O_CLOEXEC | libc::O_DIRECTORY,
                            );
                            if dir_fd < 0 {
                                is_dir = false;
                                dir_fd =
                                    libc::open(c_path.as_ptr(), libc::O_PATH | libc::O_CLOEXEC);
                            }
                            if dir_fd >= 0 {
                                let mut effective_access = *access;
                                if !is_dir {
                                    effective_access &= !(LANDLOCK_ACCESS_FS_READ_DIR
                                        | LANDLOCK_ACCESS_FS_REMOVE_DIR
                                        | LANDLOCK_ACCESS_FS_MAKE_DIR);
                                }
                                let pb_attr = LandlockPathBeneathAttr {
                                    allowed_access: effective_access,
                                    parent_fd: dir_fd,
                                };
                                let _ = libc::syscall(
                                    445, // SYS_landlock_add_rule
                                    ruleset_fd,
                                    LANDLOCK_RULE_PATH_BENEATH,
                                    &pb_attr as *const LandlockPathBeneathAttr,
                                    0,
                                );
                                libc::close(dir_fd);
                            }
                        }
                    }

                    // 4. Enforce ruleset on current process (and all subsequent child forks)
                    let restrict_res = libc::syscall(
                        446, // SYS_landlock_restrict_self
                        ruleset_fd, 0,
                    );
                    libc::close(ruleset_fd);

                    if restrict_res != 0 {
                        return Err(std::io::Error::last_os_error());
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
            applied_rules_count: child_rules.len(),
            warnings: Vec::new(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_landlock_access_masks_by_abi() {
        let driver = LandlockSandboxDriver::new();

        let mask1 = driver.compute_handled_access_mask(1);
        assert_eq!(
            mask1 & LANDLOCK_ACCESS_FS_EXECUTE,
            LANDLOCK_ACCESS_FS_EXECUTE
        );
        assert_eq!(
            mask1 & LANDLOCK_ACCESS_FS_READ_FILE,
            LANDLOCK_ACCESS_FS_READ_FILE
        );
        assert_eq!(mask1 & LANDLOCK_ACCESS_FS_REFER, 0);
        assert_eq!(mask1 & LANDLOCK_ACCESS_FS_TRUNCATE, 0);

        let mask2 = driver.compute_handled_access_mask(2);
        assert_eq!(mask2 & LANDLOCK_ACCESS_FS_REFER, LANDLOCK_ACCESS_FS_REFER);
        assert_eq!(mask2 & LANDLOCK_ACCESS_FS_TRUNCATE, 0);

        let mask3 = driver.compute_handled_access_mask(3);
        assert_eq!(mask3 & LANDLOCK_ACCESS_FS_REFER, LANDLOCK_ACCESS_FS_REFER);
        assert_eq!(
            mask3 & LANDLOCK_ACCESS_FS_TRUNCATE,
            LANDLOCK_ACCESS_FS_TRUNCATE
        );
    }

    #[test]
    fn test_landlock_fails_closed_when_unsupported_on_non_linux() {
        let driver = LandlockSandboxDriver::new();
        let ws = PathBuf::from("/tmp/workspace");
        let policy = SandboxPolicy::builder(&ws)
            .with_mode(EnforcementMode::Enforced)
            .build();
        let mut cmd = Command::new("/bin/echo");

        // On macOS or kernels without Landlock, must fail closed
        if !driver.is_supported() {
            let res = driver.apply_to_command(&mut cmd, &policy);
            assert!(res.is_err());
            assert!(matches!(res.unwrap_err(), BrainError::Authorization { .. }));
        }
    }
}
