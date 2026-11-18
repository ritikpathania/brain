//! Git Worktree Isolation Service
//!
//! Provides isolated Git worktree management for subagent task execution.
//! Enforces repository validation, subagent ownership mapping, safe cleanup
//! with dirty state protection, and stale worktree recovery.

use brain_domain::SubagentId;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, RwLock};

/// Errors encountered during Git worktree lifecycle operations.
#[derive(Debug, thiserror::Error)]
pub enum WorktreeError {
    /// Directory is not a valid Git repository root.
    #[error("Not a valid Git repository: {0}")]
    NotAGitRepository(String),

    /// Worktree name contains invalid characters, directory traversal, or is empty.
    #[error("Invalid worktree name: {0}")]
    InvalidName(String),

    /// Worktree with the requested name already exists.
    #[error("Worktree '{0}' already exists")]
    WorktreeAlreadyExists(String),

    /// Worktree branch already exists in repository.
    #[error("Worktree branch '{0}' already exists in repository")]
    BranchAlreadyExists(String),

    /// Worktree was not found by name or path.
    #[error("Worktree '{0}' not found")]
    WorktreeNotFound(String),

    /// Safety invariant violation: worktree has uncommitted modifications or untracked files.
    #[error("Worktree at {path:?} has uncommitted or untracked changes; safe deletion refused:\n{porcelain_status}")]
    WorktreeDirty {
        /// Worktree filesystem path.
        path: PathBuf,
        /// Porcelained status output detailing uncommitted files.
        porcelain_status: String,
    },

    /// Safety invariant violation: branch has unmerged commits that would be destroyed by deletion.
    #[error("Worktree branch '{branch}' contains unmerged local commits; safe deletion refused:\n{unmerged_commits}")]
    UnmergedCommitsPreserved {
        /// Dedicated worktree branch.
        branch: String,
        /// Commit summaries from git log.
        unmerged_commits: String,
    },

    /// Safety invariant violation: worktree is actively occupied by a running subagent.
    #[error("Worktree at {path:?} is currently occupied by active subagent {subagent_id}; deletion refused")]
    WorktreeActive {
        /// Worktree filesystem path.
        path: PathBuf,
        /// Active subagent identifier.
        subagent_id: SubagentId,
    },

    /// Safety invariant violation: commit reachability or integration reference could not be established.
    #[error("Cannot establish integration reachability for branch '{branch}' against base commit '{base_commit}': {reason}")]
    ReachabilityUnknown {
        /// Dedicated worktree branch.
        branch: String,
        /// Worktree creation base commit.
        base_commit: String,
        /// Failure reason or diagnostic.
        reason: String,
    },

    /// Safety invariant violation: worktree is actively locked.
    #[error("Worktree at {path:?} is locked ({reason:?}); safe deletion refused")]
    WorktreeLocked {
        /// Worktree filesystem path.
        path: PathBuf,
        /// Lock reason if available.
        reason: Option<String>,
    },

    /// Git CLI execution returned a non-zero exit code.
    #[error("Git command '{command}' failed with exit code {exit_code:?}: {stderr}")]
    GitCommandFailed {
        /// Git command executed.
        command: String,
        /// Non-zero exit status if available.
        exit_code: Option<i32>,
        /// Stderr diagnostic output.
        stderr: String,
    },

    /// Filesystem I/O error.
    #[error("I/O error during worktree operation: {0}")]
    IoError(String),
}

/// Metadata descriptor for an active Git worktree.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorktreeDescriptor {
    /// Symbolic name of the worktree.
    pub name: String,
    /// Absolute filesystem path to the worktree directory.
    pub path: PathBuf,
    /// Dedicated Git branch name (e.g. `worktree-<name>`).
    pub branch: String,
    /// Root path of the parent Git repository.
    pub repo_root: PathBuf,
    /// Subagent owner ID if assigned.
    pub subagent_id: Option<SubagentId>,
    /// Creation timestamp in milliseconds since epoch.
    pub created_at_ms: u64,
    /// Base commit SHA at which this worktree was spawned.
    pub base_commit: String,
    /// Whether the parent repository had uncommitted changes at creation time.
    pub parent_had_uncommitted_drift: bool,
    /// Whether an uncommitted source snapshot was applied to this worktree.
    pub snapshot_applied: bool,
    /// Target integration branch reference (e.g. `main`, `master`, or parent branch) at creation time.
    #[serde(default)]
    pub target_branch: Option<String>,
}

/// Callback to check if a subagent is actively executing.
pub type ActiveAgentChecker = Arc<dyn Fn(&SubagentId) -> bool + Send + Sync>;

/// Service managing the lifecycle, registration, and safe cleanup of isolated Git worktrees.
pub struct WorktreeService {
    repo_root: PathBuf,
    worktrees_dir: PathBuf,
    active_worktrees: Arc<RwLock<HashMap<String, WorktreeDescriptor>>>,
    active_agent_checker: Arc<RwLock<Option<ActiveAgentChecker>>>,
}

impl WorktreeService {
    /// Discovers the Git repository root for `start_path` via `git rev-parse --show-toplevel`.
    pub fn discover_repo_root(start_path: &Path) -> Result<PathBuf, WorktreeError> {
        let output = Command::new("git")
            .arg("-C")
            .arg(start_path)
            .arg("rev-parse")
            .arg("--show-toplevel")
            .output()
            .map_err(|e| WorktreeError::IoError(format!("Failed to execute git: {e}")))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            return Err(WorktreeError::NotAGitRepository(stderr));
        }

        let root_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
        Ok(PathBuf::from(root_str))
    }

    /// Creates a new `WorktreeService` initialized for a verified repository root.
    /// Worktrees are placed by default under `.claude/worktrees` (or `target/worktrees`).
    pub fn new(repo_root: PathBuf) -> Result<Self, WorktreeError> {
        let canonical_root = match Self::discover_repo_root(&repo_root) {
            Ok(root) => root,
            Err(_) => {
                if repo_root.join(".git").exists() {
                    repo_root.clone()
                } else {
                    return Err(WorktreeError::NotAGitRepository(format!(
                        "Path {} is not inside a git repository",
                        repo_root.display()
                    )));
                }
            }
        };

        let worktrees_dir = canonical_root.join(".claude").join("worktrees");
        Ok(Self {
            repo_root: canonical_root,
            worktrees_dir,
            active_worktrees: Arc::new(RwLock::new(HashMap::new())),
            active_agent_checker: Arc::new(RwLock::new(None)),
        })
    }

    /// Creates a new `WorktreeService` with a custom worktrees storage directory.
    pub fn with_worktrees_dir(repo_root: PathBuf, worktrees_dir: PathBuf) -> Self {
        Self {
            repo_root,
            worktrees_dir,
            active_worktrees: Arc::new(RwLock::new(HashMap::new())),
            active_agent_checker: Arc::new(RwLock::new(None)),
        }
    }

    /// Sets a callback function used to check whether a subagent is actively executing.
    pub fn set_active_agent_checker<F>(&self, checker: F)
    where
        F: Fn(&SubagentId) -> bool + Send + Sync + 'static,
    {
        let mut guard = self.active_agent_checker.write().unwrap();
        *guard = Some(Arc::new(checker));
    }

    /// Checks if a subagent is actively executing.
    pub fn is_agent_active(&self, subagent_id: &SubagentId) -> bool {
        let guard = self.active_agent_checker.read().unwrap();
        if let Some(ref checker) = *guard {
            checker(subagent_id)
        } else {
            false
        }
    }

    /// Returns the root path of the managed repository.
    pub fn repo_root(&self) -> &Path {
        &self.repo_root
    }

    /// Returns the base directory where worktrees are created.
    pub fn worktrees_dir(&self) -> &Path {
        &self.worktrees_dir
    }

    /// Validates that a worktree name is safe and does not contain path separators or directory traversal.
    pub fn validate_worktree_name(name: &str) -> Result<String, WorktreeError> {
        let clean = name.trim();
        if clean.is_empty() {
            return Err(WorktreeError::InvalidName(
                "Worktree name cannot be empty".to_string(),
            ));
        }
        if clean == "."
            || clean == ".."
            || clean.contains('/')
            || clean.contains('\\')
            || clean.contains('\0')
        {
            return Err(WorktreeError::InvalidName(format!(
                "Invalid worktree name '{clean}': cannot contain path separators, null bytes, or directory traversal"
            )));
        }
        if !clean
            .chars()
            .all(|c| c.is_alphanumeric() || c == '-' || c == '_' || c == '.')
        {
            return Err(WorktreeError::InvalidName(format!(
                "Invalid worktree name '{clean}': only alphanumeric, hyphen, underscore, and dot are permitted"
            )));
        }
        Ok(clean.to_string())
    }

    /// Checks if a worktree path contains uncommitted changes, untracked files, or dirty submodules.
    /// Returns `Ok(Some(status))` if dirty, or `Ok(None)` if clean.
    pub fn check_dirty(&self, path: &Path) -> Result<Option<String>, WorktreeError> {
        let output = Command::new("git")
            .arg("-C")
            .arg(path)
            .arg("status")
            .arg("--porcelain=v1")
            .arg("-uall")
            .arg("--ignore-submodules=none")
            .output()
            .map_err(|e| WorktreeError::IoError(format!("Failed to run git status: {e}")))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            return Err(WorktreeError::GitCommandFailed {
                command: format!("git -C {} status --porcelain=v1 -uall", path.display()),
                exit_code: output.status.code(),
                stderr,
            });
        }

        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if stdout.is_empty() {
            Ok(None)
        } else {
            Ok(Some(stdout))
        }
    }

    /// Checks whether a worktree branch contains unmerged local commits.
    ///
    /// Evaluates:
    /// 1. Branch tip SHA vs worktree creation `base_commit`.
    /// 2. Intended integration reference (`target_branch`, `main`, `master`, or non-identical HEAD).
    /// 3. Rejects self-referential false positives (when candidate integration ref is identical to branch tip).
    /// 4. Verifies commit ancestry using `git merge-base --is-ancestor`.
    /// 5. Fails closed with `ReachabilityUnknown` if reachability cannot be established.
    pub fn check_unmerged_commits(
        &self,
        branch: &str,
        base_commit: &str,
        target_branch: Option<&str>,
    ) -> Result<Option<String>, WorktreeError> {
        // 1. Resolve worktree branch tip commit SHA
        let tip_output = Command::new("git")
            .arg("-C")
            .arg(&self.repo_root)
            .args(["rev-parse", "--verify", &format!("refs/heads/{branch}")])
            .output()
            .map_err(|e| WorktreeError::IoError(format!("Failed to resolve branch tip: {e}")))?;

        if !tip_output.status.success() {
            return Err(WorktreeError::ReachabilityUnknown {
                branch: branch.to_string(),
                base_commit: base_commit.to_string(),
                reason: format!("Branch refs/heads/{branch} does not exist or cannot be resolved"),
            });
        }

        let branch_tip = String::from_utf8_lossy(&tip_output.stdout)
            .trim()
            .to_string();

        // 2. Fast-path: If the branch tip is identical to creation base_commit,
        // then no new commits were ever created on this worktree branch.
        if !base_commit.is_empty() && branch_tip == base_commit {
            return Ok(None);
        }

        // 3. Resolve candidate integration references.
        let mut candidates = Vec::new();
        if let Some(tb) = target_branch {
            candidates.push(tb.to_string());
        }
        if !candidates.contains(&"main".to_string()) {
            candidates.push("main".to_string());
        }
        if !candidates.contains(&"master".to_string()) {
            candidates.push("master".to_string());
        }

        // Also resolve parent repo HEAD SHA
        let head_rev_out = Command::new("git")
            .arg("-C")
            .arg(&self.repo_root)
            .args(["rev-parse", "HEAD"])
            .output();
        let head_sha = if let Ok(h) = head_rev_out {
            if h.status.success() {
                Some(String::from_utf8_lossy(&h.stdout).trim().to_string())
            } else {
                None
            }
        } else {
            None
        };

        let mut resolved_any_target = false;

        for candidate in &candidates {
            // Never check against the worktree branch itself
            if candidate == branch {
                continue;
            }

            let cand_verify = Command::new("git")
                .arg("-C")
                .arg(&self.repo_root)
                .args(["rev-parse", "--verify", &format!("refs/heads/{candidate}")])
                .output();

            if let Ok(cvo) = cand_verify {
                if cvo.status.success() {
                    let cand_sha = String::from_utf8_lossy(&cvo.stdout).trim().to_string();
                    resolved_any_target = true;

                    // If candidate points to the same SHA as branch_tip (and branch_tip != base_commit),
                    // verify if branch_tip is an ancestor of that candidate
                    let ancestor_check = Command::new("git")
                        .arg("-C")
                        .arg(&self.repo_root)
                        .args(["merge-base", "--is-ancestor", &branch_tip, &cand_sha])
                        .output();

                    if let Ok(aco) = ancestor_check {
                        if aco.status.success() {
                            return Ok(None);
                        }
                    }
                }
            }
        }

        // Also check parent repo HEAD if HEAD is not on the worktree branch
        if let Some(ref h_sha) = head_sha {
            if h_sha != &branch_tip {
                let head_ancestor = Command::new("git")
                    .arg("-C")
                    .arg(&self.repo_root)
                    .args(["merge-base", "--is-ancestor", &branch_tip, h_sha])
                    .output();

                if let Ok(hao) = head_ancestor {
                    if hao.status.success() {
                        return Ok(None);
                    }
                }
            }
        }

        if !resolved_any_target && head_sha.is_none() {
            return Err(WorktreeError::ReachabilityUnknown {
                branch: branch.to_string(),
                base_commit: base_commit.to_string(),
                reason:
                    "No valid integration reference (target branch, main, master, or HEAD) found"
                        .to_string(),
            });
        }

        // Unmerged commits exist! List them against base_commit or the first valid integration target
        let log_ref = if !base_commit.is_empty() {
            base_commit
        } else {
            target_branch.unwrap_or("HEAD")
        };

        let log_out = Command::new("git")
            .arg("-C")
            .arg(&self.repo_root)
            .args(["log", &format!("{log_ref}..{branch_tip}"), "--oneline"])
            .output()
            .map_err(|e| {
                WorktreeError::IoError(format!("Failed to retrieve unmerged commit log: {e}"))
            })?;

        if log_out.status.success() {
            let unmerged = String::from_utf8_lossy(&log_out.stdout).trim().to_string();
            if unmerged.is_empty() {
                Ok(Some(format!("Commit {branch_tip} differs from base {base_commit} and is not integrated into any known branch")))
            } else {
                Ok(Some(unmerged))
            }
        } else {
            let stderr = String::from_utf8_lossy(&log_out.stderr).trim().to_string();
            Err(WorktreeError::ReachabilityUnknown {
                branch: branch.to_string(),
                base_commit: base_commit.to_string(),
                reason: format!("git log failed: {stderr}"),
            })
        }
    }

    /// Checks if a worktree is locked using `git worktree list --porcelain`.
    pub fn check_locked(&self, path: &Path) -> Result<Option<String>, WorktreeError> {
        let output = Command::new("git")
            .arg("-C")
            .arg(&self.repo_root)
            .arg("worktree")
            .arg("list")
            .arg("--porcelain")
            .output()
            .map_err(|e| WorktreeError::IoError(format!("Failed to run git worktree list: {e}")))?;

        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let canonical_path = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
            let path_str = canonical_path.to_string_lossy();

            let mut current_matches = false;
            for line in stdout.lines() {
                if let Some(wt_path) = line.strip_prefix("worktree ") {
                    let wt_canon = PathBuf::from(wt_path)
                        .canonicalize()
                        .unwrap_or_else(|_| PathBuf::from(wt_path));
                    current_matches = wt_canon == canonical_path || wt_path == path_str;
                } else if current_matches && line.starts_with("locked") {
                    let reason = line.strip_prefix("locked").unwrap_or("").trim().to_string();
                    let reason_opt = if reason.is_empty() {
                        None
                    } else {
                        Some(reason)
                    };
                    return Ok(Some(
                        reason_opt.unwrap_or_else(|| "Worktree is locked".to_string()),
                    ));
                }
            }
        }
        Ok(None)
    }

    /// Applies an uncommitted source snapshot from the parent repository to the target worktree.
    ///
    /// CRITICAL SAFETY GUARANTEES:
    /// 1. Does NOT overwrite existing child changes (tracked or untracked in child worktree).
    /// 2. Strictly excludes credential files, SSH keys, private keys, certificates, and tokens.
    /// 3. Inspects symlinks and rejects targets outside repository or pointing to credentials.
    /// 4. Follows deletions consistently only if child has not modified the target file.
    pub fn apply_uncommitted_snapshot(&self, worktree_path: &Path) -> Result<usize, WorktreeError> {
        let status = match self.check_dirty(&self.repo_root)? {
            Some(s) => s,
            None => return Ok(0),
        };

        let is_sensitive_name = |name: &str| -> bool {
            let lower = name.to_lowercase();
            lower.contains(".env")
                || lower.contains("id_rsa")
                || lower.contains("id_ecdsa")
                || lower.contains("id_ed25519")
                || lower.contains("id_dsa")
                || lower.contains("credentials")
                || lower.contains("secret")
                || lower.contains("token")
                || lower.contains(".aws/")
                || lower.contains(".ssh/")
                || lower.contains(".gnupg/")
                || lower.contains(".git/")
                || lower.starts_with(".git")
                || lower.contains(".gemini/")
                || lower.ends_with(".pem")
                || lower.ends_with(".key")
                || lower.ends_with(".p12")
                || lower.ends_with(".pfx")
                || lower.ends_with(".pkcs12")
                || lower.ends_with(".kdbx")
        };

        let mut copied_count = 0;
        for line in status.lines() {
            let trimmed = line.trim();
            if trimmed.len() < 3 {
                continue;
            }
            let path_part = &trimmed[2..].trim();
            let file_rel = if let Some((_, dest)) = path_part.split_once("->") {
                dest.trim()
            } else {
                path_part
            }
            .trim_matches('"');

            let rel_path = Path::new(file_rel);
            let path_str = rel_path.to_string_lossy().to_string();

            // SEC-03 Credential & VCS Blocklist (Name Check)
            if is_sensitive_name(&path_str) {
                continue;
            }

            let src_file = self.repo_root.join(rel_path);
            let dst_file = worktree_path.join(rel_path);

            // Symlink Safety Check
            if src_file.is_symlink() {
                if let Ok(canon) = std::fs::canonicalize(&src_file) {
                    // Reject if symlink points outside repo root
                    if !canon.starts_with(&self.repo_root) {
                        continue;
                    }
                    // Reject if canonical target matches credential blocklist
                    if is_sensitive_name(&canon.to_string_lossy()) {
                        continue;
                    }
                } else {
                    // Broken or unresolvable symlink; skip
                    continue;
                }
            }

            // Check if child has existing local modifications to this file
            if dst_file.exists() {
                let child_status = Command::new("git")
                    .arg("-C")
                    .arg(worktree_path)
                    .args(["status", "--porcelain=v1", "--", file_rel])
                    .output();

                if let Ok(cso) = child_status {
                    let c_out = String::from_utf8_lossy(&cso.stdout).trim().to_string();
                    if !c_out.is_empty() {
                        // Child has existing uncommitted modifications to this file; do NOT overwrite!
                        continue;
                    }
                }
            }

            if src_file.is_file() {
                if let Some(parent) = dst_file.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                if std::fs::copy(&src_file, &dst_file).is_ok() {
                    copied_count += 1;
                }
            } else if !src_file.exists() && dst_file.exists() {
                // File deleted in parent; only delete if child hasn't modified it
                let _ = std::fs::remove_file(&dst_file);
            }
        }

        Ok(copied_count)
    }

    /// Creates and registers an isolated Git worktree for a subagent or task,
    /// optionally applying an uncommitted snapshot from the parent workspace.
    pub fn create_worktree_with_snapshot(
        &self,
        name: &str,
        subagent_id: Option<SubagentId>,
        snapshot_uncommitted: bool,
    ) -> Result<WorktreeDescriptor, WorktreeError> {
        let clean_name = Self::validate_worktree_name(name)?;

        {
            let registry = self.active_worktrees.read().unwrap();
            if registry.contains_key(&clean_name) {
                return Err(WorktreeError::WorktreeAlreadyExists(clean_name.to_string()));
            }
        }

        let target_path = self.worktrees_dir.join(&clean_name);
        if target_path.exists() {
            return Err(WorktreeError::WorktreeAlreadyExists(format!(
                "Directory already exists at {}",
                target_path.display()
            )));
        }

        let branch = format!("worktree-{clean_name}");
        let branch_check = Command::new("git")
            .arg("-C")
            .arg(&self.repo_root)
            .arg("rev-parse")
            .arg("--verify")
            .arg(format!("refs/heads/{branch}"))
            .output();

        if let Ok(bc) = branch_check {
            if bc.status.success() {
                return Err(WorktreeError::BranchAlreadyExists(branch));
            }
        }

        // Get base HEAD commit SHA
        let head_output = Command::new("git")
            .arg("-C")
            .arg(&self.repo_root)
            .arg("rev-parse")
            .arg("HEAD")
            .output()
            .map_err(|e| WorktreeError::IoError(format!("Failed to resolve HEAD: {e}")))?;

        let base_commit = if head_output.status.success() {
            String::from_utf8_lossy(&head_output.stdout)
                .trim()
                .to_string()
        } else {
            "HEAD".to_string()
        };

        // Resolve target integration branch name (e.g. main/master)
        let target_branch_output = Command::new("git")
            .arg("-C")
            .arg(&self.repo_root)
            .args(["rev-parse", "--abbrev-ref", "HEAD"])
            .output();

        let target_branch = if let Ok(tbo) = target_branch_output {
            if tbo.status.success() {
                let tb = String::from_utf8_lossy(&tbo.stdout).trim().to_string();
                if tb != "HEAD" && !tb.is_empty() {
                    Some(tb)
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        };

        let parent_had_drift = self.check_dirty(&self.repo_root)?.is_some();

        // Ensure parent worktrees directory exists
        std::fs::create_dir_all(&self.worktrees_dir)
            .map_err(|e| WorktreeError::IoError(format!("Failed to create worktrees dir: {e}")))?;

        // Execute `git worktree add -b <branch> <path> HEAD`
        let output = Command::new("git")
            .arg("-C")
            .arg(&self.repo_root)
            .arg("worktree")
            .arg("add")
            .arg("-b")
            .arg(&branch)
            .arg(&target_path)
            .arg("HEAD")
            .output()
            .map_err(|e| {
                WorktreeError::IoError(format!("Failed to spawn git worktree add: {e}"))
            })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            return Err(WorktreeError::GitCommandFailed {
                command: format!(
                    "git worktree add -b {branch} {} HEAD",
                    target_path.display()
                ),
                exit_code: output.status.code(),
                stderr,
            });
        }

        // If snapshotting uncommitted changes was requested, copy safe tracked/untracked modifications
        let snapshot_applied = if snapshot_uncommitted && parent_had_drift {
            self.apply_uncommitted_snapshot(&target_path)? > 0
        } else {
            false
        };

        let descriptor = WorktreeDescriptor {
            name: clean_name.to_string(),
            path: target_path,
            branch,
            repo_root: self.repo_root.clone(),
            subagent_id,
            created_at_ms: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64,
            base_commit,
            parent_had_uncommitted_drift: parent_had_drift,
            snapshot_applied,
            target_branch,
        };

        let mut registry = self.active_worktrees.write().unwrap();
        registry.insert(clean_name.to_string(), descriptor.clone());

        Ok(descriptor)
    }

    /// Creates and registers an isolated Git worktree for a subagent or task without snapshotting.
    pub fn create_worktree(
        &self,
        name: &str,
        subagent_id: Option<SubagentId>,
    ) -> Result<WorktreeDescriptor, WorktreeError> {
        self.create_worktree_with_snapshot(name, subagent_id, false)
    }

    /// Safely cleans up and removes a worktree.
    ///
    /// CRITICAL INVARIANTS:
    /// 0. If the worktree is occupied by an active subagent, safe deletion is refused.
    /// 1. If the worktree is locked, safe deletion is refused.
    /// 2. If the worktree has uncommitted modifications or untracked files, safe deletion is refused.
    /// 3. If the worktree branch contains unmerged local commits, safe deletion is refused.
    pub fn remove_worktree(
        &self,
        name: &str,
        force_even_if_dirty: bool,
    ) -> Result<(), WorktreeError> {
        let clean_name = Self::validate_worktree_name(name)?;
        let descriptor = {
            let registry = self.active_worktrees.read().unwrap();
            registry.get(&clean_name).cloned()
        };

        let descriptor = match descriptor {
            Some(d) => d,
            None => {
                let candidate_path = self.worktrees_dir.join(&clean_name);
                if candidate_path.exists() {
                    WorktreeDescriptor {
                        name: clean_name.to_string(),
                        path: candidate_path,
                        branch: format!("worktree-{clean_name}"),
                        repo_root: self.repo_root.clone(),
                        subagent_id: None,
                        created_at_ms: 0,
                        base_commit: "".to_string(),
                        parent_had_uncommitted_drift: false,
                        snapshot_applied: false,
                        target_branch: None,
                    }
                } else {
                    return Err(WorktreeError::WorktreeNotFound(clean_name.to_string()));
                }
            }
        };

        // SAFETY GATE 0: Active subagent check
        if !force_even_if_dirty {
            if let Some(subagent_id) = descriptor.subagent_id {
                if self.is_agent_active(&subagent_id) {
                    return Err(WorktreeError::WorktreeActive {
                        path: descriptor.path,
                        subagent_id,
                    });
                }
            }
        }

        // SAFETY GATE 1: Locked worktree check
        if !force_even_if_dirty {
            if let Some(lock_reason) = self.check_locked(&descriptor.path)? {
                return Err(WorktreeError::WorktreeLocked {
                    path: descriptor.path,
                    reason: Some(lock_reason),
                });
            }
        }

        // SAFETY GATE 2: Dirty worktree check (tracked + untracked files)
        if !force_even_if_dirty {
            if let Some(dirty_status) = self.check_dirty(&descriptor.path)? {
                return Err(WorktreeError::WorktreeDirty {
                    path: descriptor.path,
                    porcelain_status: dirty_status,
                });
            }
        }

        // SAFETY GATE 3: Unmerged local commits check
        if !force_even_if_dirty {
            if let Some(unmerged) = self.check_unmerged_commits(
                &descriptor.branch,
                &descriptor.base_commit,
                descriptor.target_branch.as_deref(),
            )? {
                return Err(WorktreeError::UnmergedCommitsPreserved {
                    branch: descriptor.branch,
                    unmerged_commits: unmerged,
                });
            }
        }

        // 1. Run `git worktree remove <path>`
        let mut cmd = Command::new("git");
        cmd.arg("-C")
            .arg(&self.repo_root)
            .arg("worktree")
            .arg("remove");
        if force_even_if_dirty {
            cmd.arg("--force");
        }
        cmd.arg(&descriptor.path);

        let remove_output = cmd.output().map_err(|e| {
            WorktreeError::IoError(format!("Failed to execute git worktree remove: {e}"))
        })?;

        if !remove_output.status.success() {
            let stderr = String::from_utf8_lossy(&remove_output.stderr)
                .trim()
                .to_string();
            return Err(WorktreeError::GitCommandFailed {
                command: format!("git worktree remove {}", descriptor.path.display()),
                exit_code: remove_output.status.code(),
                stderr,
            });
        }

        // 2. Delete the created branch `git branch -D worktree-<name>`
        let _ = Command::new("git")
            .arg("-C")
            .arg(&self.repo_root)
            .arg("branch")
            .arg("-D")
            .arg(&descriptor.branch)
            .output();

        let mut registry = self.active_worktrees.write().unwrap();
        registry.remove(&clean_name);

        Ok(())
    }

    /// Prunes stale worktree administrative references via `git worktree prune`.
    pub fn prune_stale(&self) -> Result<String, WorktreeError> {
        let output = Command::new("git")
            .arg("-C")
            .arg(&self.repo_root)
            .arg("worktree")
            .arg("prune")
            .arg("-v")
            .output()
            .map_err(|e| {
                WorktreeError::IoError(format!("Failed to run git worktree prune: {e}"))
            })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            return Err(WorktreeError::GitCommandFailed {
                command: "git worktree prune".to_string(),
                exit_code: output.status.code(),
                stderr,
            });
        }

        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }

    /// Retrieves an active worktree descriptor by name.
    pub fn get_worktree(&self, name: &str) -> Option<WorktreeDescriptor> {
        let registry = self.active_worktrees.read().unwrap();
        registry.get(name.trim()).cloned()
    }

    /// Finds the active worktree assigned to a specific subagent.
    pub fn get_worktree_for_subagent(
        &self,
        subagent_id: &SubagentId,
    ) -> Option<WorktreeDescriptor> {
        let registry = self.active_worktrees.read().unwrap();
        registry
            .values()
            .find(|w| w.subagent_id.as_ref() == Some(subagent_id))
            .cloned()
    }

    /// Lists all active registered worktrees.
    pub fn list_active(&self) -> Vec<WorktreeDescriptor> {
        let registry = self.active_worktrees.read().unwrap();
        registry.values().cloned().collect()
    }
}
