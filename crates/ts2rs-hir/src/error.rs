//! 编译错误。用户可见诊断使用 [`diag`]：位置取自 `span.lo` 在 `cm` 中的行列（与 CLI `path:line:col` 一致）。

use swc_common::{SourceMap, Span, Spanned};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum CompileError {
    #[error("{path}:{line}:{col}: {message}")]
    Diag {
        path: String,
        line: usize,
        col: usize,
        message: String,
    },
}

pub fn diag(cm: &SourceMap, path: &str, span: Span, message: impl Into<String>) -> CompileError {
    let loc = cm.lookup_char_pos(span.lo);
    CompileError::Diag {
        path: path.to_string(),
        line: loc.line,
        col: loc.col_display,
        message: message.into(),
    }
}

pub fn diag_spanned(
    cm: &SourceMap,
    path: &str,
    spanned: &impl Spanned,
    message: impl Into<String>,
) -> CompileError {
    diag(cm, path, spanned.span(), message)
}
