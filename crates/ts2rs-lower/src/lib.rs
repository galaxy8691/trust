//! TypeScript → Rust：HIR 构建、语义检查、代码生成。

use swc_common::comments::SingleThreadedComments;
use swc_common::sync::Lrc;
use swc_common::SourceMap;
use swc_ecma_ast::Program;
use thiserror::Error;
use ts2rs_hir::{CodegenOptions, CompileError, CompileWarning};

#[derive(Debug, Error)]
pub enum LowerError {
    #[error(transparent)]
    Compile(#[from] CompileError),
}

/// 将 swc `Program` 与源映射编译为 Rust 源码。
pub fn lower_program(
    program: &Program,
    cm: &Lrc<SourceMap>,
    path: &str,
) -> Result<(String, Vec<CompileWarning>), LowerError> {
    lower_program_with_options(program, cm, path, &CodegenOptions::default())
}

pub fn lower_program_with_options(
    program: &Program,
    cm: &Lrc<SourceMap>,
    path: &str,
    codegen: &CodegenOptions,
) -> Result<(String, Vec<CompileWarning>), LowerError> {
    Ok(ts2rs_hir::compile_with_options(program, cm, path, codegen)?)
}

/// 多文件模块图：与 [`ts2rs_hir::compile_graph`] 等价，供 driver/CLI 主路径调用。
pub fn lower_module_graph(
    units: &[(String, Program, Lrc<SourceMap>, SingleThreadedComments)],
    entry_path: &str,
) -> Result<(String, Vec<CompileWarning>), LowerError> {
    lower_module_graph_with_options(units, entry_path, &CodegenOptions::default())
}

pub fn lower_module_graph_with_options(
    units: &[(String, Program, Lrc<SourceMap>, SingleThreadedComments)],
    entry_path: &str,
    codegen: &CodegenOptions,
) -> Result<(String, Vec<CompileWarning>), LowerError> {
    Ok(ts2rs_hir::compile_graph_with_options(
        units, entry_path, codegen,
    )?)
}

/// 多文件：仅 HIR 构建与语义检查，不生成 Rust。
pub fn check_module_graph(
    units: &[(String, Program, Lrc<SourceMap>, SingleThreadedComments)],
    entry_path: &str,
) -> Result<Vec<CompileWarning>, LowerError> {
    Ok(ts2rs_hir::check_graph(units, entry_path)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use ts2rs_parser::parse_typescript_file;
    use ts2rs_parser::{parse_module_graph, validate_imports};

    #[test]
    fn lowers_add_and_main() {
        let src = r#"
function add(a: number, b: number): number {
  return a + b;
}
function main(): number {
  return add(1, 2);
}
"#;
        let p = parse_typescript_file("t.ts", src).unwrap();
        let (rs, _) = lower_program(&p.program, &p.source_map, "t.ts").unwrap();
        assert!(rs.contains("fn add("));
        assert!(rs.contains("fn ts_main("));
        assert!(rs.contains("println!(\"{}\", ts_main())"));
    }

    #[test]
    fn lowers_export_main() {
        let src = r#"export function main(): number { return 1; }
"#;
        let p = parse_typescript_file("m.ts", src).unwrap();
        let (rs, _) = lower_program(&p.program, &p.source_map, "m.ts").unwrap();
        assert!(rs.contains("fn ts_main"));
        assert!(rs.contains("println!(\"{}\", ts_main())"));
    }

    /// §4.2：`let mut`、块、`Assign` 与 `ts2rs-hir` codegen `emit_stmt` 对齐。
    #[test]
    fn codegen_42_let_mut_block_and_assign() {
        let src = r#"
function main(): number {
  {
    let x: number = 1;
    x = 2;
    return x;
  }
}
"#;
        let p = parse_typescript_file("scope.ts", src).unwrap();
        let (rs, _) = lower_program(&p.program, &p.source_map, "scope.ts").unwrap();
        assert!(rs.contains("let mut x"), "{rs}");
        assert!(rs.contains("x = "), "{rs}");
        assert!(
            rs.matches('{').count() >= 2,
            "expected nested block braces in output: {rs}"
        );
    }

    /// §4.2：字符串拼接走 `format!`（见 codegen `BinaryKind::StrConcat`）。
    #[test]
    fn codegen_42_string_concat_uses_format() {
        let src = r#"
function main(): number {
  let s: string = "a" + "b";
  return s.length;
}
"#;
        let p = parse_typescript_file("strcat.ts", src).unwrap();
        let (rs, _) = lower_program(&p.program, &p.source_map, "strcat.ts").unwrap();
        assert!(rs.contains("format!"), "{rs}");
    }

    /// §4.2：对象字面量为值型 `HashMap::from`，无 `Rc`/`Arc`（见 codegen `ObjectLit`）。
    #[test]
    fn codegen_42_object_literal_hashmap_without_rc() {
        let src = r#"
function main(): number {
  let o: { k: number } = { k: 1 };
  return o.k;
}
"#;
        let p = parse_typescript_file("obj.ts", src).unwrap();
        let (rs, _) = lower_program(&p.program, &p.source_map, "obj.ts").unwrap();
        assert!(
            rs.contains("HashMap::from"),
            "expected HashMap::from for object literal: {rs}"
        );
        assert!(!rs.contains("Rc::"), "{rs}");
        assert!(!rs.contains("Arc::"), "{rs}");
    }

    /// §4.3：逗号表达式降为块时，块内行与闭合 `})` 与语句层级对齐（`emit_seq_expr`）。
    #[test]
    fn codegen_43_comma_seq_indented() {
        let src = r#"function main(): number {
  return (1, 2, 3);
}
"#;
        let p = parse_typescript_file("comma.ts", src).unwrap();
        let (rs, _) = lower_program(&p.program, &p.source_map, "comma.ts").unwrap();
        assert!(rs.contains("return ({"), "{rs}");
        assert!(
            rs.contains("        1_f64;\n") && rs.contains("        2_f64;\n"),
            "expected 8-space-indented seq arms: {rs}"
        );
        assert!(
            rs.contains("\n        3_f64\n    })"),
            "expected last arm indented and `}})` aligned to return stmt: {rs}"
        );
    }

    /// §4.3：`CodegenOptions::span_comments` 在语句前注入 `// ts: path:line:col`（`emit_stmt`）。
    #[test]
    fn codegen_43_span_comments_emits_ts_anchors() {
        let src = r#"function main(): number {
  return 1;
}
"#;
        let p = parse_typescript_file("x.ts", src).unwrap();
        let opts = ts2rs_hir::CodegenOptions {
            span_comments: true,
            ..Default::default()
        };
        let (rs, _) = lower_program_with_options(&p.program, &p.source_map, "x.ts", &opts).unwrap();
        assert!(
            rs.contains("// ts: x.ts:2:"),
            "expected span comment on `return` line (line 2): {rs}"
        );
    }

    #[test]
    fn console_log_multi_arg_uses_spaced_format() {
        let src = r#"function main(): void {
  console.log("ok", 1);
}
"#;
        let p = parse_typescript_file("log.ts", src).unwrap();
        let (rs, _) = lower_program(&p.program, &p.source_map, "log.ts").unwrap();
        assert!(
            rs.contains("println!(\"{} {}\""),
            "expected space-separated `{{}}` in format string: {rs}"
        );
    }

    /// §5.1：`console.error` / `console.debug` → `eprintln!`，多参格式与 `log` 相同（`emit_builtin_log`）。
    #[test]
    fn console_error_and_debug_use_eprintln() {
        let src = r#"function main(): void {
  console.error("e", 1);
  console.debug(2);
}
"#;
        let p = parse_typescript_file("cerr.ts", src).unwrap();
        let (rs, _) = lower_program(&p.program, &p.source_map, "cerr.ts").unwrap();
        assert!(
            rs.contains("eprintln!(\"{} {}\""),
            "expected stderr console with spaced format: {rs}"
        );
        assert!(
            rs.contains("eprintln!(\"{}\", 2_f64)"),
            "expected single-arg eprintln for debug: {rs}"
        );
    }

    /// §5.2：字符串 `.length` 为 UTF-16 码元数（`encode_utf16().count()`）。
    #[test]
    fn codegen_52_string_length_utf16() {
        let src = r#"function main(): number {
  return "😀".length;
}
"#;
        let p = parse_typescript_file("u.ts", src).unwrap();
        let (rs, _) = lower_program(&p.program, &p.source_map, "u.ts").unwrap();
        assert!(
            rs.contains("encode_utf16().count()"),
            "expected JS string length semantics: {rs}"
        );
    }

    /// §5.2：对象字段 `length` 走 `HashMap::get`，非 `len()`。
    #[test]
    fn codegen_52_object_length_field_uses_get() {
        let src = r#"function main(): number {
  let o: { length: number } = { length: 9 };
  return o.length;
}
"#;
        let p = parse_typescript_file("ol.ts", src).unwrap();
        let (rs, _) = lower_program(&p.program, &p.source_map, "ol.ts").unwrap();
        assert!(
            rs.contains(r#"get("length")"#),
            "expected object field access: {rs}"
        );
        assert!(
            !rs.contains("encode_utf16()"),
            "must not use string length for object field: {rs}"
        );
    }

    /// §5.2：`Math.*` 整数子集。
    #[test]
    fn codegen_52_math_builtins() {
        let src = r#"function main(): number {
  return Math.abs(-1) + Math.min(2, 5);
}
"#;
        let p = parse_typescript_file("m.ts", src).unwrap();
        let (rs, _) = lower_program(&p.program, &p.source_map, "m.ts").unwrap();
        assert!(rs.contains(".abs()"), "{rs}");
        assert!(rs.contains(".min("), "{rs}");
    }

    #[test]
    fn lowers_while_early_return() {
        let src = r#"
function main(): number {
  let n: number = 3;
  while (n) {
    return n;
  }
  return 0;
}
"#;
        let p = parse_typescript_file("w.ts", src).unwrap();
        let (rs, _) = lower_program(&p.program, &p.source_map, "w.ts").unwrap();
        assert!(rs.contains("while "));
        assert!(rs.contains("fn ts_main"));
    }

    #[test]
    fn compile_errors_on_export_named() {
        let src = r#"export { x };"#;
        let p = parse_typescript_file("e.ts", src).unwrap();
        let err = lower_program(&p.program, &p.source_map, "e.ts").unwrap_err();
        let s = err.to_string();
        assert!(s.contains("export { ... }"), "{s}");
    }

    #[test]
    fn lowers_logical_bool_ops() {
        let src = r#"
function main(): number {
  let ok: boolean = true && false;
  if (ok || true) {
    return 1;
  }
  return 0;
}
"#;
        let p = parse_typescript_file("logical.ts", src).unwrap();
        let (rs, _) = lower_program(&p.program, &p.source_map, "logical.ts").unwrap();
        assert!(rs.contains("&&"), "{rs}");
        assert!(rs.contains("||"), "{rs}");
    }

    #[test]
    fn lowers_ternary_and_template() {
        let src = r#"
function main(): number {
  let s: string = `x${1}y`;
  return s.length > 0 ? 7 : 0;
}
"#;
        let p = parse_typescript_file("tt.ts", src).unwrap();
        let (rs, _) = lower_program(&p.program, &p.source_map, "tt.ts").unwrap();
        assert!(rs.contains("if "), "{rs}");
        assert!(rs.contains("format!"), "{rs}");
    }

    #[test]
    fn lowers_module_graph_import_add() {
        let dir = tempdir().unwrap();
        let dep = dir.path().join("lib.ts");
        let main = dir.path().join("app.ts");
        std::fs::write(
            &dep,
            "export function add(a: number, b: number): number { return a + b; }\n",
        )
        .unwrap();
        std::fs::write(
            &main,
            "import { add } from \"./lib.ts\";\nexport function main(): number { return add(1, 2); }\n",
        )
        .unwrap();
        let graph = parse_module_graph(&main).unwrap();
        validate_imports(&graph).unwrap();
        let units = graph.compile_units();
        let (rs, _) = lower_module_graph(&units, &graph.entry_path_str()).unwrap();
        assert!(rs.contains("fn add("));
        assert!(rs.contains("fn ts_main("));
    }

    #[test]
    fn lowers_nullish_optional_and_literals() {
        let src = r#"
function main(): number {
  let xs: number[] = [1, 2];
  let o: { x: number } = { x: 3 };
  let z: null = null;
  return z?.x ?? (xs[1] + o.x);
}
"#;
        let p = parse_typescript_file("n.ts", src).unwrap();
        let (rs, _) = lower_program(&p.program, &p.source_map, "n.ts").unwrap();
        assert!(rs.contains("vec!["), "{rs}");
        assert!(rs.contains("HashMap"), "{rs}");
    }
}
