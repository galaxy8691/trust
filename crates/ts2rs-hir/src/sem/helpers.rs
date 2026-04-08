use std::path::Path;

use swc_common::Span;

use crate::error::{diag, CompileError};
use crate::ir::{SendSourceMap, TsType};

pub(super) fn is_numberish(t: &TsType) -> bool {
    match t {
        TsType::Number | TsType::NumberLit(_) => true,
        TsType::Union(m) => m.iter().all(is_numberish),
        _ => false,
    }
}

pub(super) fn is_booleanish(t: &TsType) -> bool {
    match t {
        TsType::Boolean | TsType::BoolLit(_) => true,
        TsType::Union(m) => m.iter().all(is_booleanish),
        _ => false,
    }
}

pub(super) fn is_stringish(t: &TsType) -> bool {
    match t {
        TsType::String | TsType::StringLit(_) => true,
        TsType::Union(m) => m.iter().all(is_stringish),
        _ => false,
    }
}

/// `if` / `while` / 三元条件：`number`、`boolean`，或**成员全为同一族**的联合（如 `1 | 2`、`true | false`），不含 `number | boolean` 等混合。
pub(super) fn is_cond_ty(t: &TsType) -> bool {
    match t {
        TsType::Union(m) => m.iter().all(is_numberish) || m.iter().all(is_booleanish),
        _ => is_numberish(t) || is_booleanish(t),
    }
}

/// `got` 是否可赋给注解类型 `expected`（含字面量向 `number`/`boolean`/`string` 拓宽）。
pub(super) fn type_assignable(expected: &TsType, got: &TsType) -> bool {
    if expected == got {
        return true;
    }
    if let TsType::Union(members) = expected {
        return members.iter().any(|e| type_assignable(e, got));
    }
    if let TsType::Union(members) = got {
        return members.iter().all(|g| type_assignable(expected, g));
    }
    if let (
        TsType::Fn {
            params: ep,
            ret: er,
        },
        TsType::Fn {
            params: gp,
            ret: gr,
        },
    ) = (expected, got)
    {
        if ep.len() != gp.len() {
            return false;
        }
        if ep
            .iter()
            .zip(gp.iter())
            .any(|(e, g)| !type_assignable(e, g))
        {
            return false;
        }
        return type_assignable(er, gr);
    }
    if let (TsType::Promise(e), TsType::Promise(g)) = (expected, got) {
        return type_assignable(e, g);
    }
    if let (TsType::ClassInstance(a), TsType::ClassInstance(b)) = (expected, got) {
        return a == b;
    }
    if let (TsType::ObjectNum(exp), TsType::ObjectNum(got_keys)) = (expected, got) {
        return got_keys.iter().all(|k| exp.iter().any(|e| e == k));
    }
    matches!(
        (expected, got),
        (TsType::Number, TsType::NumberLit(_))
            | (TsType::Boolean, TsType::BoolLit(_))
            | (TsType::String, TsType::StringLit(_))
    )
}

pub(super) fn unify_ternary_branches(
    a: TsType,
    b: TsType,
    cm: &SendSourceMap,
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

pub(super) fn paths_equal_file(a: &str, b: &str) -> bool {
    let pa = Path::new(a);
    let pb = Path::new(b);
    let ca = pa.canonicalize().unwrap_or_else(|_| pa.to_path_buf());
    let cb = pb.canonicalize().unwrap_or_else(|_| pb.to_path_buf());
    ca == cb
}
