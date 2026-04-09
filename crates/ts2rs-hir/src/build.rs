//! 从 swc `Program` 构建 IR。

use std::collections::{HashMap, HashSet};

use swc_common::comments::{Comment, CommentKind, SingleThreadedComments};
use swc_common::{sync::Lrc, SourceMap, Span, Spanned};
use swc_ecma_ast::{
    AssignOp, AssignTarget, BinaryOp, BindingIdent, CallExpr, Callee, ClassDecl, ClassMember, Decl,
    EmptyStmt, ExportDecl, Expr, ExprOrSpread, FnDecl, ForHead, ForStmt, KeyValueProp, Lit,
    MemberExpr, MemberProp, MethodKind, ModuleDecl, ModuleItem, ObjectLit, OptCall, OptChainBase,
    OptChainExpr, Param, ParamOrTsParamProp, Pat, Program, Prop, PropName, PropOrSpread,
    SimpleAssignTarget, Stmt, SwitchStmt, Tpl, TsTypeAnn, UnaryOp, VarDecl, VarDeclKind,
    VarDeclOrExpr,
};

use crate::error::{diag, diag_spanned, push_diag, CompileError};
use crate::ir::*;
use crate::json_parse_fold::{fold_json_parse_arg, JsonParseFold};

mod build_types;

fn freeze_leading_comments(c: &SingleThreadedComments) -> TsLeadingComments {
    let mut out = TsLeadingComments::new();
    let (leading, _) = c.borrow_all();
    for (pos, cmts) in leading.iter() {
        let mut lines: Vec<String> = Vec::new();
        for cmt in cmts {
            lines.extend(comment_to_lines(cmt));
        }
        if !lines.is_empty() {
            out.insert(pos.0, lines);
        }
    }
    out
}

fn comment_to_lines(c: &Comment) -> Vec<String> {
    let t = c.text.as_str();
    match c.kind {
        CommentKind::Line => {
            let s = sanitize_ts_comment_line(t);
            if s.is_empty() {
                vec![]
            } else {
                vec![s]
            }
        }
        CommentKind::Block => t
            .lines()
            .map(sanitize_ts_comment_line)
            .filter(|s| !s.is_empty())
            .collect(),
    }
}

fn sanitize_ts_comment_line(s: &str) -> String {
    let s = s.replace('\r', "");
    let s = s.trim();
    let s = s.trim_start_matches('*').trim();
    s.to_string()
}

pub fn build_module(
    program: &Program,
    cm: &Lrc<SourceMap>,
    path: &str,
    comments: Option<&SingleThreadedComments>,
) -> Result<IRModule, CompileError> {
    let mut next_id = 0u32;
    let mut errs = Vec::new();
    let iface = build_types::collect_named_types_with_errors(program, cm, path, &mut errs);
    let classes = collect_class_decls(program, cm, path, &iface, &mut errs);
    let fns = collect_fn_decls(program, cm, path, false, &mut next_id, &iface, &mut errs);
    let mut lowered = lower_classes_to_functions(&classes, cm, path, &mut next_id, &iface)?;
    let mut all_fns = fns;
    all_fns.append(&mut lowered);
    if all_fns.is_empty() {
        let anchor = match program {
            Program::Module(m) => m.span,
            Program::Script(s) => s.span,
        };
        push_diag(
            &mut errs,
            cm.as_ref(),
            path,
            anchor,
            "no top-level function declarations found",
        );
    }
    if !errs.is_empty() {
        return Err(CompileError::merge_sorted(errs));
    }
    let mut ts_comments_by_path = HashMap::new();
    if let Some(c) = comments {
        let frozen = freeze_leading_comments(c);
        if !frozen.is_empty() {
            ts_comments_by_path.insert(path.to_string(), frozen);
        }
    }
    Ok(IRModule {
        fns: all_fns,
        classes,
        generic_types: HashMap::new(),
        entry_path: path.to_string(),
        ts_comments_by_path,
    })
}

/// 多文件模块图：合并各模块中的顶层函数，要求全局函数名唯一；`entry_path` 用于语义上定位 `main`。
pub fn build_program_multi(
    units: &[(String, Program, Lrc<SourceMap>, SingleThreadedComments)],
    entry_path: &str,
) -> Result<IRModule, CompileError> {
    let mut next_id = 0u32;
    let mut all = Vec::new();
    let mut classes_all = Vec::new();
    let mut all_errs = Vec::new();
    let mut ts_comments_by_path = HashMap::new();
    for (path, program, cm, file_comments) in units {
        let mut errs = Vec::new();
        let iface =
            build_types::collect_named_types_with_errors(program, cm, path.as_str(), &mut errs);
        let classes = collect_class_decls(program, cm, path.as_str(), &iface, &mut errs);
        let mut fns = collect_fn_decls(
            program,
            cm,
            path.as_str(),
            true,
            &mut next_id,
            &iface,
            &mut errs,
        );
        let mut lowered =
            lower_classes_to_functions(&classes, cm, path.as_str(), &mut next_id, &iface)?;
        classes_all.extend(classes);
        fns.append(&mut lowered);
        all.append(&mut fns);
        all_errs.extend(errs);
        let frozen = freeze_leading_comments(file_comments);
        if !frozen.is_empty() {
            ts_comments_by_path.insert(path.clone(), frozen);
        }
    }
    let mut seen = std::collections::HashSet::<String>::new();
    for f in &all {
        if !seen.insert(f.name.clone()) {
            push_diag(
                &mut all_errs,
                f.cm.as_ref(),
                &f.source_path,
                f.span,
                format!("duplicate function `{}`", f.name),
            );
        }
    }
    if !all_errs.is_empty() {
        return Err(CompileError::merge_sorted(all_errs));
    }
    Ok(IRModule {
        fns: all,
        classes: classes_all,
        generic_types: HashMap::new(),
        entry_path: entry_path.to_string(),
        ts_comments_by_path,
    })
}

fn collect_class_decls(
    program: &Program,
    cm: &Lrc<SourceMap>,
    path: &str,
    iface: &HashMap<String, TsType>,
    errs: &mut Vec<CompileError>,
) -> Vec<IRClass> {
    let mut out = Vec::new();
    let mut push_class = |c: &ClassDecl| -> Result<(), CompileError> {
        if c.declare {
            return Err(diag_spanned(
                cm,
                path,
                c,
                "`declare class` is not supported",
            ));
        }
        let class_name = c.ident.sym.to_string();
        let extends = match &c.class.super_class {
            Some(e) => match &**e {
                Expr::Ident(i) => Some(i.sym.to_string()),
                _ => {
                    return Err(diag_spanned(
                        cm,
                        path,
                        e,
                        "`extends` currently supports only identifier base class",
                    ));
                }
            },
            None => None,
        };
        let mut fields = Vec::new();
        let mut methods = Vec::new();
        let mut ctor = None;
        for m in &c.class.body {
            match m {
                ClassMember::ClassProp(p) => {
                    if p.is_static {
                        return Err(diag_spanned(
                            cm,
                            path,
                            p,
                            "static class fields are not supported",
                        ));
                    }
                    let PropName::Ident(id) = &p.key else {
                        return Err(diag_spanned(
                            cm,
                            path,
                            p,
                            "only identifier class fields are supported",
                        ));
                    };
                    let ty = ts_type_from_ann(&p.type_ann, cm, path, p.span, iface, None)?;
                    fields.push((id.sym.to_string(), ty));
                }
                ClassMember::Constructor(cons) => {
                    let mut params = Vec::new();
                    for p in &cons.params {
                        let ParamOrTsParamProp::Param(pp) = p else {
                            return Err(diag_spanned(
                                cm,
                                path,
                                p,
                                "parameter properties are not supported",
                            ));
                        };
                        params.push(param_binding(pp, cm, path, iface, None)?);
                    }
                    let body = match &cons.body {
                        Some(b) => build_block_stmts(&b.stmts, cm, path, &mut 0u32, iface, false)?,
                        None => Vec::new(),
                    };
                    ctor = Some(IRClassMethod {
                        name: "constructor".to_string(),
                        params,
                        ret: TsType::Void,
                        body,
                        is_override: false,
                        owner: class_name.clone(),
                        span: cons.span,
                    });
                }
                ClassMember::Method(m) => {
                    if m.is_static {
                        return Err(diag_spanned(
                            cm,
                            path,
                            m,
                            "static methods are not supported",
                        ));
                    }
                    if !matches!(m.kind, MethodKind::Method) {
                        return Err(diag_spanned(cm, path, m, "getter/setter are not supported"));
                    }
                    let PropName::Ident(id) = &m.key else {
                        return Err(diag_spanned(
                            cm,
                            path,
                            m,
                            "only identifier method names are supported",
                        ));
                    };
                    let mut params = Vec::new();
                    for p in &m.function.params {
                        params.push(param_binding(p, cm, path, iface, None)?);
                    }
                    let ret =
                        ts_type_from_ann(&m.function.return_type, cm, path, m.span, iface, None)?;
                    let body = match &m.function.body {
                        Some(b) => build_block_stmts(&b.stmts, cm, path, &mut 0u32, iface, false)?,
                        None => {
                            return Err(diag_spanned(cm, path, m, "method body is required"));
                        }
                    };
                    methods.push(IRClassMethod {
                        name: id.sym.to_string(),
                        params,
                        ret,
                        body,
                        is_override: m.is_override,
                        owner: class_name.clone(),
                        span: m.span,
                    });
                }
                _ => {
                    return Err(diag_spanned(
                        cm,
                        path,
                        m,
                        "unsupported class member in current OO subset",
                    ));
                }
            }
        }
        fields.sort_by(|a, b| a.0.cmp(&b.0));
        out.push(IRClass {
            name: class_name,
            extends,
            fields,
            ctor,
            methods,
            span: c.class.span,
            cm: SendSourceMap(cm.clone()),
            source_path: path.to_string(),
        });
        Ok(())
    };
    match program {
        Program::Module(m) => {
            for item in &m.body {
                match item {
                    ModuleItem::Stmt(Stmt::Decl(Decl::Class(c))) => {
                        if let Err(e) = push_class(c) {
                            errs.push(e);
                        }
                    }
                    ModuleItem::ModuleDecl(ModuleDecl::ExportDecl(ExportDecl {
                        decl: Decl::Class(c),
                        ..
                    })) => {
                        if let Err(e) = push_class(c) {
                            errs.push(e);
                        }
                    }
                    _ => {}
                }
            }
        }
        Program::Script(s) => {
            for stmt in &s.body {
                if let Stmt::Decl(Decl::Class(c)) = stmt {
                    if let Err(e) = push_class(c) {
                        errs.push(e);
                    }
                }
            }
        }
    }
    out
}

fn lower_classes_to_functions(
    classes: &[IRClass],
    cm: &Lrc<SourceMap>,
    path: &str,
    next_id: &mut u32,
    _iface: &HashMap<String, TsType>,
) -> Result<Vec<IRFunction>, CompileError> {
    fn merged_fields(classes: &[IRClass], c: &IRClass, out: &mut Vec<String>) {
        if let Some(p) = &c.extends {
            if let Some(pc) = classes.iter().find(|x| &x.name == p) {
                merged_fields(classes, pc, out);
            }
        }
        for (k, _) in &c.fields {
            if !out.iter().any(|x| x == k) {
                out.push(k.clone());
            }
        }
    }
    let mut out = Vec::new();
    for cls in classes {
        let mut field_keys = Vec::new();
        merged_fields(classes, cls, &mut field_keys);
        field_keys.sort();
        let self_ty = TsType::ObjectNum(field_keys.clone());
        let ctor_name = format!("{}_new", cls.name);
        let ctor_fn = if let Some(c) = &cls.ctor {
            let mut body = Vec::new();
            let mut ctor_body = c.body.clone();
            rewrite_this_in_stmts(&mut ctor_body, c.span);
            let mut consumed_super = false;
            if let Some(parent) = &cls.extends {
                if let Some(IRStmt::Expr {
                    expr:
                        IRExpr::Call {
                            callee,
                            args,
                            type_args: _,
                            span,
                        },
                    ..
                }) = ctor_body.first()
                {
                    if callee == "__super_ctor" {
                        body.push(IRStmt::Let {
                            name: "__self".to_string(),
                            ty: self_ty.clone(),
                            init: Some(IRExpr::Call {
                                callee: format!("{parent}_new"),
                                args: args.clone(),
                                type_args: Vec::new(),
                                span: *span,
                            }),
                            mutable: true,
                            span: *span,
                        });
                        consumed_super = true;
                    }
                }
                if !consumed_super {
                    body.push(IRStmt::Let {
                        name: "__self".to_string(),
                        ty: self_ty.clone(),
                        init: Some(IRExpr::Call {
                            callee: format!("{parent}_new"),
                            args: Vec::new(),
                            type_args: Vec::new(),
                            span: c.span,
                        }),
                        mutable: true,
                        span: c.span,
                    });
                }
            } else {
                body.push(IRStmt::Let {
                    name: "__self".to_string(),
                    ty: self_ty.clone(),
                    init: Some(IRExpr::ObjectLit {
                        fields: field_keys
                            .iter()
                            .map(|k| (k.clone(), IRExpr::Number(0.0, c.span)))
                            .collect(),
                        span: c.span,
                    }),
                    mutable: true,
                    span: c.span,
                });
            }
            for (idx, s) in ctor_body.into_iter().enumerate() {
                if consumed_super && idx == 0 {
                    continue;
                }
                body.push(s);
            }
            body.push(IRStmt::Return {
                arg: Some(IRExpr::Ident("__self".to_string(), c.span)),
                span: c.span,
            });
            IRFunction {
                ir_id: {
                    let id = *next_id;
                    *next_id = next_id.saturating_add(1);
                    id
                },
                name: ctor_name.clone(),
                type_params: Vec::new(),
                params: c.params.clone(),
                ret: self_ty.clone(),
                body,
                span: c.span,
                cm: SendSourceMap(cm.clone()),
                source_path: path.to_string(),
                is_async: false,
                mono_origin: None,
            }
        } else {
            IRFunction {
                ir_id: {
                    let id = *next_id;
                    *next_id = next_id.saturating_add(1);
                    id
                },
                name: ctor_name.clone(),
                type_params: Vec::new(),
                params: Vec::new(),
                ret: self_ty.clone(),
                body: vec![IRStmt::Return {
                    arg: Some(IRExpr::ObjectLit {
                        fields: field_keys
                            .iter()
                            .map(|k| (k.clone(), IRExpr::Number(0.0, cls.span)))
                            .collect(),
                        span: cls.span,
                    }),
                    span: cls.span,
                }],
                span: cls.span,
                cm: SendSourceMap(cm.clone()),
                source_path: path.to_string(),
                is_async: false,
                mono_origin: None,
            }
        };
        out.push(ctor_fn);
        for m in &cls.methods {
            let mut params = Vec::new();
            params.push(("__self".to_string(), self_ty.clone()));
            params.extend(m.params.clone());
            let mut body = m.body.clone();
            rewrite_this_in_stmts(&mut body, m.span);
            out.push(IRFunction {
                ir_id: {
                    let id = *next_id;
                    *next_id = next_id.saturating_add(1);
                    id
                },
                name: m.name.clone(),
                type_params: Vec::new(),
                params,
                ret: m.ret.clone(),
                body,
                span: m.span,
                cm: SendSourceMap(cm.clone()),
                source_path: path.to_string(),
                is_async: false,
                mono_origin: None,
            });
        }
    }
    Ok(out)
}

fn rewrite_this_in_stmts(stmts: &mut [IRStmt], span: Span) {
    for s in stmts {
        match s {
            IRStmt::Let { init, .. } => {
                if let Some(e) = init {
                    rewrite_this_in_expr(e, span);
                }
            }
            IRStmt::Assign { rhs, .. } | IRStmt::Expr { expr: rhs, .. } => {
                rewrite_this_in_expr(rhs, span)
            }
            IRStmt::MemberAssign { rhs, .. } => rewrite_this_in_expr(rhs, span),
            IRStmt::Return { arg, .. } => {
                if let Some(e) = arg {
                    rewrite_this_in_expr(e, span);
                }
            }
            IRStmt::Block { stmts, .. } => rewrite_this_in_stmts(stmts, span),
            IRStmt::If {
                cond,
                then_b,
                else_b,
                ..
            } => {
                rewrite_this_in_expr(cond, span);
                rewrite_this_in_stmts(then_b, span);
                if let Some(e) = else_b {
                    rewrite_this_in_stmts(e, span);
                }
            }
            IRStmt::While { cond, body, .. } => {
                rewrite_this_in_expr(cond, span);
                rewrite_this_in_stmts(body, span);
            }
            IRStmt::ForIn { target, body, .. } => {
                rewrite_this_in_expr(target, span);
                rewrite_this_in_stmts(body, span);
            }
            IRStmt::DoWhile { body, cond, .. } => {
                rewrite_this_in_stmts(body, span);
                rewrite_this_in_expr(cond, span);
            }
            IRStmt::FnDecl { func, .. } => rewrite_this_in_stmts(&mut func.body, span),
            IRStmt::Empty { .. } | IRStmt::Break { .. } | IRStmt::Continue { .. } => {}
        }
    }
}

fn rewrite_this_in_expr(e: &mut IRExpr, span: Span) {
    match e {
        IRExpr::This(_) => *e = IRExpr::Ident("__self".to_string(), span),
        IRExpr::Binary { left, right, .. } => {
            rewrite_this_in_expr(left, span);
            rewrite_this_in_expr(right, span);
        }
        IRExpr::Unary { arg, .. } => rewrite_this_in_expr(arg, span),
        IRExpr::Call { args, .. } | IRExpr::OptionalCall { args, .. } => {
            for a in args {
                rewrite_this_in_expr(a, span);
            }
        }
        IRExpr::MethodCall { receiver, args, .. }
        | IRExpr::OptionalMethodCall { receiver, args, .. } => {
            rewrite_this_in_expr(receiver, span);
            for a in args {
                rewrite_this_in_expr(a, span);
            }
        }
        IRExpr::BuiltinLog { args, .. }
        | IRExpr::MathBuiltin { args, .. }
        | IRExpr::NumberBuiltin { args, .. }
        | IRExpr::JsonBuiltin { args, .. }
        | IRExpr::UriBuiltin { args, .. } => {
            for a in args {
                rewrite_this_in_expr(a, span);
            }
        }
        IRExpr::StringMethodBuiltin { receiver, args, .. } => {
            rewrite_this_in_expr(receiver, span);
            for a in args {
                rewrite_this_in_expr(a, span);
            }
        }
        IRExpr::ReadStdinLine { .. } => {}
        IRExpr::Conditional {
            test, cons, alt, ..
        } => {
            rewrite_this_in_expr(test, span);
            rewrite_this_in_expr(cons, span);
            rewrite_this_in_expr(alt, span);
        }
        IRExpr::Seq { exprs, .. } => {
            for x in exprs {
                rewrite_this_in_expr(x, span);
            }
        }
        IRExpr::Tpl { parts, .. } => {
            for p in parts {
                if let TplPart::Interp(x) = p {
                    rewrite_this_in_expr(x, span);
                }
            }
        }
        IRExpr::Member { obj, .. } | IRExpr::OptionalMember { obj, .. } => {
            rewrite_this_in_expr(obj, span)
        }
        IRExpr::Fetch { url, init, .. } => {
            rewrite_this_in_expr(url, span);
            if let Some(i) = init {
                if let Some(b) = &mut i.body {
                    rewrite_this_in_expr(b.as_mut(), span);
                }
            }
        }
        IRExpr::HttpResponseMethodBuiltin { receiver, .. } => {
            rewrite_this_in_expr(receiver, span);
        }
        IRExpr::HttpResponseBodyGetReader { response, .. } => {
            rewrite_this_in_expr(response, span);
        }
        IRExpr::ReaderRead { .. } => {}
        IRExpr::NullishCoalesce { left, right, .. } => {
            rewrite_this_in_expr(left, span);
            rewrite_this_in_expr(right, span);
        }
        IRExpr::ArrayLit { elems, .. } => {
            for a in elems {
                rewrite_this_in_expr(a, span);
            }
        }
        IRExpr::ObjectLit { fields, .. } => {
            for (_, v) in fields {
                rewrite_this_in_expr(v, span);
            }
        }
        IRExpr::Index { obj, index, .. } => {
            rewrite_this_in_expr(obj, span);
            rewrite_this_in_expr(index, span);
        }
        IRExpr::ArrowFn { body, .. } => rewrite_this_in_stmts(body, span),
        IRExpr::Await { arg, .. } => rewrite_this_in_expr(arg, span),
        IRExpr::FetchText { url, .. } => rewrite_this_in_expr(url, span),
        IRExpr::PromiseAll { elems, .. } => {
            for a in elems {
                rewrite_this_in_expr(a, span);
            }
        }
        IRExpr::Number(..)
        | IRExpr::Bool(..)
        | IRExpr::Str(..)
        | IRExpr::Ident(..)
        | IRExpr::Null(..)
        | IRExpr::Undefined(..)
        | IRExpr::Super(..) => {}
    }
}

fn collect_fn_decls(
    program: &Program,
    cm: &Lrc<SourceMap>,
    path: &str,
    allow_imports: bool,
    next_id: &mut u32,
    iface: &HashMap<String, TsType>,
    errs: &mut Vec<CompileError>,
) -> Vec<IRFunction> {
    let mut out = Vec::new();
    match program {
        Program::Module(m) => {
            for item in &m.body {
                match item {
                    ModuleItem::Stmt(Stmt::Decl(Decl::TsInterface(_))) => {}
                    ModuleItem::Stmt(Stmt::Decl(Decl::TsTypeAlias(_))) => {}
                    ModuleItem::Stmt(Stmt::Decl(Decl::Class(_))) => {}
                    ModuleItem::Stmt(Stmt::Decl(Decl::Fn(f))) if !f.declare => {
                        match build_fn(f, cm, path, next_id, iface) {
                            Ok(ir) => out.push(ir),
                            Err(e) => errs.push(e),
                        }
                    }
                    ModuleItem::Stmt(s) => {
                        push_diag(
                            errs,
                            cm.as_ref(),
                            path,
                            s.span(),
                            "unsupported top-level statement (only top-level `function`, `interface`, and `type` declarations are supported)",
                        );
                    }
                    ModuleItem::ModuleDecl(ModuleDecl::Import(_)) => {
                        if allow_imports {
                            continue;
                        }
                        push_diag(
                            errs,
                            cm.as_ref(),
                            path,
                            item.span(),
                            "`import` is not supported in this compiler version",
                        );
                    }
                    ModuleItem::ModuleDecl(ModuleDecl::ExportDecl(ExportDecl { decl, .. })) => {
                        match decl {
                            Decl::TsInterface(_) | Decl::TsTypeAlias(_) => {}
                            Decl::Fn(f) if !f.declare => {
                                match build_fn(f, cm, path, next_id, iface) {
                                    Ok(ir) => out.push(ir),
                                    Err(e) => errs.push(e),
                                }
                            }
                            Decl::Class(_) => {}
                            Decl::Fn(f) if f.declare => {
                                push_diag(
                                    errs,
                                    cm.as_ref(),
                                    path,
                                    f.span(),
                                    "`export declare function` is not supported",
                                );
                            }
                            _ => {
                                push_diag(
                                    errs,
                                    cm.as_ref(),
                                    path,
                                    decl.span(),
                                    "unsupported export declaration (only `export function` / `export interface` / `export type` are supported)",
                                );
                            }
                        }
                    }
                    ModuleItem::ModuleDecl(ModuleDecl::ExportNamed(e)) => {
                        if e.src.is_some() && !e.type_only {
                            continue;
                        }
                        push_diag(
                            errs,
                            cm.as_ref(),
                            path,
                            e.span(),
                            "`export { ... }` without `from` is not supported (use `export function` or `export { x } from \"./file.ts\"`)",
                        );
                    }
                    ModuleItem::ModuleDecl(ModuleDecl::ExportDefaultDecl(e)) => {
                        push_diag(
                            errs,
                            cm.as_ref(),
                            path,
                            e.span(),
                            "`export default` is not supported (only `export function` is supported)",
                        );
                    }
                    ModuleItem::ModuleDecl(ModuleDecl::ExportDefaultExpr(e)) => {
                        push_diag(
                            errs,
                            cm.as_ref(),
                            path,
                            e.span(),
                            "`export default` is not supported (only `export function` is supported)",
                        );
                    }
                    ModuleItem::ModuleDecl(ModuleDecl::ExportAll(e)) => {
                        if !e.type_only {
                            continue;
                        }
                        push_diag(
                            errs,
                            cm.as_ref(),
                            path,
                            e.span(),
                            "`export type * from` is not supported in this compiler version",
                        );
                    }
                    ModuleItem::ModuleDecl(ModuleDecl::TsImportEquals(e)) => {
                        push_diag(
                            errs,
                            cm.as_ref(),
                            path,
                            e.span(),
                            "`import`/`export` TypeScript-specific forms are not supported in this compiler version",
                        );
                    }
                    ModuleItem::ModuleDecl(ModuleDecl::TsExportAssignment(e)) => {
                        push_diag(
                            errs,
                            cm.as_ref(),
                            path,
                            e.span(),
                            "`export =` is not supported in this compiler version",
                        );
                    }
                    ModuleItem::ModuleDecl(ModuleDecl::TsNamespaceExport(e)) => {
                        push_diag(
                            errs,
                            cm.as_ref(),
                            path,
                            e.span(),
                            "`export as namespace` is not supported in this compiler version",
                        );
                    }
                }
            }
        }
        Program::Script(s) => {
            for stmt in &s.body {
                match stmt {
                    Stmt::Decl(Decl::TsInterface(_))
                    | Stmt::Decl(Decl::TsTypeAlias(_))
                    | Stmt::Decl(Decl::Class(_)) => {}
                    Stmt::Decl(Decl::Fn(f)) if !f.declare => {
                        match build_fn(f, cm, path, next_id, iface) {
                            Ok(ir) => out.push(ir),
                            Err(e) => errs.push(e),
                        }
                    }
                    _ => {}
                }
            }
        }
    }
    out
}

fn build_fn(
    f: &FnDecl,
    cm: &Lrc<SourceMap>,
    path: &str,
    next_id: &mut u32,
    iface: &HashMap<String, TsType>,
) -> Result<IRFunction, CompileError> {
    let func = &f.function;
    if func.is_generator {
        return Err(diag_spanned(
            cm,
            path,
            f,
            "generator functions are not supported",
        ));
    }
    let mut fn_type_params = HashSet::new();
    let mut fn_type_params_vec = Vec::new();
    if let Some(tp) = &func.type_params {
        for p in &tp.params {
            let name = p.name.sym.to_string();
            if !fn_type_params.insert(name.clone()) {
                return Err(diag(
                    cm,
                    path,
                    p.span,
                    format!("duplicate type parameter `{}`", p.name.sym),
                ));
            }
            fn_type_params_vec.push(name);
        }
    }

    let ret_ann = ts_type_from_ann(
        &func.return_type,
        cm,
        path,
        f.span(),
        iface,
        if fn_type_params.is_empty() {
            None
        } else {
            Some(&fn_type_params)
        },
    )?;
    let (ret, is_async_fn) = if func.is_async {
        match ret_ann {
            TsType::Promise(inner) => match *inner {
                TsType::Number | TsType::String | TsType::Void => (*inner, true),
                _ => {
                    return Err(diag_spanned(
                        cm,
                        path,
                        f,
                        "async function `Promise<T>` requires `T` to be `number`, `string`, or `void`",
                    ));
                }
            },
            _ => {
                return Err(diag_spanned(
                    cm,
                    path,
                    f,
                    "async function must have explicit return type `Promise<...>`",
                ));
            }
        }
    } else {
        if matches!(ret_ann, TsType::Promise(_)) {
            return Err(diag_spanned(
                cm,
                path,
                f,
                "only `async function` may return `Promise<...>`",
            ));
        }
        (ret_ann, false)
    };
    let mut params = Vec::new();
    for p in &func.params {
        let (name, ty) = param_binding(
            p,
            cm,
            path,
            iface,
            if fn_type_params.is_empty() {
                None
            } else {
                Some(&fn_type_params)
            },
        )?;
        params.push((name, ty));
    }

    let body = func
        .body
        .as_ref()
        .ok_or_else(|| diag_spanned(cm, path, f, "function body is required"))?;

    let ir_id = *next_id;
    *next_id = next_id.saturating_add(1);

    let body_ir = build_block_stmts(&body.stmts, cm, path, next_id, iface, is_async_fn)?;

    Ok(IRFunction {
        ir_id,
        name: f.ident.sym.to_string(),
        type_params: fn_type_params_vec,
        params,
        ret,
        body: body_ir,
        span: f.span(),
        cm: SendSourceMap(cm.clone()),
        source_path: path.to_string(),
        is_async: is_async_fn,
        mono_origin: None,
    })
}

fn build_block_stmts(
    stmts: &[Stmt],
    cm: &Lrc<SourceMap>,
    path: &str,
    next_id: &mut u32,
    iface: &HashMap<String, TsType>,
    in_async: bool,
) -> Result<Vec<IRStmt>, CompileError> {
    let mut v = Vec::new();
    for s in stmts {
        if let Stmt::For(f) = s {
            v.extend(build_for_stmt(f, cm, path, next_id, iface, in_async)?);
        } else {
            v.push(build_stmt(s, cm, path, next_id, iface, in_async)?);
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
    in_async: bool,
) -> Result<Vec<IRStmt>, CompileError> {
    let mut out = Vec::new();
    if let Some(init) = &f.init {
        match init {
            VarDeclOrExpr::VarDecl(vd) => {
                out.push(build_var_decl_from_vardecl(vd, cm, path, iface, in_async)?);
            }
            VarDeclOrExpr::Expr(e) => {
                out.push(IRStmt::Expr {
                    expr: build_expr(e, cm, path, iface, in_async)?,
                    span: e.span(),
                });
            }
        }
    }
    let cond = if let Some(t) = &f.test {
        build_expr(t, cm, path, iface, in_async)?
    } else {
        IRExpr::Number(1.0, f.span)
    };
    let mut body = match &*f.body {
        Stmt::Block(b) => build_block_stmts(&b.stmts, cm, path, next_id, iface, in_async)?,
        s => vec![build_stmt(s, cm, path, next_id, iface, in_async)?],
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
                        let rhs = build_expr(&ax.right, cm, path, iface, in_async)?;
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
                    expr: build_expr(up, cm, path, iface, in_async)?,
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
    in_async: bool,
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
    let ty = ts_type_from_pat_ann(&d.name, cm, path, iface)?;
    let mutable = matches!(v.kind, VarDeclKind::Let | VarDeclKind::Var);
    let init = match &d.init {
        Some(init_expr) => Some(build_expr(init_expr, cm, path, iface, in_async)?),
        None => {
            if !mutable {
                return Err(diag_spanned(cm, path, v, "`const` requires an initializer"));
            }
            None
        }
    };
    Ok(IRStmt::Let {
        name,
        ty,
        init,
        mutable,
        span: v.span,
    })
}

/// Strip trailing `break;` at the end of a `case` clause (switch is lowered to `if`, so these breaks are not emitted as `IRStmt::Break`).
fn trim_trailing_breaks_in_case(cons: Vec<Stmt>) -> Vec<Stmt> {
    let mut v = cons;
    while matches!(v.last(), Some(Stmt::Break(_))) {
        v.pop();
    }
    v
}

fn expr_to_case_literal_ir(
    expr: &Expr,
    cm: &Lrc<SourceMap>,
    path: &str,
) -> Result<IRExpr, CompileError> {
    let e = match expr {
        Expr::Paren(p) => &*p.expr,
        _ => expr,
    };
    match e {
        Expr::Lit(Lit::Num(n)) => {
            let v = n.value;
            if v.fract() != 0.0 || v > i32::MAX as f64 || v < i32::MIN as f64 {
                return Err(diag_spanned(
                    cm,
                    path,
                    n,
                    "`case` numeric label must be an integer in `i32` range",
                ));
            }
            Ok(IRExpr::Number(v as f64, n.span))
        }
        Expr::Lit(Lit::Bool(b)) => Ok(IRExpr::Bool(b.value, b.span)),
        _ => Err(diag_spanned(
            cm,
            path,
            expr,
            "`case` label must be a `number` or `boolean` literal",
        )),
    }
}

fn case_literal_key(lit: &IRExpr) -> String {
    match lit {
        IRExpr::Number(n, _) => format!("n:{n}"),
        IRExpr::Bool(b, _) => format!("b:{b}"),
        _ => unreachable!("case literal must be number or bool"),
    }
}

fn build_switch_stmt(
    sw: &SwitchStmt,
    cm: &Lrc<SourceMap>,
    path: &str,
    next_id: &mut u32,
    iface: &HashMap<String, TsType>,
    in_async: bool,
) -> Result<IRStmt, CompileError> {
    if sw.cases.is_empty() {
        return Err(diag_spanned(
            cm,
            path,
            sw,
            "`switch` must contain at least one `case` or `default` clause",
        ));
    }

    let mut default_count = 0usize;
    for (idx, case) in sw.cases.iter().enumerate() {
        if case.test.is_none() {
            default_count += 1;
            if idx != sw.cases.len() - 1 {
                return Err(diag_spanned(
                    cm,
                    path,
                    case,
                    "`default` must be the last clause in `switch`",
                ));
            }
        }
    }
    if default_count > 1 {
        return Err(diag_spanned(
            cm,
            path,
            sw,
            "duplicate `default` clause in `switch`",
        ));
    }

    let disc = build_expr(&sw.discriminant, cm, path, iface, in_async)?;

    let mut labeled: Vec<(IRExpr, Vec<IRStmt>, Span)> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();

    let mut default_body: Option<Vec<IRStmt>> = None;

    for case in &sw.cases {
        if case.test.is_none() {
            if case.cons.is_empty() {
                return Err(diag_spanned(
                    cm,
                    path,
                    case,
                    "`default` clause cannot be empty",
                ));
            }
            let trimmed = trim_trailing_breaks_in_case(case.cons.clone());
            if trimmed.is_empty() {
                return Err(diag_spanned(
                    cm,
                    path,
                    case,
                    "`default` clause cannot be empty after trailing `break` removal",
                ));
            }
            default_body = Some(build_block_stmts(
                &trimmed, cm, path, next_id, iface, in_async,
            )?);
            break;
        }

        if case.cons.is_empty() {
            return Err(diag_spanned(
                cm,
                path,
                case,
                "empty `case` body (fall-through between `case` clauses is not supported)",
            ));
        }

        let test = case.test.as_ref().expect("case with test");
        let lit_ir = expr_to_case_literal_ir(test, cm, path)?;
        let key = case_literal_key(&lit_ir);
        if !seen.insert(key) {
            return Err(diag_spanned(cm, path, test, "duplicate `case` label"));
        }

        let trimmed = trim_trailing_breaks_in_case(case.cons.clone());
        if trimmed.is_empty() {
            return Err(diag_spanned(
                cm,
                path,
                case,
                "`case` body cannot be empty after trailing `break` removal",
            ));
        }

        let then_b = build_block_stmts(&trimmed, cm, path, next_id, iface, in_async)?;
        labeled.push((lit_ir, then_b, case.span));
    }

    if labeled.is_empty() {
        let body = default_body.ok_or_else(|| {
            diag_spanned(
                cm,
                path,
                sw,
                "`switch` must contain at least one `case` or `default` clause",
            )
        })?;
        return Ok(IRStmt::Block {
            stmts: body,
            span: sw.span,
        });
    }

    let mut rest = default_body;
    for (lit_ir, then_b, case_span) in labeled.into_iter().rev() {
        let cond = IRExpr::Binary {
            op: IRBinOp::Eq,
            left: Box::new(disc.clone()),
            right: Box::new(lit_ir),
            span: case_span,
            kind: None,
        };
        rest = Some(vec![IRStmt::If {
            cond,
            cond_ty: TsType::Number,
            then_b,
            else_b: rest,
            span: case_span,
        }]);
    }

    Ok(rest
        .expect("labeled non-empty implies chain built")
        .into_iter()
        .next()
        .expect("one if"))
}

fn build_for_in_stmt(
    fi: &swc_ecma_ast::ForInStmt,
    cm: &Lrc<SourceMap>,
    path: &str,
    next_id: &mut u32,
    iface: &HashMap<String, TsType>,
    in_async: bool,
) -> Result<IRStmt, CompileError> {
    let (key, key_ty) = match &fi.left {
        ForHead::VarDecl(v) => {
            if v.decls.len() != 1 {
                return Err(diag_spanned(
                    cm,
                    path,
                    v,
                    "for..in expects exactly one loop variable",
                ));
            }
            let d = &v.decls[0];
            if d.init.is_some() {
                return Err(diag_spanned(
                    cm,
                    path,
                    d,
                    "for..in loop variable initializer is not supported",
                ));
            }
            let Pat::Ident(BindingIdent { id, type_ann, .. }) = &d.name else {
                return Err(diag_spanned(
                    cm,
                    path,
                    d,
                    "for..in requires identifier loop variable",
                ));
            };
            let ty = if type_ann.is_some() {
                ts_type_from_ann(type_ann, cm, path, id.span, iface, None)?
            } else {
                TsType::String
            };
            (id.sym.to_string(), ty)
        }
        ForHead::Pat(p) => {
            let Pat::Ident(BindingIdent { id, type_ann, .. }) = &**p else {
                return Err(diag_spanned(
                    cm,
                    path,
                    p,
                    "for..in requires identifier loop variable",
                ));
            };
            let ty = if type_ann.is_some() {
                ts_type_from_ann(type_ann, cm, path, id.span, iface, None)?
            } else {
                TsType::String
            };
            (id.sym.to_string(), ty)
        }
        ForHead::UsingDecl(u) => {
            return Err(diag_spanned(
                cm,
                path,
                u,
                "for..in `using` declaration is not supported",
            ));
        }
    };
    let target = build_expr(&fi.right, cm, path, iface, in_async)?;
    let body = match &*fi.body {
        Stmt::Block(b) => build_block_stmts(&b.stmts, cm, path, next_id, iface, in_async)?,
        s => vec![build_stmt(s, cm, path, next_id, iface, in_async)?],
    };
    Ok(IRStmt::ForIn {
        key,
        key_ty,
        target,
        kind: None,
        body,
        span: fi.span,
    })
}

fn build_stmt(
    stmt: &Stmt,
    cm: &Lrc<SourceMap>,
    path: &str,
    next_id: &mut u32,
    iface: &HashMap<String, TsType>,
    in_async: bool,
) -> Result<IRStmt, CompileError> {
    match stmt {
        Stmt::Empty(EmptyStmt { span }) => Ok(IRStmt::Empty { span: *span }),
        Stmt::Block(b) => Ok(IRStmt::Block {
            stmts: build_block_stmts(&b.stmts, cm, path, next_id, iface, in_async)?,
            span: b.span,
        }),
        Stmt::Return(r) => Ok(IRStmt::Return {
            arg: match &r.arg {
                Some(e) => Some(build_expr(e, cm, path, iface, in_async)?),
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
                        let rhs = build_expr(&ax.right, cm, path, iface, in_async)?;
                        Ok(IRStmt::Assign {
                            name: i.id.sym.to_string(),
                            rhs,
                            span: ax.span,
                        })
                    }
                    AssignTarget::Simple(SimpleAssignTarget::Member(m)) => {
                        let obj = match &*m.obj {
                            Expr::Ident(i) => i.sym.to_string(),
                            Expr::This(_) => "__self".to_string(),
                            _ => {
                                return Err(diag_spanned(
                                    cm,
                                    path,
                                    m,
                                    "member assignment object must be identifier or `this`",
                                ));
                            }
                        };
                        let prop = match &m.prop {
                            MemberProp::Ident(id) => id.sym.to_string(),
                            _ => {
                                return Err(diag_spanned(
                                    cm,
                                    path,
                                    m,
                                    "computed member assignment is not supported",
                                ));
                            }
                        };
                        let rhs = build_expr(&ax.right, cm, path, iface, in_async)?;
                        Ok(IRStmt::MemberAssign {
                            obj,
                            prop,
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
                expr: build_expr(&e.expr, cm, path, iface, in_async)?,
                span: e.span,
            }),
        },
        Stmt::If(i) => {
            let cond = build_expr(&i.test, cm, path, iface, in_async)?;
            let then_b = match &*i.cons {
                Stmt::Block(b) => build_block_stmts(&b.stmts, cm, path, next_id, iface, in_async)?,
                s => vec![build_stmt(s, cm, path, next_id, iface, in_async)?],
            };
            let else_b = i
                .alt
                .as_ref()
                .map(|alt| match &**alt {
                    Stmt::Block(b) => {
                        build_block_stmts(&b.stmts, cm, path, next_id, iface, in_async)
                    }
                    s => Ok(vec![build_stmt(s, cm, path, next_id, iface, in_async)?]),
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
            let cond = build_expr(&w.test, cm, path, iface, in_async)?;
            let body = match &*w.body {
                Stmt::Block(b) => build_block_stmts(&b.stmts, cm, path, next_id, iface, in_async)?,
                s => vec![build_stmt(s, cm, path, next_id, iface, in_async)?],
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
                Stmt::Block(b) => build_block_stmts(&b.stmts, cm, path, next_id, iface, in_async)?,
                s => vec![build_stmt(s, cm, path, next_id, iface, in_async)?],
            };
            let cond = build_expr(&dw.test, cm, path, iface, in_async)?;
            Ok(IRStmt::DoWhile {
                body: body_ir,
                cond,
                cond_ty: TsType::Number,
                span: dw.span,
            })
        }
        Stmt::Break(b) => Ok(IRStmt::Break { span: b.span }),
        Stmt::Continue(c) => Ok(IRStmt::Continue { span: c.span }),
        Stmt::Decl(Decl::Var(v)) => build_var_decl_from_vardecl(v, cm, path, iface, in_async),
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
        Stmt::Switch(sw) => build_switch_stmt(sw, cm, path, next_id, iface, in_async),
        Stmt::ForIn(fi) => build_for_in_stmt(fi, cm, path, next_id, iface, in_async),
        Stmt::ForOf(_) => Err(diag_spanned(
            cm,
            path,
            stmt,
            "`for-of` is not supported in this compiler version",
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
    build_types::ts_type_from_pat_ann(pat, cm, path, iface)
}

fn ts_type_from_ann(
    ann: &Option<Box<TsTypeAnn>>,
    cm: &Lrc<SourceMap>,
    path: &str,
    fallback_span: Span,
    iface: &HashMap<String, TsType>,
    type_params: Option<&HashSet<String>>,
) -> Result<TsType, CompileError> {
    build_types::ts_type_from_ann(ann, cm, path, fallback_span, iface, type_params)
}

fn param_binding(
    p: &Param,
    cm: &Lrc<SourceMap>,
    path: &str,
    iface: &HashMap<String, TsType>,
    type_params: Option<&HashSet<String>>,
) -> Result<(String, TsType), CompileError> {
    match &p.pat {
        Pat::Ident(BindingIdent { id, type_ann, .. }) => {
            let ty = ts_type_from_ann(type_ann, cm, path, id.span, iface, type_params)?;
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

fn call_type_args(
    c: &swc_ecma_ast::CallExpr,
    cm: &Lrc<SourceMap>,
    path: &str,
    iface: &HashMap<String, TsType>,
) -> Result<Vec<TsType>, CompileError> {
    let Some(targs) = &c.type_args else {
        return Ok(Vec::new());
    };
    let mut out = Vec::with_capacity(targs.params.len());
    for t in &targs.params {
        out.push(build_types::ts_type_from_ast(t, cm, path, iface, None)?);
    }
    Ok(out)
}

fn build_arrow_expr(
    a: &swc_ecma_ast::ArrowExpr,
    cm: &Lrc<SourceMap>,
    path: &str,
    next_id: &mut u32,
    iface: &HashMap<String, TsType>,
) -> Result<IRExpr, CompileError> {
    let mut params = Vec::with_capacity(a.params.len());
    for p in &a.params {
        let Pat::Ident(BindingIdent { id, type_ann, .. }) = p else {
            return Err(diag_spanned(
                cm,
                path,
                p,
                "only simple identifier parameters are supported in arrow functions",
            ));
        };
        let ty = ts_type_from_ann(type_ann, cm, path, id.span, iface, None)?;
        params.push((id.sym.to_string(), ty));
    }
    let ret = ts_type_from_ann(&a.return_type, cm, path, a.span, iface, None)?;
    let body = match &*a.body {
        swc_ecma_ast::BlockStmtOrExpr::BlockStmt(b) => {
            build_block_stmts(&b.stmts, cm, path, next_id, iface, false)?
        }
        swc_ecma_ast::BlockStmtOrExpr::Expr(e) => vec![IRStmt::Return {
            arg: Some(build_expr(e, cm, path, iface, false)?),
            span: e.span(),
        }],
    };
    Ok(IRExpr::ArrowFn {
        params,
        ret,
        body,
        span: a.span,
    })
}

fn build_expr(
    expr: &Expr,
    cm: &Lrc<SourceMap>,
    path: &str,
    iface: &HashMap<String, TsType>,
    in_async: bool,
) -> Result<IRExpr, CompileError> {
    match expr {
        Expr::Await(a) => {
            if !in_async {
                return Err(diag_spanned(
                    cm,
                    path,
                    a,
                    "`await` is only allowed inside `async function` bodies",
                ));
            }
            Ok(IRExpr::Await {
                arg: Box::new(build_expr(&a.arg, cm, path, iface, in_async)?),
                span: a.span,
            })
        }
        Expr::Lit(ref l) => match l {
            Lit::Num(n) => {
                let v = n.value;
                if v.is_nan() || v.is_infinite() {
                    return Err(diag_spanned(cm, path, n, "numeric literal must be finite"));
                }
                Ok(IRExpr::Number(v, n.span))
            }
            Lit::Bool(b) => Ok(IRExpr::Bool(b.value, b.span)),
            Lit::Str(s) => Ok(IRExpr::Str(s.value.to_string_lossy().into_owned(), s.span)),
            Lit::Null(n) => Ok(IRExpr::Null(n.span)),
            _ => Err(diag_spanned(cm, path, l, "unsupported literal")),
        },
        Expr::Ident(i) => Ok(IRExpr::Ident(i.sym.to_string(), i.span)),
        Expr::Paren(p) => build_expr(&p.expr, cm, path, iface, in_async),
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
            let arg = build_expr(&u.arg, cm, path, iface, in_async)?;
            Ok(IRExpr::Unary {
                op,
                arg: Box::new(arg),
                span: u.span,
            })
        }
        Expr::Bin(b) => {
            if b.op == BinaryOp::NullishCoalescing {
                return Ok(IRExpr::NullishCoalesce {
                    left: Box::new(build_expr(&b.left, cm, path, iface, in_async)?),
                    right: Box::new(build_expr(&b.right, cm, path, iface, in_async)?),
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
            let left = build_expr(&b.left, cm, path, iface, in_async)?;
            let right = build_expr(&b.right, cm, path, iface, in_async)?;
            Ok(IRExpr::Binary {
                op,
                left: Box::new(left),
                right: Box::new(right),
                span: b.span,
                kind: None,
            })
        }
        Expr::Cond(c) => Ok(IRExpr::Conditional {
            test: Box::new(build_expr(&c.test, cm, path, iface, in_async)?),
            cons: Box::new(build_expr(&c.cons, cm, path, iface, in_async)?),
            alt: Box::new(build_expr(&c.alt, cm, path, iface, in_async)?),
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
                exprs.push(build_expr(e, cm, path, iface, in_async)?);
            }
            Ok(IRExpr::Seq {
                exprs,
                span: s.span,
            })
        }
        Expr::Tpl(t) => build_tpl(t, cm, path, iface, in_async),
        Expr::TaggedTpl(t) => Err(diag_spanned(
            cm,
            path,
            t,
            "tagged template literals are not supported in this compiler version",
        )),
        Expr::Member(m) => build_member_expr(m, cm, path, iface, in_async),
        Expr::OptChain(o) => build_opt_chain_expr(o, cm, path, iface, in_async),
        Expr::Array(a) => build_array_expr(a, cm, path, iface, in_async),
        Expr::Object(o) => build_object_expr(o, cm, path, iface, in_async),
        Expr::This(t) => Ok(IRExpr::This(t.span)),
        Expr::New(n) => {
            let callee = match &*n.callee {
                Expr::Ident(i) => format!("{}_new", i.sym),
                _ => {
                    return Err(diag_spanned(
                        cm,
                        path,
                        n,
                        "`new` currently supports only identifier callee",
                    ));
                }
            };
            let mut args = Vec::new();
            if let Some(a) = &n.args {
                for x in a {
                    if x.spread.is_some() {
                        return Err(diag_spanned(
                            cm,
                            path,
                            x,
                            "spread arguments are not supported",
                        ));
                    }
                    args.push(build_expr(&x.expr, cm, path, iface, in_async)?);
                }
            }
            Ok(IRExpr::Call {
                callee,
                args,
                type_args: Vec::new(),
                span: n.span,
            })
        }
        Expr::Arrow(a) => {
            let mut tmp_id = 0u32;
            build_arrow_expr(a, cm, path, &mut tmp_id, iface)
        }
        Expr::Call(c) => {
            let type_args = call_type_args(c, cm, path, iface)?;
            if let Callee::Super(_) = &c.callee {
                let mut args = Vec::new();
                for a in &c.args {
                    match a {
                        ExprOrSpread { spread: None, expr } => {
                            args.push(build_expr(expr, cm, path, iface, in_async)?);
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
                    callee: "__super_ctor".to_string(),
                    args,
                    type_args,
                    span: c.span,
                });
            }
            if let Callee::Expr(ce) = &c.callee {
                if let Expr::Member(m) = &**ce {
                    if let MemberProp::Ident(prop) = &m.prop {
                        if prop.sym == "then" {
                            return Err(diag_spanned(
                                cm,
                                path,
                                c,
                                "`Promise.prototype.then` is not supported; use `async`/`await` instead",
                            ));
                        }
                    }
                }
                if let Expr::Ident(fname) = &**ce {
                    if fname.sym == "fetchText" {
                        if !type_args.is_empty() {
                            return Err(diag_spanned(
                                cm,
                                path,
                                c,
                                "`fetchText` does not take type arguments",
                            ));
                        }
                        if c.args.len() != 1 {
                            return Err(diag_spanned(
                                cm,
                                path,
                                c,
                                "`fetchText` expects exactly one argument (url: string)",
                            ));
                        }
                        let a0 = match &c.args[0] {
                            ExprOrSpread { spread: None, expr } => expr,
                            _ => {
                                return Err(diag_spanned(
                                    cm,
                                    path,
                                    c,
                                    "spread arguments are not supported",
                                ));
                            }
                        };
                        let url = build_expr(a0, cm, path, iface, in_async)?;
                        return Ok(IRExpr::FetchText {
                            url: Box::new(url),
                            span: c.span,
                        });
                    }
                    if fname.sym == "fetch" {
                        if !type_args.is_empty() {
                            return Err(diag_spanned(
                                cm,
                                path,
                                c,
                                "`fetch` does not take type arguments",
                            ));
                        }
                        if c.args.is_empty() || c.args.len() > 2 {
                            return Err(diag_spanned(
                                cm,
                                path,
                                c,
                                "`fetch` expects one argument (url: string) or two (url, init object)",
                            ));
                        }
                        let a0 = match &c.args[0] {
                            ExprOrSpread { spread: None, expr } => expr,
                            _ => {
                                return Err(diag_spanned(
                                    cm,
                                    path,
                                    c,
                                    "spread arguments are not supported",
                                ));
                            }
                        };
                        let url = build_expr(a0, cm, path, iface, in_async)?;
                        let init = if c.args.len() == 2 {
                            let a1 = match &c.args[1] {
                                ExprOrSpread { spread: None, expr } => expr,
                                _ => {
                                    return Err(diag_spanned(
                                        cm,
                                        path,
                                        c,
                                        "spread arguments are not supported",
                                    ));
                                }
                            };
                            Some(build_fetch_init(a1, cm, path, iface, in_async)?)
                        } else {
                            None
                        };
                        return Ok(IRExpr::Fetch {
                            url: Box::new(url),
                            init,
                            span: c.span,
                        });
                    }
                    if matches!(
                        fname.sym.as_ref(),
                        "encodeURIComponent" | "decodeURIComponent"
                    ) {
                        if !type_args.is_empty() {
                            return Err(diag_spanned(
                                cm,
                                path,
                                c,
                                "`encodeURIComponent` / `decodeURIComponent` do not take type arguments",
                            ));
                        }
                        if c.args.len() != 1 {
                            return Err(diag_spanned(
                                cm,
                                path,
                                c,
                                "`encodeURIComponent` / `decodeURIComponent` expect exactly one argument (string)",
                            ));
                        }
                        let a0 = match &c.args[0] {
                            ExprOrSpread { spread: None, expr } => expr,
                            _ => {
                                return Err(diag_spanned(
                                    cm,
                                    path,
                                    c,
                                    "spread arguments are not supported",
                                ));
                            }
                        };
                        let arg = build_expr(a0, cm, path, iface, in_async)?;
                        let kind = if fname.sym == "encodeURIComponent" {
                            UriBuiltinKind::EncodeComponent
                        } else {
                            UriBuiltinKind::DecodeComponent
                        };
                        return Ok(IRExpr::UriBuiltin {
                            kind,
                            args: vec![arg],
                            span: c.span,
                        });
                    }
                }
                if let Expr::Member(m) = &**ce {
                    if let Expr::Ident(obj) = &*m.obj {
                        if obj.sym == "Promise" {
                            if let MemberProp::Ident(prop) = &m.prop {
                                if prop.sym == "all" {
                                    if !type_args.is_empty() {
                                        return Err(diag_spanned(
                                            cm,
                                            path,
                                            c,
                                            "`Promise.all` does not take type arguments",
                                        ));
                                    }
                                    if c.args.len() != 1 {
                                        return Err(diag_spanned(
                                            cm,
                                            path,
                                            c,
                                            "`Promise.all` expects exactly one argument",
                                        ));
                                    }
                                    let a0 = match &c.args[0] {
                                        ExprOrSpread { spread: None, expr } => expr,
                                        _ => {
                                            return Err(diag_spanned(
                                                cm,
                                                path,
                                                c,
                                                "spread arguments are not supported",
                                            ));
                                        }
                                    };
                                    let inner = build_expr(a0, cm, path, iface, in_async)?;
                                    let elems = match inner {
                                        IRExpr::ArrayLit { elems, .. } => elems,
                                        _ => {
                                            return Err(diag_spanned(
                                                cm,
                                                path,
                                                c,
                                                "`Promise.all` requires an array literal `[...]` argument",
                                            ));
                                        }
                                    };
                                    return Ok(IRExpr::PromiseAll {
                                        elems,
                                        span: c.span,
                                    });
                                }
                            }
                        }
                        if obj.sym == "console" {
                            if let MemberProp::Ident(prop) = &m.prop {
                                let stderr = match prop.sym.as_ref() {
                                    "log" => false,
                                    "error" | "debug" => true,
                                    _ => {
                                        return Err(diag_spanned(
                                            cm,
                                            path,
                                            c,
                                            "only `console.log`, `console.error`, and `console.debug` are supported",
                                        ));
                                    }
                                };
                                let mut args = Vec::new();
                                for a in &c.args {
                                    match a {
                                        ExprOrSpread { spread: None, expr } => {
                                            args.push(build_expr(expr, cm, path, iface, in_async)?);
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
                                return Ok(IRExpr::BuiltinLog {
                                    args,
                                    stderr,
                                    span: c.span,
                                });
                            }
                        }
                        if obj.sym == "Math" {
                            if let MemberProp::Ident(prop) = &m.prop {
                                let kind = match (prop.sym.as_ref(), c.args.len()) {
                                    ("abs", 1) => MathBuiltinKind::Abs,
                                    ("floor", 1) => MathBuiltinKind::Floor,
                                    ("ceil", 1) => MathBuiltinKind::Ceil,
                                    ("min", 2) => MathBuiltinKind::Min,
                                    ("max", 2) => MathBuiltinKind::Max,
                                    ("sign", 1) => MathBuiltinKind::Sign,
                                    ("trunc", 1) => MathBuiltinKind::Trunc,
                                    ("round", 1) => MathBuiltinKind::Round,
                                    ("pow", 2) => MathBuiltinKind::Pow,
                                    ("abs" | "floor" | "ceil" | "sign" | "trunc" | "round", _) => {
                                        return Err(diag_spanned(
                                            cm,
                                            path,
                                            c,
                                            "this `Math` method expects exactly 1 argument",
                                        ));
                                    }
                                    ("min" | "max" | "pow", _) => {
                                        return Err(diag_spanned(
                                            cm,
                                            path,
                                            c,
                                            "`Math.min`, `Math.max`, and `Math.pow` expect exactly 2 arguments",
                                        ));
                                    }
                                    _ => {
                                        return Err(diag_spanned(
                                            cm,
                                            path,
                                            c,
                                            "unsupported `Math` builtin (see README for supported methods)",
                                        ));
                                    }
                                };
                                let mut args = Vec::new();
                                for a in &c.args {
                                    match a {
                                        ExprOrSpread { spread: None, expr } => {
                                            args.push(build_expr(expr, cm, path, iface, in_async)?);
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
                                return Ok(IRExpr::MathBuiltin {
                                    kind,
                                    args,
                                    span: c.span,
                                });
                            }
                        }
                        if obj.sym == "Number" {
                            if let MemberProp::Ident(prop) = &m.prop {
                                let kind = match (prop.sym.as_ref(), c.args.len()) {
                                    ("parseInt", 1 | 2) => NumberBuiltinKind::ParseInt,
                                    ("parseFloat", 1) => NumberBuiltinKind::ParseFloat,
                                    ("parseInt", _) => {
                                        return Err(diag_spanned(
                                            cm,
                                            path,
                                            c,
                                            "`Number.parseInt` expects 1 or 2 arguments",
                                        ));
                                    }
                                    ("parseFloat", _) => {
                                        return Err(diag_spanned(
                                            cm,
                                            path,
                                            c,
                                            "`Number.parseFloat` expects exactly 1 argument",
                                        ));
                                    }
                                    _ => {
                                        return Err(diag_spanned(
                                            cm,
                                            path,
                                            c,
                                            "only `Number.parseInt` and `Number.parseFloat` are supported",
                                        ));
                                    }
                                };
                                let mut args = Vec::new();
                                for a in &c.args {
                                    match a {
                                        ExprOrSpread { spread: None, expr } => {
                                            args.push(build_expr(expr, cm, path, iface, in_async)?);
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
                                return Ok(IRExpr::NumberBuiltin {
                                    kind,
                                    args,
                                    span: c.span,
                                });
                            }
                        }
                        if obj.sym == "JSON" {
                            if let MemberProp::Ident(prop) = &m.prop {
                                let kind = match (prop.sym.as_ref(), c.args.len()) {
                                    ("stringify", 1) => JsonBuiltinKind::Stringify,
                                    ("parse", 1) => JsonBuiltinKind::Parse,
                                    ("stringify", _) => {
                                        return Err(diag_spanned(
                                            cm,
                                            path,
                                            c,
                                            "`JSON.stringify` expects exactly 1 argument",
                                        ));
                                    }
                                    ("parse", _) => {
                                        return Err(diag_spanned(
                                            cm,
                                            path,
                                            c,
                                            "`JSON.parse` expects exactly 1 argument",
                                        ));
                                    }
                                    _ => {
                                        return Err(diag_spanned(
                                            cm,
                                            path,
                                            c,
                                            "only `JSON.stringify` and `JSON.parse` are supported",
                                        ));
                                    }
                                };
                                let mut args = Vec::new();
                                for a in &c.args {
                                    match a {
                                        ExprOrSpread { spread: None, expr } => {
                                            args.push(build_expr(expr, cm, path, iface, in_async)?);
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
                                if kind == JsonBuiltinKind::Parse {
                                    match fold_json_parse_arg(args[0].clone()) {
                                        Ok(JsonParseFold::Folded(e)) => return Ok(e),
                                        Ok(JsonParseFold::NotStringLiteral) => {}
                                        Err(msg) => {
                                            return Err(diag_spanned(cm, path, c, &msg));
                                        }
                                    }
                                }
                                return Ok(IRExpr::JsonBuiltin {
                                    kind,
                                    args,
                                    span: c.span,
                                    stringify_inferred_ty: None,
                                });
                            }
                        }
                    }
                    if let MemberProp::Ident(prop) = &m.prop {
                        if let Some(kind) = string_method_kind(prop.sym.as_ref()) {
                            if !type_args.is_empty() {
                                return Err(diag_spanned(
                                    cm,
                                    path,
                                    c,
                                    "string prototype builtins do not take type arguments",
                                ));
                            }
                            if !string_method_arity_matches(kind, c.args.len()) {
                                return Err(diag_spanned(
                                    cm,
                                    path,
                                    c,
                                    "wrong argument count for this string method",
                                ));
                            }
                            let mut args = Vec::new();
                            for a in &c.args {
                                match a {
                                    ExprOrSpread { spread: None, expr } => {
                                        args.push(build_expr(expr, cm, path, iface, in_async)?);
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
                            let receiver = Box::new(build_expr(&m.obj, cm, path, iface, in_async)?);
                            return Ok(IRExpr::StringMethodBuiltin {
                                kind,
                                receiver,
                                args,
                                span: c.span,
                            });
                        }
                        // `expr.body.getReader()`
                        if prop.sym == "getReader" {
                            if !type_args.is_empty() {
                                return Err(diag_spanned(
                                    cm,
                                    path,
                                    c,
                                    "`body.getReader` does not take type arguments",
                                ));
                            }
                            if !c.args.is_empty() {
                                return Err(diag_spanned(
                                    cm,
                                    path,
                                    c,
                                    "`body.getReader()` expects no arguments",
                                ));
                            }
                            if let Expr::Member(inner) = &*m.obj {
                                if let MemberProp::Ident(ip) = &inner.prop {
                                    if ip.sym == "body" {
                                        let response =
                                            build_expr(&inner.obj, cm, path, iface, in_async)?;
                                        return Ok(IRExpr::HttpResponseBodyGetReader {
                                            response: Box::new(response),
                                            span: c.span,
                                            stream_slot: None,
                                        });
                                    }
                                }
                            }
                        }
                        // `reader.read()` — streaming body
                        if prop.sym == "read" {
                            if !type_args.is_empty() {
                                return Err(diag_spanned(
                                    cm,
                                    path,
                                    c,
                                    "`reader.read` does not take type arguments",
                                ));
                            }
                            if !c.args.is_empty() {
                                return Err(diag_spanned(
                                    cm,
                                    path,
                                    c,
                                    "`reader.read()` expects no arguments",
                                ));
                            }
                            if let Expr::Ident(rid) = &*m.obj {
                                return Ok(IRExpr::ReaderRead {
                                    reader_name: rid.sym.to_string(),
                                    span: c.span,
                                    reader_slot: None,
                                });
                            }
                        }
                        if !type_args.is_empty() {
                            return Err(diag_spanned(
                                cm,
                                path,
                                c,
                                "`response.text` / `response.json` do not take type arguments",
                            ));
                        }
                        match prop.sym.as_ref() {
                            "text" if c.args.is_empty() => {
                                let receiver =
                                    Box::new(build_expr(&m.obj, cm, path, iface, in_async)?);
                                return Ok(IRExpr::HttpResponseMethodBuiltin {
                                    kind: HttpResponseMethodKind::Text,
                                    receiver,
                                    span: c.span,
                                });
                            }
                            "json" if c.args.is_empty() => {
                                let receiver =
                                    Box::new(build_expr(&m.obj, cm, path, iface, in_async)?);
                                return Ok(IRExpr::HttpResponseMethodBuiltin {
                                    kind: HttpResponseMethodKind::Json,
                                    receiver,
                                    span: c.span,
                                });
                            }
                            "text" | "json" => {
                                return Err(diag_spanned(
                                    cm,
                                    path,
                                    c,
                                    "`response.text()` / `response.json()` expect no arguments",
                                ));
                            }
                            _ => {}
                        }
                        let mut args = Vec::new();
                        for a in &c.args {
                            match a {
                                ExprOrSpread { spread: None, expr } => {
                                    args.push(build_expr(expr, cm, path, iface, in_async)?);
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
                        let receiver = Box::new(build_expr(&m.obj, cm, path, iface, in_async)?);
                        return Ok(IRExpr::MethodCall {
                            receiver,
                            method: prop.sym.to_string(),
                            args,
                            type_args,
                            span: c.span,
                        });
                    }
                    return Err(diag_spanned(
                        cm,
                        path,
                        c,
                        "computed method calls `obj[expr](...)` are not supported in this compiler version",
                    ));
                }
                if let Expr::Ident(i) = &**ce {
                    if i.sym == "readLine" {
                        if !type_args.is_empty() {
                            return Err(diag_spanned(
                                cm,
                                path,
                                c,
                                "`readLine` does not take type arguments",
                            ));
                        }
                        if !c.args.is_empty() {
                            return Err(diag_spanned(
                                cm,
                                path,
                                c,
                                "`readLine` expects no arguments",
                            ));
                        }
                        return Ok(IRExpr::ReadStdinLine { span: c.span });
                    }
                    let mut args = Vec::new();
                    for a in &c.args {
                        match a {
                            ExprOrSpread { spread: None, expr } => {
                                args.push(build_expr(expr, cm, path, iface, in_async)?);
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
                        type_args,
                        span: c.span,
                    });
                }
            }
            Err(diag_spanned(
                cm,
                path,
                c,
                "unsupported call expression (see README for supported builtins and call forms)",
            ))
        }
        _ => Err(diag_spanned(cm, path, expr, "unsupported expression")),
    }
}

fn string_method_kind(name: &str) -> Option<StringMethodKind> {
    match name {
        "charAt" => Some(StringMethodKind::CharAt),
        "charCodeAt" => Some(StringMethodKind::CharCodeAt),
        "slice" => Some(StringMethodKind::Slice),
        "substring" => Some(StringMethodKind::Substring),
        "indexOf" => Some(StringMethodKind::IndexOf),
        "includes" => Some(StringMethodKind::Includes),
        _ => None,
    }
}

fn string_method_arity_matches(kind: StringMethodKind, argc: usize) -> bool {
    match kind {
        StringMethodKind::CharAt | StringMethodKind::CharCodeAt => argc == 1,
        StringMethodKind::Slice
        | StringMethodKind::Substring
        | StringMethodKind::IndexOf
        | StringMethodKind::Includes => argc == 1 || argc == 2,
    }
}

fn build_tpl(
    t: &Tpl,
    cm: &Lrc<SourceMap>,
    path: &str,
    iface: &HashMap<String, TsType>,
    in_async: bool,
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
            parts.push(TplPart::Interp(Box::new(build_expr(
                &t.exprs[i],
                cm,
                path,
                iface,
                in_async,
            )?)));
        }
    }
    Ok(IRExpr::Tpl {
        parts,
        span: t.span,
    })
}

fn allowed_http_method(m: &str) -> bool {
    matches!(
        m,
        "GET" | "POST" | "PUT" | "DELETE" | "PATCH" | "HEAD" | "OPTIONS"
    )
}

fn build_fetch_init(
    expr: &Expr,
    cm: &Lrc<SourceMap>,
    path: &str,
    iface: &HashMap<String, TsType>,
    in_async: bool,
) -> Result<FetchInit, CompileError> {
    let o: &ObjectLit = match expr {
        Expr::Object(ol) => ol,
        _ => {
            return Err(diag_spanned(
                cm,
                path,
                expr,
                "`fetch` second argument must be an object literal `{ ... }`",
            ));
        }
    };
    let mut method: Option<String> = None;
    let mut headers: Vec<(String, String)> = Vec::new();
    let mut body: Option<Box<IRExpr>> = None;
    let mut saw_headers = false;
    for p in &o.props {
        match p {
            PropOrSpread::Spread(_) => {
                return Err(diag_spanned(
                    cm,
                    path,
                    o,
                    "object spread is not supported in fetch init",
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
                                "fetch init: key must be identifier or string literal",
                            ));
                        }
                    };
                    match k.as_str() {
                        "method" => {
                            if method.is_some() {
                                return Err(diag(
                                    cm,
                                    path,
                                    o.span,
                                    "duplicate `method` in fetch init",
                                ));
                            }
                            let s = match &**value {
                                Expr::Lit(Lit::Str(s)) => s.value.to_string_lossy().into_owned(),
                                _ => {
                                    return Err(diag_spanned(
                                        cm,
                                        path,
                                        value,
                                        "`method` must be a string literal (e.g. `\"POST\"`)",
                                    ));
                                }
                            };
                            method = Some(s.to_uppercase());
                        }
                        "headers" => {
                            if saw_headers {
                                return Err(diag(
                                    cm,
                                    path,
                                    o.span,
                                    "duplicate `headers` in fetch init",
                                ));
                            }
                            saw_headers = true;
                            let ho = match &**value {
                                Expr::Object(hobj) => hobj,
                                _ => {
                                    return Err(diag_spanned(
                                        cm,
                                        path,
                                        value,
                                        "`headers` must be an object literal `{ \"Key\": \"value\" }`",
                                    ));
                                }
                            };
                            for hp in &ho.props {
                                match hp {
                                    PropOrSpread::Spread(_) => {
                                        return Err(diag_spanned(
                                            cm,
                                            path,
                                            ho,
                                            "object spread is not supported in fetch headers",
                                        ));
                                    }
                                    PropOrSpread::Prop(hpp) => match &**hpp {
                                        Prop::KeyValue(KeyValueProp { key: hk, value: hv }) => {
                                            let hn = match hk {
                                                PropName::Ident(i) => i.sym.to_string(),
                                                PropName::Str(s) => {
                                                    s.value.to_string_lossy().into_owned()
                                                }
                                                _ => {
                                                    return Err(diag_spanned(
                                                        cm,
                                                        path,
                                                        ho,
                                                        "header name must be identifier or string literal",
                                                    ));
                                                }
                                            };
                                            let hv_str = match &**hv {
                                                Expr::Lit(Lit::Str(s)) => {
                                                    s.value.to_string_lossy().into_owned()
                                                }
                                                _ => {
                                                    return Err(diag_spanned(
                                                        cm,
                                                        path,
                                                        hv,
                                                        "header values must be string literals in this subset",
                                                    ));
                                                }
                                            };
                                            headers.push((hn, hv_str));
                                        }
                                        _ => {
                                            return Err(diag_spanned(
                                                cm,
                                                path,
                                                ho,
                                                "only `key: value` header entries are supported",
                                            ));
                                        }
                                    },
                                }
                            }
                            headers.sort_by(|a, b| a.0.cmp(&b.0));
                            for w in headers.windows(2) {
                                if w[0].0 == w[1].0 {
                                    return Err(diag(
                                        cm,
                                        path,
                                        ho.span,
                                        format!("duplicate header `{}`", w[0].0),
                                    ));
                                }
                            }
                        }
                        "body" => {
                            if body.is_some() {
                                return Err(diag(
                                    cm,
                                    path,
                                    o.span,
                                    "duplicate `body` in fetch init",
                                ));
                            }
                            body = Some(Box::new(build_expr(value, cm, path, iface, in_async)?));
                        }
                        _ => {
                            return Err(diag_spanned(
                                cm,
                                path,
                                o,
                                format!(
                                    "unknown fetch init field `{k}` (supported: method, headers, body)",
                                ),
                            ));
                        }
                    }
                }
                _ => {
                    return Err(diag_spanned(
                        cm,
                        path,
                        o,
                        "only `key: value` properties are supported in fetch init",
                    ));
                }
            },
        }
    }
    let mut method_res = method;
    if method_res.is_none() && body.is_some() {
        method_res = Some("POST".to_string());
    }
    if let Some(ref m) = method_res {
        if !allowed_http_method(m) {
            return Err(diag(
                cm,
                path,
                o.span,
                format!(
                    "unsupported HTTP `method` `{}` (supported: GET, POST, PUT, DELETE, PATCH, HEAD, OPTIONS)",
                    m
                ),
            ));
        }
    }
    Ok(FetchInit {
        method: method_res,
        headers,
        body,
    })
}

fn peel_expr_parens<'a>(mut e: &'a Expr) -> &'a Expr {
    while let Expr::Paren(p) = e {
        e = &p.expr;
    }
    e
}

fn build_opt_chain_call_expr(
    c: &OptCall,
    cm: &Lrc<SourceMap>,
    path: &str,
    iface: &HashMap<String, TsType>,
    in_async: bool,
) -> Result<IRExpr, CompileError> {
    let call: CallExpr = c.clone().into();
    let type_args = call_type_args(&call, cm, path, iface)?;
    let mut args = Vec::new();
    for a in &call.args {
        match a {
            ExprOrSpread { spread: None, expr } => {
                args.push(build_expr(expr, cm, path, iface, in_async)?);
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
    let Callee::Expr(ce) = &call.callee else {
        return Err(diag_spanned(
            cm,
            path,
            c,
            "optional call (`?.()`) does not support this callee form",
        ));
    };
    let inner = peel_expr_parens(ce);
    if let Expr::Member(m) = inner {
        if let MemberProp::Ident(prop) = &m.prop {
            if prop.sym == "then" {
                return Err(diag_spanned(
                    cm,
                    path,
                    c,
                    "`Promise.prototype.then` is not supported; use `async`/`await` instead",
                ));
            }
        }
    }
    match inner {
        Expr::Ident(i) => Ok(IRExpr::OptionalCall {
            callee: i.sym.to_string(),
            args,
            type_args,
            span: call.span,
        }),
        Expr::Member(m) => {
            if let MemberProp::Ident(prop) = &m.prop {
                let receiver = Box::new(build_expr(&m.obj, cm, path, iface, in_async)?);
                Ok(IRExpr::OptionalMethodCall {
                    receiver,
                    method: prop.sym.to_string(),
                    args,
                    type_args,
                    span: call.span,
                })
            } else {
                Err(diag_spanned(
                    cm,
                    path,
                    c,
                    "computed method optional calls `obj?.[expr](...)` are not supported",
                ))
            }
        }
        Expr::OptChain(och) => {
            // e.g. `make()?.get_v()`：`OptCall` 的 callee 为 `obj?.prop` 形式的 `Expr::OptChain`。
            if !och.optional {
                return Err(diag_spanned(
                    cm,
                    path,
                    c,
                    "optional call (`?.()`) supports only a plain identifier or `obj.prop` callee",
                ));
            }
            if let OptChainBase::Member(m) = &*och.base {
                if let MemberProp::Ident(prop) = &m.prop {
                    if prop.sym == "then" {
                        return Err(diag_spanned(
                            cm,
                            path,
                            c,
                            "`Promise.prototype.then` is not supported; use `async`/`await` instead",
                        ));
                    }
                    let receiver = Box::new(build_expr(&m.obj, cm, path, iface, in_async)?);
                    Ok(IRExpr::OptionalMethodCall {
                        receiver,
                        method: prop.sym.to_string(),
                        args,
                        type_args,
                        span: call.span,
                    })
                } else {
                    Err(diag_spanned(
                        cm,
                        path,
                        c,
                        "computed method optional calls `obj?.[expr](...)` are not supported",
                    ))
                }
            } else {
                Err(diag_spanned(
                    cm,
                    path,
                    c,
                    "optional call (`?.()`) supports only a plain identifier or `obj.prop` callee",
                ))
            }
        }
        _ => Err(diag_spanned(
            cm,
            path,
            c,
            "optional call (`?.()`) supports only a plain identifier or `obj.prop` callee",
        )),
    }
}

fn build_member_expr(
    m: &MemberExpr,
    cm: &Lrc<SourceMap>,
    path: &str,
    iface: &HashMap<String, TsType>,
    in_async: bool,
) -> Result<IRExpr, CompileError> {
    let obj = Box::new(build_expr(&m.obj, cm, path, iface, in_async)?);
    match &m.prop {
        MemberProp::Ident(id) => Ok(IRExpr::Member {
            obj,
            prop: id.sym.to_string(),
            span: m.span,
            length_dispatch: None,
            http_response_member: None,
            stream_read_member: None,
        }),
        MemberProp::Computed(c) => Ok(IRExpr::Index {
            obj,
            index: Box::new(build_expr(&c.expr, cm, path, iface, in_async)?),
            span: m.span,
            index_kind: None,
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
    in_async: bool,
) -> Result<IRExpr, CompileError> {
    match &*o.base {
        OptChainBase::Call(c) => build_opt_chain_call_expr(c, cm, path, iface, in_async),
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
                    obj: Box::new(build_expr(&m.obj, cm, path, iface, in_async)?),
                    prop,
                    span: o.span,
                    length_dispatch: None,
                    http_response_member: None,
                    stream_read_member: None,
                })
            } else {
                Ok(IRExpr::Member {
                    obj: Box::new(build_expr(&m.obj, cm, path, iface, in_async)?),
                    prop,
                    span: o.span,
                    length_dispatch: None,
                    http_response_member: None,
                    stream_read_member: None,
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
    in_async: bool,
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
        elems.push(build_expr(&e.expr, cm, path, iface, in_async)?);
    }
    Ok(IRExpr::ArrayLit {
        elems,
        span: a.span,
    })
}

fn build_object_expr(
    o: &swc_ecma_ast::ObjectLit,
    cm: &Lrc<SourceMap>,
    path: &str,
    iface: &HashMap<String, TsType>,
    in_async: bool,
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
                    fields.push((k, build_expr(value, cm, path, iface, in_async)?));
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

#[cfg(test)]
mod tests {
    use super::*;
    use ts2rs_parser::parse_typescript_file;

    #[test]
    fn build_module_records_main() {
        let src = r#"function main(): number { return 0; }"#;
        let p = parse_typescript_file("t.ts", src).unwrap();
        let m = build_module(&p.program, &p.source_map, "t.ts", Some(&p.comments)).unwrap();
        assert!(m.fns.iter().any(|f| f.name == "main"));
    }
}
