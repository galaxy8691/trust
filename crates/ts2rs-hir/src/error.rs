//! 编译错误与警告。用户可见诊断使用 [`diag`] / [`warn`]：位置取自 `span.lo` 在 `cm` 中的行列（与 CLI `path:line:col` 一致）。

use std::fmt;

use swc_common::{SourceMap, Span, Spanned};
use thiserror::Error;

/// 非致命警告（编译仍成功）。
#[derive(Debug, Clone)]
pub struct CompileWarning {
    pub path: String,
    pub line: usize,
    pub col: usize,
    pub message: String,
}

impl fmt::Display for CompileWarning {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}:{}:{}: {}",
            self.path, self.line, self.col, self.message
        )
    }
}

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

pub fn warn(cm: &SourceMap, path: &str, span: Span, message: impl Into<String>) -> CompileWarning {
    let loc = cm.lookup_char_pos(span.lo);
    CompileWarning {
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
