use std::path::Path;

use crate::DriverError;

/// 从单个 TypeScript 入口文件编译：解析器构建相对路径 `import` 的**模块图**（不合并 AST），
/// 校验导出后，经 HIR 生成 Rust，写入临时 crate 并 `cargo build --release`。
///
/// 成功时返回 `(TempDir, PathBuf)`：**先**为临时目录句柄（须保持存活至用完二进制），**后**为
/// `target/release/ts2rs_generated`（Windows 下为 `.exe`）。详见 [crate 级说明](crate#临时目录与-tempdir)。
pub(crate) fn compile_entrypoint_to_executable_impl(
    path: &Path,
) -> Result<(tempfile::TempDir, std::path::PathBuf), DriverError> {
    let graph = ts2rs_parser::parse_module_graph(path)?;
    ts2rs_parser::validate_imports(&graph)?;
    let units = graph.compile_units();
    let entry_path = graph.entry_path_str();
    let (rust, warnings) = ts2rs_lower::lower_module_graph(&units, &entry_path)?;
    for w in &warnings {
        eprintln!("warning: {w}");
    }
    super::build_rust_to_executable(&rust)
}
