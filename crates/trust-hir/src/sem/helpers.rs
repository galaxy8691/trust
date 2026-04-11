use std::path::Path;

use swc_common::Span;

use crate::error::{diag, CompileError};
use crate::ir::{ObjectProp, SendSourceMap, TsType};

/// R1/R2: 宽度子类型检查（字段 + 方法）
/// `got` 的每个成员必须在 `expected` 中存在且类型可赋。
/// 不强制要求 `expected` 的所有成员都在 `got` 中（支持类继承）。
/// R2: Field(Fn { ... }) 与 Method { ... } 兼容当签名匹配时
fn object_shape_assignable(expected: &[ObjectProp], got: &[ObjectProp]) -> bool {
    use crate::ir::ObjectMemberKind;

    // got 的每个成员必须在 expected 中存在且类型可赋
    for g in got {
        let Some(e) = expected.iter().find(|p| p.name == g.name) else {
            return false;
        };
        // 检查成员兼容性
        if !object_member_assignable(e, g) {
            return false;
        }
    }

    true
}

/// R2: 检查单个对象成员是否可赋
/// - Field(T) 与 Field(T) 比较类型
/// - Method { params, ret } 与 Method { params, ret } 比较签名
/// - Field(Fn { params, ret }) 与 Method { params, ret } 兼容（允许函数作为方法）
fn object_member_assignable(expected: &ObjectProp, got: &ObjectProp) -> bool {
    use crate::ir::ObjectMemberKind;

    match (&expected.kind, &got.kind) {
        // 字段与字段比较
        (ObjectMemberKind::Field(e_ty), ObjectMemberKind::Field(g_ty)) => {
            type_assignable(e_ty, g_ty)
        }
        // 方法与方法比较
        (
            ObjectMemberKind::Method {
                params: ep,
                ret: er,
            },
            ObjectMemberKind::Method {
                params: gp,
                ret: gr,
            },
        ) => {
            if ep.len() != gp.len() {
                return false;
            }
            if ep.iter().zip(gp.iter()).any(|(e, g)| !type_assignable(e, g)) {
                return false;
            }
            type_assignable(er, gr)
        }
        // R2: 期望 Method，得到 Field(Fn) —— 函数作为方法实现
        (
            ObjectMemberKind::Method {
                params: ep,
                ret: er,
            },
            ObjectMemberKind::Field(g_ty),
        ) => {
            if let TsType::Fn { params: gp, ret: gr } = g_ty.as_ref() {
                // R2: 函数作为方法时，函数的第一个参数是 receiver
                // 所以函数参数数量 = 方法参数数量 + 1 (receiver)
                if gp.len() != ep.len() + 1 {
                    return false;
                }
                // 比较参数（跳过函数的 receiver 参数）
                if ep.iter().zip(gp.iter().skip(1)).any(|(e, g)| !type_assignable(e, g)) {
                    return false;
                }
                return type_assignable(er, gr);
            }
            false
        }
        // 其他情况不兼容
        _ => false,
    }
}

pub(super) fn is_numberish(t: &TsType) -> bool {
    match t {
        TsType::Number | TsType::NumberLit(_) => true,
        TsType::Union(m) => m.iter().all(is_numberish),
        TsType::Intersection(m) => m.iter().any(is_numberish),
        _ => false,
    }
}

pub(super) fn is_booleanish(t: &TsType) -> bool {
    match t {
        TsType::Boolean | TsType::BoolLit(_) => true,
        TsType::Union(m) => m.iter().all(is_booleanish),
        TsType::Intersection(m) => m.iter().any(is_booleanish),
        _ => false,
    }
}

pub(super) fn is_stringish(t: &TsType) -> bool {
    match t {
        TsType::String | TsType::StringLit(_) => true,
        TsType::Union(m) => m.iter().all(is_stringish),
        TsType::Intersection(m) => m.iter().any(is_stringish),
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
    // 交集类型: 值必须满足所有成员类型
    if let TsType::Intersection(members) = expected {
        return members.iter().all(|e| type_assignable(e, got));
    }
    // 值如果是交集类型，则满足 expected 当且仅当任一成员满足
    if let TsType::Intersection(members) = got {
        return members.iter().any(|g| type_assignable(expected, g));
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
    if let (TsType::ObjectNum(exp), TsType::ObjectNum(got)) = (expected, got) {
        // R1: 如果 expected 是空的（前向引用占位符），认为匹配
        if exp.is_empty() {
            return true;
        }
        return object_shape_assignable(exp, got);
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
    if matches!(&a, TsType::Fn { .. }) && matches!(&b, TsType::Fn { .. }) {
        if type_assignable(&a, &b) && type_assignable(&b, &a) {
            return Ok(a);
        }
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

/// D1: 尝试根据 discriminant 字段值收窄联合类型。
/// 
/// 参数:
/// - `union_ty`: 联合类型 (应为 TsType::Union)
/// - `field_name`: discriminant 字段名
/// - `literal_ty`: 要匹配的字面量类型
/// 
/// 返回:
/// - `Some(TsType)`: 收窄后的类型（单个 ObjectNum 或剩余成员的 Union）
/// - `None`: 无法收窄（不满足 discriminated union 条件）
pub(super) fn narrow_union_by_discriminant(
    union_ty: &TsType,
    field_name: &str,
    literal_ty: &TsType,
) -> Option<(TsType, Option<TsType>)> {
    // 必须是联合类型
    let members = match union_ty {
        TsType::Union(m) => m,
        _ => return None,
    };

    // 所有成员必须是 ObjectNum
    let object_members: Vec<&Vec<ObjectProp>> = members
        .iter()
        .filter_map(|t| match t {
            TsType::ObjectNum(props) => Some(props),
            _ => None,
        })
        .collect();

    if object_members.len() != members.len() {
        return None; // 不是所有成员都是对象
    }

    // 检查每个成员是否有 discriminant 字段，且是字面量类型
    let mut discriminant_values: Vec<(&Vec<ObjectProp>, TsType)> = Vec::new();
    for props in &object_members {
        let field = props.iter().find(|p| p.name == field_name)?;
        if field.optional {
            return None; // discriminant 字段必须是必需的
        }
        // R1: discriminant 必须是字段（方法不能作为 discriminant）
        let field_ty = field.ty()?;
        let lit_ty = match field_ty {
            TsType::NumberLit(n) => TsType::NumberLit(*n),
            TsType::BoolLit(b) => TsType::BoolLit(*b),
            TsType::StringLit(s) => TsType::StringLit(s.clone()),
            _ => return None, // 不是字面量类型
        };
        discriminant_values.push((props, lit_ty));
    }

    // 验证字面量值互不相同（pairwise-distinct）
    for i in 0..discriminant_values.len() {
        for j in (i + 1)..discriminant_values.len() {
            if discriminant_values[i].1 == discriminant_values[j].1 {
                return None; // 字面量值不唯一，不是有效的 discriminated union
            }
        }
    }

    // 找到匹配的成员（then 分支）
    let then_member = discriminant_values
        .iter()
        .find(|(_, lit)| lit == literal_ty)
        .map(|(props, _)| (*props).clone());

    // 计算 else 分支的剩余类型
    let else_members: Vec<TsType> = discriminant_values
        .iter()
        .filter(|(_, lit)| lit != literal_ty)
        .map(|(props, _)| TsType::ObjectNum((*props).clone()))
        .collect();

    let then_ty = then_member.map(TsType::ObjectNum);
    let else_ty = match else_members.len() {
        0 => None,
        1 => Some(else_members.into_iter().next().unwrap()),
        _ => Some(TsType::Union(else_members)),
    };

    Some((then_ty?, else_ty))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::ObjectMemberKind;

    #[test]
    fn test_narrow_union_by_discriminant_basic() {
        // type Shape = { kind: 'circle', radius: number } | { kind: 'square', side: number }
        let circle = TsType::ObjectNum(vec![
            ObjectProp { name: "kind".to_string(), optional: false, kind: ObjectMemberKind::Field(Box::new(TsType::StringLit("circle".to_string()))) },
            ObjectProp { name: "radius".to_string(), optional: false, kind: ObjectMemberKind::Field(Box::new(TsType::Number)) },
        ]);
        let square = TsType::ObjectNum(vec![
            ObjectProp { name: "kind".to_string(), optional: false, kind: ObjectMemberKind::Field(Box::new(TsType::StringLit("square".to_string()))) },
            ObjectProp { name: "side".to_string(), optional: false, kind: ObjectMemberKind::Field(Box::new(TsType::Number)) },
        ]);
        let union = TsType::Union(vec![circle.clone(), square.clone()]);

        // 匹配 'circle'
        let result = narrow_union_by_discriminant(&union, "kind", &TsType::StringLit("circle".to_string()));
        assert!(result.is_some());
        let (then_ty, else_ty) = result.unwrap();
        assert_eq!(then_ty, circle); // then 分支收窄为 circle
        assert_eq!(else_ty, Some(square)); // else 分支收窄为 square
    }

    #[test]
    fn test_narrow_union_by_discriminant_three_arms() {
        // type ABC = A | B | C
        let a = TsType::ObjectNum(vec![
            ObjectProp { name: "kind".to_string(), optional: false, kind: ObjectMemberKind::Field(Box::new(TsType::StringLit("a".to_string()))) },
            ObjectProp { name: "val".to_string(), optional: false, kind: ObjectMemberKind::Field(Box::new(TsType::Number)) },
        ]);
        let b = TsType::ObjectNum(vec![
            ObjectProp { name: "kind".to_string(), optional: false, kind: ObjectMemberKind::Field(Box::new(TsType::StringLit("b".to_string()))) },
            ObjectProp { name: "str".to_string(), optional: false, kind: ObjectMemberKind::Field(Box::new(TsType::String)) },
        ]);
        let c = TsType::ObjectNum(vec![
            ObjectProp { name: "kind".to_string(), optional: false, kind: ObjectMemberKind::Field(Box::new(TsType::StringLit("c".to_string()))) },
            ObjectProp { name: "flag".to_string(), optional: false, kind: ObjectMemberKind::Field(Box::new(TsType::Boolean)) },
        ]);
        let union = TsType::Union(vec![a.clone(), b.clone(), c.clone()]);

        // 匹配 'a'
        let result = narrow_union_by_discriminant(&union, "kind", &TsType::StringLit("a".to_string()));
        assert!(result.is_some());
        let (then_ty, else_ty) = result.unwrap();
        assert_eq!(then_ty, a); // then 分支收窄为 a
        // else 分支应该是 b | c
        assert_eq!(else_ty, Some(TsType::Union(vec![b, c])));
    }

    #[test]
    fn test_narrow_union_by_discriminant_not_literal() {
        // discriminant 不是字面量类型
        let a = TsType::ObjectNum(vec![
            ObjectProp { name: "kind".to_string(), optional: false, kind: ObjectMemberKind::Field(Box::new(TsType::String)) }, // 不是 StringLit
            ObjectProp { name: "val".to_string(), optional: false, kind: ObjectMemberKind::Field(Box::new(TsType::Number)) },
        ]);
        let b = TsType::ObjectNum(vec![
            ObjectProp { name: "kind".to_string(), optional: false, kind: ObjectMemberKind::Field(Box::new(TsType::String)) },
            ObjectProp { name: "str".to_string(), optional: false, kind: ObjectMemberKind::Field(Box::new(TsType::String)) },
        ]);
        let union = TsType::Union(vec![a, b]);

        let result = narrow_union_by_discriminant(&union, "kind", &TsType::StringLit("a".to_string()));
        assert!(result.is_none()); // 无法收窄，因为 kind 是 string 不是字面量
    }

    #[test]
    fn test_narrow_union_by_discriminant_duplicate_values() {
        // discriminant 值不唯一
        let a = TsType::ObjectNum(vec![
            ObjectProp { name: "kind".to_string(), optional: false, kind: ObjectMemberKind::Field(Box::new(TsType::StringLit("x".to_string()))) },
            ObjectProp { name: "val".to_string(), optional: false, kind: ObjectMemberKind::Field(Box::new(TsType::Number)) },
        ]);
        let b = TsType::ObjectNum(vec![
            ObjectProp { name: "kind".to_string(), optional: false, kind: ObjectMemberKind::Field(Box::new(TsType::StringLit("x".to_string()))) }, // 重复的 'x'
            ObjectProp { name: "str".to_string(), optional: false, kind: ObjectMemberKind::Field(Box::new(TsType::String)) },
        ]);
        let union = TsType::Union(vec![a, b]);

        let result = narrow_union_by_discriminant(&union, "kind", &TsType::StringLit("x".to_string()));
        assert!(result.is_none()); // 无法收窄，因为字面量值不唯一
    }
}
