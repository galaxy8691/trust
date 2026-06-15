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
            HirItem::Const(c) => {
                check_expr(&mut c.init, &hir.scope, diagnostics, &HirType::Void);
                let init_ty = expr_type(&c.init);
                if c.ty != HirType::Error
                    && init_ty != HirType::Error
                    && !types_compatible(&c.ty, &init_ty)
                {
                    diagnostics.push(DiagError::new(
                        format!("const type mismatch: expected `{}`, found `{}`", c.ty, init_ty),
                        c.span.clone(),
                    ));
                }
            }
            HirItem::Shared(s) => {
                check_expr(&mut s.init, &hir.scope, diagnostics, &HirType::Void);
                let init_ty = expr_type(&s.init);
                if s.ty != HirType::Error
                    && init_ty != HirType::Error
                    && !types_compatible(&s.ty, &init_ty)
                {
                    diagnostics.push(DiagError::new(
                        format!("shared type mismatch: expected `{}`, found `{}`", s.ty, init_ty),
                        s.span.clone(),
                    ));
                }
            }
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
    let fn_scope = func.scope.clone();
    let ret_ty = func.return_type.clone();
    check_block(&mut func.body, &fn_scope, diagnostics, &ret_ty);

    // §2.3: 表达式体函数返回类型推断
    if func.is_expression_body && func.return_type == HirType::Error {
        func.return_type = infer_return_type(&func.body);
    }
}

fn check_block(
    block: &mut HirBlock,
    scope: &Scope,
    diagnostics: &mut Vec<DiagError>,
    fn_return_type: &HirType,
) {
    for stmt in &mut block.statements {
        check_stmt(stmt, scope, diagnostics, fn_return_type);
    }
}

fn check_stmt(
    stmt: &mut HirStmt,
    scope: &Scope,
    diagnostics: &mut Vec<DiagError>,
    fn_return_type: &HirType,
) {
    match stmt {
        HirStmt::Let(let_s) => {
            check_expr(&mut let_s.init, scope, diagnostics, fn_return_type);
            let init_ty = expr_type(&let_s.init);
            // v2.0: number 统一，不再 I32↔F64 提升

            let was_unannotated = let_s.ty == HirType::Error;
            let mismatched = let_s.ty != HirType::Error
                && init_ty != HirType::Error
                && !types_compatible(&let_s.ty, &init_ty);

            if mismatched {
                diagnostics.push(DiagError::new(
                    format!("type mismatch: expected `{}`, found `{}`", let_s.ty, init_ty),
                    let_s.span.clone(),
                ));
                let_s.ty = HirType::Error;
            } else if was_unannotated && init_ty != HirType::Error {
                // 只有原本无类型标注时才从 init 推断——类型不匹配后不应覆盖 Error 哨兵
                let_s.ty = init_ty;
            }
        }
        HirStmt::Const(c) => {
            check_expr(&mut c.init, scope, diagnostics, fn_return_type);
            let init_ty = expr_type(&c.init);
            // v2.0: number 统一

            let was_unannotated = c.ty == HirType::Error;
            let mismatched = c.ty != HirType::Error
                && init_ty != HirType::Error
                && !types_compatible(&c.ty, &init_ty);
            if mismatched {
                diagnostics.push(DiagError::new(
                    format!("type mismatch: expected `{}`, found `{}`", c.ty, init_ty),
                    c.span.clone(),
                ));
                c.ty = HirType::Error;
            } else if was_unannotated && init_ty != HirType::Error {
                c.ty = init_ty;
            }
        }
        HirStmt::Shared(s) => {
            check_expr(&mut s.init, scope, diagnostics, fn_return_type);
            let init_ty = expr_type(&s.init);
            // v2.0: number 统一

            let was_unannotated = s.ty == HirType::Error;
            let mismatched = s.ty != HirType::Error
                && init_ty != HirType::Error
                && !types_compatible(&s.ty, &init_ty);
            if mismatched {
                diagnostics.push(DiagError::new(
                    format!("type mismatch: expected `{}`, found `{}`", s.ty, init_ty),
                    s.span.clone(),
                ));
                s.ty = HirType::Error;
            } else if was_unannotated && init_ty != HirType::Error {
                s.ty = init_ty;
            }
        }
        HirStmt::If(if_s) => {
            check_expr(&mut if_s.condition, scope, diagnostics, fn_return_type);
            check_block(&mut if_s.then_branch, scope, diagnostics, fn_return_type);
            if let Some(ref mut else_b) = if_s.else_branch {
                check_block(else_b, scope, diagnostics, fn_return_type);
            }
        }
        HirStmt::For(f) => {
            check_stmt(&mut f.init, scope, diagnostics, fn_return_type);
            check_expr(&mut f.condition, scope, diagnostics, fn_return_type);
            check_expr(&mut f.update, scope, diagnostics, fn_return_type);
            check_block(&mut f.body, scope, diagnostics, fn_return_type);
        }
        HirStmt::ForOf(f) => {
            check_expr(&mut f.iterator, scope, diagnostics, fn_return_type);
            check_block(&mut f.body, scope, diagnostics, fn_return_type);
        }
        HirStmt::While(w) => {
            check_expr(&mut w.condition, scope, diagnostics, fn_return_type);
            check_block(&mut w.body, scope, diagnostics, fn_return_type);
        }
        // v2.0: Loop removed
        HirStmt::Return(r) => {
            if let Some(ref mut v) = r.value {
                check_expr(v, scope, diagnostics, fn_return_type);
                let val_ty = expr_type(v);
                if *fn_return_type != HirType::Error
                    && val_ty != HirType::Error
                    && !types_compatible(fn_return_type, &val_ty)
                {
                    diagnostics.push(DiagError::new(
                        format!(
                            "return type mismatch: expected `{}`, found `{}`",
                            fn_return_type, val_ty
                        ),
                        r.span.clone(),
                    ));
                }
            }
        }
        HirStmt::Break(b) => {
            if let Some(ref mut v) = b.value {
                check_expr(v, scope, diagnostics, fn_return_type);
            }
        }
        HirStmt::Continue(_) => {}
        HirStmt::Expr(e) => {
            check_expr(e, scope, diagnostics, fn_return_type);
        }
        HirStmt::Error => {}
    }
}

// ============================================================================
// 表达式类型检查
// ============================================================================

fn check_expr(
    expr: &mut HirExpr,
    scope: &Scope,
    diagnostics: &mut Vec<DiagError>,
    fn_return_type: &HirType,
) {
    match expr {
        // 字面量——类型已固定
        HirExpr::IntLiteral(v, sp) => {
            // v2.0 §2.2: 超 IEEE 754 安全整数范围发出 Warning
            // 注: 当前 trust_error 无 Warning 级别，使用 DiagError 占位；
            // 完整 Warning+Help 子诊断需 trust_error Diagnostic 扩展（归后续 Phase）
            const MAX_SAFE_INTEGER: f64 = 9007199254740992.0;
            if v.abs() > MAX_SAFE_INTEGER {
                diagnostics.push(DiagError::new(
                    format!("warning: integer literal `{v}` exceeds IEEE 754 safe integer range (±2^53); precision may be lost"),
                    sp.clone(),
                ));
            }
        }
        HirExpr::FloatLiteral(..) => {}
        HirExpr::StringLiteral(..) => {}
        HirExpr::BoolLiteral(..) => {}
        HirExpr::Null(..) => {} // v2.0: lexer→AST→HIR path, full type check → Phase 4
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
            check_expr(lhs, scope, diagnostics, fn_return_type);
            check_expr(rhs, scope, diagnostics, fn_return_type);

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

        HirExpr::Unary(op, inner, ref mut ty, _span) => {
            check_expr(inner, scope, diagnostics, fn_return_type);
            let inner_ty = expr_type(inner);
            *ty = match (op, &inner_ty) {
                (UnaryOp::Neg, HirType::Number) => inner_ty.clone(),
                (UnaryOp::Neg, HirType::Error) => HirType::Error,
                (UnaryOp::Neg, _) => {
                    diagnostics.push(DiagError::new(
                        format!("cannot negate type `{inner_ty}` (only numeric types allowed)"),
                        Span::dummy(),
                    ));
                    HirType::Error
                }
                (UnaryOp::Not, HirType::Bool) => HirType::Bool,
                (UnaryOp::Not, HirType::Error) => HirType::Error,
                (UnaryOp::Not, _) => {
                    diagnostics.push(DiagError::new(
                        format!("cannot apply `!` to type `{inner_ty}` (only booleans allowed)"),
                        Span::dummy(),
                    ));
                    HirType::Error
                }
            };
        }

        HirExpr::Call(callee, args, ref mut ty, span) => {
            check_expr(callee, scope, diagnostics, fn_return_type);

            for arg in args.iter_mut() {
                check_expr(&mut arg.expr, scope, diagnostics, fn_return_type);
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
                    diagnostics.push(DiagError::new(
                        format!("cannot call value of type `{callee_ty}` (only functions are callable in Phase 1)"),
                        span.clone(),
                    ));
                    *ty = HirType::Error;
                    *expr = HirExpr::Error(span.clone());
                }
            }
        }

        HirExpr::ArrowFn(params, ret, body, _is_move, _span) => {
            // 闭包类型检查：推断返回类型
            // 使用 parent scope 以访问外部变量（与 name_res 行为一致）
            // §2.3: 使用箭头自身的返回类型检查 body，非外围函数返回类型
            let mut fn_scope = Scope::new_child(Box::new(scope.clone()));
            for p in params.iter() {
                fn_scope.insert(
                    &p.name,
                    HirBinding::LocalVar { ty: p.ty.clone(), mutable: false, span: p.span.clone() },
                );
            }
            check_block(body, &fn_scope, diagnostics, ret);

            // 从 body 推断返回类型
            if *ret == HirType::Error {
                *ret = infer_return_type(body);
            }
        }

        HirExpr::AsCast(inner, ref target_ty, span) => {
            check_expr(inner, scope, diagnostics, fn_return_type);
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
            check_expr(inner, scope, diagnostics, fn_return_type);
        }

        HirExpr::TemplateLiteral(parts, _span) => {
            for part in parts {
                if let HirTemplatePartKind::Expr(ref mut e) = part.kind {
                    check_expr(e, scope, diagnostics, fn_return_type);
                }
            }
        }

        // AC-SEM-002: if 表达式类型检查
        HirExpr::If(if_s, _span) => {
            check_expr(&mut if_s.condition, scope, diagnostics, fn_return_type);
            check_block(&mut if_s.then_branch, scope, diagnostics, fn_return_type);
            if let Some(ref mut else_b) = if_s.else_branch {
                check_block(else_b, scope, diagnostics, fn_return_type);
            }
        }

        // v2.0: Loop removed
        HirExpr::Block(b, _span) => {
            check_block(b, scope, diagnostics, fn_return_type);
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
                HirType::Number => Ok(lhs.clone()),
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
        // Eq/Ne 允许 Bool；Lt/Gt/Le/Ge 禁止 Bool（布尔值不可排序）
        BinOp::Eq | BinOp::Ne => {
            if *lhs != *rhs {
                diagnostics.push(DiagError::new(
                    format!("type mismatch: cannot compare `{lhs}` and `{rhs}` (operands must be same type)"),
                    span,
                ));
                return Err(());
            }
            Ok(HirType::Bool)
        }
        BinOp::Lt | BinOp::Gt | BinOp::Le | BinOp::Ge => {
            if *lhs != *rhs {
                diagnostics.push(DiagError::new(
                    format!("type mismatch: cannot compare `{lhs}` and `{rhs}` (operands must be same type)"),
                    span,
                ));
                return Err(());
            }
            // 禁止 Bool 排序比较
            if *lhs == HirType::Bool {
                diagnostics.push(DiagError::new(
                    "cannot compare booleans with ordering operators (use `==` or `!=`)"
                        .to_string(),
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

        // v2.0 §2.2: 位运算 — 操作数必须为 Number
        BinOp::BitAnd | BinOp::BitOr | BinOp::BitXor | BinOp::Shl | BinOp::Shr => {
            if *lhs != HirType::Number || *rhs != HirType::Number {
                diagnostics.push(DiagError::new(
                    format!("bitwise operators require `number` operands, got `{lhs}` and `{rhs}`"),
                    span,
                ));
                return Err(());
            }
            Ok(HirType::Number)
        }

        // ?? 运算符 — Phase 1 排除，不应到达此处
        BinOp::QuestionQuestion => {
            diagnostics.push(DiagError::new("`??` not supported in Phase 1".into(), span));
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
    // v2.0: number→number as 恒等变换，拒绝（无意义代码）
    // 注：此检查在 `src == target` 之前，因为 number 统一后 `Number == Number` 为恒等
    match (src, target) {
        // v2.0: number 统一——number→number as 恒等变换，拒绝无意义代码
        (HirType::Number, HirType::Number) => {
            diagnostics.push(DiagError::new(
                "`as` between number types is unnecessary — number is unified as f64".into(),
                span,
            ));
            false
        }
        // Bool → 数字禁止
        (HirType::Bool, HirType::Number) => {
            diagnostics.push(DiagError::new(format!("cannot cast `bool` to `{target}`"), span));
            false
        }

        // 数字 → Bool 禁止
        (HirType::Number, HirType::Bool) => {
            diagnostics.push(DiagError::new(format!("cannot cast `{src}` to `bool`"), span));
            false
        }

        // String → 数字禁止
        (HirType::String, HirType::Number | HirType::Bool) => {
            diagnostics.push(DiagError::new(format!("cannot cast `string` to `{target}`"), span));
            false
        }

        // 同类型 no-op（非 Number——Number 已在上面拒绝）
        _ if src == target => true,
        // 其余跨族转换禁止
        _ => {
            diagnostics.push(DiagError::new(format!("invalid cast: `{src}` to `{target}`"), span));
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
                format!("function expects {} arguments, got {}", param_types.len(), args.len()),
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
        diagnostics
            .push(DiagError::new(format!("cannot call non-function type `{func_type}`"), span));
        Err(())
    }
}

// ============================================================================
// 工具函数
// ============================================================================

fn expr_type(expr: &HirExpr) -> HirType {
    match expr {
        HirExpr::IntLiteral(..) => HirType::Number, // v2.0
        HirExpr::FloatLiteral(..) => HirType::Number,
        HirExpr::StringLiteral(..) => HirType::String,
        HirExpr::BoolLiteral(..) => HirType::Bool,
        HirExpr::Null(..) => HirType::Void, // v2.0: null placeholder, 完整类型归 Phase 4
        HirExpr::Ident(_, binding, _) => match binding {
            HirBinding::LocalVar { ty, .. } => ty.clone(),
            HirBinding::ModuleConst { ty, .. } => ty.clone(),
            HirBinding::ModuleShared { ty, .. } => ty.clone(),
            HirBinding::Function { param_types, return_type, .. } => {
                HirType::Function(param_types.clone(), Box::new(return_type.clone()))
            }
            HirBinding::Import { ty, .. } => ty.clone(),
            HirBinding::Unresolved { .. } => HirType::Error,
        },
        HirExpr::Binary(.., ty, _) => ty.clone(),
        HirExpr::Unary(.., ty, _) => ty.clone(),
        HirExpr::Call(.., ty, _) => ty.clone(),
        HirExpr::AsCast(_, ty, _) => ty.clone(),
        HirExpr::If(if_s, _) => infer_if_type(if_s),
        HirExpr::Block(block, _) => infer_block_type(block),
        HirExpr::ArrowFn(params, ret, ..) => {
            let param_types: Vec<HirType> = params.iter().map(|p| p.ty.clone()).collect();
            HirType::Function(param_types, Box::new(ret.clone()))
        }
        HirExpr::Reference(inner, _) => HirType::Ref(Box::new(expr_type(inner))),
        HirExpr::TemplateLiteral(..) => HirType::String,
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

// v2.0: collect_break_info retains for/while break analysis.
// Break values are no longer possible (loop removed, break value deprecated).
#[allow(dead_code, unused_variables, clippy::only_used_in_recursion)]
fn collect_break_info(block: &HirBlock, types: &mut Vec<HirType>, has_bare: &mut bool) {
    for stmt in &block.statements {
        match stmt {
            // v2.0: break value removed
            HirStmt::Break(..) => {}
            HirStmt::If(if_s) => {
                collect_break_info(&if_s.then_branch, types, has_bare);
                if let Some(ref else_b) = if_s.else_branch {
                    collect_break_info(else_b, types, has_bare);
                }
            }
            HirStmt::For(f) => collect_break_info(&f.body, types, has_bare),
            HirStmt::While(w) => collect_break_info(&w.body, types, has_bare),
            // v2.0: Loop removed
            _ => {}
        }
    }
}

fn infer_if_type(if_s: &HirIf) -> HirType {
    let then_ty = infer_block_type(&if_s.then_branch);
    let else_ty = if_s.else_branch.as_ref().map(infer_block_type).unwrap_or(HirType::Void);
    if then_ty == else_ty {
        then_ty
    } else {
        // Sentinel: Error type in either branch, or incompatible types
        HirType::Error
    }
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
    fn check_binary_number_plus_number_ok() {
        let mut diags = vec![];
        let r = check_binary_op(
            BinOp::Add,
            &HirType::Number,
            &HirType::Number,
            Span::dummy(),
            &mut diags,
        );
        assert!(r.is_ok(), "number + number should be allowed (v2.0 number=f64)");
    }

    // AC-TYP-002/003: as 转换通过
    #[test]
    fn check_as_number_to_number_rejected() {
        let mut diags = vec![];
        let ok = check_as_cast(&HirType::Number, &HirType::Number, Span::dummy(), &mut diags);
        assert!(!ok, "number as number should be rejected (v2.0 identity cast)");
    }

    #[test]
    fn check_as_bool_to_number_forbidden() {
        let mut diags = vec![];
        let ok = check_as_cast(&HirType::Bool, &HirType::Number, Span::dummy(), &mut diags);
        assert!(!ok, "bool as i32 should be forbidden");
    }

    #[test]
    fn check_as_string_to_i32_forbidden() {
        let mut diags = vec![];
        let ok = check_as_cast(&HirType::String, &HirType::Number, Span::dummy(), &mut diags);
        assert!(!ok, "string as i32 should be forbidden");
    }

    // TY-RULE-01: I32+I32 → I32
    #[test]
    fn check_binary_i32_plus_i32_ok() {
        let mut diags = vec![];
        let r = check_binary_op(
            BinOp::Add,
            &HirType::Number,
            &HirType::Number,
            Span::dummy(),
            &mut diags,
        );
        assert_eq!(r, Ok(HirType::Number));
        assert!(diags.is_empty());
    }

    // TY-RULE-01: F64+F64 → F64
    #[test]
    fn check_binary_f64_plus_f64_ok() {
        let mut diags = vec![];
        let r = check_binary_op(
            BinOp::Add,
            &HirType::Number,
            &HirType::Number,
            Span::dummy(),
            &mut diags,
        );
        assert_eq!(r, Ok(HirType::Number));
    }

    // TY-RULE-02: I32 == I32 → Bool
    #[test]
    fn check_binary_eq_i32_returns_bool() {
        let mut diags = vec![];
        let r = check_binary_op(
            BinOp::Eq,
            &HirType::Number,
            &HirType::Number,
            Span::dummy(),
            &mut diags,
        );
        assert_eq!(r, Ok(HirType::Bool));
    }

    // TY-RULE-03: Bool && Bool → Bool
    #[test]
    fn check_binary_and_bool_returns_bool() {
        let mut diags = vec![];
        let r =
            check_binary_op(BinOp::And, &HirType::Bool, &HirType::Bool, Span::dummy(), &mut diags);
        assert_eq!(r, Ok(HirType::Bool));
    }

    // TY-RULE-03: I32 && I32 → 错误
    #[test]
    fn check_binary_and_i32_error() {
        let mut diags = vec![];
        let r = check_binary_op(
            BinOp::And,
            &HirType::Number,
            &HirType::Number,
            Span::dummy(),
            &mut diags,
        );
        assert!(r.is_err());
    }

    // 函数调用签名验证
    #[test]
    fn check_call_matching_params_returns_ret_type() {
        let func_ty =
            HirType::Function(vec![HirType::Number, HirType::Number], Box::new(HirType::Bool));
        let args = vec![
            HirCallArg {
                mode: ParamMode::Default,
                expr: Box::new(HirExpr::IntLiteral(1.0, Span::dummy())),
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
        let func_ty = HirType::Function(vec![HirType::Number], Box::new(HirType::Void));
        let args = vec![];
        let mut diags = vec![];
        let r = check_call("f", &func_ty, &args, Span::dummy(), &mut diags);
        assert!(r.is_err());
    }

    #[test]
    fn check_call_wrong_arg_type_error() {
        let func_ty = HirType::Function(vec![HirType::Number], Box::new(HirType::Void));
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
            &HirType::Number,
            Span::dummy(),
            &mut diags,
        );
        assert!(r.is_err()); // Error 哨兵仍返回 Err
    }
}
