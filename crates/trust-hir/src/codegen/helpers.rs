use swc_common::Span;

use crate::error::{diag, CompileError};
use crate::ir::{IRFunction, IRStmt, TsType};

/// 生成可嵌入 Rust 源码的 `f64` 字面量（`number` → `f64`）。
pub(super) fn rust_f64_literal(n: f64) -> String {
    if n.is_nan() {
        return "f64::NAN".to_string();
    }
    if n.is_infinite() {
        return if n.is_sign_positive() {
            "f64::INFINITY".to_string()
        } else {
            "f64::NEG_INFINITY".to_string()
        };
    }
    let s = n.to_string();
    format!("{}_f64", s)
}

pub(super) fn rust_fn_name(name: &str) -> &str {
    if name == "main" {
        "ts_main"
    } else {
        name
    }
}

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

pub(super) fn rust_ty_scalar(t: &TsType) -> &'static str {
    match t {
        TsType::Number | TsType::NumberLit(_) => "f64",
        TsType::Boolean | TsType::BoolLit(_) => "bool",
        TsType::String | TsType::StringLit(_) => "String",
        TsType::Void => "()",
        TsType::Null => "()",
        TsType::Undefined => "()",
        TsType::ArrayNumber => "Vec<f64>",
        TsType::ArrayString => "Vec<String>",
        TsType::ArrayHttpResponse => "Vec<reqwest::Response>",
        TsType::HttpResponse => "reqwest::Response",
        TsType::ReadableStream => "()",
        TsType::ReadableStreamDefaultReader => "()",
        TsType::StreamReadResult => "__TrustStreamReadResult",
        TsType::Uint8Array => "Vec<u8>",
        TsType::ObjectNum(_) => "serde_json::Value",
        TsType::TypeParam(_) => unreachable!("type params must be monomorphized before codegen"),
        TsType::Fn { .. } => "std::rc::Rc<dyn Fn(f64) -> f64>",
        TsType::ClassInstance(_) => "std::collections::HashMap<String, f64>",
        TsType::Promise(_) => unreachable!("rust_ty_scalar: awaitable wrapper is not a Rust value type"),
        TsType::Union(_) => unreachable!("rust_ty_scalar: use rust_ty_string for unions"),
        TsType::Intersection(_) => unreachable!("rust_ty_scalar: use rust_ty_string for intersections"),
        TsType::RustExtern { .. } => {
            unreachable!("rust_ty_scalar: use rust_ty_string for Rust extern types")
        }
    }
}

/// Rust 类型字符串（含 [`TsType::RustExtern`] 与同质联合）。
pub(super) fn rust_ty_string(t: &TsType, f: &IRFunction) -> Result<String, CompileError> {
    match t {
        TsType::Union(members) => {
            if members.is_empty() {
                return Err(diag(
                    f.cm.as_ref(),
                    &f.source_path,
                    f.span,
                    "empty union type",
                ));
            }
            if members.len() == 2 {
                use TsType::{Null, String as TsString, StringLit};
                let (a, b) = (&members[0], &members[1]);
                let stringish = |t: &TsType| matches!(t, TsString | StringLit(_));
                if matches!((a, b), (Null, _) | (_, Null)) && (stringish(a) || stringish(b)) {
                    return Ok("Option<String>".to_string());
                }
            }
            let mut it = members.iter();
            let first = rust_ty_string(it.next().unwrap(), f)?;
            for m in it {
                let r = rust_ty_string(m, f)?;
                if r != first {
                    return Err(diag(
                        f.cm.as_ref(),
                        &f.source_path,
                        f.span,
                        "union type cannot be mapped to a single Rust type (heterogeneous members)",
                    ));
                }
            }
            Ok(first)
        }
        TsType::TypeParam(name) => Err(diag(
            f.cm.as_ref(),
            &f.source_path,
            f.span,
            format!("internal error: uninstantiated type parameter `{name}` reached codegen"),
        )),
        TsType::RustExtern { rust_type, .. } => Ok(rust_type.clone()),
        _ => Ok(rust_ty_scalar(t).to_string()),
    }
}

pub(super) fn indent(n: usize) -> String {
    "    ".repeat(n)
}

pub(super) fn stmt_span(s: &IRStmt) -> Span {
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

pub(super) fn emit_ts_span_comment(out: &mut String, ind: &str, f: &IRFunction, span: Span) {
    let loc = f.cm.lookup_char_pos(span.lo);
    out.push_str(ind);
    out.push_str("// ts: ");
    out.push_str(&f.source_path);
    out.push(':');
    out.push_str(&loc.line.to_string());
    out.push(':');
    out.push_str(&loc.col_display.to_string());
    out.push('\n');
}
