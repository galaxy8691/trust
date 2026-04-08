use std::fs;
use std::path::{Path, PathBuf};

use crate::{DriverError, RustBuildOptions, CRATE_NAME};

pub(crate) fn write_minimal_crate(
    root: &Path,
    rust_source: &str,
    opts: &RustBuildOptions,
) -> Result<(), DriverError> {
    let cargo_toml = if opts.link_ts2rs_rt {
        let rt_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../ts2rs_rt");
        let rt_canon = rt_path
            .canonicalize()
            .map_err(|_| DriverError::Ts2rsRtPathResolveFailed(rt_path.display().to_string()))?;
        let path_toml = rt_canon.to_string_lossy().replace('\\', "/");
        format!(
            r#"[package]
name = "{name}"
version = "0.1.0"
edition = "2021"

[dependencies]
ts2rs_rt = {{ path = "{path}", optional = true }}

[features]
default = []
ts2rs_rt = ["dep:ts2rs_rt"]
"#,
            name = CRATE_NAME,
            path = path_toml,
        )
    } else {
        format!(
            r#"[package]
name = "{name}"
version = "0.1.0"
edition = "2021"

[dependencies]
"#,
            name = CRATE_NAME
        )
    };
    fs::write(root.join("Cargo.toml"), cargo_toml)?;
    fs::create_dir_all(root.join("src"))?;
    fs::write(root.join("src/main.rs"), rust_source)?;
    Ok(())
}
