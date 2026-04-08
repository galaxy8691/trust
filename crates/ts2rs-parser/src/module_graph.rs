//! 多文件模块图：递归解析相对路径 `import`，不合并 AST。

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use swc_common::sync::Lrc;
use swc_common::SourceMap;
use swc_ecma_ast::{
    Decl, ExportDecl, ImportDecl, ImportSpecifier, ModuleDecl, ModuleExportName, ModuleItem,
    Program,
};

use crate::{parse_typescript_file, ParseError, ParsedSource};

/// 解析得到的单个模块。
pub struct ParsedModule {
    pub path: PathBuf,
    pub source: ParsedSource,
}

/// 入口文件及其依赖（后序 DFS：依赖在前，入口在后）。
pub struct ParsedModuleGraph {
    pub modules: Vec<ParsedModule>,
    pub entry: PathBuf,
}

impl ParsedModuleGraph {
    /// 供 `ts2rs_hir::compile_graph` / `lower_module_graph` 使用的 `(path, program, cm)` 列表。
    pub fn compile_units(&self) -> Vec<(String, Program, Lrc<SourceMap>)> {
        self.modules
            .iter()
            .map(|m| {
                (
                    m.path.to_string_lossy().into_owned(),
                    m.source.program.clone(),
                    m.source.source_map.clone(),
                )
            })
            .collect()
    }

    /// 入口文件路径字符串（与语义层校验 `main` 所在文件一致）。
    pub fn entry_path_str(&self) -> String {
        self.entry.to_string_lossy().into_owned()
    }
}

/// 从入口 `.ts` 文件解析所有可达模块（相对路径 import）。
pub fn parse_module_graph(entry: &Path) -> Result<ParsedModuleGraph, ParseError> {
    parse_module_graph_with_extra_roots(entry, &[])
}

/// 从入口 DFS 解析依赖；再对每个 `extra` 根路径若尚未被访问则继续 DFS。
/// `extra` 为空时与 [`parse_module_graph`] 一致。
pub fn parse_module_graph_with_extra_roots(
    entry: &Path,
    extra: &[PathBuf],
) -> Result<ParsedModuleGraph, ParseError> {
    let entry_canon = entry.canonicalize().unwrap_or_else(|_| entry.to_path_buf());
    let mut visited = HashSet::<PathBuf>::new();
    let mut stack = HashSet::<PathBuf>::new();
    let mut modules = Vec::new();
    visit_module(entry, &mut visited, &mut stack, &mut modules)?;
    for p in extra {
        visit_module(p, &mut visited, &mut stack, &mut modules)?;
    }
    Ok(ParsedModuleGraph {
        modules,
        entry: entry_canon,
    })
}

fn visit_module(
    path: &Path,
    visited: &mut HashSet<PathBuf>,
    stack: &mut HashSet<PathBuf>,
    modules: &mut Vec<ParsedModule>,
) -> Result<(), ParseError> {
    let canon = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    if visited.contains(&canon) {
        return Ok(());
    }
    if stack.contains(&canon) {
        return Err(ParseError::Message(format!(
            "circular import detected at `{}`",
            path.display()
        )));
    }
    stack.insert(canon.clone());

    let text = fs::read_to_string(path)
        .map_err(|e| ParseError::Message(format!("cannot read `{}`: {e}", path.display())))?;
    let source = parse_typescript_file(path, &text)?;

    if let Program::Module(ref m) = source.program {
        for item in &m.body {
            if let ModuleItem::ModuleDecl(ModuleDecl::Import(imp)) = item {
                let dep = resolve_import_path(path, imp)?;
                visit_module(&dep, visited, stack, modules)?;
            }
        }
    }

    stack.remove(&canon);
    visited.insert(canon.clone());
    modules.push(ParsedModule {
        path: path.to_path_buf(),
        source,
    });
    Ok(())
}

fn resolve_import_path(file: &Path, imp: &ImportDecl) -> Result<PathBuf, ParseError> {
    if imp.type_only {
        return Err(ParseError::Message(
            "`import type` is not supported for import resolution".to_string(),
        ));
    }
    let raw = imp.src.value.to_string_lossy();
    let raw = raw.trim_matches(|c| c == '"' || c == '\'');
    if !(raw.starts_with("./") || raw.starts_with("../")) {
        return Err(ParseError::Message(format!(
            "only relative imports like `./file.ts` are supported, got `{raw}`"
        )));
    }
    let dir = file.parent().ok_or_else(|| {
        ParseError::Message(format!("cannot resolve parent of `{}`", file.display()))
    })?;
    Ok(dir.join(raw))
}

/// 每个模块导出的函数名（仅 `export function name`）。
pub fn exported_function_names(program: &Program) -> HashSet<String> {
    let mut out = HashSet::new();
    let Program::Module(m) = program else {
        return out;
    };
    for item in &m.body {
        if let ModuleItem::ModuleDecl(ModuleDecl::ExportDecl(ExportDecl {
            decl: Decl::Fn(f),
            ..
        })) = item
        {
            if !f.declare {
                out.insert(f.ident.sym.to_string());
            }
        }
    }
    out
}

/// 校验每个 `import { x }` 在目标模块中有对应 `export function x`。
pub fn validate_imports(graph: &ParsedModuleGraph) -> Result<(), ParseError> {
    let mut exports_by_path: HashMap<PathBuf, HashSet<String>> = HashMap::new();
    for m in &graph.modules {
        let canon = m.path.canonicalize().unwrap_or_else(|_| m.path.clone());
        let names = exported_function_names(&m.source.program);
        exports_by_path.insert(canon, names);
    }

    for m in &graph.modules {
        let Program::Module(mod_body) = &m.source.program else {
            continue;
        };
        for item in &mod_body.body {
            let ModuleItem::ModuleDecl(ModuleDecl::Import(imp)) = item else {
                continue;
            };
            let dep_path = resolve_import_path(&m.path, imp)?;
            let dep_canon = dep_path.canonicalize().unwrap_or_else(|_| dep_path.clone());
            let exports = exports_by_path.get(&dep_canon).ok_or_else(|| {
                ParseError::Message(format!(
                    "internal error: missing exports for `{}`",
                    dep_path.display()
                ))
            })?;

            for spec in &imp.specifiers {
                match spec {
                    ImportSpecifier::Named(named) => {
                        if named.is_type_only {
                            return Err(ParseError::Message(
                                "type-only import specifiers are not supported".to_string(),
                            ));
                        }
                        let want = match &named.imported {
                            Some(ModuleExportName::Ident(id)) => id.sym.to_string(),
                            Some(ModuleExportName::Str(s)) => {
                                s.value.to_string_lossy().into_owned()
                            }
                            None => named.local.sym.to_string(),
                        };
                        if !exports.contains(&want) {
                            return Err(ParseError::Message(format!(
                                "no exported function `{want}` in `{}`",
                                dep_path.display()
                            )));
                        }
                    }
                    ImportSpecifier::Default(_) | ImportSpecifier::Namespace(_) => {
                        return Err(ParseError::Message(
                            "only named imports `{ foo }` are supported".to_string(),
                        ));
                    }
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn circular_import_errors() {
        let dir = tempdir().unwrap();
        let a = dir.path().join("a.ts");
        let b = dir.path().join("b.ts");
        std::fs::write(
            &a,
            "import { g } from \"./b.ts\";\nexport function main(): number { return 0; }\n",
        )
        .unwrap();
        std::fs::write(
            &b,
            "import { main } from \"./a.ts\";\nexport function g(): number { return 1; }\n",
        )
        .unwrap();
        let err = match parse_module_graph(&a) {
            Ok(_) => panic!("expected circular import error"),
            Err(e) => e,
        };
        let s = err.to_string();
        assert!(
            s.contains("circular import"),
            "expected circular diagnostic, got: {s}"
        );
    }

    #[test]
    fn extra_root_includes_unreachable_file() {
        let dir = tempdir().unwrap();
        let main = dir.path().join("main.ts");
        let side = dir.path().join("side.ts");
        std::fs::write(&main, "export function main(): number { return 0; }\n").unwrap();
        std::fs::write(&side, "export function side(): number { return 1; }\n").unwrap();
        let g = parse_module_graph_with_extra_roots(&main, std::slice::from_ref(&side)).unwrap();
        let paths: Vec<_> = g
            .modules
            .iter()
            .map(|m| m.path.file_name().unwrap())
            .collect();
        assert_eq!(paths.len(), 2);
        assert!(paths.iter().any(|n| *n == "main.ts"));
        assert!(paths.iter().any(|n| *n == "side.ts"));
    }

    #[test]
    fn validate_import_not_exported() {
        let dir = tempdir().unwrap();
        let dep = dir.path().join("lib.ts");
        let main = dir.path().join("app.ts");
        std::fs::write(&dep, "export function bar(): number { return 1; }\n").unwrap();
        std::fs::write(
            &main,
            "import { foo } from \"./lib.ts\";\nexport function main(): number { return foo(); }\n",
        )
        .unwrap();
        let g = parse_module_graph(&main).unwrap();
        let err = validate_imports(&g).unwrap_err();
        let s = err.to_string();
        assert!(
            s.contains("no exported function `foo`"),
            "expected missing export diagnostic, got: {s}"
        );
    }
}
