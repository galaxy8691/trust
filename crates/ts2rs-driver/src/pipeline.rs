use std::path::Path;

use crate::{DriverError, RustBuildOptions};

/// 从单个 TypeScript 入口文件编译：解析器构建相对路径 `import` 的**模块图**（不合并 AST），
/// 校验导出后，经 HIR 生成 Rust，写入临时 crate 并 `cargo build --release`。
///
/// 成功时返回 `(TempDir, PathBuf)`：**先**为临时目录句柄（须保持存活至用完二进制），**后**为
/// `target/release/ts2rs_generated`（Windows 下为 `.exe`）。详见 [crate 级说明](crate#临时目录与-tempdir)。
pub(crate) fn compile_entrypoint_to_executable_impl(
    path: &Path,
) -> Result<(tempfile::TempDir, std::path::PathBuf), DriverError> {
    let trust = match ts2rs_trust_manifest::discover_trust_toml(path) {
        Some(p) => Some(ts2rs_trust_manifest::TrustManifest::load(&p)?),
        None => None,
    };
    let graph = ts2rs_parser::parse_module_graph_with_trust(path, &[], trust.as_ref())?;
    ts2rs_parser::validate_imports(&graph)?;
    let units = graph.compile_units();
    let entry_path = graph.entry_path_str();
    let codegen = ts2rs_hir::CodegenOptions::default();
    let (rust, warnings) = ts2rs_lower::lower_module_graph_with_options(
        &units,
        &entry_path,
        trust.as_ref(),
        &codegen,
    )?;
    for w in &warnings {
        eprintln!("warning: {w}");
    }
    let mut opts = RustBuildOptions::default();
    if let Some(ref m) = trust {
        opts.trust_dependency_lines = m.cargo_dependency_lines.clone();
    }
    super::build_rust_to_executable_with_options(&rust, &opts)
}
