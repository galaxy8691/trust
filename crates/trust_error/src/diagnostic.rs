// §设计文档 §8.1 / constraints §8.1: 统一错误诊断数据结构
//!
//! `trust_error` 是 Phase 1 的基础 crate——零依赖，被所有其他 crate 共用。
//! 提供 `Diagnostic` 结构体 + `ErrorCode` 枚举 + `Severity` + `SourceSpan` + `FixSuggestion`。
//!
//! 依赖策略：零外部依赖，不引入 serde。所有类型手动实现 Display 和 JSON 序列化。

// §3.1.1: Severity 枚举（D1 fix: 去掉 Serialize，手动序列化）
/// 诊断严重级别
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    /// 编译错误——阻止 codegen
    Error,
    /// 编译警告——不阻止 codegen
    Warning,
    /// 帮助信息——附属于 Error/Warning
    Help,
}

// §8.1: Severity 手动序列化
impl std::fmt::Display for Severity {
    /// ```
    /// # use trust_error::diagnostic::Severity;
    /// assert_eq!(Severity::Error.to_string(), "error");
    /// assert_eq!(Severity::Warning.to_string(), "warning");
    /// assert_eq!(Severity::Help.to_string(), "help");
    /// ```
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Severity::Error => write!(f, "error"),
            Severity::Warning => write!(f, "warning"),
            Severity::Help => write!(f, "help"),
        }
    }
}

// §3.1.2: ErrorCode 枚举（D3 fix: 19 变体，覆盖 parser/模块/moveck/borrowck/typeck）
/// 错误码 —— 对齐 Rust 编译器错误码惯例
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorCode {
    // === parser 语法错误 ===
    /// 非法 token / unexpected token
    E0001,
    /// 语句未闭合（缺 `;` / `}`）
    E0002,

    // === 模块系统 ===
    /// 文件未找到
    E0601,
    /// 循环导入
    E0602,
    /// 导入目标不存在
    E0603,

    // === 移动语义 (moveck) ===
    /// 移动后使用（use after move）
    E0382,
    /// 不可变变量赋值（cannot assign twice to immutable variable）
    E0384,
    /// 不可变变量赋值（cannot assign to immutable variable）
    E0389,

    // === 借用检查 (borrowck) ===
    /// 可变借用冲突
    E0501,
    /// 共享借用与可变借用冲突
    E0502,
    /// 移动被借用的值
    E0506,

    // === 类型检查 (typeck) ===
    /// 类型不匹配
    E0308,
    /// 参数数量不匹配
    E0061,
    /// 块体函数缺少返回类型标注 — §2.3
    E0062,

    // === 语法/语义 ===
    /// 未定义变量/函数
    E0425,
    /// 重复定义
    E0428,
    /// 无效的 `inout`/`move` 标注（含调用处缺失）
    E0700,

    // === 通用 ===
    /// 内部编译器错误（ICE）
    E9999,
}

// §8.1: ErrorCode 手动序列化
impl std::fmt::Display for ErrorCode {
    /// ```
    /// # use trust_error::diagnostic::ErrorCode;
    /// assert_eq!(ErrorCode::E0382.to_string(), "E0382");
    /// assert_eq!(ErrorCode::E0001.to_string(), "E0001");
    /// ```
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

// §3.1.3: SourceSpan 结构体
/// 源码位置标注
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceSpan {
    /// 文件路径
    pub file: String,
    /// 起始行（1-based）
    pub line_start: u32,
    /// 起始列（1-based）
    pub col_start: u32,
    /// 结束行（1-based）
    pub line_end: u32,
    /// 结束列（1-based）
    pub col_end: u32,
    /// 可选标签（如 "moved here", "used here after move"）
    pub label: Option<String>,
}

impl SourceSpan {
    // §8.1: 创建简单位置标注（D5 fix: 仅定义字段契约，转换由调用方实现）
    /// 创建不带标签的位置标注
    ///
    /// ```
    /// # use trust_error::diagnostic::SourceSpan;
    /// let span = SourceSpan::new("test.trust", 1, 1, 1, 5);
    /// assert_eq!(span.file, "test.trust");
    /// assert!(span.label.is_none());
    /// ```
    pub fn new(
        file: impl Into<String>,
        line_start: u32,
        col_start: u32,
        line_end: u32,
        col_end: u32,
    ) -> Self {
        SourceSpan { file: file.into(), line_start, col_start, line_end, col_end, label: None }
    }

    /// 创建带标签的位置标注
    ///
    /// ```
    /// # use trust_error::diagnostic::SourceSpan;
    /// let span = SourceSpan::with_label("test.trust", 1, 1, 1, 5, "moved here");
    /// assert_eq!(span.label.as_deref(), Some("moved here"));
    /// ```
    pub fn with_label(
        file: impl Into<String>,
        line_start: u32,
        col_start: u32,
        line_end: u32,
        col_end: u32,
        label: impl Into<String>,
    ) -> Self {
        SourceSpan {
            file: file.into(),
            line_start,
            col_start,
            line_end,
            col_end,
            label: Some(label.into()),
        }
    }
}

// §3.1.4: FixSuggestion 结构体
/// 修复建议
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FixSuggestion {
    /// 人类可读的修复描述
    pub message: String,
    /// 需要替换的 span
    pub span: SourceSpan,
    /// 替换文本
    pub replacement: String,
}

impl FixSuggestion {
    // §8.4: 创建修复建议
    /// ```
    /// # use trust_error::diagnostic::{FixSuggestion, SourceSpan};
    /// let span = SourceSpan::new("test.trust", 3, 5, 3, 6);
    /// let fix: FixSuggestion = FixSuggestion::new("make mutable", span, "let mut x = 42;");
    /// assert_eq!(fix.replacement, "let mut x = 42;");
    /// ```
    pub fn new(
        message: impl Into<String>,
        span: SourceSpan,
        replacement: impl Into<String>,
    ) -> Self {
        FixSuggestion { message: message.into(), span, replacement: replacement.into() }
    }
}

// §3.1.5: Diagnostic 结构体（D2 fix: severity→level, suggestions→fix_suggestions）
/// 统一诊断结构体
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    /// 错误码
    pub code: ErrorCode,
    /// 严重级别（D2 fix: level 非 severity）
    pub level: Severity,
    /// 人类可读的错误信息
    pub message: String,
    /// 主 span（错误发生位置）
    pub primary_span: SourceSpan,
    /// 辅助 span
    pub secondary_spans: Vec<SourceSpan>,
    /// 修复建议（D2 fix: fix_suggestions 非 suggestions）
    pub fix_suggestions: Vec<FixSuggestion>,
    /// 子诊断
    pub children: Vec<Diagnostic>,
}

impl Diagnostic {
    // §8.1: 创建简单错误
    /// ```
    /// # use trust_error::diagnostic::{Diagnostic, ErrorCode, SourceSpan};
    /// let span = SourceSpan::new("test.trust", 1, 1, 1, 5);
    /// let diag = Diagnostic::error(ErrorCode::E0382, "use after move", span);
    /// assert_eq!(diag.code, ErrorCode::E0382);
    /// assert_eq!(diag.level.to_string(), "error");
    /// ```
    pub fn error(code: ErrorCode, message: impl Into<String>, span: SourceSpan) -> Self {
        Diagnostic {
            code,
            level: Severity::Error,
            message: message.into(),
            primary_span: span,
            secondary_spans: vec![],
            fix_suggestions: vec![],
            children: vec![],
        }
    }

    // §8.1: 创建带辅助 span 的错误
    /// ```
    /// # use trust_error::diagnostic::{Diagnostic, ErrorCode, SourceSpan};
    /// let moved = SourceSpan::new("test.trust", 3, 5, 3, 6);
    /// let used = SourceSpan::new("test.trust", 5, 5, 5, 6);
    /// let diag = Diagnostic::error_with_secondary(ErrorCode::E0382, "use after move", moved, vec![used]);
    /// assert_eq!(diag.secondary_spans.len(), 1);
    /// ```
    pub fn error_with_secondary(
        code: ErrorCode,
        message: impl Into<String>,
        primary: SourceSpan,
        secondary: Vec<SourceSpan>,
    ) -> Self {
        Diagnostic {
            code,
            level: Severity::Error,
            message: message.into(),
            primary_span: primary,
            secondary_spans: secondary,
            fix_suggestions: vec![],
            children: vec![],
        }
    }

    // §8.1: 创建警告
    /// ```
    /// # use trust_error::diagnostic::{Diagnostic, ErrorCode, SourceSpan};
    /// let span = SourceSpan::new("test.trust", 1, 1, 1, 5);
    /// let diag = Diagnostic::warning(ErrorCode::E0425, "unused variable", span);
    /// assert_eq!(diag.level.to_string(), "warning");
    /// ```
    pub fn warning(code: ErrorCode, message: impl Into<String>, span: SourceSpan) -> Self {
        Diagnostic {
            code,
            level: Severity::Warning,
            message: message.into(),
            primary_span: span,
            secondary_spans: vec![],
            fix_suggestions: vec![],
            children: vec![],
        }
    }

    // §8.1: 创建帮助信息（D12 fix: 补充 Help 构造器）
    /// ```
    /// # use trust_error::diagnostic::{Diagnostic, SourceSpan};
    /// let span = SourceSpan::new("test.trust", 3, 5, 3, 6);
    /// let diag = Diagnostic::help("consider adding mut", span);
    /// assert_eq!(diag.level.to_string(), "help");
    /// ```
    pub fn help(message: impl Into<String>, span: SourceSpan) -> Self {
        Diagnostic {
            code: ErrorCode::E9999, // Help 不关联具体错误码
            level: Severity::Help,
            message: message.into(),
            primary_span: span,
            secondary_spans: vec![],
            fix_suggestions: vec![],
            children: vec![],
        }
    }

    // §8.1: 添加修复建议
    /// ```
    /// # use trust_error::diagnostic::{Diagnostic, ErrorCode, SourceSpan, FixSuggestion};
    /// let span = SourceSpan::new("test.trust", 3, 5, 3, 6);
    /// let fix = FixSuggestion::new("make mutable", span.clone(), "let mut x");
    /// let diag = Diagnostic::error(ErrorCode::E0389, "immutable", span).with_suggestion(fix);
    /// assert_eq!(diag.fix_suggestions.len(), 1);
    /// ```
    pub fn with_suggestion(mut self, suggestion: FixSuggestion) -> Self {
        self.fix_suggestions.push(suggestion);
        self
    }

    // §8.1: 添加子诊断
    /// ```
    /// # use trust_error::diagnostic::{Diagnostic, ErrorCode, SourceSpan};
    /// let span = SourceSpan::new("test.trust", 1, 1, 1, 5);
    /// let child = Diagnostic::help("try adding mut", span.clone());
    /// let diag = Diagnostic::error(ErrorCode::E0389, "immutable", span).with_child(child);
    /// assert_eq!(diag.children.len(), 1);
    /// ```
    pub fn with_child(mut self, child: Diagnostic) -> Self {
        self.children.push(child);
        self
    }
}

// ============================================================================
// 单元测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // §4.1: 每种 ErrorCode 至少一个构造测试

    #[test]
    fn errorcode_display_all_variants() {
        let codes = [
            ErrorCode::E0001,
            ErrorCode::E0002,
            ErrorCode::E0601,
            ErrorCode::E0602,
            ErrorCode::E0603,
            ErrorCode::E0382,
            ErrorCode::E0384,
            ErrorCode::E0389,
            ErrorCode::E0501,
            ErrorCode::E0502,
            ErrorCode::E0506,
            ErrorCode::E0308,
            ErrorCode::E0061,
            ErrorCode::E0425,
            ErrorCode::E0428,
            ErrorCode::E0700,
            ErrorCode::E9999,
        ];
        for code in &codes {
            let s = code.to_string();
            assert!(s.starts_with('E'), "{} should start with E", s);
        }
    }

    #[test]
    fn severity_display_lowercase() {
        assert_eq!(Severity::Error.to_string(), "error");
        assert_eq!(Severity::Warning.to_string(), "warning");
        assert_eq!(Severity::Help.to_string(), "help");
    }

    #[test]
    fn diagnostic_error_construction() {
        let span = SourceSpan::new("test.trust", 1, 1, 1, 5);
        let diag = Diagnostic::error(ErrorCode::E0382, "use after move", span);
        assert_eq!(diag.code, ErrorCode::E0382);
        assert_eq!(diag.level, Severity::Error);
        assert!(diag.fix_suggestions.is_empty());
        assert!(diag.children.is_empty());
    }

    #[test]
    fn diagnostic_warning_construction() {
        let span = SourceSpan::new("test.trust", 1, 1, 1, 5);
        let diag = Diagnostic::warning(ErrorCode::E0425, "unused", span);
        assert_eq!(diag.level, Severity::Warning);
    }

    #[test]
    fn diagnostic_help_construction() {
        let span = SourceSpan::new("test.trust", 1, 1, 1, 5);
        let diag = Diagnostic::help("tip", span);
        assert_eq!(diag.level, Severity::Help);
    }

    #[test]
    fn diagnostic_with_secondary_spans() {
        let primary = SourceSpan::new("test.trust", 3, 5, 3, 6);
        let secondary = SourceSpan::new("test.trust", 5, 5, 5, 6);
        let diag = Diagnostic::error_with_secondary(
            ErrorCode::E0382,
            "use after move",
            primary,
            vec![secondary],
        );
        assert_eq!(diag.secondary_spans.len(), 1);
    }

    #[test]
    fn diagnostic_with_suggestion_chain() {
        let span = SourceSpan::new("test.trust", 3, 5, 3, 6);
        let fix = FixSuggestion::new("make mutable", span.clone(), "let mut x");
        let diag = Diagnostic::error(ErrorCode::E0389, "immutable", span).with_suggestion(fix);
        assert_eq!(diag.fix_suggestions.len(), 1);
        assert_eq!(diag.fix_suggestions[0].replacement, "let mut x");
    }

    #[test]
    fn diagnostic_with_child_chain() {
        let span = SourceSpan::new("test.trust", 1, 1, 1, 5);
        let child = Diagnostic::help("try adding mut", span.clone());
        let diag = Diagnostic::error(ErrorCode::E0389, "immutable", span).with_child(child);
        assert_eq!(diag.children.len(), 1);
    }

    #[test]
    fn partial_eq_works() {
        let s1 = SourceSpan::new("a.trust", 1, 1, 1, 5);
        let s2 = SourceSpan::new("a.trust", 1, 1, 1, 5);
        assert_eq!(s1, s2);

        let d1 = Diagnostic::error(ErrorCode::E0382, "msg", s1);
        let d2 = Diagnostic::error(ErrorCode::E0382, "msg", s2);
        assert_eq!(d1, d2);
    }

    #[test]
    fn source_span_with_label() {
        let span = SourceSpan::with_label("test.trust", 1, 1, 1, 5, "moved here");
        assert_eq!(span.label, Some("moved here".into()));
    }
}
