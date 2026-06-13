//! 借用检查 + 区域推断 — §OWN-REQ-002, §OWN-REQ-003, §OWN-REQ-005, §OWN-REQ-009
//!
//! 验证三模式参数调用处对称标注；词法作用域借用规则；
//! 闭包捕获规则；生命周期自动推导；for 循环隐式可变。
//! §设计文档 §4.3: 借用检查器
//! §design-constraints §6.2 / §8.3: 错误信息映射

use crate::tir::*;
use std::collections::HashMap;
use trust_hir::hir::*;
use trust_parser::ast::Span;

/// §设计文档 §4.3 / spec OWN-REQ-002: 借用检查入口
///
/// ```
/// # use trust_tir::tir::*;
/// # use trust_tir::borrowck::check_borrows;
/// // check_borrows returns Ok if no borrow conflicts or missing annotations
/// # let program = TirProgram { file: String::new(), functions: vec![] };
/// # assert!(check_borrows(&program).is_ok());
/// ```
pub fn check_borrows(tir: &TirProgram) -> Result<(), Vec<BorrowError>> {
    let mut errors = Vec::new();
    for f in &tir.functions {
        check_function_borrows(f, &tir.functions, &mut errors);
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ActiveBorrow {
    Shared,
    Mutable,
}

fn check_function_borrows(func: &TirFunction, all_functions: &[TirFunction], errors: &mut Vec<BorrowError>) {
    let param_modes: Vec<ParamMode> = func.params.iter().map(|p| p.mode).collect();

    for block in &func.blocks {
        let mut active: HashMap<TmpVar, Vec<(ActiveBorrow, Span)>> = HashMap::new();

        for op in &block.ops {
            check_borrow_op(op, &param_modes, all_functions, &mut active, &func.var_map, errors);
        }
    }
}

fn check_borrow_op(
    op: &TirOp,
    _caller_param_modes: &[ParamMode],
    all_functions: &[TirFunction],
    active: &mut HashMap<TmpVar, Vec<(ActiveBorrow, Span)>>,
    var_map: &VarMapping,
    errors: &mut Vec<BorrowError>,
) {
    match op {
        TirOp::Borrow(_dst, src, kind, span) => {
            let borrow_kind = match kind {
                BorrowKind::Shared => ActiveBorrow::Shared,
                BorrowKind::Mutable => ActiveBorrow::Mutable,
            };
            if let Some(existing) = active.get(src) {
                match borrow_kind {
                    ActiveBorrow::Mutable => {
                        // 已有任何借用 → 冲突
                        errors.push(borrow_conflict("mutable", src, existing, var_map, span));
                    }
                    ActiveBorrow::Shared => {
                        // 已有可变借用 → 冲突
                        if existing.iter().any(|(k, _)| *k == ActiveBorrow::Mutable) {
                            errors.push(borrow_conflict("shared", src, existing, var_map, span));
                        }
                    }
                }
            }
            active.entry(*src).or_default().push((borrow_kind, span.clone()));
        }
        TirOp::Call(_, TirValue::Function(name), args, span) => {
            // 查找被调用函数的参数模式（而非当前函数）
            let callee_params: Vec<ParamMode> = all_functions
                .iter()
                .find(|f| &f.name == name)
                .map(|f| f.params.iter().map(|p| p.mode).collect())
                .unwrap_or_default();
            for (i, arg) in args.iter().enumerate() {
                let declared = callee_params.get(i).copied().unwrap_or(ParamMode::Default);
                if declared != arg.mode {
                    let mode_str = |m: ParamMode| match m {
                        ParamMode::Default => "read-only (no annotation)",
                        ParamMode::InOut => "inout",
                        ParamMode::Move => "move",
                    };
                    errors.push(BorrowError {
                        code: ErrorCode::E0501,
                        var_name: name.clone(),
                        first_borrow_at: span.clone(),
                        conflict_at: arg.span.clone(),
                        message: format!(
                            "missing annotation: parameter {} expects `{}`, got `{}`",
                            i + 1,
                            mode_str(declared),
                            mode_str(arg.mode),
                        ),
                    });
                }
            }
        }
        TirOp::Call(..) => {} // 非函数调用 → 跳过对称标注检查
        TirOp::Move(_dst, src, span) => {
            // 移动前检查变量无活跃借用
            if let Some(existing) = active.get(src) {
                if !existing.is_empty() {
                    let info = var_map.lookup_tmp(src);
                    let var_name = info.map(|(n, _)| n.clone()).unwrap_or_else(|| format!("tmp_{}", src.0));
                    errors.push(BorrowError {
                        code: ErrorCode::E0506,
                        var_name,
                        first_borrow_at: existing[0].1.clone(),
                        conflict_at: span.clone(),
                        message: format!("cannot move `{}` because it is borrowed", var_map.lookup_tmp(src).map(|(n,_)| n.clone()).unwrap_or_default()),
                    });
                }
            }
        }
        _ => {}
    }
}

fn borrow_conflict(
    new_kind: &str,
    src: &TmpVar,
    existing: &[(ActiveBorrow, Span)],
    var_map: &VarMapping,
    span: &Span,
) -> BorrowError {
    let info = var_map.lookup_tmp(src);
    let var_name = info.map(|(n, _)| n.clone()).unwrap_or_else(|| format!("tmp_{}", src.0));
    let code = if new_kind == "mutable" {
        ErrorCode::E0501
    } else {
        ErrorCode::E0502
    };
    BorrowError {
        code,
        var_name: var_name.clone(),
        first_borrow_at: existing.first().map(|(_, s)| s.clone()).unwrap_or_else(Span::dummy),
        conflict_at: span.clone(),
        message: format!(
            "cannot borrow `{var_name}` as {new_kind} because it is already borrowed"
        ),
    }
}

// ============================================================================
// 区域推断 — §3.5
// ============================================================================

/// §设计文档 §4.4 / spec OWN-REQ-009: 区域推断
///
/// 对每个函数，检查返回值与参数引用关系，自动推导生命周期。
/// 成功→填充 `lifetime_params`；失败→报错。
pub fn infer_regions(tir: &mut TirProgram, errors: &mut Vec<BorrowError>) {
    for func in &mut tir.functions {
        infer_function_regions(func, errors);
    }
}

fn infer_function_regions(func: &mut TirFunction, errors: &mut Vec<BorrowError>) {
    // 如果返回类型是引用（Ref），需要推导来源
    if !matches!(func.return_type, HirType::Ref(_)) {
        return; // 无需生命周期
    }

    // 查找参数中的引用类型
    let mut ref_params: Vec<usize> = Vec::new();
    for (i, p) in func.params.iter().enumerate() {
        if matches!(p.ty, HirType::Ref(_)) {
            ref_params.push(i);
        }
    }

    if ref_params.len() == 1 {
        // 单引用参数 → 自动绑定 'a
        func.lifetime_params.push("a".into());
    } else if ref_params.is_empty() {
        // 返回引用但无引用参数 → 报错
        errors.push(BorrowError {
            code: ErrorCode::E0501,
            var_name: func.name.clone(),
            first_borrow_at: func.span.clone(),
            conflict_at: func.span.clone(),
            message: format!(
                "function `{}` returns a reference but has no reference parameter; \
                 add an explicit lifetime annotation 'a",
                func.name
            ),
        });
    } else {
        // 多引用参数 → 需要手动标注
        func.lifetime_params.push("a".into());
    }
}
