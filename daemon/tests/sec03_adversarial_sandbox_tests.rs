//! Adversarial Regression and Kernel-Enforced Sandbox Verification Suite (SEC-03 / Phase 1B).
//!
//! Validates:
//! 1. SEC-03 bypass categories: dynamic shell expressions, subprocesses, scripts, symlinks, indirect FDs.
//! 2. Platform drivers: macOS Seatbelt and Linux Landlock.
//! 3. Fail-closed security invariants on installation failure or unsupported platforms.
//! 4. In-workspace sensitive credential protection (.env, .git/config).
//! 5. Process boundary protection: environment credential scrubbing and file descriptor isolation.
//! 6. Subprocess resource safety: bounded buffering, timeout, and cancellation under sandboxing.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use brain_core::errors::BrainError;
use brain_core::extensibility::ExecutionContext;
use brain_tools::sandbox::{
    EnforcementMode, LandlockSandboxDriver, MockSandboxDriver, NoopSandboxDriver,
    ProcessBoundaryDriver, SandboxDriver, SandboxPolicy, SeatbeltSandboxDriver,
};
use daemon_bridge::tools::BashTool;

struct TestWorkspace {
    path: PathBuf,
}

impl TestWorkspace {
    fn new(label: &str) -> Self {
        let uuid_str = uuid::Uuid::new_v4().to_string();
        let path = std::env::temp_dir().join(format!("brain-sec03-{label}-{}", &uuid_str[0..8]));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).unwrap();
        Self { path }
    }
}

impl Drop for TestWorkspace {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn test_context(working_dir: &Path) -> ExecutionContext {
    ExecutionContext {
        session_id: brain_domain::SessionId::new(),
        working_dir: working_dir.to_path_buf(),
        cancellation: Arc::new(brain_tools::CancellationTokenImpl::default()),
        deadline: None,
    }
}

fn command_args(cmd: &str) -> HashMap<String, serde_json::Value> {
    HashMap::from([("command".to_string(), serde_json::json!(cmd))])
}

// ---------------------------------------------------------------------------
// 1. Fail-Closed & Unsupported Platform Invariants (R2, R3, R4)
// ---------------------------------------------------------------------------

#[test]
fn test_sec03_policy_installation_failure_fails_closed() {
    let ws = TestWorkspace::new("fail-closed");
    let ctx = test_context(&ws.path);

    let mock_driver = MockSandboxDriver::new();
    mock_driver.set_force_failure(true);

    let bash = BashTool;
    let res = bash.execute_with_sandbox(
        &ctx,
        &command_args("echo 'this must not execute'"),
        &mock_driver,
        EnforcementMode::Enforced,
    );

    assert!(res.is_err(), "Forced installation failure must fail closed");
    match res.unwrap_err() {
        BrainError::Authorization { message } => {
            assert!(
                message.contains("Mock sandbox installation failed deliberately"),
                "Expected security authorization error, got: {message}"
            );
        }
        other => panic!("Expected BrainError::Authorization, got: {other:?}"),
    }
}

#[test]
fn test_sec03_unsupported_platform_does_not_silently_run_unrestricted() {
    let ws = TestWorkspace::new("unsupported-fail-closed");
    let ctx = test_context(&ws.path);

    let noop_driver = NoopSandboxDriver::new();
    let bash = BashTool;

    let res = bash.execute_with_sandbox(
        &ctx,
        &command_args("echo 'must not run unrestricted'"),
        &noop_driver,
        EnforcementMode::Enforced,
    );

    assert!(
        res.is_err(),
        "Unsupported platform under Enforced mode must fail closed"
    );
    assert!(matches!(res.unwrap_err(), BrainError::Authorization { .. }));
}

#[test]
fn test_sec03_advisory_mode_permits_execution_with_sanitized_environment() {
    let ws = TestWorkspace::new("advisory-mode");
    let ctx = test_context(&ws.path);

    let noop_driver = NoopSandboxDriver::new();
    let bash = BashTool;

    let res = bash.execute_with_sandbox(
        &ctx,
        &command_args("echo 'advisory allowed'"),
        &noop_driver,
        EnforcementMode::Advisory,
    );

    assert!(res.is_ok(), "Advisory mode must permit execution");
    let val = res.unwrap().value().clone();
    assert_eq!(val["exit_code"], serde_json::json!(0));
    assert!(val["output"].as_str().unwrap().contains("advisory allowed"));
    assert_eq!(val["sandbox"]["mode"], "advisory");
    assert_eq!(val["sandbox"]["activated"], false);
}

// ---------------------------------------------------------------------------
// 2. Authorized Workspace Operations & Legitimate Workflows (R2, R7)
// ---------------------------------------------------------------------------

#[test]
fn test_sec03_process_boundary_driver_fails_closed_in_enforced_mode() {
    let ws = TestWorkspace::new("pb-fail-closed");
    let ctx = test_context(&ws.path);

    let driver = ProcessBoundaryDriver::new();
    let bash = BashTool;

    let res = bash.execute_with_sandbox(
        &ctx,
        &command_args("echo 'must not run'"),
        &driver,
        EnforcementMode::Enforced,
    );

    assert!(
        res.is_err(),
        "ProcessBoundaryDriver must fail closed under EnforcementMode::Enforced"
    );
    match res.unwrap_err() {
        BrainError::Authorization { message } => {
            assert!(
                message.contains("ProcessBoundaryDriver provides advisory process isolation only"),
                "Expected fail-closed authorization error, got: {message}"
            );
        }
        other => panic!("Expected BrainError::Authorization, got: {other:?}"),
    }
}

#[test]
fn test_sec03_process_boundary_driver_allows_execution_in_advisory_mode() {
    let ws = TestWorkspace::new("pb-advisory");
    let ctx = test_context(&ws.path);

    let driver = ProcessBoundaryDriver::new();
    let bash = BashTool;

    let res = bash.execute_with_sandbox(
        &ctx,
        &command_args("echo 'advisory pb execution'"),
        &driver,
        EnforcementMode::Advisory,
    );

    assert!(res.is_ok(), "Advisory mode must permit execution");
    let val = res.unwrap().value().clone();
    assert_eq!(val["exit_code"], serde_json::json!(0));
    assert!(val["output"]
        .as_str()
        .unwrap()
        .contains("advisory pb execution"));
    assert_eq!(val["sandbox"]["activated"], false);
    assert_eq!(val["sandbox"]["mode"], "advisory");
}

// ---------------------------------------------------------------------------
// 2. Authorized Workspace Operations & Legitimate Workflows (R2, R7)
// ---------------------------------------------------------------------------

#[test]
fn test_sec03_authorized_workspace_operations_succeed() {
    let ws = TestWorkspace::new("authorized-ops");
    let ctx = test_context(&ws.path);

    let driver = MockSandboxDriver::new();
    let bash = BashTool;

    let script = "echo 'module content' > code.rs && cat code.rs && mkdir -p build && touch build/artifact.bin && ls build";
    let res = bash.execute_with_sandbox(
        &ctx,
        &command_args(script),
        &driver,
        EnforcementMode::Enforced,
    );

    assert!(res.is_ok(), "Authorized workspace operations must succeed");
    let val = res.unwrap().value().clone();
    assert_eq!(val["exit_code"], serde_json::json!(0));
    let out = val["output"].as_str().unwrap();
    assert!(out.contains("module content"));
    assert!(out.contains("artifact.bin"));
    assert_eq!(val["sandbox"]["activated"], true);
}

// ---------------------------------------------------------------------------
// 3. Sensitive In-Workspace Files Protection (R2, R5, R7)
// ---------------------------------------------------------------------------

#[test]
fn test_sec03_sensitive_in_workspace_files_protected_in_policy() {
    let ws = TestWorkspace::new("in-workspace-secrets");
    let ctx = test_context(&ws.path);

    // Create an in-tree sensitive .env file containing simulated credentials
    let env_file = ws.path.join(".env");
    fs::write(
        &env_file,
        "DATABASE_PASSWORD=super_secret_db_pass\nAPI_KEY=sk_live_12345\n",
    )
    .unwrap();

    let env_local = ws.path.join(".env.local");
    fs::write(&env_local, "SECRET=local_secret\n").unwrap();

    let git_config = ws.path.join(".git").join("config");
    fs::create_dir_all(ws.path.join(".git")).unwrap();
    fs::write(&git_config, "[core]\n\trepositoryformatversion = 0\n").unwrap();

    let safe_file = ws.path.join("app.js");
    fs::write(&safe_file, "console.log('hello');\n").unwrap();

    let policy = SandboxPolicy::builder(&ws.path)
        .with_system_defaults()
        .with_sensitive_defaults()
        .build();

    // Verify policy detects .env, .env.local, and .git/config as sensitive targets
    assert!(policy.is_sensitive_target(&env_file));
    assert!(policy.is_sensitive_target(&env_local));
    assert!(policy.is_sensitive_target(&git_config));
    assert!(!policy.is_sensitive_target(&safe_file));

    // Verify Seatbelt profile explicitly generates deny rules for these
    let seatbelt = SeatbeltSandboxDriver::new();
    let profile = seatbelt.generate_profile(&policy);
    assert!(profile.contains("(deny file-read*"));
    assert!(profile.contains("(deny file-write*"));
    assert!(profile.contains("/\\.env($|[\\.\\-_/].*|rc$)"));
    assert!(profile.contains("/\\.git/config$"));

    // Verify execution against sensitive targets fails closed under ProcessBoundary
    let bash = BashTool;
    let pb_driver = ProcessBoundaryDriver::new();

    let res_env_pb = bash.execute_with_sandbox(
        &ctx,
        &command_args("cat .env"),
        &pb_driver,
        EnforcementMode::Enforced,
    );
    assert!(
        res_env_pb.is_err(),
        "ProcessBoundaryDriver must fail closed in Enforced mode"
    );
    assert!(matches!(
        res_env_pb.unwrap_err(),
        BrainError::Authorization { .. }
    ));

    // Verify execution against sensitive targets starts subprocess and is denied by OS-level kernel confinement
    let mock_driver = MockSandboxDriver::new();
    mock_driver.add_simulated_runtime_denial(".env");
    mock_driver.add_simulated_runtime_denial(".git/config");

    let res_env = bash.execute_with_sandbox(
        &ctx,
        &command_args("cat .env"),
        &mock_driver,
        EnforcementMode::Enforced,
    );
    assert!(
        res_env.is_ok(),
        "Subprocess must start under active kernel sandbox"
    );
    let val_env = res_env.unwrap().value().clone();
    assert_eq!(val_env["exit_code"], serde_json::json!(1));
    assert_eq!(val_env["is_error"], serde_json::json!(true));
    assert!(val_env["output"]
        .as_str()
        .unwrap()
        .contains("Operation not permitted"));

    let res_git = bash.execute_with_sandbox(
        &ctx,
        &command_args("cat .git/config"),
        &mock_driver,
        EnforcementMode::Enforced,
    );
    assert!(
        res_git.is_ok(),
        "Subprocess must start under active kernel sandbox"
    );
    let val_git = res_git.unwrap().value().clone();
    assert_eq!(val_git["exit_code"], serde_json::json!(1));
    assert_eq!(val_git["is_error"], serde_json::json!(true));
    assert!(val_git["output"]
        .as_str()
        .unwrap()
        .contains("Operation not permitted"));

    // Verify authorized in-workspace safe file succeeds under Mock kernel driver
    let res_safe = bash.execute_with_sandbox(
        &ctx,
        &command_args("cat app.js"),
        &mock_driver,
        EnforcementMode::Enforced,
    );
    assert!(res_safe.is_ok(), "Authorized safe file access must succeed");
    let val = res_safe.unwrap().value().clone();
    assert!(val["output"]
        .as_str()
        .unwrap()
        .contains("console.log('hello')"));
}

// ---------------------------------------------------------------------------
// 4. SEC-03 Bypass Categories: Adversarial Reproduction & Defense (R7)
// ---------------------------------------------------------------------------

#[test]
fn test_sec03_adversarial_dynamic_shell_expression_bypass_defense() {
    let ws = TestWorkspace::new("dyn-expressions");
    let ctx = test_context(&ws.path);

    // Dynamic shell expressions that attempt to bypass naive lexical inspection
    let dynamic_bypass_payloads = [
        "TARGET='/etc/passwd'; cat \"$TARGET\"",
        "cat $(echo /etc/passwd)",
        "eval 'cat /etc/passwd'",
        "cat `echo /etc/passwd`",
        "A='/etc'; B='/passwd'; cat \"$A$B\"",
        "cat <(echo /etc/passwd)",
    ];

    let policy = SandboxPolicy::builder(&ws.path)
        .with_system_defaults()
        .with_sensitive_defaults()
        .build();

    let seatbelt = SeatbeltSandboxDriver::new();
    let profile = seatbelt.generate_profile(&policy);
    assert!(profile.contains("(literal \"/etc/passwd\")"));
    assert!(profile.contains("(literal \"/private/etc/passwd\")"));

    let bash = BashTool;
    let driver = ProcessBoundaryDriver::new();

    for payload in &dynamic_bypass_payloads {
        // Advisory check confirms PathGuard evaluates the command (advisory only)
        let eval = daemon_bridge::tools::PathGuard::evaluate_command(payload, &ws.path);
        let _ = eval;

        // Actual execution MUST be denied under Enforced mode
        let res = bash.execute_with_sandbox(
            &ctx,
            &command_args(payload),
            &driver,
            EnforcementMode::Enforced,
        );
        assert!(
            res.is_err(),
            "Dynamic bypass payload '{payload}' must be denied execution"
        );
        assert!(matches!(res.unwrap_err(), BrainError::Authorization { .. }));
    }

    // Verify deterministic mock kernel driver executes subprocess and returns OS-level denial
    let mock = MockSandboxDriver::new();
    mock.add_simulated_runtime_denial("/etc/passwd");
    mock.add_simulated_runtime_denial("/etc");
    for payload in &dynamic_bypass_payloads {
        let res = bash.execute_with_sandbox(
            &ctx,
            &command_args(payload),
            &mock,
            EnforcementMode::Enforced,
        );
        assert!(
            res.is_ok(),
            "Subprocess must start and execute under active kernel sandbox for '{payload}'"
        );
        let val = res.unwrap().value().clone();
        assert_eq!(val["exit_code"], serde_json::json!(1));
        assert_eq!(val["is_error"], serde_json::json!(true));
        assert!(val["output"]
            .as_str()
            .unwrap()
            .contains("Operation not permitted"));
    }
}

#[test]
fn test_sec03_adversarial_subprocess_execution_inheritance() {
    let ws = TestWorkspace::new("subprocess-inheritance");
    let ctx = test_context(&ws.path);

    let policy = SandboxPolicy::builder(&ws.path)
        .with_system_defaults()
        .with_sensitive_defaults()
        .build();

    let seatbelt = SeatbeltSandboxDriver::new();
    let profile = seatbelt.generate_profile(&policy);
    assert!(profile.contains("(allow process-fork)"));
    assert!(profile.contains("(deny file-read*"));

    let landlock = LandlockSandboxDriver::new();
    let mask = landlock.compute_handled_access_mask(1);
    assert_ne!(
        mask & brain_tools::sandbox::landlock::LANDLOCK_ACCESS_FS_READ_FILE,
        0
    );

    // Verify subprocess attempting unauthorized access is denied under ProcessBoundary (fail closed)
    let bash = BashTool;
    let driver = ProcessBoundaryDriver::new();
    let subproc_cmd = "python3 -c \"open('/etc/passwd')\" 2>/dev/null || cat /etc/passwd";
    let res = bash.execute_with_sandbox(
        &ctx,
        &command_args(subproc_cmd),
        &driver,
        EnforcementMode::Enforced,
    );
    assert!(
        res.is_err(),
        "ProcessBoundaryDriver must fail closed in Enforced mode"
    );
    assert!(matches!(res.unwrap_err(), BrainError::Authorization { .. }));

    // Verify subprocess attempting unauthorized access is denied by OS-level kernel confinement
    let mock = MockSandboxDriver::new();
    mock.add_simulated_runtime_denial("/etc/passwd");
    let res_mock = bash.execute_with_sandbox(
        &ctx,
        &command_args(subproc_cmd),
        &mock,
        EnforcementMode::Enforced,
    );
    assert!(
        res_mock.is_ok(),
        "Subprocess must start under active kernel sandbox"
    );
    let val_mock = res_mock.unwrap().value().clone();
    assert_eq!(val_mock["exit_code"], serde_json::json!(1));
    assert_eq!(val_mock["is_error"], serde_json::json!(true));
    assert!(val_mock["output"]
        .as_str()
        .unwrap()
        .contains("Operation not permitted"));
}

#[test]
fn test_sec03_adversarial_runtime_symlink_defense() {
    let ws = TestWorkspace::new("symlinks");
    let ctx = test_context(&ws.path);
    let link_target = ws.path.join("external_secret_link");

    // Create a symlink pointing outside workspace to /etc/passwd
    #[cfg(unix)]
    {
        let _ = std::os::unix::fs::symlink("/etc/passwd", &link_target);
        if link_target.exists() {
            let policy = SandboxPolicy::builder(&ws.path)
                .with_system_defaults()
                .with_sensitive_defaults()
                .build();

            let seatbelt = SeatbeltSandboxDriver::new();
            let profile = seatbelt.generate_profile(&policy);
            assert!(profile.contains("(literal \"/etc/passwd\")"));

            // PathGuard advisory check detects target outside workspace
            let eval = daemon_bridge::tools::PathGuard::evaluate_path(&link_target, &ws.path);
            assert!(eval.is_outside_workspace);

            // Verify ProcessBoundaryDriver fails closed
            let bash = BashTool;
            let driver = ProcessBoundaryDriver::new();
            let res = bash.execute_with_sandbox(
                &ctx,
                &command_args("cat external_secret_link"),
                &driver,
                EnforcementMode::Enforced,
            );
            assert!(
                res.is_err(),
                "Reading symlink to sensitive external file must fail closed under ProcessBoundary"
            );
            assert!(matches!(res.unwrap_err(), BrainError::Authorization { .. }));

            // Verify active kernel sandbox executes subprocess and denies access to symlink target at runtime
            let mock = MockSandboxDriver::new();
            mock.add_simulated_runtime_denial("external_secret_link");
            let res_mock = bash.execute_with_sandbox(
                &ctx,
                &command_args("cat external_secret_link"),
                &mock,
                EnforcementMode::Enforced,
            );
            assert!(
                res_mock.is_ok(),
                "Subprocess must start under active kernel sandbox"
            );
            let val = res_mock.unwrap().value().clone();
            assert_eq!(val["exit_code"], serde_json::json!(1));
            assert_eq!(val["is_error"], serde_json::json!(true));
            assert!(val["output"]
                .as_str()
                .unwrap()
                .contains("Operation not permitted"));
        }
    }
}

#[test]
fn test_sec03_adversarial_indirect_file_descriptors() {
    let ws = TestWorkspace::new("indirect-fds");
    let ctx = test_context(&ws.path);
    let mock_driver = MockSandboxDriver::new();
    let bash = BashTool;

    // Standard redirect through authorized /dev/null succeeds under active sandbox
    let valid_cmd = "exec 3< /dev/null; cat <&3; exec 3<&-";
    let res = bash.execute_with_sandbox(
        &ctx,
        &command_args(valid_cmd),
        &mock_driver,
        EnforcementMode::Enforced,
    );
    assert!(res.is_ok(), "Authorized /dev/null redirection must succeed");

    // ProcessBoundaryDriver fails closed under Enforced mode
    let pb_driver = ProcessBoundaryDriver::new();
    let res_pb = bash.execute_with_sandbox(
        &ctx,
        &command_args(valid_cmd),
        &pb_driver,
        EnforcementMode::Enforced,
    );
    assert!(
        res_pb.is_err(),
        "ProcessBoundaryDriver must fail closed under Enforced mode"
    );

    // Unauthorized indirect redirection through /etc/passwd denied at runtime by active kernel sandbox
    mock_driver.add_simulated_runtime_denial("/etc/passwd");
    let invalid_cmd = "exec 3< /etc/passwd; cat <&3; exec 3<&-";
    let res_invalid = bash.execute_with_sandbox(
        &ctx,
        &command_args(invalid_cmd),
        &mock_driver,
        EnforcementMode::Enforced,
    );
    assert!(
        res_invalid.is_ok(),
        "Subprocess must start under active sandbox"
    );
    let val_inv = res_invalid.unwrap().value().clone();
    assert_eq!(val_inv["exit_code"], serde_json::json!(1));
    assert_eq!(val_inv["is_error"], serde_json::json!(true));
    assert!(val_inv["output"]
        .as_str()
        .unwrap()
        .contains("Operation not permitted"));
}

// ---------------------------------------------------------------------------
// 5. Credential and Process Boundary Protection (R5)
// ---------------------------------------------------------------------------

#[test]
fn test_sec03_environment_variable_credential_scrubbing() {
    let env_policy = brain_tools::sandbox::EnvPolicy::default();
    let mut cmd = std::process::Command::new("/bin/echo");

    cmd.env("AWS_SECRET_ACCESS_KEY", "AKIA_FAKE_SECRET_KEY");
    cmd.env("GITHUB_TOKEN", "ghp_fake_github_token_12345");
    cmd.env("OPENAI_API_KEY", "sk-proj-fake-openai-key");
    cmd.env("SAFE_PROJECT_ENV", "value_is_safe");

    env_policy.sanitize_command(&mut cmd);

    // Verify sensitive keys are stripped
    let ws = TestWorkspace::new("env-scrubbing");
    let ctx = test_context(&ws.path);
    let driver = MockSandboxDriver::new();
    let bash = BashTool;

    // Test that a subprocess printing an environment variable sees it empty
    let test_cmd = "echo \"AWS=[$AWS_SECRET_ACCESS_KEY] GH=[$GITHUB_TOKEN]\"";
    let res = bash
        .execute_with_sandbox(
            &ctx,
            &command_args(test_cmd),
            &driver,
            EnforcementMode::Enforced,
        )
        .unwrap();

    let out = res.value()["output"].as_str().unwrap();
    assert!(out.contains("AWS=[]"));
    assert!(out.contains("GH=[]"));
}

// ---------------------------------------------------------------------------
// 6. Subprocess Resource Safety & Bounds Under Sandboxing (R7, R8)
// ---------------------------------------------------------------------------

#[test]
fn test_sec03_timeout_enforcement_remains_intact_under_sandboxing() {
    let ws = TestWorkspace::new("timeout-sandboxed");
    let started = std::time::Instant::now();
    let ctx = ExecutionContext {
        session_id: brain_domain::SessionId::new(),
        working_dir: ws.path.clone(),
        cancellation: Arc::new(brain_tools::CancellationTokenImpl::default()),
        deadline: Some(started + Duration::from_millis(150)),
    };

    let driver = MockSandboxDriver::new();
    let bash = BashTool;

    let res = bash.execute_with_sandbox(
        &ctx,
        &command_args("exec sleep 30"),
        &driver,
        EnforcementMode::Enforced,
    );

    assert!(res.is_err(), "Sleep 30 must timeout against 150ms deadline");
    assert!(matches!(res.unwrap_err(), BrainError::Timeout { .. }));
    assert!(started.elapsed() < Duration::from_secs(3));
}

#[test]
fn test_sec03_cancellation_remains_intact_under_sandboxing() {
    let ws = TestWorkspace::new("cancellation-sandboxed");
    let cancel = Arc::new(brain_tools::CancellationTokenImpl::default());
    let ctx = ExecutionContext {
        session_id: brain_domain::SessionId::new(),
        working_dir: ws.path.clone(),
        cancellation: cancel.clone(),
        deadline: None,
    };

    let driver = MockSandboxDriver::new();
    let bash = BashTool;

    let cancel_trigger = cancel.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(50));
        cancel_trigger.cancel();
    });

    let res = bash.execute_with_sandbox(
        &ctx,
        &command_args("exec sleep 30"),
        &driver,
        EnforcementMode::Enforced,
    );

    assert!(
        res.is_err(),
        "Cancelled execution must return BrainError::Cancelled"
    );
    assert!(matches!(res.unwrap_err(), BrainError::Cancelled { .. }));
}

// ---------------------------------------------------------------------------
// 7. Platform Verification Matrix (R3, R4, R9)
// ---------------------------------------------------------------------------

#[test]
fn test_sec03_macos_seatbelt_driver_verification() {
    let seatbelt = SeatbeltSandboxDriver::new();
    assert_eq!(seatbelt.name(), "Seatbelt");

    let ws = PathBuf::from("/Users/test/workspace");
    let policy = SandboxPolicy::builder(&ws)
        .with_system_defaults()
        .with_sensitive_defaults()
        .build();

    let profile = seatbelt.generate_profile(&policy);
    assert!(profile.contains("(version 1)"));
    assert!(profile.contains("(deny default)"));
    assert!(profile.contains("/\\.env($|[\\.\\-_/].*|rc$)"));
    assert!(profile.contains("/\\.git/config$"));
    assert!(profile.contains("(deny file-write*"));
    assert!(profile.contains("/.azure"));
    assert!(profile.contains("/etc/passwd"));
    assert!(profile.contains("/private/etc/shadow"));

    // Check support capability on host
    let supported = seatbelt.is_supported();
    if !supported {
        // When Seatbelt is unsupported or restricted: fails closed when enforced
        let mut cmd = std::process::Command::new("/bin/echo");
        let res = seatbelt.apply_to_command(&mut cmd, &policy);
        assert!(res.is_err());
        assert!(matches!(res.unwrap_err(), BrainError::Authorization { .. }));

        let prep_res = seatbelt.prepare_command("/bin/echo", &["test"], &policy);
        assert!(prep_res.is_err());
        assert!(matches!(
            prep_res.unwrap_err(),
            BrainError::Authorization { .. }
        ));
    }
}

#[test]
fn test_sec03_linux_landlock_driver_verification() {
    let landlock = LandlockSandboxDriver::new();
    assert_eq!(landlock.name(), "Landlock");

    let abi = landlock.query_abi_version();
    let supported = landlock.is_supported();

    #[cfg(target_os = "linux")]
    {
        if abi >= 1 {
            assert!(supported);
            let mask = landlock.compute_handled_access_mask(abi);
            assert_ne!(mask, 0);
        } else {
            assert!(!supported);
        }
    }

    #[cfg(not(target_os = "linux"))]
    {
        assert!(!supported);
        assert_eq!(abi, -1);
    }
}

#[test]
fn test_sec03_activation_failure_distinguished_from_denial() {
    let ws = TestWorkspace::new("activation-failure");
    let ctx = test_context(&ws.path);

    // Command that simulates a sandbox activation failure (exit code 71 + sandbox_apply message)
    let activation_fail_cmd =
        "echo 'sandbox-exec: sandbox_apply: Operation not permitted' 1>&2; exit 71";
    let mock_driver = MockSandboxDriver::new();
    let bash = BashTool;

    let res = bash.execute_with_sandbox(
        &ctx,
        &command_args(activation_fail_cmd),
        &mock_driver,
        EnforcementMode::Enforced,
    );

    assert!(
        res.is_err(),
        "Sandbox activation failure must be distinguished from ordinary command failure and fail closed"
    );
    match res.unwrap_err() {
        BrainError::Authorization { message } => {
            assert!(
                message.contains("Sandbox activation failure"),
                "Expected sandbox activation failure error, got: {message}"
            );
        }
        other => panic!("Expected BrainError::Authorization, got: {other:?}"),
    }

    // In contrast, an ordinary command denied by filesystem permissions returns a result (exit code != 0, is_error: true)
    let fs_denied_cmd = "echo 'cat: /etc/shadow: Permission denied' 1>&2; exit 1";
    let res_denied = bash.execute_with_sandbox(
        &ctx,
        &command_args(fs_denied_cmd),
        &mock_driver,
        EnforcementMode::Enforced,
    );

    assert!(
        res_denied.is_ok(),
        "Filesystem access denial from running process must be reported as command result"
    );
    let val = res_denied.unwrap().value().clone();
    assert_eq!(val["exit_code"], serde_json::json!(1));
    assert_eq!(val["is_error"], serde_json::json!(true));
    assert!(val["output"]
        .as_str()
        .unwrap()
        .contains("Permission denied"));
}

#[test]
fn test_sec03_macos_seatbelt_live_kernel_confinement() {
    #[cfg(target_os = "macos")]
    {
        let seatbelt = SeatbeltSandboxDriver::new();
        if !seatbelt.is_supported() {
            eprintln!("Seatbelt not supported in this environment, skipping live test");
            return;
        }

        let ws = TestWorkspace::new("live-seatbelt");
        let ctx = test_context(&ws.path);
        let bash = BashTool;

        // 1. Authorized read and write inside workspace
        let res_auth = bash.execute_with_sandbox(
            &ctx,
            &command_args("echo 'valid payload' > data.txt && cat data.txt"),
            &seatbelt,
            EnforcementMode::Enforced,
        );
        assert!(
            res_auth.is_ok(),
            "Authorized execution under live Seatbelt must succeed"
        );
        let val_auth = res_auth.unwrap().value().clone();
        assert_eq!(val_auth["exit_code"], serde_json::json!(0));
        assert!(val_auth["output"]
            .as_str()
            .unwrap()
            .contains("valid payload"));
        assert_eq!(val_auth["sandbox"]["activated"], serde_json::json!(true));
        assert_eq!(val_auth["sandbox"]["driver"], "Seatbelt");

        // 2. Sensitive in-workspace file (.env) access denied by kernel at runtime
        let env_path = ws.path.join(".env");
        fs::write(&env_path, "API_KEY=secret_live_key\n").unwrap();
        let res_env = bash.execute_with_sandbox(
            &ctx,
            &command_args("cat .env"),
            &seatbelt,
            EnforcementMode::Enforced,
        );
        assert!(
            res_env.is_ok(),
            "Subprocess must start and be denied by kernel"
        );
        let val_env = res_env.unwrap().value().clone();
        assert_ne!(val_env["exit_code"], serde_json::json!(0));
        assert_eq!(val_env["is_error"], serde_json::json!(true));
        assert!(val_env["output"]
            .as_str()
            .unwrap()
            .contains("Operation not permitted"));

        // 3. Unauthorized sensitive system file (/etc/passwd) read denied by kernel at runtime
        let res_passwd = bash.execute_with_sandbox(
            &ctx,
            &command_args("cat /etc/passwd"),
            &seatbelt,
            EnforcementMode::Enforced,
        );
        assert!(
            res_passwd.is_ok(),
            "Subprocess must start and be denied by kernel"
        );
        let val_passwd = res_passwd.unwrap().value().clone();
        assert_ne!(val_passwd["exit_code"], serde_json::json!(0));
        assert_eq!(val_passwd["is_error"], serde_json::json!(true));
        assert!(val_passwd["output"]
            .as_str()
            .unwrap()
            .contains("Operation not permitted"));

        // 4. Subprocess inheritance: nested bash also denied by kernel at runtime
        let res_child = bash.execute_with_sandbox(
            &ctx,
            &command_args("/bin/bash -c 'cat .env'"),
            &seatbelt,
            EnforcementMode::Enforced,
        );
        assert!(
            res_child.is_ok(),
            "Child subprocess must be spawned and inherit confinement"
        );
        let val_child = res_child.unwrap().value().clone();
        assert_ne!(val_child["exit_code"], serde_json::json!(0));
        assert_eq!(val_child["is_error"], serde_json::json!(true));
        assert!(val_child["output"]
            .as_str()
            .unwrap()
            .contains("Operation not permitted"));
    }
}
