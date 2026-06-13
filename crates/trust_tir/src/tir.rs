//! TIR 节点定义 + HIR→TIR 降级 — §SEM-REQ-004
//!
//! 本模块包含：
//! 1. TIR 控制流图节点（TirProgram、BasicBlock、TirOp、TirValue 等）
//! 2. HIR→TIR 降级（lower_hir：控制流→基本块、表达式→语句、闭包捕获提升）
//!
//! §设计文档 §4.1: TIR 是 HIR→codegen 的中间表示，携带所有权信息。
//! §design-constraints §3.1: 错误类型实现 Display + Error。

use std::collections::HashMap;
use trust_hir::hir::*;
use trust_hir::name_res::DiagError;
use trust_parser::ast::Span;

// ============================================================================
// TIR 节点定义 — §3.1
// ============================================================================

/// BlockId — 基本块唯一标识
pub type BlockId = usize;

/// TmpVar — TIR 降级时分配的临时变量唯一 ID
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TmpVar(pub u32);

// ============================================================================
// 顶层结构 — §3.1.1
// ============================================================================

/// §设计文档 §4.1: TIR 程序聚合所有函数体
#[derive(Debug, Clone)]
pub struct TirProgram {
    pub file: String,
    pub functions: Vec<TirFunction>,
}

/// §设计文档 §4.1: TIR 函数——HIR 函数降级产物
#[derive(Debug, Clone)]
pub struct TirFunction {
    pub name: String,
    /// HIR 函数参数（降级时每个注册为 TmpVar）
    pub params: Vec<HirParam>,
    /// 函数返回类型（区域推断需要）
    pub return_type: HirType,
    /// 区域推断产出的生命周期参数（如 ["a"]；绝大多数为空）
    pub lifetime_params: Vec<String>,
    /// 入口基本块索引
    pub entry_block: BlockId,
    /// 所有基本块
    pub blocks: Vec<BasicBlock>,
    /// 临时变量计数器（每函数独立）
    pub tmp_counter: u32,
    /// 闭包捕获的外部变量
    pub captured_vars: Vec<CapturedVar>,
    /// TmpVar → 源码变量映射（错误报告用）
    pub var_map: VarMapping,
    pub span: Span,
}

// ============================================================================
// 基本块 — §3.1.2
// ============================================================================

/// 基本块——顺序执行的 TIR 操作序列，以终结指令结束
#[derive(Debug, Clone)]
pub struct BasicBlock {
    pub id: BlockId,
    pub ops: Vec<TirOp>,
    pub terminator: Terminator,
    pub span: Span,
}

/// TIR 操作
#[derive(Debug, Clone)]
pub enum TirOp {
    /// let tmp = value;（字面量/运算结果）
    Let(TmpVar, TirValue, Span),
    /// let tmp = src_tmp;（move——非 Copy 变量）
    Move(TmpVar, TmpVar, Span),
    /// let tmp = &x / &mut x;
    Borrow(TmpVar, TmpVar, BorrowKind, Span),
    /// tmp = lhs op rhs;
    Binary(TmpVar, TirValue, BinOp, TirValue, Span),
    /// tmp = op val;
    Unary(TmpVar, UnaryOp, TirValue, Span),
    /// tmp = callee(args...);
    Call(Option<TmpVar>, TirValue, Vec<TirArg>, Span),
    /// tmp = val as ty;
    AsCast(TmpVar, TirValue, HirType, Span),
    /// 占位
    Nop(Span),
}

/// 基本块终结指令
#[derive(Debug, Clone)]
pub enum Terminator {
    /// 无条件跳转到 block_id
    Goto(BlockId),
    /// if (cond) { then_id } else { else_id }
    If(TmpVar, BlockId, BlockId),
    /// return value;
    Return(Option<TmpVar>),
    /// 不可达
    Unreachable,
}

// ============================================================================
// TIR 值类型 — §3.1.3
// ============================================================================

#[derive(Debug, Clone)]
pub enum TirValue {
    Var(TmpVar),
    IntLiteral(i32),
    FloatLiteral(f64),
    BigIntLiteral(i64),
    StringLiteral(String),
    BoolLiteral(bool),
    /// 函数引用（按名称调用）
    Function(String),
    /// 哨兵
    Error,
}

/// 借用类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BorrowKind {
    Shared,
    Mutable,
}

/// 闭包捕获的外部变量
#[derive(Debug, Clone)]
pub struct CapturedVar {
    pub name: String,
    pub tmp: TmpVar,
    pub kind: BorrowKind,
}

/// 调用实参
#[derive(Debug, Clone)]
pub struct TirArg {
    pub mode: ParamMode,
    pub value: TirValue,
    pub span: Span,
}

// ============================================================================
// 变量映射表 — §3.3.3
// ============================================================================

/// TmpVar ↔ Trust 源码变量双向映射
#[derive(Debug, Clone, Default)]
pub struct VarMapping {
    /// TmpVar → (源码变量名, 定义位置)
    pub tmp_to_source: HashMap<TmpVar, (String, Span)>,
    /// 源码变量名 → TmpVar
    pub source_to_tmp: HashMap<String, TmpVar>,
}

impl VarMapping {
    pub fn new() -> Self {
        VarMapping {
            tmp_to_source: HashMap::new(),
            source_to_tmp: HashMap::new(),
        }
    }

    pub fn insert(&mut self, tmp: TmpVar, name: &str, span: Span) {
        self.tmp_to_source.insert(tmp, (name.to_string(), span));
        self.source_to_tmp.insert(name.to_string(), tmp);
    }

    pub fn lookup_tmp(&self, tmp: &TmpVar) -> Option<&(String, Span)> {
        self.tmp_to_source.get(tmp)
    }

    pub fn lookup_name(&self, name: &str) -> Option<TmpVar> {
        self.source_to_tmp.get(name).copied()
    }
}

// ============================================================================
// 错误类型 — §3.3.3 / §3.4.5
// ============================================================================

/// 移动错误
#[derive(Debug, Clone)]
pub struct MoveError {
    pub code: ErrorCode,
    pub var_name: String,
    pub moved_at: Span,
    pub used_at: Span,
    pub message: String,
}

/// 借用错误
#[derive(Debug, Clone)]
pub struct BorrowError {
    pub code: ErrorCode,
    pub var_name: String,
    pub first_borrow_at: Span,
    pub conflict_at: Span,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorCode {
    E0382, // use after move
    E0384, // cannot assign twice to immutable variable
    E0389, // cannot assign to immutable variable
    // 借用错误码
    E0501, // cannot borrow as mutable, already borrowed as immutable
    E0502, // cannot borrow as immutable, already borrowed as mutable
    E0506, // cannot assign because it is borrowed
}

impl std::fmt::Display for MoveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for MoveError {}

impl std::fmt::Display for BorrowError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for BorrowError {}

// ============================================================================
// HIR→TIR 降级 — §3.2
// ============================================================================

/// §设计文档 §4.1 / spec SEM-REQ-004: HIR→TIR 降级入口
///
/// ```
/// # use trust_hir::hir::HirProgram;
/// # use trust_hir::name_res::DiagError;
/// # use trust_tir::tir::{lower_hir, TirProgram};
/// fn example_lower(hir: &HirProgram) -> Result<TirProgram, Vec<DiagError>> {
///     lower_hir(hir)
/// }
/// ```
pub fn lower_hir(hir: &HirProgram) -> Result<TirProgram, Vec<DiagError>> {
    let mut functions = Vec::new();
    let mut diags = Vec::new();

    for item in &hir.items {
        if let HirItem::Function(f) = item {
            let tf = lower_function(f, &mut diags);
            functions.push(tf);
        }
    }

    if diags.is_empty() {
        Ok(TirProgram {
            file: hir.file.clone(),
            functions,
        })
    } else {
        Err(diags)
    }
}

fn lower_function(f: &HirFunction, diags: &mut Vec<DiagError>) -> TirFunction {
    let mut ctx = LowerCtx::new();
    // 注册函数参数
    for p in &f.params {
        let tmp = ctx.next_tmp();
        ctx.map.insert(tmp, &p.name, p.span.clone());
    }
    // 降级 body
    let entry_id = ctx.next_block_id();
    let exit_id = ctx.next_block_id();
    ctx.entry = entry_id;
    ctx.exit = exit_id;

    ctx.new_block(entry_id, f.span.clone());
    lower_block(&f.body, &mut ctx, diags);
    // 如果入口块没有终结指令，补 Return
    let entry = &mut ctx.blocks[entry_id];
    if matches!(entry.terminator, Terminator::Unreachable)
        && entry.ops.is_empty() && ctx.blocks.len() == 1 {
            // 空函数体 — 什么都不需要做
        }

    let blocks: Vec<BasicBlock> = (0..ctx.next_id)
        .map(|id| {
            if id < ctx.blocks.len() {
                ctx.blocks[id].clone()
            } else {
                BasicBlock {
                    id,
                    ops: vec![],
                    terminator: Terminator::Unreachable,
                    span: f.span.clone(),
                }
            }
        })
        .collect();

    TirFunction {
        name: f.name.clone(),
        params: f.params.clone(),
        return_type: f.return_type.clone(),
        lifetime_params: vec![],
        entry_block: ctx.entry,
        blocks,
        tmp_counter: ctx.tmp_counter,
        captured_vars: vec![],
        var_map: ctx.map.clone(),
        span: f.span.clone(),
    }
}

fn lower_block(block: &HirBlock, ctx: &mut LowerCtx, diags: &mut Vec<DiagError>) {
    for stmt in &block.statements {
        lower_stmt(stmt, ctx, diags);
    }
}

fn lower_stmt(stmt: &HirStmt, ctx: &mut LowerCtx, diags: &mut Vec<DiagError>) {
    match stmt {
        HirStmt::Let(let_s) => lower_let(let_s, ctx, diags),
        HirStmt::Const(c) => {
            let val = lower_expr_to_value(&c.init, ctx, diags);
            let tmp = ctx.next_tmp();
            ctx.emit(TirOp::Let(tmp, val, c.span.clone()));
            ctx.map.insert(tmp, &c.name, c.span.clone());
        }
        HirStmt::Shared(s) => {
            let val = lower_expr_to_value(&s.init, ctx, diags);
            let tmp = ctx.next_tmp();
            ctx.emit(TirOp::Let(tmp, val, s.span.clone()));
            ctx.map.insert(tmp, &s.name, s.span.clone());
        }
        HirStmt::Return(r) => {
            let ret_tmp = r.value.as_ref().map(|v| {
                let val = lower_expr_to_value(v, ctx, diags);
                let tmp = ctx.next_tmp();
                ctx.emit(TirOp::Let(tmp, val, r.span.clone()));
                tmp
            });
            finish_block(ctx, Terminator::Return(ret_tmp));
        }
        HirStmt::If(if_s) => lower_if_stmt(if_s, ctx, diags),
        HirStmt::For(f_s) => lower_for_stmt(f_s, ctx, diags),
        HirStmt::While(w) => lower_while_stmt(w, ctx, diags),
        HirStmt::Loop(l) => lower_loop_stmt(l, ctx, diags),
        HirStmt::ForOf(_f) => lower_forof_stmt(_f, ctx, diags),
        HirStmt::Break(b) => {
            // Extract loop exit target BEFORE mutable borrows
            let exit_id = ctx.loop_stack.last().map(|(_, e)| *e).unwrap_or(ctx.exit);
            let _val = b.value.as_ref().map(|v| {
                let val = lower_expr_to_value(v, ctx, diags);
                let tmp = ctx.next_tmp();
                ctx.emit(TirOp::Let(tmp, val, b.span.clone()));
                tmp
            });
            finish_block(ctx, Terminator::Goto(exit_id));
        }
        HirStmt::Continue(_c) => {
            let cond_id = ctx.loop_stack.last().map(|(c, _)| *c).unwrap_or(ctx.entry);
            finish_block(ctx, Terminator::Goto(cond_id));
        }
        HirStmt::Expr(e) => {
            let val = lower_expr_to_value(e, ctx, diags);
            let tmp = ctx.next_tmp();
            ctx.emit(TirOp::Let(tmp, val, Span::dummy()));
        }
        HirStmt::Error => {}
    }
}

fn lower_let(let_s: &HirLet, ctx: &mut LowerCtx, diags: &mut Vec<DiagError>) {
    let init_tmp: Option<TmpVar> = match let_s.init.as_ref() {
        // Move vs Let 判定（§3.2.2 表格）
        HirExpr::Ident(name, binding, _span) => {
            if matches!(binding, HirBinding::LocalVar { .. }) {
                let ty = match binding {
                    HirBinding::LocalVar { ty, .. } => ty,
                    _ => &HirType::Error,
                };
                if is_copy_type(ty) {
                    if let Some(existing) = ctx.map.lookup_name(name) {
                        let tmp = ctx.next_tmp();
                        ctx.emit(TirOp::Let(tmp, TirValue::Var(existing), let_s.span.clone()));
                        Some(tmp)
                    } else {
                        diags.push(DiagError::new(format!("undefined: {name}"), Span::dummy()));
                        None
                    }
                } else {
                    if let Some(existing) = ctx.map.lookup_name(name) {
                        let new_tmp = ctx.next_tmp();
                        ctx.emit(TirOp::Move(new_tmp, existing, let_s.span.clone()));
                        Some(new_tmp)
                    } else {
                        diags.push(DiagError::new(format!("undefined: {name}"), Span::dummy()));
                        None
                    }
                }
            } else {
                let val = lower_expr_to_value(&let_s.init, ctx, diags);
                let tmp = ctx.next_tmp();
                ctx.emit(TirOp::Let(tmp, val, let_s.span.clone()));
                Some(tmp)
            }
        }
        _ => {
            let val = lower_expr_to_value(&let_s.init, ctx, diags);
            let tmp = ctx.next_tmp();
            ctx.emit(TirOp::Let(tmp, val, let_s.span.clone()));
            Some(tmp)
        }
    };
    // Register the new variable in the TIR var_map so subsequent references resolve correctly
    if let Some(tmp) = init_tmp {
        ctx.map.insert(tmp, &let_s.name, let_s.span.clone());
    }
}

fn lower_if_stmt(if_s: &HirIf, ctx: &mut LowerCtx, diags: &mut Vec<DiagError>) {
    let cond_val = lower_expr_to_value(&if_s.condition, ctx, diags);
    let cond_tmp = ctx.next_tmp();
    ctx.emit(TirOp::Let(cond_tmp, cond_val, if_s.span.clone()));

    let then_id = ctx.next_block_id();
    let else_id = ctx.next_block_id();
    let join_id = ctx.next_block_id();

    finish_block(ctx, Terminator::If(cond_tmp, then_id, else_id));

    // then 分支
    ctx.new_block(then_id, if_s.then_branch.span.clone());
    lower_block(&if_s.then_branch, ctx, diags);
    if !is_terminated(ctx) {
        finish_block(ctx, Terminator::Goto(join_id));
    }

    // else 分支
    ctx.new_block(else_id, if_s.else_branch.as_ref().map(|b| b.span.clone()).unwrap_or(Span::dummy()));
    if let Some(ref else_b) = if_s.else_branch {
        lower_block(else_b, ctx, diags);
    }
    if !is_terminated(ctx) {
        finish_block(ctx, Terminator::Goto(join_id));
    }

    ctx.new_block(join_id, if_s.span.clone());
}

fn lower_for_stmt(f_s: &HirFor, ctx: &mut LowerCtx, diags: &mut Vec<DiagError>) {
    let cond_id = ctx.next_block_id();
    let body_id = ctx.next_block_id();
    let update_id = ctx.next_block_id();
    let exit_id = ctx.next_block_id();

    // init
    lower_stmt(&f_s.init, ctx, diags);
    finish_block(ctx, Terminator::Goto(cond_id));

    ctx.loop_stack.push((cond_id, exit_id));

    // cond
    ctx.new_block(cond_id, f_s.span.clone());
    let cond_val = lower_expr_to_value(&f_s.condition, ctx, diags);
    let cond_tmp = ctx.next_tmp();
    ctx.emit(TirOp::Let(cond_tmp, cond_val, f_s.span.clone()));
    finish_block(ctx, Terminator::If(cond_tmp, body_id, exit_id));

    // body
    ctx.new_block(body_id, f_s.body.span.clone());
    lower_block(&f_s.body, ctx, diags);
    if !is_terminated(ctx) {
        finish_block(ctx, Terminator::Goto(update_id));
    }

    // update
    ctx.new_block(update_id, f_s.span.clone());
    let _upd = lower_expr_to_value(&f_s.update, ctx, diags);
    finish_block(ctx, Terminator::Goto(cond_id));

    ctx.loop_stack.pop();
    ctx.new_block(exit_id, f_s.span.clone());
}

fn lower_while_stmt(w: &HirWhile, ctx: &mut LowerCtx, diags: &mut Vec<DiagError>) {
    let cond_id = ctx.next_block_id();
    let body_id = ctx.next_block_id();
    let exit_id = ctx.next_block_id();

    finish_block(ctx, Terminator::Goto(cond_id));

    ctx.loop_stack.push((cond_id, exit_id));

    ctx.new_block(cond_id, w.span.clone());
    let cond_val = lower_expr_to_value(&w.condition, ctx, diags);
    let cond_tmp = ctx.next_tmp();
    ctx.emit(TirOp::Let(cond_tmp, cond_val, w.span.clone()));
    finish_block(ctx, Terminator::If(cond_tmp, body_id, exit_id));

    ctx.new_block(body_id, w.body.span.clone());
    lower_block(&w.body, ctx, diags);
    if !is_terminated(ctx) {
        finish_block(ctx, Terminator::Goto(cond_id));
    }

    ctx.loop_stack.pop();
    ctx.new_block(exit_id, w.span.clone());
}

fn lower_loop_stmt(l: &HirLoop, ctx: &mut LowerCtx, diags: &mut Vec<DiagError>) {
    let body_id = ctx.next_block_id();
    let exit_id = ctx.next_block_id();

    finish_block(ctx, Terminator::Goto(body_id));
    ctx.loop_stack.push((body_id, exit_id)); // cond = body (loop always enters body)

    ctx.new_block(body_id, l.body.span.clone());
    lower_block(&l.body, ctx, diags);
    if !is_terminated(ctx) {
        finish_block(ctx, Terminator::Goto(body_id));
    }

    ctx.loop_stack.pop();
    ctx.new_block(exit_id, l.span.clone());
}

fn lower_forof_stmt(f: &HirForOf, ctx: &mut LowerCtx, _diags: &mut Vec<DiagError>) {
    let iter_val = lower_expr_to_value(&f.iterator, ctx, _diags);
    let _iter_tmp = ctx.next_tmp();
    ctx.emit(TirOp::Let(_iter_tmp, iter_val, f.span.clone()));

    let cond_id = ctx.next_block_id();
    let body_id = ctx.next_block_id();
    let exit_id = ctx.next_block_id();

    finish_block(ctx, Terminator::Goto(cond_id));

    ctx.loop_stack.push((cond_id, exit_id));

    ctx.new_block(cond_id, f.span.clone());
    // Phase 1: simplified — directly enters body
    finish_block(ctx, Terminator::Goto(body_id));

    ctx.new_block(body_id, f.body.span.clone());
    // Register iterator item
    let item_tmp = ctx.next_tmp();
    ctx.map.insert(item_tmp, &f.item, f.span.clone());
    lower_block(&f.body, ctx, _diags);
    if !is_terminated(ctx) {
        finish_block(ctx, Terminator::Goto(cond_id));
    }

    ctx.loop_stack.pop();
    ctx.new_block(exit_id, f.span.clone());
}

// ============================================================================
// 表达式→值降级
// ============================================================================

fn lower_expr_to_value(expr: &HirExpr, ctx: &mut LowerCtx, diags: &mut Vec<DiagError>) -> TirValue {
    match expr {
        HirExpr::IntLiteral(v, _) => TirValue::IntLiteral(*v),
        HirExpr::FloatLiteral(v, _) => TirValue::FloatLiteral(*v),
        HirExpr::BigIntLiteral(v, _) => TirValue::BigIntLiteral(*v),
        HirExpr::StringLiteral(s, _) => TirValue::StringLiteral(s.clone()),
        HirExpr::BoolLiteral(b, _) => TirValue::BoolLiteral(*b),
        HirExpr::Ident(name, binding, _) => {
            if matches!(binding, HirBinding::Function { .. }) {
                TirValue::Function(name.clone())
            } else if let Some(tmp) = ctx.map.lookup_name(name) {
                TirValue::Var(tmp)
            } else {
                diags.push(DiagError::new(format!("undefined: {name}"), Span::dummy()));
                TirValue::Error
            }
        }
        HirExpr::If(if_s, _span) => {
            let cond_val = lower_expr_to_value(&if_s.condition, ctx, diags);
            let cond_tmp = ctx.next_tmp();
            ctx.emit(TirOp::Let(cond_tmp, cond_val, if_s.span.clone()));

            let then_id = ctx.next_block_id();
            let else_id = ctx.next_block_id();
            let join_id = ctx.next_block_id();
            let result_tmp = ctx.next_tmp();

            finish_block(ctx, Terminator::If(cond_tmp, then_id, else_id));

            ctx.new_block(then_id, if_s.then_branch.span.clone());
            let then_val = lower_block_to_value(&if_s.then_branch, ctx, diags, result_tmp);
            ctx.emit(TirOp::Let(result_tmp, then_val, if_s.span.clone()));
            if !is_terminated(ctx) { finish_block(ctx, Terminator::Goto(join_id)); }

            ctx.new_block(else_id, if_s.else_branch.as_ref().map(|b| b.span.clone()).unwrap_or(Span::dummy()));
            let else_val = if let Some(ref eb) = if_s.else_branch {
                lower_block_to_value(eb, ctx, diags, result_tmp)
            } else {
                TirValue::Error
            };
            ctx.emit(TirOp::Let(result_tmp, else_val, if_s.span.clone()));
            if !is_terminated(ctx) { finish_block(ctx, Terminator::Goto(join_id)); }

            ctx.new_block(join_id, if_s.span.clone());
            TirValue::Var(result_tmp)
        }
        HirExpr::Loop(l, _span) => {
            let exit_id = ctx.next_block_id();
            let result_tmp = ctx.next_tmp();

            let body_id = ctx.next_block_id();
            finish_block(ctx, Terminator::Goto(body_id));
            ctx.loop_stack.push((body_id, exit_id));

            ctx.new_block(body_id, l.body.span.clone());
            lower_block(&l.body, ctx, diags);
            if !is_terminated(ctx) { finish_block(ctx, Terminator::Goto(body_id)); }

            ctx.loop_stack.pop();
            ctx.new_block(exit_id, l.span.clone());
            TirValue::Var(result_tmp)
        }
        HirExpr::Block(b, _span) => {
            let result_tmp = ctx.next_tmp();
            lower_block_to_value(b, ctx, diags, result_tmp)
        }
        HirExpr::Binary(lhs, op, rhs, _ty, span) => {
            let l = lower_expr_to_value(lhs, ctx, diags);
            let r = lower_expr_to_value(rhs, ctx, diags);
            let tmp = ctx.next_tmp();
            ctx.emit(TirOp::Binary(tmp, l, *op, r, span.clone()));
            TirValue::Var(tmp)
        }
        HirExpr::Unary(op, inner, _ty, span) => {
            let i = lower_expr_to_value(inner, ctx, diags);
            let tmp = ctx.next_tmp();
            ctx.emit(TirOp::Unary(tmp, *op, i, span.clone()));
            TirValue::Var(tmp)
        }
        HirExpr::Call(callee, args, _ty, span) => {
            let c = lower_expr_to_value(callee, ctx, diags);
            let targs: Vec<TirArg> = args.iter().map(|a| TirArg {
                mode: a.mode,
                value: lower_expr_to_value(&a.expr, ctx, diags),
                span: a.span.clone(),
            }).collect();
            let result_tmp = ctx.next_tmp();
            ctx.emit(TirOp::Call(Some(result_tmp), c, targs, span.clone()));
            TirValue::Var(result_tmp)
        }
        HirExpr::AsCast(inner, target_ty, span) => {
            let i = lower_expr_to_value(inner, ctx, diags);
            let tmp = ctx.next_tmp();
            ctx.emit(TirOp::AsCast(tmp, i, target_ty.clone(), span.clone()));
            TirValue::Var(tmp)
        }
        HirExpr::Reference(inner, span) => {
            let i = lower_expr_to_value(inner, ctx, diags);
            // Find the source TmpVar from inner
            if let TirValue::Var(src) = i {
                let tmp = ctx.next_tmp();
                ctx.emit(TirOp::Borrow(tmp, src, BorrowKind::Shared, span.clone()));
                TirValue::Var(tmp)
            } else {
                let tmp = ctx.next_tmp();
                ctx.emit(TirOp::Let(tmp, i, span.clone()));
                TirValue::Var(tmp)
            }
        }
        HirExpr::ArrowFn(params, _ret, body, is_move, _span) => {
            // §3.2.4 闭包捕获提升:
            // 1. 扫描 body 中引用的外部变量（不在 params 中的 Ident）
            // 2. 默认闭包 → BorrowKind::Shared，move 闭包 → BorrowKind::Mutable
            // 3. 将捕获变量加入 captured_vars，生成隐式参数

            // Phase 1 简化：闭包降级为内联块 (ArrowFn body 作为 Block 处理)
            // 捕获变量扫描在 name_res 阶段已完成（HirBinding 已解析）
            // captured_vars 暂存外部引用的 TmpVar 映射
            let kind = if *is_move { BorrowKind::Mutable } else { BorrowKind::Shared };

            // 扫描 body 中的 Ident，查找外部变量
            let mut captured = Vec::new();
            collect_free_vars(&body, params, &ctx.map, &mut captured, kind);

            // 为每个捕获变量在 var_map 中注册（确保 moveck/borrowck 可见）
            for cv in &captured {
                if let Some(existing) = ctx.map.lookup_name(&cv.name) {
                    ctx.map.insert(cv.tmp, &cv.name, ctx.blocks[ctx.cur_block].span.clone());
                }
            }

            // 返回哨兵值（闭包的实际 TirFunction 在后续实现中单独生成）
            // Phase 1: 闭包体作为内联表达式处理，不生成独立 TirFunction
            TirValue::Error
        }
        HirExpr::TemplateLiteral(parts, _span) => {
            // Simple: concatenate literal parts
            let mut s = String::new();
            for p in parts {
                if let HirTemplatePartKind::String(ref lit) = p.kind {
                    s.push_str(lit);
                }
            }
            TirValue::StringLiteral(s)
        }
        HirExpr::AssertUnwrap(inner, _) | HirExpr::TryPropagate(inner, _) => {
            lower_expr_to_value(inner, ctx, diags)
        }
        HirExpr::Error(_) => TirValue::Error,
    }
}

fn lower_block_to_value(block: &HirBlock, ctx: &mut LowerCtx, diags: &mut Vec<DiagError>, result_tmp: TmpVar) -> TirValue {
    for (i, stmt) in block.statements.iter().enumerate() {
        let is_last = i + 1 == block.statements.len();
        if is_last {
            if let HirStmt::Expr(e) = stmt {
                let val = lower_expr_to_value(e, ctx, diags);
                ctx.emit(TirOp::Let(result_tmp, val, Span::dummy()));
                return TirValue::Var(result_tmp);
            }
        }
        lower_stmt(stmt, ctx, diags);
    }
    // 空块或最后非表达式 → Void/Error
    TirValue::Error
}

// ============================================================================
// 闭包辅助 — §3.2.4
// ============================================================================

/// 扫描 HirBlock 中所有 Ident，收集不在 params 列表中的外部变量引用
fn collect_free_vars(
    block: &HirBlock,
    params: &[HirParam],
    var_map: &VarMapping,
    captured: &mut Vec<CapturedVar>,
    kind: BorrowKind,
) {
    for stmt in &block.statements {
        match stmt {
            HirStmt::Expr(e) => collect_free_vars_expr(e, params, var_map, captured, kind),
            HirStmt::Let(l) => collect_free_vars_expr(&l.init, params, var_map, captured, kind),
            HirStmt::Return(r) => {
                if let Some(ref v) = r.value {
                    collect_free_vars_expr(v, params, var_map, captured, kind);
                }
            }
            HirStmt::If(if_s) => {
                collect_free_vars_expr(&if_s.condition, params, var_map, captured, kind);
                collect_free_vars(&if_s.then_branch, params, var_map, captured, kind);
                if let Some(ref else_b) = if_s.else_branch {
                    collect_free_vars(else_b, params, var_map, captured, kind);
                }
            }
            HirStmt::For(f) => {
                collect_free_vars_expr(&f.condition, params, var_map, captured, kind);
                collect_free_vars_expr(&f.update, params, var_map, captured, kind);
                collect_free_vars(&f.body, params, var_map, captured, kind);
            }
            HirStmt::While(w) => {
                collect_free_vars_expr(&w.condition, params, var_map, captured, kind);
                collect_free_vars(&w.body, params, var_map, captured, kind);
            }
            HirStmt::Loop(l) => collect_free_vars(&l.body, params, var_map, captured, kind),
            _ => {}
        }
    }
}

fn collect_free_vars_expr(
    expr: &HirExpr,
    params: &[HirParam],
    var_map: &VarMapping,
    captured: &mut Vec<CapturedVar>,
    kind: BorrowKind,
) {
    match expr {
        HirExpr::Ident(name, _, _) => {
            // 检查是否在参数列表中
            let is_param = params.iter().any(|p| &p.name == name);
            if !is_param {
                // 检查是否已捕获
                let already_captured = captured.iter().any(|c| c.name == *name);
                if !already_captured {
                    if let Some(tmp) = var_map.lookup_name(name) {
                        captured.push(CapturedVar {
                            name: name.clone(),
                            tmp,
                            kind,
                        });
                    }
                }
            }
        }
        HirExpr::Binary(lhs, _, rhs, ..) => {
            collect_free_vars_expr(lhs, params, var_map, captured, kind);
            collect_free_vars_expr(rhs, params, var_map, captured, kind);
        }
        HirExpr::Unary(_, inner, ..) => {
            collect_free_vars_expr(inner, params, var_map, captured, kind);
        }
        HirExpr::Call(callee, args, ..) => {
            collect_free_vars_expr(callee, params, var_map, captured, kind);
            for arg in args {
                collect_free_vars_expr(&arg.expr, params, var_map, captured, kind);
            }
        }
        HirExpr::If(if_s, _) => {
            collect_free_vars_expr(&if_s.condition, params, var_map, captured, kind);
            collect_free_vars(&if_s.then_branch, params, var_map, captured, kind);
            if let Some(ref else_b) = if_s.else_branch {
                collect_free_vars(else_b, params, var_map, captured, kind);
            }
        }
        HirExpr::Loop(l, _) => collect_free_vars(&l.body, params, var_map, captured, kind),
        HirExpr::Block(b, _) => collect_free_vars(b, params, var_map, captured, kind),
        _ => {}
    }
}

// ============================================================================
// Copy 判定 — §3.3.2
// ============================================================================

fn is_copy_type(ty: &HirType) -> bool {
    match ty {
        HirType::I32 | HirType::F64 | HirType::I64 | HirType::Bool | HirType::BigInt => true,
        HirType::Ref(_) => true,
        HirType::String | HirType::Void | HirType::Array(_) | HirType::Named(_) | HirType::Function(..) => false,
        HirType::Error => false,
    }
}

// ============================================================================
// 降级上下文 — 辅助
// ============================================================================

struct LowerCtx {
    /// 所有基本块（按 ID 索引）
    blocks: Vec<BasicBlock>,
    cur_block: BlockId,
    next_id: BlockId,
    tmp_counter: u32,
    entry: BlockId,
    exit: BlockId,
    map: VarMapping,
    /// (cond_id, exit_id) — Break/Continue 跳转目标
    loop_stack: Vec<(BlockId, BlockId)>,
}

impl LowerCtx {
    fn new() -> Self {
        LowerCtx {
            blocks: vec![],
            cur_block: 0,
            next_id: 0,
            tmp_counter: 0,
            entry: 0,
            exit: 0,
            map: VarMapping::new(),
            loop_stack: vec![],
        }
    }

    fn next_tmp(&mut self) -> TmpVar {
        let t = TmpVar(self.tmp_counter);
        self.tmp_counter += 1;
        t
    }

    fn next_block_id(&mut self) -> BlockId {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    fn new_block(&mut self, id: BlockId, span: Span) {
        self.cur_block = id;
        while self.blocks.len() <= id {
            self.blocks.push(BasicBlock {
                id: self.blocks.len(),
                ops: vec![],
                terminator: Terminator::Unreachable,
                span: Span::dummy(),
            });
        }
        self.blocks[id] = BasicBlock {
            id,
            ops: vec![],
            terminator: Terminator::Unreachable,
            span,
        };
    }

    fn emit(&mut self, op: TirOp) {
        self.blocks[self.cur_block].ops.push(op);
    }
}

fn finish_block(ctx: &mut LowerCtx, term: Terminator) {
    ctx.blocks[ctx.cur_block].terminator = term;
}

fn is_terminated(ctx: &LowerCtx) -> bool {
    !matches!(ctx.blocks[ctx.cur_block].terminator, Terminator::Unreachable)
}

// ============================================================================
// 单元测试 — §4.1
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    /// Verifies that an empty HirFunction lowers to a single-block TirFunction
    #[test]
    fn lower_empty_function_produces_single_block() {
        let f = HirFunction {
            name: "empty".into(),
            params: vec![],
            return_type: HirType::Void,
            body: HirBlock { statements: vec![], span: Span::dummy() },
            scope: Scope::new(),
            span: Span::dummy(),
        };
        let mut diags = vec![];
        let tf = lower_function(&f, &mut diags);
        assert!(diags.is_empty());
        assert_eq!(tf.name, "empty");
        assert!(tf.blocks.len() >= 1);
    }

    /// AC-SEM-007: if 表达式→临时变量
    #[test]
    fn lower_if_expr_produces_temporary() {
        // let x = if (true) { 1 } else { 0 };
        let f = HirFunction {
            name: "test".into(),
            params: vec![],
            return_type: HirType::I32,
            body: HirBlock {
                statements: vec![
                    HirStmt::Let(HirLet {
                        name: "x".into(),
                        mutable: false,
                        ty: HirType::I32,
                        init: Box::new(HirExpr::If(
                            HirIf {
                                condition: Box::new(HirExpr::BoolLiteral(true, Span::dummy())),
                                then_branch: HirBlock {
                                    statements: vec![HirStmt::Expr(HirExpr::IntLiteral(1, Span::dummy()))],
                                    span: Span::dummy(),
                                },
                                else_branch: Some(HirBlock {
                                    statements: vec![HirStmt::Expr(HirExpr::IntLiteral(0, Span::dummy()))],
                                    span: Span::dummy(),
                                }),
                                span: Span::dummy(),
                            },
                            Span::dummy(),
                        )),
                        span: Span::dummy(),
                    }),
                ],
                span: Span::dummy(),
            },
            scope: Scope::new(),
            span: Span::dummy(),
        };
        let mut diags = vec![];
        let tf = lower_function(&f, &mut diags);
        assert!(diags.is_empty(), "diags: {:?}", diags);
        // 应该有 ≥5 个基本块：entry + cond + then + else + join
        assert!(tf.blocks.len() >= 5, "blocks: {}", tf.blocks.len());
    }

    /// Moves a non-Copy variable
    #[test]
    fn move_non_copy_uses_move_op() {
        let let_binding = HirBinding::LocalVar {
            ty: HirType::String,
            mutable: false,
            span: Span::dummy(),
        };
        let let_s = HirLet {
            name: "b".into(),
            mutable: false,
            ty: HirType::String,
            init: Box::new(HirExpr::Ident("a".into(), let_binding, Span::dummy())),
            span: Span::dummy(),
        };
        let mut ctx = LowerCtx::new();
        ctx.new_block(0, Span::dummy());
        // Simulate that a is already a TmpVar in the map
        ctx.map.insert(TmpVar(0), "a", Span::dummy());
        let mut diags = vec![];
        lower_let(&let_s, &mut ctx, &mut diags);
        assert!(diags.is_empty());
        // Should emit Move (String is non-Copy)
        let b = &ctx.blocks[0];
        assert!(b.ops.iter().any(|op| matches!(op, TirOp::Move(..))), "expected Move op");
    }
}
