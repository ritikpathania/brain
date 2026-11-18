use std::path::{Path, PathBuf};
use std::process::ExitStatus;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

static REQUEST_ID: AtomicU64 = AtomicU64::new(1);

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
        let body = Self::call_versioned(
            "v1/search",
            serde_json::json!({
                "text": text,
                "kinds": null,
                "pagination": null,
            }),
        )
        .await?;
        let results: Vec<serde_json::Value> = serde_json::from_str(&body).map_err(|error| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("daemon returned an invalid v1/search response: {error}"),
            )
        })?;

        if results.is_empty() {
            return Ok("No matching context found in your memory graph.".to_string());
        }

        let mut output = format!(
            "Found {} result(s) from your memory graph:\n",
            results.len()
        );
        for result in results {
            let title = result
                .get("title")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("(untitled)");
            output.push_str(&format!("  • {title}\n"));
        }
        Ok(output.trim_end().to_string())
    }

    /// Direct memory ingestion over Unix Domain Socket.
    pub async fn run_ingest(content: &str) -> Result<String, Box<dyn std::error::Error>> {
        let envelope = brain_integrations::IngestionEnvelope {
            event: brain_integrations::IngestionEvent::Text {
                content: content.to_string(),
                metadata: std::collections::BTreeMap::new(),
            },
            event_model_version: "1.0".to_string(),
            identity: brain_integrations::EventIdentity {
                event_id: brain_domain::EventId::new(),
                parent_event_id: None,
                workspace_id: brain_domain::WorkspaceId::new("brain-cli"),
                client_id: brain_domain::ClientId::new("brain-cli"),
                adapter_id: brain_domain::AdapterId::new("brain-cli"),
                session_id: brain_domain::SessionId::new(),
                conversation_id: None,
                timestamp: chrono::Utc::now(),
            },
        };
        let body = Self::call_versioned("v1/ingest", serde_json::to_value(envelope)?).await?;
        let response: serde_json::Value = serde_json::from_str(&body).map_err(|error| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("daemon returned an invalid v1/ingest response: {error}"),
            )
        })?;
        if response
            .get("processed")
            .and_then(serde_json::Value::as_bool)
            != Some(true)
        {
            return Err(std::io::Error::other("daemon did not process v1/ingest request").into());
        }

        let event_id = response
            .get("event_id")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("unknown");
        Ok(format!(
            "✔ Ingested memory successfully (Event ID: {event_id})"
        ))
    }

    /// Retrieves the daemon's typed operational status through the current UDS protocol.
    pub async fn run_status() -> Result<serde_json::Value, Box<dyn std::error::Error>> {
        let body = Self::call_versioned("v1/status", serde_json::Value::Null).await?;
        serde_json::from_str(&body).map_err(|error| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("daemon returned an invalid v1/status response: {error}"),
            )
            .into()
        })
    }

    async fn call_versioned(
        action: &str,
        body: serde_json::Value,
    ) -> Result<String, Box<dyn std::error::Error>> {
        let mut stream = tokio::net::UnixStream::connect(Self::resolve_socket_path()).await?;
        let request_id = REQUEST_ID.fetch_add(1, Ordering::Relaxed);
        let request = serde_json::json!({
            "version": "1.0",
            "type": "Request",
            "id": request_id,
            "action": action,
            "body": if body.is_null() { String::new() } else { serde_json::to_string(&body)? },
        });
        let payload = format!("{}\n", serde_json::to_string(&request)?);
        stream.write_all(payload.as_bytes()).await?;
        stream.flush().await?;

        let mut reader = BufReader::new(stream);
        let mut line = String::new();
        tokio::time::timeout(
            std::time::Duration::from_secs(5),
            reader.read_line(&mut line),
        )
        .await
        .map_err(|_| {
            std::io::Error::new(std::io::ErrorKind::TimedOut, "daemon response timed out")
        })??;
        if line.trim().is_empty() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::UnexpectedEof,
                "daemon closed the UDS connection without a response",
            )
            .into());
        }

        let response: serde_json::Value = serde_json::from_str(line.trim()).map_err(|error| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("daemon returned invalid UDS JSON: {error}"),
            )
        })?;
        if response.get("id") != Some(&serde_json::json!(request_id)) {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "daemon response did not correlate to the CLI request",
            )
            .into());
        }
        match response.get("type").and_then(serde_json::Value::as_str) {
            Some("Response")
                if response.get("status").and_then(serde_json::Value::as_str)
                    == Some("success") =>
            {
                response
                    .get("body")
                    .and_then(serde_json::Value::as_str)
                    .map(str::to_owned)
                    .ok_or_else(|| {
                        std::io::Error::new(
                            std::io::ErrorKind::InvalidData,
                            "successful daemon response omitted its body",
                        )
                    })
                    .map_err(Into::into)
            }
            Some("Error") => Err(std::io::Error::other(
                response
                    .get("body")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("daemon rejected the request"),
            )
            .into()),
            _ => Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "daemon returned an unexpected versioned response",
            )
            .into()),
        }
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
