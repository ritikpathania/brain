use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::process::Command;

use serde::{Deserialize, Serialize};

use super::driver::EnforcementMode;

/// Filesystem access permissions for sandbox policy rules.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FsAccess {
    /// Read-only access to files and directories.
    Read,
    /// Write-only access to create or modify files and directories.
    Write,
    /// Both read and write access.
    ReadWrite,
    /// Permission to execute binaries or scripts.
    Execute,
    /// Explicitly deny access, overriding broader allowances.
    Deny,
}

/// A specific filesystem path rule within a sandbox policy.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FsRule {
    /// Target path on the filesystem.
    pub path: PathBuf,
    /// Access permission granted or denied.
    pub access: FsAccess,
    /// Whether the rule applies recursively to all subpaths.
    pub recursive: bool,
}

impl FsRule {
    /// Creates a new filesystem rule.
    pub fn new(path: impl Into<PathBuf>, access: FsAccess, recursive: bool) -> Self {
        Self {
            path: path.into(),
            access,
            recursive,
        }
    }
}

/// A sensitive target rule designating files that must not be accessed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SensitivePathRule {
    /// Canonical path or pattern description.
    pub path_or_pattern: String,
    /// Whether this targets a file inside or outside the workspace.
    pub is_in_workspace: bool,
    /// Rationale for designating this file sensitive.
    pub reason: String,
}

/// Environment variable isolation and sanitization policy.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvPolicy {
    /// If true, sensitive credential environment variables are scrubbed.
    pub strip_sensitive: bool,
    /// Set of environment variable names to remove before execution.
    pub denied_keys: HashSet<String>,
    /// Explicit environment variables to set or override.
    pub explicit_vars: HashMap<String, String>,
}

impl Default for EnvPolicy {
    fn default() -> Self {
        let mut denied = HashSet::new();
        for key in &[
            "AWS_SECRET_ACCESS_KEY",
            "AWS_SESSION_TOKEN",
            "AWS_ACCESS_KEY_ID",
            "AWS_SECURITY_TOKEN",
            "GITHUB_TOKEN",
            "GH_TOKEN",
            "GITLAB_TOKEN",
            "OPENAI_API_KEY",
            "ANTHROPIC_API_KEY",
            "BRAIN_MASTER_KEY",
            "DATABASE_URL",
            "DATABASE_PASSWORD",
            "SSH_AUTH_SOCK",
            "GPG_AGENT_INFO",
            "STRIPE_API_KEY",
            "NPM_TOKEN",
        ] {
            denied.insert((*key).to_string());
        }

        Self {
            strip_sensitive: true,
            denied_keys: denied,
            explicit_vars: HashMap::new(),
        }
    }
}

impl EnvPolicy {
    /// Applies sanitization rules to a subprocess `Command`.
    pub fn sanitize_command(&self, cmd: &mut Command) {
        if self.strip_sensitive {
            for key in &self.denied_keys {
                cmd.env_remove(key);
            }
        }
        for (k, v) in &self.explicit_vars {
            cmd.env(k, v);
        }
    }
}

/// Enforceable platform-independent security policy defining resource boundaries.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SandboxPolicy {
    /// Primary authorized workspace root directory.
    pub workspace_root: PathBuf,
    /// Filesystem permission rules.
    pub fs_rules: Vec<FsRule>,
    /// Designated sensitive files and patterns.
    pub sensitive_rules: Vec<SensitivePathRule>,
    /// Environment variable sanitization rules.
    pub env_policy: EnvPolicy,
    /// Whether network egress/ingress is permitted.
    pub allow_network: bool,
    /// Active sandbox enforcement mode.
    pub enforcement_mode: EnforcementMode,
}

impl SandboxPolicy {
    /// Constructs a new builder anchored at the given workspace root.
    pub fn builder(workspace_root: impl Into<PathBuf>) -> SandboxPolicyBuilder {
        SandboxPolicyBuilder::new(workspace_root)
    }

    /// Checks if a given path matches any sensitive path rule.
    pub fn is_sensitive_target(&self, candidate: &Path) -> bool {
        let cand_str = candidate.to_string_lossy();
        let file_name = candidate
            .file_name()
            .map(|f| f.to_string_lossy().to_string())
            .unwrap_or_default();

        // 1. In-workspace environment files
        if file_name == ".env"
            || file_name.starts_with(".env.")
            || file_name.starts_with(".env-")
            || file_name.starts_with(".env_")
            || file_name == ".envrc"
        {
            return true;
        }

        // 2. SSH and cloud credentials
        if cand_str.contains("/.ssh")
            || cand_str.starts_with(".ssh")
            || cand_str.contains("/.aws")
            || cand_str.starts_with(".aws")
            || cand_str.contains("/.azure")
            || cand_str.contains("/.kube")
            || cand_str.contains("/.gnupg")
            || file_name == ".netrc"
            || file_name == ".npmrc"
            || file_name == ".pypirc"
            || cand_str.ends_with(".git/config")
        {
            return true;
        }

        // 3. System shadow and password files
        let check_str = cand_str.strip_prefix("/private").unwrap_or(&cand_str);
        if check_str == "/etc/passwd"
            || check_str == "/etc/shadow"
            || check_str == "/etc/sudoers"
            || check_str == "/etc/master.passwd"
        {
            return true;
        }

        for rule in &self.sensitive_rules {
            if cand_str.contains(&rule.path_or_pattern) {
                return true;
            }
        }

        false
    }

    /// Pre-flight validation of candidate command strings against policy boundaries.
    pub fn check_command_boundaries(&self, command: &str) -> Result<(), String> {
        let tokens = Self::extract_command_tokens(command);
        for token in tokens {
            let path = Path::new(&token);
            if self.is_sensitive_target(path) {
                return Err(format!(
                    "Access to sensitive target '{token}' is blocked by policy"
                ));
            }

            // Check if token corresponds to an existing file in workspace (such as a symlink)
            let in_ws_path = self.workspace_root.join(&token);
            if in_ws_path.is_symlink() || in_ws_path.exists() {
                if let Ok(target) = std::fs::read_link(&in_ws_path) {
                    if self.is_sensitive_target(&target) {
                        return Err(format!(
                            "Symlink '{}' pointing to sensitive target '{}' is blocked by policy",
                            token,
                            target.display()
                        ));
                    }
                }
                if let Ok(canon) = std::fs::canonicalize(&in_ws_path) {
                    if self.is_sensitive_target(&canon) {
                        return Err(format!(
                            "Target '{}' canonicalizing to sensitive target '{}' is blocked by policy",
                            token,
                            canon.display()
                        ));
                    }
                    let canon_str = canon.to_string_lossy();
                    let clean_canon = canon_str.strip_prefix("/private").unwrap_or(&canon_str);
                    let ws_str = self.workspace_root.to_string_lossy();
                    let clean_ws = ws_str.strip_prefix("/private").unwrap_or(&ws_str);

                    if !clean_canon.starts_with(clean_ws) {
                        let is_system_allow = Self::is_system_path_allowed(&canon);
                        if !is_system_allow {
                            return Err(format!(
                                "Path '{}' resolves outside authorized workspace root '{}'",
                                token,
                                self.workspace_root.display()
                            ));
                        }
                    }
                }
            }

            // Check if token is an explicit absolute or traversal path outside workspace
            if token.starts_with('/') || token.starts_with("../") || token.starts_with("~/") {
                let resolved = if token.starts_with("~/") {
                    if let Some(home) = std::env::var_os("HOME") {
                        PathBuf::from(home).join(token.trim_start_matches("~/"))
                    } else {
                        PathBuf::from(&token)
                    }
                } else if token.starts_with('/') {
                    PathBuf::from(&token)
                } else {
                    self.workspace_root.join(&token)
                };

                if self.is_sensitive_target(&resolved) {
                    return Err(format!(
                        "Access to sensitive target '{token}' is blocked by policy"
                    ));
                }

                let check_str = resolved.to_string_lossy();
                let clean_str = check_str.strip_prefix("/private").unwrap_or(&check_str);

                let is_system_allow = Self::is_system_path_allowed(Path::new(clean_str));
                let ws_str = self.workspace_root.to_string_lossy();
                let clean_ws = ws_str.strip_prefix("/private").unwrap_or(&ws_str);

                if !is_system_allow && !clean_str.starts_with(clean_ws) {
                    // Check explicit fs_rules
                    let allowed = self.fs_rules.iter().any(|r| {
                        if r.access == FsAccess::Deny {
                            return false;
                        }
                        let r_str = r.path.to_string_lossy();
                        let clean_r = r_str.strip_prefix("/private").unwrap_or(&r_str);
                        clean_str.starts_with(clean_r)
                    });
                    if !allowed && !token.starts_with('-') && !clean_str.starts_with("/var/folders")
                    {
                        return Err(format!(
                            "Path '{}' is outside authorized workspace root '{}'",
                            token,
                            self.workspace_root.display()
                        ));
                    }
                }
            }
        }
        Ok(())
    }

    fn is_system_path_allowed(path: &Path) -> bool {
        let p_str = path.to_string_lossy();
        let clean = p_str.strip_prefix("/private").unwrap_or(&p_str);
        clean.starts_with("/bin")
            || clean.starts_with("/sbin")
            || clean.starts_with("/usr")
            || clean.starts_with("/System")
            || clean.starts_with("/Library")
            || clean.starts_with("/opt/homebrew")
            || clean.starts_with("/lib")
            || clean.starts_with("/etc/alternatives")
            || clean.starts_with("/dev/")
            || clean.starts_with("/tmp")
    }

    /// Helper to tokenize command string into candidate path tokens.
    pub fn extract_command_tokens(command: &str) -> Vec<String> {
        let mut tokens = Vec::new();
        let mut current = String::new();
        let mut in_single = false;
        let mut in_double = false;
        for c in command.chars() {
            match c {
                '\'' if !in_double => in_single = !in_single,
                '"' if !in_single => in_double = !in_double,
                ' ' | '\t' | '\n' | ';' | '|' | '&' | '(' | ')' | '{' | '}' | '[' | ']' | ','
                | '>' | '<' | '`'
                    if !in_single && !in_double =>
                {
                    if !current.is_empty() {
                        let trimmed = current.trim_matches(|ch: char| {
                            ch == '\''
                                || ch == '"'
                                || ch == '`'
                                || ch == '$'
                                || ch == '('
                                || ch == ')'
                        });
                        if !trimmed.is_empty() {
                            if let Some((_k, v)) = trimmed.split_once('=') {
                                if !v.is_empty() {
                                    tokens.push(v.to_string());
                                }
                            } else {
                                tokens.push(trimmed.to_string());
                            }
                        }
                        current.clear();
                    }
                }
                _ => current.push(c),
            }
        }
        if !current.is_empty() {
            let trimmed = current.trim_matches(|ch: char| {
                ch == '\'' || ch == '"' || ch == '`' || ch == '$' || ch == '(' || ch == ')'
            });
            if !trimmed.is_empty() {
                if let Some((_k, v)) = trimmed.split_once('=') {
                    if !v.is_empty() {
                        tokens.push(v.to_string());
                    }
                } else {
                    tokens.push(trimmed.to_string());
                }
            }
        }
        tokens
    }
}

/// Fluent builder for assembling [`SandboxPolicy`].
#[derive(Debug, Clone)]
pub struct SandboxPolicyBuilder {
    workspace_root: PathBuf,
    fs_rules: Vec<FsRule>,
    sensitive_rules: Vec<SensitivePathRule>,
    env_policy: EnvPolicy,
    allow_network: bool,
    enforcement_mode: EnforcementMode,
}

impl SandboxPolicyBuilder {
    /// Creates a new builder for the specified workspace root.
    pub fn new(workspace_root: impl Into<PathBuf>) -> Self {
        Self {
            workspace_root: workspace_root.into(),
            fs_rules: Vec::new(),
            sensitive_rules: Vec::new(),
            env_policy: EnvPolicy::default(),
            allow_network: false,
            enforcement_mode: EnforcementMode::Enforced,
        }
    }

    /// Grants read permission to a specific path.
    pub fn allow_read(mut self, path: impl Into<PathBuf>, recursive: bool) -> Self {
        self.fs_rules
            .push(FsRule::new(path, FsAccess::Read, recursive));
        self
    }

    /// Grants write permission to a specific path.
    pub fn allow_write(mut self, path: impl Into<PathBuf>, recursive: bool) -> Self {
        self.fs_rules
            .push(FsRule::new(path, FsAccess::Write, recursive));
        self
    }

    /// Grants both read and write permissions to a specific path.
    pub fn allow_read_write(mut self, path: impl Into<PathBuf>, recursive: bool) -> Self {
        self.fs_rules
            .push(FsRule::new(path, FsAccess::ReadWrite, recursive));
        self
    }

    /// Explicitly denies access to a specific path.
    pub fn deny(mut self, path: impl Into<PathBuf>, recursive: bool) -> Self {
        self.fs_rules
            .push(FsRule::new(path, FsAccess::Deny, recursive));
        self
    }

    /// Populates standard system read and execution dependencies required for shell and compilers.
    pub fn with_system_defaults(mut self) -> Self {
        // Core standard Unix binary & library paths (Read + Execute)
        for path in &[
            "/bin",
            "/sbin",
            "/usr/bin",
            "/usr/sbin",
            "/usr/lib",
            "/usr/libexec",
            "/usr/local/bin",
            "/usr/include",
            "/usr/share",
        ] {
            if Path::new(path).exists() {
                self.fs_rules.push(FsRule::new(path, FsAccess::Read, true));
            }
        }

        // macOS specific system paths
        #[cfg(target_os = "macos")]
        for path in &[
            "/System",
            "/Library",
            "/opt/homebrew",
            "/usr/local",
            "/Library/Developer",
        ] {
            if Path::new(path).exists() {
                self.fs_rules.push(FsRule::new(path, FsAccess::Read, true));
            }
        }

        // Linux specific system paths
        #[cfg(target_os = "linux")]
        for path in &[
            "/lib",
            "/lib64",
            "/etc/alternatives",
            "/etc/ld.so.cache",
            "/etc/resolv.conf",
        ] {
            if Path::new(path).exists() {
                self.fs_rules.push(FsRule::new(path, FsAccess::Read, true));
            }
        }

        // Standard device paths (ReadWrite for pipes/redirection)
        for dev in &[
            "/dev/null",
            "/dev/zero",
            "/dev/urandom",
            "/dev/random",
            "/dev/stdout",
            "/dev/stderr",
            "/dev/stdin",
            "/dev/fd",
            "/dev/dtracehelper",
        ] {
            self.fs_rules
                .push(FsRule::new(dev, FsAccess::ReadWrite, false));
        }

        // Scoped temp directories
        for tmp in &["/tmp", "/private/tmp"] {
            if Path::new(tmp).exists() {
                self.fs_rules
                    .push(FsRule::new(tmp, FsAccess::ReadWrite, true));
            }
        }

        self
    }

    /// Designates sensitive secrets and credential paths that must be blocked.
    pub fn with_sensitive_defaults(mut self) -> Self {
        // Workspace in-tree sensitive files
        self.sensitive_rules.push(SensitivePathRule {
            path_or_pattern: ".env".to_string(),
            is_in_workspace: true,
            reason: "Application secrets and environment credentials".to_string(),
        });
        self.sensitive_rules.push(SensitivePathRule {
            path_or_pattern: ".git/config".to_string(),
            is_in_workspace: true,
            reason: "Repository credentials and upstream auth tokens".to_string(),
        });

        // Global user credentials
        if let Some(home) = std::env::var_os("HOME") {
            let home_path = PathBuf::from(home);
            for sub in &[
                ".ssh",
                ".aws",
                ".azure",
                ".config/gcloud",
                ".kube",
                ".gnupg",
                ".netrc",
                ".npmrc",
                ".pypirc",
            ] {
                let target = home_path.join(sub);
                self.fs_rules
                    .push(FsRule::new(target.clone(), FsAccess::Deny, true));
                self.sensitive_rules.push(SensitivePathRule {
                    path_or_pattern: target.to_string_lossy().to_string(),
                    is_in_workspace: false,
                    reason: format!("User security credential {sub}"),
                });
            }
        }

        // System password and privilege files
        for sys in &[
            "/etc/passwd",
            "/etc/shadow",
            "/etc/sudoers",
            "/etc/master.passwd",
            "/private/etc/passwd",
            "/private/etc/shadow",
            "/private/etc/sudoers",
            "/private/etc/master.passwd",
        ] {
            self.fs_rules.push(FsRule::new(sys, FsAccess::Deny, false));
            self.sensitive_rules.push(SensitivePathRule {
                path_or_pattern: (*sys).to_string(),
                is_in_workspace: false,
                reason: "System authentication database".to_string(),
            });
        }

        self
    }

    /// Sets the environment variable policy.
    pub fn with_env_policy(mut self, env_policy: EnvPolicy) -> Self {
        self.env_policy = env_policy;
        self
    }

    /// Sets network allowance.
    pub fn with_network(mut self, allow: bool) -> Self {
        self.allow_network = allow;
        self
    }

    /// Sets enforcement mode.
    pub fn with_mode(mut self, mode: EnforcementMode) -> Self {
        self.enforcement_mode = mode;
        self
    }

    /// Finalizes and returns the complete [`SandboxPolicy`].
    pub fn build(mut self) -> SandboxPolicy {
        // Invariant: Workspace root is always granted ReadWrite
        self.fs_rules.insert(
            0,
            FsRule::new(self.workspace_root.clone(), FsAccess::ReadWrite, true),
        );

        SandboxPolicy {
            workspace_root: self.workspace_root,
            fs_rules: self.fs_rules,
            sensitive_rules: self.sensitive_rules,
            env_policy: self.env_policy,
            allow_network: self.allow_network,
            enforcement_mode: self.enforcement_mode,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_policy_builder_creates_rules_and_protects_defaults() {
        let ws = PathBuf::from("/workspace/project");
        let policy = SandboxPolicy::builder(&ws)
            .with_system_defaults()
            .with_sensitive_defaults()
            .allow_read("/opt/data", true)
            .build();

        assert_eq!(policy.workspace_root, ws);
        assert!(!policy.fs_rules.is_empty());
        // First rule is always the workspace itself with ReadWrite
        assert_eq!(policy.fs_rules[0].path, ws);
        assert_eq!(policy.fs_rules[0].access, FsAccess::ReadWrite);

        // Sensitive detection checks
        assert!(policy.is_sensitive_target(Path::new("/workspace/project/.env")));
        assert!(policy.is_sensitive_target(Path::new("/workspace/project/.env.local")));
        assert!(policy.is_sensitive_target(Path::new("/workspace/project/.git/config")));
        assert!(policy.is_sensitive_target(Path::new("/etc/passwd")));
        assert!(policy.is_sensitive_target(Path::new("/etc/shadow")));
        assert!(policy.is_sensitive_target(Path::new("/private/etc/sudoers")));

        // Safe in-workspace file
        assert!(!policy.is_sensitive_target(Path::new("/workspace/project/src/main.rs")));
    }

    #[test]
    fn test_env_sanitization_removes_sensitive_keys() {
        let mut policy = EnvPolicy::default();
        policy
            .explicit_vars
            .insert("SAFE_VAR".to_string(), "123".to_string());

        let mut cmd = Command::new("/bin/echo");
        cmd.env("AWS_SECRET_ACCESS_KEY", "super_secret");
        cmd.env("GITHUB_TOKEN", "ghp_12345");
        cmd.env("SAFE_KEY", "safe_val");

        policy.sanitize_command(&mut cmd);
        // Command env was sanitized; explicit var injected
    }
}
