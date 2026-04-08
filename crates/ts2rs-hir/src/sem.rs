//! 名字解析、类型检查、简化 return 路径检查。

use std::collections::HashMap;

use swc_common::{sync::Lrc, SourceMap, Span, DUMMY_SP};

use crate::error::{diag, warn, CompileError, CompileWarning};
use crate::ir::*;

mod helpers;
mod mono;

#[derive(Clone)]
struct FnSig {
    type_params: Vec<String>,
    params: Vec<TsType>,
    ret: TsType,
    is_async: bool,
}

#[derive(Clone)]
struct Binding {
    ty: TsType,
    mutable: bool,
    initialized: bool,
}

fn is_numberish(t: &TsType) -> bool {
    helpers::is_numberish(t)
}

fn is_booleanish(t: &TsType) -> bool {
    helpers::is_booleanish(t)
}

fn is_stringish(t: &TsType) -> bool {
    helpers::is_stringish(t)
}

/// `if` / `while` / 三元条件：`number`、`boolean`，或**成员全为同一族**的联合（如 `1 | 2`、`true | false`），不含 `number | boolean` 等混合。
fn is_cond_ty(t: &TsType) -> bool {
    helpers::is_cond_ty(t)
}

/// `got` 是否可赋给注解类型 `expected`（含字面量向 `number`/`boolean`/`string` 拓宽）。
fn type_assignable(expected: &TsType, got: &TsType) -> bool {
    helpers::type_assignable(expected, got)
}

fn unify_ternary_branches(
    a: TsType,
    b: TsType,
    cm: &Lrc<SourceMap>,
    path: &str,
    span: Span,
) -> Result<TsType, CompileError> {
    helpers::unify_ternary_branches(a, b, cm, path, span)
}

fn paths_equal_file(a: &str, b: &str) -> bool {
    helpers::paths_equal_file(a, b)
}

fn subst_type_local(t: &TsType, subst: &HashMap<String, TsType>) -> TsType {
    match t {
        TsType::TypeParam(n) => subst.get(n).cloned().unwrap_or_else(|| t.clone()),
        TsType::Union(v) => normalize_union(v.iter().map(|x| subst_type_local(x, subst)).collect()),
        TsType::Fn { params, ret } => TsType::Fn {
            params: params.iter().map(|x| subst_type_local(x, subst)).collect(),
            ret: Box::new(subst_type_local(ret, subst)),
        },
        TsType::Promise(inner) => TsType::Promise(Box::new(subst_type_local(inner, subst))),
        _ => t.clone(),
    }
}

pub fn check_module(module: &mut IRModule) -> Result<Vec<CompileWarning>, CompileError> {
    validate_classes(module)?;
    mono::monomorphize_module_functions(module)?;

    let mut globals: HashMap<String, FnSig> = HashMap::new();
    for f in &module.fns {
        if globals
            .insert(
                f.name.clone(),
                FnSig {
                    type_params: f.type_params.clone(),
                    params: f.params.iter().map(|(_, t)| t.clone()).collect(),
                    ret: f.ret.clone(),
                    is_async: f.is_async,
                },
            )
            .is_some()
        {
            return Err(diag(
                &f.cm,
                &f.source_path,
                f.span,
                format!("duplicate function `{}`", f.name),
            ));
        }
    }

    let main_fn = module
        .fns
        .iter()
        .find(|f| f.name == "main")
        .ok_or_else(|| {
            if let Some(first) = module.fns.first() {
                return diag(
                    &first.cm,
                    &first.source_path,
                    first.span,
                    "missing required entry point `function main()`",
                );
            }
            // 退化：无顶层函数时由 build 阶段报错；此处仅占位。
            diag(
                &Lrc::new(SourceMap::default()),
                module.entry_path.as_str(),
                DUMMY_SP,
                "missing required entry point `function main()`",
            )
        })?;
    if !paths_equal_file(&main_fn.source_path, &module.entry_path) {
        return Err(diag(
            &main_fn.cm,
            &main_fn.source_path,
            main_fn.span,
            "entry point `main` must be defined in the entry file",
        ));
    }

    let mut warnings = Vec::new();
    for f in &mut module.fns {
        let cm = f.cm.clone();
        let p = f.source_path.clone();
        check_function(f, &globals, &cm, &p, &mut warnings)?;
    }
    Ok(warnings)
}

fn validate_classes(module: &IRModule) -> Result<(), CompileError> {
    let mut cmap: HashMap<String, &IRClass> = HashMap::new();
    for c in &module.classes {
        if cmap.insert(c.name.clone(), c).is_some() {
            return Err(diag(
                c.cm.as_ref(),
                &c.source_path,
                c.span,
                format!("duplicate class `{}`", c.name),
            ));
        }
    }
    for c in &module.classes {
        if let Some(p) = &c.extends {
            if p == &c.name {
                return Err(diag(
                    c.cm.as_ref(),
                    &c.source_path,
                    c.span,
                    "class cannot extend itself",
                ));
            }
            let parent = cmap.get(p).ok_or_else(|| {
                diag(
                    c.cm.as_ref(),
                    &c.source_path,
                    c.span,
                    format!("unknown base class `{p}`"),
                )
            })?;
            let mut p_methods = HashMap::<String, (&Vec<(String, TsType)>, &TsType)>::new();
            for m in &parent.methods {
                p_methods.insert(m.name.clone(), (&m.params, &m.ret));
            }
            for m in &c.methods {
                if let Some((pp, pr)) = p_methods.get(&m.name) {
                    if !m.is_override {
                        return Err(diag(
                            c.cm.as_ref(),
                            &c.source_path,
                            m.span,
                            format!(
                                "method `{}` overrides base method and must use `override`",
                                m.name
                            ),
                        ));
                    }
                    if *pp != &m.params || *pr != &m.ret {
                        return Err(diag(
                            c.cm.as_ref(),
                            &c.source_path,
                            m.span,
                            format!("override signature mismatch for method `{}`", m.name),
                        ));
                    }
                } else if m.is_override {
                    return Err(diag(
                        c.cm.as_ref(),
                        &c.source_path,
                        m.span,
                        format!("`override` method `{}` not found in base class", m.name),
                    ));
                }
            }
            if let Some(ctor) = &c.ctor {
                let has_super_first = matches!(
                    ctor.body.first(),
                    Some(IRStmt::Expr {
                        expr: IRExpr::Call { callee, .. },
                        ..
                    }) if callee == "__super_ctor"
                );
                if !has_super_first {
                    return Err(diag(
                        c.cm.as_ref(),
                        &c.source_path,
                        ctor.span,
                        "subclass constructor must start with `super(...)`",
                    ));
                }
            }
        } else if let Some(ctor) = &c.ctor {
            for s in &ctor.body {
                if matches!(
                    s,
                    IRStmt::Expr {
                        expr: IRExpr::Call { callee, .. },
                        ..
                    } if callee == "__super_ctor"
                ) {
                    return Err(diag(
                        c.cm.as_ref(),
                        &c.source_path,
                        ctor.span,
                        "`super(...)` is only valid in subclass constructor",
                    ));
                }
            }
        }
    }
    Ok(())
}

fn collect_fn_sigs_in_stmts(stmts: &[IRStmt]) -> HashMap<String, FnSig> {
    let mut m = HashMap::new();
    for s in stmts {
        if let IRStmt::FnDecl { func, .. } = s {
            if m.insert(
                func.name.clone(),
                FnSig {
                    type_params: func.type_params.clone(),
                    params: func.params.iter().map(|(_, t)| t.clone()).collect(),
                    ret: func.ret.clone(),
                    is_async: func.is_async,
                },
            )
            .is_some()
            {
                // duplicate nested name — checked later with span if needed
            }
        }
    }
    m
}

fn check_function(
    f: &mut IRFunction,
    globals: &HashMap<String, FnSig>,
    cm: &Lrc<SourceMap>,
    path: &str,
    warnings: &mut Vec<CompileWarning>,
) -> Result<(), CompileError> {
    let mut merged = globals.clone();
    merged.extend(collect_fn_sigs_in_stmts(&f.body));

    let mut stack: Vec<HashMap<String, Binding>> = Vec::new();
    let mut root = HashMap::new();
    for (n, t) in &f.params {
        if root
            .insert(
                n.clone(),
                Binding {
                    ty: t.clone(),
                    mutable: true,
                    initialized: true,
                },
            )
            .is_some()
        {
            return Err(diag(cm, path, f.span, format!("duplicate parameter `{n}`")));
        }
    }
    stack.push(root);

    if f.is_async {
        check_async_mvp_stmts(&f.body, cm, path, f.span)?;
        reject_naked_fetchtext_in_stmts(&f.body, cm, path)?;
        reject_readline_in_async_stmts(&f.body, cm, path)?;
    }

    let mut reachable = true;
    check_stmts(
        &mut f.body,
        &mut stack,
        &merged,
        &f.ret,
        0,
        cm,
        path,
        warnings,
        &mut reachable,
    )?;

    if f.ret != TsType::Void && !fn_body_returns(&f.body, &f.ret) {
        return Err(diag(
            cm,
            path,
            f.span,
            "not all control paths return a value (check last statement and `if`/`else`)",
        ));
    }
    Ok(())
}

fn push_scope(stack: &mut Vec<HashMap<String, Binding>>) {
    stack.push(HashMap::new());
}

fn pop_scope(stack: &mut Vec<HashMap<String, Binding>>) {
    stack.pop();
}

fn lookup(stack: &[HashMap<String, Binding>], name: &str) -> Option<Binding> {
    for m in stack.iter().rev() {
        if let Some(b) = m.get(name) {
            return Some(b.clone());
        }
    }
    None
}

fn insert_let(
    stack: &mut Vec<HashMap<String, Binding>>,
    name: &str,
    ty: TsType,
    mutable: bool,
    initialized: bool,
    span: Span,
    cm: &Lrc<SourceMap>,
    path: &str,
) -> Result<(), CompileError> {
    if stack
        .last_mut()
        .unwrap()
        .insert(
            name.to_string(),
            Binding {
                ty,
                mutable,
                initialized,
            },
        )
        .is_some()
    {
        return Err(diag(
            cm,
            path,
            span,
            format!("duplicate `let`/`const` binding `{name}` in the same block"),
        ));
    }
    Ok(())
}

fn mark_initialized(stack: &mut Vec<HashMap<String, Binding>>, name: &str) {
    for m in stack.iter_mut().rev() {
        if let Some(b) = m.get_mut(name) {
            b.initialized = true;
            return;
        }
    }
}

fn snapshot_inits(stack: &[HashMap<String, Binding>]) -> Vec<HashMap<String, bool>> {
    stack
        .iter()
        .map(|m| m.iter().map(|(k, v)| (k.clone(), v.initialized)).collect())
        .collect()
}

fn restore_inits(stack: &mut [HashMap<String, Binding>], snap: &[HashMap<String, bool>]) {
    for (m, s) in stack.iter_mut().zip(snap.iter()) {
        for (name, init) in s {
            if let Some(b) = m.get_mut(name) {
                b.initialized = *init;
            }
        }
    }
}

fn merge_init_after_if_else(
    stack: &mut [HashMap<String, Binding>],
    pre: &[HashMap<String, bool>],
    then_snap: &[HashMap<String, bool>],
    else_snap: &[HashMap<String, bool>],
) {
    for i in 0..stack.len() {
        let names: Vec<String> = stack[i].keys().cloned().collect();
        for name in names {
            let p = pre
                .get(i)
                .and_then(|m| m.get(&name))
                .copied()
                .unwrap_or(true);
            let t = then_snap
                .get(i)
                .and_then(|m| m.get(&name))
                .copied()
                .unwrap_or(p);
            let e = else_snap
                .get(i)
                .and_then(|m| m.get(&name))
                .copied()
                .unwrap_or(p);
            if let Some(b) = stack[i].get_mut(&name) {
                b.initialized = t && e;
            }
        }
    }
}

fn merge_init_after_if_no_else(
    stack: &mut [HashMap<String, Binding>],
    pre: &[HashMap<String, bool>],
    then_snap: &[HashMap<String, bool>],
) {
    for i in 0..stack.len() {
        let names: Vec<String> = stack[i].keys().cloned().collect();
        for name in names {
            let p = pre
                .get(i)
                .and_then(|m| m.get(&name))
                .copied()
                .unwrap_or(true);
            let t = then_snap
                .get(i)
                .and_then(|m| m.get(&name))
                .copied()
                .unwrap_or(p);
            if let Some(b) = stack[i].get_mut(&name) {
                b.initialized = p && t;
            }
        }
    }
}

fn apply_loop_conservative_init(
    stack: &mut [HashMap<String, Binding>],
    pre: &[HashMap<String, bool>],
) {
    for i in 0..stack.len() {
        for (name, b) in stack[i].iter_mut() {
            let was_init = pre
                .get(i)
                .and_then(|m| m.get(name))
                .copied()
                .unwrap_or(false);
            if !was_init {
                b.initialized = false;
            }
        }
    }
}

fn stmt_span(s: &IRStmt) -> Span {
    match s {
        IRStmt::Empty { span }
        | IRStmt::Let { span, .. }
        | IRStmt::Assign { span, .. }
        | IRStmt::MemberAssign { span, .. }
        | IRStmt::Expr { span, .. }
        | IRStmt::Return { span, .. }
        | IRStmt::Block { span, .. }
        | IRStmt::If { span, .. }
        | IRStmt::While { span, .. }
        | IRStmt::ForIn { span, .. }
        | IRStmt::DoWhile { span, .. }
        | IRStmt::Break { span }
        | IRStmt::Continue { span }
        | IRStmt::FnDecl { span, .. } => *span,
    }
}

fn check_async_mvp_stmts(
    stmts: &[IRStmt],
    cm: &Lrc<SourceMap>,
    path: &str,
    _fn_span: Span,
) -> Result<(), CompileError> {
    for s in stmts {
        match s {
            IRStmt::If { span, .. }
            | IRStmt::While { span, .. }
            | IRStmt::ForIn { span, .. }
            | IRStmt::DoWhile { span, .. } => {
                return Err(diag(
                    cm,
                    path,
                    *span,
                    "async MVP: `if` / `while` / `for..in` / `do..while` are not allowed",
                ));
            }
            IRStmt::Break { span } | IRStmt::Continue { span } => {
                return Err(diag(
                    cm,
                    path,
                    *span,
                    "async MVP: `break` / `continue` are not allowed",
                ));
            }
            IRStmt::FnDecl { span, .. } => {
                return Err(diag(
                    cm,
                    path,
                    *span,
                    "async MVP: nested `function` declarations are not allowed",
                ));
            }
            IRStmt::Block { stmts: inner, .. } => {
                check_async_mvp_stmts(inner, cm, path, _fn_span)?;
            }
            _ => {}
        }
    }
    Ok(())
}

fn reject_naked_fetchtext_in_stmts(
    stmts: &[IRStmt],
    cm: &Lrc<SourceMap>,
    path: &str,
) -> Result<(), CompileError> {
    for s in stmts {
        match s {
            IRStmt::Let { init, span, .. } => {
                if let Some(e) = init {
                    if matches!(e, IRExpr::FetchText { .. }) {
                        return Err(diag(
                            cm,
                            path,
                            *span,
                            "`fetchText(...)` must be used as `await fetchText(...)`",
                        ));
                    }
                }
            }
            IRStmt::Return { arg: Some(e), span } => {
                if matches!(e, IRExpr::FetchText { .. }) {
                    return Err(diag(
                        cm,
                        path,
                        *span,
                        "`fetchText(...)` must be used as `await fetchText(...)`",
                    ));
                }
            }
            IRStmt::Expr { expr, span } => {
                if matches!(expr, IRExpr::FetchText { .. }) {
                    return Err(diag(
                        cm,
                        path,
                        *span,
                        "`fetchText(...)` must be used as `await fetchText(...)`",
                    ));
                }
            }
            IRStmt::Assign { rhs, span, .. } => {
                if matches!(rhs, IRExpr::FetchText { .. }) {
                    return Err(diag(
                        cm,
                        path,
                        *span,
                        "`fetchText(...)` must be used as `await fetchText(...)`",
                    ));
                }
            }
            IRStmt::MemberAssign { rhs, span, .. } => {
                if matches!(rhs, IRExpr::FetchText { .. }) {
                    return Err(diag(
                        cm,
                        path,
                        *span,
                        "`fetchText(...)` must be used as `await fetchText(...)`",
                    ));
                }
            }
            IRStmt::Block { stmts: inner, .. } => {
                reject_naked_fetchtext_in_stmts(inner, cm, path)?;
            }
            IRStmt::If {
                cond,
                then_b,
                else_b,
                ..
            } => {
                if matches!(cond, IRExpr::FetchText { .. }) {
                    return Err(diag(
                        cm,
                        path,
                        stmt_span(s),
                        "`fetchText(...)` must be used as `await fetchText(...)`",
                    ));
                }
                reject_naked_fetchtext_in_stmts(then_b, cm, path)?;
                if let Some(eb) = else_b {
                    reject_naked_fetchtext_in_stmts(eb, cm, path)?;
                }
            }
            IRStmt::While { cond, body, .. } => {
                if matches!(cond, IRExpr::FetchText { .. }) {
                    return Err(diag(
                        cm,
                        path,
                        stmt_span(s),
                        "`fetchText(...)` must be used as `await fetchText(...)`",
                    ));
                }
                reject_naked_fetchtext_in_stmts(body, cm, path)?;
            }
            IRStmt::ForIn { target, body, .. } => {
                if matches!(target, IRExpr::FetchText { .. }) {
                    return Err(diag(
                        cm,
                        path,
                        stmt_span(s),
                        "`fetchText(...)` must be used as `await fetchText(...)`",
                    ));
                }
                reject_naked_fetchtext_in_stmts(body, cm, path)?;
            }
            IRStmt::DoWhile { body, cond, .. } => {
                reject_naked_fetchtext_in_stmts(body, cm, path)?;
                if matches!(cond, IRExpr::FetchText { .. }) {
                    return Err(diag(
                        cm,
                        path,
                        stmt_span(s),
                        "`fetchText(...)` must be used as `await fetchText(...)`",
                    ));
                }
            }
            IRStmt::FnDecl { func, .. } => {
                reject_naked_fetchtext_in_stmts(&func.body, cm, path)?;
            }
            _ => {}
        }
    }
    Ok(())
}

fn reject_readline_in_async_stmts(
    stmts: &[IRStmt],
    cm: &Lrc<SourceMap>,
    path: &str,
) -> Result<(), CompileError> {
    for s in stmts {
        match s {
            IRStmt::Let { init, span, .. } => {
                if let Some(e) = init {
                    reject_readline_in_async_expr(e, cm, path, *span)?;
                }
            }
            IRStmt::Return { arg: Some(e), span } => {
                reject_readline_in_async_expr(e, cm, path, *span)?;
            }
            IRStmt::Expr { expr, span } => {
                reject_readline_in_async_expr(expr, cm, path, *span)?;
            }
            IRStmt::Assign { rhs, span, .. } => {
                reject_readline_in_async_expr(rhs, cm, path, *span)?;
            }
            IRStmt::MemberAssign { rhs, span, .. } => {
                reject_readline_in_async_expr(rhs, cm, path, *span)?;
            }
            IRStmt::Block { stmts: inner, .. } => {
                reject_readline_in_async_stmts(inner, cm, path)?;
            }
            IRStmt::If {
                cond,
                then_b,
                else_b,
                ..
            } => {
                reject_readline_in_async_expr(cond, cm, path, stmt_span(s))?;
                reject_readline_in_async_stmts(then_b, cm, path)?;
                if let Some(eb) = else_b {
                    reject_readline_in_async_stmts(eb, cm, path)?;
                }
            }
            IRStmt::While { cond, body, .. } => {
                reject_readline_in_async_expr(cond, cm, path, stmt_span(s))?;
                reject_readline_in_async_stmts(body, cm, path)?;
            }
            IRStmt::ForIn { target, body, .. } => {
                reject_readline_in_async_expr(target, cm, path, stmt_span(s))?;
                reject_readline_in_async_stmts(body, cm, path)?;
            }
            IRStmt::DoWhile { body, cond, .. } => {
                reject_readline_in_async_stmts(body, cm, path)?;
                reject_readline_in_async_expr(cond, cm, path, stmt_span(s))?;
            }
            IRStmt::FnDecl { func, .. } => {
                reject_readline_in_async_stmts(&func.body, cm, path)?;
            }
            _ => {}
        }
    }
    Ok(())
}

fn reject_readline_in_async_expr(
    e: &IRExpr,
    cm: &Lrc<SourceMap>,
    path: &str,
    span: Span,
) -> Result<(), CompileError> {
    match e {
        IRExpr::ReadStdinLine { .. } => Err(diag(
            cm,
            path,
            span,
            "`readLine()` is not supported in `async` functions",
        )),
        IRExpr::Binary { left, right, .. } => {
            reject_readline_in_async_expr(left, cm, path, span)?;
            reject_readline_in_async_expr(right, cm, path, span)
        }
        IRExpr::Unary { arg, .. } => reject_readline_in_async_expr(arg, cm, path, span),
        IRExpr::Call { args, .. } => {
            for a in args {
                reject_readline_in_async_expr(a, cm, path, span)?;
            }
            Ok(())
        }
        IRExpr::MethodCall { receiver, args, .. } => {
            reject_readline_in_async_expr(receiver, cm, path, span)?;
            for a in args {
                reject_readline_in_async_expr(a, cm, path, span)?;
            }
            Ok(())
        }
        IRExpr::BuiltinLog { args, .. }
        | IRExpr::MathBuiltin { args, .. }
        | IRExpr::NumberBuiltin { args, .. }
        | IRExpr::JsonBuiltin { args, .. } => {
            for a in args {
                reject_readline_in_async_expr(a, cm, path, span)?;
            }
            Ok(())
        }
        IRExpr::StringMethodBuiltin { receiver, args, .. } => {
            reject_readline_in_async_expr(receiver, cm, path, span)?;
            for a in args {
                reject_readline_in_async_expr(a, cm, path, span)?;
            }
            Ok(())
        }
        IRExpr::Conditional { test, cons, alt, .. } => {
            reject_readline_in_async_expr(test, cm, path, span)?;
            reject_readline_in_async_expr(cons, cm, path, span)?;
            reject_readline_in_async_expr(alt, cm, path, span)
        }
        IRExpr::Seq { exprs, .. } => {
            for x in exprs {
                reject_readline_in_async_expr(x, cm, path, span)?;
            }
            Ok(())
        }
        IRExpr::Tpl { parts, .. } => {
            for p in parts {
                if let TplPart::Interp(x) = p {
                    reject_readline_in_async_expr(x, cm, path, span)?;
                }
            }
            Ok(())
        }
        IRExpr::Member { obj, .. } | IRExpr::OptionalMember { obj, .. } => {
            reject_readline_in_async_expr(obj, cm, path, span)
        }
        IRExpr::NullishCoalesce { left, right, .. } => {
            reject_readline_in_async_expr(left, cm, path, span)?;
            reject_readline_in_async_expr(right, cm, path, span)
        }
        IRExpr::ArrayLit { elems, .. } => {
            for x in elems {
                reject_readline_in_async_expr(x, cm, path, span)?;
            }
            Ok(())
        }
        IRExpr::ObjectLit { fields, .. } => {
            for (_, v) in fields {
                reject_readline_in_async_expr(v, cm, path, span)?;
            }
            Ok(())
        }
        IRExpr::Index { obj, index, .. } => {
            reject_readline_in_async_expr(obj, cm, path, span)?;
            reject_readline_in_async_expr(index, cm, path, span)
        }
        IRExpr::ArrowFn { body, .. } => reject_readline_in_async_stmts(body, cm, path),
        IRExpr::Await { arg, .. } => reject_readline_in_async_expr(arg, cm, path, span),
        IRExpr::FetchText { url, .. } => reject_readline_in_async_expr(url, cm, path, span),
        IRExpr::Number(..)
        | IRExpr::Bool(..)
        | IRExpr::Str(..)
        | IRExpr::Ident(..)
        | IRExpr::Null(..)
        | IRExpr::Undefined(..)
        | IRExpr::This(..)
        | IRExpr::Super(..) => Ok(()),
    }
}

/// 当前语句执行后，同一块内后续语句是否均不可达（用于不可达警告）。
fn stmt_block_diverges(s: &IRStmt, ret_ty: &TsType, loop_depth: usize) -> bool {
    match s {
        IRStmt::Return { .. } => true,
        IRStmt::Break { .. } | IRStmt::Continue { .. } => loop_depth > 0,
        IRStmt::Block { stmts, .. } => stmts_block_seq_diverges(stmts, ret_ty, loop_depth),
        IRStmt::If {
            then_b,
            else_b: Some(else_b),
            ..
        } => {
            stmts_block_seq_diverges(then_b, ret_ty, loop_depth)
                && stmts_block_seq_diverges(else_b, ret_ty, loop_depth)
        }
        IRStmt::If { else_b: None, .. } => false,
        IRStmt::While { .. } | IRStmt::ForIn { .. } | IRStmt::DoWhile { .. } => false,
        IRStmt::Empty { .. }
        | IRStmt::Let { .. }
        | IRStmt::Assign { .. }
        | IRStmt::MemberAssign { .. }
        | IRStmt::Expr { .. }
        | IRStmt::FnDecl { .. } => false,
    }
}

fn stmts_block_seq_diverges(stmts: &[IRStmt], ret_ty: &TsType, loop_depth: usize) -> bool {
    for s in stmts {
        if stmt_block_diverges(s, ret_ty, loop_depth) {
            return true;
        }
    }
    false
}

/// 与旧版 `stmts_return` / `last_returns` 一致：仅看循环体语句列表的「尾部」是否保证带值返回（保持 while/do-while 语义不变）。
fn tail_returns_while_body(stmts: &[IRStmt]) -> bool {
    if stmts.is_empty() {
        return false;
    }
    match &stmts[stmts.len() - 1] {
        IRStmt::Return { arg: Some(_), .. } => true,
        IRStmt::If {
            then_b,
            else_b: Some(else_b),
            ..
        } => tail_returns_while_body(then_b) && tail_returns_while_body(else_b),
        IRStmt::Block { stmts: b, .. } => tail_returns_while_body(b),
        IRStmt::While { body, .. } | IRStmt::ForIn { body, .. } | IRStmt::DoWhile { body, .. } => {
            tail_returns_while_body(body)
        }
        _ => false,
    }
}

/// 该语句是否在非 void 函数中保证「所有路径」均带值返回（用于提前穷尽返回）。
fn stmt_fn_returns_complete(s: &IRStmt, ret: &TsType) -> bool {
    if *ret == TsType::Void {
        return false;
    }
    match s {
        IRStmt::Return { arg: Some(_), .. } => true,
        IRStmt::Return { arg: None, .. } => false,
        IRStmt::If {
            then_b,
            else_b: Some(else_b),
            ..
        } => fn_body_returns(then_b, ret) && fn_body_returns(else_b, ret),
        IRStmt::If { else_b: None, .. } => false,
        IRStmt::Block { stmts, .. } => fn_body_returns(stmts, ret),
        IRStmt::While { body, .. } | IRStmt::ForIn { body, .. } | IRStmt::DoWhile { body, .. } => {
            tail_returns_while_body(body)
        }
        _ => false,
    }
}

/// 函数体是否在所有控制路径上带值返回（非 void）；含序列内「提前穷尽返回」。
fn fn_body_returns(stmts: &[IRStmt], ret: &TsType) -> bool {
    if *ret == TsType::Void {
        return true;
    }
    for s in stmts {
        if stmt_fn_returns_complete(s, ret) {
            return true;
        }
    }
    tail_returns_last_only(stmts)
}

/// 仅检查最后一条语句（与旧 `last_returns` 一致）。
fn tail_returns_last_only(stmts: &[IRStmt]) -> bool {
    if stmts.is_empty() {
        return false;
    }
    match &stmts[stmts.len() - 1] {
        IRStmt::Return { arg: Some(_), .. } => true,
        IRStmt::If {
            then_b,
            else_b: Some(else_b),
            ..
        } => tail_returns_while_body(then_b) && tail_returns_while_body(else_b),
        IRStmt::Block { stmts: b, .. } => tail_returns_last_only(b),
        IRStmt::While { body, .. } | IRStmt::DoWhile { body, .. } => tail_returns_while_body(body),
        _ => false,
    }
}

fn check_stmts(
    stmts: &mut [IRStmt],
    stack: &mut Vec<HashMap<String, Binding>>,
    globals: &HashMap<String, FnSig>,
    ret_ty: &TsType,
    loop_depth: usize,
    cm: &Lrc<SourceMap>,
    path: &str,
    warnings: &mut Vec<CompileWarning>,
    reachable: &mut bool,
) -> Result<(), CompileError> {
    for s in stmts.iter_mut() {
        if !*reachable {
            warnings.push(warn(cm, path, stmt_span(s), "unreachable code"));
        }
        check_stmt(s, stack, globals, ret_ty, loop_depth, cm, path, warnings)?;
        if *reachable && stmt_block_diverges(s, ret_ty, loop_depth) {
            *reachable = false;
        }
    }
    Ok(())
}

fn check_stmt(
    s: &mut IRStmt,
    stack: &mut Vec<HashMap<String, Binding>>,
    globals: &HashMap<String, FnSig>,
    ret_ty: &TsType,
    loop_depth: usize,
    cm: &Lrc<SourceMap>,
    path: &str,
    warnings: &mut Vec<CompileWarning>,
) -> Result<(), CompileError> {
    match s {
        IRStmt::Empty { .. } => Ok(()),
        IRStmt::Let {
            name,
            ty,
            mutable,
            ref mut init,
            span,
        } => {
            if *ty == TsType::Void {
                return Err(diag(
                    cm,
                    path,
                    *span,
                    "`void` cannot be used in `let`/`const` binding",
                ));
            }
            let initialized = if let Some(init_e) = init {
                let got = infer_expr_mut(init_e, stack, globals, cm, path)?;
                if got == TsType::Void {
                    return Err(diag(
                        cm,
                        path,
                        *span,
                        "cannot use `void` expression in initializer",
                    ));
                }
                if !type_assignable(ty, &got) {
                    return Err(diag(
                        cm,
                        path,
                        *span,
                        format!("initializer type mismatch: expected `{ty:?}`, found `{got:?}`"),
                    ));
                }
                true
            } else {
                false
            };
            insert_let(
                stack,
                name,
                ty.clone(),
                *mutable,
                initialized,
                *span,
                cm,
                path,
            )?;
            Ok(())
        }
        IRStmt::Assign {
            name,
            ref mut rhs,
            span,
        } => {
            let b = lookup(stack, name).ok_or_else(|| {
                diag(
                    cm,
                    path,
                    *span,
                    format!("assignment to unknown identifier `{name}`"),
                )
            })?;
            if !b.mutable {
                return Err(diag(
                    cm,
                    path,
                    *span,
                    format!("cannot assign to `const` binding `{name}`"),
                ));
            }
            let got = infer_expr_mut(rhs, stack, globals, cm, path)?;
            if !type_assignable(&b.ty, &got) {
                return Err(diag(
                    cm,
                    path,
                    *span,
                    format!(
                        "assignment type mismatch: expected `{:?}`, found `{got:?}`",
                        b.ty
                    ),
                ));
            }
            mark_initialized(stack, name);
            Ok(())
        }
        IRStmt::MemberAssign {
            obj,
            prop: _,
            rhs,
            span,
        } => {
            let b = lookup(stack, obj).ok_or_else(|| {
                diag(
                    cm,
                    path,
                    *span,
                    format!("assignment to unknown identifier `{obj}`"),
                )
            })?;
            if !b.mutable {
                return Err(diag(
                    cm,
                    path,
                    *span,
                    format!("cannot assign through const binding `{obj}`"),
                ));
            }
            let got = infer_expr_mut(rhs, stack, globals, cm, path)?;
            if !is_numberish(&got) {
                return Err(diag(
                    cm,
                    path,
                    *span,
                    "member assignment currently supports only `number` value",
                ));
            }
            Ok(())
        }
        IRStmt::Expr { ref mut expr, .. } => {
            infer_expr_mut(expr, stack, globals, cm, path)?;
            Ok(())
        }
        IRStmt::Return { ref mut arg, span } => {
            match (ret_ty, arg.as_mut()) {
                (TsType::Void, None) => {}
                (TsType::Void, Some(_)) => {
                    return Err(diag(
                        cm,
                        path,
                        *span,
                        "`void` function cannot return a value",
                    ));
                }
                (_, None) => {
                    return Err(diag(
                        cm,
                        path,
                        *span,
                        "`return` with value required for this function",
                    ));
                }
                (expected, Some(e)) => {
                    let got = infer_expr_mut(e, stack, globals, cm, path)?;
                    if !type_assignable(expected, &got) {
                        return Err(diag(
                            cm,
                            path,
                            *span,
                            format!(
                                "return type mismatch: expected `{expected:?}`, found `{got:?}`"
                            ),
                        ));
                    }
                }
            }
            Ok(())
        }
        IRStmt::Block { stmts, .. } => {
            push_scope(stack);
            let mut inner_reachable = true;
            check_stmts(
                stmts,
                stack,
                globals,
                ret_ty,
                loop_depth,
                cm,
                path,
                warnings,
                &mut inner_reachable,
            )?;
            pop_scope(stack);
            Ok(())
        }
        IRStmt::If {
            ref mut cond,
            cond_ty,
            then_b,
            else_b,
            span,
        } => {
            let ct = infer_expr_mut(cond, stack, globals, cm, path)?;
            if !is_cond_ty(&ct) {
                return Err(diag(
                    cm,
                    path,
                    *span,
                    "`if` condition must be `number` or `boolean` (or a union of one primitive family)",
                ));
            }
            *cond_ty = ct.clone();
            let pre = snapshot_inits(stack);
            push_scope(stack);
            let mut then_r = true;
            check_stmts(
                then_b,
                stack,
                globals,
                ret_ty,
                loop_depth,
                cm,
                path,
                warnings,
                &mut then_r,
            )?;
            pop_scope(stack);
            let then_snap = snapshot_inits(stack);
            restore_inits(stack, &pre);
            if let Some(else_b) = else_b {
                push_scope(stack);
                let mut else_r = true;
                check_stmts(
                    else_b,
                    stack,
                    globals,
                    ret_ty,
                    loop_depth,
                    cm,
                    path,
                    warnings,
                    &mut else_r,
                )?;
                pop_scope(stack);
                let else_snap = snapshot_inits(stack);
                restore_inits(stack, &pre);
                merge_init_after_if_else(stack, &pre, &then_snap, &else_snap);
            } else {
                merge_init_after_if_no_else(stack, &pre, &then_snap);
            }
            Ok(())
        }
        IRStmt::While {
            ref mut cond,
            cond_ty,
            body,
            span,
        } => {
            let ct = infer_expr_mut(cond, stack, globals, cm, path)?;
            if !is_cond_ty(&ct) {
                return Err(diag(
                    cm,
                    path,
                    *span,
                    "`while` condition must be `number` or `boolean` (or a union of one primitive family)",
                ));
            }
            *cond_ty = ct.clone();
            let pre = snapshot_inits(stack);
            push_scope(stack);
            let mut body_r = true;
            check_stmts(
                body,
                stack,
                globals,
                ret_ty,
                loop_depth + 1,
                cm,
                path,
                warnings,
                &mut body_r,
            )?;
            pop_scope(stack);
            apply_loop_conservative_init(stack, &pre);
            Ok(())
        }
        IRStmt::ForIn {
            key,
            key_ty,
            target,
            kind,
            body,
            span,
        } => {
            if !type_assignable(&TsType::String, key_ty) {
                return Err(diag(
                    cm,
                    path,
                    *span,
                    "for..in loop variable type must be `string`",
                ));
            }
            let tt = infer_expr_mut(target, stack, globals, cm, path)?;
            *kind = Some(match tt {
                TsType::ArrayNumber => ForInKind::ArrayIndices,
                TsType::ObjectNum(_) | TsType::ClassInstance(_) => ForInKind::ObjectKeys,
                _ => {
                    return Err(diag(
                        cm,
                        path,
                        *span,
                        "for..in right side must be object/class-instance/number[]",
                    ));
                }
            });
            let pre = snapshot_inits(stack);
            push_scope(stack);
            insert_let(stack, key, key_ty.clone(), true, true, *span, cm, path)?;
            let mut body_r = true;
            check_stmts(
                body,
                stack,
                globals,
                ret_ty,
                loop_depth + 1,
                cm,
                path,
                warnings,
                &mut body_r,
            )?;
            pop_scope(stack);
            apply_loop_conservative_init(stack, &pre);
            Ok(())
        }
        IRStmt::DoWhile {
            ref mut cond,
            cond_ty,
            body,
            span,
        } => {
            let ct = infer_expr_mut(cond, stack, globals, cm, path)?;
            if !is_cond_ty(&ct) {
                return Err(diag(
                    cm,
                    path,
                    *span,
                    "`do-while` condition must be `number` or `boolean` (or a union of one primitive family)",
                ));
            }
            *cond_ty = ct.clone();
            let pre = snapshot_inits(stack);
            push_scope(stack);
            let mut body_r = true;
            check_stmts(
                body,
                stack,
                globals,
                ret_ty,
                loop_depth + 1,
                cm,
                path,
                warnings,
                &mut body_r,
            )?;
            pop_scope(stack);
            apply_loop_conservative_init(stack, &pre);
            Ok(())
        }
        IRStmt::Break { span } => {
            if loop_depth == 0 {
                return Err(diag(cm, path, *span, "`break` is only valid inside a loop"));
            }
            Ok(())
        }
        IRStmt::Continue { span } => {
            if loop_depth == 0 {
                return Err(diag(
                    cm,
                    path,
                    *span,
                    "`continue` is only valid inside a loop",
                ));
            }
            Ok(())
        }
        IRStmt::FnDecl { func, .. } => {
            let c = func.cm.clone();
            let p = func.source_path.clone();
            check_function(func, globals, &c, &p, warnings)?;
            Ok(())
        }
    }
}

fn infer_expr_mut(
    e: &mut IRExpr,
    stack: &[HashMap<String, Binding>],
    globals: &HashMap<String, FnSig>,
    cm: &Lrc<SourceMap>,
    path: &str,
) -> Result<TsType, CompileError> {
    match e {
        IRExpr::Number(n, _) => Ok(TsType::NumberLit(*n)),
        IRExpr::Bool(b, _) => Ok(TsType::BoolLit(*b)),
        IRExpr::Str(s, _) => Ok(TsType::StringLit(s.clone())),
        IRExpr::Null(_) => Ok(TsType::Null),
        IRExpr::Undefined(_) => Ok(TsType::Undefined),
        IRExpr::Ident(name, span) => {
            if let Some(b) = lookup(stack, name) {
                if !b.initialized {
                    return Err(diag(
                        cm,
                        path,
                        *span,
                        "variable may be used before being assigned",
                    ));
                }
                return Ok(b.ty.clone());
            }
            if let Some(sig) = globals.get(name) {
                let ret = if sig.is_async {
                    TsType::Promise(Box::new(sig.ret.clone()))
                } else {
                    sig.ret.clone()
                };
                return Ok(TsType::Fn {
                    params: sig.params.clone(),
                    ret: Box::new(ret),
                });
            }
            Err(diag(
                cm,
                path,
                *span,
                format!("unknown identifier `{name}`"),
            ))
        }
        IRExpr::Unary { op, arg, span } => {
            let t = infer_expr_mut(arg, stack, globals, cm, path)?;
            match op {
                IRUnaryOp::Neg => {
                    if !is_numberish(&t) {
                        return Err(diag(cm, path, *span, "unary `-` expects `number`"));
                    }
                    Ok(TsType::Number)
                }
                IRUnaryOp::Not => {
                    if !is_booleanish(&t) {
                        return Err(diag(cm, path, *span, "unary `!` expects `boolean`"));
                    }
                    Ok(TsType::Boolean)
                }
            }
        }
        IRExpr::Binary {
            op,
            left,
            right,
            span,
            kind,
        } => {
            let lt = infer_expr_mut(left, stack, globals, cm, path)?;
            let rt = infer_expr_mut(right, stack, globals, cm, path)?;
            use IRBinOp::*;
            match op {
                Add | Sub | Mul | Div => {
                    if is_numberish(&lt) && is_numberish(&rt) {
                        *kind = Some(BinaryKind::Int);
                        Ok(TsType::Number)
                    } else if *op == Add && is_stringish(&lt) && is_stringish(&rt) {
                        *kind = Some(BinaryKind::StrConcat);
                        Ok(TsType::String)
                    } else {
                        Err(diag(
                            cm,
                            path,
                            *span,
                            "binary arithmetic expects two `number`s, or `+` with two `string`s",
                        ))
                    }
                }
                Eq | Ne | Lt | Le | Gt | Ge => {
                    if is_numberish(&lt) && is_numberish(&rt) {
                        Ok(TsType::Boolean)
                    } else if is_booleanish(&lt) && is_booleanish(&rt) && matches!(op, Eq | Ne) {
                        Ok(TsType::Boolean)
                    } else if is_stringish(&lt) && is_stringish(&rt) && matches!(op, Eq | Ne) {
                        Ok(TsType::Boolean)
                    } else {
                        Err(diag(
                            cm,
                            path,
                            *span,
                            "comparison operands must be compatible",
                        ))
                    }
                }
                LogicalAnd | LogicalOr => {
                    let lhs_n = is_numberish(&lt);
                    let rhs_n = is_numberish(&rt);
                    let lhs_b = is_booleanish(&lt);
                    let rhs_b = is_booleanish(&rt);
                    if (lhs_b || lhs_n) && (rhs_b || rhs_n) {
                        *kind = Some(BinaryKind::Logical {
                            lhs_number_truthy: lhs_n,
                            rhs_number_truthy: rhs_n,
                        });
                        Ok(TsType::Boolean)
                    } else {
                        Err(diag(
                            cm,
                            path,
                            *span,
                            "`&&` / `||` operands must be `boolean` and/or `number` (same rules as condition positions)",
                        ))
                    }
                }
            }
        }
        IRExpr::Call {
            callee,
            args,
            type_args,
            span,
        } => {
            let from_global = globals.get(callee).map(|sig| {
                (
                    sig.params.clone(),
                    sig.ret.clone(),
                    sig.type_params.clone(),
                    true,
                    sig.is_async,
                )
            });
            let from_local = lookup(stack, callee).and_then(|b| match &b.ty {
                TsType::Fn { params, ret } => Some((
                    params.clone(),
                    (**ret).clone(),
                    Vec::<String>::new(),
                    false,
                    false,
                )),
                _ => None,
            });
            let Some((sig_params, sig_ret, sig_type_params, allow_type_args, callee_is_async)) =
                from_global.or(from_local)
            else {
                return Err(diag(
                    cm,
                    path,
                    *span,
                    format!("call to unknown function `{callee}`"),
                ));
            };
            if sig_params.len() != args.len() {
                return Err(diag(
                    cm,
                    path,
                    *span,
                    format!(
                        "wrong argument count for `{callee}`: expected {}, got {}",
                        sig_params.len(),
                        args.len()
                    ),
                ));
            }
            if !allow_type_args || sig_type_params.is_empty() {
                if !type_args.is_empty() {
                    return Err(diag(
                        cm,
                        path,
                        *span,
                        format!("non-generic function `{callee}` does not accept type arguments"),
                    ));
                }
            } else if sig_type_params.len() != type_args.len() {
                return Err(diag(
                    cm,
                    path,
                    *span,
                    format!(
                        "wrong type argument count for `{callee}`: expected {}, got {}",
                        sig_type_params.len(),
                        type_args.len()
                    ),
                ));
            }
            let mut subst = HashMap::new();
            for (n, t) in sig_type_params.iter().zip(type_args.iter()) {
                subst.insert(n.clone(), t.clone());
            }
            for (a, pt) in args.iter_mut().zip(sig_params.iter()) {
                let at = infer_expr_mut(a, stack, globals, cm, path)?;
                let expected = subst_type_local(pt, &subst);
                if !type_assignable(&expected, &at) {
                    return Err(diag(
                        cm,
                        path,
                        *span,
                        format!("argument type mismatch for `{callee}`"),
                    ));
                }
            }
            let ret = subst_type_local(&sig_ret, &subst);
            if callee_is_async {
                Ok(TsType::Promise(Box::new(ret)))
            } else {
                Ok(ret)
            }
        }
        IRExpr::MethodCall {
            receiver,
            method,
            args,
            type_args: _,
            span,
        } => {
            let sig = globals.get(method).ok_or_else(|| {
                diag(
                    cm,
                    path,
                    *span,
                    format!("call to unknown function `{method}`"),
                )
            })?;
            let expected_arity = 1 + args.len();
            if sig.params.len() != expected_arity {
                return Err(diag(
                    cm,
                    path,
                    *span,
                    format!(
                        "wrong argument count for `{method}` in `obj.m(...)` form: global function must have {} parameter(s) (receiver first), signature has {}",
                        expected_arity,
                        sig.params.len()
                    ),
                ));
            }
            let rt = infer_expr_mut(receiver, stack, globals, cm, path)?;
            if !type_assignable(&sig.params[0], &rt) {
                return Err(diag(
                    cm,
                    path,
                    *span,
                    format!("receiver type mismatch for `{method}`"),
                ));
            }
            for (a, pt) in args.iter_mut().zip(sig.params.iter().skip(1)) {
                let at = infer_expr_mut(a, stack, globals, cm, path)?;
                if !type_assignable(pt, &at) {
                    return Err(diag(
                        cm,
                        path,
                        *span,
                        format!("argument type mismatch for `{method}`"),
                    ));
                }
            }
            Ok(sig.ret.clone())
        }
        IRExpr::BuiltinLog { args, .. } => {
            for a in args.iter_mut() {
                infer_expr_mut(a, stack, globals, cm, path)?;
            }
            Ok(TsType::Void)
        }
        IRExpr::Conditional {
            test,
            cons,
            alt,
            span,
            cond_ty,
        } => {
            let ct = infer_expr_mut(test, stack, globals, cm, path)?;
            if !is_cond_ty(&ct) {
                return Err(diag(
                    cm,
                    path,
                    *span,
                    "ternary condition must be `number` or `boolean` (or a union of one primitive family)",
                ));
            }
            *cond_ty = Some(ct.clone());
            let tt = infer_expr_mut(cons, stack, globals, cm, path)?;
            let at = infer_expr_mut(alt, stack, globals, cm, path)?;
            unify_ternary_branches(tt, at, cm, path, *span)
        }
        IRExpr::Seq { exprs, span } => {
            if exprs.is_empty() {
                return Err(diag(cm, path, *span, "empty sequence expression"));
            }
            let n = exprs.len();
            for e in exprs.iter_mut().take(n - 1) {
                infer_expr_mut(e, stack, globals, cm, path)?;
            }
            infer_expr_mut(&mut exprs[n - 1], stack, globals, cm, path)
        }
        IRExpr::Tpl { parts, span } => {
            for p in parts {
                match p {
                    TplPart::Static(_) => {}
                    TplPart::Interp(e) => {
                        let t = infer_expr_mut(e, stack, globals, cm, path)?;
                        if matches!(t, TsType::Void) {
                            return Err(diag(
                                cm,
                                path,
                                *span,
                                "cannot interpolate `void` expression in template literal",
                            ));
                        }
                    }
                }
            }
            Ok(TsType::String)
        }
        IRExpr::Member {
            obj,
            prop,
            span,
            length_dispatch,
        } => {
            let ot = infer_expr_mut(obj, stack, globals, cm, path)?;
            if prop == "length" {
                if is_stringish(&ot) {
                    *length_dispatch = Some(MemberLengthDispatch::JsStringUtf16);
                    return Ok(TsType::Number);
                }
                if ot == TsType::ArrayNumber {
                    *length_dispatch = Some(MemberLengthDispatch::VecLen);
                    return Ok(TsType::Number);
                }
                if let TsType::ObjectNum(keys) = &ot {
                    if keys.iter().any(|k| k == prop) {
                        *length_dispatch = None;
                        return Ok(TsType::Number);
                    }
                }
            } else if let TsType::ObjectNum(keys) = &ot {
                if keys.iter().any(|k| k == prop) {
                    return Ok(TsType::Number);
                }
            }
            Err(diag(
                cm,
                path,
                *span,
                "unsupported member access (supports `string.length`, `number[].length`, or object number fields)",
            ))
        }
        IRExpr::OptionalMember {
            obj,
            prop,
            span,
            length_dispatch,
        } => {
            match &**obj {
                IRExpr::Null(_) | IRExpr::Undefined(_) => return Ok(TsType::Undefined),
                _ => {}
            }
            let ot = infer_expr_mut(obj, stack, globals, cm, path)?;
            if ot == TsType::Null || ot == TsType::Undefined {
                return Ok(TsType::Undefined);
            }
            if prop == "length" {
                if is_stringish(&ot) {
                    *length_dispatch = Some(MemberLengthDispatch::JsStringUtf16);
                    return Ok(TsType::Number);
                }
                if ot == TsType::ArrayNumber {
                    *length_dispatch = Some(MemberLengthDispatch::VecLen);
                    return Ok(TsType::Number);
                }
                if let TsType::ObjectNum(keys) = &ot {
                    if keys.iter().any(|k| k == prop) {
                        *length_dispatch = None;
                        return Ok(TsType::Number);
                    }
                }
            } else if let TsType::ObjectNum(keys) = &ot {
                if keys.iter().any(|k| k == prop) {
                    return Ok(TsType::Number);
                }
            }
            Err(diag(cm, path, *span, "unsupported optional member access"))
        }
        IRExpr::MathBuiltin { kind, args, span } => {
            for a in args.iter_mut() {
                let t = infer_expr_mut(a, stack, globals, cm, path)?;
                if !is_numberish(&t) {
                    return Err(diag(
                        cm,
                        path,
                        *span,
                        "Math builtin arguments must be `number`",
                    ));
                }
            }
            if *kind == MathBuiltinKind::Pow {
                if let Some(e2) = args.get(1) {
                    if let IRExpr::Number(n, _) = e2 {
                        if *n < 0 {
                            return Err(diag(
                                cm,
                                path,
                                *span,
                                "Math.pow: exponent literal must be non-negative",
                            ));
                        }
                    }
                }
            }
            Ok(TsType::Number)
        }
        IRExpr::NumberBuiltin { kind, args, span } => {
            match kind {
                NumberBuiltinKind::ParseInt => {
                    if args.is_empty() || args.len() > 2 {
                        return Err(diag(
                            cm,
                            path,
                            *span,
                            "internal: Number.parseInt arity",
                        ));
                    }
                    let t0 = infer_expr_mut(&mut args[0], stack, globals, cm, path)?;
                    if !is_stringish(&t0) {
                        return Err(diag(
                            cm,
                            path,
                            *span,
                            "`Number.parseInt` first argument must be `string`",
                        ));
                    }
                    if args.len() == 2 {
                        let t1 = infer_expr_mut(&mut args[1], stack, globals, cm, path)?;
                        if !is_numberish(&t1) {
                            return Err(diag(
                                cm,
                                path,
                                *span,
                                "`Number.parseInt` radix must be `number`",
                            ));
                        }
                        if let IRExpr::Number(r, _) = &args[1] {
                            if *r < 2 || *r > 36 {
                                return Err(diag(
                                    cm,
                                    path,
                                    *span,
                                    "`Number.parseInt` radix must be between 2 and 36",
                                ));
                            }
                        }
                    }
                    Ok(TsType::Number)
                }
                NumberBuiltinKind::ParseFloat => {
                    let t0 = infer_expr_mut(&mut args[0], stack, globals, cm, path)?;
                    if !is_stringish(&t0) {
                        return Err(diag(
                            cm,
                            path,
                            *span,
                            "`Number.parseFloat` argument must be `string`",
                        ));
                    }
                    Ok(TsType::Number)
                }
            }
        }
        IRExpr::JsonBuiltin {
            kind,
            args,
            span,
            stringify_inferred_ty,
        } => {
            match kind {
                JsonBuiltinKind::Stringify => {
                    let t = infer_expr_mut(&mut args[0], stack, globals, cm, path)?;
                    if !is_stringish(&t) && !is_numberish(&t) && !is_booleanish(&t) {
                        return Err(diag(
                            cm,
                            path,
                            *span,
                            "`JSON.stringify` supports only `string`, `number`, or `boolean`",
                        ));
                    }
                    *stringify_inferred_ty = Some(t.clone());
                    Ok(TsType::String)
                }
                JsonBuiltinKind::Parse => {
                    let t = infer_expr_mut(&mut args[0], stack, globals, cm, path)?;
                    if !is_stringish(&t) {
                        return Err(diag(
                            cm,
                            path,
                            *span,
                            "`JSON.parse` argument must be `string`",
                        ));
                    }
                    Ok(TsType::Number)
                }
            }
        }
        IRExpr::StringMethodBuiltin {
            kind,
            receiver,
            args,
            span,
        } => {
            let rt = infer_expr_mut(receiver, stack, globals, cm, path)?;
            if !is_stringish(&rt) {
                return Err(diag(
                    cm,
                    path,
                    *span,
                    "string builtin method receiver must be `string`",
                ));
            }
            match kind {
                StringMethodKind::CharAt => {
                    if args.len() != 1 {
                        return Err(diag(
                            cm,
                            path,
                            *span,
                            "internal: charAt arity",
                        ));
                    }
                    let at = infer_expr_mut(&mut args[0], stack, globals, cm, path)?;
                    if !is_numberish(&at) {
                        return Err(diag(
                            cm,
                            path,
                            *span,
                            "argument must be `number`",
                        ));
                    }
                    Ok(TsType::String)
                }
                StringMethodKind::CharCodeAt => {
                    if args.len() != 1 {
                        return Err(diag(
                            cm,
                            path,
                            *span,
                            "internal: charCodeAt arity",
                        ));
                    }
                    let at = infer_expr_mut(&mut args[0], stack, globals, cm, path)?;
                    if !is_numberish(&at) {
                        return Err(diag(
                            cm,
                            path,
                            *span,
                            "argument must be `number`",
                        ));
                    }
                    Ok(TsType::Number)
                }
                StringMethodKind::Slice | StringMethodKind::Substring => {
                    for a in args.iter_mut() {
                        let at = infer_expr_mut(a, stack, globals, cm, path)?;
                        if !is_numberish(&at) {
                            return Err(diag(
                                cm,
                                path,
                                *span,
                                "`slice` / `substring` arguments must be `number`",
                            ));
                        }
                    }
                    Ok(TsType::String)
                }
                StringMethodKind::IndexOf | StringMethodKind::Includes => {
                    let st = infer_expr_mut(&mut args[0], stack, globals, cm, path)?;
                    if !is_stringish(&st) {
                        return Err(diag(
                            cm,
                            path,
                            *span,
                            "`indexOf` / `includes` search string must be `string`",
                        ));
                    }
                    if args.len() == 2 {
                        let at = infer_expr_mut(&mut args[1], stack, globals, cm, path)?;
                        if !is_numberish(&at) {
                            return Err(diag(
                                cm,
                                path,
                                *span,
                                "`indexOf` / `includes` position must be `number`",
                            ));
                        }
                    }
                    Ok(match *kind {
                        StringMethodKind::IndexOf => TsType::Number,
                        StringMethodKind::Includes => TsType::Boolean,
                        _ => unreachable!(),
                    })
                }
            }
        }
        IRExpr::ReadStdinLine { .. } => Ok(TsType::String),
        IRExpr::NullishCoalesce { left, right, span } => {
            let lt = infer_expr_mut(left, stack, globals, cm, path)?;
            let rt = infer_expr_mut(right, stack, globals, cm, path)?;
            if lt == TsType::Null || lt == TsType::Undefined {
                // 在受限子集中，左侧静态可判空值时直接折叠为右侧，避免 codegen 生成不匹配类型。
                *e = (**right).clone();
                return Ok(rt);
            }
            if lt == rt {
                return Ok(lt);
            }
            match &**left {
                IRExpr::Null(_) | IRExpr::Undefined(_) => Ok(rt),
                IRExpr::OptionalMember { .. } if is_numberish(&rt) => Ok(TsType::Number),
                _ => Err(diag(
                    cm,
                    path,
                    *span,
                    "nullish coalescing currently requires nullable-left or same-type operands",
                )),
            }
        }
        IRExpr::ArrayLit { elems, span } => {
            for e in elems {
                let t = infer_expr_mut(e, stack, globals, cm, path)?;
                if !is_numberish(&t) {
                    return Err(diag(
                        cm,
                        path,
                        *span,
                        "array literal currently supports only `number` elements",
                    ));
                }
            }
            Ok(TsType::ArrayNumber)
        }
        IRExpr::ObjectLit { fields, span } => {
            let mut keys = Vec::with_capacity(fields.len());
            for (k, v) in fields {
                let t = infer_expr_mut(v, stack, globals, cm, path)?;
                if !is_numberish(&t) {
                    return Err(diag(
                        cm,
                        path,
                        *span,
                        "object literal currently supports only `number` field values",
                    ));
                }
                keys.push(k.clone());
            }
            keys.sort();
            for w in keys.windows(2) {
                if w[0] == w[1] {
                    return Err(diag(
                        cm,
                        path,
                        *span,
                        format!("duplicate object field `{}`", w[0]),
                    ));
                }
            }
            Ok(TsType::ObjectNum(keys))
        }
        IRExpr::ArrowFn {
            params,
            ret,
            body,
            span,
        } => {
            let mut local_stack = stack.to_vec();
            let mut local_warnings = Vec::new();
            push_scope(&mut local_stack);
            for (name, ty) in params.iter() {
                insert_let(
                    &mut local_stack,
                    name,
                    ty.clone(),
                    false,
                    true,
                    *span,
                    cm,
                    path,
                )?;
            }
            let mut reaches_end = true;
            check_stmts(
                body,
                &mut local_stack,
                globals,
                ret,
                0,
                cm,
                path,
                &mut local_warnings,
                &mut reaches_end,
            )?;
            if *ret != TsType::Void && reaches_end {
                return Err(diag(
                    cm,
                    path,
                    *span,
                    "arrow function must return on all control-flow paths",
                ));
            }
            Ok(TsType::Fn {
                params: params.iter().map(|(_, t)| t.clone()).collect(),
                ret: Box::new(ret.clone()),
            })
        }
        IRExpr::This(span) => Err(diag(
            cm,
            path,
            *span,
            "`this` is only valid inside class method lowering",
        )),
        IRExpr::Super(span) => Err(diag(
            cm,
            path,
            *span,
            "`super` is only valid inside subclass constructor lowering",
        )),
        IRExpr::Index {
            obj,
            index,
            span,
            index_kind,
        } => {
            let ot = infer_expr_mut(obj, stack, globals, cm, path)?;
            let it = infer_expr_mut(index, stack, globals, cm, path)?;
            if !is_numberish(&it) {
                return Err(diag(cm, path, *span, "index must be `number`"));
            }
            if ot == TsType::ArrayNumber {
                *index_kind = Some(IndexKind::ArrayNumber);
                Ok(TsType::Number)
            } else if is_stringish(&ot) {
                *index_kind = Some(IndexKind::StringUtf16);
                Ok(TsType::String)
            } else {
                Err(diag(
                    cm,
                    path,
                    *span,
                    "index access supports only `number[]` or `string`",
                ))
            }
        }
        IRExpr::FetchText { url, span } => {
            let t = infer_expr_mut(url, stack, globals, cm, path)?;
            if !is_stringish(&t) {
                return Err(diag(
                    cm,
                    path,
                    *span,
                    "`fetchText` argument must be `string`",
                ));
            }
            Ok(TsType::Promise(Box::new(TsType::String)))
        }
        IRExpr::Await { arg, span } => {
            match &**arg {
                IRExpr::FetchText { .. } | IRExpr::Call { .. } => {}
                _ => {
                    return Err(diag(
                        cm,
                        path,
                        *span,
                        "async MVP: `await` only supports `fetchText(...)` or an async function call",
                    ));
                }
            }
            let inner = infer_expr_mut(arg, stack, globals, cm, path)?;
            match inner {
                TsType::Promise(t) => Ok(*t),
                _ => Err(diag(
                    cm,
                    path,
                    *span,
                    "`await` expects a `Promise<T>` value",
                )),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::build::build_module;
    use ts2rs_parser::parse_typescript_file;

    #[test]
    fn check_module_accepts_simple_main() {
        let src = r#"function main(): number { return 0; }"#;
        let p = parse_typescript_file("t.ts", src).unwrap();
        let mut m = build_module(&p.program, &p.source_map, "t.ts").unwrap();
        let w = check_module(&mut m).unwrap();
        assert!(w.is_empty());
    }

    #[test]
    fn check_module_rejects_missing_return() {
        let src = r#"function main(): number { let x: number = 1; }"#;
        let p = parse_typescript_file("t.ts", src).unwrap();
        let mut m = build_module(&p.program, &p.source_map, "t.ts").unwrap();
        let e = check_module(&mut m).unwrap_err();
        let s = e.to_string();
        assert!(
            s.contains("not all control paths return"),
            "unexpected diagnostic: {s}"
        );
    }
}
