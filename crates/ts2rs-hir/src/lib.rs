//! TypeScript 高层 IR、语义检查与 Rust 代码生成。

mod build;
mod codegen;
mod error;
mod ir;
mod sem;

pub use build::{build_module, build_program_multi};
pub use codegen::{emit_rust, emit_rust_with_options, CodegenOptions};
pub use error::{CompileError, CompileWarning};
pub use ir::*;

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
    let mut module = build_module(program, cm, path)?;
    let warnings = sem::check_module(&mut module)?;
    let rust = emit_rust_with_options(&module, codegen)?;
    Ok((rust, warnings))
}

/// 多文件模块图：各单元为 `(source_path, Program, SourceMap)`，须与入口文件路径一致以便校验 `main`。
pub fn compile_graph(
    units: &[(String, Program, Lrc<SourceMap>)],
    entry_path: &str,
) -> Result<(String, Vec<CompileWarning>), CompileError> {
    compile_graph_with_options(units, entry_path, &CodegenOptions::default())
}

/// 多文件模块图：构建 IR 并完成语义检查（不生成 Rust）。
pub fn build_checked_module(
    units: &[(String, Program, Lrc<SourceMap>)],
    entry_path: &str,
) -> Result<(IRModule, Vec<CompileWarning>), CompileError> {
    let mut module = build_program_multi(units, entry_path)?;
    let warnings = sem::check_module(&mut module)?;
    Ok((module, warnings))
}

/// 仅语义检查（与 [`compile_graph`] 前半段相同，无 codegen）。
pub fn check_graph(
    units: &[(String, Program, Lrc<SourceMap>)],
    entry_path: &str,
) -> Result<Vec<CompileWarning>, CompileError> {
    let (_, warnings) = build_checked_module(units, entry_path)?;
    Ok(warnings)
}

pub fn compile_graph_with_options(
    units: &[(String, Program, Lrc<SourceMap>)],
    entry_path: &str,
    codegen: &CodegenOptions,
) -> Result<(String, Vec<CompileWarning>), CompileError> {
    let (module, warnings) = build_checked_module(units, entry_path)?;
    let rust = emit_rust_with_options(&module, codegen)?;
    Ok((rust, warnings))
}
