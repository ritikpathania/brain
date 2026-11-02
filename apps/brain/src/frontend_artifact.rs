//! First-party installed frontend validation. No source discovery or backend policy.
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::fs::{self, File};
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

pub type ArtifactError = io::Error;

fn invalid(message: impl std::fmt::Display) -> ArtifactError {
    io::Error::new(
        io::ErrorKind::InvalidData,
        format!("Invalid packaged Brain frontend: {message}"),
    )
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
struct SourceIdentity {
    sha: String,
    dirty: bool,
    fingerprint_algorithm: String,
    fingerprint_sha256: String,
    untracked_input_count: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BuildIdentity {
    schema_version: u64,
    identity: String,
    package: String,
    frontend_version: String,
    brain_version: String,
    source: SourceIdentity,
    target_triple: String,
    runtime_model: String,
    bun_version: String,
    lockfile_sha256: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FrontendManifest {
    schema_version: u64,
    identity: String,
    package: String,
    frontend_version: String,
    brain_version: String,
    source: SourceIdentity,
    target_triple: String,
    runtime_model: String,
    bun_version: String,
    lockfile_sha256: String,
    entrypoint: String,
    executable_sha256: String,
}

impl FrontendManifest {
    fn identity(&self) -> BuildIdentity {
        BuildIdentity {
            schema_version: self.schema_version,
            identity: self.identity.clone(),
            package: self.package.clone(),
            frontend_version: self.frontend_version.clone(),
            brain_version: self.brain_version.clone(),
            source: self.source.clone(),
            target_triple: self.target_triple.clone(),
            runtime_model: self.runtime_model.clone(),
            bun_version: self.bun_version.clone(),
            lockfile_sha256: self.lockfile_sha256.clone(),
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FileDigest {
    path: String,
    sha256: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ManifestDigest {
    manifest: String,
    sha256: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct BuildTools {
    rustc: String,
    cargo: String,
    bun: String,
    python: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ProductManifest {
    schema_version: u64,
    artifact: String,
    brain_version: String,
    source: SourceIdentity,
    target_triple: String,
    cli: FileDigest,
    frontend: ManifestDigest,
    daemon: ManifestDigest,
    python_files: ManifestDigest,
    licenses: Vec<FileDigest>,
    build_tools: BuildTools,
    contents: Vec<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DaemonSource {
    dirty: bool,
    untracked_file_count: u64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PythonRuntime {
    version: String,
    home: String,
    library: String,
    library_sha256: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DaemonManifest {
    schema_version: u64,
    artifact: String,
    brain_version: String,
    daemon_source_sha: String,
    source_tree: DaemonSource,
    target_triple: String,
    rust_version: String,
    python: PythonRuntime,
    daemon_sha256: String,
    runtime_files_manifest: String,
    contents: Vec<String>,
}

pub struct PackagedFrontend {
    pub root: PathBuf,
    pub executable: PathBuf,
    pub identity: BuildIdentity,
}

/// Only the canonical executable's immediate bin/ parent defines an installation.
pub fn resolve(executable: &Path) -> Result<Option<PackagedFrontend>, ArtifactError> {
    let Some(bin) = executable
        .parent()
        .filter(|p| p.file_name().is_some_and(|n| n == "bin"))
    else {
        return Ok(None);
    };
    let root = bin
        .parent()
        .ok_or_else(|| invalid("missing installation root"))?;
    let detected = [
        "product-manifest.json",
        "runtime-manifest.json",
        "frontend/brain-frontend",
    ]
    .iter()
    .any(|sentinel| match fs::symlink_metadata(root.join(sentinel)) {
        Ok(_) => true,
        Err(error) => error.kind() != io::ErrorKind::NotFound,
    });
    if detected {
        validate(root).map(Some)
    } else {
        Ok(None)
    }
}

fn read_manifest<T: serde::de::DeserializeOwned>(
    path: &Path,
    limit: u64,
) -> Result<T, ArtifactError> {
    let mut bytes = Vec::new();
    File::open(path)
        .map_err(invalid)?
        .take(limit + 1)
        .read_to_end(&mut bytes)
        .map_err(invalid)?;
    if bytes.len() as u64 > limit {
        return Err(invalid("manifest exceeds size limit"));
    }
    serde_json::from_slice(&bytes).map_err(invalid)
}

fn contained(root: &Path, relative: &str) -> Result<PathBuf, ArtifactError> {
    if relative.is_empty()
        || relative.contains(['\\', '\0'])
        || relative
            .split('/')
            .any(|part| matches!(part, "" | "." | ".."))
        || Path::new(relative).is_absolute()
    {
        return Err(invalid(
            "artifact path must be a non-traversing relative POSIX path",
        ));
    }
    let canonical_root = root.canonicalize().map_err(invalid)?;
    let path = canonical_root
        .join(relative)
        .canonicalize()
        .map_err(invalid)?;
    if !path.starts_with(canonical_root) {
        return Err(invalid("artifact symlink escapes containment"));
    }
    Ok(path)
}

fn require_hex(value: &str, length: usize) -> Result<(), ArtifactError> {
    if value.len() != length
        || !value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err(invalid("invalid lowercase SHA digest"));
    }
    Ok(())
}

fn verify_digest(path: &Path, expected: &str) -> Result<(), ArtifactError> {
    require_hex(expected, 64)?;
    let mut file = File::open(path).map_err(invalid)?;
    let mut digest = Sha256::new();
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let size = file.read(&mut buffer).map_err(invalid)?;
        if size == 0 {
            break;
        }
        digest.update(&buffer[..size]);
    }
    let actual: String = digest
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    if actual != expected {
        return Err(invalid("artifact digest mismatch"));
    }
    Ok(())
}

fn validate_identity(identity: &BuildIdentity) -> Result<(), ArtifactError> {
    if identity.schema_version != 1
        || identity.identity != "brain-frontend"
        || identity.package != "@brain/frontend"
        || identity.runtime_model != "bun-standalone"
        || identity.bun_version != "1.4.4"
        || identity.brain_version != env!("CARGO_PKG_VERSION")
        || identity.target_triple != env!("BRAIN_BUILD_TARGET")
        || !matches!(
            identity.target_triple.as_str(),
            "aarch64-apple-darwin" | "x86_64-unknown-linux-gnu"
        )
        || identity.source.fingerprint_algorithm != "brain-source-tree-v1"
    {
        return Err(invalid("incompatible frontend identity/version/target"));
    }
    for version in [&identity.frontend_version, &identity.brain_version] {
        semver::Version::parse(version).map_err(invalid)?;
    }
    require_hex(&identity.source.sha, 40)?;
    require_hex(&identity.source.fingerprint_sha256, 64)?;
    require_hex(&identity.lockfile_sha256, 64)
}

pub fn validate(root: &Path) -> Result<PackagedFrontend, ArtifactError> {
    let root = root.canonicalize().map_err(invalid)?;
    let product: ProductManifest =
        read_manifest(&contained(&root, "product-manifest.json")?, 2 * 1024 * 1024)?;
    if product.schema_version != 1
        || product.artifact != "brain-product"
        || product.brain_version != env!("CARGO_PKG_VERSION")
        || product.target_triple != env!("BRAIN_BUILD_TARGET")
        || product.cli.path != "bin/brain"
        || product.frontend.manifest != "frontend/brain-frontend/manifest.json"
        || product.daemon.manifest != "runtime-manifest.json"
        || product.python_files.manifest != "runtime/python-files.json"
    {
        return Err(invalid(
            "invalid product manifest identity or fixed component paths",
        ));
    }
    verify_digest(&contained(&root, &product.cli.path)?, &product.cli.sha256)?;
    for component in [&product.frontend, &product.daemon, &product.python_files] {
        verify_digest(&contained(&root, &component.manifest)?, &component.sha256)?;
    }
    let frontend_root = contained(&root, "frontend/brain-frontend")?;
    let manifest: FrontendManifest =
        read_manifest(&contained(&frontend_root, "manifest.json")?, 16 * 1024)?;
    let identity = manifest.identity();
    validate_identity(&identity)?;
    if product.source != identity.source || manifest.entrypoint != "brain-frontend" {
        return Err(invalid(
            "product/frontend source identity or entrypoint mismatch",
        ));
    }
    let executable = contained(&frontend_root, &manifest.entrypoint)?;
    let metadata = fs::metadata(&executable).map_err(invalid)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if !metadata.is_file() || metadata.permissions().mode() & 0o111 == 0 {
            return Err(invalid("frontend missing executable permission"));
        }
    }
    let daemon: DaemonManifest =
        read_manifest(&contained(&root, &product.daemon.manifest)?, 64 * 1024)?;
    if daemon.schema_version != 1
        || daemon.artifact != "brain-daemon-runtime"
        || daemon.brain_version != identity.brain_version
        || daemon.target_triple != identity.target_triple
        || daemon.daemon_source_sha != identity.source.sha
        || daemon.python.home != "runtime/python"
        || daemon.python.version != "3.15.2"
        || daemon.runtime_files_manifest != product.python_files.manifest
        || daemon.rust_version.is_empty()
        || daemon.contents
            != [
                "bin/brain-daemon",
                "runtime/python",
                "runtime-manifest.json",
                "runtime/python-files.json",
            ]
    {
        return Err(invalid("incompatible daemon runtime manifest"));
    }
    // D's earlier source-tree metadata is not the source-input fingerprint.
    let _component_dirty_metadata = (
        daemon.source_tree.dirty,
        daemon.source_tree.untracked_file_count,
    );
    verify_digest(
        &contained(&root, "bin/brain-daemon")?,
        &daemon.daemon_sha256,
    )?;
    verify_digest(
        &contained(
            &contained(&root, &daemon.python.home)?,
            &daemon.python.library,
        )?,
        &daemon.python.library_sha256,
    )?;
    if product.build_tools.rustc.is_empty()
        || product.build_tools.cargo.is_empty()
        || product.build_tools.bun != identity.bun_version
        || product.build_tools.python != daemon.python.version
        || product.licenses.is_empty()
    {
        return Err(invalid("missing build tools or delivered notices"));
    }
    let mut names = HashSet::new();
    for path in &product.contents {
        if !names.insert(path.to_lowercase()) || !contained(&root, path)?.is_file() {
            return Err(invalid("invalid/duplicate delivered file inventory"));
        }
    }
    for required in [
        "bin/brain",
        "bin/brain-daemon",
        "frontend/brain-frontend/brain-frontend",
        "frontend/brain-frontend/manifest.json",
        "product-manifest.json",
        "runtime-manifest.json",
        "runtime/python-files.json",
    ] {
        if !product.contents.iter().any(|path| path == required) {
            return Err(invalid("incomplete delivered inventory"));
        }
    }
    for notice in &product.licenses {
        if !notice.path.starts_with("licenses/") || !product.contents.contains(&notice.path) {
            return Err(invalid("invalid delivered notice path"));
        }
        verify_digest(&contained(&root, &notice.path)?, &notice.sha256)?;
    }
    verify_digest(&executable, &manifest.executable_sha256)?;
    let packaged = PackagedFrontend {
        root: frontend_root,
        executable,
        identity,
    };
    if probe_identity(&packaged)? != packaged.identity {
        return Err(invalid(
            "compiled identity does not match frontend manifest",
        ));
    }
    Ok(packaged)
}

pub fn sanitized_packaged_command(frontend: &PackagedFrontend, socket: &Path) -> Command {
    let mut command = Command::new(&frontend.executable);
    command
        .current_dir(&frontend.root)
        .env("BRAIN_SOCKET_PATH", socket);
    for key in [
        "BUN_BE_BUN",
        "BUN_OPTIONS",
        "NODE_OPTIONS",
        "BRAIN_FRONTEND_RUNTIME",
        "BRAIN_FRONTEND_VERSION",
        "BRAIN_FRONTEND_BUILD_ID",
    ] {
        command.env_remove(key);
    }
    command
}

/// Owns the diagnostic process group, including children that retain its pipes.
struct ProbeProcess(Child);
impl Drop for ProbeProcess {
    fn drop(&mut self) {
        #[cfg(unix)]
        unsafe {
            kill(-(self.0.id() as i32), 9);
        }
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

#[cfg(unix)]
unsafe extern "C" {
    fn kill(pid: i32, signal: i32) -> i32;
    fn fcntl(fd: i32, command: i32, ...) -> i32;
}

#[cfg(unix)]
fn nonblocking(pipe: &impl std::os::fd::AsRawFd) -> Result<(), ArtifactError> {
    #[cfg(target_os = "macos")]
    const NONBLOCK: i32 = 4;
    #[cfg(not(target_os = "macos"))]
    const NONBLOCK: i32 = 0x800;
    let fd = pipe.as_raw_fd();
    // F_GETFL / F_SETFL, shared by supported macOS and Linux targets.
    let flags = unsafe { fcntl(fd, 3) };
    if flags < 0 || unsafe { fcntl(fd, 4, flags | NONBLOCK) } < 0 {
        return Err(invalid(io::Error::last_os_error()));
    }
    Ok(())
}

fn probe_identity(frontend: &PackagedFrontend) -> Result<BuildIdentity, ArtifactError> {
    let mut command = sanitized_packaged_command(frontend, Path::new("/unused-identity-socket"));
    command
        .arg("--build-identity")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
    let mut child = ProbeProcess(command.spawn().map_err(invalid)?);
    let mut stdout = child
        .0
        .stdout
        .take()
        .ok_or_else(|| invalid("missing identity pipe"))?;
    let mut stderr = child
        .0
        .stderr
        .take()
        .ok_or_else(|| invalid("missing identity pipe"))?;
    #[cfg(unix)]
    {
        nonblocking(&stdout)?;
        nonblocking(&stderr)?;
    }
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut outputs = [Vec::new(), Vec::new()];
    let mut closed = [false; 2];
    loop {
        if Instant::now() >= deadline {
            return Err(invalid("compiled identity diagnostic exceeded deadline"));
        }
        for (index, pipe) in [&mut stdout as &mut dyn Read, &mut stderr]
            .into_iter()
            .enumerate()
        {
            if closed[index] {
                continue;
            }
            let mut bytes = [0u8; 4096];
            match pipe.read(&mut bytes) {
                Ok(0) => closed[index] = true,
                Ok(count) => outputs[index].extend_from_slice(&bytes[..count]),
                Err(error)
                    if matches!(
                        error.kind(),
                        io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted
                    ) => {}
                Err(error) => return Err(invalid(error)),
            }
            if outputs.iter().map(Vec::len).sum::<usize>() > 16 * 1024 {
                return Err(invalid(
                    "compiled identity diagnostic exceeded output limit",
                ));
            }
        }
        if let Some(status) = child.0.try_wait().map_err(invalid)? {
            if closed.iter().all(|value| *value) {
                if !status.success() || !outputs[1].is_empty() {
                    return Err(invalid("compiled identity diagnostic failed"));
                }
                let identity = serde_json::from_slice(&outputs[0])
                    .map_err(|error| invalid(format!("malformed compiled identity: {error}")))?;
                validate_identity(&identity)?;
                return Ok(identity);
            }
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}
