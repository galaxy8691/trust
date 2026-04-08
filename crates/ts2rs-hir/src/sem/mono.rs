use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};

use crate::error::{diag, CompileError};
use crate::ir::*;

#[derive(Clone, Debug)]
struct Req {
    callee: String,
    args_key: String,
    args: Vec<TsType>,
}

pub(super) fn monomorphize_module_functions(module: &mut IRModule) -> Result<(), CompileError> {
    let mut templates = HashMap::<String, IRFunction>::new();
    for f in &module.fns {
        if !f.type_params.is_empty() {
            templates.insert(f.name.clone(), f.clone());
        }
    }
    if templates.is_empty() {
        return Ok(());
    }

    let mut out = Vec::<IRFunction>::new();
    let mut queue = VecDeque::<Req>::new();
    let mut seen = HashSet::<String>::new();

    for mut f in module.fns.clone() {
        if !f.type_params.is_empty() {
            continue;
        }
        let cm = f.cm.clone();
        let source_path = f.source_path.clone();
        let span = f.span;
        rewrite_calls_and_collect(&mut f.body, &templates, &mut queue, &cm, &source_path, span)?;
        out.push(f);
    }

    while let Some(req) = queue.pop_front() {
        let req_key = format!("{}::{}", req.callee, req.args_key);
        if !seen.insert(req_key) {
            continue;
        }
        let tpl = templates.get(&req.callee).ok_or_else(|| {
            let first = module
                .fns
                .first()
                .expect("module should have at least one function");
            diag(
                first.cm.as_ref(),
                &first.source_path,
                first.span,
                format!("call to unknown function `{}`", req.callee),
            )
        })?;
        if tpl.type_params.len() != req.args.len() {
            return Err(diag(
                tpl.cm.as_ref(),
                &tpl.source_path,
                tpl.span,
                format!(
                    "wrong type argument count for `{}`: expected {}, got {}",
                    tpl.name,
                    tpl.type_params.len(),
                    req.args.len()
                ),
            ));
        }
        let mut subst = BTreeMap::new();
        for (k, v) in tpl.type_params.iter().zip(req.args.iter()) {
            subst.insert(k.clone(), v.clone());
        }
        let mut inst = instantiate_function(tpl, &subst);
        inst.name = mangle_name(&tpl.name, &req.args);
        inst.type_params.clear();
        inst.mono_origin = Some(format!("{}<{}>", tpl.name, render_types(&req.args)));
        let cm = inst.cm.clone();
        let source_path = inst.source_path.clone();
        let span = inst.span;
        rewrite_calls_and_collect(
            &mut inst.body,
            &templates,
            &mut queue,
            &cm,
            &source_path,
            span,
        )?;
        out.push(inst);
    }

    module.fns = out;
    Ok(())
}

fn rewrite_calls_and_collect(
    body: &mut [IRStmt],
    templates: &HashMap<String, IRFunction>,
    queue: &mut VecDeque<Req>,
    cm: &swc_common::sync::Lrc<swc_common::SourceMap>,
    path: &str,
    fn_span: swc_common::Span,
) -> Result<(), CompileError> {
    for s in body {
        rewrite_stmt(s, templates, queue, cm, path, fn_span)?;
    }
    Ok(())
}

fn rewrite_stmt(
    s: &mut IRStmt,
    templates: &HashMap<String, IRFunction>,
    queue: &mut VecDeque<Req>,
    cm: &swc_common::sync::Lrc<swc_common::SourceMap>,
    path: &str,
    fn_span: swc_common::Span,
) -> Result<(), CompileError> {
    match s {
        IRStmt::Let { init, ty, .. } => {
            *ty = subst_type(ty, &BTreeMap::new());
            if let Some(e) = init {
                rewrite_expr(e, templates, queue, cm, path, fn_span)?;
            }
        }
        IRStmt::Assign { rhs, .. } => rewrite_expr(rhs, templates, queue, cm, path, fn_span)?,
        IRStmt::MemberAssign { rhs, .. } => rewrite_expr(rhs, templates, queue, cm, path, fn_span)?,
        IRStmt::Expr { expr, .. } => rewrite_expr(expr, templates, queue, cm, path, fn_span)?,
        IRStmt::Return { arg, .. } => {
            if let Some(e) = arg {
                rewrite_expr(e, templates, queue, cm, path, fn_span)?;
            }
        }
        IRStmt::Block { stmts, .. } => {
            rewrite_calls_and_collect(stmts, templates, queue, cm, path, fn_span)?
        }
        IRStmt::If {
            cond,
            then_b,
            else_b,
            ..
        } => {
            rewrite_expr(cond, templates, queue, cm, path, fn_span)?;
            rewrite_calls_and_collect(then_b, templates, queue, cm, path, fn_span)?;
            if let Some(e) = else_b {
                rewrite_calls_and_collect(e, templates, queue, cm, path, fn_span)?;
            }
        }
        IRStmt::While { cond, body, .. } => {
            rewrite_expr(cond, templates, queue, cm, path, fn_span)?;
            rewrite_calls_and_collect(body, templates, queue, cm, path, fn_span)?;
        }
        IRStmt::ForIn { target, body, .. } => {
            rewrite_expr(target, templates, queue, cm, path, fn_span)?;
            rewrite_calls_and_collect(body, templates, queue, cm, path, fn_span)?;
        }
        IRStmt::DoWhile { body, cond, .. } => {
            rewrite_calls_and_collect(body, templates, queue, cm, path, fn_span)?;
            rewrite_expr(cond, templates, queue, cm, path, fn_span)?;
        }
        IRStmt::FnDecl { func, .. } => {
            rewrite_calls_and_collect(&mut func.body, templates, queue, cm, path, fn_span)?;
        }
        IRStmt::Empty { .. } | IRStmt::Break { .. } | IRStmt::Continue { .. } => {}
    }
    Ok(())
}

fn rewrite_expr(
    e: &mut IRExpr,
    templates: &HashMap<String, IRFunction>,
    queue: &mut VecDeque<Req>,
    cm: &swc_common::sync::Lrc<swc_common::SourceMap>,
    path: &str,
    fn_span: swc_common::Span,
) -> Result<(), CompileError> {
    match e {
        IRExpr::Call {
            callee,
            args,
            type_args,
            span,
        } => {
            for a in args {
                rewrite_expr(a, templates, queue, cm, path, fn_span)?;
            }
            if templates.contains_key(callee) {
                if type_args.is_empty() {
                    return Err(diag(
                        cm.as_ref(),
                        path,
                        *span,
                        format!(
                            "generic function `{}` requires explicit type arguments",
                            callee
                        ),
                    ));
                }
                let req = Req {
                    callee: callee.clone(),
                    args_key: render_types(type_args),
                    args: type_args.clone(),
                };
                queue.push_back(req.clone());
                *callee = mangle_name(&req.callee, &req.args);
                type_args.clear();
            } else if !type_args.is_empty() {
                return Err(diag(
                    cm.as_ref(),
                    path,
                    *span,
                    format!(
                        "type arguments are only allowed on generic functions, got `{}`",
                        callee
                    ),
                ));
            }
        }
        IRExpr::MethodCall {
            receiver,
            args,
            type_args,
            ..
        } => {
            rewrite_expr(receiver, templates, queue, cm, path, fn_span)?;
            for a in args {
                rewrite_expr(a, templates, queue, cm, path, fn_span)?;
            }
            if !type_args.is_empty() {
                // `obj.m<T>` desugars to global call and does not support explicit type args in this subset.
                type_args.clear();
            }
        }
        IRExpr::Binary { left, right, .. } => {
            rewrite_expr(left, templates, queue, cm, path, fn_span)?;
            rewrite_expr(right, templates, queue, cm, path, fn_span)?;
        }
        IRExpr::Unary { arg, .. } => rewrite_expr(arg, templates, queue, cm, path, fn_span)?,
        IRExpr::Conditional {
            test, cons, alt, ..
        } => {
            rewrite_expr(test, templates, queue, cm, path, fn_span)?;
            rewrite_expr(cons, templates, queue, cm, path, fn_span)?;
            rewrite_expr(alt, templates, queue, cm, path, fn_span)?;
        }
        IRExpr::Seq { exprs, .. } => {
            for x in exprs {
                rewrite_expr(x, templates, queue, cm, path, fn_span)?;
            }
        }
        IRExpr::Tpl { parts, .. } => {
            for p in parts {
                if let TplPart::Interp(e) = p {
                    rewrite_expr(e, templates, queue, cm, path, fn_span)?;
                }
            }
        }
        IRExpr::Member { obj, .. } => rewrite_expr(obj, templates, queue, cm, path, fn_span)?,
        IRExpr::OptionalMember { obj, .. } => {
            rewrite_expr(obj, templates, queue, cm, path, fn_span)?
        }
        IRExpr::NullishCoalesce { left, right, .. } => {
            rewrite_expr(left, templates, queue, cm, path, fn_span)?;
            rewrite_expr(right, templates, queue, cm, path, fn_span)?;
        }
        IRExpr::MathBuiltin { args, .. } | IRExpr::BuiltinLog { args, .. } => {
            for a in args {
                rewrite_expr(a, templates, queue, cm, path, fn_span)?;
            }
        }
        IRExpr::ArrayLit { elems, .. } => {
            for a in elems {
                rewrite_expr(a, templates, queue, cm, path, fn_span)?;
            }
        }
        IRExpr::ObjectLit { fields, .. } => {
            for (_, v) in fields {
                rewrite_expr(v, templates, queue, cm, path, fn_span)?;
            }
        }
        IRExpr::Index { obj, index, .. } => {
            rewrite_expr(obj, templates, queue, cm, path, fn_span)?;
            rewrite_expr(index, templates, queue, cm, path, fn_span)?;
        }
        IRExpr::ArrowFn { body, .. } => {
            rewrite_calls_and_collect(body, templates, queue, cm, path, fn_span)?;
        }
        IRExpr::Await { arg, .. } => rewrite_expr(arg, templates, queue, cm, path, fn_span)?,
        IRExpr::FetchText { url, .. } => rewrite_expr(url, templates, queue, cm, path, fn_span)?,
        IRExpr::Number(..)
        | IRExpr::Bool(..)
        | IRExpr::Str(..)
        | IRExpr::Ident(..)
        | IRExpr::Null(..)
        | IRExpr::Undefined(..)
        | IRExpr::This(..)
        | IRExpr::Super(..) => {}
    }
    Ok(())
}

fn instantiate_function(tpl: &IRFunction, subst: &BTreeMap<String, TsType>) -> IRFunction {
    let mut f = tpl.clone();
    f.params = f
        .params
        .into_iter()
        .map(|(n, t)| (n, subst_type(&t, subst)))
        .collect();
    f.ret = subst_type(&f.ret, subst);
    subst_stmts(&mut f.body, subst);
    f
}

fn subst_stmts(stmts: &mut [IRStmt], subst: &BTreeMap<String, TsType>) {
    for s in stmts {
        match s {
            IRStmt::Let { ty, init, .. } => {
                *ty = subst_type(ty, subst);
                if let Some(e) = init {
                    subst_expr(e, subst);
                }
            }
            IRStmt::Assign { rhs, .. } => subst_expr(rhs, subst),
            IRStmt::MemberAssign { rhs, .. } => subst_expr(rhs, subst),
            IRStmt::Expr { expr, .. } => subst_expr(expr, subst),
            IRStmt::Return { arg, .. } => {
                if let Some(e) = arg {
                    subst_expr(e, subst);
                }
            }
            IRStmt::Block { stmts, .. } => subst_stmts(stmts, subst),
            IRStmt::If {
                cond,
                cond_ty,
                then_b,
                else_b,
                ..
            } => {
                subst_expr(cond, subst);
                *cond_ty = subst_type(cond_ty, subst);
                subst_stmts(then_b, subst);
                if let Some(e) = else_b {
                    subst_stmts(e, subst);
                }
            }
            IRStmt::While {
                cond,
                cond_ty,
                body,
                ..
            } => {
                subst_expr(cond, subst);
                *cond_ty = subst_type(cond_ty, subst);
                subst_stmts(body, subst);
            }
            IRStmt::ForIn {
                key_ty,
                target,
                body,
                ..
            } => {
                *key_ty = subst_type(key_ty, subst);
                subst_expr(target, subst);
                subst_stmts(body, subst);
            }
            IRStmt::DoWhile {
                body,
                cond,
                cond_ty,
                ..
            } => {
                subst_stmts(body, subst);
                subst_expr(cond, subst);
                *cond_ty = subst_type(cond_ty, subst);
            }
            IRStmt::FnDecl { func, .. } => {
                func.params = func
                    .params
                    .clone()
                    .into_iter()
                    .map(|(n, t)| (n, subst_type(&t, subst)))
                    .collect();
                func.ret = subst_type(&func.ret, subst);
                subst_stmts(&mut func.body, subst);
            }
            IRStmt::Empty { .. } | IRStmt::Break { .. } | IRStmt::Continue { .. } => {}
        }
    }
}

fn subst_expr(e: &mut IRExpr, subst: &BTreeMap<String, TsType>) {
    match e {
        IRExpr::Call {
            args, type_args, ..
        } => {
            for a in args {
                subst_expr(a, subst);
            }
            for t in type_args {
                *t = subst_type(t, subst);
            }
        }
        IRExpr::MethodCall {
            receiver,
            args,
            type_args,
            ..
        } => {
            subst_expr(receiver, subst);
            for a in args {
                subst_expr(a, subst);
            }
            for t in type_args {
                *t = subst_type(t, subst);
            }
        }
        IRExpr::Binary { left, right, .. } => {
            subst_expr(left, subst);
            subst_expr(right, subst);
        }
        IRExpr::Unary { arg, .. } => subst_expr(arg, subst),
        IRExpr::Conditional {
            test,
            cons,
            alt,
            cond_ty,
            ..
        } => {
            subst_expr(test, subst);
            subst_expr(cons, subst);
            subst_expr(alt, subst);
            if let Some(t) = cond_ty {
                *t = subst_type(t, subst);
            }
        }
        IRExpr::Seq { exprs, .. } => {
            for x in exprs {
                subst_expr(x, subst);
            }
        }
        IRExpr::Tpl { parts, .. } => {
            for p in parts {
                if let TplPart::Interp(e) = p {
                    subst_expr(e, subst);
                }
            }
        }
        IRExpr::Member { obj, .. } => subst_expr(obj, subst),
        IRExpr::OptionalMember { obj, .. } => subst_expr(obj, subst),
        IRExpr::NullishCoalesce { left, right, .. } => {
            subst_expr(left, subst);
            subst_expr(right, subst);
        }
        IRExpr::MathBuiltin { args, .. } | IRExpr::BuiltinLog { args, .. } => {
            for a in args {
                subst_expr(a, subst);
            }
        }
        IRExpr::ArrayLit { elems, .. } => {
            for a in elems {
                subst_expr(a, subst);
            }
        }
        IRExpr::ObjectLit { fields, .. } => {
            for (_, v) in fields {
                subst_expr(v, subst);
            }
        }
        IRExpr::Index { obj, index, .. } => {
            subst_expr(obj, subst);
            subst_expr(index, subst);
        }
        IRExpr::ArrowFn {
            params, ret, body, ..
        } => {
            for (_, t) in params {
                *t = subst_type(t, subst);
            }
            *ret = subst_type(ret, subst);
            subst_stmts(body, subst);
        }
        IRExpr::Await { arg, .. } => subst_expr(arg, subst),
        IRExpr::FetchText { url, .. } => subst_expr(url, subst),
        IRExpr::Number(..)
        | IRExpr::Bool(..)
        | IRExpr::Str(..)
        | IRExpr::Ident(..)
        | IRExpr::Null(..)
        | IRExpr::Undefined(..)
        | IRExpr::This(..)
        | IRExpr::Super(..) => {}
    }
}

fn subst_type(t: &TsType, subst: &BTreeMap<String, TsType>) -> TsType {
    match t {
        TsType::TypeParam(n) => subst.get(n).cloned().unwrap_or_else(|| t.clone()),
        TsType::Union(v) => normalize_union(v.iter().map(|x| subst_type(x, subst)).collect()),
        TsType::Fn { params, ret } => TsType::Fn {
            params: params.iter().map(|x| subst_type(x, subst)).collect(),
            ret: Box::new(subst_type(ret, subst)),
        },
        TsType::Promise(inner) => TsType::Promise(Box::new(subst_type(inner, subst))),
        TsType::ClassInstance(_) => t.clone(),
        _ => t.clone(),
    }
}

fn mangle_name(name: &str, args: &[TsType]) -> String {
    format!("{name}__{}", render_types(args))
}

fn render_types(args: &[TsType]) -> String {
    args.iter().map(type_key).collect::<Vec<_>>().join("_")
}

fn type_key(t: &TsType) -> String {
    match t {
        TsType::Number => "n".to_string(),
        TsType::Boolean => "b".to_string(),
        TsType::String => "s".to_string(),
        TsType::NumberLit(x) => format!("nl{x}"),
        TsType::BoolLit(x) => format!("bl{x}"),
        TsType::StringLit(x) => {
            format!("sl{}", x.replace(|c: char| !c.is_ascii_alphanumeric(), "_"))
        }
        TsType::Void => "v".to_string(),
        TsType::Null => "null".to_string(),
        TsType::Undefined => "undef".to_string(),
        TsType::ArrayNumber => "arrn".to_string(),
        TsType::ObjectNum(fields) => format!("obj{}", fields.join("_")),
        TsType::Union(m) => format!("u{}", m.iter().map(type_key).collect::<Vec<_>>().join("_")),
        TsType::TypeParam(n) => format!("tp{n}"),
        TsType::ClassInstance(n) => format!("cls{n}"),
        TsType::Fn { params, ret } => format!(
            "fn{}_r{}",
            params.iter().map(type_key).collect::<Vec<_>>().join("_"),
            type_key(ret)
        ),
        TsType::Promise(inner) => format!("P{}", type_key(inner)),
    }
}
