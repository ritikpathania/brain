//! Artifact-relative Python runtime discovery for packaged daemons.
//!
//! Development binaries continue to use the interpreter selected at build time.
//! A packaged daemon is identified solely by the documented `bin/brain-daemon`
//! layout and must find a complete runtime at `../runtime/python`; it never
//! searches PATH, a user home, or the current working directory.

use std::ffi::OsStr;
use std::io;
use std::path::{Path, PathBuf};

/// Returns the packaged Python home for an executable in the documented
/// `bin/brain-daemon` layout when that runtime is complete.
pub fn packaged_python_home_from_executable(executable: &Path) -> Option<PathBuf> {
    let binary_directory = executable.parent()?;
    if binary_directory.file_name() != Some(OsStr::new("bin")) {
        return None;
    }

    let artifact_root = binary_directory.parent()?;
    let python_home = artifact_root.join("runtime/python");
    is_complete_python_home(&python_home).then_some(python_home)
}

/// Configures CPython's standard-library home before PyO3 initializes it.
///
/// Returns `Ok(None)` for development layouts. A binary in the documented
/// package layout without a complete sibling runtime is an invalid artifact and
/// fails rather than falling back to an arbitrary host interpreter.
pub fn configure_packaged_python_runtime() -> io::Result<Option<PathBuf>> {
    let executable = std::env::current_exe()?;
    let python_home = resolve_packaged_python_runtime_from_executable(&executable)?;
    if let Some(python_home) = python_home.as_ref() {
        // CPython reads PYTHONHOME during initialization. This happens before the
        // application runtime reaches its mandatory PyO3 initialization phase.
        std::env::set_var("PYTHONHOME", python_home);
    }
    Ok(python_home)
}

/// Resolves the Python home required by an executable in the package layout.
///
/// This is public so process-level release tests can exercise the exact layout
/// policy without changing the process-wide Python environment.
pub fn resolve_packaged_python_runtime_from_executable(
    executable: &Path,
) -> io::Result<Option<PathBuf>> {
    let binary_directory = executable.parent().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "brain-daemon executable has no parent directory",
        )
    })?;

    if binary_directory.file_name() != Some(OsStr::new("bin")) {
        return Ok(None);
    }

    let artifact_root = binary_directory.parent().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "packaged brain-daemon binary has no artifact root",
        )
    })?;
    let python_home = artifact_root.join("runtime/python");
    if !is_complete_python_home(&python_home) {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!(
                "packaged brain-daemon requires a complete CPython runtime at {}",
                python_home.display()
            ),
        ));
    }
    Ok(Some(python_home))
}

fn is_complete_python_home(python_home: &Path) -> bool {
    let library_directory = python_home.join("lib");
    if !library_directory.is_dir() || !contains_libpython(&library_directory) {
        return false;
    }

    std::fs::read_dir(&library_directory)
        .ok()
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .any(|path| {
            path.is_dir()
                && path
                    .file_name()
                    .and_then(OsStr::to_str)
                    .is_some_and(|name| name.starts_with("python3."))
                && path.join("os.py").is_file()
        })
}

fn contains_libpython(library_directory: &Path) -> bool {
    std::fs::read_dir(library_directory)
        .ok()
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .filter_map(|entry| entry.file_name().into_string().ok())
        .any(|name| {
            name.starts_with("libpython3.")
                && (name.ends_with(".dylib")
                    || name.contains(".dylib.")
                    || name.ends_with(".so")
                    || name.contains(".so."))
        })
}

#[cfg(test)]
mod tests {
    use super::contains_libpython;
    use std::fs;

    #[test]
    fn recognizes_supported_dynamic_libpython_names() {
        let directory = std::env::temp_dir().join(format!(
            "brain-daemon-libpython-name-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&directory);
        fs::create_dir_all(&directory).expect("create temporary library directory");
        fs::write(directory.join("libpython3.12.so.1.0"), b"fixture")
            .expect("write libpython fixture");

        assert!(contains_libpython(&directory));
        let _ = fs::remove_dir_all(directory);
    }
}
