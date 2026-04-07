//! 将 `import { x } from "./dep.ts"` 展开为同一模块内的顶层 `function`（仅支持相对路径与 `export function`）。

use std::collections::HashSet;
use std::fs;
use std::path::Path;

use swc_ecma_ast::{
    Decl, ExportDecl, FnDecl, ImportSpecifier, Module, ModuleDecl, ModuleExportName, ModuleItem,
    Program, Stmt,
};

use crate::{parse_typescript_file, ParseError, ParsedSource};

/// 解析入口文件并递归展开支持的 `import`（旧策略：合并进单模块 AST）。
///
/// 主路径请使用 [`crate::parse_module_graph`] + [`crate::validate_imports`] + HIR `compile_graph`。
#[deprecated(
    note = "use parse_module_graph + validate_imports + compile_graph / lower_module_graph instead"
)]
pub fn parse_typescript_resolving_imports(
    path: &Path,
    source: &str,
) -> Result<ParsedSource, ParseError> {
    let mut visited = HashSet::new();
    parse_ts_resolved(path, source, &mut visited)
}

fn parse_ts_resolved(
    path: &Path,
    source: &str,
    visited: &mut HashSet<std::path::PathBuf>,
) -> Result<ParsedSource, ParseError> {
    let canon = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    if !visited.insert(canon.clone()) {
        return Err(ParseError::Message(format!(
            "circular import detected at `{}`",
            path.display()
        )));
    }

    let mut parsed = parse_typescript_file(path, source)?;
    let Program::Module(m) = &mut parsed.program else {
        visited.remove(&canon);
        return Ok(parsed);
    };

    let merged = merge_module_body(path, m, visited)?;
    *m = Module {
        span: m.span,
        shebang: m.shebang.clone(),
        body: merged,
    };

    visited.remove(&canon);
    Ok(parsed)
}

fn merge_module_body(
    file: &Path,
    m: &Module,
    visited: &mut HashSet<std::path::PathBuf>,
) -> Result<Vec<ModuleItem>, ParseError> {
    let mut prefix: Vec<ModuleItem> = Vec::new();
    let mut out: Vec<ModuleItem> = Vec::new();

    for item in &m.body {
        match item {
            ModuleItem::ModuleDecl(ModuleDecl::Import(imp)) => {
                process_import(file, imp, visited, &mut prefix)?;
            }
            _ => out.push(item.clone()),
        }
    }

    let mut body = prefix;
    body.extend(out);
    Ok(body)
}

fn process_import(
    file: &Path,
    imp: &swc_ecma_ast::ImportDecl,
    visited: &mut HashSet<std::path::PathBuf>,
    prefix: &mut Vec<ModuleItem>,
) -> Result<(), ParseError> {
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
    let dep_path = dir.join(raw);

    let text = fs::read_to_string(&dep_path).map_err(|e| {
        ParseError::Message(format!("cannot read `{}`: {e}", dep_path.display()))
    })?;

    let dep_parsed = parse_ts_resolved(&dep_path, &text, visited)?;
    let Program::Module(dm) = dep_parsed.program else {
        return Err(ParseError::Message(
            "imported file must be an ES module".to_string(),
        ));
    };

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
                    Some(ModuleExportName::Str(s)) => s.value.to_string_lossy().into_owned(),
                    None => named.local.sym.to_string(),
                };
                let f = find_exported_function(&dm, &want).ok_or_else(|| {
                    ParseError::Message(format!(
                        "no exported function `{want}` in `{}`",
                        dep_path.display()
                    ))
                })?;
                prefix.push(fn_decl_as_module_item(f));
            }
            ImportSpecifier::Default(_) | ImportSpecifier::Namespace(_) => {
                return Err(ParseError::Message(
                    "only named imports `{ foo }` are supported".to_string(),
                ));
            }
        }
    }

    Ok(())
}

fn find_exported_function(m: &Module, name: &str) -> Option<FnDecl> {
    for item in &m.body {
        match item {
            ModuleItem::ModuleDecl(ModuleDecl::ExportDecl(ExportDecl {
                decl: Decl::Fn(f),
                ..
            })) if !f.declare && f.ident.sym == name => {
                return Some(f.clone());
            }
            ModuleItem::Stmt(Stmt::Decl(Decl::Fn(f))) if !f.declare && f.ident.sym == name => {
                return Some(f.clone());
            }
            _ => {}
        }
    }
    None
}

fn fn_decl_as_module_item(f: FnDecl) -> ModuleItem {
    ModuleItem::Stmt(Stmt::Decl(Decl::Fn(f)))
}
