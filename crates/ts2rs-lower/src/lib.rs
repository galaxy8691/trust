//! TypeScript → Rust：HIR 构建、语义检查、代码生成。

use swc_common::sync::Lrc;
use swc_common::SourceMap;
use swc_ecma_ast::Program;
use thiserror::Error;
use ts2rs_hir::CompileError;

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
) -> Result<String, LowerError> {
    Ok(ts2rs_hir::compile(program, cm, path)?)
}

/// 多文件模块图：与 [`ts2rs_hir::compile_graph`] 等价，供 driver/CLI 主路径调用。
pub fn lower_module_graph(
    units: &[(String, Program, Lrc<SourceMap>)],
    entry_path: &str,
) -> Result<String, LowerError> {
    Ok(ts2rs_hir::compile_graph(units, entry_path)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use ts2rs_parser::{parse_module_graph, validate_imports};
    use ts2rs_parser::parse_typescript_file;

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
        let rs = lower_program(&p.program, &p.source_map, "t.ts").unwrap();
        assert!(rs.contains("fn add("));
        assert!(rs.contains("fn ts_main("));
        assert!(rs.contains("println!(\"{}\", ts_main())"));
    }

    #[test]
    fn lowers_export_main() {
        let src = r#"export function main(): number { return 1; }
"#;
        let p = parse_typescript_file("m.ts", src).unwrap();
        let rs = lower_program(&p.program, &p.source_map, "m.ts").unwrap();
        assert!(rs.contains("fn ts_main"));
        assert!(rs.contains("println!(\"{}\", ts_main())"));
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
        let rs = lower_program(&p.program, &p.source_map, "w.ts").unwrap();
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
        let rs = lower_program(&p.program, &p.source_map, "logical.ts").unwrap();
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
        let rs = lower_program(&p.program, &p.source_map, "tt.ts").unwrap();
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
        let rs = lower_module_graph(&units, &graph.entry_path_str()).unwrap();
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
        let rs = lower_program(&p.program, &p.source_map, "n.ts").unwrap();
        assert!(rs.contains("vec!["), "{rs}");
        assert!(rs.contains("HashMap"), "{rs}");
    }
}
