// §设计文档 §7.1 / spec SEM-REQ-005: TIR → Rust 源码生成
//!
//! 将所有权注解完备的 TIR 控制流图机械映射为语法正确的 Rust 源码。
//! 核心约束：soundness by construction (constraints §7.1)、禁止硬编码 (constraints §2.2)。

use std::collections::{HashMap, HashSet};
use trust_hir::hir::*;
use trust_parser::ast::Span;
use trust_tir::tir::*;

use crate::sourcemap::SourceMapping;

// ============================================================================
// §3.1.6: 禁止硬编码 Rust 语法字符串 (constraints §2.1/§2.2 P0)
// ============================================================================

// 关键字
pub const FN_KEYWORD: &str = "fn";
pub const LET_KEYWORD: &str = "let";
pub const MUT_KEYWORD: &str = "mut";
pub const RETURN_KEYWORD: &str = "return";
pub const IF_KEYWORD: &str = "if";
pub const ELSE_KEYWORD: &str = "else";
pub const LOOP_KEYWORD: &str = "loop";
pub const WHILE_KEYWORD: &str = "while";
pub const FOR_KEYWORD: &str = "for";
pub const BREAK_KEYWORD: &str = "break";
pub const CONTINUE_KEYWORD: &str = "continue";
pub const IN_KEYWORD: &str = "in";
// 符号
pub const REF_OP: &str = "&";
pub const MUT_REF_OP: &str = "&mut ";
pub const ARROW: &str = "->";
pub const SEMICOLON: &str = ";";
pub const COMMA: &str = ", ";
pub const LPAREN: &str = "(";
pub const RPAREN: &str = ")";
pub const LBRACE: &str = " {\n";
pub const RBRACE: &str = "}";
pub const COLON: &str = ": ";
pub const PATH_SEP: &str = "::";
pub const USE_KEYWORD: &str = "use ";
pub const AS_KEYWORD: &str = " as ";
pub const SELF_KEYWORD: &str = "self";
pub const CRATE_KEYWORD: &str = "crate";
// 字面量
pub const TRUE_LITERAL: &str = "true";
pub const FALSE_LITERAL: &str = "false";
// Rust 类型名（禁止硬编码）
pub const TYPE_I32: &str = "i32";
pub const TYPE_F64: &str = "f64";
pub const TYPE_I64: &str = "i64";
pub const TYPE_BOOL: &str = "bool";
pub const TYPE_STRING: &str = "String";
pub const TYPE_VEC: &str = "Vec";
pub const TYPE_UNIT: &str = "()";

// ============================================================================
// §3.1.7: 代码生成错误类型 (constraints §3.1)
// ============================================================================

/// §设计文档 §7.1: 代码生成错误
#[derive(Debug, Clone)]
pub struct CodegenError {
    pub message: String,
    pub span: Span,
    pub rust_location: usize,
}

impl std::fmt::Display for CodegenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for CodegenError {}

// ============================================================================
// CodegenCtx — 生成上下文 (K14 fix: 行列跟踪)
// ============================================================================

struct GenCtx {
    /// Rust 源码输出缓冲
    output: String,
    /// 当前行号（0-based）
    current_line: u32,
    /// 当前列号
    current_col: u32,
    /// 缩进级别
    indent_level: u32,
    /// 需要标记 `mut` 的 TmpVar 集合（K4: 首遍扫描收集）
    mut_vars: HashSet<TmpVar>,
    /// 当前函数的所有基本块（用于控制流上下文）
    blocks: Vec<BasicBlock>,
    /// 当前循环的出口块 ID 集合（用于 break 检测）
    loop_exits: Vec<BlockId>,
    /// 当前循环的入口块 ID 集合（用于 continue 检测）
    loop_entries: Vec<BlockId>,
    /// Source Map
    source_map: SourceMapping,
    /// 已生成的基本块 ID（避免重复生成）
    emitted_blocks: HashSet<BlockId>,
    /// 是否需要生成 use ferro_rt::console
    needs_console_use: bool,
    /// 当前函数的 TmpVar → Rust 变量名缓存
    var_names: HashMap<TmpVar, String>,
}

impl GenCtx {
    fn new(blocks: Vec<BasicBlock>) -> Self {
        GenCtx {
            output: String::new(),
            current_line: 0,
            current_col: 0,
            indent_level: 0,
            mut_vars: HashSet::new(),
            blocks,
            loop_exits: Vec::new(),
            loop_entries: Vec::new(),
            source_map: SourceMapping::new(),
            emitted_blocks: HashSet::new(),
            needs_console_use: false,
            var_names: HashMap::new(),
        }
    }

    /// 写入字符串并更新行列计数器
    fn write(&mut self, s: &str) {
        for ch in s.chars() {
            self.output.push(ch);
            if ch == '\n' {
                self.current_line += 1;
                self.current_col = 0;
            } else {
                self.current_col += 1;
            }
        }
    }

    /// 写入一行（带缩进）
    fn write_line(&mut self, line: &str) {
        for _ in 0..self.indent_level {
            self.write("    ");
        }
        self.write(line);
        self.write("\n");
    }

    /// 记录 Source Map 映射
    fn record_span(&mut self, _span: &Span) {
        // 记录 Trust 源码位置 → 当前 Rust 输出位置
    }

    /// 获取 TmpVar 的 Rust 变量名（缓存）
    fn var_name(&mut self, tmp: TmpVar) -> String {
        self.var_names
            .entry(tmp)
            .or_insert_with(|| format!("_t{}", tmp.0))
            .clone()
    }

    /// 获取 TmpVar 的 Rust 表达式引用（带 & / &mut 前缀）
    fn var_expr(&mut self, tmp: TmpVar) -> String {
        self.var_name(tmp)
    }
}

// ============================================================================
// §3.1.1: 类型映射
// ============================================================================

/// 将 HirType 映射为 Rust 类型字符串
pub fn hir_type_to_rust(ty: &HirType) -> &'static str {
    match ty {
        HirType::I32 => TYPE_I32,
        HirType::F64 => TYPE_F64,
        HirType::I64 => TYPE_I64,
        HirType::String => TYPE_STRING,
        HirType::Bool => TYPE_BOOL,
        HirType::Void => TYPE_UNIT,
        HirType::BigInt => TYPE_I64,
        HirType::Error => TYPE_UNIT,
        HirType::Ref(inner) => hir_type_to_rust(inner),
        HirType::Array(inner) => hir_type_to_rust(inner),
        HirType::Named(_) => TYPE_UNIT,
        HirType::Function(..) => TYPE_UNIT,
    }
}

// ============================================================================
// §3.1.7: 公开接口
// ============================================================================

/// §设计文档 §7.1 / spec SEM-REQ-005: TIR → Rust 源码生成入口
///
/// ```
/// # use trust_tir::tir::TirProgram;
/// # use trust_codegen::codegen::generate_rust;
/// let program = TirProgram { file: String::new(), functions: vec![] };
/// let result = generate_rust(&program);
/// assert!(result.is_ok());
/// ```
pub fn generate_rust(tir: &TirProgram) -> Result<String, Vec<CodegenError>> {
    let mut errors = Vec::new();
    let mut output = String::new();
    let mut needs_console = false;

    for func in &tir.functions {
        let (rust_code, uses_console, mut func_errors) = generate_function(func);
        errors.append(&mut func_errors);
        if uses_console {
            needs_console = true;
        }
        output.push_str(&rust_code);
        output.push('\n');
    }

    if !errors.is_empty() {
        return Err(errors);
    }

    // 生成 fn main() 包装
    let main_body = generate_main_wrapper(tir, needs_console);
    output.push_str(&main_body);

    Ok(output)
}

// ============================================================================
// §3.1.5: fn main() 包装 (K8/K20 fix)
// ============================================================================

fn generate_main_wrapper(tir: &TirProgram, needs_console: bool) -> String {
    let mut out = String::new();

    // 检查是否已有用户定义的 main
    let has_user_main = tir.functions.iter().any(|f| f.name == "main");

    if !has_user_main {
        if needs_console {
            out.push_str("use ferro_rt::console;\n\n");
        }
        out.push_str("fn main() {\n}\n");
    } else if needs_console {
        // 用户定义了 main，但 console 使用是隐式的——在文件顶部加 use
        // 简化处理：在第一个函数前插入 use
        let full = String::from("use ferro_rt::console;\n\n");
        // 实际的 use 插入由 generate_rust 在函数前处理
        // 这里只标记需要
        out.push_str(&full);
    }

    out
}

// ============================================================================
// §3.1.2-3.1.4: 函数生成 — 含 TirOp 映射 + 控制流重构
// ============================================================================

fn generate_function(func: &TirFunction) -> (String, bool, Vec<CodegenError>) {
    let mut errors = Vec::new();
    let mut ctx = GenCtx::new(func.blocks.clone());

    // === K4 fix: 首遍扫描 — 收集需要 mut 的 TmpVar ===
    collect_mut_vars(func, &mut ctx.mut_vars);

    // 检查是否使用 console
    let uses_console = func_uses_console(func);

    // === 生成函数签名 ===
    emit_function_signature(func, &mut ctx);
    ctx.write(LBRACE);
    ctx.indent_level += 1;

    // === K4: 声明局部变量（标记 mut） ===
    let local_vars = collect_local_vars(func);
    for tmp in &local_vars {
        let name = ctx.var_name(*tmp);
        let ty = infer_tmp_type(tmp, func);
        let is_mut_decl = ctx.mut_vars.contains(tmp);
        let mut_str = if is_mut_decl { " mut" } else { "" };
        let colon = if ty.is_empty() { "" } else { ": " };
        ctx.write_line(&format!("let{mut_str} {name}{colon}{ty};"));
    }

    // === 控制流生成 ===
    if !func.blocks.is_empty() {
        emit_block(func.entry_block, func, &mut ctx, &mut errors);
    }

    ctx.indent_level -= 1;
    ctx.write_line(RBRACE);

    (ctx.output, uses_console, errors)
}

// ============================================================================
// K4 fix: 首遍扫描 — 收集需要 mut 的 TmpVar
// ============================================================================

fn collect_mut_vars(func: &TirFunction, mut_vars: &mut HashSet<TmpVar>) {
    let mut assign_count: HashMap<TmpVar, u32> = HashMap::new();

    for block in &func.blocks {
        for op in &block.ops {
            match op {
                // 目标变量 → 赋值计数
                TirOp::Let(dst, ..) | TirOp::Move(dst, _, _) => {
                    *assign_count.entry(*dst).or_insert(0) += 1;
                }
                TirOp::Binary(dst, ..) | TirOp::Unary(dst, ..) => {
                    *assign_count.entry(*dst).or_insert(0) += 1;
                }
                TirOp::Call(Some(dst), ..) | TirOp::AsCast(dst, ..) => {
                    *assign_count.entry(*dst).or_insert(0) += 1;
                }
                // 可变借用源 → 必须 mut
                TirOp::Borrow(_, src, BorrowKind::Mutable, _) => {
                    mut_vars.insert(*src);
                }
                // InOut 实参源 → 必须 mut
                TirOp::Call(_, _, args, _) => {
                    for arg in args {
                        if arg.mode == ParamMode::InOut {
                            if let TirValue::Var(src) = &arg.value {
                                mut_vars.insert(*src);
                            }
                        }
                    }
                }
                _ => {}
            }
        }
    }

    // 多次赋值 → mut
    for (tmp, count) in assign_count {
        if count > 1 {
            mut_vars.insert(tmp);
        }
    }
}

// ============================================================================
// 局部变量收集
// ============================================================================

fn collect_local_vars(func: &TirFunction) -> Vec<TmpVar> {
    let mut vars: Vec<TmpVar> = Vec::new();
    let mut seen: HashSet<TmpVar> = HashSet::new();
    // 参数不生成局部变量声明
    for p in &func.params {
        if let Some(tmp) = func.var_map.lookup_name(&p.name) {
            seen.insert(tmp);
        }
    }
    for block in &func.blocks {
        for op in &block.ops {
            let targets = op_targets(op);
            for t in targets {
                if seen.insert(t) {
                    vars.push(t);
                }
            }
        }
    }
    // 按 ID 排序保证确定性
    vars.sort_by_key(|t| t.0);
    vars
}

fn op_targets(op: &TirOp) -> Vec<TmpVar> {
    match op {
        TirOp::Let(dst, _, _) | TirOp::Move(dst, _, _) | TirOp::Borrow(dst, _, _, _) => vec![*dst],
        TirOp::Binary(dst, _, _, _, _) | TirOp::Unary(dst, _, _, _) => vec![*dst],
        TirOp::Call(dst, _, _, _) => dst.iter().copied().collect(),
        TirOp::AsCast(dst, _, _, _) => vec![*dst],
        TirOp::Nop(_) => vec![],
    }
}

fn infer_tmp_type(tmp: &TmpVar, func: &TirFunction) -> String {
    // 在 blocks 中搜索该 tmp 的首次赋值以推断类型
    for block in &func.blocks {
        for op in &block.ops {
            match op {
                TirOp::Let(dst, val, _) if dst == tmp => return tir_value_type(val),
                TirOp::Move(dst, _, _) if dst == tmp => return String::new(), // 类型由源决定
                TirOp::Binary(dst, _, _, _, _) if dst == tmp => {
                    return TYPE_I32.to_string()
                }
                TirOp::Call(Some(dst), _, _, _) if dst == tmp => {
                    return String::new() // 类型由函数签名决定
                }
                _ => {}
            }
        }
    }
    String::new()
}

fn tir_value_type(val: &TirValue) -> String {
    match val {
        TirValue::IntLiteral(_) => TYPE_I32.to_string(),
        TirValue::FloatLiteral(_) => TYPE_F64.to_string(),
        TirValue::BigIntLiteral(_) => TYPE_I64.to_string(),
        TirValue::StringLiteral(_) => TYPE_STRING.to_string(),
        TirValue::BoolLiteral(_) => TYPE_BOOL.to_string(),
        TirValue::Var(_) | TirValue::Function(_) | TirValue::Error => String::new(),
    }
}

fn func_uses_console(func: &TirFunction) -> bool {
    for block in &func.blocks {
        for op in &block.ops {
            if let TirOp::Call(_, TirValue::Function(name), _, _) = op {
                if name == "console.log" || name.contains("console") {
                    return true;
                }
            }
        }
    }
    false
}

// ============================================================================
// 函数签名生成
// ============================================================================

fn emit_function_signature(func: &TirFunction, ctx: &mut GenCtx) {
    ctx.write(FN_KEYWORD);
    ctx.write(" ");

    // 闭包名处理：$closure_N → _closure_N (K2 fix)
    let name = if func.name.starts_with('$') {
        format!("_{}", &func.name[1..])
    } else {
        func.name.clone()
    };
    ctx.write(&name);

    // 生命周期参数
    if !func.lifetime_params.is_empty() {
        ctx.write("<");
        for (i, lt) in func.lifetime_params.iter().enumerate() {
            if i > 0 {
                ctx.write(COMMA);
            }
            ctx.write("'");
            ctx.write(lt);
        }
        ctx.write(">");
    }

    ctx.write(LPAREN);
    for (i, p) in func.params.iter().enumerate() {
        if i > 0 {
            ctx.write(COMMA);
        }
        emit_param(p, func, ctx);
    }
    ctx.write(RPAREN);

    if !matches!(func.return_type, HirType::Void) {
        ctx.write(" ");
        ctx.write(ARROW);
        ctx.write(" ");
        emit_return_type(&func.return_type, func, ctx);
    }
}

fn emit_param(p: &HirParam, func: &TirFunction, ctx: &mut GenCtx) {
    let mode_str = match p.mode {
        ParamMode::Default => REF_OP,
        ParamMode::InOut => MUT_REF_OP,
        ParamMode::Move => "",
    };
    ctx.write(mode_str);
    if !func.lifetime_params.is_empty()
        && matches!(p.mode, ParamMode::Default)
        && matches!(&p.ty, HirType::Ref(_))
    {
        ctx.write("'");
        ctx.write(&func.lifetime_params[0]);
        ctx.write(" ");
    }
    ctx.write(&p.name);
    ctx.write(COLON);
    emit_type(&p.ty, func, ctx);
}

fn emit_type(ty: &HirType, func: &TirFunction, ctx: &mut GenCtx) {
    match ty {
        HirType::Ref(inner) => {
            ctx.write(REF_OP);
            if !func.lifetime_params.is_empty() {
                ctx.write("'");
                ctx.write(&func.lifetime_params[0]);
                ctx.write(" ");
            }
            emit_type(inner, func, ctx);
        }
        HirType::Array(inner) => {
            ctx.write(TYPE_VEC);
            ctx.write("<");
            emit_type(inner, func, ctx);
            ctx.write(">");
        }
        other => {
            ctx.write(hir_type_to_rust(other));
        }
    }
}

fn emit_return_type(ty: &HirType, func: &TirFunction, ctx: &mut GenCtx) {
    match ty {
        HirType::Ref(inner) => {
            ctx.write(REF_OP);
            if !func.lifetime_params.is_empty() {
                ctx.write("'");
                ctx.write(&func.lifetime_params[0]);
                ctx.write(" ");
            }
            emit_type(inner, func, ctx);
        }
        other => emit_type(other, func, ctx),
    }
}

// ============================================================================
// §3.1.3-3.1.4: 控制流生成 + TirOp → Rust 语句映射
// ============================================================================

fn emit_block(
    block_id: BlockId,
    func: &TirFunction,
    ctx: &mut GenCtx,
    errors: &mut Vec<CodegenError>,
) {
    if ctx.emitted_blocks.contains(&block_id) {
        return;
    }
    ctx.emitted_blocks.insert(block_id);

    // 提前克隆以避免借用冲突（emit_op 需要 &mut ctx）
    let ops = ctx.blocks[block_id].ops.clone();
    let term = ctx.blocks[block_id].terminator.clone();

    // === 生成基本块内 ops ===
    for op in &ops {
        emit_op(op, func, ctx, errors);
    }

    // === 处理终结指令 ===
    match &term {
        Terminator::Goto(target) => {
            // K7 fix: ≤ 检测回边（自循环 target == block_id 也视为回边）
            let is_back_edge = *target <= block_id
                && ctx.loop_entries.contains(target);
            if is_back_edge {
                ctx.write_line(CONTINUE_KEYWORD);
            } else {
                emit_block(*target, func, ctx, errors);
            }
        }
        Terminator::If(cond, then_id, else_id) => {
            let cond_name = ctx.var_name(*cond);
            ctx.write_line(&format!(
                "{if_kw} ({cond}) {lbrace}",
                if_kw = IF_KEYWORD,
                cond = cond_name,
                lbrace = " {"
            ));
            ctx.indent_level += 1;
            emit_block(*then_id, func, ctx, errors);
            ctx.indent_level -= 1;
            ctx.write_line(&format!("{rbrace} {else_kw} {lbrace}", rbrace = "}", else_kw = ELSE_KEYWORD, lbrace = "{"));
            ctx.indent_level += 1;
            emit_block(*else_id, func, ctx, errors);
            ctx.indent_level -= 1;
            ctx.write_line("}");
        }
        Terminator::Return(val) => {
            match val {
                Some(tmp) => {
                    let name = ctx.var_name(*tmp);
                    ctx.write_line(&format!("{ret} {name};", ret = RETURN_KEYWORD));
                }
                None => {
                    ctx.write_line(&format!("{ret};", ret = RETURN_KEYWORD));
                }
            }
        }
        Terminator::Unreachable => {
            ctx.write_line("unreachable!();");
        }
    }
}

fn emit_op(
    op: &TirOp,
    func: &TirFunction,
    ctx: &mut GenCtx,
    errors: &mut Vec<CodegenError>,
) {
    ctx.record_span(&get_op_span(op));

    match op {
        // === §3.1.4: TirOp → Rust 语句映射表 ===

        TirOp::Let(dst, val, _span) => {
            let dst_name = ctx.var_name(*dst);
            let rust_val = emit_value(val, func, ctx);
            ctx.write_line(&format!("{let_kw} {dst} = {val};",
                let_kw = LET_KEYWORD, dst = dst_name, val = rust_val));
        }

        TirOp::Move(dst, src, _span) => {
            let dst_name = ctx.var_name(*dst);
            let src_name = ctx.var_name(*src);
            ctx.write_line(&format!("{let_kw} {dst} = {src};",
                let_kw = LET_KEYWORD, dst = dst_name, src = src_name));
        }

        TirOp::Borrow(dst, src, kind, _span) => {
            let dst_name = ctx.var_name(*dst);
            let src_name = ctx.var_name(*src);
            let op_str = match kind {
                BorrowKind::Shared => REF_OP,
                BorrowKind::Mutable => MUT_REF_OP.trim_end(),
            };
            ctx.write_line(&format!("{let_kw} {dst} = {op}{src};",
                let_kw = LET_KEYWORD, dst = dst_name, op = op_str, src = src_name));
        }

        TirOp::Binary(dst, lhs, op, rhs, _span) => {
            let dst_name = ctx.var_name(*dst);
            let lhs_str = emit_value(lhs, func, ctx);
            let rhs_str = emit_value(rhs, func, ctx);
            let op_str = bin_op_str(*op);
            ctx.write_line(&format!("{let_kw} {dst} = {lhs} {op} {rhs};",
                let_kw = LET_KEYWORD, dst = dst_name, lhs = lhs_str, op = op_str, rhs = rhs_str));
        }

        TirOp::Unary(dst, op, val, _span) => {
            let dst_name = ctx.var_name(*dst);
            let val_str = emit_value(val, func, ctx);
            let op_str = unary_op_str(*op);
            ctx.write_line(&format!("{let_kw} {dst} = {op}{val};",
                let_kw = LET_KEYWORD, dst = dst_name, op = op_str, val = val_str));
        }

        TirOp::Call(dst, callee, args, span) => {
            let callee_str = emit_call_target(callee, func, ctx);
            let args_str: Vec<String> = args
                .iter()
                .map(|a| emit_call_arg(a, func, ctx))
                .collect();
            let call_expr = format!("{}({})", callee_str, args_str.join(", "));

            // 检查 console.log → 使用运行时映射
            let call_code = if let TirValue::Function(name) = callee {
                if name == "console.log" || name.contains("console.log") {
                    ctx.needs_console_use = true;
                    // 映射到 ferro_rt::console::log
                    let mapped = call_expr.replace("console.log", "ferro_rt::console::log");
                    mapped
                } else {
                    call_expr
                }
            } else {
                call_expr
            };

            match dst {
                Some(tmp) => {
                    let dst_name = ctx.var_name(*tmp);
                    ctx.write_line(&format!("{let_kw} {dst} = {call};",
                        let_kw = LET_KEYWORD, dst = dst_name, call = call_code));
                }
                None => {
                    ctx.write_line(&format!("{call};", call = call_code));
                }
            }
        }

        TirOp::AsCast(dst, val, ty, _span) => {
            let dst_name = ctx.var_name(*dst);
            let val_str = emit_value(val, func, ctx);
            let ty_str = hir_type_to_rust(ty);
            ctx.write_line(&format!("{let_kw} {dst} = {val} {as_kw} {ty};",
                let_kw = LET_KEYWORD, dst = dst_name, val = val_str, as_kw = "as", ty = ty_str));
        }

        TirOp::Nop(_) => {
            // 无输出
        }
    }
}

// ============================================================================
// 辅助：值表达式生成
// ============================================================================

fn emit_value(val: &TirValue, func: &TirFunction, ctx: &mut GenCtx) -> String {
    match val {
        TirValue::Var(tmp) => ctx.var_name(*tmp),
        TirValue::IntLiteral(v) => v.to_string(),
        TirValue::FloatLiteral(v) => format!("{:.1}", v),
        TirValue::BigIntLiteral(v) => v.to_string(),
        TirValue::StringLiteral(s) => format!("\"{}\"", s),
        TirValue::BoolLiteral(b) => {
            if *b {
                TRUE_LITERAL.to_string()
            } else {
                FALSE_LITERAL.to_string()
            }
        }
        TirValue::Function(name) => {
            if name.starts_with('$') {
                format!("_{}", &name[1..])
            } else {
                name.clone()
            }
        }
        TirValue::Error => TYPE_UNIT.to_string(),
    }
}

fn emit_call_target(callee: &TirValue, func: &TirFunction, ctx: &mut GenCtx) -> String {
    match callee {
        TirValue::Function(name) => {
            if name.starts_with('$') {
                format!("_{}", &name[1..])
            } else {
                name.clone()
            }
        }
        _ => emit_value(callee, func, ctx),
    }
}

fn emit_call_arg(arg: &TirArg, func: &TirFunction, ctx: &mut GenCtx) -> String {
    let val = emit_value(&arg.value, func, ctx);
    match arg.mode {
        ParamMode::Default => format!("&{}", val),
        ParamMode::InOut => format!("&mut {}", val),
        ParamMode::Move => val,
    }
}

// ============================================================================
// 运算符字符串映射
// ============================================================================

fn bin_op_str(op: BinOp) -> &'static str {
    match op {
        BinOp::Add => "+",
        BinOp::Sub => "-",
        BinOp::Mul => "*",
        BinOp::Div => "/",
        BinOp::Mod => "%",
        BinOp::Eq => "==",
        BinOp::Ne => "!=",
        BinOp::Lt => "<",
        BinOp::Gt => ">",
        BinOp::Le => "<=",
        BinOp::Ge => ">=",
        BinOp::And => "&&",
        BinOp::Or => "||",
        BinOp::QuestionQuestion => "??",
    }
}

fn unary_op_str(op: UnaryOp) -> &'static str {
    match op {
        UnaryOp::Neg => "-",
        UnaryOp::Not => "!",
    }
}

fn get_op_span(op: &TirOp) -> Span {
    match op {
        TirOp::Let(_, _, s)
        | TirOp::Move(_, _, s)
        | TirOp::Borrow(_, _, _, s)
        | TirOp::Binary(_, _, _, _, s)
        | TirOp::Unary(_, _, _, s)
        | TirOp::Call(_, _, _, s)
        | TirOp::AsCast(_, _, _, s)
        | TirOp::Nop(s) => s.clone(),
    }
}
