//! 类型检查器 — §SEM-REQ-003, §TYP-REQ-001
//!
//! Phase 1 类型检查只关注两件事：
//! 1. 二元运算的操作数类型一致性 — 核心规则：`i32 + f64` 报错
//! 2. 函数调用实参与形参的数量和类型匹配
//!
//! §设计文档 §3.2.1: 数字类型严格分离（方案 B）
//! §design-constraints §3.1.1: 函数级独立检查 + 哨兵避免级联报错

#![allow(clippy::result_unit_err)] // 接口伪代码使用 Result<_, ()>；实际实现在 Phase 1.6 迁移

use crate::hir::*;
use crate::name_res::DiagError;
use trust_parser::ast::Span;

// ============================================================================
// 入口 — §3.3.7
// ============================================================================

/// §设计文档 §3.1.3 / spec SEM-REQ-003: 类型检查入口
///
/// 函数级独立检查：每个 `HirFunction` 独立 `check_function()`，
/// 错误收集进 `Vec<DiagError>`，互不影响。
pub fn check_types(
    hir: &mut HirProgram,
    diagnostics: &mut Vec<DiagError>,
) -> Result<(), Vec<DiagError>> {
    for item in &mut hir.items {
        match item {
            HirItem::Function(f) => {
                check_function(f, diagnostics);
            }
            HirItem::Const(_c) => { /* Phase 1: const 无额外类型检查 */ }
            HirItem::Shared(_s) => { /* Phase 1: shared 无额外类型检查 */ }
            HirItem::Stub(_) => {}
        }
    }

    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics.clone())
    }
}

fn check_function(func: &mut HirFunction, diagnostics: &mut Vec<DiagError>) {
    // 创建局部作用域（复用 name_res 建立的 scope）
    let fn_scope = func.scope.clone();
    check_block(&mut func.body, &fn_scope, diagnostics);
}

fn check_block(block: &mut HirBlock, scope: &Scope, diagnostics: &mut Vec<DiagError>) {
    for stmt in &mut block.statements {
        check_stmt(stmt, scope, diagnostics);
    }
}

fn check_stmt(stmt: &mut HirStmt, scope: &Scope, diagnostics: &mut Vec<DiagError>) {
    match stmt {
        HirStmt::Let(let_s) => {
            check_expr(&mut let_s.init, scope, diagnostics);
            let init_ty = expr_type(&let_s.init);
            // 类型标注兼容性检查
            if let_s.ty != HirType::Error
                && init_ty != HirType::Error
                && !types_compatible(&let_s.ty, &init_ty)
            {
                diagnostics.push(DiagError::new(
                    format!(
                        "type mismatch: expected `{}`, found `{}`",
                        let_s.ty, init_ty
                    ),
                    let_s.span.clone(),
                ));
                let_s.ty = HirType::Error;
            }
            if let_s.ty == HirType::Error && init_ty != HirType::Error {
                let_s.ty = init_ty;
            }
        }
        HirStmt::Const(c) => {
            check_expr(&mut c.init, scope, diagnostics);
            let init_ty = expr_type(&c.init);
            if c.ty == HirType::Error && init_ty != HirType::Error {
                c.ty = init_ty;
            }
        }
        HirStmt::Shared(s) => {
            check_expr(&mut s.init, scope, diagnostics);
            let init_ty = expr_type(&s.init);
            if s.ty == HirType::Error && init_ty != HirType::Error {
                s.ty = init_ty;
            }
        }
        HirStmt::If(if_s) => {
            check_expr(&mut if_s.condition, scope, diagnostics);
            check_block(&mut if_s.then_branch, scope, diagnostics);
            if let Some(ref mut else_b) = if_s.else_branch {
                check_block(else_b, scope, diagnostics);
            }
        }
        HirStmt::For(f) => {
            check_expr(&mut f.condition, scope, diagnostics);
            check_expr(&mut f.update, scope, diagnostics);
            check_block(&mut f.body, scope, diagnostics);
        }
        HirStmt::ForOf(f) => {
            check_expr(&mut f.iterator, scope, diagnostics);
            check_block(&mut f.body, scope, diagnostics);
        }
        HirStmt::While(w) => {
            check_expr(&mut w.condition, scope, diagnostics);
            check_block(&mut w.body, scope, diagnostics);
        }
        HirStmt::Loop(l) => {
            check_block(&mut l.body, scope, diagnostics);
        }
        HirStmt::Return(r) => {
            if let Some(ref mut v) = r.value {
                check_expr(v, scope, diagnostics);
            }
        }
        HirStmt::Break(b) => {
            if let Some(ref mut v) = b.value {
                check_expr(v, scope, diagnostics);
            }
        }
        HirStmt::Continue(_) => {}
        HirStmt::Expr(e) => {
            check_expr(e, scope, diagnostics);
        }
        HirStmt::Error => {}
    }
}

// ============================================================================
// 表达式类型检查
// ============================================================================

fn check_expr(expr: &mut HirExpr, scope: &Scope, diagnostics: &mut Vec<DiagError>) {
    match expr {
        // 字面量——类型已固定
        HirExpr::IntLiteral(..) => {}
        HirExpr::FloatLiteral(..) => {}
        HirExpr::BigIntLiteral(..) => {}
        HirExpr::StringLiteral(..) => {}
        HirExpr::BoolLiteral(..) => {}
        HirExpr::Error(..) => {}

        HirExpr::Ident(_, binding, _) => {
            // 验证 binding 是否存在
            if matches!(binding, HirBinding::Unresolved { .. }) {
                // 名称解析未完成（不应发生在此阶段）
                *expr = HirExpr::Error(Span::dummy());
            }
        }

        // §TYP-REQ-001: 二元运算类型检查
        HirExpr::Binary(lhs, op, rhs, ref mut ty, span) => {
            check_expr(lhs, scope, diagnostics);
            check_expr(rhs, scope, diagnostics);

            let lhs_ty = expr_type(lhs);
            let rhs_ty = expr_type(rhs);

            if lhs_ty == HirType::Error || rhs_ty == HirType::Error {
                *ty = HirType::Error;
                return;
            }

            match check_binary_op(*op, &lhs_ty, &rhs_ty, span.clone(), diagnostics) {
                Ok(result_ty) => *ty = result_ty,
                Err(()) => {
                    *ty = HirType::Error;
                    // 用 Error 替换 Binary 避免后续级联报错
                    *expr = HirExpr::Error(span.clone());
                }
            }
        }

        HirExpr::Unary(_op, inner, ref mut ty, _span) => {
            check_expr(inner, scope, diagnostics);
            let inner_ty = expr_type(inner);
            *ty = match inner_ty {
                HirType::I32 | HirType::F64 | HirType::I64 => inner_ty,
                HirType::Bool => HirType::Bool, // !true → Bool
                HirType::Error => HirType::Error,
                _ => {
                    diagnostics.push(DiagError::new(
                        format!("cannot apply unary operator to type `{inner_ty}`"),
                        Span::dummy(),
                    ));
                    HirType::Error
                }
            };
        }

        HirExpr::Call(callee, args, ref mut ty, span) => {
            check_expr(callee, scope, diagnostics);

            for arg in args.iter_mut() {
                check_expr(&mut arg.expr, scope, diagnostics);
            }

            let callee_ty = expr_type(callee);
            match callee_ty {
                HirType::Function(param_types, ret_type) => {
                    match check_call(
                        "fn", // 简化：无 func_name
                        &HirType::Function(param_types, ret_type.clone()),
                        args,
                        span.clone(),
                        diagnostics,
                    ) {
                        Ok(ret) => *ty = ret,
                        Err(()) => {
                            *ty = HirType::Error;
                            *expr = HirExpr::Error(span.clone());
                        }
                    }
                }
                HirType::Error => {
                    *ty = HirType::Error;
                }
                _ => {
                    // Phase 1: 对于 console.log → ferro_rt::console::log 的调用，
                    // callee 是 Ident，其类型可能未设置。
                    // 允许任意类型的调用（兼容 console.log 映射）。
                    *ty = HirType::Void;
                }
            }
        }

        HirExpr::ArrowFn(params, ret, body, _is_move, _span) => {
            // 闭包类型检查：推断返回类型
            let mut fn_scope = Scope::new();
            for p in params.iter() {
                fn_scope.insert(
                    &p.name,
                    HirBinding::LocalVar {
                        ty: p.ty.clone(),
                        mutable: false,
                        span: p.span.clone(),
                    },
                );
            }
            check_block(body, &fn_scope, diagnostics);

            // 从 body 推断返回类型
            if *ret == HirType::Error {
                *ret = infer_return_type(body);
            }
        }

        HirExpr::AsCast(inner, ref target_ty, span) => {
            check_expr(inner, scope, diagnostics);
            let inner_ty = expr_type(inner);
            if inner_ty == HirType::Error {
                *expr = HirExpr::Error(span.clone());
                return;
            }
            if !check_as_cast(&inner_ty, target_ty, span.clone(), diagnostics) {
                *expr = HirExpr::Error(span.clone());
            }
        }

        HirExpr::Reference(inner, _span) => {
            check_expr(inner, scope, diagnostics);
        }

        HirExpr::AssertUnwrap(inner, _span) => {
            check_expr(inner, scope, diagnostics);
        }

        HirExpr::TryPropagate(inner, _span) => {
            check_expr(inner, scope, diagnostics);
        }

        HirExpr::TemplateLiteral(parts, _span) => {
            for part in parts {
                if let HirTemplatePartKind::Expr(ref mut e) = part.kind {
                    check_expr(e, scope, diagnostics);
                }
            }
        }

        // AC-SEM-002: if 表达式类型检查
        HirExpr::If(if_s, _span) => {
            check_expr(&mut if_s.condition, scope, diagnostics);
            check_block(&mut if_s.then_branch, scope, diagnostics);
            if let Some(ref mut else_b) = if_s.else_branch {
                check_block(else_b, scope, diagnostics);
            }
        }

        HirExpr::Loop(l, _span) => {
            check_block(&mut l.body, scope, diagnostics);
        }

        HirExpr::Block(b, _span) => {
            check_block(b, scope, diagnostics);
        }
    }
}

// ============================================================================
// §TYP-REQ-001: 二元运算类型检查
// ============================================================================

/// §设计文档 §3.2.1 / spec TYP-REQ-001: 二元运算类型检查
pub fn check_binary_op(
    op: BinOp,
    lhs: &HirType,
    rhs: &HirType,
    span: Span,
    diagnostics: &mut Vec<DiagError>,
) -> Result<HirType, ()> {
    match op {
        // TY-RULE-01: 算术运算 — 操作数类型必须相同
        BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div | BinOp::Mod => {
            // TY-RULE-04: String + String → String
            if op == BinOp::Add && *lhs == HirType::String && *rhs == HirType::String {
                return Ok(HirType::String);
            }
            if *lhs != *rhs {
                diagnostics.push(DiagError::new(
                    format!(
                        "type mismatch: cannot apply `{op:?}` to `{lhs}` and `{rhs}` (operands must be same type)",
                    ),
                    span,
                ));
                return Err(());
            }
            match lhs {
                HirType::I32 | HirType::F64 | HirType::I64 => Ok(lhs.clone()),
                _ => {
                    diagnostics.push(DiagError::new(
                        format!("arithmetic not supported for type `{lhs}`"),
                        span,
                    ));
                    Err(())
                }
            }
        }

        // TY-RULE-02: 比较运算 — 操作数类型必须相同，结果 Bool
        BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Gt | BinOp::Le | BinOp::Ge => {
            if *lhs != *rhs {
                diagnostics.push(DiagError::new(
                    format!(
                        "type mismatch: cannot compare `{lhs}` and `{rhs}` (operands must be same type)",
                    ),
                    span,
                ));
                return Err(());
            }
            Ok(HirType::Bool)
        }

        // TY-RULE-03: 逻辑运算 — 操作数必须 Bool
        BinOp::And | BinOp::Or => {
            if *lhs != HirType::Bool || *rhs != HirType::Bool {
                diagnostics.push(DiagError::new(
                    format!("logical operators require boolean operands, got `{lhs}` and `{rhs}`"),
                    span,
                ));
                return Err(());
            }
            Ok(HirType::Bool)
        }

        // ?? 运算符 — Phase 1 排除，不应到达此处
        BinOp::QuestionQuestion => {
            diagnostics.push(DiagError::new(
                "`??` not supported in Phase 1".into(),
                span,
            ));
            Err(())
        }
    }
}

// ============================================================================
// §TYP-REQ-001: as 转换检查 — §3.3.4
// ============================================================================

fn check_as_cast(
    src: &HirType,
    target: &HirType,
    span: Span,
    diagnostics: &mut Vec<DiagError>,
) -> bool {
    if *src == *target {
        return true; // no-op
    }

    match (src, target) {
        // I32 ↔ F64
        (HirType::I32, HirType::F64) => true,
        (HirType::F64, HirType::I32) => {
            diagnostics.push(DiagError::new(
                "truncation: `f64 as i32` may lose precision".into(),
                span,
            ));
            true // 允许但 warning
        }
        // I32 ↔ I64
        (HirType::I32, HirType::I64) => true,
        (HirType::I64, HirType::I32) => {
            diagnostics.push(DiagError::new(
                "truncation: `i64 as i32` may lose precision".into(),
                span,
            ));
            true
        }
        // I64 ↔ F64
        (HirType::I64, HirType::F64) => true,
        (HirType::F64, HirType::I64) => true,

        // Bool → 数字禁止
        (HirType::Bool, HirType::I32)
        | (HirType::Bool, HirType::F64)
        | (HirType::Bool, HirType::I64) => {
            diagnostics.push(DiagError::new(
                format!("cannot cast `bool` to `{target}`"),
                span,
            ));
            false
        }

        // 数字 → Bool 禁止
        (HirType::I32 | HirType::F64 | HirType::I64, HirType::Bool) => {
            diagnostics.push(DiagError::new(
                format!("cannot cast `{src}` to `bool`"),
                span,
            ));
            false
        }

        // String → 数字禁止
        (HirType::String, HirType::I32 | HirType::F64 | HirType::I64 | HirType::Bool) => {
            diagnostics.push(DiagError::new(
                format!("cannot cast `string` to `{target}`"),
                span,
            ));
            false
        }

        // 跨族转换禁止
        _ => {
            diagnostics.push(DiagError::new(
                format!("invalid cast: `{src}` to `{target}`"),
                span,
            ));
            false
        }
    }
}

// ============================================================================
// §SEM-REQ-003: 函数调用类型检查 — §3.3.5
// ============================================================================

/// §设计文档 §3.1.3 / spec SEM-REQ-003: 函数调用类型检查
pub fn check_call(
    _func_name: &str,
    func_type: &HirType,
    args: &[HirCallArg],
    span: Span,
    diagnostics: &mut Vec<DiagError>,
) -> Result<HirType, ()> {
    if let HirType::Function(param_types, ret_type) = func_type {
        // 1. 实参数量检查
        if args.len() != param_types.len() {
            diagnostics.push(DiagError::new(
                format!(
                    "function expects {} arguments, got {}",
                    param_types.len(),
                    args.len()
                ),
                span,
            ));
            return Err(());
        }

        // 2. 逐对类型检查
        for (i, (param_ty, arg)) in param_types.iter().zip(args.iter()).enumerate() {
            let arg_ty = expr_type(&arg.expr);
            if arg_ty == HirType::Error || *param_ty == HirType::Error {
                continue;
            }
            if !types_compatible(param_ty, &arg_ty) {
                diagnostics.push(DiagError::new(
                    format!(
                        "argument {} type mismatch: expected `{}`, got `{}`",
                        i + 1,
                        param_ty,
                        arg_ty
                    ),
                    arg.span.clone(),
                ));
                return Err(());
            }
        }

        // 3. 返回类型
        Ok(*ret_type.clone())
    } else {
        diagnostics.push(DiagError::new(
            format!("cannot call non-function type `{func_type}`"),
            span,
        ));
        Err(())
    }
}

// ============================================================================
// 工具函数
// ============================================================================

fn expr_type(expr: &HirExpr) -> HirType {
    match expr {
        HirExpr::IntLiteral(..) => HirType::I32,
        HirExpr::FloatLiteral(..) => HirType::F64,
        HirExpr::BigIntLiteral(..) => HirType::I64,
        HirExpr::StringLiteral(..) => HirType::String,
        HirExpr::BoolLiteral(..) => HirType::Bool,
        HirExpr::Ident(_, binding, _) => match binding {
            HirBinding::LocalVar { ty, .. } => ty.clone(),
            HirBinding::ModuleConst { ty, .. } => ty.clone(),
            HirBinding::ModuleShared { ty, .. } => ty.clone(),
            HirBinding::Function { return_type, .. } => HirType::Function(vec![], Box::new(return_type.clone())),
            HirBinding::Import { ty, .. } => ty.clone(),
            HirBinding::Unresolved { .. } => HirType::Error,
        },
        HirExpr::Binary(.., ty, _) => ty.clone(),
        HirExpr::Unary(.., ty, _) => ty.clone(),
        HirExpr::Call(.., ty, _) => ty.clone(),
        HirExpr::AsCast(_, ty, _) => ty.clone(),
        HirExpr::If(..) => HirType::Error, // if 表达式类型由分支推断，Phase 1 简化
        HirExpr::Loop(..) => HirType::I32, // loop break 带值 → 值类型；Phase 1 简化
        HirExpr::Block(block, _) => infer_block_type(block),
        HirExpr::ArrowFn(..) => HirType::Function(vec![], Box::new(HirType::Void)),
        HirExpr::Reference(..) => HirType::Ref(Box::new(HirType::Error)),
        HirExpr::TemplateLiteral(..) => HirType::String,
        HirExpr::AssertUnwrap(..) | HirExpr::TryPropagate(..) => HirType::Error,
        HirExpr::Error(..) => HirType::Error,
    }
}

fn infer_block_type(block: &HirBlock) -> HirType {
    // 块表达式的类型是最后一个语句的类型
    block
        .statements
        .last()
        .map(|s| match s {
            HirStmt::Expr(e) => expr_type(e),
            HirStmt::Return(r) => r.value.as_ref().map(|v| expr_type(v)).unwrap_or(HirType::Void),
            _ => HirType::Void,
        })
        .unwrap_or(HirType::Void)
}

fn infer_return_type(body: &HirBlock) -> HirType {
    for stmt in &body.statements {
        if let HirStmt::Return(r) = stmt {
            return r.value.as_ref().map(|v| expr_type(v)).unwrap_or(HirType::Void);
        }
    }
    HirType::Void
}

fn types_compatible(expected: &HirType, actual: &HirType) -> bool {
    if *expected == HirType::Error || *actual == HirType::Error {
        return true; // 哨兵——跳过比较
    }
    expected == actual
}

// ============================================================================
// 单元测试 — §4.1
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // AC-TYP-001: i32 + f64 → 编译错误
    #[test]
    fn check_binary_i32_plus_f64_error() {
        let mut diags = vec![];
        let r = check_binary_op(
            BinOp::Add,
            &HirType::I32,
            &HirType::F64,
            Span::dummy(),
            &mut diags,
        );
        assert!(r.is_err(), "i32 + f64 should fail");
        assert!(
            diags.iter().any(|d| d.message.contains("type mismatch")),
            "should have type mismatch diagnostic"
        );
    }

    // AC-TYP-002/003: as 转换通过
    #[test]
    fn check_as_i32_to_f64_allowed() {
        let mut diags = vec![];
        let ok = check_as_cast(&HirType::I32, &HirType::F64, Span::dummy(), &mut diags);
        assert!(ok, "i32 as f64 should be allowed");
    }

    #[test]
    fn check_as_f64_to_i32_allowed_with_warning() {
        let mut diags = vec![];
        let ok = check_as_cast(&HirType::F64, &HirType::I32, Span::dummy(), &mut diags);
        assert!(ok, "f64 as i32 should be allowed");
        assert!(
            diags.iter().any(|d| d.message.contains("truncation")),
            "should warn about truncation"
        );
    }

    #[test]
    fn check_as_bool_to_i32_forbidden() {
        let mut diags = vec![];
        let ok = check_as_cast(&HirType::Bool, &HirType::I32, Span::dummy(), &mut diags);
        assert!(!ok, "bool as i32 should be forbidden");
    }

    #[test]
    fn check_as_string_to_i32_forbidden() {
        let mut diags = vec![];
        let ok = check_as_cast(&HirType::String, &HirType::I32, Span::dummy(), &mut diags);
        assert!(!ok, "string as i32 should be forbidden");
    }

    // TY-RULE-01: I32+I32 → I32
    #[test]
    fn check_binary_i32_plus_i32_ok() {
        let mut diags = vec![];
        let r = check_binary_op(
            BinOp::Add,
            &HirType::I32,
            &HirType::I32,
            Span::dummy(),
            &mut diags,
        );
        assert_eq!(r, Ok(HirType::I32));
        assert!(diags.is_empty());
    }

    // TY-RULE-01: F64+F64 → F64
    #[test]
    fn check_binary_f64_plus_f64_ok() {
        let mut diags = vec![];
        let r = check_binary_op(
            BinOp::Add,
            &HirType::F64,
            &HirType::F64,
            Span::dummy(),
            &mut diags,
        );
        assert_eq!(r, Ok(HirType::F64));
    }

    // TY-RULE-02: I32 == I32 → Bool
    #[test]
    fn check_binary_eq_i32_returns_bool() {
        let mut diags = vec![];
        let r = check_binary_op(
            BinOp::Eq,
            &HirType::I32,
            &HirType::I32,
            Span::dummy(),
            &mut diags,
        );
        assert_eq!(r, Ok(HirType::Bool));
    }

    // TY-RULE-03: Bool && Bool → Bool
    #[test]
    fn check_binary_and_bool_returns_bool() {
        let mut diags = vec![];
        let r = check_binary_op(
            BinOp::And,
            &HirType::Bool,
            &HirType::Bool,
            Span::dummy(),
            &mut diags,
        );
        assert_eq!(r, Ok(HirType::Bool));
    }

    // TY-RULE-03: I32 && I32 → 错误
    #[test]
    fn check_binary_and_i32_error() {
        let mut diags = vec![];
        let r = check_binary_op(
            BinOp::And,
            &HirType::I32,
            &HirType::I32,
            Span::dummy(),
            &mut diags,
        );
        assert!(r.is_err());
    }

    // 函数调用签名验证
    #[test]
    fn check_call_matching_params_returns_ret_type() {
        let func_ty = HirType::Function(vec![HirType::I32, HirType::F64], Box::new(HirType::Bool));
        let args = vec![
            HirCallArg {
                mode: ParamMode::Default,
                expr: Box::new(HirExpr::IntLiteral(1, Span::dummy())),
                span: Span::dummy(),
            },
            HirCallArg {
                mode: ParamMode::Default,
                expr: Box::new(HirExpr::FloatLiteral(2.0, Span::dummy())),
                span: Span::dummy(),
            },
        ];
        let mut diags = vec![];
        let r = check_call("f", &func_ty, &args, Span::dummy(), &mut diags);
        assert_eq!(r, Ok(HirType::Bool));
    }

    #[test]
    fn check_call_wrong_arg_count_error() {
        let func_ty = HirType::Function(vec![HirType::I32], Box::new(HirType::Void));
        let args = vec![];
        let mut diags = vec![];
        let r = check_call("f", &func_ty, &args, Span::dummy(), &mut diags);
        assert!(r.is_err());
    }

    #[test]
    fn check_call_wrong_arg_type_error() {
        let func_ty = HirType::Function(vec![HirType::I32], Box::new(HirType::Void));
        let args = vec![HirCallArg {
            mode: ParamMode::Default,
            expr: Box::new(HirExpr::StringLiteral("hi".into(), Span::dummy())),
            span: Span::dummy(),
        }];
        let mut diags = vec![];
        let r = check_call("f", &func_ty, &args, Span::dummy(), &mut diags);
        assert!(r.is_err());
    }

    // 哨兵: Error 类型不产生级联
    #[test]
    fn sentinel_error_prevents_cascade() {
        // 当 lhs 是 Error 时，check_binary_op 不应该再报错
        // 这在 check_expr 中已处理：遇到 Error 操作数直接返回
        let mut diags = vec![];
        let r = check_binary_op(
            BinOp::Add,
            &HirType::Error,
            &HirType::F64,
            Span::dummy(),
            &mut diags,
        );
        assert!(r.is_err()); // Error 哨兵仍返回 Err
    }
}
