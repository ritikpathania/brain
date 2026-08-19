use std::path::PathBuf;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

pub struct CLIHost;

impl CLIHost {
    /// Launches the React + Ink + Yoga production frontend (`packages/brain-frontend`).
    pub async fn run_tui() -> Result<(), Box<dyn std::error::Error>> {
        // 1. Locate frontend entrypoint
        let frontend_main = Self::resolve_frontend_entrypoint()?;
        let preload_script = frontend_main.parent().map(|p| p.join("preload.ts"));

        // 2. Resolve JS runtime (prefer bun, fallback to node)
        let runtime = Self::resolve_runtime()?;

        if std::env::var("BRAIN_DEBUG").is_ok() {
            eprintln!(
                "[frontend launcher] Spawning {:?} with {:?}",
                runtime, frontend_main
            );
        }

        // 3. Spawn child process with inherited stdio
        let mut cmd = std::process::Command::new(&runtime);
        if runtime.ends_with("bun") || runtime.to_string_lossy().contains("bun") {
            if let Some(ref preload) = preload_script {
                if preload.exists() {
                    cmd.args(["run", "--feature", "AUTO_THEME", "--preload", preload.to_str().unwrap_or("./src/preload.ts"), frontend_main.to_str().unwrap_or("src/main.tsx")]);
                } else {
                    cmd.args(["run", "--feature", "AUTO_THEME", frontend_main.to_str().unwrap_or("src/main.tsx")]);
                }
            } else {
                cmd.args(["run", "--feature", "AUTO_THEME", frontend_main.to_str().unwrap_or("src/main.tsx")]);
            }
        } else {
            cmd.arg(frontend_main);
        }

        // Pass along environment variables
        if let Ok(socket_path) = std::env::var("BRAIN_SOCKET_PATH") {
            cmd.env("BRAIN_SOCKET_PATH", socket_path);
        }

        let status = cmd
            .stdin(std::process::Stdio::inherit())
            .stdout(std::process::Stdio::inherit())
            .stderr(std::process::Stdio::inherit())
            .status()?;

        std::process::exit(status.code().unwrap_or(0));
    }

    /// Direct non-interactive query over Unix Domain Socket.
    pub async fn run_query(text: &str) -> Result<String, Box<dyn std::error::Error>> {
        let socket_path = if let Ok(path) = std::env::var("BRAIN_SOCKET_PATH") {
            PathBuf::from(path)
        } else {
            dirs::home_dir()
                .map(|h| h.join(".brain").join("daemon.sock"))
                .unwrap_or_else(|| PathBuf::from("daemon.sock"))
        };

        let mut stream = tokio::net::UnixStream::connect(socket_path).await?;
        let payload = serde_json::json!({
            "action": "query",
            "payload": text,
        })
        .to_string()
            + "\n";

        stream.write_all(payload.as_bytes()).await?;
        stream.flush().await?;

        let mut reader = BufReader::new(stream);
        let mut line = String::new();
        let mut output = String::new();

        while reader.read_line(&mut line).await? > 0 {
            let trimmed = line.trim();
            if let Ok(value) = serde_json::from_str::<serde_json::Value>(trimmed) {
                if let Some(event_type) = value.get("type").and_then(|t| t.as_str()) {
                    match event_type {
                        "stream_chunk" => {
                            if let Some(content) = value.get("content").and_then(|c| c.as_str()) {
                                output.push_str(content);
                            }
                        }
                        "stream_end" => {
                            break;
                        }
                        _ => {}
                    }
                }
            }
            line.clear();
        }

        Ok(output)
    }

    /// Direct memory ingestion over Unix Domain Socket.
    pub async fn run_ingest(content: &str) -> Result<String, Box<dyn std::error::Error>> {
        let socket_path = if let Ok(path) = std::env::var("BRAIN_SOCKET_PATH") {
            PathBuf::from(path)
        } else {
            dirs::home_dir()
                .map(|h| h.join(".brain").join("daemon.sock"))
                .unwrap_or_else(|| PathBuf::from("daemon.sock"))
        };

        let mut stream = tokio::net::UnixStream::connect(socket_path).await?;
        let payload = serde_json::json!({
            "action": "ingest",
            "payload": content,
        })
        .to_string()
            + "\n";

        stream.write_all(payload.as_bytes()).await?;
        stream.flush().await?;

        let mut reader = BufReader::new(stream);
        let mut line = String::new();
        reader.read_line(&mut line).await?;
        let trimmed = line.trim();

        // Parse response JSON for clean CLI output
        if let Ok(value) = serde_json::from_str::<serde_json::Value>(trimmed) {
            if value.get("status").and_then(|s| s.as_str()) == Some("ok") {
                if let Some(msg_str) = value.get("message").and_then(|m| m.as_str()) {
                    if let Ok(inner) = serde_json::from_str::<serde_json::Value>(msg_str) {
                        let event_id = inner
                            .get("event_id")
                            .and_then(|id| id.as_str())
                            .unwrap_or("unknown");
                        return Ok(format!(
                            "✔ Ingested memory successfully (Event ID: {})",
                            event_id
                        ));
                    }
                }
                return Ok("✔ Ingested memory successfully".to_string());
            } else if let Some(msg) = value.get("message").and_then(|m| m.as_str()) {
                return Ok(format!("✖ Ingest failed: {}", msg));
            }
        }

        Ok(trimmed.to_string())
    }

    fn resolve_frontend_entrypoint() -> Result<PathBuf, Box<dyn std::error::Error>> {
        // 1. Current working directory lookup
        if let Ok(cwd) = std::env::current_dir() {
            let candidate = cwd.join("packages/brain-shell/src/main.tsx");
            if candidate.exists() {
                return Ok(candidate);
            }
        }

        // 2. Traversal relative to current exe
        if let Ok(exe_path) = std::env::current_exe() {
            for ancestor in exe_path.ancestors() {
                let candidate = ancestor.join("packages/brain-shell/src/main.tsx");
                if candidate.exists() {
                    return Ok(candidate);
                }
            }
        }

        // 3. User home bundle fallback
        if let Some(home) = dirs::home_dir() {
            let user_bundle = home.join(".brain/shell/src/main.tsx");
            if user_bundle.exists() {
                return Ok(user_bundle);
            }
        }

        Err(Box::new(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "Could not locate 'packages/brain-shell/src/main.tsx'. Ensure packages/brain-shell is present.",
        )))
    }

    fn resolve_runtime() -> Result<PathBuf, Box<dyn std::error::Error>> {
        // 1. Direct command probe
        for binary in &["bun", "node"] {
            if let Ok(output) = std::process::Command::new(binary).arg("--version").output() {
                if output.status.success() {
                    return Ok(PathBuf::from(binary));
                }
            }
        }

        // 2. Search common user install locations
        if let Some(home) = dirs::home_dir() {
            let bun_home = home.join(".bun/bin/bun");
            if bun_home.exists() {
                return Ok(bun_home);
            }
        }

        for path_str in &[
            "/opt/homebrew/bin/bun",
            "/usr/local/bin/bun",
            "/opt/homebrew/bin/node",
            "/usr/local/bin/node",
        ] {
            let p = PathBuf::from(path_str);
            if p.exists() {
                return Ok(p);
            }
        }

        Err(Box::new(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "No JavaScript runtime found on PATH. Please install 'bun' (recommended) or 'node' to run the Brain frontend.",
        )))
    }
}
