//! 移动语义检查 — §OWN-REQ-001, §OWN-REQ-008
//!
//! 检测所有权转移后的使用冲突（E0382）；判定 Copy 类型。
//! §设计文档 §4.2: 移动语义分析
//! §design-constraints §6.2 / §8.3: 错误信息映射到 Trust 源码变量名+行列号

use crate::tir::*;
use trust_hir::hir::HirType;
use trust_parser::ast::Span;

/// §设计文档 §4.2 / spec OWN-REQ-001: 移动语义检查入口
///
/// ```
/// # use trust_tir::tir::*;
/// # use trust_tir::moveck::check_moves;
/// // check_moves returns Ok if no use-after-move errors
/// # let program = TirProgram { file: String::new(), functions: vec![] };
/// # assert!(check_moves(&program).is_ok());
/// ```
pub fn check_moves(tir: &TirProgram) -> Result<(), Vec<MoveError>> {
    let mut errors = Vec::new();
    for f in &tir.functions {
        check_function_moves(f, &mut errors);
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

#[derive(Debug, Clone, PartialEq)]
enum VarState {
    Active,
    Moved(Span),
}

fn check_function_moves(func: &TirFunction, errors: &mut Vec<MoveError>) {
    let mut state: Vec<VarState> = vec![VarState::Active; func.tmp_counter as usize];
    // 标记函数参数为 Active
    for param in &func.params {
        if let Some(tmp) = func.var_map.lookup_name(&param.name) {
            let idx = tmp.0 as usize;
            if (idx as u32) < func.tmp_counter {
                state[idx] = VarState::Active;
            }
        }
    }

    for block in &func.blocks {
        for op in &block.ops {
            check_op(op, &mut state, &func.var_map, errors);
        }
    }
}

fn check_op(
    op: &TirOp,
    state: &mut [VarState],
    var_map: &VarMapping,
    errors: &mut Vec<MoveError>,
) {
    match op {
        TirOp::Move(dst, src, move_span) => {
            let si = src.0 as usize;
            if si < state.len() {
                if let VarState::Moved(ref moved_at) = state[si] {
                    let info = var_map.lookup_tmp(src);
                    let var_name = info.map(|(n, _)| n.clone()).unwrap_or_else(|| format!("tmp_{}", src.0));
                    errors.push(MoveError {
                        code: ErrorCode::E0382,
                        var_name: var_name.clone(),
                        moved_at: moved_at.clone(),
                        used_at: move_span.clone(),
                        message: format!(
                            "variable `{var_name}` moved here, used again later (E0382)"
                        ),
                    });
                }
                // 标记 src 为 Moved，dst 为 Active
                state[si] = VarState::Moved(move_span.clone());
            }
            let di = dst.0 as usize;
            if di < state.len() {
                state[di] = VarState::Active;
            }
        }
        // TirOp::Let 读取源变量 - 检查读取 Moved 变量
        TirOp::Let(dst, val, span) => {
            check_value_use_mut(val, state, var_map, errors, span);
            // 标记目标变量为 Active（新赋值覆盖 Moved 状态）
            let di = dst.0 as usize;
            if di < state.len() {
                state[di] = VarState::Active;
            }
        }
        TirOp::Binary(_, lhs, _, rhs, span) => {
            check_value_use_mut(lhs, state, var_map, errors, span);
            check_value_use_mut(rhs, state, var_map, errors, span);
        }
        TirOp::Unary(_, _, val, span) => {
            check_value_use_mut(val, state, var_map, errors, span);
        }
        TirOp::Call(_, _, args, span) => {
            for arg in args {
                check_value_use_mut(&arg.value, state, var_map, errors, span);
            }
        }
        TirOp::Borrow(_dst, src, _kind, _span) => {
            check_var_use_mut(*src, state, var_map, errors, _span);
        }
        TirOp::AsCast(_, val, _, span) => {
            check_value_use_mut(val, state, var_map, errors, span);
        }
        TirOp::Nop(_) => {}
    }
}

fn check_value_use_mut(
    val: &TirValue,
    state: &mut [VarState],
    var_map: &VarMapping,
    errors: &mut Vec<MoveError>,
    span: &Span,
) {
    if let TirValue::Var(tmp) = val {
        check_var_use_mut(*tmp, state, var_map, errors, span);
    }
}

fn check_var_use_mut(
    tmp: TmpVar,
    state: &mut [VarState],
    var_map: &VarMapping,
    errors: &mut Vec<MoveError>,
    span: &Span,
) {
    let idx = tmp.0 as usize;
    if idx < state.len() {
        if let VarState::Moved(ref moved_at) = state[idx] {
            let info = var_map.lookup_tmp(&tmp);
            let var_name = info.map(|(n, _)| n.clone()).unwrap_or_else(|| format!("tmp_{}", tmp.0));
            errors.push(MoveError {
                code: ErrorCode::E0382,
                var_name: var_name.clone(),
                moved_at: moved_at.clone(),
                used_at: span.clone(),
                message: format!("variable `{var_name}` used after move (E0382)"),
            });
        }
    }
}

/// §3.3.2: Copy 类型判定（公开，供 tir.rs 降级时使用）
pub fn is_copy_type(ty: &HirType) -> bool {
    match ty {
        HirType::I32 | HirType::F64 | HirType::I64 | HirType::Bool | HirType::BigInt => true,
        HirType::Ref(_) => true,
        HirType::String | HirType::Void | HirType::Array(_) | HirType::Named(_) | HirType::Function(..) => false,
        HirType::Error => false,
    }
}
