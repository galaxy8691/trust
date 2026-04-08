use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::{DriverError, RustBuildOptions, CRATE_NAME};

pub(crate) fn cargo_build(root: &Path, opts: &RustBuildOptions) -> Result<PathBuf, DriverError> {
    let mut cmd = Command::new("cargo");
    cmd.arg("build").current_dir(root);
    if opts.release {
        cmd.arg("--release");
    }
    let output = cmd.output().map_err(map_cargo_spawn_error)?;

    if !output.status.success() {
        let combined = format!(
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        return Err(DriverError::CargoBuild {
            status: output.status.to_string(),
            combined,
        });
    }

    let profile_dir = if opts.release { "release" } else { "debug" };
    let mut exe = root.join("target").join(profile_dir).join(CRATE_NAME);
    if cfg!(windows) {
        exe.set_extension("exe");
    }
    if !exe.is_file() {
        return Err(DriverError::MissingBinary(exe));
    }
    Ok(exe)
}

pub(crate) fn map_cargo_spawn_error(e: io::Error) -> DriverError {
    if e.kind() == io::ErrorKind::NotFound {
        DriverError::CargoNotFound
    } else {
        DriverError::Io(e)
    }
}
