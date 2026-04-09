//! TypeScript 源文件解析（基于 swc）。

mod import_utils;
mod module_graph;
mod resolve_imports;

use std::fmt;
use std::path::Path;

use swc_common::{sync::Lrc, FileName, SourceMap, Span, Spanned};
use swc_ecma_ast::Program;
use swc_ecma_parser::{Parser, StringInput, Syntax, TsSyntax};

/// 解析失败：一条或多条诊断（多行 `Display`）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseError {
    WithLocation {
        path: String,
        line: usize,
        col: usize,
        message: String,
        byte_pos: u32,
    },
    Message(String),
    Many(Vec<ParseError>),
}

impl ParseError {
    /// 展开嵌套的 `Many`，保留 `WithLocation` 与 `Message`。
    pub fn flatten(self) -> Vec<ParseError> {
        match self {
            ParseError::Many(inner) => inner.into_iter().flat_map(ParseError::flatten).collect(),
            other => vec![other],
        }
    }

    fn sort_key(&self) -> (String, u32, usize, usize, String) {
        match self {
            ParseError::WithLocation {
                path,
                line,
                col,
                message,
                byte_pos,
            } => (path.clone(), *byte_pos, *line, *col, message.clone()),
            ParseError::Message(m) => (String::new(), 0, 0, 0, m.clone()),
            ParseError::Many(_) => (String::new(), 0, 0, 0, String::new()),
        }
    }
}

fn sort_parse_errors_vec(v: &mut [ParseError]) {
    v.sort_by_key(|a| a.sort_key());
}

/// 合并多条诊断为单一 `ParseError`（已排序；单条则不含 `Many` 包装）。
pub fn merge_parse_errors(items: Vec<ParseError>) -> ParseError {
    let mut flat: Vec<ParseError> = items.into_iter().flat_map(ParseError::flatten).collect();
    flat.retain(|e| !matches!(e, ParseError::Many(_)));
    sort_parse_errors_vec(&mut flat);
    flat.dedup_by(|a, b| a.sort_key() == b.sort_key());
    match flat.len() {
        0 => ParseError::Message("parse failed".to_string()),
        1 => flat.pop().expect("len 1"),
        _ => ParseError::Many(flat),
    }
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ParseError::WithLocation {
                path,
                line,
                col,
                message,
                ..
            } => write!(f, "{path}:{line}:{col}: {message}"),
            ParseError::Message(msg) => write!(f, "{msg}"),
            ParseError::Many(items) => {
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

impl std::error::Error for ParseError {}

/// 解析结果：AST + 源映射（供语义/诊断行列号）。
pub struct ParsedSource {
    pub program: Program,
    pub source_map: Lrc<SourceMap>,
}

/// 解析单文件 TypeScript。
pub fn parse_typescript_file(
    path: impl AsRef<Path>,
    source: &str,
) -> Result<ParsedSource, ParseError> {
    let path = path.as_ref();
    let path_str = path.to_string_lossy().into_owned();

    let cm: Lrc<SourceMap> = Default::default();
    let fm = cm.new_source_file(
        FileName::Real(path.to_path_buf()).into(),
        source.to_string(),
    );

    let ts = TsSyntax {
        tsx: false,
        decorators: false,
        dts: false,
        no_early_errors: false,
        disallow_ambiguous_jsx_like: true,
    };

    let mut parser = Parser::new(Syntax::Typescript(ts), StringInput::from(&*fm), None);

    let program_result = parser.parse_program();

    let mut collected: Vec<ParseError> = parser
        .take_errors()
        .into_iter()
        .map(|e| diagnostic_to_error(&cm, &path_str, e.span(), e.kind().msg().as_ref()))
        .collect();

    match program_result {
        Ok(program) => {
            if collected.is_empty() {
                return Ok(ParsedSource {
                    program,
                    source_map: cm,
                });
            }
            Err(merge_parse_errors(collected))
        }
        Err(e) => {
            let primary = diagnostic_to_error(&cm, &path_str, e.span(), e.kind().msg().as_ref());
            collected.insert(0, primary);
            Err(merge_parse_errors(collected))
        }
    }
}

pub use module_graph::{
    exported_function_names, parse_module_graph, parse_module_graph_with_extra_roots,
    validate_imports, ParsedModule, ParsedModuleGraph,
};
#[allow(deprecated)]
pub use resolve_imports::parse_typescript_resolving_imports;

fn diagnostic_to_error(cm: &Lrc<SourceMap>, path: &str, span: Span, message: &str) -> ParseError {
    let loc = cm.lookup_char_pos(span.lo);
    ParseError::WithLocation {
        path: path.to_string(),
        line: loc.line,
        col: loc.col_display,
        message: message.to_string(),
        byte_pos: span.lo.0,
    }
}

/// 便于下游 crate 统一引用 AST 类型。
pub mod ast {
    pub use swc_ecma_ast::*;
}

#[cfg(test)]
mod tests {
    use super::*;
    use swc_ecma_ast::Program;

    #[test]
    fn parses_empty_main() {
        let src = r#"function main(): number { return 0; }"#;
        let p = parse_typescript_file("test.ts", src).unwrap();
        assert!(matches!(p.program, Program::Module(_) | Program::Script(_)));
    }

    #[test]
    fn parse_rejects_unclosed_function_body() {
        let src = r#"function main(): number {"#;
        let e = match parse_typescript_file("bad.ts", src) {
            Err(e) => e,
            Ok(_) => panic!("expected parse error for unclosed function"),
        };
        let s = e.to_string();
        assert!(s.contains("bad.ts"), "{s}");
    }

    #[test]
    fn parse_collects_multiple_take_errors_when_present() {
        // swc 在同一段恢复解析中可对后续片段再报 take_errors（两行常落在同一逻辑行）。
        let src = "enum E { A,\nenum F { B,\n";
        let e = match parse_typescript_file("multi.ts", src) {
            Err(e) => e,
            Ok(_) => panic!("expected parse errors"),
        };
        let s = e.to_string();
        assert!(
            s.lines().count() >= 2,
            "expected multiple diagnostic lines, got:\n{s}"
        );
        assert!(s.contains("multi.ts"), "{s}");
    }

    #[test]
    fn merge_parse_errors_formats_multiple_lines() {
        let e = merge_parse_errors(vec![
            ParseError::WithLocation {
                path: "a.ts".into(),
                line: 1,
                col: 1,
                message: "first".into(),
                byte_pos: 0,
            },
            ParseError::WithLocation {
                path: "a.ts".into(),
                line: 3,
                col: 2,
                message: "second".into(),
                byte_pos: 20,
            },
        ]);
        let s = e.to_string();
        assert!(s.contains("first"), "{s}");
        assert!(s.contains("second"), "{s}");
        assert!(s.lines().count() >= 2, "{s}");
    }

    #[test]
    fn parses_module_with_import_and_export_main() {
        let src = r#"import { add } from "./dep.ts";
export function main(): number { return add(1, 2); }
"#;
        let p = parse_typescript_file("entry.ts", src).unwrap();
        assert!(
            matches!(p.program, Program::Module(_)),
            "expected module for import/export"
        );
        let dbg = format!("{:?}", p.program);
        assert!(dbg.contains("Import"), "expected Import in AST: {dbg}");
    }

    /// Deterministic pseudo-random source strings; `parse_typescript_file` must not panic.
    #[test]
    fn parse_fuzz_inputs_do_not_panic() {
        let base = r#"function main(): number { return 0; }
import { x } from "./a.ts";
export function f(): void {}
"#;
        for i in 0u32..800 {
            let mut s = String::new();
            for (j, ch) in base.chars().enumerate() {
                let k = (i ^ (j as u32).wrapping_mul(31)) & 0xff;
                if k < 8 && ch.is_whitespace() {
                    s.push(if k & 1 == 0 { '\n' } else { '\t' });
                } else {
                    s.push(ch);
                }
                if j % 11 == (i as usize % 7) {
                    s.push(char::from_u32(0x20 + (k % 60)).unwrap_or('x'));
                }
            }
            let _ = parse_typescript_file("fuzz.ts", &s);

            let garbled: String = (0..32)
                .map(|b| char::from_u32(0x20 + ((i.wrapping_add(b)) & 0x5f)).unwrap_or('?'))
                .collect();
            let mixed = format!("{s}{garbled}{base}");
            let _ = parse_typescript_file("fuzz.ts", &mixed);
        }
    }
}
