//! 将 Rust 源码写入**临时目录**中的最小 crate，并调用 `cargo build`（默认 `--release`，见 [`RustBuildOptions::release`]）得到可执行文件。
//!
//! # 临时目录与 [`TempDir`]
//!
//! [`build_rust_to_executable`] / [`compile_entrypoint_to_executable`] 返回的 [`TempDir`] 在 **drop 时会删除整个临时目录**
//!（含 `target/` 与生成好的二进制）。调用方必须在**仍持有** `TempDir` 时运行可执行文件，或先用 [`std::fs::copy`]
//! 复制到持久路径后再丢弃句柄。常见模式：`let (_dir, exe) = build_rust_to_executable(...)?` 仅在同一进程内紧接着
//! `Command::new(&exe)`；若只需输出文件，优先 [`build_rust_and_copy`]（内部复制后丢弃临时目录）。
//!
//! # `cargo` 与构建失败
//!
//! 本 crate 依赖本机 **`cargo` 在 `PATH` 中**。若无法启动 `cargo`（例如未安装 Rust），将返回 [`DriverError::CargoNotFound`]。
//! 若 `cargo` 已启动但编译失败（含离线网络导致无法拉取依赖等），错误为 [`DriverError::CargoBuild`]，其中 `combined`
//! 含 stdout/stderr，便于排查。

use std::fs;
use std::path::{Path, PathBuf};

use tempfile::TempDir;
use thiserror::Error;

mod cargo_runner;
mod crate_writer;
mod pipeline;

const CRATE_NAME: &str = "ts2rs_generated";

#[derive(Debug, Error)]
pub enum DriverError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),

    #[error("cargo not found in PATH; install a Rust toolchain (https://rustup.rs/)")]
    CargoNotFound,

    #[error("cargo build failed (status {status}):\n{combined}")]
    CargoBuild { status: String, combined: String },

    #[error("built binary not found at {0}")]
    MissingBinary(PathBuf),

    #[error(transparent)]
    Parse(#[from] ts2rs_parser::ParseError),

    #[error(transparent)]
    Lower(#[from] ts2rs_lower::LowerError),

    #[error("cannot resolve ts2rs_rt path dependency; build from the ts2rs source tree or omit --link-ts2rs-rt (looked for {0})")]
    Ts2rsRtPathResolveFailed(String),
}

/// Options for generated temporary crate / `cargo build` (e.g. optional `ts2rs_rt` path dependency).
#[derive(Debug, Clone)]
pub struct RustBuildOptions {
    /// When true, `Cargo.toml` includes an optional path dependency on `ts2rs_rt` and a matching feature.
    pub link_ts2rs_rt: bool,
    /// When true (default), runs `cargo build --release` and uses `target/release/`; when false, `cargo build` and `target/debug/`.
    pub release: bool,
}

impl Default for RustBuildOptions {
    fn default() -> Self {
        Self {
            link_ts2rs_rt: false,
            release: true,
        }
    }
}

/// 从单个 TypeScript 入口文件编译：解析器构建相对路径 `import` 的**模块图**（不合并 AST），
/// 校验导出后，经 HIR 生成 Rust，写入临时 crate 并 `cargo build --release`。
///
/// 成功时返回 `(TempDir, PathBuf)`：**先**为临时目录句柄（须保持存活至用完二进制），**后**为
/// `target/release/ts2rs_generated`（Windows 下为 `.exe`）。详见 [crate 级说明](crate#临时目录与-tempdir)。
pub fn compile_entrypoint_to_executable(path: &Path) -> Result<(TempDir, PathBuf), DriverError> {
    pipeline::compile_entrypoint_to_executable_impl(path)
}

/// 在临时目录中生成 crate、编译，返回 `(TempDir, PathBuf)`：
/// **临时目录句柄**（保活目录）与 **`target/release/ts2rs_generated`** 可执行路径（Windows 带 `.exe`）。
///
/// 丢弃 `TempDir` 前须已运行或可执行文件已复制。若只需要写入固定路径，用 [`build_rust_and_copy`]。
pub fn build_rust_to_executable(rust_source: &str) -> Result<(TempDir, PathBuf), DriverError> {
    build_rust_to_executable_with_options(rust_source, &RustBuildOptions::default())
}

/// Same as [`build_rust_to_executable`], with [`RustBuildOptions`] (e.g. optional `ts2rs_rt` in `Cargo.toml`).
pub fn build_rust_to_executable_with_options(
    rust_source: &str,
    opts: &RustBuildOptions,
) -> Result<(TempDir, PathBuf), DriverError> {
    let dir = tempfile::tempdir()?;
    let root = dir.path();

    crate_writer::write_minimal_crate(root, rust_source, opts)?;
    let exe = cargo_runner::cargo_build(root, opts)?;

    Ok((dir, exe))
}

/// 编译并将可执行文件复制到 `output`（覆盖已存在文件）。
///
/// 内部调用 [`build_rust_to_executable`] 后立刻复制并丢弃临时目录，适合不需要长期保留 `TempDir` 的场景。
pub fn build_rust_and_copy(rust_source: &str, output: &Path) -> Result<(), DriverError> {
    let (_dir, exe) = build_rust_to_executable(rust_source)?;
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::copy(&exe, output)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;

    #[test]
    fn write_minimal_crate_with_link_ts2rs_rt_contains_optional_path_dep() {
        let dir = tempfile::tempdir().expect("tempdir");
        let opts = RustBuildOptions {
            link_ts2rs_rt: true,
            ..Default::default()
        };
        crate_writer::write_minimal_crate(dir.path(), "fn main() {}", &opts).expect("write crate");
        let toml = fs::read_to_string(dir.path().join("Cargo.toml")).expect("read Cargo.toml");
        assert!(
            toml.contains("ts2rs_rt") && toml.contains("optional = true"),
            "unexpected Cargo.toml:\n{toml}"
        );
        assert!(toml.contains("[features]") && toml.contains("dep:ts2rs_rt"));
    }

    #[test]
    fn debug_build_writes_binary_under_target_debug() {
        let (_dir, exe) = build_rust_to_executable_with_options(
            "fn main() { println!(\"ok\"); }",
            &RustBuildOptions {
                release: false,
                ..Default::default()
            },
        )
        .expect("debug build");
        assert!(
            exe.to_string_lossy().contains("debug"),
            "expected debug profile path, got {}",
            exe.display()
        );
        let out = Command::new(&exe).output().expect("run");
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        assert_eq!(String::from_utf8_lossy(&out.stdout), "ok\n");
    }

    #[test]
    fn map_cargo_spawn_error_maps_not_found_to_cargo_not_found() {
        let io_err = Command::new("this_binary_must_not_exist_ts2rs_6_1")
            .output()
            .expect_err("missing executable should yield io error");
        assert_eq!(io_err.kind(), std::io::ErrorKind::NotFound);
        let mapped = cargo_runner::map_cargo_spawn_error(io_err);
        assert!(matches!(mapped, DriverError::CargoNotFound), "{mapped}");
    }

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
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        assert_eq!(String::from_utf8_lossy(&out.stdout), "3\n");
    }
}
