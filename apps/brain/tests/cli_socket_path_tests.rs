use std::path::PathBuf;
use std::process::Command;

fn get_temp_socket_path() -> PathBuf {
    let rand_val = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("brain-test-socket-{}.sock", rand_val))
}

#[test]
fn test_socket_path_flag_nonexistent_socket_fails() {
    let bin_path = env!("CARGO_BIN_EXE_brain");
    let fake_socket = get_temp_socket_path();

    let output = Command::new(bin_path)
        .arg("--socket-path")
        .arg(&fake_socket)
        .arg("query")
        .arg("test")
        .output()
        .expect("failed to execute brain binary");

    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(
        stdout.contains("Daemon is not running"),
        "Expected output to report daemon not running when custom socket does not exist, got: {}",
        stdout
    );
}

#[test]
fn test_socket_path_flag_config_output() {
    let bin_path = env!("CARGO_BIN_EXE_brain");
    let fake_socket = get_temp_socket_path();

    let output = Command::new(bin_path)
        .arg("--socket-path")
        .arg(&fake_socket)
        .arg("config")
        .output()
        .expect("failed to execute brain binary");

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(
        stdout.contains(&fake_socket.display().to_string()),
        "Expected config output to display custom socket path {}, got: {}",
        fake_socket.display(),
        stdout
    );
}
