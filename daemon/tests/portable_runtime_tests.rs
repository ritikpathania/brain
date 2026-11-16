use std::fs;
use std::path::PathBuf;

use daemon_bridge::python_runtime::{
    packaged_python_home_from_executable, resolve_packaged_python_runtime_from_executable,
};

struct TemporaryDirectory(PathBuf);

impl TemporaryDirectory {
    fn new() -> Self {
        let unique = uuid::Uuid::new_v4();
        let path = std::env::temp_dir().join(format!(
            "brain-daemon-portable-runtime-{}-{unique}",
            std::process::id()
        ));
        fs::create_dir_all(&path).expect("create temporary directory");
        Self(path)
    }
}

impl Drop for TemporaryDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn make_runtime_layout(root: &std::path::Path) -> PathBuf {
    let executable = root.join("bin/brain-daemon");
    fs::create_dir_all(executable.parent().expect("binary parent"))
        .expect("create binary directory");
    fs::write(&executable, b"fixture binary").expect("write binary fixture");

    let python_home = root.join("runtime/python");
    fs::create_dir_all(python_home.join("lib/python3.12")).expect("create stdlib directory");
    fs::write(python_home.join("lib/python3.12/os.py"), b"# fixture").expect("write stdlib marker");
    fs::write(
        python_home.join("lib/libpython3.12.dylib"),
        b"fixture dylib",
    )
    .expect("write dylib fixture");
    executable
}

#[test]
fn resolves_a_complete_python_runtime_adjacent_to_the_packaged_daemon() {
    let temporary = TemporaryDirectory::new();
    let executable = make_runtime_layout(&temporary.0);

    assert_eq!(
        packaged_python_home_from_executable(&executable),
        Some(temporary.0.join("runtime/python"))
    );
}

#[test]
fn rejects_an_incomplete_adjacent_runtime_instead_of_guessing_from_the_host() {
    let temporary = TemporaryDirectory::new();
    let executable = make_runtime_layout(&temporary.0);
    fs::remove_file(temporary.0.join("runtime/python/lib/python3.12/os.py"))
        .expect("remove stdlib marker");

    assert_eq!(packaged_python_home_from_executable(&executable), None);
}

#[test]
fn ignores_a_runtime_that_is_not_in_the_artifact_relative_layout() {
    let temporary = TemporaryDirectory::new();
    let executable = temporary.0.join("brain-daemon");
    fs::write(&executable, b"fixture binary").expect("write binary fixture");
    make_runtime_layout(&temporary.0.join("unrelated"));

    assert_eq!(packaged_python_home_from_executable(&executable), None);
}

#[test]
fn rejects_a_missing_runtime_for_a_binary_in_the_packaged_layout() {
    let temporary = TemporaryDirectory::new();
    let executable = temporary.0.join("bin/brain-daemon");
    fs::create_dir_all(executable.parent().expect("binary parent"))
        .expect("create binary directory");
    fs::write(&executable, b"fixture binary").expect("write binary fixture");

    let error = resolve_packaged_python_runtime_from_executable(&executable)
        .expect_err("a packaged binary without its runtime must fail");
    assert_eq!(error.kind(), std::io::ErrorKind::NotFound);
    assert!(error.to_string().contains("runtime/python"));
}

fn make_runtime_layout_315(root: &std::path::Path) -> PathBuf {
    let executable = root.join("bin/brain-daemon");
    fs::create_dir_all(executable.parent().expect("binary parent"))
        .expect("create binary directory");
    fs::write(&executable, b"fixture binary").expect("write binary fixture");

    let python_home = root.join("runtime/python");
    fs::create_dir_all(python_home.join("lib/python3.15")).expect("create stdlib directory");
    fs::write(python_home.join("lib/python3.15/os.py"), b"# fixture").expect("write stdlib marker");
    fs::write(
        python_home.join("lib/libpython3.15.dylib"),
        b"fixture dylib",
    )
    .expect("write dylib fixture");
    executable
}

#[test]
fn resolves_a_complete_python_315_runtime_adjacent_to_the_packaged_daemon() {
    let temporary = TemporaryDirectory::new();
    let executable = make_runtime_layout_315(&temporary.0);

    assert_eq!(
        packaged_python_home_from_executable(&executable),
        Some(temporary.0.join("runtime/python"))
    );
}
