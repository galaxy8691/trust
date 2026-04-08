//! `JSON.parse` 编译期折叠：字符串字面量 → 合法 JSON → trust 闭合的 IR。

use serde_json::Value;
use swc_common::Span;

use crate::ir::IRExpr;

pub(crate) enum JsonParseFold {
    /// 非字符串字面量，保留 `JsonBuiltin::Parse`。
    NotStringLiteral,
    Folded(IRExpr),
}

/// 将 `JSON.parse` 的唯一实参若为 [`IRExpr::Str`]，则解析 JSON 并降为字面量 IR；否则返回 [`JsonParseFold::NotStringLiteral`]。
pub(crate) fn fold_json_parse_arg(arg: IRExpr) -> Result<JsonParseFold, String> {
    let IRExpr::Str(text, span) = arg else {
        return Ok(JsonParseFold::NotStringLiteral);
    };
    let v: Value = serde_json::from_str(&text).map_err(|e| e.to_string())?;
    let ir = json_value_to_ir(v, span)?;
    Ok(JsonParseFold::Folded(ir))
}

fn json_value_to_ir(v: Value, span: Span) -> Result<IRExpr, String> {
    match v {
        Value::Null => Ok(IRExpr::Null(span)),
        Value::Bool(b) => Ok(IRExpr::Bool(b, span)),
        Value::Number(n) => {
            let f = n
                .as_f64()
                .ok_or_else(|| "JSON number out of range for f64".to_string())?;
            if !f.is_finite() {
                return Err("JSON number must be finite".to_string());
            }
            Ok(IRExpr::Number(f, span))
        }
        Value::String(s) => Ok(IRExpr::Str(s, span)),
        Value::Array(arr) => json_array_to_ir(arr, span),
        Value::Object(map) => json_object_flat_numbers_to_ir(map, span),
    }
}

fn json_array_to_ir(arr: Vec<Value>, span: Span) -> Result<IRExpr, String> {
    if arr.is_empty() {
        return Ok(IRExpr::ArrayLit {
            elems: vec![],
            span,
        });
    }
    match &arr[0] {
        Value::Number(_) => {
            let mut elems = Vec::with_capacity(arr.len());
            for v in arr {
                match v {
                    Value::Number(n) => {
                        let f = n
                            .as_f64()
                            .ok_or_else(|| "JSON number out of range for f64".to_string())?;
                        if !f.is_finite() {
                            return Err("JSON number must be finite".to_string());
                        }
                        elems.push(IRExpr::Number(f, span));
                    }
                    _ => {
                        return Err(
                            "`JSON.parse` literal array must be homogeneous numbers or strings"
                                .to_string(),
                        );
                    }
                }
            }
            Ok(IRExpr::ArrayLit { elems, span })
        }
        Value::String(_) => {
            let mut elems = Vec::with_capacity(arr.len());
            for v in arr {
                match v {
                    Value::String(s) => elems.push(IRExpr::Str(s, span)),
                    _ => {
                        return Err(
                            "`JSON.parse` literal array must be homogeneous numbers or strings"
                                .to_string(),
                        );
                    }
                }
            }
            Ok(IRExpr::ArrayLit { elems, span })
        }
        _ => Err(
            "`JSON.parse` literal only supports homogeneous number[] or string[]".to_string(),
        ),
    }
}

fn json_object_flat_numbers_to_ir(
    map: serde_json::Map<String, Value>,
    span: Span,
) -> Result<IRExpr, String> {
    let mut fields: Vec<(String, IRExpr)> = Vec::with_capacity(map.len());
    for (k, v) in map {
        match v {
            Value::Number(n) => {
                let f = n
                    .as_f64()
                    .ok_or_else(|| "JSON number out of range for f64".to_string())?;
                if !f.is_finite() {
                    return Err("JSON number must be finite".to_string());
                }
                fields.push((k, IRExpr::Number(f, span)));
            }
            _ => {
                return Err(
                    "`JSON.parse` literal object values must be numbers (no nesting)".to_string(),
                );
            }
        }
    }
    fields.sort_by(|a, b| a.0.cmp(&b.0));
    for w in fields.windows(2) {
        if w[0].0 == w[1].0 {
            return Err(format!("duplicate object field `{}`", w[0].0));
        }
    }
    Ok(IRExpr::ObjectLit { fields, span })
}
