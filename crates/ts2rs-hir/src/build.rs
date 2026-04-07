//! 从 swc `Program` 构建 IR。

use std::collections::HashMap;

use swc_common::{sync::Lrc, SourceMap, Span, Spanned};
use swc_ecma_ast::{
    AssignOp, AssignTarget, BinaryOp, BindingIdent, Callee, Decl, EmptyStmt, ExportDecl, Expr,
    ExprOrSpread, FnDecl, ForStmt, KeyValueProp, Lit, MemberExpr, MemberProp, ModuleDecl,
    ModuleItem, OptChainBase, OptChainExpr, Param, Pat, Program, Prop, PropName, PropOrSpread,
    SimpleAssignTarget, Stmt, TsEntityName, TsIntersectionType, TsInterfaceDecl, TsKeywordTypeKind,
    TsLit, TsType as AstTsType, TsTypeAliasDecl, TsTypeAnn, TsTypeElement, TsUnionOrIntersectionType,
    TsUnionType, Tpl, UnaryOp, VarDecl, VarDeclKind, VarDeclOrExpr,
};

use crate::error::{diag, diag_spanned, CompileError};
use crate::ir::*;

pub fn build_module(
    program: &Program,
    cm: &Lrc<SourceMap>,
    path: &str,
) -> Result<IRModule, CompileError> {
    let mut next_id = 0u32;
    let iface = collect_named_types(program, cm, path)?;
    let fns = collect_fn_decls(program, cm, path, false, &mut next_id, &iface)?;
    if fns.is_empty() {
        let anchor = match program {
            Program::Module(m) => m.span,
            Program::Script(s) => s.span,
        };
        return Err(diag(
            cm,
            path,
            anchor,
            "no top-level function declarations found",
        ));
    }
    Ok(IRModule {
        fns,
        entry_path: path.to_string(),
    })
}

/// 多文件模块图：合并各模块中的顶层函数，要求全局函数名唯一；`entry_path` 用于语义上定位 `main`。
pub fn build_program_multi(
    units: &[(String, Program, Lrc<SourceMap>)],
    entry_path: &str,
) -> Result<IRModule, CompileError> {
    let mut next_id = 0u32;
    let mut all = Vec::new();
    for (path, program, cm) in units {
        let iface = collect_named_types(program, cm, path.as_str())?;
        let mut fns = collect_fn_decls(program, cm, path.as_str(), true, &mut next_id, &iface)?;
        all.append(&mut fns);
    }
    let mut seen = std::collections::HashSet::<String>::new();
    for f in &all {
        if !seen.insert(f.name.clone()) {
            return Err(diag(
                &f.cm,
                &f.source_path,
                f.span,
                format!("duplicate function `{}`", f.name),
            ));
        }
    }
    Ok(IRModule {
        fns: all,
        entry_path: entry_path.to_string(),
    })
}

/// 顶层 `interface` 声明体与对象类型字面量一致：必选、`number` 字段 → [`TsType::ObjectNum`]。
/// 同一文件内按**出现顺序**解析；引用尚未声明的接口名将报错。
fn object_num_from_type_elements(
    members: &[TsTypeElement],
    cm: &Lrc<SourceMap>,
    path: &str,
    dup_span: Span,
    iface: &HashMap<String, TsType>,
) -> Result<TsType, CompileError> {
    let mut keys: Vec<String> = Vec::new();
    for m in members {
        let TsTypeElement::TsPropertySignature(p) = m else {
            return Err(diag(
                cm,
                path,
                m.span(),
                "only property signatures are supported in object type literal",
            ));
        };
        if p.optional {
            return Err(diag(
                cm,
                path,
                p.span,
                "optional object fields are not supported",
            ));
        }
        let key = match &*p.key {
            Expr::Ident(i) => i.sym.to_string(),
            Expr::Lit(Lit::Str(s)) => s.value.to_string_lossy().into_owned(),
            _ => {
                return Err(diag(
                    cm,
                    path,
                    p.span,
                    "object type field key must be identifier or string literal",
                ));
            }
        };
        let Some(type_ann) = &p.type_ann else {
            return Err(diag(
                cm,
                path,
                p.span,
                "object type field annotation is required",
            ));
        };
        let ft = ts_type_from_ast(&type_ann.type_ann, cm, path, iface)?;
        if ft != TsType::Number {
            return Err(diag(
                cm,
                path,
                p.span,
                "only `number` object fields are supported",
            ));
        }
        keys.push(key);
    }
    keys.sort();
    for w in keys.windows(2) {
        if w[0] == w[1] {
            return Err(diag(
                cm,
                path,
                dup_span,
                format!("duplicate object type field `{}`", w[0]),
            ));
        }
    }
    Ok(TsType::ObjectNum(keys))
}

fn collect_named_types(
    program: &Program,
    cm: &Lrc<SourceMap>,
    path: &str,
) -> Result<HashMap<String, TsType>, CompileError> {
    let mut map = HashMap::new();
    match program {
        Program::Module(m) => {
            for item in &m.body {
                match item {
                    ModuleItem::Stmt(Stmt::Decl(Decl::TsInterface(i))) => {
                        collect_one_interface(i.as_ref(), &mut map, cm, path)?;
                    }
                    ModuleItem::Stmt(Stmt::Decl(Decl::TsTypeAlias(a))) => {
                        collect_one_type_alias(a.as_ref(), &mut map, cm, path)?;
                    }
                    ModuleItem::ModuleDecl(ModuleDecl::ExportDecl(ExportDecl { decl, .. })) => {
                        match decl {
                            Decl::TsInterface(i) => collect_one_interface(i.as_ref(), &mut map, cm, path)?,
                            Decl::TsTypeAlias(a) => collect_one_type_alias(a.as_ref(), &mut map, cm, path)?,
                            _ => {}
                        }
                    }
                    _ => {}
                }
            }
        }
        Program::Script(s) => {
            for stmt in &s.body {
                match stmt {
                    Stmt::Decl(Decl::TsInterface(i)) => {
                        collect_one_interface(i.as_ref(), &mut map, cm, path)?;
                    }
                    Stmt::Decl(Decl::TsTypeAlias(a)) => {
                        collect_one_type_alias(a.as_ref(), &mut map, cm, path)?;
                    }
                    _ => {}
                }
            }
        }
    }
    Ok(map)
}

fn collect_one_interface(
    d: &TsInterfaceDecl,
    map: &mut HashMap<String, TsType>,
    cm: &Lrc<SourceMap>,
    path: &str,
) -> Result<(), CompileError> {
    let name = d.id.sym.to_string();
    if map.contains_key(&name) {
        return Err(diag(
            cm,
            path,
            d.id.span,
            format!("duplicate type name `{}`", d.id.sym),
        ));
    }
    if d.type_params.is_some() {
        return Err(diag(
            cm,
            path,
            d.span,
            "generic interfaces are not supported",
        ));
    }
    if !d.extends.is_empty() {
        return Err(diag(
            cm,
            path,
            d.extends[0].span,
            "interface extends clauses are not supported",
        ));
    }
    let ty = object_num_from_type_elements(&d.body.body, cm, path, d.body.span, map)?;
    map.insert(name, ty);
    Ok(())
}

fn collect_one_type_alias(
    d: &TsTypeAliasDecl,
    map: &mut HashMap<String, TsType>,
    cm: &Lrc<SourceMap>,
    path: &str,
) -> Result<(), CompileError> {
    let name = d.id.sym.to_string();
    if map.contains_key(&name) {
        return Err(diag(
            cm,
            path,
            d.id.span,
            format!("duplicate type name `{}`", d.id.sym),
        ));
    }
    if d.type_params.is_some() {
        return Err(diag(
            cm,
            path,
            d.span,
            "generic type aliases are not supported",
        ));
    }
    let ty = ts_type_from_ast(d.type_ann.as_ref(), cm, path, map)?;
    map.insert(name, ty);
    Ok(())
}

fn collect_fn_decls(
    program: &Program,
    cm: &Lrc<SourceMap>,
    path: &str,
    allow_imports: bool,
    next_id: &mut u32,
    iface: &HashMap<String, TsType>,
) -> Result<Vec<IRFunction>, CompileError> {
    let mut out = Vec::new();
    match program {
        Program::Module(m) => {
            for item in &m.body {
                match item {
                    ModuleItem::Stmt(Stmt::Decl(Decl::TsInterface(_))) => {}
                    ModuleItem::Stmt(Stmt::Decl(Decl::TsTypeAlias(_))) => {}
                    ModuleItem::Stmt(Stmt::Decl(Decl::Fn(f))) if !f.declare => {
                        out.push(build_fn(f, cm, path, next_id, iface)?);
                    }
                    ModuleItem::Stmt(s) => {
                        return Err(diag_spanned(
                            cm,
                            path,
                            s,
                            "unsupported top-level statement (only top-level `function`, `interface`, and `type` declarations are supported)",
                        ));
                    }
                    ModuleItem::ModuleDecl(ModuleDecl::Import(_)) => {
                        if allow_imports {
                            continue;
                        }
                        return Err(diag(
                            cm,
                            path,
                            item.span(),
                            "`import` is not supported in this compiler version",
                        ));
                    }
                    ModuleItem::ModuleDecl(ModuleDecl::ExportDecl(ExportDecl { decl, .. })) => {
                        match decl {
                            Decl::TsInterface(_) | Decl::TsTypeAlias(_) => {}
                            Decl::Fn(f) if !f.declare => {
                                out.push(build_fn(f, cm, path, next_id, iface)?);
                            }
                            Decl::Fn(f) if f.declare => {
                                return Err(diag_spanned(
                                    cm,
                                    path,
                                    f,
                                    "`export declare function` is not supported",
                                ));
                            }
                            _ => {
                                return Err(diag_spanned(
                                    cm,
                                    path,
                                    decl,
                                    "unsupported export declaration (only `export function` / `export interface` / `export type` are supported)",
                                ));
                            }
                        }
                    }
                    ModuleItem::ModuleDecl(ModuleDecl::ExportNamed(e)) => {
                        return Err(diag_spanned(
                            cm,
                            path,
                            e,
                            "`export { ... }` / named re-exports are not supported (only `export function` is supported)",
                        ));
                    }
                    ModuleItem::ModuleDecl(ModuleDecl::ExportDefaultDecl(e)) => {
                        return Err(diag_spanned(
                            cm,
                            path,
                            e,
                            "`export default` is not supported (only `export function` is supported)",
                        ));
                    }
                    ModuleItem::ModuleDecl(ModuleDecl::ExportDefaultExpr(e)) => {
                        return Err(diag_spanned(
                            cm,
                            path,
                            e,
                            "`export default` is not supported (only `export function` is supported)",
                        ));
                    }
                    ModuleItem::ModuleDecl(ModuleDecl::ExportAll(e)) => {
                        return Err(diag_spanned(
                            cm,
                            path,
                            e,
                            "`export * from` / re-export-from is not supported in this compiler version",
                        ));
                    }
                    ModuleItem::ModuleDecl(ModuleDecl::TsImportEquals(e)) => {
                        return Err(diag_spanned(
                            cm,
                            path,
                            e,
                            "`import`/`export` TypeScript-specific forms are not supported in this compiler version",
                        ));
                    }
                    ModuleItem::ModuleDecl(ModuleDecl::TsExportAssignment(e)) => {
                        return Err(diag_spanned(
                            cm,
                            path,
                            e,
                            "`export =` is not supported in this compiler version",
                        ));
                    }
                    ModuleItem::ModuleDecl(ModuleDecl::TsNamespaceExport(e)) => {
                        return Err(diag_spanned(
                            cm,
                            path,
                            e,
                            "`export as namespace` is not supported in this compiler version",
                        ));
                    }
                }
            }
        }
        Program::Script(s) => {
            for stmt in &s.body {
                match stmt {
                    Stmt::Decl(Decl::TsInterface(_)) | Stmt::Decl(Decl::TsTypeAlias(_)) => {}
                    Stmt::Decl(Decl::Fn(f)) if !f.declare => {
                        out.push(build_fn(f, cm, path, next_id, iface)?);
                    }
                    _ => {}
                }
            }
        }
    }
    Ok(out)
}

fn build_fn(
    f: &FnDecl,
    cm: &Lrc<SourceMap>,
    path: &str,
    next_id: &mut u32,
    iface: &HashMap<String, TsType>,
) -> Result<IRFunction, CompileError> {
    let func = &f.function;
    if func.is_async {
        return Err(diag_spanned(
            cm,
            path,
            f,
            "async functions are not supported",
        ));
    }
    if func.is_generator {
        return Err(diag_spanned(
            cm,
            path,
            f,
            "generator functions are not supported",
        ));
    }
    if func.type_params.is_some() {
        return Err(diag_spanned(
            cm,
            path,
            f,
            "generic functions are not supported",
        ));
    }

    let ret = ts_type_from_ann(&func.return_type, cm, path, f.span(), iface)?;
    let mut params = Vec::new();
    for p in &func.params {
        let (name, ty) = param_binding(p, cm, path, iface)?;
        params.push((name, ty));
    }

    let body = func
        .body
        .as_ref()
        .ok_or_else(|| diag_spanned(cm, path, f, "function body is required"))?;

    let ir_id = *next_id;
    *next_id = next_id.saturating_add(1);

    let body_ir = build_block_stmts(&body.stmts, cm, path, next_id, iface)?;

    Ok(IRFunction {
        ir_id,
        name: f.ident.sym.to_string(),
        params,
        ret,
        body: body_ir,
        span: f.span(),
        cm: cm.clone(),
        source_path: path.to_string(),
    })
}

fn build_block_stmts(
    stmts: &[Stmt],
    cm: &Lrc<SourceMap>,
    path: &str,
    next_id: &mut u32,
    iface: &HashMap<String, TsType>,
) -> Result<Vec<IRStmt>, CompileError> {
    let mut v = Vec::new();
    for s in stmts {
        if let Stmt::For(f) = s {
            v.extend(build_for_stmt(f, cm, path, next_id, iface)?);
        } else {
            v.push(build_stmt(s, cm, path, next_id, iface)?);
        }
    }
    Ok(v)
}

fn build_for_stmt(
    f: &ForStmt,
    cm: &Lrc<SourceMap>,
    path: &str,
    next_id: &mut u32,
    iface: &HashMap<String, TsType>,
) -> Result<Vec<IRStmt>, CompileError> {
    let mut out = Vec::new();
    if let Some(init) = &f.init {
        match init {
            VarDeclOrExpr::VarDecl(vd) => {
                out.push(build_var_decl_from_vardecl(vd, cm, path, iface)?);
            }
            VarDeclOrExpr::Expr(e) => {
                out.push(IRStmt::Expr {
                    expr: build_expr(e, cm, path, iface)?,
                    span: e.span(),
                });
            }
        }
    }
    let cond = if let Some(t) = &f.test {
        build_expr(t, cm, path, iface)?
    } else {
        IRExpr::Number(1, f.span)
    };
    let mut body = match &*f.body {
        Stmt::Block(b) => build_block_stmts(&b.stmts, cm, path, next_id, iface)?,
        s => vec![build_stmt(s, cm, path, next_id, iface)?],
    };
    if let Some(up) = &f.update {
        match &**up {
            Expr::Assign(ax) => {
                if ax.op != AssignOp::Assign {
                    return Err(diag_spanned(
                        cm,
                        path,
                        ax,
                        "only `=` assignment is supported",
                    ));
                }
                match &ax.left {
                    AssignTarget::Simple(SimpleAssignTarget::Ident(i)) => {
                        let rhs = build_expr(&ax.right, cm, path, iface)?;
                        body.push(IRStmt::Assign {
                            name: i.id.sym.to_string(),
                            rhs,
                            span: ax.span,
                        });
                    }
                    _ => {
                        return Err(diag_spanned(
                            cm,
                            path,
                            ax,
                            "only assignment to a simple identifier is supported",
                        ));
                    }
                }
            }
            _ => {
                body.push(IRStmt::Expr {
                    expr: build_expr(up, cm, path, iface)?,
                    span: up.span(),
                });
            }
        }
    }
    out.push(IRStmt::While {
        cond,
        cond_ty: TsType::Number,
        body,
        span: f.span,
    });
    Ok(out)
}

fn build_var_decl_from_vardecl(
    v: &VarDecl,
    cm: &Lrc<SourceMap>,
    path: &str,
    iface: &HashMap<String, TsType>,
) -> Result<IRStmt, CompileError> {
    if v.decls.len() != 1 {
        return Err(diag_spanned(
            cm,
            path,
            v,
            "only one declarator per `let`/`const`/`var` is supported",
        ));
    }
    let d = &v.decls[0];
    let name = match &d.name {
        Pat::Ident(BindingIdent { id, .. }) => id.sym.to_string(),
        _ => {
            return Err(diag_spanned(
                cm,
                path,
                v,
                "only simple identifier bindings are supported",
            ));
        }
    };
    let init = d
        .init
        .as_ref()
        .ok_or_else(|| diag_spanned(cm, path, v, "initializer is required"))?;
    let ty = ts_type_from_pat_ann(&d.name, cm, path, iface)?;
    let expr = build_expr(init, cm, path, iface)?;
    let mutable = matches!(v.kind, VarDeclKind::Let | VarDeclKind::Var);
    Ok(IRStmt::Let {
        name,
        ty,
        init: expr,
        mutable,
        span: v.span,
    })
}

fn build_stmt(
    stmt: &Stmt,
    cm: &Lrc<SourceMap>,
    path: &str,
    next_id: &mut u32,
    iface: &HashMap<String, TsType>,
) -> Result<IRStmt, CompileError> {
    match stmt {
        Stmt::Empty(EmptyStmt { span }) => Ok(IRStmt::Empty { span: *span }),
        Stmt::Block(b) => Ok(IRStmt::Block {
            stmts: build_block_stmts(&b.stmts, cm, path, next_id, iface)?,
            span: b.span,
        }),
        Stmt::Return(r) => Ok(IRStmt::Return {
            arg: match &r.arg {
                Some(e) => Some(build_expr(e, cm, path, iface)?),
                None => None,
            },
            span: r.span,
        }),
        Stmt::Expr(e) => match &*e.expr {
            Expr::Assign(ax) => {
                if ax.op != AssignOp::Assign {
                    return Err(diag_spanned(
                        cm,
                        path,
                        ax,
                        "only `=` assignment is supported",
                    ));
                }
                match &ax.left {
                    AssignTarget::Simple(SimpleAssignTarget::Ident(i)) => {
                        let rhs = build_expr(&ax.right, cm, path, iface)?;
                        Ok(IRStmt::Assign {
                            name: i.id.sym.to_string(),
                            rhs,
                            span: ax.span,
                        })
                    }
                    _ => Err(diag_spanned(
                        cm,
                        path,
                        ax,
                        "only assignment to a simple identifier is supported",
                    )),
                }
            }
            _ => Ok(IRStmt::Expr {
                expr: build_expr(&e.expr, cm, path, iface)?,
                span: e.span,
            }),
        },
        Stmt::If(i) => {
            let cond = build_expr(&i.test, cm, path, iface)?;
            let then_b = match &*i.cons {
                Stmt::Block(b) => build_block_stmts(&b.stmts, cm, path, next_id, iface)?,
                s => vec![build_stmt(s, cm, path, next_id, iface)?],
            };
            let else_b = i
                .alt
                .as_ref()
                .map(|alt| match &**alt {
                    Stmt::Block(b) => build_block_stmts(&b.stmts, cm, path, next_id, iface),
                    s => Ok(vec![build_stmt(s, cm, path, next_id, iface)?]),
                })
                .transpose()?;
            Ok(IRStmt::If {
                cond,
                cond_ty: TsType::Number,
                then_b,
                else_b,
                span: i.span,
            })
        }
        Stmt::While(w) => {
            let cond = build_expr(&w.test, cm, path, iface)?;
            let body = match &*w.body {
                Stmt::Block(b) => build_block_stmts(&b.stmts, cm, path, next_id, iface)?,
                s => vec![build_stmt(s, cm, path, next_id, iface)?],
            };
            Ok(IRStmt::While {
                cond,
                cond_ty: TsType::Number,
                body,
                span: w.span,
            })
        }
        Stmt::DoWhile(dw) => {
            let body_ir = match &*dw.body {
                Stmt::Block(b) => build_block_stmts(&b.stmts, cm, path, next_id, iface)?,
                s => vec![build_stmt(s, cm, path, next_id, iface)?],
            };
            let cond = build_expr(&dw.test, cm, path, iface)?;
            Ok(IRStmt::DoWhile {
                body: body_ir,
                cond,
                cond_ty: TsType::Number,
                span: dw.span,
            })
        }
        Stmt::Break(b) => Ok(IRStmt::Break { span: b.span }),
        Stmt::Continue(c) => Ok(IRStmt::Continue { span: c.span }),
        Stmt::Decl(Decl::Var(v)) => build_var_decl_from_vardecl(v, cm, path, iface),
        Stmt::Decl(Decl::Fn(f)) => {
            if f.declare {
                return Err(diag_spanned(
                    cm,
                    path,
                    f,
                    "`declare function` is not supported",
                ));
            }
            let inner = build_fn(f, cm, path, next_id, iface)?;
            Ok(IRStmt::FnDecl {
                func: Box::new(inner),
                span: f.span(),
            })
        }
        Stmt::Switch(_) => Err(diag_spanned(
            cm,
            path,
            stmt,
            "`switch` is not supported in this compiler version",
        )),
        Stmt::ForIn(_) | Stmt::ForOf(_) => Err(diag_spanned(
            cm,
            path,
            stmt,
            "`for-in` / `for-of` are not supported in this compiler version",
        )),
        Stmt::Labeled(_) => Err(diag_spanned(
            cm,
            path,
            stmt,
            "labeled statements are not supported in this compiler version",
        )),
        _ => Err(diag_spanned(cm, path, stmt, "unsupported statement")),
    }
}

fn ts_type_from_pat_ann(
    pat: &Pat,
    cm: &Lrc<SourceMap>,
    path: &str,
    iface: &HashMap<String, TsType>,
) -> Result<TsType, CompileError> {
    match pat {
        Pat::Ident(BindingIdent { type_ann, .. }) => {
            ts_type_from_ann(type_ann, cm, path, pat.span(), iface)
        }
        _ => Err(diag(
            cm,
            path,
            pat.span(),
            "type annotation required for binding",
        )),
    }
}

fn ts_type_from_ann(
    ann: &Option<Box<TsTypeAnn>>,
    cm: &Lrc<SourceMap>,
    path: &str,
    fallback_span: Span,
    iface: &HashMap<String, TsType>,
) -> Result<TsType, CompileError> {
    let Some(ann) = ann else {
        return Err(diag(cm, path, fallback_span, "type annotation is required"));
    };
    ts_type_from_ast(&ann.type_ann, cm, path, iface)
}

fn ts_type_from_ast(
    ty: &AstTsType,
    cm: &Lrc<SourceMap>,
    path: &str,
    iface: &HashMap<String, TsType>,
) -> Result<TsType, CompileError> {
    match ty {
        AstTsType::TsKeywordType(k) => match k.kind {
            TsKeywordTypeKind::TsNumberKeyword => Ok(TsType::Number),
            TsKeywordTypeKind::TsBooleanKeyword => Ok(TsType::Boolean),
            TsKeywordTypeKind::TsStringKeyword => Ok(TsType::String),
            TsKeywordTypeKind::TsVoidKeyword => Ok(TsType::Void),
            TsKeywordTypeKind::TsNullKeyword => Ok(TsType::Null),
            TsKeywordTypeKind::TsUndefinedKeyword => Ok(TsType::Undefined),
            _ => Err(diag(
                cm,
                path,
                k.span,
                "only `number`, `boolean`, `string`, `void`, `null`, `undefined` keyword types are supported",
            )),
        },
        AstTsType::TsArrayType(a) => {
            let elem = ts_type_from_ast(&a.elem_type, cm, path, iface)?;
            if elem == TsType::Number {
                Ok(TsType::ArrayNumber)
            } else {
                Err(diag(
                    cm,
                    path,
                    a.span,
                    "only `number[]` is supported for array type annotation (not numeric literal types)",
                ))
            }
        }
        AstTsType::TsLitType(lt) => match &lt.lit {
            TsLit::Number(n) => {
                let v = n.value;
                if v.fract() != 0.0 || !v.is_finite() {
                    return Err(diag(
                        cm,
                        path,
                        lt.span,
                        "only integer literal types are supported for `number` literals in type position",
                    ));
                }
                if v < i32::MIN as f64 || v > i32::MAX as f64 {
                    return Err(diag(
                        cm,
                        path,
                        lt.span,
                        "numeric literal type is out of range for `i32`",
                    ));
                }
                Ok(TsType::NumberLit(v as i32))
            }
            TsLit::Str(s) => Ok(TsType::StringLit(s.value.to_string_lossy().into_owned())),
            TsLit::Bool(b) => Ok(TsType::BoolLit(b.value)),
            TsLit::BigInt(_) => Err(diag(
                cm,
                path,
                lt.span,
                "`bigint` literal types are not supported",
            )),
            TsLit::Tpl(_) => Err(diag(
                cm,
                path,
                lt.span,
                "template literal types are not supported",
            )),
        },
        AstTsType::TsTypeLit(tl) => {
            object_num_from_type_elements(&tl.members, cm, path, tl.span, iface)
        }
        AstTsType::TsTypeRef(r) => {
            if r.type_params.is_some() {
                return Err(diag(
                    cm,
                    path,
                    r.span,
                    "type arguments on type references are not supported",
                ));
            }
            match &r.type_name {
                TsEntityName::Ident(id) => {
                    let name = id.sym.to_string();
                    iface.get(&name).cloned().ok_or_else(|| {
                        diag(
                            cm,
                            path,
                            r.span,
                            format!("unknown type name `{}`", id.sym),
                        )
                    })
                }
                TsEntityName::TsQualifiedName(q) => Err(diag(
                    cm,
                    path,
                    q.span,
                    "qualified type names are not supported",
                )),
            }
        }
        AstTsType::TsUnionOrIntersectionType(u) => match u {
            TsUnionOrIntersectionType::TsUnionType(TsUnionType { types, span }) => {
                if types.is_empty() {
                    return Err(diag(
                        cm,
                        path,
                        *span,
                        "union type must have at least one member",
                    ));
                }
                let mut members = Vec::with_capacity(types.len());
                for t in types {
                    members.push(ts_type_from_ast(t.as_ref(), cm, path, iface)?);
                }
                Ok(normalize_union(members))
            }
            TsUnionOrIntersectionType::TsIntersectionType(TsIntersectionType { span, .. }) => Err(
                diag(
                    cm,
                    path,
                    *span,
                    "intersection types are not supported",
                ),
            ),
        },
        _ => Err(diag(
            cm,
            path,
            ty.span(),
            "unsupported type annotation",
        )),
    }
}

fn param_binding(
    p: &Param,
    cm: &Lrc<SourceMap>,
    path: &str,
    iface: &HashMap<String, TsType>,
) -> Result<(String, TsType), CompileError> {
    match &p.pat {
        Pat::Ident(BindingIdent { id, type_ann, .. }) => {
            let ty = ts_type_from_ann(type_ann, cm, path, id.span, iface)?;
            Ok((id.sym.to_string(), ty))
        }
        _ => Err(diag_spanned(
            cm,
            path,
            p,
            "only simple identifier parameters are supported",
        )),
    }
}

fn build_expr(
    expr: &Expr,
    cm: &Lrc<SourceMap>,
    path: &str,
    iface: &HashMap<String, TsType>,
) -> Result<IRExpr, CompileError> {
    match expr {
        Expr::Lit(ref l) => match l {
            Lit::Num(n) => {
                let v = n.value;
                if v.fract() != 0.0 {
                    return Err(diag_spanned(
                        cm,
                        path,
                        n,
                        "only integer numeric literals are supported",
                    ));
                }
                if v < (i32::MIN as f64) || v > (i32::MAX as f64) {
                    return Err(diag_spanned(
                        cm,
                        path,
                        n,
                        "numeric literal out of i32 range",
                    ));
                }
                Ok(IRExpr::Number(v as i32, n.span))
            }
            Lit::Bool(b) => Ok(IRExpr::Bool(b.value, b.span)),
            Lit::Str(s) => Ok(IRExpr::Str(s.value.to_string_lossy().into_owned(), s.span)),
            Lit::Null(n) => Ok(IRExpr::Null(n.span)),
            _ => Err(diag_spanned(cm, path, l, "unsupported literal")),
        },
        Expr::Ident(i) => Ok(IRExpr::Ident(i.sym.to_string(), i.span)),
        Expr::Paren(p) => build_expr(&p.expr, cm, path, iface),
        Expr::Unary(u) => {
            let op = match u.op {
                UnaryOp::Minus => IRUnaryOp::Neg,
                UnaryOp::Bang => IRUnaryOp::Not,
                UnaryOp::Void => {
                    return Ok(IRExpr::Undefined(u.span));
                }
                _ => {
                    return Err(diag_spanned(
                        cm,
                        path,
                        u,
                        "only unary `-` and `!` are supported",
                    ));
                }
            };
            let arg = build_expr(&u.arg, cm, path, iface)?;
            Ok(IRExpr::Unary {
                op,
                arg: Box::new(arg),
                span: u.span,
            })
        }
        Expr::Bin(b) => {
            if b.op == BinaryOp::NullishCoalescing {
                return Ok(IRExpr::NullishCoalesce {
                    left: Box::new(build_expr(&b.left, cm, path, iface)?),
                    right: Box::new(build_expr(&b.right, cm, path, iface)?),
                    span: b.span,
                });
            }
            let op = match b.op {
                BinaryOp::Add => IRBinOp::Add,
                BinaryOp::Sub => IRBinOp::Sub,
                BinaryOp::Mul => IRBinOp::Mul,
                BinaryOp::Div => IRBinOp::Div,
                BinaryOp::EqEq | BinaryOp::EqEqEq => IRBinOp::Eq,
                BinaryOp::NotEq | BinaryOp::NotEqEq => IRBinOp::Ne,
                BinaryOp::Lt => IRBinOp::Lt,
                BinaryOp::LtEq => IRBinOp::Le,
                BinaryOp::Gt => IRBinOp::Gt,
                BinaryOp::GtEq => IRBinOp::Ge,
                BinaryOp::LogicalAnd => IRBinOp::LogicalAnd,
                BinaryOp::LogicalOr => IRBinOp::LogicalOr,
                _ => {
                    return Err(diag_spanned(cm, path, b, "unsupported binary operator"));
                }
            };
            let left = build_expr(&b.left, cm, path, iface)?;
            let right = build_expr(&b.right, cm, path, iface)?;
            Ok(IRExpr::Binary {
                op,
                left: Box::new(left),
                right: Box::new(right),
                span: b.span,
                kind: None,
            })
        }
        Expr::Cond(c) => Ok(IRExpr::Conditional {
            test: Box::new(build_expr(&c.test, cm, path, iface)?),
            cons: Box::new(build_expr(&c.cons, cm, path, iface)?),
            alt: Box::new(build_expr(&c.alt, cm, path, iface)?),
            span: c.span,
            cond_ty: None,
        }),
        Expr::Seq(s) => {
            if s.exprs.is_empty() {
                return Err(diag_spanned(
                    cm,
                    path,
                    s,
                    "empty sequence expression is not supported",
                ));
            }
            let mut exprs = Vec::with_capacity(s.exprs.len());
            for e in &s.exprs {
                exprs.push(build_expr(e, cm, path, iface)?);
            }
            Ok(IRExpr::Seq {
                exprs,
                span: s.span,
            })
        }
        Expr::Tpl(t) => build_tpl(t, cm, path, iface),
        Expr::TaggedTpl(t) => Err(diag_spanned(
            cm,
            path,
            t,
            "tagged template literals are not supported in this compiler version",
        )),
        Expr::Member(m) => build_member_expr(m, cm, path, iface),
        Expr::OptChain(o) => build_opt_chain_expr(o, cm, path, iface),
        Expr::Array(a) => build_array_expr(a, cm, path, iface),
        Expr::Object(o) => build_object_expr(o, cm, path, iface),
        Expr::Call(c) => {
            if let Callee::Expr(ce) = &c.callee {
                if let Expr::Member(m) = &**ce {
                    if let Expr::Ident(obj) = &*m.obj {
                        if obj.sym == "console" {
                            if let MemberProp::Ident(prop) = &m.prop {
                                if prop.sym == "log" {
                                    let mut args = Vec::new();
                                    for a in &c.args {
                                        match a {
                                            ExprOrSpread { spread: None, expr } => {
                                                args.push(build_expr(expr, cm, path, iface)?);
                                            }
                                            _ => {
                                                return Err(diag_spanned(
                                                    cm,
                                                    path,
                                                    c,
                                                    "spread arguments are not supported",
                                                ));
                                            }
                                        }
                                    }
                                    return Ok(IRExpr::BuiltinLog { args, span: c.span });
                                }
                            }
                        }
                    }
                }
                if let Expr::Ident(i) = &**ce {
                    let mut args = Vec::new();
                    for a in &c.args {
                        match a {
                            ExprOrSpread { spread: None, expr } => {
                                args.push(build_expr(expr, cm, path, iface)?);
                            }
                            _ => {
                                return Err(diag_spanned(
                                    cm,
                                    path,
                                    c,
                                    "spread arguments are not supported",
                                ));
                            }
                        }
                    }
                    return Ok(IRExpr::Call {
                        callee: i.sym.to_string(),
                        args,
                        span: c.span,
                    });
                }
            }
            Err(diag_spanned(
                cm,
                path,
                c,
                "only direct calls `f(...)` or `console.log(...)` are supported",
            ))
        }
        _ => Err(diag_spanned(cm, path, expr, "unsupported expression")),
    }
}

fn build_tpl(
    t: &Tpl,
    cm: &Lrc<SourceMap>,
    path: &str,
    iface: &HashMap<String, TsType>,
) -> Result<IRExpr, CompileError> {
    if t.quasis.len() != t.exprs.len() + 1 {
        return Err(diag_spanned(
            cm,
            path,
            t,
            "invalid template literal structure",
        ));
    }
    let mut parts = Vec::new();
    for (i, q) in t.quasis.iter().enumerate() {
        let s = q.raw.to_string();
        parts.push(TplPart::Static(s));
        if i < t.exprs.len() {
            parts.push(TplPart::Interp(Box::new(build_expr(&t.exprs[i], cm, path, iface)?)));
        }
    }
    Ok(IRExpr::Tpl {
        parts,
        span: t.span,
    })
}

fn build_member_expr(
    m: &MemberExpr,
    cm: &Lrc<SourceMap>,
    path: &str,
    iface: &HashMap<String, TsType>,
) -> Result<IRExpr, CompileError> {
    let obj = Box::new(build_expr(&m.obj, cm, path, iface)?);
    match &m.prop {
        MemberProp::Ident(id) => Ok(IRExpr::Member {
            obj,
            prop: id.sym.to_string(),
            span: m.span,
        }),
        MemberProp::Computed(c) => Ok(IRExpr::Index {
            obj,
            index: Box::new(build_expr(&c.expr, cm, path, iface)?),
            span: m.span,
        }),
        _ => Err(diag_spanned(
            cm,
            path,
            m,
            "only identifier property access and computed index are supported",
        )),
    }
}

fn build_opt_chain_expr(
    o: &OptChainExpr,
    cm: &Lrc<SourceMap>,
    path: &str,
    iface: &HashMap<String, TsType>,
) -> Result<IRExpr, CompileError> {
    match &*o.base {
        OptChainBase::Call(_) => Err(diag_spanned(
            cm,
            path,
            o,
            "optional call (`?.()`) is not supported in this compiler version",
        )),
        OptChainBase::Member(m) => {
            let prop = match &m.prop {
                MemberProp::Ident(id) => id.sym.to_string(),
                _ => {
                    return Err(diag_spanned(
                        cm,
                        path,
                        m,
                        "optional chaining currently supports only identifier property access",
                    ));
                }
            };
            if o.optional {
                Ok(IRExpr::OptionalMember {
                    obj: Box::new(build_expr(&m.obj, cm, path, iface)?),
                    prop,
                    span: o.span,
                })
            } else {
                Ok(IRExpr::Member {
                    obj: Box::new(build_expr(&m.obj, cm, path, iface)?),
                    prop,
                    span: o.span,
                })
            }
        }
    }
}

fn build_array_expr(
    a: &swc_ecma_ast::ArrayLit,
    cm: &Lrc<SourceMap>,
    path: &str,
    iface: &HashMap<String, TsType>,
) -> Result<IRExpr, CompileError> {
    let mut elems = Vec::with_capacity(a.elems.len());
    for it in &a.elems {
        let Some(e) = it else {
            return Err(diag_spanned(
                cm,
                path,
                a,
                "array holes are not supported in this compiler version",
            ));
        };
        if e.spread.is_some() {
            return Err(diag_spanned(
                cm,
                path,
                a,
                "array spread elements are not supported in this compiler version",
            ));
        }
        elems.push(build_expr(&e.expr, cm, path, iface)?);
    }
    Ok(IRExpr::ArrayLit { elems, span: a.span })
}

fn build_object_expr(
    o: &swc_ecma_ast::ObjectLit,
    cm: &Lrc<SourceMap>,
    path: &str,
    iface: &HashMap<String, TsType>,
) -> Result<IRExpr, CompileError> {
    let mut fields: Vec<(String, IRExpr)> = Vec::new();
    for p in &o.props {
        match p {
            PropOrSpread::Spread(_) => {
                return Err(diag_spanned(
                    cm,
                    path,
                    o,
                    "object spread is not supported in this compiler version",
                ));
            }
            PropOrSpread::Prop(pp) => match &**pp {
                Prop::KeyValue(KeyValueProp { key, value }) => {
                    let k = match key {
                        PropName::Ident(i) => i.sym.to_string(),
                        PropName::Str(s) => s.value.to_string_lossy().into_owned(),
                        _ => {
                            return Err(diag_spanned(
                                cm,
                                path,
                                o,
                                "object literal key must be identifier or string literal",
                            ));
                        }
                    };
                    fields.push((k, build_expr(value, cm, path, iface)?));
                }
                _ => {
                    return Err(diag_spanned(
                        cm,
                        path,
                        o,
                        "only `key: value` object properties are supported",
                    ));
                }
            },
        }
    }
    fields.sort_by(|a, b| a.0.cmp(&b.0));
    for w in fields.windows(2) {
        if w[0].0 == w[1].0 {
            return Err(diag(
                cm,
                path,
                o.span,
                format!("duplicate object literal key `{}`", w[0].0),
            ));
        }
    }
    Ok(IRExpr::ObjectLit {
        fields,
        span: o.span,
    })
}
