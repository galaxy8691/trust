// §设计文档 §8.4: 修复建议引擎
//!
//! 对常见错误（缺 `.clone()`、缺 `inout`、缺 `mut`）生成启发式修复建议。
//! Phase 1 为纯启发式——仅凭 Diagnostic 信号匹配，不保证建议 100% 正确（D13 fix）。

use crate::diagnostic::{Diagnostic, ErrorCode, FixSuggestion};

/// §8.4: 修复建议引擎入口
///
/// 注意（D13 fix）：函数仅凭 Diagnostic 信号做启发式匹配。
/// 准确修复需编译器上下文（类型是否 Clone、变量声明位置等）——押后 Phase 2 增强。
///
/// ```
/// # use trust_error::diagnostic::{Diagnostic, ErrorCode, SourceSpan};
/// # use trust_error::fix_suggest::suggest_fixes;
/// let span = SourceSpan::new("test.trust", 3, 5, 3, 6);
/// let diag = Diagnostic::error(ErrorCode::E0382, "use after move", span);
/// let fixes = suggest_fixes(&diag);
/// assert!(!fixes.is_empty());
/// assert_eq!(fixes[0].message, "consider cloning before move");
/// ```
pub fn suggest_fixes(diagnostic: &Diagnostic) -> Vec<FixSuggestion> {
    match diagnostic.code {
        // §3.3.1 规则 1: 缺 `.clone()` → E0382
        ErrorCode::E0382 => {
            let span = diagnostic.primary_span.clone();
            let var_name = extract_var_name(&diagnostic.message);
            vec![FixSuggestion::new(
                "consider cloning before move",
                span,
                format!("{}.clone()", var_name),
            )]
        }
        // §3.3.1 规则 2: 缺 `inout` 标注 → E0700 (D8 fix)
        ErrorCode::E0700 => {
            let span = diagnostic.primary_span.clone();
            let var_name = extract_var_name(&diagnostic.message);
            vec![FixSuggestion::new("add `inout` annotation", span, format!("inout {}", var_name))]
        }
        // §3.3.1 规则 3: 缺 `mut` → E0389
        ErrorCode::E0389 => {
            let span = diagnostic.primary_span.clone();
            let var_name = extract_var_name(&diagnostic.message);
            vec![FixSuggestion::new(
                "consider making variable mutable",
                span,
                format!("let mut {} = ...", var_name),
            )]
        }
        _ => vec![],
    }
}

/// 从诊断消息中启发式提取变量名
fn extract_var_name(message: &str) -> &str {
    // 简单启发式：消息中第一个引号内的词
    if let Some(start) = message.find('`') {
        let rest = &message[start + 1..];
        if let Some(end) = rest.find('`') {
            return &rest[..end];
        }
    }
    // 回退：消息中第一个单词
    message.split_whitespace().next().unwrap_or("x")
}

// ============================================================================
// 单元测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagnostic::{Diagnostic, ErrorCode, SourceSpan};

    #[test]
    fn suggest_clone_for_e0382() {
        let span = SourceSpan::new("test.trust", 3, 5, 3, 6);
        let diag = Diagnostic::error(ErrorCode::E0382, "use of moved value `x`", span);
        let fixes = suggest_fixes(&diag);
        assert_eq!(fixes.len(), 1);
        assert!(fixes[0].replacement.contains("clone"));
    }

    #[test]
    fn suggest_inout_for_e0700() {
        let span = SourceSpan::new("test.trust", 4, 10, 4, 15);
        let diag =
            Diagnostic::error(ErrorCode::E0700, "missing `inout` annotation for `data`", span);
        let fixes = suggest_fixes(&diag);
        assert_eq!(fixes.len(), 1);
        assert!(fixes[0].replacement.contains("inout"));
    }

    #[test]
    fn suggest_mut_for_e0389() {
        let span = SourceSpan::new("test.trust", 3, 5, 3, 6);
        let diag =
            Diagnostic::error(ErrorCode::E0389, "cannot assign to immutable variable `x`", span);
        let fixes = suggest_fixes(&diag);
        assert_eq!(fixes.len(), 1);
        assert!(fixes[0].replacement.contains("mut"));
    }

    #[test]
    fn no_suggestion_for_unknown_code() {
        let span = SourceSpan::new("test.trust", 1, 1, 1, 5);
        let diag = Diagnostic::error(ErrorCode::E9999, "ICE", span);
        let fixes = suggest_fixes(&diag);
        assert!(fixes.is_empty());
    }

    #[test]
    fn no_suggestion_for_empty_diagnostic_list() {
        let span = SourceSpan::new("test.trust", 1, 1, 1, 5);
        let diag = Diagnostic::warning(ErrorCode::E0425, "unused", span);
        let fixes = suggest_fixes(&diag);
        assert!(fixes.is_empty());
    }
}
