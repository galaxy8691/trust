use std::fs;
use std::path::{Path, PathBuf};

use crate::{DriverError, RustBuildOptions, CRATE_NAME};

pub(crate) fn write_minimal_crate(
    root: &Path,
    rust_source: &str,
    opts: &RustBuildOptions,
) -> Result<(), DriverError> {
    let stdlib_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../trust-stdlib");
    let stdlib_canon = stdlib_path.canonicalize().map_err(|_| {
        DriverError::TrustStdlibPathResolveFailed(stdlib_path.display().to_string())
    })?;
    let stdlib_toml = stdlib_canon.to_string_lossy().replace('\\', "/");
    let mut stdlib_features: Vec<&str> = Vec::new();
    if rust_source.contains("trust_stdlib::io::read_file_text_async") {
        stdlib_features.push("async-io");
    }
    if rust_source.contains("trust_stdlib::http::") {
        stdlib_features.push("http");
    }
    let trust_stdlib_dep = if stdlib_features.is_empty() {
        format!("trust_stdlib = {{ path = \"{stdlib_toml}\", default-features = false }}\n")
    } else {
        let joined = stdlib_features
            .iter()
            .map(|f| format!("\"{f}\""))
            .collect::<Vec<_>>()
            .join(", ");
        format!(
            "trust_stdlib = {{ path = \"{stdlib_toml}\", default-features = false, features = [{joined}] }}\n"
        )
    };

    let needs_async = rust_source.contains("#[tokio::main]");
    let needs_futures_util = rust_source.contains("futures_util");
    // URI / dynamic JSON number parsing live in `trust_stdlib`; only inject `serde_json` when the
    // emitted Rust still mentions `serde_json::` (e.g. object literals, `JSON.stringify` on objects).
    let serde_json_dep = if rust_source.contains("serde_json::") {
        "serde_json = \"1.0\"\n"
    } else {
        ""
    };
    let urlencoding_dep = if rust_source.contains("urlencoding::") {
        "urlencoding = \"2\"\n"
    } else {
        ""
    };
    let async_deps = if needs_async {
        let mut s = String::from(
            r#"tokio = { version = "1", features = ["rt-multi-thread", "macros", "fs"] }
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
    let trust_deps = opts.trust_dependency_lines.trim();
    let trust_block = if trust_deps.is_empty() {
        trust_stdlib_dep.clone()
    } else {
        format!("{trust_stdlib_dep}{trust_deps}\n")
    };

    let cargo_toml = if opts.link_trust_rt {
        let rt_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../trust_rt");
        let rt_canon = rt_path
            .canonicalize()
            .map_err(|_| DriverError::TrustRtPathResolveFailed(rt_path.display().to_string()))?;
        let path_toml = rt_canon.to_string_lossy().replace('\\', "/");
        format!(
            r#"[package]
name = "{name}"
version = "0.1.0"
edition = "2021"

[dependencies]
trust_rt = {{ path = "{path}", optional = true }}
{serde_json_dep}{urlencoding_dep}{async_deps}{trust_block}
[features]
default = []
trust_rt = ["dep:trust_rt"]
"#,
            name = CRATE_NAME,
            path = path_toml,
            serde_json_dep = serde_json_dep,
            urlencoding_dep = urlencoding_dep,
            async_deps = async_deps,
            trust_block = trust_block,
        )
    } else {
        format!(
            r#"[package]
name = "{name}"
version = "0.1.0"
edition = "2021"

[dependencies]
{serde_json_dep}{urlencoding_dep}{async_deps}{trust_block}"#,
            name = CRATE_NAME,
            serde_json_dep = serde_json_dep,
            urlencoding_dep = urlencoding_dep,
            async_deps = async_deps,
            trust_block = trust_block,
        )
    };
    fs::write(root.join("Cargo.toml"), cargo_toml)?;
    fs::create_dir_all(root.join("src"))?;
    fs::write(root.join("src/main.rs"), rust_source)?;
    Ok(())
}
