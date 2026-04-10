//! 编译错误与警告。用户可见诊断使用 [`diag`] / [`warn`]：位置取自 `span.lo` 在 `cm` 中的行列（与 CLI `path:line:col` 一致）。

use std::fmt;

use swc_common::{SourceMap, Span, Spanned};

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

/// 编译失败：单条 [`CompileError::Diag`] 或多条 [`CompileError::Many`]。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompileError {
    Diag {
        path: String,
        line: usize,
        col: usize,
        message: String,
    },
    Many(Vec<CompileError>),
}

impl CompileError {
    /// 展开嵌套的 `Many`。
    pub fn flatten(self) -> Vec<CompileError> {
        match self {
            CompileError::Many(inner) => {
                inner.into_iter().flat_map(CompileError::flatten).collect()
            }
            other => vec![other],
        }
    }

    fn sort_key(&self) -> (String, usize, usize, String) {
        match self {
            CompileError::Diag {
                path,
                line,
                col,
                message,
            } => (path.clone(), *line, *col, message.clone()),
            CompileError::Many(_) => (String::new(), 0, 0, String::new()),
        }
    }

    /// 合并、按位置排序并去重；仅一条则返回 `Diag`，否则 `Many`。
    pub fn merge_sorted(items: Vec<CompileError>) -> CompileError {
        let mut flat: Vec<CompileError> =
            items.into_iter().flat_map(CompileError::flatten).collect();
        flat.retain(|e| !matches!(e, CompileError::Many(_)));
        flat.sort_by_key(|a| a.sort_key());
        flat.dedup_by(|a, b| a.sort_key() == b.sort_key());
        match flat.len() {
            0 => CompileError::Diag {
                path: String::new(),
                line: 0,
                col: 0,
                message: "compile failed".to_string(),
            },
            1 => flat.pop().expect("len 1"),
            _ => CompileError::Many(flat),
        }
    }
}

impl fmt::Display for CompileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CompileError::Diag {
                path,
                line,
                col,
                message,
            } => write!(f, "{path}:{line}:{col}: {message}"),
            CompileError::Many(items) => {
                for (i, e) in items.iter().enumerate() {
                    if i > 0 {
                        writeln!(f)?;
                    }
                    write!(f, "{e}")?;
                }
                Ok(())
            }
        }
    }
}

impl std::error::Error for CompileError {}

/// Appends a short note that other diagnostics may appear after the user fixes and recompiles.
pub fn with_monomorphization_followup(err: CompileError) -> CompileError {
    append_followup_to_diagnostics(
        err,
        " Further semantic errors are not reported until monomorphization succeeds; fix and recompile.",
    )
}

/// Same idea for the first codegen failure (codegen is otherwise fail-fast).
pub fn with_codegen_followup(err: CompileError) -> CompileError {
    append_followup_to_diagnostics(
        err,
        " Further codegen errors may appear after this one is fixed; recompile to see them.",
    )
}

fn append_followup_to_diagnostics(err: CompileError, note: &'static str) -> CompileError {
    match err {
        CompileError::Diag {
            path,
            line,
            col,
            message,
        } => CompileError::Diag {
            path,
            line,
            col,
            message: format!("{message}{note}"),
        },
        CompileError::Many(items) => CompileError::Many(
            items
                .into_iter()
                .map(|e| append_followup_to_diagnostics(e, note))
                .collect(),
        ),
    }
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

/// 追加一条与 [`diag`] 相同格式的诊断（用于批量收集）。
pub fn push_diag(
    out: &mut Vec<CompileError>,
    cm: &SourceMap,
    path: &str,
    span: Span,
    message: impl Into<String>,
) {
    out.push(diag(cm, path, span, message));
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codegen_followup_appends_to_message() {
        let e = CompileError::Diag {
            path: "a.ts".into(),
            line: 1,
            col: 0,
            message: "oops".into(),
        };
        let CompileError::Diag { message, .. } = with_codegen_followup(e) else {
            panic!("expected Diag");
        };
        assert!(message.contains("oops"));
        assert!(message.contains("recompile"));
    }
}
