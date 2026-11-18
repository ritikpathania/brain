use brain_domain::SubagentId;
use brain_services::worktree::{WorktreeError, WorktreeService};
use std::fs::{self, File};
use std::io::Write;
use std::path::PathBuf;
use std::process::Command;

/// Helper to initialize an isolated git repository fixture in target/tmp/
fn create_test_repo_fixture(fixture_name: &str) -> (PathBuf, PathBuf) {
    let base = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("target")
        .join("tmp")
        .join(format!(
            "worktree_fixture_{}_{}",
            fixture_name,
            std::process::id()
        ));

    let _ = fs::remove_dir_all(&base);
    fs::create_dir_all(&base).expect("create fixture dir");

    let repo_dir = base.join("repo");
    fs::create_dir_all(&repo_dir).expect("create repo dir");

    // git init
    let init_out = Command::new("git")
        .arg("init")
        .arg(&repo_dir)
        .output()
        .expect("git init");
    assert!(init_out.status.success(), "git init failed");

    // Configure test user
    Command::new("git")
        .arg("-C")
        .arg(&repo_dir)
        .args(["config", "user.name", "Test User"])
        .output()
        .expect("git config user.name");
    Command::new("git")
        .arg("-C")
        .arg(&repo_dir)
        .args(["config", "user.email", "test@example.com"])
        .output()
        .expect("git config user.email");

    // Create initial commit
    let readme = repo_dir.join("README.md");
    let mut f = File::create(&readme).expect("create readme");
    writeln!(f, "# Fixture Repo").expect("write readme");

    Command::new("git")
        .arg("-C")
        .arg(&repo_dir)
        .args(["add", "README.md"])
        .output()
        .expect("git add");

    Command::new("git")
        .arg("-C")
        .arg(&repo_dir)
        .args(["commit", "-m", "Initial commit"])
        .output()
        .expect("git commit");

    let worktrees_dir = base.join("worktrees");
    fs::create_dir_all(&worktrees_dir).expect("create worktrees dir");

    (repo_dir, worktrees_dir)
}

#[test]
fn test_worktree_lifecycle_and_ownership() {
    let (repo_dir, worktrees_dir) = create_test_repo_fixture("lifecycle");
    let service = WorktreeService::with_worktrees_dir(repo_dir.clone(), worktrees_dir.clone());

    let subagent_id = SubagentId::new();
    let desc = service
        .create_worktree("feature-a", Some(subagent_id))
        .expect("create worktree");

    assert_eq!(desc.name, "feature-a");
    assert_eq!(desc.subagent_id, Some(subagent_id));
    assert_eq!(desc.branch, "worktree-feature-a");
    assert!(desc.path.exists());
    assert!(desc.path.join("README.md").exists());

    // Check ownership lookup
    let found = service.get_worktree_for_subagent(&subagent_id);
    assert!(found.is_some());
    assert_eq!(found.unwrap().name, "feature-a");

    // Clean worktree removal succeeds
    service
        .remove_worktree("feature-a", false)
        .expect("clean removal succeeds");
    assert!(!desc.path.exists());

    // Worktree is no longer in active list
    assert!(service.get_worktree("feature-a").is_none());
}

#[test]
fn test_dirty_worktree_preservation_invariant() {
    let (repo_dir, worktrees_dir) = create_test_repo_fixture("dirty_preservation");
    let service = WorktreeService::with_worktrees_dir(repo_dir.clone(), worktrees_dir.clone());

    let desc = service
        .create_worktree("dirty-task", None)
        .expect("create worktree");

    // Introduce uncommitted change
    let dirty_file = desc.path.join("new_work.txt");
    let mut f = File::create(&dirty_file).expect("create dirty file");
    writeln!(f, "Important subagent work in progress").expect("write dirty");

    // Check dirty directly
    let dirty_status = service.check_dirty(&desc.path).expect("check dirty status");
    assert!(dirty_status.is_some());
    assert!(dirty_status.unwrap().contains("new_work.txt"));

    // Attempting safe removal MUST FAIL and NEVER delete the worktree
    let remove_err = service
        .remove_worktree("dirty-task", false)
        .expect_err("safe removal must fail on dirty worktree");

    match remove_err {
        WorktreeError::WorktreeDirty {
            path,
            porcelain_status,
        } => {
            assert_eq!(path, desc.path);
            assert!(porcelain_status.contains("new_work.txt"));
        }
        other => panic!("Expected WorktreeDirty error, got {other:?}"),
    }

    // INVARIANT CHECK: Filesystem directory and untracked file MUST STILL EXIST
    assert!(desc.path.exists());
    assert!(dirty_file.exists());

    // Now explicit force removal deletes it
    service
        .remove_worktree("dirty-task", true)
        .expect("forced removal succeeds");
    assert!(!desc.path.exists());
}

#[test]
fn test_stale_worktree_pruning() {
    let (repo_dir, worktrees_dir) = create_test_repo_fixture("prune");
    let service = WorktreeService::with_worktrees_dir(repo_dir.clone(), worktrees_dir.clone());

    let desc = service
        .create_worktree("stale-task", None)
        .expect("create worktree");
    assert!(desc.path.exists());

    // Prune runs cleanly without errors
    let prune_res = service.prune_stale();
    assert!(prune_res.is_ok());

    // Clean up
    service.remove_worktree("stale-task", false).unwrap();
}

#[test]
fn test_worktree_name_traversal_validation() {
    let invalid_names = vec![
        "",
        "   ",
        ".",
        "..",
        "../escape",
        "nested/path",
        "windows\\path",
        "null\0byte",
        "has spaces",
        "has$symbol",
        "has;command",
    ];

    for name in invalid_names {
        let res = WorktreeService::validate_worktree_name(name);
        assert!(
            res.is_err(),
            "Expected name '{}' to fail validation, but got: {:?}",
            name,
            res
        );
        match res.unwrap_err() {
            WorktreeError::InvalidName(_) => {}
            other => panic!("Expected InvalidName, got {:?}", other),
        }
    }

    let valid_names = vec!["feature-123", "subagent_task.v2", "alpha-beta-gamma"];

    for name in valid_names {
        let res = WorktreeService::validate_worktree_name(name);
        assert!(
            res.is_ok(),
            "Expected valid name '{}', got error: {:?}",
            name,
            res
        );
    }
}

#[test]
fn test_unmerged_commits_preservation() {
    let (repo_dir, worktrees_dir) = create_test_repo_fixture("unmerged_commits");
    let service = WorktreeService::with_worktrees_dir(repo_dir.clone(), worktrees_dir.clone());

    let desc = service
        .create_worktree("unmerged-branch", None)
        .expect("create worktree");

    // Commit a new file inside the worktree
    let commit_file = desc.path.join("committed_work.txt");
    let mut f = File::create(&commit_file).expect("create file");
    writeln!(f, "Permanent commit on worktree branch").expect("write commit file");

    let add_out = Command::new("git")
        .arg("-C")
        .arg(&desc.path)
        .args(["add", "committed_work.txt"])
        .output()
        .expect("git add in worktree");
    assert!(add_out.status.success());

    let commit_out = Command::new("git")
        .arg("-C")
        .arg(&desc.path)
        .args(["commit", "-m", "Worktree commit on branch"])
        .output()
        .expect("git commit in worktree");
    assert!(commit_out.status.success());

    // Worktree is clean according to status
    let dirty_status = service.check_dirty(&desc.path).expect("check dirty");
    assert!(
        dirty_status.is_none(),
        "Working directory should be clean after commit"
    );

    // Attempting safe deletion MUST fail because of unmerged commits
    let remove_err = service
        .remove_worktree("unmerged-branch", false)
        .expect_err("safe deletion must fail when branch has unmerged commits");

    match remove_err {
        WorktreeError::UnmergedCommitsPreserved {
            branch,
            unmerged_commits,
        } => {
            assert_eq!(branch, desc.branch);
            assert!(unmerged_commits.contains("Worktree commit on branch"));
        }
        other => panic!("Expected UnmergedCommitsPreserved, got {:?}", other),
    }

    // Worktree path and file must still exist
    assert!(desc.path.exists());
    assert!(commit_file.exists());

    // Forced removal cleans it up
    service
        .remove_worktree("unmerged-branch", true)
        .expect("forced removal should succeed");
    assert!(!desc.path.exists());
}

#[test]
fn test_uncommitted_snapshot_application() {
    let (repo_dir, worktrees_dir) = create_test_repo_fixture("snapshot");
    let service = WorktreeService::with_worktrees_dir(repo_dir.clone(), worktrees_dir.clone());

    // Create an uncommitted change to tracked README.md in parent repo
    let mut f = File::create(repo_dir.join("README.md")).expect("open readme");
    writeln!(f, "# Modified Readme In Parent").expect("write readme");

    // Create an untracked source file in parent repo
    let src_dir = repo_dir.join("src");
    fs::create_dir_all(&src_dir).expect("create src dir");
    let mut f_src = File::create(src_dir.join("main.rs")).expect("create main.rs");
    writeln!(f_src, "fn main() {{ println!(\"hello\"); }}").expect("write main.rs");

    // Create sensitive credential files in parent repo
    let mut f_env = File::create(repo_dir.join(".env.local")).expect("create .env");
    writeln!(f_env, "SECRET_KEY=supersecret").expect("write .env");

    let mut f_ssh = File::create(repo_dir.join("id_rsa")).expect("create id_rsa");
    writeln!(f_ssh, "-----BEGIN RSA PRIVATE KEY-----").expect("write id_rsa");

    // Create worktree WITH snapshot enabled
    let desc = service
        .create_worktree_with_snapshot("snap-test", None, true)
        .expect("create worktree with snapshot");

    assert!(desc.parent_had_uncommitted_drift);
    assert!(desc.snapshot_applied);

    // Verify tracked and untracked code files were copied
    let wt_readme = fs::read_to_string(desc.path.join("README.md")).expect("read wt readme");
    assert!(wt_readme.contains("# Modified Readme In Parent"));

    let wt_src = fs::read_to_string(desc.path.join("src").join("main.rs")).expect("read wt src");
    assert!(wt_src.contains("fn main()"));

    // SEC-03 VERIFICATION: Sensitive credentials MUST NOT be copied!
    assert!(
        !desc.path.join(".env.local").exists(),
        "SEC-03 VIOLATION: .env.local must not be snapshotted into worktree"
    );
    assert!(
        !desc.path.join("id_rsa").exists(),
        "SEC-03 VIOLATION: id_rsa must not be snapshotted into worktree"
    );

    // Clean up
    service.remove_worktree("snap-test", true).unwrap();
}

#[test]
fn test_clean_worktree_no_additional_commits() {
    let (repo_dir, worktrees_dir) = create_test_repo_fixture("clean_no_commits");
    let service = WorktreeService::with_worktrees_dir(repo_dir.clone(), worktrees_dir.clone());

    let desc = service
        .create_worktree("clean-task", None)
        .expect("create worktree");

    // No commits, no dirty files
    let unmerged = service
        .check_unmerged_commits(
            &desc.branch,
            &desc.base_commit,
            desc.target_branch.as_deref(),
        )
        .expect("check unmerged");
    assert!(
        unmerged.is_none(),
        "Clean worktree with no commits should report no unmerged work"
    );

    // Safe removal succeeds
    service
        .remove_worktree("clean-task", false)
        .expect("safe removal must succeed when clean and no commits");
    assert!(!desc.path.exists());
}

#[test]
fn test_clean_worktree_multiple_unmerged_commits() {
    let (repo_dir, worktrees_dir) = create_test_repo_fixture("multiple_commits");
    let service = WorktreeService::with_worktrees_dir(repo_dir.clone(), worktrees_dir.clone());

    let desc = service
        .create_worktree("multi-commit-task", None)
        .expect("create worktree");

    // Create commit 1
    let f1 = desc.path.join("file1.txt");
    fs::write(&f1, "Commit 1 content").unwrap();
    Command::new("git")
        .arg("-C")
        .arg(&desc.path)
        .args(["add", "file1.txt"])
        .output()
        .unwrap();
    Command::new("git")
        .arg("-C")
        .arg(&desc.path)
        .args(["commit", "-m", "Commit 1: Implement feature core"])
        .output()
        .unwrap();

    // Create commit 2
    let f2 = desc.path.join("file2.txt");
    fs::write(&f2, "Commit 2 content").unwrap();
    Command::new("git")
        .arg("-C")
        .arg(&desc.path)
        .args(["add", "file2.txt"])
        .output()
        .unwrap();
    Command::new("git")
        .arg("-C")
        .arg(&desc.path)
        .args(["commit", "-m", "Commit 2: Add validation layer"])
        .output()
        .unwrap();

    // Create commit 3
    let f3 = desc.path.join("file3.txt");
    fs::write(&f3, "Commit 3 content").unwrap();
    Command::new("git")
        .arg("-C")
        .arg(&desc.path)
        .args(["add", "file3.txt"])
        .output()
        .unwrap();
    Command::new("git")
        .arg("-C")
        .arg(&desc.path)
        .args(["commit", "-m", "Commit 3: Add documentation"])
        .output()
        .unwrap();

    // Working directory is clean
    assert!(service.check_dirty(&desc.path).unwrap().is_none());

    // Safe deletion MUST fail and list all 3 commits
    let err = service
        .remove_worktree("multi-commit-task", false)
        .unwrap_err();
    match err {
        WorktreeError::UnmergedCommitsPreserved {
            branch,
            unmerged_commits,
        } => {
            assert_eq!(branch, desc.branch);
            assert!(unmerged_commits.contains("Commit 1: Implement feature core"));
            assert!(unmerged_commits.contains("Commit 2: Add validation layer"));
            assert!(unmerged_commits.contains("Commit 3: Add documentation"));
        }
        other => panic!("Expected UnmergedCommitsPreserved, got {other:?}"),
    }

    assert!(desc.path.exists());
    service.remove_worktree("multi-commit-task", true).unwrap();
}

#[test]
fn test_worktree_changes_merged_into_target_branch() {
    let (repo_dir, worktrees_dir) = create_test_repo_fixture("merged_task");
    let service = WorktreeService::with_worktrees_dir(repo_dir.clone(), worktrees_dir.clone());

    let desc = service
        .create_worktree("merged-task", None)
        .expect("create worktree");

    // Create a commit in the worktree
    let f1 = desc.path.join("feature.txt");
    fs::write(&f1, "Feature merged").unwrap();
    Command::new("git")
        .arg("-C")
        .arg(&desc.path)
        .args(["add", "feature.txt"])
        .output()
        .unwrap();
    Command::new("git")
        .arg("-C")
        .arg(&desc.path)
        .args(["commit", "-m", "Completed feature"])
        .output()
        .unwrap();

    // Merge the worktree branch into parent repo HEAD (master/main)
    let merge_out = Command::new("git")
        .arg("-C")
        .arg(&repo_dir)
        .args(["merge", &desc.branch])
        .output()
        .expect("git merge");
    assert!(
        merge_out.status.success(),
        "Merge should succeed in parent repo"
    );

    // Now check_unmerged_commits should detect that it is fully merged!
    let unmerged = service
        .check_unmerged_commits(
            &desc.branch,
            &desc.base_commit,
            desc.target_branch.as_deref(),
        )
        .expect("check unmerged");
    assert!(
        unmerged.is_none(),
        "Merged worktree should report no unmerged commits"
    );

    // Safe removal should now SUCCEED!
    service
        .remove_worktree("merged-task", false)
        .expect("safe removal must succeed when commits are merged");
    assert!(!desc.path.exists());
}

#[test]
fn test_divergent_history_and_rebase_safety() {
    let (repo_dir, worktrees_dir) = create_test_repo_fixture("divergent");
    let service = WorktreeService::with_worktrees_dir(repo_dir.clone(), worktrees_dir.clone());

    let desc = service
        .create_worktree("divergent-task", None)
        .expect("create worktree");

    // Commit on worktree
    let f_wt = desc.path.join("wt_change.txt");
    fs::write(&f_wt, "Worktree change").unwrap();
    Command::new("git")
        .arg("-C")
        .arg(&desc.path)
        .args(["add", "wt_change.txt"])
        .output()
        .unwrap();
    Command::new("git")
        .arg("-C")
        .arg(&desc.path)
        .args(["commit", "-m", "Worktree commit on branch"])
        .output()
        .unwrap();

    // Also commit on parent master (creating divergent branch history)
    let f_parent = repo_dir.join("parent_change.txt");
    fs::write(&f_parent, "Parent change").unwrap();
    Command::new("git")
        .arg("-C")
        .arg(&repo_dir)
        .args(["add", "parent_change.txt"])
        .output()
        .unwrap();
    Command::new("git")
        .arg("-C")
        .arg(&repo_dir)
        .args(["commit", "-m", "Parent master commit"])
        .output()
        .unwrap();

    // Safe deletion MUST refuse because worktree commit has not been merged into parent
    let err = service
        .remove_worktree("divergent-task", false)
        .unwrap_err();
    match err {
        WorktreeError::UnmergedCommitsPreserved {
            unmerged_commits, ..
        } => {
            assert!(unmerged_commits.contains("Worktree commit on branch"));
        }
        other => panic!("Expected UnmergedCommitsPreserved on divergent branch, got {other:?}"),
    }

    assert!(desc.path.exists());
    service.remove_worktree("divergent-task", true).unwrap();
}

#[test]
fn test_concurrent_cleanup_while_agent_is_active() {
    let (repo_dir, worktrees_dir) = create_test_repo_fixture("active_agent");
    let service = WorktreeService::with_worktrees_dir(repo_dir.clone(), worktrees_dir.clone());

    let subagent_id = SubagentId::new();
    let desc = service
        .create_worktree("agent-occupied", Some(subagent_id))
        .expect("create worktree");

    // Configure active agent checker to report that this subagent is currently running
    let active_id = subagent_id;
    service.set_active_agent_checker(move |id| *id == active_id);

    // Attempting safe removal MUST FAIL because subagent is actively executing
    let err = service
        .remove_worktree("agent-occupied", false)
        .expect_err("safe removal must fail when subagent is active");

    match err {
        WorktreeError::WorktreeActive {
            path,
            subagent_id: sid,
        } => {
            assert_eq!(path, desc.path);
            assert_eq!(sid, subagent_id);
        }
        other => panic!("Expected WorktreeActive error, got {other:?}"),
    }

    assert!(desc.path.exists());

    // Once subagent finishes (checker returns false), removal succeeds
    service.set_active_agent_checker(|_| false);
    service
        .remove_worktree("agent-occupied", false)
        .expect("removal succeeds after agent finishes");
    assert!(!desc.path.exists());
}

#[test]
fn test_snapshot_preserves_child_changes_and_excludes_symlink_secrets() {
    let (repo_dir, worktrees_dir) = create_test_repo_fixture("snapshot_safety");
    let service = WorktreeService::with_worktrees_dir(repo_dir.clone(), worktrees_dir.clone());

    // Create a tracked file in parent repo
    let code_file = repo_dir.join("logic.rs");
    fs::write(&code_file, "original code").unwrap();
    Command::new("git")
        .arg("-C")
        .arg(&repo_dir)
        .args(["add", "logic.rs"])
        .output()
        .unwrap();
    Command::new("git")
        .arg("-C")
        .arg(&repo_dir)
        .args(["commit", "-m", "Add logic.rs"])
        .output()
        .unwrap();

    // Create child worktree
    let desc = service
        .create_worktree("snap-safe", None)
        .expect("create worktree");

    // 1. Child modifies logic.rs
    fs::write(desc.path.join("logic.rs"), "child modified code").unwrap();

    // 2. Parent modifies logic.rs as well (uncommitted)
    fs::write(&code_file, "parent modified code").unwrap();

    // 3. Parent creates secret token file and a symlink to outside repo
    fs::write(repo_dir.join("auth_token.txt"), "secret_bearer_token").unwrap();
    let external_secret = repo_dir.parent().unwrap().join("external_secret.key");
    fs::write(&external_secret, "PRIVATE_KEY_DATA").unwrap();

    #[cfg(unix)]
    let _ = std::os::unix::fs::symlink(&external_secret, repo_dir.join("symlink_escape.key"));

    // Apply snapshot
    let _copied = service
        .apply_uncommitted_snapshot(&desc.path)
        .expect("apply snapshot");

    // INVARIANT 1: Child's local modification to logic.rs MUST NOT be overwritten!
    let child_content = fs::read_to_string(desc.path.join("logic.rs")).unwrap();
    assert_eq!(
        child_content, "child modified code",
        "Snapshot must NOT overwrite existing child edits"
    );

    // INVARIANT 2: Secret tokens and symlink escapes MUST NOT be copied
    assert!(
        !desc.path.join("auth_token.txt").exists(),
        "auth_token.txt must be excluded"
    );
    assert!(
        !desc.path.join("symlink_escape.key").exists(),
        "Symlink escape must be excluded"
    );

    let _ = fs::remove_file(&external_secret);
    service.remove_worktree("snap-safe", true).unwrap();
}
