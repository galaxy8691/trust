//! 多文件模块图：递归解析相对路径 `import`，不合并 AST。

use std::collections::{HashMap, HashSet, VecDeque};
use std::fs;
use std::path::{Path, PathBuf};

use swc_common::comments::SingleThreadedComments;
use swc_common::sync::Lrc;
use swc_common::SourceMap;
use swc_ecma_ast::{
    Decl, ExportDecl, ExportSpecifier, ModuleDecl, ModuleExportName, ModuleItem, Program,
};

use crate::import_utils::{
    named_import_target, resolve_relative_ts_path, resolve_supported_import_path,
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
    /// 供 `ts2rs_hir::compile_graph` / `lower_module_graph` 使用的 `(path, program, cm, comments)` 列表。
    pub fn compile_units(&self) -> Vec<(String, Program, Lrc<SourceMap>, SingleThreadedComments)> {
        self.modules
            .iter()
            .map(|m| {
                (
                    m.path.to_string_lossy().into_owned(),
                    m.source.program.clone(),
                    m.source.source_map.clone(),
                    m.source.comments.clone(),
                )
            })
            .collect()
    }

    /// 入口文件路径字符串（与语义层校验 `main` 所在文件一致）。
    pub fn entry_path_str(&self) -> String {
        self.entry.to_string_lossy().into_owned()
    }

    /// Canonical path for a module in this graph（与 [`Self::forward_deps`] 的 key 一致）。
    pub fn canonical_module_path(pm: &ParsedModule) -> PathBuf {
        pm.path.canonicalize().unwrap_or_else(|_| pm.path.clone())
    }

    /// 每个模块直接依赖的其它模块（canonical path）；与 [`visit_module`] 的 import / re-export 规则一致。
    pub fn forward_deps(&self) -> Result<HashMap<PathBuf, Vec<PathBuf>>, ParseError> {
        let mut m = HashMap::new();
        for pm in &self.modules {
            let canon = Self::canonical_module_path(pm);
            let deps = program_direct_deps(&pm.path, &pm.source.program)?;
            m.insert(canon, deps);
        }
        Ok(m)
    }

    /// `dirty` 为内容已变的模块（canonical path）。返回需重建 HIR 的模块集：
    /// `dirty` ∪ 所有（直接 or 传递）**导入**了 `dirty` 中任一节点的模块。
    pub fn rebuild_transitive_importers(
        &self,
        dirty: &HashSet<PathBuf>,
    ) -> Result<HashSet<PathBuf>, ParseError> {
        let forward = self.forward_deps()?;
        Ok(rebuild_transitive_importers_from_forward(&forward, dirty))
    }
}

/// 由正向依赖图计算增量重建集合（供测试与调用方复用）。
pub fn rebuild_transitive_importers_from_forward(
    forward: &HashMap<PathBuf, Vec<PathBuf>>,
    dirty: &HashSet<PathBuf>,
) -> HashSet<PathBuf> {
    let mut reverse: HashMap<PathBuf, Vec<PathBuf>> = HashMap::new();
    for (from, deps) in forward {
        for d in deps {
            reverse.entry(d.clone()).or_default().push(from.clone());
        }
    }
    let mut out: HashSet<PathBuf> = dirty.clone();
    let mut q: VecDeque<PathBuf> = dirty.iter().cloned().collect();
    while let Some(dep) = q.pop_front() {
        if let Some(importers) = reverse.get(&dep) {
            for imp in importers {
                if out.insert(imp.clone()) {
                    q.push_back(imp.clone());
                }
            }
        }
    }
    out
}

fn program_direct_deps(module_path: &Path, program: &Program) -> Result<Vec<PathBuf>, ParseError> {
    let mut seen = HashSet::<PathBuf>::new();
    let mut out = Vec::new();
    let Program::Module(ref m) = program else {
        return Ok(out);
    };
    for item in &m.body {
        match item {
            ModuleItem::ModuleDecl(ModuleDecl::Import(imp)) => {
                let dep = resolve_supported_import_path(module_path, imp)?;
                let c = dep.canonicalize().unwrap_or(dep);
                if seen.insert(c.clone()) {
                    out.push(c);
                }
            }
            ModuleItem::ModuleDecl(ModuleDecl::ExportAll(ea)) if !ea.type_only => {
                let dep = resolve_relative_ts_path(module_path, &ea.src)?;
                let c = dep.canonicalize().unwrap_or(dep);
                if seen.insert(c.clone()) {
                    out.push(c);
                }
            }
            ModuleItem::ModuleDecl(ModuleDecl::ExportNamed(ne))
                if ne.src.is_some() && !ne.type_only =>
            {
                let dep =
                    resolve_relative_ts_path(module_path, ne.src.as_deref().expect("checked"))?;
                let c = dep.canonicalize().unwrap_or(dep);
                if seen.insert(c.clone()) {
                    out.push(c);
                }
            }
            _ => {}
        }
    }
    Ok(out)
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
            match item {
                ModuleItem::ModuleDecl(ModuleDecl::Import(imp)) => {
                    let dep = resolve_supported_import_path(path, imp)?;
                    visit_module(&dep, visited, stack, modules)?;
                }
                ModuleItem::ModuleDecl(ModuleDecl::ExportAll(ea)) if !ea.type_only => {
                    let dep = resolve_relative_ts_path(path, &ea.src)?;
                    visit_module(&dep, visited, stack, modules)?;
                }
                ModuleItem::ModuleDecl(ModuleDecl::ExportNamed(ne))
                    if ne.src.is_some() && !ne.type_only =>
                {
                    let dep = resolve_relative_ts_path(path, ne.src.as_deref().expect("checked"))?;
                    visit_module(&dep, visited, stack, modules)?;
                }
                _ => {}
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

fn module_export_name_str(n: &ModuleExportName) -> String {
    match n {
        ModuleExportName::Ident(i) => i.sym.to_string(),
        ModuleExportName::Str(s) => s.value.to_string_lossy().into_owned(),
    }
}

fn try_add_exported_fn(
    set: &mut HashSet<String>,
    module_path: &Path,
    name: String,
) -> Result<(), ParseError> {
    if !set.insert(name.clone()) {
        return Err(ParseError::Message(format!(
            "duplicate exported function `{name}` in `{}`",
            module_path.display()
        )));
    }
    Ok(())
}

struct ModuleEntry<'a> {
    path: PathBuf,
    program: &'a Program,
}

fn build_module_index(graph: &ParsedModuleGraph) -> HashMap<PathBuf, ModuleEntry<'_>> {
    let mut m = HashMap::new();
    for pm in &graph.modules {
        let canon = pm.path.canonicalize().unwrap_or_else(|_| pm.path.clone());
        m.insert(
            canon,
            ModuleEntry {
                path: pm.path.clone(),
                program: &pm.source.program,
            },
        );
    }
    m
}

fn effective_exported_function_names_at(
    canon: &PathBuf,
    index: &HashMap<PathBuf, ModuleEntry<'_>>,
    memo: &mut HashMap<PathBuf, HashSet<String>>,
    visiting: &mut HashSet<PathBuf>,
) -> Result<HashSet<String>, ParseError> {
    if let Some(cached) = memo.get(canon) {
        return Ok(cached.clone());
    }
    if visiting.contains(canon) {
        return Err(ParseError::Message(format!(
            "circular re-export involving `{}`",
            canon.display()
        )));
    }
    let entry = index.get(canon).ok_or_else(|| {
        ParseError::Message(format!(
            "internal error: missing module `{}` in export resolution",
            canon.display()
        ))
    })?;
    visiting.insert(canon.clone());

    let build = (|| -> Result<HashSet<String>, ParseError> {
        let mut set = HashSet::new();
        let file_path = &entry.path;
        match entry.program {
            Program::Script(_) => {}
            Program::Module(m) => {
                for item in &m.body {
                    if let ModuleItem::ModuleDecl(ModuleDecl::ExportDecl(ExportDecl {
                        decl: Decl::Fn(f),
                        ..
                    })) = item
                    {
                        if !f.declare {
                            try_add_exported_fn(&mut set, file_path, f.ident.sym.to_string())?;
                        }
                    }
                    if let ModuleItem::ModuleDecl(ModuleDecl::ExportAll(ea)) = item {
                        if ea.type_only {
                            continue;
                        }
                        let dep = resolve_relative_ts_path(file_path, &ea.src)?;
                        let dep_canon = dep.canonicalize().unwrap_or_else(|_| dep.clone());
                        let sub = effective_exported_function_names_at(
                            &dep_canon, index, memo, visiting,
                        )?;
                        for n in sub {
                            try_add_exported_fn(&mut set, file_path, n)?;
                        }
                    }
                    if let ModuleItem::ModuleDecl(ModuleDecl::ExportNamed(ne)) = item {
                        if ne.type_only {
                            continue;
                        }
                        if let Some(src) = ne.src.as_deref() {
                            let dep = resolve_relative_ts_path(file_path, src)?;
                            let dep_canon = dep.canonicalize().unwrap_or_else(|_| dep.clone());
                            let sub = effective_exported_function_names_at(
                                &dep_canon, index, memo, visiting,
                            )?;
                            for spec in &ne.specifiers {
                                match spec {
                                    ExportSpecifier::Namespace(_) => {
                                        return Err(ParseError::Message(
                                            "`export * as` re-export is not supported".to_string(),
                                        ));
                                    }
                                    ExportSpecifier::Default(_) => {
                                        return Err(ParseError::Message(
                                            "`export default` re-export from is not supported"
                                                .to_string(),
                                        ));
                                    }
                                    ExportSpecifier::Named(named) => {
                                        if named.is_type_only {
                                            continue;
                                        }
                                        let orig = module_export_name_str(&named.orig);
                                        let exported = named
                                            .exported
                                            .as_ref()
                                            .map(module_export_name_str)
                                            .unwrap_or_else(|| orig.clone());
                                        if !sub.contains(&orig) {
                                            return Err(ParseError::Message(format!(
                                                "no exported function `{orig}` in `{}`",
                                                dep.display()
                                            )));
                                        }
                                        try_add_exported_fn(&mut set, file_path, exported)?;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        Ok(set)
    })();

    visiting.remove(canon);
    let set = build?;
    memo.insert(canon.clone(), set.clone());
    Ok(set)
}

/// 每个模块**有效**导出的函数名：`export function`、相对路径 `export * from` / `export { … } from`。
pub fn effective_exported_function_names_by_path(
    graph: &ParsedModuleGraph,
) -> Result<HashMap<PathBuf, HashSet<String>>, ParseError> {
    let index = build_module_index(graph);
    let mut memo = HashMap::new();
    let mut out = HashMap::new();
    let mut keys: Vec<PathBuf> = index.keys().cloned().collect();
    keys.sort();
    for canon in keys {
        let s =
            effective_exported_function_names_at(&canon, &index, &mut memo, &mut HashSet::new())?;
        out.insert(canon, s);
    }
    Ok(out)
}

/// 校验每个 `import { x }` 在目标模块的有效导出中有对应函数名。
pub fn validate_imports(graph: &ParsedModuleGraph) -> Result<(), ParseError> {
    let exports_by_path = effective_exported_function_names_by_path(graph)?;

    for m in &graph.modules {
        let Program::Module(mod_body) = &m.source.program else {
            continue;
        };
        for item in &mod_body.body {
            let ModuleItem::ModuleDecl(ModuleDecl::Import(imp)) = item else {
                continue;
            };
            let dep_path = resolve_supported_import_path(&m.path, imp)?;
            let dep_canon = dep_path.canonicalize().unwrap_or_else(|_| dep_path.clone());
            let exports = exports_by_path.get(&dep_canon).ok_or_else(|| {
                ParseError::Message(format!(
                    "internal error: missing exports for `{}`",
                    dep_path.display()
                ))
            })?;

            for spec in &imp.specifiers {
                let want = named_import_target(spec)?;
                if !exports.contains(&want) {
                    return Err(ParseError::Message(format!(
                        "no exported function `{want}` in `{}`",
                        dep_path.display()
                    )));
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

    #[test]
    fn validate_import_via_export_star_from() {
        let dir = tempdir().unwrap();
        let lib = dir.path().join("lib.ts");
        let barrel = dir.path().join("barrel.ts");
        let app = dir.path().join("app.ts");
        fs::write(
            &lib,
            "export function add(a: number, b: number): number { return a + b; }\n",
        )
        .unwrap();
        fs::write(&barrel, "export * from \"./lib.ts\";\n").unwrap();
        fs::write(
            &app,
            "import { add } from \"./barrel.ts\";\nexport function main(): number { return add(1, 2); }\n",
        )
        .unwrap();
        let g = parse_module_graph(&app).unwrap();
        validate_imports(&g).unwrap();
    }

    #[test]
    fn validate_import_export_named_from_alias() {
        let dir = tempdir().unwrap();
        let lib = dir.path().join("lib.ts");
        let barrel = dir.path().join("barrel.ts");
        let app = dir.path().join("app.ts");
        fs::write(
            &lib,
            "export function add(a: number, b: number): number { return a + b; }\n",
        )
        .unwrap();
        fs::write(&barrel, "export { add as plus } from \"./lib.ts\";\n").unwrap();
        fs::write(
            &app,
            "import { plus } from \"./barrel.ts\";\nexport function main(): number { return plus(1, 2); }\n",
        )
        .unwrap();
        let g = parse_module_graph(&app).unwrap();
        validate_imports(&g).unwrap();
    }

    #[test]
    fn duplicate_export_star_twice_errors() {
        let dir = tempdir().unwrap();
        let lib = dir.path().join("lib.ts");
        let barrel = dir.path().join("barrel.ts");
        fs::write(
            &lib,
            "export function add(a: number, b: number): number { return a + b; }\n",
        )
        .unwrap();
        fs::write(
            &barrel,
            "export * from \"./lib.ts\";\nexport * from \"./lib.ts\";\n",
        )
        .unwrap();
        let g = parse_module_graph(&barrel).unwrap();
        let e = validate_imports(&g).unwrap_err();
        let s = e.to_string();
        assert!(
            s.contains("duplicate exported function"),
            "expected duplicate export diagnostic, got: {s}"
        );
    }

    #[test]
    fn rebuild_transitive_importers_chain() {
        let mut forward = HashMap::new();
        forward.insert(p("C"), vec![]);
        forward.insert(p("B"), vec![p("C")]);
        forward.insert(p("A"), vec![p("B")]);
        let dirty = HashSet::from([p("C")]);
        let r = rebuild_transitive_importers_from_forward(&forward, &dirty);
        assert_eq!(r, HashSet::from([p("A"), p("B"), p("C")]));
    }

    #[test]
    fn rebuild_transitive_importers_middle_dirty() {
        let mut forward = HashMap::new();
        forward.insert(p("C"), vec![]);
        forward.insert(p("B"), vec![p("C")]);
        forward.insert(p("A"), vec![p("B")]);
        let dirty = HashSet::from([p("B")]);
        let r = rebuild_transitive_importers_from_forward(&forward, &dirty);
        assert_eq!(r, HashSet::from([p("A"), p("B")]));
    }

    #[test]
    fn forward_deps_and_rebuild_match_visit_order() {
        let dir = tempdir().unwrap();
        let c = dir.path().join("c.ts");
        let b = dir.path().join("b.ts");
        let a = dir.path().join("a.ts");
        fs::write(&c, "export function c(): number { return 0; }\n").unwrap();
        fs::write(
            &b,
            "import { c } from \"./c.ts\";\nexport function b(): number { return c(); }\n",
        )
        .unwrap();
        fs::write(
            &a,
            "import { b } from \"./b.ts\";\nexport function main(): number { return b(); }\n",
        )
        .unwrap();
        let g = parse_module_graph(&a).unwrap();
        let _ = g.forward_deps().unwrap();
        let c_canon = c.canonicalize().unwrap();
        let rebuild = g
            .rebuild_transitive_importers(&HashSet::from([c_canon]))
            .unwrap();
        assert!(rebuild.len() >= 3);
        for pm in &g.modules {
            assert!(rebuild.contains(&ParsedModuleGraph::canonical_module_path(pm)));
        }
    }

    fn p(s: &str) -> PathBuf {
        PathBuf::from(s)
    }
}
