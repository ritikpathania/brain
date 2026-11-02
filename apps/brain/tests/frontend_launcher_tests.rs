#![cfg(unix)]

use std::fs;
use std::io::Read;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::UnixListener;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

struct TemporaryDirectory {
    path: PathBuf,
}

impl TemporaryDirectory {
    fn new(label: &str) -> Self {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock after unix epoch")
            .as_nanos();
        let path =
            std::env::temp_dir().join(format!("brain-{label}-{}-{unique}", std::process::id()));
        fs::create_dir_all(&path).expect("create temporary directory");
        Self { path }
    }
}

impl Drop for TemporaryDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

struct LiveSocket {
    path: PathBuf,
    stop: Arc<AtomicBool>,
    thread: Option<thread::JoinHandle<()>>,
}

impl LiveSocket {
    fn start(path: PathBuf) -> Self {
        let listener = UnixListener::bind(&path).expect("bind test UDS socket");
        listener
            .set_nonblocking(true)
            .expect("set test UDS listener non-blocking");
        let stop = Arc::new(AtomicBool::new(false));
        let thread_stop = Arc::clone(&stop);
        let thread = thread::spawn(move || {
            while !thread_stop.load(Ordering::Acquire) {
                match listener.accept() {
                    Ok((mut connection, _)) => {
                        let mut ignored = [0_u8; 128];
                        let _ = connection.read(&mut ignored);
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(5));
                    }
                    Err(_) => break,
                }
            }
        });
        Self {
            path,
            stop,
            thread: Some(thread),
        }
    }
}

impl Drop for LiveSocket {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
        let _ = fs::remove_file(&self.path);
    }
}

fn write_frontend_fixture(temp: &TemporaryDirectory) -> (PathBuf, PathBuf, PathBuf) {
    let root = temp.path.join("canonical-frontend");
    fs::create_dir_all(root.join("src/cli")).expect("create fixture frontend source tree");
    fs::write(
        root.join("package.json"),
        r#"{"name":"@brain/frontend","version":"9.8.7","scripts":{"start":"bun run src/cli/main.ts"}}"#,
    )
    .expect("write fixture frontend manifest");
    fs::write(root.join("src/cli/main.ts"), "// fixture entrypoint\n")
        .expect("write fixture frontend entrypoint");

    let record = temp.path.join("launch-record.txt");
    let runtime = temp.path.join("fixture-bun");
    fs::write(
        &runtime,
        "#!/bin/sh\nif [ \"$1\" = \"--version\" ]; then\n  exit 0\nfi\n{\n  printf 'cwd=%s\\n' \"$PWD\"\n  printf 'mode=%s\\n' \"$BRAIN_MODE\"\n  printf 'config=%s\\n' \"$BRAIN_CONFIG_DIR\"\n  printf 'socket=%s\\n' \"$BRAIN_SOCKET_PATH\"\n  printf 'version=%s\\n' \"$BRAIN_FRONTEND_VERSION\"\n  printf 'bun=%s\\n' \"$BUN_BE_BUN\"\n  printf 'args=%s\\n' \"$*\"\n} > \"$BRAIN_FIXTURE_RECORD\"\nexit \"${BRAIN_FIXTURE_EXIT:-0}\"\n",
    )
    .expect("write fixture Bun runtime");
    let mut permissions = fs::metadata(&runtime)
        .expect("read fixture runtime permissions")
        .permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&runtime, permissions).expect("make fixture runtime executable");

    (root, runtime, record)
}

fn brain_ui(
    socket: &Path,
    frontend_root: Option<&Path>,
    runtime: Option<&Path>,
    record: Option<&Path>,
    child_exit: Option<i32>,
) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_brain"));
    let cwd = TemporaryDirectory::new("launcher-cwd");
    command
        .current_dir(&cwd.path)
        .arg("--socket-path")
        .arg(socket)
        .arg("ui")
        .env_remove("BRAIN_FRONTEND_ROOT")
        .env_remove("BRAIN_FRONTEND_RUNTIME")
        .env_remove("BRAIN_FIXTURE_RECORD")
        .env_remove("BRAIN_FIXTURE_EXIT")
        .env_remove("BRAIN_MODE")
        .env("BRAIN_CONFIG_DIR", &cwd.path);
    if let Some(frontend_root) = frontend_root {
        command.env("BRAIN_FRONTEND_ROOT", frontend_root);
    }
    if let Some(runtime) = runtime {
        command.env("BRAIN_FRONTEND_RUNTIME", runtime);
    }
    if let Some(record) = record {
        command.env("BRAIN_FIXTURE_RECORD", record);
    }
    if let Some(child_exit) = child_exit {
        command.env("BRAIN_FIXTURE_EXIT", child_exit.to_string());
    }
    command.output().expect("run brain ui")
}

fn combined_output(output: &Output) -> String {
    format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

fn test_ui_socket(suffix: impl std::fmt::Display) -> PathBuf {
    std::env::temp_dir().join(format!("bui-{}-{suffix}.sock", std::process::id()))
}

#[test]
fn configured_canonical_frontend_is_launched_from_its_own_root_with_explicit_brain_environment() {
    let temp = TemporaryDirectory::new("frontend-launcher-success");
    let (root, runtime, record) = write_frontend_fixture(&temp);
    let socket = LiveSocket::start(test_ui_socket(1));

    let output = brain_ui(
        &socket.path,
        Some(&root),
        Some(&runtime),
        Some(&record),
        None,
    );

    assert!(output.status.success(), "{}", combined_output(&output));
    let launch_record = fs::read_to_string(record).expect("fixture frontend was launched");
    let canonical_root = root
        .canonicalize()
        .expect("canonical frontend fixture root");
    assert!(
        launch_record.contains(&format!("cwd={}", canonical_root.display())),
        "{launch_record}"
    );
    assert!(launch_record.contains("mode=\n"), "{launch_record}");
    assert!(launch_record.contains("config="), "{launch_record}");
    assert!(
        launch_record.contains(&format!("socket={}", socket.path.display())),
        "{launch_record}"
    );
    assert!(launch_record.contains("version=9.8.7"), "{launch_record}");
    assert!(launch_record.contains("args=run start"), "{launch_record}");
}

#[test]
fn default_frontend_is_selected_without_override_from_an_unrelated_cwd() {
    let temp = TemporaryDirectory::new("frontend-launcher-default");
    let (_, runtime, record) = write_frontend_fixture(&temp);
    let socket = LiveSocket::start(test_ui_socket(3));
    let output = brain_ui(&socket.path, None, Some(&runtime), Some(&record), None);
    assert!(output.status.success(), "{}", combined_output(&output));
    let launch_record = fs::read_to_string(record).expect("default was launched");
    let expected = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../packages/brain-frontend")
        .canonicalize()
        .unwrap();
    assert!(
        launch_record.contains(&format!("cwd={}", expected.display())),
        "{launch_record}"
    );
    assert!(launch_record.contains("version=0.1.0"), "{launch_record}");
}

#[test]
fn frontend_root_without_canonical_entrypoint_is_actionable_and_never_falls_back_to_legacy_shell() {
    let temp = TemporaryDirectory::new("frontend-launcher-missing-entrypoint");
    let root = temp.path.join("canonical-frontend");
    fs::create_dir_all(&root).expect("create fixture root");
    fs::write(root.join("package.json"), r#"{"name":"@brain/frontend","version":"1.0","scripts":{"start":"bun run src/cli/main.ts"}}"#).expect("write fixture manifest");

    let output = brain_ui(
        &temp.path.join("unused.sock"),
        Some(&root),
        None,
        None,
        None,
    );

    assert!(!output.status.success(), "{}", combined_output(&output));
    let text = combined_output(&output);
    assert!(text.contains("src/cli/main.ts"), "{text}");
    assert!(!text.contains("packages/brain-shell"), "{text}");
}

#[test]
fn invalid_override_manifest_fails_instead_of_using_valid_default() {
    let temp = TemporaryDirectory::new("frontend-launcher-invalid-manifest");
    let (root, runtime, record) = write_frontend_fixture(&temp);
    for manifest in [
        "not json",
        r#"{"name":"other-frontend","version":"1","scripts":{"start":"bun run src/cli/main.ts"}}"#,
        r#"{"name":"@brain/frontend","version":"","scripts":{"start":"bun run src/cli/main.ts"}}"#,
        r#"{"name":"@brain/frontend","version":"1","scripts":{"start":"bun run other.ts"}}"#,
    ] {
        fs::write(root.join("package.json"), manifest).unwrap();
        let output = brain_ui(
            &temp.path.join("unused.sock"),
            Some(&root),
            Some(&runtime),
            Some(&record),
            None,
        );
        assert!(!output.status.success(), "{}", combined_output(&output));
        assert!(
            combined_output(&output).contains("package.json"),
            "{}",
            combined_output(&output)
        );
        assert!(!record.exists(), "invalid override launched a fallback");
    }
}

#[test]
fn missing_override_path_is_not_a_request_for_default_resolution() {
    let temp = TemporaryDirectory::new("frontend-launcher-invalid-path");
    let (_, runtime, record) = write_frontend_fixture(&temp);
    let output = brain_ui(
        &temp.path.join("unused.sock"),
        Some(&temp.path.join("missing")),
        Some(&runtime),
        Some(&record),
        None,
    );
    assert!(!output.status.success());
    assert!(combined_output(&output).contains("BRAIN_FRONTEND_ROOT"));
    assert!(!record.exists());
}

#[test]
fn copied_cli_outside_repository_fails_clearly_without_searching_host_frontends() {
    let temp = TemporaryDirectory::new("frontend-launcher-no-repo");
    let cli = temp.path.join("brain");
    fs::copy(env!("CARGO_BIN_EXE_brain"), &cli).unwrap();
    let output = Command::new(cli)
        .arg("ui")
        .current_dir(&temp.path)
        .env_remove("BRAIN_FRONTEND_ROOT")
        .env_remove("BRAIN_FRONTEND_RUNTIME")
        .env("HOME", &temp.path)
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(
        combined_output(&output).contains("development frontend"),
        "{}",
        combined_output(&output)
    );
}

#[test]
fn unavailable_daemon_prevents_frontend_launch() {
    let temp = TemporaryDirectory::new("frontend-launcher-no-daemon");
    let (root, runtime, record) = write_frontend_fixture(&temp);
    let cli = temp.path.join("brain");
    fs::copy(env!("CARGO_BIN_EXE_brain"), &cli).unwrap();
    let output = Command::new(cli)
        .arg("--socket-path")
        .arg(temp.path.join("absent.sock"))
        .arg("ui")
        .env("BRAIN_FRONTEND_ROOT", root)
        .env("BRAIN_FRONTEND_RUNTIME", runtime)
        .env("BRAIN_FIXTURE_RECORD", &record)
        .env("BRAIN_CONFIG_DIR", &temp.path)
        .env("PATH", "")
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(
        combined_output(&output).contains("daemon is unavailable"),
        "{}",
        combined_output(&output)
    );
    assert!(!record.exists());
}

#[test]
fn empty_explicit_override_is_invalid_not_an_implicit_default() {
    let temp = TemporaryDirectory::new("frontend-launcher-empty-root");
    let (_, runtime, record) = write_frontend_fixture(&temp);
    let output = Command::new(env!("CARGO_BIN_EXE_brain"))
        .arg("ui")
        .env("BRAIN_FRONTEND_ROOT", "")
        .env("BRAIN_FRONTEND_RUNTIME", runtime)
        .env("BRAIN_FIXTURE_RECORD", &record)
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(
        combined_output(&output).contains("must not be empty"),
        "{}",
        combined_output(&output)
    );
    assert!(!record.exists());
}

#[test]
fn absent_bun_does_not_select_another_runtime() {
    let temp = TemporaryDirectory::new("frontend-launcher-no-bun");
    let (root, _, record) = write_frontend_fixture(&temp);
    let output = Command::new(env!("CARGO_BIN_EXE_brain"))
        .arg("ui")
        .env("BRAIN_FRONTEND_ROOT", root)
        .env_remove("BRAIN_FRONTEND_RUNTIME")
        .env("PATH", "")
        .env("BRAIN_FIXTURE_RECORD", &record)
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(
        combined_output(&output).contains("Bun"),
        "{}",
        combined_output(&output)
    );
    assert!(!record.exists());
}

#[test]
fn relative_runtime_is_resolved_before_frontend_changes_working_directory() {
    let temp = TemporaryDirectory::new("frontend-launcher-relative-bun");
    let (root, _, record) = write_frontend_fixture(&temp);
    let socket = LiveSocket::start(test_ui_socket(4));
    let output = Command::new(env!("CARGO_BIN_EXE_brain"))
        .current_dir(&temp.path)
        .arg("--socket-path")
        .arg(&socket.path)
        .arg("ui")
        .env("BRAIN_FRONTEND_ROOT", root)
        .env("BRAIN_FRONTEND_RUNTIME", "./fixture-bun")
        .env("BRAIN_FIXTURE_RECORD", &record)
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", combined_output(&output));
    assert!(record.exists());
}

#[test]
fn relative_socket_remains_the_preflight_socket_after_frontend_cwd_changes() {
    let temp = TemporaryDirectory::new("frontend-launcher-relative-socket");
    let (root, runtime, record) = write_frontend_fixture(&temp);
    let socket = LiveSocket::start(test_ui_socket(5));
    let output = Command::new(env!("CARGO_BIN_EXE_brain"))
        .current_dir(std::env::temp_dir())
        .arg("--socket-path")
        .arg(socket.path.file_name().unwrap())
        .arg("ui")
        .env("BRAIN_FRONTEND_ROOT", root)
        .env("BRAIN_FRONTEND_RUNTIME", runtime)
        .env("BRAIN_FIXTURE_RECORD", &record)
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", combined_output(&output));
    let launch_record = fs::read_to_string(record).unwrap();
    assert!(
        launch_record.contains(&format!(
            "socket={}",
            socket.path.canonicalize().unwrap().display()
        )),
        "{launch_record}"
    );
}

#[test]
fn unavailable_configured_runtime_is_actionable_and_nonzero() {
    let temp = TemporaryDirectory::new("frontend-launcher-missing-runtime");
    let (root, _runtime, _record) = write_frontend_fixture(&temp);
    let missing_runtime = temp.path.join("not-a-runtime");

    let output = brain_ui(
        &temp.path.join("unused.sock"),
        Some(&root),
        Some(&missing_runtime),
        None,
        None,
    );

    assert!(!output.status.success(), "{}", combined_output(&output));
    assert!(
        combined_output(&output).contains("BRAIN_FRONTEND_RUNTIME"),
        "{}",
        combined_output(&output)
    );
}

#[test]
fn canonical_frontend_nonzero_exit_is_propagated() {
    let temp = TemporaryDirectory::new("frontend-launcher-child-failure");
    let (root, runtime, record) = write_frontend_fixture(&temp);
    let socket = LiveSocket::start(test_ui_socket(2));

    let output = brain_ui(
        &socket.path,
        Some(&root),
        Some(&runtime),
        Some(&record),
        Some(23),
    );

    assert_eq!(
        output.status.code(),
        Some(23),
        "{}",
        combined_output(&output)
    );
    assert!(record.exists(), "fixture frontend was not launched");
}

// Independently authored artifact fixtures, not packaged application source.
fn fixture_sha256(path: &Path) -> String {
    let output = Command::new("shasum")
        .args(["-a", "256"])
        .arg(path)
        .output()
        .expect("fixture SHA-256 tool");
    assert!(output.status.success());
    String::from_utf8(output.stdout)
        .unwrap()
        .split_whitespace()
        .next()
        .unwrap()
        .to_owned()
}

fn fixture_json(path: &Path, value: serde_json::Value) {
    fs::write(path, serde_json::to_vec(&value).unwrap()).unwrap();
}

fn write_packaged_fixture(temp: &TemporaryDirectory) -> PathBuf {
    let root = &temp.path;
    fs::create_dir_all(root.join("bin")).unwrap();
    fs::create_dir_all(root.join("frontend/brain-frontend")).unwrap();
    fs::create_dir_all(root.join("runtime/python/lib")).unwrap();
    fs::create_dir_all(root.join("licenses")).unwrap();
    let cli = root.join("bin/brain");
    fs::copy(env!("CARGO_BIN_EXE_brain"), &cli).unwrap();
    let target = if cfg!(target_os = "macos") {
        "aarch64-apple-darwin"
    } else {
        "x86_64-unknown-linux-gnu"
    };
    let source = serde_json::json!({"sha": "a".repeat(40), "dirty": false,
        "fingerprint_algorithm": "brain-source-tree-v1",
        "fingerprint_sha256": "b".repeat(64), "untracked_input_count": 0});
    let identity = serde_json::json!({"schema_version": 1,
        "identity": "brain-frontend", "package": "@brain/frontend",
        "frontend_version": "0.1.0", "brain_version": env!("CARGO_PKG_VERSION"),
        "source": source, "target_triple": target, "runtime_model": "bun-standalone",
        "bun_version": "1.4.4", "lockfile_sha256": "c".repeat(64)});
    let executable = root.join("frontend/brain-frontend/brain-frontend");
    fs::write(&executable, format!(
        "#!/bin/sh\nif [ \"$1\" = --build-identity ]; then printf '%s\\n' '{}'; exit 0; fi\n{{\nprintf 'cwd=%s\\n' \"$PWD\"\nprintf 'socket=%s\\n' \"$BRAIN_SOCKET_PATH\"\nprintf 'args=%s\\n' \"$*\"\nprintf 'bun=%s\\n' \"$BUN_BE_BUN\"\nprintf 'bun-options=%s\\n' \"$BUN_OPTIONS\"\nprintf 'node-options=%s\\n' \"$NODE_OPTIONS\"\n}} > \"$BRAIN_FIXTURE_RECORD\"\nexit \"${{BRAIN_FIXTURE_EXIT:-0}}\"\n",
        identity
    )).unwrap();
    fs::set_permissions(&executable, fs::Permissions::from_mode(0o755)).unwrap();
    let mut frontend = identity.as_object().unwrap().clone();
    frontend.insert("entrypoint".into(), serde_json::json!("brain-frontend"));
    frontend.insert(
        "executable_sha256".into(),
        serde_json::json!(fixture_sha256(&executable)),
    );
    let frontend_manifest = root.join("frontend/brain-frontend/manifest.json");
    fixture_json(&frontend_manifest, frontend.into());
    let daemon = root.join("bin/brain-daemon");
    fs::write(&daemon, "fixture daemon bytes").unwrap();
    let library = root.join("runtime/python/lib/fixture-python");
    fs::write(&library, "fixture library bytes").unwrap();
    let python_files = root.join("runtime/python-files.json");
    fixture_json(
        &python_files,
        serde_json::json!({"schema_version": 1, "files": [
        {"path":"lib/fixture-python", "type":"file", "sha256":fixture_sha256(&library)}]}),
    );
    let daemon_manifest = root.join("runtime-manifest.json");
    fixture_json(
        &daemon_manifest,
        serde_json::json!({"schema_version": 1,
        "artifact":"brain-daemon-runtime", "brain_version":env!("CARGO_PKG_VERSION"),
        "daemon_source_sha":"a".repeat(40), "source_tree":{"dirty":false,"untracked_file_count":0},
        "target_triple":target, "rust_version":"fixture",
        "python":{"version":"3.15.2","home":"runtime/python","library":"lib/fixture-python",
            "library_sha256":fixture_sha256(&library)}, "daemon_sha256":fixture_sha256(&daemon),
        "runtime_files_manifest":"runtime/python-files.json",
        "contents":["bin/brain-daemon","runtime/python","runtime-manifest.json","runtime/python-files.json"]}),
    );
    let notice = root.join("licenses/fixture.txt");
    fs::write(&notice, "owned fixture notice").unwrap();
    fixture_json(
        &root.join("product-manifest.json"),
        serde_json::json!({
        "schema_version":1, "artifact":"brain-product", "brain_version":env!("CARGO_PKG_VERSION"),
        "source":source, "target_triple":target,
        "cli":{"path":"bin/brain","sha256":fixture_sha256(&cli)},
        "frontend":{"manifest":"frontend/brain-frontend/manifest.json","sha256":fixture_sha256(&frontend_manifest)},
        "daemon":{"manifest":"runtime-manifest.json","sha256":fixture_sha256(&daemon_manifest)},
        "python_files":{"manifest":"runtime/python-files.json","sha256":fixture_sha256(&python_files)},
        "licenses":[{"path":"licenses/fixture.txt","sha256":fixture_sha256(&notice)}],
        "build_tools":{"rustc":"fixture","cargo":"fixture","bun":"1.4.4","python":"3.15.2"},
        "contents":["bin/brain","bin/brain-daemon","frontend/brain-frontend/brain-frontend",
          "frontend/brain-frontend/manifest.json","licenses/fixture.txt","product-manifest.json",
          "runtime-manifest.json","runtime/python-files.json","runtime/python/lib/fixture-python"]}),
    );
    cli
}

#[test]
fn packaged_frontend_launches_without_bun_and_sanitizes_runtime_overrides() {
    let temp = TemporaryDirectory::new("packaged-launch");
    let cli = write_packaged_fixture(&temp);
    let socket = LiveSocket::start(test_ui_socket(6));
    let record = temp.path.join("packaged-record");
    let output = Command::new(cli)
        .args(["--socket-path", socket.path.to_str().unwrap(), "ui"])
        .current_dir(std::env::temp_dir())
        .env_remove("BRAIN_FRONTEND_ROOT")
        .env("PATH", "")
        .env("BRAIN_FRONTEND_RUNTIME", "/nonexistent/runtime")
        .env("BRAIN_CONFIG_DIR", &temp.path)
        .env("BRAIN_FIXTURE_RECORD", &record)
        .env("BRAIN_FIXTURE_EXIT", "23")
        .env("BUN_BE_BUN", "1")
        .env("BUN_OPTIONS", "--preload /nonexistent/preload")
        .env("NODE_OPTIONS", "--require /nonexistent/preload")
        .output()
        .unwrap();
    assert_eq!(
        output.status.code(),
        Some(23),
        "{}",
        combined_output(&output)
    );
    let record = fs::read_to_string(record).expect("packaged frontend actually ran");
    assert!(record.contains("args=\n"), "{record}");
    assert!(record.contains("bun=\n"), "{record}");
    assert!(record.contains("bun-options=\n"), "{record}");
    assert!(record.contains("node-options=\n"), "{record}");
    assert!(record.contains(&format!("socket={}\n", socket.path.display())));
}

#[test]
fn partial_packaged_installation_inside_repo_must_not_fall_back_to_source() {
    let temp = TemporaryDirectory::new("packaged-partial");
    let (development, runtime, record) = write_frontend_fixture(&temp);
    fs::create_dir_all(temp.path.join("packages")).unwrap();
    fs::rename(development, temp.path.join("packages/brain-frontend")).unwrap();
    fs::create_dir_all(temp.path.join("apps/brain")).unwrap();
    fs::write(temp.path.join("Cargo.toml"), "[workspace]\n").unwrap();
    fs::write(temp.path.join("apps/brain/Cargo.toml"), "[package]\n").unwrap();
    fs::create_dir_all(temp.path.join("bin")).unwrap();
    let cli = temp.path.join("bin/brain");
    fs::copy(env!("CARGO_BIN_EXE_brain"), &cli).unwrap();
    fs::write(temp.path.join("product-manifest.json"), "{}").unwrap();
    let socket = LiveSocket::start(test_ui_socket(7));
    let output = Command::new(cli)
        .args(["--socket-path", socket.path.to_str().unwrap(), "ui"])
        .env_remove("BRAIN_FRONTEND_ROOT")
        .env("BRAIN_FRONTEND_RUNTIME", runtime)
        .env("BRAIN_CONFIG_DIR", &temp.path)
        .env("BRAIN_FIXTURE_RECORD", &record)
        .output()
        .unwrap();
    assert!(
        !output.status.success(),
        "invalid installed artifact used source fallback"
    );
    assert!(
        combined_output(&output).contains("packaged"),
        "{}",
        combined_output(&output)
    );
    assert!(!record.exists());
}

fn refresh_component_hash(root: &Path, component: &str, file: &str) {
    let manifest = root.join("product-manifest.json");
    let mut product: serde_json::Value =
        serde_json::from_slice(&fs::read(&manifest).unwrap()).unwrap();
    product[component]["sha256"] = fixture_sha256(&root.join(file)).into();
    fixture_json(&manifest, product);
}

fn change_frontend_manifest(root: &Path, change: impl FnOnce(&mut serde_json::Value)) {
    let path = root.join("frontend/brain-frontend/manifest.json");
    let mut frontend: serde_json::Value =
        serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    change(&mut frontend);
    fixture_json(&path, frontend);
    refresh_component_hash(root, "frontend", "frontend/brain-frontend/manifest.json");
}

fn packaged_ui(cli: &Path, root: &Path) -> Output {
    Command::new(cli)
        .arg("ui")
        .env_remove("BRAIN_FRONTEND_ROOT")
        .env("BRAIN_CONFIG_DIR", root.join("config"))
        .env("BRAIN_SOCKET_PATH", root.join("absent.sock"))
        .output()
        .unwrap()
}

#[test]
fn packaged_manifest_defects_fail_before_preflight_without_source_fallback() {
    for case in [
        "identity",
        "target",
        "version",
        "digest",
        "traversal",
        "unknown",
        "compiled",
        "missing",
        "mode",
        "cli",
        "daemon",
        "library",
        "python-inventory",
        "symlink",
    ] {
        let temp = TemporaryDirectory::new(&format!("packaged-invalid-{case}"));
        let cli = write_packaged_fixture(&temp);
        match case {
            "identity" => change_frontend_manifest(&temp.path, |f| f["identity"] = "other".into()),
            "target" => {
                change_frontend_manifest(&temp.path, |f| f["target_triple"] = "wrong-target".into())
            }
            "version" => {
                change_frontend_manifest(&temp.path, |f| f["brain_version"] = "99.0.0".into())
            }
            "digest" => change_frontend_manifest(&temp.path, |f| {
                f["executable_sha256"] = "d".repeat(64).into()
            }),
            "traversal" => change_frontend_manifest(&temp.path, |f| {
                f["entrypoint"] = "../../../bin/brain".into()
            }),
            "unknown" => change_frontend_manifest(&temp.path, |f| f["extra"] = true.into()),
            "compiled" => {
                change_frontend_manifest(&temp.path, |f| f["frontend_version"] = "0.2.0".into())
            }
            "missing" => {
                fs::remove_file(temp.path.join("frontend/brain-frontend/brain-frontend")).unwrap()
            }
            "mode" => fs::set_permissions(
                temp.path.join("frontend/brain-frontend/brain-frontend"),
                fs::Permissions::from_mode(0o644),
            )
            .unwrap(),
            "cli" => {
                let path = temp.path.join("product-manifest.json");
                let mut m: serde_json::Value =
                    serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
                m["cli"]["sha256"] = "d".repeat(64).into();
                fixture_json(&path, m);
            }
            "daemon" => fs::write(temp.path.join("bin/brain-daemon"), "tampered").unwrap(),
            "library" => fs::write(
                temp.path.join("runtime/python/lib/fixture-python"),
                "tampered",
            )
            .unwrap(),
            "python-inventory" => {
                fs::write(temp.path.join("runtime/python-files.json"), "{}").unwrap()
            }
            "symlink" => {
                let entry = temp.path.join("frontend/brain-frontend/brain-frontend");
                let outside = temp.path.join("outside-entry");
                fs::rename(&entry, &outside).unwrap();
                std::os::unix::fs::symlink(&outside, &entry).unwrap();
            }
            _ => unreachable!(),
        }
        let output = packaged_ui(&cli, &temp.path);
        let text = combined_output(&output);
        assert!(!output.status.success(), "{case}: {text}");
        assert!(text.contains("packaged"), "{case}: {text}");
        assert!(
            !text.contains("daemon is unavailable"),
            "validation ran after preflight: {case}: {text}"
        );
    }
}

#[test]
fn packaged_identity_probe_is_bounded_and_reaps_its_process_group() {
    for (label, script, expected) in [
        ("hang", "while :; do :; done", "deadline"),
        (
            "overflow",
            "while :; do printf 'oversized diagnostic output'; done",
            "output limit",
        ),
        ("wrong", "printf '{}\\n'", "identity"),
    ] {
        let temp = TemporaryDirectory::new(label);
        let cli = write_packaged_fixture(&temp);
        let entry = temp.path.join("frontend/brain-frontend/brain-frontend");
        let pid_file = temp.path.join("probe.pid");
        fs::write(
            &entry,
            format!(
                "#!/bin/sh\nprintf '%s' \"$$\" > '{}'\n{script}\n",
                pid_file.display()
            ),
        )
        .unwrap();
        change_frontend_manifest(&temp.path, |f| {
            f["executable_sha256"] = fixture_sha256(&entry).into()
        });
        let start = std::time::Instant::now();
        let output = packaged_ui(&cli, &temp.path);
        assert!(!output.status.success());
        assert!(
            start.elapsed() < Duration::from_secs(7),
            "probe was not bounded"
        );
        assert!(
            combined_output(&output).contains(expected),
            "{}",
            combined_output(&output)
        );
        let pid = fs::read_to_string(pid_file).expect("diagnostic actually ran");
        assert!(
            !Command::new("/bin/kill")
                .args(["-0", &pid])
                .status()
                .unwrap()
                .success(),
            "probe survived"
        );
    }
}

#[test]
fn explicit_development_override_still_wins_over_packaged_installation() {
    let temp = TemporaryDirectory::new("packaged-override");
    let cli = write_packaged_fixture(&temp);
    let (root, runtime, record) = write_frontend_fixture(&temp);
    let socket = LiveSocket::start(test_ui_socket(8));
    let output = Command::new(&cli)
        .args(["--socket-path", socket.path.to_str().unwrap(), "ui"])
        .env("BRAIN_FRONTEND_ROOT", &root)
        .env("BRAIN_FRONTEND_RUNTIME", &runtime)
        .env("BRAIN_FIXTURE_RECORD", &record)
        .env("BUN_BE_BUN", "development-env-preserved")
        .env("BRAIN_CONFIG_DIR", &temp.path)
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", combined_output(&output));
    assert!(fs::read_to_string(&record)
        .unwrap()
        .contains("args=run start"));
    assert!(fs::read_to_string(&record)
        .unwrap()
        .contains("bun=development-env-preserved"));
    fs::remove_file(root.join("package.json")).unwrap();
    let invalid = Command::new(&cli)
        .arg("ui")
        .env("BRAIN_FRONTEND_ROOT", &root)
        .output()
        .unwrap();
    assert!(!invalid.status.success());
    assert!(combined_output(&invalid).contains("package.json"));
}

#[test]
fn installed_cli_symlink_keeps_bundle_sibling_daemon() {
    let temp = TemporaryDirectory::new("packaged-cli-symlink");
    let cli = write_packaged_fixture(&temp);
    let unrelated = TemporaryDirectory::new("packaged-cli-link");
    let link = unrelated.path.join("brain");
    std::os::unix::fs::symlink(&cli, &link).unwrap();
    let daemon = temp.path.join("bin/brain-daemon");
    let record = temp.path.join("daemon-selection");
    fs::write(
        &daemon,
        format!("#!/bin/sh\nprintf '%s' \"$*\" > '{}'\n", record.display()),
    )
    .unwrap();
    fs::set_permissions(&daemon, fs::Permissions::from_mode(0o755)).unwrap();
    let output = Command::new(&link)
        .args(["daemon", "status"])
        .env("PATH", "")
        .env("BRAIN_CONFIG_DIR", &unrelated.path)
        .current_dir(&unrelated.path)
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", combined_output(&output));
    assert_eq!(fs::read_to_string(record).unwrap(), "daemon status");
}
