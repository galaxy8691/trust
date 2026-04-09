use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use wait_timeout::ChildExt;

use crate::{DriverError, RustBuildOptions, CRATE_NAME};

pub(crate) fn cargo_build(root: &Path, opts: &RustBuildOptions) -> Result<PathBuf, DriverError> {
    let mut cmd = Command::new("cargo");
    cmd.arg("build").current_dir(root);
    if opts.release {
        cmd.arg("--release");
    }
    cmd.stdout(Stdio::piped()).stderr(Stdio::piped());

    let mut child = cmd.spawn().map_err(map_cargo_spawn_error)?;

    let status = match opts.cargo_timeout {
        Some(limit) => match child.wait_timeout(limit).map_err(DriverError::Io)? {
            None => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(DriverError::CargoTimeout { limit });
            }
            Some(st) => st,
        },
        None => child.wait().map_err(DriverError::Io)?,
    };

    let max = opts.max_cargo_output_bytes;
    let stdout = read_child_stream_limited(child.stdout.take(), max, "stdout")?;
    let stderr = read_child_stream_limited(child.stderr.take(), max, "stderr")?;

    if !status.success() {
        let combined = format!(
            "{}{}",
            String::from_utf8_lossy(&stdout),
            String::from_utf8_lossy(&stderr)
        );
        return Err(DriverError::CargoBuild {
            status: status.to_string(),
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

/// Reads from `stream` into a buffer, respecting `max` per-stream when `Some`.
fn read_child_stream_limited(
    stream: Option<impl Read>,
    max: Option<usize>,
    stream_name: &'static str,
) -> Result<Vec<u8>, DriverError> {
    let Some(mut r) = stream else {
        return Ok(Vec::new());
    };
    match max {
        None => {
            let mut v = Vec::new();
            r.read_to_end(&mut v).map_err(DriverError::Io)?;
            Ok(v)
        }
        Some(limit) => {
            let mut out = Vec::new();
            let mut buf = [0u8; 8192];
            loop {
                if out.len() >= limit {
                    let n = r.read(&mut buf).map_err(DriverError::Io)?;
                    if n > 0 {
                        return Err(DriverError::CargoOutputTruncated {
                            max_bytes: limit,
                            stream: stream_name,
                        });
                    }
                    return Ok(out);
                }
                let room = limit - out.len();
                let n = r.read(&mut buf).map_err(DriverError::Io)?;
                if n == 0 {
                    return Ok(out);
                }
                let take = n.min(room);
                out.extend_from_slice(&buf[..take]);
                if n > take {
                    return Err(DriverError::CargoOutputTruncated {
                        max_bytes: limit,
                        stream: stream_name,
                    });
                }
            }
        }
    }
}

pub(crate) fn map_cargo_spawn_error(e: io::Error) -> DriverError {
    if e.kind() == io::ErrorKind::NotFound {
        DriverError::CargoNotFound
    } else {
        DriverError::Io(e)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn read_limited_truncates_with_err_when_more_data() {
        let data = vec![b'a'; 16];
        let err =
            read_child_stream_limited(Some(Cursor::new(data)), Some(4), "stdout").unwrap_err();
        assert!(
            matches!(
                err,
                DriverError::CargoOutputTruncated {
                    max_bytes: 4,
                    stream: "stdout"
                }
            ),
            "{err:?}"
        );
    }

    #[test]
    fn read_limited_ok_under_cap() {
        let data = b"hi".to_vec();
        let v = read_child_stream_limited(Some(Cursor::new(data)), Some(100), "stderr").unwrap();
        assert_eq!(v, b"hi");
    }
}
