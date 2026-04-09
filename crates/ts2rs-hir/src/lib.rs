//! TypeScript 高层 IR、语义检查与 Rust 代码生成。

mod build;
mod codegen;
mod error;
mod ir;
mod ir_cache;
mod json_parse_fold;
mod sem;

pub use build::{
    build_module, build_module_ir_fragment, build_program_multi, merge_module_ir_fragments,
    ModuleIrFragment,
};
pub use codegen::{emit_rust, emit_rust_with_options, CodegenOptions};
pub use error::{CompileError, CompileWarning};
pub use ir::*;
pub use ir_cache::{
    decode_fragment_from_bytes, encode_fragment_to_bytes, source_map_for_path, IrCacheError,
    SCHEMA_VERSION,
};
pub use ts2rs_trust_manifest::TrustManifest;

pub use swc_common::comments::SingleThreadedComments;
use swc_common::sync::Lrc;
use swc_common::SourceMap;
use swc_ecma_ast::Program;

pub fn compile(
    program: &Program,
    cm: &Lrc<SourceMap>,
    path: &str,
) -> Result<(String, Vec<CompileWarning>), CompileError> {
    compile_with_options(program, cm, path, &CodegenOptions::default())
}

pub fn compile_with_options(
    program: &Program,
    cm: &Lrc<SourceMap>,
    path: &str,
    codegen: &CodegenOptions,
) -> Result<(String, Vec<CompileWarning>), CompileError> {
    let mut module = build_module(program, cm, path, None)?;
    let warnings = sem::check_module(&mut module)?;
    let rust = emit_rust_with_options(&module, codegen)?;
    Ok((rust, warnings))
}

/// 多文件模块图：各单元为 `(source_path, Program, SourceMap, SingleThreadedComments)`（与 [`ts2rs_parser::ParsedModuleGraph::compile_units`] 一致）。
pub fn compile_graph(
    units: &[(String, Program, Lrc<SourceMap>, SingleThreadedComments)],
    entry_path: &str,
) -> Result<(String, Vec<CompileWarning>), CompileError> {
    compile_graph_with_options(units, entry_path, None, &CodegenOptions::default())
}

/// 多文件模块图：构建 IR 并完成语义检查（不生成 Rust）。
pub fn build_checked_module(
    units: &[(String, Program, Lrc<SourceMap>, SingleThreadedComments)],
    entry_path: &str,
    trust: Option<&TrustManifest>,
) -> Result<(IRModule, Vec<CompileWarning>), CompileError> {
    let mut module = build_program_multi(units, entry_path, trust)?;
    let warnings = sem::check_module(&mut module)?;
    Ok((module, warnings))
}

/// 仅语义检查（与 [`compile_graph`] 前半段相同，无 codegen）。
pub fn check_graph(
    units: &[(String, Program, Lrc<SourceMap>, SingleThreadedComments)],
    entry_path: &str,
) -> Result<Vec<CompileWarning>, CompileError> {
    check_graph_with_trust(units, entry_path, None)
}

/// 与 [`check_graph`] 相同，但传入 `Trust.toml` 清单（若有）。
pub fn check_graph_with_trust(
    units: &[(String, Program, Lrc<SourceMap>, SingleThreadedComments)],
    entry_path: &str,
    trust: Option<&TrustManifest>,
) -> Result<Vec<CompileWarning>, CompileError> {
    let mut module = build_program_multi(units, entry_path, trust)?;
    sem::check_module(&mut module)
}

pub fn compile_graph_with_options(
    units: &[(String, Program, Lrc<SourceMap>, SingleThreadedComments)],
    entry_path: &str,
    trust: Option<&TrustManifest>,
    codegen: &CodegenOptions,
) -> Result<(String, Vec<CompileWarning>), CompileError> {
    let (module, warnings) = build_checked_module(units, entry_path, trust)?;
    let rust = emit_rust_with_options(&module, codegen)?;
    Ok((rust, warnings))
}

/// 已构建的模块片段按图顺序合并后：语义检查 + Rust 生成（与全量 [`compile_graph_with_options`] 等价）。
pub fn compile_merged_fragments_with_options(
    fragments: &[(String, ModuleIrFragment)],
    entry_path: &str,
    trust: Option<std::sync::Arc<TrustManifest>>,
    codegen: &CodegenOptions,
) -> Result<(String, Vec<CompileWarning>), CompileError> {
    let mut module = crate::build::merge_module_ir_fragments(fragments, entry_path, trust)?;
    let warnings = sem::check_module(&mut module)?;
    let rust = emit_rust_with_options(&module, codegen)?;
    Ok((rust, warnings))
}
