//! TypeScript 源文件解析（基于 swc）。

mod module_graph;
mod resolve_imports;

use std::path::Path;

use swc_common::{sync::Lrc, FileName, SourceMap, Span, Spanned};
use swc_ecma_ast::Program;
use swc_ecma_parser::{Parser, StringInput, Syntax, TsSyntax};
use thiserror::Error;

/// 解析失败（含源位置信息，便于 CLI 输出）。
#[derive(Debug, Error)]
pub enum ParseError {
    #[error("{path}:{line}:{col}: {message}")]
    WithLocation {
        path: String,
        line: usize,
        col: usize,
        message: String,
    },
    #[error("parse failed: {0}")]
    Message(String),
}

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

    let program = parser
        .parse_program()
        .map_err(|e| diagnostic_to_error(&cm, &path_str, e.span(), e.kind().msg().as_ref()))?;

    if let Some(e) = parser.take_errors().into_iter().next() {
        return Err(diagnostic_to_error(
            &cm,
            &path_str,
            e.span(),
            e.kind().msg().as_ref(),
        ));
    }

    Ok(ParsedSource {
        program,
        source_map: cm,
    })
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
