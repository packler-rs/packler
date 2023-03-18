//! Common functionality and types.
//! Borrowed from Trunk!

use std::ffi::OsStr;
use std::fmt::Debug;
use std::fs::Metadata;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::process::Stdio;

use anyhow::{bail, Context, Result};
use once_cell::sync::Lazy;
use tokio::fs;
use tokio::process::Command;

static CWD: Lazy<PathBuf> =
    Lazy::new(|| std::env::current_dir().expect("error getting current dir"));

/// Checks if path exists.
pub async fn path_exists(path: impl AsRef<Path>) -> Result<bool> {
    fs::metadata(path.as_ref())
        .await
        .map(|_| true)
        .or_else(|error| {
            if error.kind() == ErrorKind::NotFound {
                Ok(false)
            } else {
                Err(error)
            }
        })
        .with_context(|| {
            format!(
                "error checking for existence of path at {:?}",
                path.as_ref()
            )
        })
}

/// Check whether a given path exists, is a file and marked as executable.
pub async fn is_executable(path: impl AsRef<Path>) -> Result<bool> {
    #[cfg(unix)]
    let has_executable_flag = |meta: Metadata| {
        use std::os::unix::fs::PermissionsExt;
        meta.permissions().mode() & 0o100 != 0
    };
    #[cfg(not(unix))]
    let has_executable_flag = |meta: Metadata| true;

    fs::metadata(path.as_ref())
        .await
        .map(|meta| meta.is_file() && has_executable_flag(meta))
        .or_else(|error| {
            if error.kind() == ErrorKind::NotFound {
                Ok(false)
            } else {
                Err(error)
            }
        })
        .with_context(|| format!("error checking file mode for file {:?}", path.as_ref()))
}

/// Strip the CWD prefix from the given path.
///
/// Returns `target` unmodified if an error is returned from the operation.
pub fn strip_prefix(target: &Path) -> &Path {
    match target.strip_prefix(CWD.as_path()) {
        Ok(relative) => relative,
        Err(_) => target,
    }
}

/// Run a global command with the given arguments and make sure it completes successfully. If it
/// fails an error is returned.
pub async fn run_command(
    name: &str,
    path: &Path,
    args: &[impl AsRef<OsStr> + Debug],
) -> Result<()> {
    log::debug!("Run external binary: {name} (bin: {path:?})");
    let status = Command::new(path)
        .args(args)
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()
        .map_err(|e| {
            log::error!("error spawning {name} call: {e}");
            e
        })
        .with_context(|| {
            log::error!("error spawning {} call", name);
            format!("error spawning {} call", name)
        })?
        .wait()
        .await
        .with_context(|| {
            log::error!("error during {} call", name);
            format!("error during {} call", name)
        })?;
    if !status.success() {
        log::error!("{} call returned a bad status", name);
        bail!("{} call returned a bad status", name);
    }
    Ok(())
}
