//! 将 Rust 源码写入临时 crate 并调用 `cargo build --release`。

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use tempfile::TempDir;
use thiserror::Error;

const CRATE_NAME: &str = "ts2rs_generated";

#[derive(Debug, Error)]
pub enum DriverError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),

    #[error("cargo build failed (status {status}):\n{combined}")]
    CargoBuild { status: String, combined: String },

    #[error("built binary not found at {0}")]
    MissingBinary(PathBuf),

    #[error(transparent)]
    Parse(#[from] ts2rs_parser::ParseError),

    #[error(transparent)]
    Lower(#[from] ts2rs_lower::LowerError),
}

/// 从单个 TypeScript 入口文件编译：解析器构建相对路径 `import` 的**模块图**（不合并 AST），
/// 校验导出后，经 HIR 生成 Rust，写入临时 crate 并 `cargo build --release`。
pub fn compile_entrypoint_to_executable(path: &Path) -> Result<(TempDir, PathBuf), DriverError> {
    let graph = ts2rs_parser::parse_module_graph(path)?;
    ts2rs_parser::validate_imports(&graph)?;
    let units = graph.compile_units();
    let entry_path = graph.entry_path_str();
    let rust = ts2rs_lower::lower_module_graph(&units, &entry_path)?;
    build_rust_to_executable(&rust)
}

/// 在临时目录中生成 crate、编译，返回 **可执行文件路径** 与 **临时目录句柄**。
/// 在丢弃 `TempDir` 之前必须已使用完可执行文件（或已复制到别处）。
pub fn build_rust_to_executable(rust_source: &str) -> Result<(TempDir, PathBuf), DriverError> {
    let dir = tempfile::tempdir()?;
    let root = dir.path();

    write_minimal_crate(root, rust_source)?;

    let output = Command::new("cargo")
        .args(["build", "--release"])
        .current_dir(root)
        .output()?;

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

    let mut exe = root.join("target/release").join(CRATE_NAME);
    if cfg!(windows) {
        exe.set_extension("exe");
    }
    if !exe.is_file() {
        return Err(DriverError::MissingBinary(exe));
    }

    Ok((dir, exe))
}

/// 编译并将可执行文件复制到 `output`（覆盖已存在文件）。
pub fn build_rust_and_copy(rust_source: &str, output: &Path) -> Result<(), DriverError> {
    let (_dir, exe) = build_rust_to_executable(rust_source)?;
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::copy(&exe, output)?;
    Ok(())
}

fn write_minimal_crate(root: &Path, rust_source: &str) -> Result<(), DriverError> {
    let cargo_toml = format!(
        r#"[package]
name = "{name}"
version = "0.1.0"
edition = "2021"

[dependencies]
"#,
        name = CRATE_NAME
    );
    fs::write(root.join("Cargo.toml"), cargo_toml)?;
    fs::create_dir_all(root.join("src"))?;
    fs::write(root.join("src/main.rs"), rust_source)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;

    #[test]
    fn entrypoint_resolves_import_and_builds() {
        let dir = tempfile::tempdir().expect("tempdir");
        let dep = dir.path().join("lib.ts");
        let main = dir.path().join("app.ts");
        fs::write(
            &dep,
            "export function add(a: number, b: number): number { return a + b; }\n",
        )
        .unwrap();
        fs::write(
            &main,
            "import { add } from \"./lib.ts\";\nexport function main(): number { return add(1, 2); }\n",
        )
        .unwrap();

        let (_tmp, exe) = compile_entrypoint_to_executable(&main).expect("compile");
        let out = Command::new(&exe).output().expect("run");
        assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
        assert_eq!(String::from_utf8_lossy(&out.stdout), "3\n");
    }
}
