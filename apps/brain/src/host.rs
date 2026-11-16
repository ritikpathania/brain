use std::path::{Path, PathBuf};
use std::process::ExitStatus;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

pub struct CLIHost;

/// Explicit development override, validated installed artifact, or owned source.
pub enum CanonicalFrontend {
    Development {
        root: PathBuf,
        runtime: PathBuf,
        version: String,
    },
    Packaged(Box<crate::frontend_artifact::PackagedFrontend>),
}

impl CLIHost {
    /// Explicit override wins; malformed detected installations never fall back.
    pub fn resolve_canonical_frontend() -> Result<CanonicalFrontend, Box<dyn std::error::Error>> {
        let root = match std::env::var_os("BRAIN_FRONTEND_ROOT") {
            Some(value) if value.is_empty() => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    "BRAIN_FRONTEND_ROOT must not be empty; unset it to use the first-party development frontend.",
                ).into());
            }
            Some(value) => PathBuf::from(value),
            None => {
                let executable = std::env::current_exe()?.canonicalize()?;
                if let Some(frontend) = crate::frontend_artifact::resolve(&executable)? {
                    return Ok(CanonicalFrontend::Packaged(Box::new(frontend)));
                }
                Self::development_frontend_root(&executable)?
            }
        };

        if !root.is_dir() {
            return Err(Box::new(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!(
                    "BRAIN_FRONTEND_ROOT must name an existing frontend directory: {}",
                    root.display()
                ),
            )));
        }

        let root = root.canonicalize()?;
        let manifest = root.join("package.json");
        if !manifest.is_file() {
            return Err(Box::new(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!(
                    "Configured canonical frontend is missing package.json: {}",
                    manifest.display()
                ),
            )));
        }

        let entrypoint = root.join("src/cli/main.ts");
        if !entrypoint.is_file() {
            return Err(Box::new(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!(
                    "Brain development frontend is missing src/cli/main.ts: {}",
                    entrypoint.display()
                ),
            )));
        }

        let manifest_body = std::fs::read_to_string(&manifest)?;
        let value: serde_json::Value = serde_json::from_str(&manifest_body).map_err(|error| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!(
                    "Invalid frontend package.json at {}: {error}",
                    manifest.display()
                ),
            )
        })?;
        if value.get("name").and_then(serde_json::Value::as_str) != Some("@brain/frontend")
            || value
                .pointer("/scripts/start")
                .and_then(serde_json::Value::as_str)
                != Some("bun run src/cli/main.ts")
        {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("Frontend package.json must identify @brain/frontend with start command 'bun run src/cli/main.ts': {}", manifest.display()),
            ).into());
        }
        let version = value
            .get("version")
            .and_then(serde_json::Value::as_str)
            .filter(|version| !version.trim().is_empty())
            .map(str::to_owned)
            .ok_or_else(|| {
                std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!(
                        "Configured canonical frontend package.json must contain a non-empty version: {}",
                        manifest.display()
                    ),
                )
            })?;

        Ok(CanonicalFrontend::Development {
            root,
            runtime: Self::resolve_frontend_runtime()?,
            version,
        })
    }

    fn development_frontend_root(executable: &Path) -> Result<PathBuf, std::io::Error> {
        for ancestor in executable.ancestors().skip(1) {
            // Only executable ancestry is consulted, never cwd, HOME, or private
            // checkout discovery. Both workspace markers must identify a Brain tree.
            if ancestor.join("Cargo.toml").is_file()
                && ancestor.join("apps/brain/Cargo.toml").is_file()
            {
                return Ok(ancestor.join("packages/brain-frontend"));
            }
        }
        Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "Cannot locate a packaged or first-party development frontend from this executable. Install the complete Brain distribution, build/run brain inside its repository, or explicitly set BRAIN_FRONTEND_ROOT to an @brain/frontend development package.",
        ))
    }

    /// Launches the configured canonical frontend with the Brain transport environment.
    pub fn run_tui(frontend: CanonicalFrontend) -> Result<ExitStatus, Box<dyn std::error::Error>> {
        let socket_path = Self::resolve_socket_path();
        // Keep the same socket used by preflight when the child changes cwd.
        let socket_path = if socket_path.is_absolute() {
            socket_path
        } else {
            std::env::current_dir()?.join(socket_path)
        };

        let mut command = match frontend {
            CanonicalFrontend::Development {
                root,
                runtime,
                version,
            } => {
                let mut command = std::process::Command::new(runtime);
                command
                    .args(["run", "start"])
                    .current_dir(root)
                    .env("BRAIN_SOCKET_PATH", &socket_path)
                    .env("BRAIN_FRONTEND_VERSION", version);
                command
            }
            CanonicalFrontend::Packaged(frontend) => {
                crate::frontend_artifact::sanitized_packaged_command(&frontend, &socket_path)
            }
        };
        if std::env::var("BRAIN_DEBUG").is_ok() {
            if let Some(root) = command.get_current_dir() {
                eprintln!(
                    "[frontend launcher] Spawning {:?} from {}",
                    command.get_program(),
                    root.display()
                );
            }
        }
        command
            .stdin(std::process::Stdio::inherit())
            .stdout(std::process::Stdio::inherit())
            .stderr(std::process::Stdio::inherit());

        // The Unix CLI gives terminal ownership to the frontend after preflight. Replacing
        // the wrapper preserves signal/exit status without an orphanable CLI parent.
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            Err(command.exec().into())
        }
        #[cfg(not(unix))]
        command.status().map_err(Into::into)
    }

    /// Resolves the socket path shared by the CLI launcher and direct UDS commands.
    pub fn resolve_socket_path() -> PathBuf {
        brain_config::resolve_runtime_paths().socket_path
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

    fn resolve_frontend_runtime() -> Result<PathBuf, Box<dyn std::error::Error>> {
        let configured = if let Some(runtime) = std::env::var_os("BRAIN_FRONTEND_RUNTIME")
            .filter(|value| !value.is_empty())
            .map(PathBuf::from)
        {
            if (runtime.is_absolute() || runtime.components().count() > 1) && !runtime.is_file() {
                return Err(Box::new(std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    format!(
                        "BRAIN_FRONTEND_RUNTIME must name an executable runtime: {}",
                        runtime.display()
                    ),
                )));
            }
            runtime
        } else {
            PathBuf::from("bun")
        };

        // Freeze an absolute executable before switching to the package root.
        // Bare names use only the explicit PATH; no home-directory Bun discovery.
        let runtime = if configured.is_absolute() || configured.components().count() > 1 {
            configured.canonicalize().ok()
        } else {
            std::env::var_os("PATH").and_then(|path| {
                std::env::split_paths(&path)
                    .map(|directory| directory.join(&configured))
                    .find(|candidate| candidate.is_file() && is_executable(candidate))
                    .and_then(|candidate| candidate.canonicalize().ok())
            })
        }.ok_or_else(|| std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "Bun runtime is unavailable. Install Bun on PATH or set BRAIN_FRONTEND_RUNTIME to its executable.",
        ))?;

        match std::process::Command::new(&runtime).arg("--version").output() {
            Ok(output) if output.status.success() => Ok(runtime),
            Ok(output) => Err(Box::new(std::io::Error::other(format!(
                "Bun runtime probe failed with {}. Install Bun or set BRAIN_FRONTEND_RUNTIME.",
                output.status
            )))),
            Err(error) => Err(Box::new(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!(
                    "Configured Bun runtime is unavailable. Install Bun or set BRAIN_FRONTEND_RUNTIME: {error}"
                ),
            ))),
        }
    }
}

fn is_executable(path: &Path) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::metadata(path).is_ok_and(|metadata| metadata.permissions().mode() & 0o111 != 0)
    }
    #[cfg(not(unix))]
    path.is_file()
}
