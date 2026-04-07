//! TypeScript 高层 IR、语义检查与 Rust 代码生成。

mod build;
mod codegen;
mod error;
mod ir;
mod sem;

pub use build::{build_module, build_program_multi};
pub use codegen::emit_rust;
pub use error::CompileError;
pub use ir::*;

use swc_common::sync::Lrc;
use swc_common::SourceMap;
use swc_ecma_ast::Program;

pub fn compile(program: &Program, cm: &Lrc<SourceMap>, path: &str) -> Result<String, CompileError> {
    let mut module = build_module(program, cm, path)?;
    sem::check_module(&mut module)?;
    emit_rust(&module)
}

/// 多文件模块图：各单元为 `(source_path, Program, SourceMap)`，须与入口文件路径一致以便校验 `main`。
pub fn compile_graph(
    units: &[(String, Program, Lrc<SourceMap>)],
    entry_path: &str,
) -> Result<String, CompileError> {
    let mut module = build_program_multi(units, entry_path)?;
    sem::check_module(&mut module)?;
    emit_rust(&module)
}
