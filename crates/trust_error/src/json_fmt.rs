// §设计文档 §9.1.1 / constraints §8.2: JSON 格式化输出
//!
//! 手动构造 JSON 字符串（零外部依赖，不引入 serde_json）。
//! 输出 NDJSON 格式（\n 分隔），字段名对齐 constraints §8.2（D2 fix: level/fix_suggestions）。

use crate::diagnostic::{Diagnostic, SourceSpan};

/// §3.2 / constraints §8.2: JSON 字符串转义（D14 fix: 5 种控制字符，E8 fix: 补齐章引）
///
/// ```
/// # use trust_error::json_fmt::escape_json_string;
/// assert_eq!(escape_json_string("hello"), "hello");
/// assert_eq!(escape_json_string("say \"hi\""), "say \\\"hi\\\"");
/// assert_eq!(escape_json_string("a\\b"), "a\\\\b");
/// assert_eq!(escape_json_string("line1\nline2"), "line1\\nline2");
/// ```
pub fn escape_json_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            '\r' => out.push_str("\\r"),
            other => out.push(other),
        }
    }
    out
}

/// 序列化单个 SourceSpan 为 JSON 对象
fn format_span(span: &SourceSpan, indent: &str) -> String {
    let compact = indent.is_empty();
    let (nl, sp, _sp2): (&str, String, String) = if compact {
        ("", String::new(), String::new())
    } else {
        ("\n", format!("{}  ", indent), format!("{}    ", indent))
    };
    let inner = if compact { "" } else { &sp };
    let mut s = String::new();
    s.push_str(&format!("{}{{{}", if compact { "" } else { indent }, nl));
    s.push_str(&format!("{inner}\"file\": \"{}\",{nl}", escape_json_string(&span.file)));
    s.push_str(&format!("{inner}\"line_start\": {},{nl}", span.line_start));
    s.push_str(&format!("{inner}\"col_start\": {},{nl}", span.col_start));
    s.push_str(&format!("{inner}\"line_end\": {},{nl}", span.line_end));
    s.push_str(&format!("{inner}\"col_end\": {}", span.col_end));
    if let Some(ref label) = span.label {
        s.push_str(&format!(",{nl}{inner}\"label\": \"{}\"", escape_json_string(label)));
    }
    s.push_str(&format!("{nl}{}}}", if compact { "" } else { indent }));
    s
}

/// 序列化单个 Diagnostic 为 JSON 对象
fn format_diagnostic(diag: &Diagnostic, indent: &str, _pretty: bool) -> String {
    let compact = indent == "__COMPACT__";
    let (nl, sp, sp2) = if compact { ("", "", "") } else { ("\n", "  ", "    ") };

    let mut s = String::new();
    s.push_str(&format!("{}{{{}", if compact { "" } else { indent }, nl));
    s.push_str(&format!("{sp}\"code\": \"{}\",{nl}", diag.code));
    s.push_str(&format!("{sp}\"level\": \"{}\",{nl}", diag.level));
    s.push_str(&format!("{sp}\"message\": \"{}\",{nl}", escape_json_string(&diag.message)));

    // spans
    s.push_str(&format!("{sp}\"spans\": [{nl}",));
    s.push_str(&format_span(&diag.primary_span, sp));
    for sec in &diag.secondary_spans {
        s.push_str(&format!(",{nl}"));
        s.push_str(&format_span(sec, sp));
    }
    s.push_str(&format!("{nl}{sp}],{nl}"));

    // children
    s.push_str(&format!("{sp}\"children\": ["));
    if diag.children.is_empty() {
        s.push_str(&format!("],{nl}"));
    } else {
        s.push_str(nl);
        for (i, child) in diag.children.iter().enumerate() {
            if i > 0 {
                s.push_str(&format!(",{nl}"));
            }
            s.push_str(&format_diagnostic(child, sp2, _pretty));
        }
        s.push_str(&format!("{nl}{sp}],{nl}"));
    }

    // fix_suggestions
    s.push_str(&format!("{sp}\"fix_suggestions\": ["));
    if diag.fix_suggestions.is_empty() {
        s.push(']');
    } else {
        s.push_str(nl);
        for (i, fix) in diag.fix_suggestions.iter().enumerate() {
            if i > 0 {
                s.push_str(&format!(",{nl}"));
            }
            s.push_str(&format!("{sp2}{{{nl}",));
            s.push_str(&format!(
                "{sp2}{sp}\"message\": \"{}\",{nl}",
                escape_json_string(&fix.message)
            ));
            s.push_str(&format!("{sp2}{sp}\"span\": ",));
            s.push_str(&format_span(&fix.span, &format!("{sp2}{sp}")));
            s.push_str(&format!(
                ",{nl}{sp2}{sp}\"replacement\": \"{}\"{nl}",
                escape_json_string(&fix.replacement)
            ));
            s.push_str(&format!("{sp2}}}"));
        }
        s.push_str(&format!("{nl}{sp}]"));
    }

    s.push_str(&format!("{nl}{}}}", if compact { "" } else { indent }));
    s
}

/// §9.1.1 / constraints §8.2: 格式化诊断列表为 NDJSON
///
/// 空输入返回 `""`（D10 fix）。
///
/// ```
/// # use trust_error::diagnostic::{Diagnostic, ErrorCode, SourceSpan};
/// # use trust_error::json_fmt::format_diagnostics;
/// let span = SourceSpan::new("test.trust", 1, 1, 1, 5);
/// let diag = Diagnostic::error(ErrorCode::E0382, "use after move", span);
/// let json = format_diagnostics(&[diag], /* pretty */ true);
/// assert!(json.contains("\"E0382\""));
/// assert!(json.contains("\"level\": \"error\""));
/// ```
pub fn format_diagnostics(diagnostics: &[Diagnostic], pretty: bool) -> String {
    if diagnostics.is_empty() {
        return String::new();
    }

    // E3 fix: pretty=false 时输出紧凑单行 NDJSON
    let indent = if pretty { "" } else { "__COMPACT__" };
    let mut out = String::new();
    for (i, diag) in diagnostics.iter().enumerate() {
        if i > 0 {
            out.push('\n');
        }
        out.push_str(&format_diagnostic(diag, indent, pretty));
    }
    out
}

// ============================================================================
// 单元测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagnostic::{Diagnostic, ErrorCode, FixSuggestion, SourceSpan};

    #[test]
    fn format_empty_list() {
        let json = format_diagnostics(&[], true);
        assert!(json.is_empty());
    }

    #[test]
    fn format_empty_fix_suggestions_is_brackets() {
        let span = SourceSpan::new("t.trust", 1, 1, 1, 5);
        let diag = Diagnostic::error(ErrorCode::E0382, "err", span);
        let json = format_diagnostics(&[diag], false);
        assert!(json.contains("\"fix_suggestions\": []"), "E10 fix: empty array");
    }

    #[test]
    fn format_pretty_false_outputs_compact_ndjson() {
        let span = SourceSpan::new("t.trust", 1, 1, 1, 5);
        let diag = Diagnostic::error(ErrorCode::E0382, "err", span);
        let json = format_diagnostics(&[diag], false);
        // 紧凑模式：不应有内部换行或缩进空格
        assert!(!json.contains("\n  "), "E3 fix: compact NDJSON should be single-line per object");
    }

    #[test]
    fn escape_json_carriage_return() {
        assert_eq!(escape_json_string("a\rb"), "a\\rb");
    }

    #[test]
    fn format_spans_primary_first() {
        let primary = SourceSpan::with_label("t.trust", 1, 1, 1, 5, "moved");
        let secondary = SourceSpan::with_label("t.trust", 3, 1, 3, 5, "used");
        let diag = Diagnostic::error_with_secondary(
            ErrorCode::E0382,
            "err",
            primary.clone(),
            vec![secondary],
        );
        let json = format_diagnostics(&[diag], false);
        // E11 fix: primary_span 应出现在 secondary 之前
        let primary_pos = json.find("\"moved\"").unwrap();
        let secondary_pos = json.find("\"used\"").unwrap();
        assert!(primary_pos < secondary_pos, "primary_span should appear before secondary");
    }

    #[test]
    fn format_single_error_contains_fields() {
        let span = SourceSpan::new("test.trust", 1, 1, 1, 5);
        let diag = Diagnostic::error(ErrorCode::E0382, "use after move", span);
        let json = format_diagnostics(&[diag], true);
        assert!(json.contains("\"E0382\""));
        assert!(json.contains("\"level\": \"error\""));
        assert!(json.contains("\"message\": \"use after move\""));
        assert!(json.contains("\"spans\""));
    }

    #[test]
    fn format_with_secondary_spans() {
        let primary = SourceSpan::new("test.trust", 3, 5, 3, 6);
        let secondary = SourceSpan::new("test.trust", 5, 5, 5, 6);
        let diag = Diagnostic::error_with_secondary(
            ErrorCode::E0382,
            "use after move",
            primary,
            vec![secondary],
        );
        let json = format_diagnostics(&[diag], true);
        assert!(json.contains("\"spans\""), "should have spans array");
    }

    #[test]
    fn format_with_fix_suggestion() {
        let span = SourceSpan::new("test.trust", 3, 5, 3, 6);
        let fix = FixSuggestion::new("make mutable", span.clone(), "let mut x");
        let diag = Diagnostic::error(ErrorCode::E0389, "immutable", span).with_suggestion(fix);
        let json = format_diagnostics(&[diag], true);
        assert!(json.contains("\"fix_suggestions\""));
        assert!(json.contains("\"replacement\": \"let mut x\""));
    }

    #[test]
    fn format_with_child() {
        let span = SourceSpan::new("test.trust", 1, 1, 1, 5);
        let child = Diagnostic::help("tip", span.clone());
        let diag = Diagnostic::error(ErrorCode::E0389, "immutable", span).with_child(child);
        let json = format_diagnostics(&[diag], true);
        assert!(json.contains("\"children\""));
    }

    #[test]
    fn escape_json_quotes_and_backslash() {
        assert_eq!(escape_json_string("say \"hi\""), "say \\\"hi\\\"");
        assert_eq!(escape_json_string("a\\b"), "a\\\\b");
    }

    #[test]
    fn escape_json_newline_and_tab() {
        assert_eq!(escape_json_string("a\nb"), "a\\nb");
        assert_eq!(escape_json_string("a\tb"), "a\\tb");
    }

    #[test]
    fn ndjson_multiple_diagnostics() {
        let s1 = SourceSpan::new("a.trust", 1, 1, 1, 5);
        let s2 = SourceSpan::new("b.trust", 2, 1, 2, 5);
        let d1 = Diagnostic::error(ErrorCode::E0382, "err1", s1);
        let d2 = Diagnostic::warning(ErrorCode::E0425, "warn1", s2);
        let json = format_diagnostics(&[d1, d2], true);
        // NDJSON: 两个独立 JSON 对象由换行分隔
        assert!(json.contains("}\n{"), "should have NDJSON separator");
    }
}
