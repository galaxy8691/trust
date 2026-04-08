use std::fs;
use std::path::{Path, PathBuf};

use crate::{DriverError, RustBuildOptions, CRATE_NAME};

pub(crate) fn write_minimal_crate(
    root: &Path,
    rust_source: &str,
    opts: &RustBuildOptions,
) -> Result<(), DriverError> {
    let needs_async = rust_source.contains("#[tokio::main]");
    let needs_futures_util = rust_source.contains("futures_util");
    let async_deps = if needs_async {
        let mut s = String::from(
            r#"tokio = { version = "1", features = ["rt-multi-thread", "macros"] }
reqwest = { version = "0.12", default-features = false, features = ["rustls-tls"] }
"#,
        );
        if needs_futures_util {
            s.push_str("futures-util = { version = \"0.3\", default-features = false, features = [\"std\"] }\n");
        }
        s
    } else {
        String::new()
    };

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
{async_deps}
[features]
default = []
ts2rs_rt = ["dep:ts2rs_rt"]
"#,
            name = CRATE_NAME,
            path = path_toml,
            async_deps = async_deps,
        )
    } else {
        format!(
            r#"[package]
name = "{name}"
version = "0.1.0"
edition = "2021"

[dependencies]
{async_deps}"#,
            name = CRATE_NAME,
            async_deps = async_deps,
        )
    };
    fs::write(root.join("Cargo.toml"), cargo_toml)?;
    fs::create_dir_all(root.join("src"))?;
    fs::write(root.join("src/main.rs"), rust_source)?;
    Ok(())
}
