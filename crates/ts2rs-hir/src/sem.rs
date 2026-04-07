//! 名字解析、类型检查、简化 return 路径检查。

use std::collections::HashMap;
use std::path::Path;

use swc_common::{sync::Lrc, SourceMap, Span, DUMMY_SP};

use crate::error::{diag, CompileError};
use crate::ir::*;

#[derive(Clone)]
struct FnSig {
    params: Vec<TsType>,
    ret: TsType,
}

#[derive(Clone)]
struct Binding {
    ty: TsType,
    mutable: bool,
}

fn is_numberish(t: &TsType) -> bool {
    match t {
        TsType::Number | TsType::NumberLit(_) => true,
        TsType::Union(m) => m.iter().all(is_numberish),
        _ => false,
    }
}

fn is_booleanish(t: &TsType) -> bool {
    match t {
        TsType::Boolean | TsType::BoolLit(_) => true,
        TsType::Union(m) => m.iter().all(is_booleanish),
        _ => false,
    }
}

fn is_stringish(t: &TsType) -> bool {
    match t {
        TsType::String | TsType::StringLit(_) => true,
        TsType::Union(m) => m.iter().all(is_stringish),
        _ => false,
    }
}

/// `if` / `while` / 三元条件：`number`、`boolean`，或**成员全为同一族**的联合（如 `1 | 2`、`true | false`），不含 `number | boolean` 等混合。
fn is_cond_ty(t: &TsType) -> bool {
    match t {
        TsType::Union(m) => m.iter().all(is_numberish) || m.iter().all(is_booleanish),
        _ => is_numberish(t) || is_booleanish(t),
    }
}

/// `got` 是否可赋给注解类型 `expected`（含字面量向 `number`/`boolean`/`string` 拓宽）。
fn type_assignable(expected: &TsType, got: &TsType) -> bool {
    if expected == got {
        return true;
    }
    if let TsType::Union(members) = expected {
        return members.iter().any(|e| type_assignable(e, got));
    }
    if let TsType::Union(members) = got {
        return members.iter().all(|g| type_assignable(expected, g));
    }
    matches!(
        (expected, got),
        (TsType::Number, TsType::NumberLit(_))
            | (TsType::Boolean, TsType::BoolLit(_))
            | (TsType::String, TsType::StringLit(_))
    )
}

fn unify_ternary_branches(
    a: TsType,
    b: TsType,
    cm: &Lrc<SourceMap>,
    path: &str,
    span: Span,
) -> Result<TsType, CompileError> {
    if a == b {
        return Ok(a);
    }
    if is_numberish(&a) && is_numberish(&b) {
        return Ok(TsType::Number);
    }
    if is_booleanish(&a) && is_booleanish(&b) {
        return Ok(TsType::Boolean);
    }
    if is_stringish(&a) && is_stringish(&b) {
        return Ok(TsType::String);
    }
    Err(diag(
        cm,
        path,
        span,
        "ternary `?:` branches must have compatible types",
    ))
}

fn paths_equal_file(a: &str, b: &str) -> bool {
    let pa = Path::new(a);
    let pb = Path::new(b);
    let ca = pa.canonicalize().unwrap_or_else(|_| pa.to_path_buf());
    let cb = pb.canonicalize().unwrap_or_else(|_| pb.to_path_buf());
    ca == cb
}

pub fn check_module(module: &mut IRModule) -> Result<(), CompileError> {
    let mut globals: HashMap<String, FnSig> = HashMap::new();
    for f in &module.fns {
        if globals
            .insert(
                f.name.clone(),
                FnSig {
                    params: f.params.iter().map(|(_, t)| t.clone()).collect(),
                    ret: f.ret.clone(),
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

    let main_fn = module.fns.iter().find(|f| f.name == "main").ok_or_else(|| {
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

    for f in &mut module.fns {
        let cm = f.cm.clone();
        let p = f.source_path.clone();
        check_function(f, &globals, &cm, &p)?;
    }
    Ok(())
}

fn collect_fn_sigs_in_stmts(stmts: &[IRStmt]) -> HashMap<String, FnSig> {
    let mut m = HashMap::new();
    for s in stmts {
        if let IRStmt::FnDecl { func, .. } = s {
            if m
                .insert(
                    func.name.clone(),
                    FnSig {
                        params: func.params.iter().map(|(_, t)| t.clone()).collect(),
                        ret: func.ret.clone(),
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
                },
            )
            .is_some()
        {
            return Err(diag(cm, path, f.span, format!("duplicate parameter `{n}`")));
        }
    }
    stack.push(root);

    check_stmts(
        &mut f.body,
        &mut stack,
        &merged,
        &f.ret,
        0,
        cm,
        path,
    )?;

    if f.ret != TsType::Void && !stmts_return(&f.body) {
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
    span: Span,
    cm: &Lrc<SourceMap>,
    path: &str,
) -> Result<(), CompileError> {
    if stack
        .last_mut()
        .unwrap()
        .insert(
            name.to_string(),
            Binding { ty, mutable },
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

fn check_stmts(
    stmts: &mut [IRStmt],
    stack: &mut Vec<HashMap<String, Binding>>,
    globals: &HashMap<String, FnSig>,
    ret_ty: &TsType,
    loop_depth: usize,
    cm: &Lrc<SourceMap>,
    path: &str,
) -> Result<(), CompileError> {
    for s in stmts.iter_mut() {
        check_stmt(
            s,
            stack,
            globals,
            ret_ty,
            loop_depth,
            cm,
            path,
        )?;
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
            let got = infer_expr_mut(init, stack, globals, cm, path)?;
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
            insert_let(stack, name, ty.clone(), *mutable, *span, cm, path)?;
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
                    format!("assignment type mismatch: expected `{:?}`, found `{got:?}`", b.ty),
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
                            format!("return type mismatch: expected `{expected:?}`, found `{got:?}`"),
                        ));
                    }
                }
            }
            Ok(())
        }
        IRStmt::Block { stmts, .. } => {
            push_scope(stack);
            check_stmts(stmts, stack, globals, ret_ty, loop_depth, cm, path)?;
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
            push_scope(stack);
            check_stmts(then_b, stack, globals, ret_ty, loop_depth, cm, path)?;
            pop_scope(stack);
            if let Some(else_b) = else_b {
                push_scope(stack);
                check_stmts(else_b, stack, globals, ret_ty, loop_depth, cm, path)?;
                pop_scope(stack);
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
            push_scope(stack);
            check_stmts(
                body,
                stack,
                globals,
                ret_ty,
                loop_depth + 1,
                cm,
                path,
            )?;
            pop_scope(stack);
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
            push_scope(stack);
            check_stmts(
                body,
                stack,
                globals,
                ret_ty,
                loop_depth + 1,
                cm,
                path,
            )?;
            pop_scope(stack);
            Ok(())
        }
        IRStmt::Break { span } => {
            if loop_depth == 0 {
                return Err(diag(
                    cm,
                    path,
                    *span,
                    "`break` is only valid inside a loop",
                ));
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
            check_function(func, globals, &c, &p)?;
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
                return Ok(b.ty.clone());
            }
            if globals.contains_key(name) {
                return Err(diag(
                    cm,
                    path,
                    *span,
                    format!("`{name}` is a function; did you mean to call it?"),
                ));
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
        IRExpr::Call { callee, args, span } => {
            let sig = globals.get(callee).ok_or_else(|| {
                diag(
                    cm,
                    path,
                    *span,
                    format!("call to unknown function `{callee}`"),
                )
            })?;
            if sig.params.len() != args.len() {
                return Err(diag(
                    cm,
                    path,
                    *span,
                    format!(
                        "wrong argument count for `{callee}`: expected {}, got {}",
                        sig.params.len(),
                        args.len()
                    ),
                ));
            }
            for (a, pt) in args.iter_mut().zip(sig.params.iter()) {
                let at = infer_expr_mut(a, stack, globals, cm, path)?;
                if !type_assignable(pt, &at) {
                    return Err(diag(
                        cm,
                        path,
                        *span,
                        format!("argument type mismatch for `{callee}`"),
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
                return Err(diag(
                    cm,
                    path,
                    *span,
                    "empty sequence expression",
                ));
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
        IRExpr::Member { obj, prop, span } => {
            let ot = infer_expr_mut(obj, stack, globals, cm, path)?;
            if prop == "length" && is_stringish(&ot) {
                return Ok(TsType::Number);
            }
            if let TsType::ObjectNum(keys) = &ot {
                if keys.iter().any(|k| k == prop) {
                    return Ok(TsType::Number);
                }
            }
            Err(diag(
                cm,
                path,
                *span,
                "unsupported member access (supports `string.length` or object number fields)",
            ))
        }
        IRExpr::OptionalMember { obj, prop, span } => {
            match &**obj {
                IRExpr::Null(_) | IRExpr::Undefined(_) => return Ok(TsType::Undefined),
                _ => {}
            }
            let ot = infer_expr_mut(obj, stack, globals, cm, path)?;
            if ot == TsType::Null || ot == TsType::Undefined {
                return Ok(TsType::Undefined);
            }
            if prop == "length" && is_stringish(&ot) {
                return Ok(TsType::Number);
            }
            if let TsType::ObjectNum(keys) = &ot {
                if keys.iter().any(|k| k == prop) {
                    return Ok(TsType::Number);
                }
            }
            Err(diag(
                cm,
                path,
                *span,
                "unsupported optional member access",
            ))
        }
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
        IRExpr::Index { obj, index, span } => {
            let ot = infer_expr_mut(obj, stack, globals, cm, path)?;
            let it = infer_expr_mut(index, stack, globals, cm, path)?;
            if !is_numberish(&it) {
                return Err(diag(cm, path, *span, "array index must be `number`"));
            }
            if ot == TsType::ArrayNumber {
                Ok(TsType::Number)
            } else {
                Err(diag(
                    cm,
                    path,
                    *span,
                    "index access currently supports only `number[]`",
                ))
            }
        }
    }
}

fn last_returns(stmts: &[IRStmt]) -> bool {
    if stmts.is_empty() {
        return false;
    }
    match &stmts[stmts.len() - 1] {
        IRStmt::Return { arg: Some(_), .. } => true,
        IRStmt::If {
            then_b,
            else_b: Some(else_b),
            ..
        } => stmts_return(then_b) && stmts_return(else_b),
        IRStmt::Block { stmts: b, .. } => stmts_return(b),
        IRStmt::While { body, .. } | IRStmt::DoWhile { body, .. } => stmts_return(body),
        _ => false,
    }
}

fn stmts_return(stmts: &[IRStmt]) -> bool {
    if stmts.is_empty() {
        return false;
    }
    last_returns(stmts)
}
