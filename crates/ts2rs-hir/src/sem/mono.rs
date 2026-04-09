use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};

use crate::error::{diag, CompileError};
use crate::ir::*;

#[derive(Clone, Debug)]
struct Req {
    callee: String,
    args_fingerprint: u64,
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
    let mut seen = HashSet::<(String, u64)>::new();
    let mut errs = Vec::<CompileError>::new();

    for mut f in module.fns.clone() {
        if !f.type_params.is_empty() {
            continue;
        }
        let cm = f.cm.clone();
        let source_path = f.source_path.clone();
        let span = f.span;
        let mut env: HashMap<String, TsType> = f.params.iter().cloned().collect();
        rewrite_stmts_in_scope(
            &mut f.body,
            &templates,
            &mut queue,
            &cm,
            &source_path,
            span,
            &mut env,
            &mut errs,
        );
        out.push(f);
    }

    while let Some(req) = queue.pop_front() {
        let req_key = (req.callee.clone(), req.args_fingerprint);
        if !seen.insert(req_key) {
            continue;
        }
        let Some(tpl) = templates.get(&req.callee) else {
            let first = module
                .fns
                .first()
                .expect("module should have at least one function");
            errs.push(diag(
                first.cm.as_ref(),
                &first.source_path,
                first.span,
                format!("call to unknown function `{}`", req.callee),
            ));
            continue;
        };
        if tpl.type_params.len() != req.args.len() {
            errs.push(diag(
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
            continue;
        }
        let mut subst = BTreeMap::new();
        for (k, v) in tpl.type_params.iter().zip(req.args.iter()) {
            subst.insert(k.clone(), v.clone());
        }
        let mut inst = instantiate_function(tpl, &subst);
        inst.name = mangle_name(&tpl.name, &req.args);
        inst.type_params.clear();
        inst.mono_origin = Some(format!(
            "{}<{}>",
            tpl.name,
            req.args
                .iter()
                .map(|t| format!("{t:?}"))
                .collect::<Vec<_>>()
                .join(", ")
        ));
        let cm = inst.cm.clone();
        let source_path = inst.source_path.clone();
        let span = inst.span;
        let mut env: HashMap<String, TsType> = inst.params.iter().cloned().collect();
        rewrite_stmts_in_scope(
            &mut inst.body,
            &templates,
            &mut queue,
            &cm,
            &source_path,
            span,
            &mut env,
            &mut errs,
        );
        out.push(inst);
    }

    if !errs.is_empty() {
        return Err(CompileError::merge_sorted(errs));
    }

    module.fns = out;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn rewrite_stmts_in_scope(
    stmts: &mut [IRStmt],
    templates: &HashMap<String, IRFunction>,
    queue: &mut VecDeque<Req>,
    cm: &crate::ir::SendSourceMap,
    path: &str,
    fn_span: swc_common::Span,
    env: &mut HashMap<String, TsType>,
    errs: &mut Vec<CompileError>,
) {
    for s in stmts.iter_mut() {
        rewrite_stmt_scoped(s, templates, queue, cm, path, fn_span, env, errs);
    }
}

#[allow(clippy::too_many_arguments)]
fn rewrite_stmt_scoped(
    s: &mut IRStmt,
    templates: &HashMap<String, IRFunction>,
    queue: &mut VecDeque<Req>,
    cm: &crate::ir::SendSourceMap,
    path: &str,
    fn_span: swc_common::Span,
    env: &mut HashMap<String, TsType>,
    errs: &mut Vec<CompileError>,
) {
    match s {
        IRStmt::Let { name, ty, init, .. } => {
            *ty = subst_type(ty, &BTreeMap::new());
            if let Some(e) = init {
                rewrite_expr(e, templates, queue, cm, path, fn_span, env, errs);
            }
            env.insert(name.clone(), ty.clone());
        }
        IRStmt::Assign { rhs, .. } => {
            rewrite_expr(rhs, templates, queue, cm, path, fn_span, env, errs);
        }
        IRStmt::MemberAssign { rhs, .. } => {
            rewrite_expr(rhs, templates, queue, cm, path, fn_span, env, errs);
        }
        IRStmt::Expr { expr, .. } => {
            rewrite_expr(expr, templates, queue, cm, path, fn_span, env, errs);
        }
        IRStmt::Return { arg, .. } => {
            if let Some(e) = arg {
                rewrite_expr(e, templates, queue, cm, path, fn_span, env, errs);
            }
        }
        IRStmt::Block { stmts, .. } => {
            let mut inner = env.clone();
            rewrite_stmts_in_scope(stmts, templates, queue, cm, path, fn_span, &mut inner, errs);
        }
        IRStmt::If {
            cond,
            then_b,
            else_b,
            ..
        } => {
            rewrite_expr(cond, templates, queue, cm, path, fn_span, env, errs);
            let mut then_env = env.clone();
            rewrite_stmts_in_scope(
                then_b,
                templates,
                queue,
                cm,
                path,
                fn_span,
                &mut then_env,
                errs,
            );
            if let Some(e) = else_b {
                let mut else_env = env.clone();
                rewrite_stmts_in_scope(e, templates, queue, cm, path, fn_span, &mut else_env, errs);
            }
        }
        IRStmt::While { cond, body, .. } => {
            rewrite_expr(cond, templates, queue, cm, path, fn_span, env, errs);
            let mut body_env = env.clone();
            rewrite_stmts_in_scope(
                body,
                templates,
                queue,
                cm,
                path,
                fn_span,
                &mut body_env,
                errs,
            );
        }
        IRStmt::ForIn { target, body, .. } => {
            rewrite_expr(target, templates, queue, cm, path, fn_span, env, errs);
            let mut body_env = env.clone();
            rewrite_stmts_in_scope(
                body,
                templates,
                queue,
                cm,
                path,
                fn_span,
                &mut body_env,
                errs,
            );
        }
        IRStmt::DoWhile { body, cond, .. } => {
            let mut body_env = env.clone();
            rewrite_stmts_in_scope(
                body,
                templates,
                queue,
                cm,
                path,
                fn_span,
                &mut body_env,
                errs,
            );
            rewrite_expr(cond, templates, queue, cm, path, fn_span, env, errs);
        }
        IRStmt::FnDecl { func, .. } => {
            let mut inner: HashMap<String, TsType> = func.params.iter().cloned().collect();
            rewrite_stmts_in_scope(
                &mut func.body,
                templates,
                queue,
                cm,
                path,
                fn_span,
                &mut inner,
                errs,
            );
        }
        IRStmt::Empty { .. } | IRStmt::Break { .. } | IRStmt::Continue { .. } => {}
    }
}

#[allow(clippy::too_many_arguments)]
fn rewrite_expr(
    e: &mut IRExpr,
    templates: &HashMap<String, IRFunction>,
    queue: &mut VecDeque<Req>,
    cm: &crate::ir::SendSourceMap,
    path: &str,
    fn_span: swc_common::Span,
    env: &HashMap<String, TsType>,
    errs: &mut Vec<CompileError>,
) {
    match e {
        IRExpr::Call {
            callee,
            args,
            type_args,
            span,
        }
        | IRExpr::OptionalCall {
            callee,
            args,
            type_args,
            span,
        } => {
            for a in args.iter_mut() {
                rewrite_expr(a, templates, queue, cm, path, fn_span, env, errs);
            }
            if templates.contains_key(callee) {
                let Some(tpl) = templates.get(callee) else {
                    return;
                };
                let arg_refs: Vec<&IRExpr> = args.iter().collect();
                let resolved = resolve_generic_type_args(
                    tpl,
                    &tpl.params,
                    &arg_refs,
                    env,
                    type_args,
                    cm.as_ref(),
                    path,
                    *span,
                    errs,
                );
                let Some(args_res) = resolved else {
                    return;
                };
                let fp = type_fingerprint(&args_res);
                let req = Req {
                    callee: callee.clone(),
                    args_fingerprint: fp,
                    args: args_res,
                };
                queue.push_back(req.clone());
                *callee = mangle_name(&req.callee, &req.args);
                type_args.clear();
            } else if !type_args.is_empty() {
                errs.push(diag(
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
            method,
            args,
            type_args,
            span,
            inherent_rust,
            ..
        }
        | IRExpr::OptionalMethodCall {
            receiver,
            method,
            args,
            type_args,
            span,
            inherent_rust,
            ..
        } => {
            rewrite_expr(receiver, templates, queue, cm, path, fn_span, env, errs);
            for a in args.iter_mut() {
                rewrite_expr(a, templates, queue, cm, path, fn_span, env, errs);
            }
            if inherent_rust.is_some() {
                return;
            }
            if templates.contains_key(method.as_str()) {
                let Some(tpl) = templates.get(method.as_str()) else {
                    return;
                };
                let mut arg_refs: Vec<&IRExpr> = Vec::new();
                arg_refs.push(receiver.as_ref());
                arg_refs.extend(args.iter());
                let resolved = resolve_generic_type_args(
                    tpl,
                    &tpl.params,
                    &arg_refs,
                    env,
                    type_args,
                    cm.as_ref(),
                    path,
                    *span,
                    errs,
                );
                let Some(args_res) = resolved else {
                    return;
                };
                let req = Req {
                    callee: method.clone(),
                    args_fingerprint: type_fingerprint(&args_res),
                    args: args_res,
                };
                queue.push_back(req.clone());
                *method = mangle_name(method.as_str(), &req.args);
                type_args.clear();
            } else if !type_args.is_empty() {
                errs.push(diag(
                    cm.as_ref(),
                    path,
                    *span,
                    format!(
                        "type arguments are only allowed on generic functions, got `{method}`",
                    ),
                ));
            }
        }
        IRExpr::Binary { left, right, .. } => {
            rewrite_expr(left, templates, queue, cm, path, fn_span, env, errs);
            rewrite_expr(right, templates, queue, cm, path, fn_span, env, errs);
        }
        IRExpr::Unary { arg, .. } => {
            rewrite_expr(arg, templates, queue, cm, path, fn_span, env, errs);
        }
        IRExpr::Conditional {
            test, cons, alt, ..
        } => {
            rewrite_expr(test, templates, queue, cm, path, fn_span, env, errs);
            rewrite_expr(cons, templates, queue, cm, path, fn_span, env, errs);
            rewrite_expr(alt, templates, queue, cm, path, fn_span, env, errs);
        }
        IRExpr::Seq { exprs, .. } => {
            for x in exprs.iter_mut() {
                rewrite_expr(x, templates, queue, cm, path, fn_span, env, errs);
            }
        }
        IRExpr::Tpl { parts, .. } => {
            for p in parts.iter_mut() {
                if let TplPart::Interp(e) = p {
                    rewrite_expr(e, templates, queue, cm, path, fn_span, env, errs);
                }
            }
        }
        IRExpr::Member { obj, .. } => {
            rewrite_expr(obj, templates, queue, cm, path, fn_span, env, errs);
        }
        IRExpr::OptionalMember { obj, .. } => {
            rewrite_expr(obj, templates, queue, cm, path, fn_span, env, errs);
        }
        IRExpr::NullishCoalesce { left, right, .. } => {
            rewrite_expr(left, templates, queue, cm, path, fn_span, env, errs);
            rewrite_expr(right, templates, queue, cm, path, fn_span, env, errs);
        }
        IRExpr::MathBuiltin { args, .. }
        | IRExpr::BuiltinLog { args, .. }
        | IRExpr::NumberBuiltin { args, .. }
        | IRExpr::JsonBuiltin { args, .. }
        | IRExpr::UriBuiltin { args, .. } => {
            for a in args.iter_mut() {
                rewrite_expr(a, templates, queue, cm, path, fn_span, env, errs);
            }
        }
        IRExpr::StringMethodBuiltin { receiver, args, .. } => {
            rewrite_expr(receiver, templates, queue, cm, path, fn_span, env, errs);
            for a in args.iter_mut() {
                rewrite_expr(a, templates, queue, cm, path, fn_span, env, errs);
            }
        }
        IRExpr::ReadStdinLine { .. } => {}
        IRExpr::ArrayLit { elems, .. } => {
            for a in elems.iter_mut() {
                rewrite_expr(a, templates, queue, cm, path, fn_span, env, errs);
            }
        }
        IRExpr::ObjectLit { fields, .. } => {
            for (_, v) in fields.iter_mut() {
                rewrite_expr(v, templates, queue, cm, path, fn_span, env, errs);
            }
        }
        IRExpr::Index { obj, index, .. } => {
            rewrite_expr(obj, templates, queue, cm, path, fn_span, env, errs);
            rewrite_expr(index, templates, queue, cm, path, fn_span, env, errs);
        }
        IRExpr::ArrowFn { params, body, .. } => {
            let mut inner: HashMap<String, TsType> = params.iter().cloned().collect();
            rewrite_stmts_in_scope(body, templates, queue, cm, path, fn_span, &mut inner, errs);
        }
        IRExpr::Await { arg, .. } => {
            rewrite_expr(arg, templates, queue, cm, path, fn_span, env, errs);
        }
        IRExpr::FetchText { url, .. } => {
            rewrite_expr(url, templates, queue, cm, path, fn_span, env, errs);
        }
        IRExpr::Fetch { url, init, .. } => {
            rewrite_expr(url, templates, queue, cm, path, fn_span, env, errs);
            if let Some(i) = init {
                if let Some(b) = &mut i.body {
                    rewrite_expr(b, templates, queue, cm, path, fn_span, env, errs);
                }
            }
        }
        IRExpr::HttpResponseMethodBuiltin { receiver, .. } => {
            rewrite_expr(receiver, templates, queue, cm, path, fn_span, env, errs);
        }
        IRExpr::HttpResponseBodyGetReader { response, .. } => {
            rewrite_expr(response, templates, queue, cm, path, fn_span, env, errs);
        }
        IRExpr::ReaderRead { .. } => {}
        IRExpr::PromiseAll { elems, .. } => {
            for a in elems.iter_mut() {
                rewrite_expr(a, templates, queue, cm, path, fn_span, env, errs);
            }
        }
        IRExpr::RustNew { args, .. } => {
            for a in args.iter_mut() {
                rewrite_expr(a, templates, queue, cm, path, fn_span, env, errs);
            }
        }
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

#[allow(clippy::too_many_arguments)]
fn resolve_generic_type_args(
    tpl: &IRFunction,
    param_pairs: &[(String, TsType)],
    arg_exprs: &[&IRExpr],
    env: &HashMap<String, TsType>,
    type_args: &[TsType],
    cm: &swc_common::SourceMap,
    path: &str,
    span: swc_common::Span,
    errs: &mut Vec<CompileError>,
) -> Option<Vec<TsType>> {
    if !type_args.is_empty() {
        if tpl.type_params.len() != type_args.len() {
            errs.push(diag(
                cm,
                path,
                span,
                format!(
                    "wrong type argument count for `{}`: expected {}, got {}",
                    tpl.name,
                    tpl.type_params.len(),
                    type_args.len()
                ),
            ));
            return None;
        }
        return Some(type_args.to_vec());
    }
    match infer_type_args_from_call(tpl, param_pairs, arg_exprs, env, cm, path, span) {
        Ok(v) => Some(v),
        Err(e) => {
            errs.push(e);
            None
        }
    }
}

fn infer_type_args_from_call(
    tpl: &IRFunction,
    param_pairs: &[(String, TsType)],
    arg_exprs: &[&IRExpr],
    env: &HashMap<String, TsType>,
    cm: &swc_common::SourceMap,
    path: &str,
    span: swc_common::Span,
) -> Result<Vec<TsType>, CompileError> {
    if param_pairs.len() != arg_exprs.len() {
        return Err(diag(
            cm,
            path,
            span,
            format!(
                "internal: generic `{}` parameter count mismatch at monomorphization",
                tpl.name
            ),
        ));
    }
    let mut subst: BTreeMap<String, TsType> = BTreeMap::new();
    for ((_, pt), arg) in param_pairs.iter().zip(arg_exprs.iter()) {
        let Some(at) = synth_expr_ty(arg, env) else {
            return Err(diag(
                cm,
                path,
                span,
                format!(
                    "cannot infer type arguments for generic function `{}` (argument type not known at monomorphization)",
                    tpl.name
                ),
            ));
        };
        unify_infer(pt, &at, &mut subst, cm, path, span)?;
    }
    let mut out = Vec::new();
    for tp_name in &tpl.type_params {
        let Some(t) = subst.get(tp_name) else {
            return Err(diag(
                cm,
                path,
                span,
                format!(
                    "cannot infer type parameter `{tp_name}` for generic function `{fn_name}`",
                    tp_name = tp_name,
                    fn_name = tpl.name
                ),
            ));
        };
        out.push(t.clone());
    }
    Ok(out)
}

fn synth_expr_ty(e: &IRExpr, env: &HashMap<String, TsType>) -> Option<TsType> {
    match e {
        IRExpr::Number(_, _) => Some(TsType::Number),
        IRExpr::Bool(_, _) => Some(TsType::Boolean),
        IRExpr::Str(_, _) => Some(TsType::String),
        IRExpr::Ident(n, _) => env.get(n).cloned(),
        IRExpr::Null(_) => Some(TsType::Null),
        IRExpr::Undefined(_) => Some(TsType::Undefined),
        _ => None,
    }
}

fn widen_for_infer(t: &TsType) -> TsType {
    match t {
        TsType::NumberLit(_) => TsType::Number,
        TsType::BoolLit(_) => TsType::Boolean,
        TsType::StringLit(_) => TsType::String,
        _ => t.clone(),
    }
}

fn types_compatible_infer(a: &TsType, b: &TsType) -> bool {
    widen_for_infer(a) == widen_for_infer(b)
}

fn contains_type_param(t: &TsType) -> bool {
    match t {
        TsType::TypeParam(_) => true,
        TsType::Union(v) => v.iter().any(contains_type_param),
        TsType::Promise(inner) => contains_type_param(inner),
        TsType::Fn { params, ret } => {
            params.iter().any(contains_type_param) || contains_type_param(ret)
        }
        _ => false,
    }
}

fn unify_infer(
    formal: &TsType,
    actual: &TsType,
    subst: &mut BTreeMap<String, TsType>,
    cm: &swc_common::SourceMap,
    path: &str,
    span: swc_common::Span,
) -> Result<(), CompileError> {
    let actual_w = widen_for_infer(actual);
    match formal {
        TsType::TypeParam(p) => match subst.get(p) {
            None => {
                subst.insert(p.clone(), actual_w);
                Ok(())
            }
            Some(prev) => {
                if types_compatible_infer(prev, &actual_w) {
                    Ok(())
                } else {
                    Err(diag(
                        cm,
                        path,
                        span,
                        format!("conflicting inferred type arguments for type parameter `{p}`"),
                    ))
                }
            }
        },
        TsType::Promise(f_inner) => match &actual_w {
            TsType::Promise(a_inner) => unify_infer(f_inner, a_inner, subst, cm, path, span),
            _ => Err(diag(
                cm,
                path,
                span,
                "type mismatch for generic call (`Promise` expected)",
            )),
        },
        TsType::Fn {
            params: fp,
            ret: fret,
        } => match &actual_w {
            TsType::Fn {
                params: ap,
                ret: aret,
            } => {
                if fp.len() != ap.len() {
                    return Err(diag(
                        cm,
                        path,
                        span,
                        "function type arity mismatch for generic call",
                    ));
                }
                for (f, a) in fp.iter().zip(ap.iter()) {
                    unify_infer(f, a, subst, cm, path, span)?;
                }
                unify_infer(fret, aret, subst, cm, path, span)
            }
            _ => Err(diag(
                cm,
                path,
                span,
                "type mismatch for generic call (function type expected)",
            )),
        },
        _ => {
            if contains_type_param(formal) {
                return Err(diag(
                    cm,
                    path,
                    span,
                    "cannot infer type arguments (unsupported parameter type for this subset)",
                ));
            }
            if types_compatible_infer(formal, &actual_w) {
                Ok(())
            } else {
                Err(diag(
                    cm,
                    path,
                    span,
                    format!(
                        "type mismatch for generic call: expected `{formal:?}`, got `{actual_w:?}`"
                    ),
                ))
            }
        }
    }
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
        }
        | IRExpr::OptionalCall {
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
        }
        | IRExpr::OptionalMethodCall {
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
        IRExpr::MathBuiltin { args, .. }
        | IRExpr::BuiltinLog { args, .. }
        | IRExpr::NumberBuiltin { args, .. } => {
            for a in args {
                subst_expr(a, subst);
            }
        }
        IRExpr::JsonBuiltin {
            args,
            stringify_inferred_ty,
            ..
        } => {
            for a in args {
                subst_expr(a, subst);
            }
            if let Some(t) = stringify_inferred_ty {
                *t = subst_type(t, subst);
            }
        }
        IRExpr::UriBuiltin { args, .. } => {
            for a in args {
                subst_expr(a, subst);
            }
        }
        IRExpr::StringMethodBuiltin { receiver, args, .. } => {
            subst_expr(receiver, subst);
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
        IRExpr::Fetch { url, init, .. } => {
            subst_expr(url, subst);
            if let Some(i) = init {
                if let Some(b) = &mut i.body {
                    subst_expr(b, subst);
                }
            }
        }
        IRExpr::HttpResponseMethodBuiltin { receiver, .. } => subst_expr(receiver, subst),
        IRExpr::HttpResponseBodyGetReader { response, .. } => subst_expr(response, subst),
        IRExpr::ReaderRead { .. } => {}
        IRExpr::PromiseAll { elems, .. } => {
            for a in elems {
                subst_expr(a, subst);
            }
        }
        IRExpr::RustNew {
            result_ty, args, ..
        } => {
            *result_ty = subst_type(result_ty, subst);
            for a in args {
                subst_expr(a, subst);
            }
        }
        IRExpr::Number(..)
        | IRExpr::Bool(..)
        | IRExpr::Str(..)
        | IRExpr::Ident(..)
        | IRExpr::Null(..)
        | IRExpr::Undefined(..)
        | IRExpr::This(..)
        | IRExpr::Super(..)
        | IRExpr::ReadStdinLine { .. } => {}
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
        TsType::ObjectNum(props) => TsType::ObjectNum(
            props
                .iter()
                .map(|p| ObjectProp {
                    name: p.name.clone(),
                    optional: p.optional,
                    ty: Box::new(subst_type(&p.ty, subst)),
                })
                .collect(),
        ),
        TsType::ClassInstance(_) | TsType::RustExtern { .. } => t.clone(),
        _ => t.clone(),
    }
}

fn fnv1a64(bytes: &[u8]) -> u64 {
    const OFFSET: u64 = 14695981039346656037;
    const PRIME: u64 = 1099511628211;
    let mut h = OFFSET;
    for b in bytes {
        h ^= *b as u64;
        h = h.wrapping_mul(PRIME);
    }
    h
}

fn type_fingerprint(args: &[TsType]) -> u64 {
    let s = args.iter().map(type_key).collect::<Vec<_>>().join("|");
    fnv1a64(s.as_bytes())
}

fn mangle_name(name: &str, args: &[TsType]) -> String {
    format!("{}__{:016x}", name, type_fingerprint(args))
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
        TsType::ArrayString => "arrs".to_string(),
        TsType::ArrayHttpResponse => "arrhr".to_string(),
        TsType::HttpResponse => "httpres".to_string(),
        TsType::ReadableStream => "rstream".to_string(),
        TsType::ReadableStreamDefaultReader => "rsreader".to_string(),
        TsType::StreamReadResult => "sreadres".to_string(),
        TsType::Uint8Array => "u8arr".to_string(),
        TsType::ObjectNum(props) => {
            let mut s = String::from("obj");
            for p in props {
                s.push('_');
                s.push_str(&p.name);
                s.push('_');
                s.push(if p.optional { '1' } else { '0' });
                s.push('_');
                s.push_str(&type_key(&p.ty));
            }
            s
        }
        TsType::Union(m) => format!("u{}", m.iter().map(type_key).collect::<Vec<_>>().join("_")),
        TsType::TypeParam(n) => format!("tp{n}"),
        TsType::ClassInstance(n) => format!("cls{n}"),
        TsType::Fn { params, ret } => format!(
            "fn{}_r{}",
            params.iter().map(type_key).collect::<Vec<_>>().join("_"),
            type_key(ret)
        ),
        TsType::Promise(inner) => format!("P{}", type_key(inner)),
        TsType::RustExtern {
            crate_key,
            export_name,
            rust_type,
            ..
        } => {
            let rt = rust_type.replace(':', "_");
            format!("rust_{crate_key}_{export_name}_{rt}")
        }
    }
}
