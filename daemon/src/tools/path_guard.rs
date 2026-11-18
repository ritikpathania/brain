//! Security path confinement and credential guard for tool execution (P0-B / SEC-03).
//!
//! Enforces workspace boundary confinement and sensitive credential protection across
//! file access and command execution entry points.

use std::path::{Component, Path, PathBuf};

/// Security evaluation results for tool invocation.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SecurityEvaluation {
    pub is_outside_workspace: bool,
    pub is_sensitive_credential: bool,
    pub target_paths: Vec<PathBuf>,
    pub warning_message: Option<String>,
}

impl SecurityEvaluation {
    /// Creates a safe evaluation with no warnings.
    pub fn safe() -> Self {
        Self {
            is_outside_workspace: false,
            is_sensitive_credential: false,
            target_paths: Vec::new(),
            warning_message: None,
        }
    }

    /// Returns true if permission elevation or security warning is required.
    pub fn is_escalation_required(&self) -> bool {
        self.is_outside_workspace || self.is_sensitive_credential
    }
}

pub struct PathGuard;

impl PathGuard {
    /// Standard system device paths explicitly authorized for ordinary shell redirection.
    const AUTHORIZED_DEVICE_PATHS: &'static [&'static str] = &[
        "/dev/null",
        "/dev/zero",
        "/dev/urandom",
        "/dev/random",
        "/dev/stdout",
        "/dev/stderr",
        "/dev/stdin",
    ];

    /// Evaluates a shell command string against the workspace root.
    pub fn evaluate_command(command: &str, workspace_root: &Path) -> SecurityEvaluation {
        let candidate_tokens = Self::extract_candidate_paths(command, workspace_root);
        let mut target_paths = Vec::new();
        let mut is_outside_workspace = false;
        let mut is_sensitive_credential = false;
        let mut warnings = Vec::new();

        for token in candidate_tokens {
            let path_eval = Self::evaluate_path(Path::new(&token), workspace_root);
            if path_eval.is_sensitive_credential {
                is_sensitive_credential = true;
                if let Some(ref msg) = path_eval.warning_message {
                    if !warnings.contains(msg) {
                        warnings.push(msg.clone());
                    }
                }
            }
            if path_eval.is_outside_workspace {
                is_outside_workspace = true;
                if let Some(ref msg) = path_eval.warning_message {
                    if !warnings.contains(msg) {
                        warnings.push(msg.clone());
                    }
                }
            }
            target_paths.extend(path_eval.target_paths);
        }

        let warning_message = if warnings.is_empty() {
            None
        } else {
            Some(warnings.join("; "))
        };

        SecurityEvaluation {
            is_outside_workspace,
            is_sensitive_credential,
            target_paths,
            warning_message,
        }
    }

    /// Evaluates a specific path against the workspace root.
    pub fn evaluate_path(candidate: &Path, workspace_root: &Path) -> SecurityEvaluation {
        let candidate_str = candidate.to_string_lossy();
        for auth in Self::AUTHORIZED_DEVICE_PATHS {
            if candidate_str == *auth {
                return SecurityEvaluation::safe();
            }
        }

        let expanded = Self::expand_tilde(candidate);

        // Normalize path and resolve symlinks where possible
        let resolved = Self::resolve_with_canonical_ancestor(&expanded, workspace_root);
        let is_sensitive = Self::is_sensitive_path(&expanded) || Self::is_sensitive_path(&resolved);
        let is_outside = !Self::is_subpath(&resolved, workspace_root);

        let mut warnings = Vec::new();
        if is_sensitive {
            warnings.push(format!(
                "Access to sensitive credential or configuration path '{}' detected",
                candidate.display()
            ));
        }
        if is_outside {
            warnings.push(format!(
                "Path '{}' is outside the workspace root '{}'",
                resolved.display(),
                workspace_root.display()
            ));
        }

        let warning_message = if warnings.is_empty() {
            None
        } else {
            Some(warnings.join("; "))
        };

        SecurityEvaluation {
            is_outside_workspace: is_outside,
            is_sensitive_credential: is_sensitive,
            target_paths: vec![resolved],
            warning_message,
        }
    }

    /// Evaluates tool inputs generically for any tool invocation.
    pub fn evaluate_tool_input(
        tool_name: &str,
        input: &serde_json::Value,
        workspace_root: &Path,
    ) -> SecurityEvaluation {
        if tool_name == "bash" {
            if let Some(command) = input.get("command").and_then(|c| c.as_str()) {
                return Self::evaluate_command(command, workspace_root);
            }
        }
        if let Some(obj) = input.as_object() {
            for key in [
                "file_path",
                "filePath",
                "path",
                "file",
                "target",
                "destination",
                "dest",
                "source",
                "src",
                "directory",
                "dir",
                "filepath",
            ] {
                if let Some(p) = obj.get(key).and_then(|v| v.as_str()) {
                    let eval = Self::evaluate_path(Path::new(p), workspace_root);
                    if eval.is_escalation_required() {
                        return eval;
                    }
                }
            }
        }
        SecurityEvaluation::safe()
    }

    /// Replaces leading `~` with the user's home directory.
    pub fn expand_tilde(path: &Path) -> PathBuf {
        let path_str = path.to_string_lossy();
        if path_str == "~" || path_str.starts_with("~/") {
            if let Some(home) = std::env::var_os("HOME") {
                let home_path = PathBuf::from(home);
                if path_str == "~" {
                    return home_path;
                }
                return home_path.join(path_str.trim_start_matches("~/"));
            }
        }
        path.to_path_buf()
    }

    /// Checks if a path targets sensitive credentials, keys, or security files.
    pub fn is_sensitive_path(path: &Path) -> bool {
        let path_str = path.to_string_lossy();
        let file_name = path
            .file_name()
            .map(|f| f.to_string_lossy().to_string())
            .unwrap_or_default();

        // 1. .env files (.env, .env.local, .env.production, .envrc, etc.)
        if file_name == ".env"
            || file_name.starts_with(".env.")
            || file_name.starts_with(".env-")
            || file_name.starts_with(".env_")
            || file_name == ".envrc"
        {
            return true;
        }

        // 2. SSH credentials and keys
        if path_str.contains("/.ssh")
            || path_str.starts_with(".ssh")
            || path_str.ends_with("/.ssh")
            || (file_name.starts_with("id_")
                && (file_name.contains("rsa")
                    || file_name.contains("ed25519")
                    || file_name.contains("ecdsa")
                    || file_name.contains("dsa")))
            || file_name == "authorized_keys"
            || file_name == "known_hosts"
        {
            return true;
        }

        // 3. Cloud & Container credentials
        if path_str.contains("/.aws")
            || path_str.starts_with(".aws")
            || path_str.contains("/.azure")
            || path_str.starts_with(".azure")
            || path_str.contains("/.config/gcloud")
            || path_str.contains("/.kube")
            || path_str.starts_with(".kube")
            || path_str.contains("/.docker/config.json")
            || file_name == "application_default_credentials.json"
        {
            return true;
        }

        // 4. Git config containing potential tokens/credentials
        if path_str.ends_with(".git/config")
            || file_name == ".gitconfig"
            || file_name == ".git-credentials"
        {
            return true;
        }

        // 5. System password / privilege files (accounting for macOS /private prefix)
        let check_str = path_str.strip_prefix("/private").unwrap_or(&path_str);
        if check_str == "/etc/passwd"
            || check_str == "/etc/shadow"
            || check_str == "/etc/sudoers"
            || check_str == "/etc/master.passwd"
        {
            return true;
        }

        // 6. GPG and secret tokens
        if path_str.contains("/.gnupg")
            || file_name == ".netrc"
            || file_name == ".npmrc"
            || file_name == ".pypirc"
        {
            return true;
        }

        false
    }

    /// Resolves path lexical traversal (`..` and `.`) and canonicalizes existing ancestor directories.
    pub fn resolve_with_canonical_ancestor(path: &Path, workspace_root: &Path) -> PathBuf {
        let absolute = if path.is_absolute() {
            path.to_path_buf()
        } else {
            workspace_root.join(path)
        };

        // Normalize components lexically first
        let normalized = Self::normalize_lexical(&absolute);

        // Find longest existing ancestor on disk
        let mut existing = normalized.clone();
        let mut tail_components = Vec::new();

        while !existing.exists() {
            if let Some(name) = existing.file_name() {
                tail_components.push(name.to_os_string());
                if !existing.pop() {
                    break;
                }
            } else {
                break;
            }
        }

        tail_components.reverse();

        let base = if existing.exists() {
            std::fs::canonicalize(&existing).unwrap_or(existing)
        } else {
            existing
        };

        let mut result = base;
        for part in tail_components {
            result.push(part);
        }
        Self::normalize_lexical(&result)
    }

    /// Determines if `path` is contained within `workspace_root`.
    pub fn is_subpath(path: &Path, workspace_root: &Path) -> bool {
        let canonical_workspace = std::fs::canonicalize(workspace_root)
            .unwrap_or_else(|_| Self::normalize_lexical(workspace_root));
        let canonical_path =
            std::fs::canonicalize(path).unwrap_or_else(|_| Self::normalize_lexical(path));

        canonical_path.starts_with(&canonical_workspace)
    }

    /// Normalizes path lexically without hitting the disk for nonexistent components.
    fn normalize_lexical(path: &Path) -> PathBuf {
        let mut components = Vec::new();
        for component in path.components() {
            match component {
                Component::Prefix(..) | Component::RootDir => {
                    components.push(component);
                }
                Component::CurDir => {}
                Component::ParentDir => {
                    if let Some(last) = components.last() {
                        if matches!(last, Component::Normal(..)) {
                            components.pop();
                            continue;
                        }
                    }
                    components.push(component);
                }
                Component::Normal(..) => {
                    components.push(component);
                }
            }
        }
        components.into_iter().collect()
    }

    /// Extracts candidate path strings from a shell command string.
    pub fn extract_candidate_paths(command: &str, workspace_root: &Path) -> Vec<String> {
        let mut candidates = Vec::new();
        let mut current_token = String::new();
        let mut in_single_quote = false;
        let mut in_double_quote = false;
        let mut chars = command.chars().peekable();

        while let Some(c) = chars.next() {
            match c {
                '\'' if !in_double_quote => {
                    in_single_quote = !in_single_quote;
                }
                '"' if !in_single_quote => {
                    in_double_quote = !in_double_quote;
                }
                ' ' | '\t' | '\n' | ';' | '|' | '&' | '(' | ')' | '{' | '}' | '[' | ']' | ','
                    if !in_single_quote && !in_double_quote =>
                {
                    if !current_token.is_empty() {
                        Self::push_candidate_if_relevant(
                            &mut candidates,
                            &current_token,
                            workspace_root,
                        );
                        current_token.clear();
                    }
                }
                '>' | '<' if !in_single_quote && !in_double_quote => {
                    if !current_token.is_empty() {
                        Self::push_candidate_if_relevant(
                            &mut candidates,
                            &current_token,
                            workspace_root,
                        );
                        current_token.clear();
                    }
                    if chars.peek() == Some(&'>') {
                        chars.next();
                    }
                }
                _ => {
                    current_token.push(c);
                }
            }
        }
        if !current_token.is_empty() {
            Self::push_candidate_if_relevant(&mut candidates, &current_token, workspace_root);
        }

        candidates
    }

    fn push_candidate_if_relevant(
        candidates: &mut Vec<String>,
        raw_token: &str,
        workspace_root: &Path,
    ) {
        let token = raw_token.trim_matches(|c: char| {
            c == '\''
                || c == '"'
                || c == '`'
                || c == '('
                || c == ')'
                || c == '{'
                || c == '}'
                || c == '['
                || c == ']'
                || c == ','
                || c == ';'
                || c == '$'
        });
        if token.is_empty() {
            return;
        }

        // Split options like --file=/path/to/file or KEY=value
        if let Some((_k, val)) = token.split_once('=') {
            let trimmed_val = val.trim_matches(|c: char| {
                c == '\''
                    || c == '"'
                    || c == '`'
                    || c == '('
                    || c == ')'
                    || c == '{'
                    || c == '}'
                    || c == '['
                    || c == ']'
                    || c == ','
                    || c == ';'
            });
            if Self::is_likely_path_or_credential(trimmed_val, workspace_root) {
                candidates.push(trimmed_val.to_string());
                return;
            }
        }

        // Skip command-line flags like -la, --verbose, -f
        if token.starts_with('-') {
            return;
        }

        if Self::is_likely_path_or_credential(token, workspace_root) {
            candidates.push(token.to_string());
        }
    }

    fn is_likely_path_or_credential(token: &str, workspace_root: &Path) -> bool {
        if token.contains('/') || token.contains('\\') {
            return true;
        }
        if token.starts_with('~') || token.contains('.') {
            return true;
        }
        if token == ".env"
            || token.starts_with(".env.")
            || token.starts_with(".env-")
            || token.starts_with(".env_")
            || token == ".envrc"
        {
            return true;
        }
        if token.starts_with("id_") {
            return true;
        }
        if token == "authorized_keys" || token == "known_hosts" {
            return true;
        }
        // Inspect if candidate token exists in workspace root (e.g. symlinks or files without extension)
        let candidate_path = workspace_root.join(token);
        candidate_path.is_symlink() || candidate_path.exists()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cat_etc_passwd_triggers_outside_and_sensitive() {
        let ws = std::env::temp_dir().join("brain_ws_test_passwd");
        let _ = std::fs::create_dir_all(&ws);

        let eval = PathGuard::evaluate_command("cat /etc/passwd", &ws);
        assert!(eval.is_outside_workspace);
        assert!(eval.is_sensitive_credential);
        assert!(eval.warning_message.is_some());
        let _ = std::fs::remove_dir_all(&ws);
    }

    #[test]
    fn test_env_in_workspace_triggers_sensitive() {
        let ws = std::env::temp_dir().join("brain_ws_test_env");
        let _ = std::fs::create_dir_all(&ws);

        let eval = PathGuard::evaluate_command("cat .env", &ws);
        assert!(!eval.is_outside_workspace);
        assert!(eval.is_sensitive_credential);
        assert!(eval.warning_message.is_some());
        let _ = std::fs::remove_dir_all(&ws);
    }

    #[test]
    fn test_traversal_dot_dot_env_triggers_both() {
        let ws = std::env::temp_dir().join("brain_ws_test_traversal");
        let _ = std::fs::create_dir_all(&ws);

        let eval = PathGuard::evaluate_command("cat ../../.env", &ws);
        assert!(eval.is_outside_workspace);
        assert!(eval.is_sensitive_credential);
        let _ = std::fs::remove_dir_all(&ws);
    }

    #[test]
    fn test_ssh_keys_trigger_sensitive() {
        let ws = std::env::temp_dir().join("brain_ws_test_ssh");
        let _ = std::fs::create_dir_all(&ws);

        let eval = PathGuard::evaluate_command("ssh-keygen -f ~/.ssh/id_rsa", &ws);
        assert!(eval.is_sensitive_credential);
        assert!(eval.is_outside_workspace);
        let _ = std::fs::remove_dir_all(&ws);
    }

    #[test]
    fn test_aws_credentials_trigger_sensitive() {
        let ws = std::env::temp_dir().join("brain_ws_test_aws");
        let _ = std::fs::create_dir_all(&ws);

        let eval = PathGuard::evaluate_command("cat ~/.aws/credentials", &ws);
        assert!(eval.is_sensitive_credential);
        let _ = std::fs::remove_dir_all(&ws);
    }

    #[test]
    fn test_safe_in_workspace_path_is_safe() {
        let ws = std::env::temp_dir().join("brain_ws_test_safe");
        let _ = std::fs::create_dir_all(&ws);

        let eval = PathGuard::evaluate_command("cargo test --lib src/main.rs", &ws);
        assert!(!eval.is_outside_workspace);
        assert!(!eval.is_sensitive_credential);
        assert!(eval.warning_message.is_none());
        let _ = std::fs::remove_dir_all(&ws);
    }

    #[test]
    fn test_dev_null_is_authorized_and_safe() {
        let ws = std::env::temp_dir().join("brain_ws_test_dev_null");
        let _ = std::fs::create_dir_all(&ws);

        let eval = PathGuard::evaluate_command("echo hello > /dev/null 2>&1", &ws);
        assert!(!eval.is_outside_workspace);
        assert!(!eval.is_sensitive_credential);
        assert!(eval.warning_message.is_none());
        let _ = std::fs::remove_dir_all(&ws);
    }

    #[test]
    fn test_nonexistent_file_inside_workspace_is_safe() {
        let ws = std::env::temp_dir().join("brain_ws_test_new_file");
        let _ = std::fs::create_dir_all(&ws);

        let eval = PathGuard::evaluate_command("echo test > new_folder/output.txt", &ws);
        assert!(!eval.is_outside_workspace);
        assert!(!eval.is_sensitive_credential);
        let _ = std::fs::remove_dir_all(&ws);
    }

    #[test]
    fn test_nonexistent_file_outside_workspace_is_detected() {
        let ws = std::env::temp_dir().join("brain_ws_test_outside_new");
        let _ = std::fs::create_dir_all(&ws);

        let eval = PathGuard::evaluate_command("touch ../../escaped_new_file.txt", &ws);
        assert!(eval.is_outside_workspace);
        let _ = std::fs::remove_dir_all(&ws);
    }

    #[cfg(unix)]
    #[test]
    fn test_symlink_pointing_outside_workspace_is_detected() {
        let ws = std::env::temp_dir().join("brain_ws_test_symlink");
        let _ = std::fs::create_dir_all(&ws);
        let link_path = ws.join("link_to_root");
        let _ = std::os::unix::fs::symlink("/", &link_path);

        let eval = PathGuard::evaluate_path(&link_path, &ws);
        assert!(eval.is_outside_workspace);

        let _ = std::fs::remove_file(&link_path);
        let _ = std::fs::remove_dir_all(&ws);
    }

    #[cfg(unix)]
    #[test]
    fn test_symlink_to_sensitive_env_file_is_detected() {
        let ws = std::env::temp_dir().join("brain_ws_test_symlink_env");
        let _ = std::fs::create_dir_all(&ws);
        let target_env = ws.join(".env");
        let _ = std::fs::write(&target_env, "SECRET=123");
        let link_path = ws.join("safe_name.txt");
        let _ = std::os::unix::fs::symlink(&target_env, &link_path);

        let eval = PathGuard::evaluate_path(&link_path, &ws);
        assert!(
            eval.is_sensitive_credential,
            "symlink target .env must be recognized as sensitive"
        );
        assert!(!eval.is_outside_workspace, "inside workspace");

        let eval_cmd = PathGuard::evaluate_command("cat safe_name.txt", &ws);
        assert!(eval_cmd.is_sensitive_credential);

        let _ = std::fs::remove_file(&link_path);
        let _ = std::fs::remove_file(&target_env);
        let _ = std::fs::remove_dir_all(&ws);
    }

    #[test]
    fn test_macos_private_etc_passwd_is_detected() {
        let ws = std::env::temp_dir().join("brain_ws_test_private_passwd");
        let _ = std::fs::create_dir_all(&ws);

        let eval = PathGuard::evaluate_command("cat /private/etc/passwd", &ws);
        assert!(
            eval.is_sensitive_credential,
            "/private/etc/passwd must be recognized as sensitive"
        );
        assert!(eval.is_outside_workspace);
        let _ = std::fs::remove_dir_all(&ws);
    }

    #[test]
    fn test_file_read_tool_input_with_file_path_key_is_evaluated() {
        let ws = std::env::temp_dir().join("brain_ws_test_tool_input");
        let _ = std::fs::create_dir_all(&ws);

        let input = serde_json::json!({
            "file_path": "/etc/passwd"
        });
        let eval = PathGuard::evaluate_tool_input("FileReadTool", &input, &ws);
        assert!(eval.is_escalation_required());
        assert!(eval.is_sensitive_credential);
        assert!(eval.is_outside_workspace);

        let _ = std::fs::remove_dir_all(&ws);
    }

    #[test]
    fn test_command_substitution_and_direnv_patterns() {
        let ws = std::env::temp_dir().join("brain_ws_test_subshell");
        let _ = std::fs::create_dir_all(&ws);

        let eval = PathGuard::evaluate_command("cat <(cat /etc/passwd)", &ws);
        assert!(eval.is_sensitive_credential);
        assert!(eval.is_outside_workspace);

        let eval_direnv = PathGuard::evaluate_command("cat .envrc", &ws);
        assert!(eval_direnv.is_sensitive_credential);

        let eval_kube = PathGuard::evaluate_command("cat ~/.kube/config", &ws);
        assert!(eval_kube.is_sensitive_credential);
        assert!(eval_kube.is_outside_workspace);

        let _ = std::fs::remove_dir_all(&ws);
    }
}
