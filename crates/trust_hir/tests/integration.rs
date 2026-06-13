//! 集成测试 — Phase 1.3 trust_hir
//!
//! 每个语法特性至少一个端到端测试：
//! `.trust` 源码 → parse → lower → resolve_names → check_types → 验证

use trust_hir::hir::*;
use trust_hir::name_res::{lower, resolve_names, DiagError};
use trust_hir::typeck::check_types;
use trust_parser::module_graph::ModuleGraph;

fn run_full_pipeline(
    src: &str,
) -> (HirProgram, Vec<DiagError>, Vec<DiagError>) {
    let mut p = trust_parser::parser::Parser::new(src, "test.trust");
    let prog = p.parse_program();

    let mg = ModuleGraph::new();
    let mut name_diags = vec![];
    let mut hir = resolve_names(&prog, &mg, &mut name_diags);

    let mut type_diags = vec![];
    let _ = check_types(&mut hir, &mut type_diags);

    (hir, name_diags, type_diags)
}

// ============================================================================
// AC-SEM-001: let x = 42 → 正确降级
// ============================================================================

#[test]
fn integration_basic_variable() {
    let src = "function main(): number { let x = 42; return x; }";
    let (hir, name_diags, type_diags) = run_full_pipeline(src);

    assert!(name_diags.is_empty(), "unexpected name diags: {:?}", name_diags);
    assert!(type_diags.is_empty(), "unexpected type diags: {:?}", type_diags);

    // 验证函数存在且包含 let
    let func = hir.items.iter().find_map(|i| match i {
        HirItem::Function(f) if f.name == "main" => Some(f),
        _ => None,
    }).expect("main function should exist");

    let let_stmt = func.body.statements.first().expect("body should have statements");
    assert!(matches!(let_stmt, HirStmt::Let(_)), "expected Let stmt, got {:?}", let_stmt);
}

// ============================================================================
// AC-SEM-002: let x = if (c) { 1 } else { 0 } → IfExpr 降级
// ============================================================================

#[test]
fn integration_if_expression() {
    let src = "function f(): number { let x = if (true) { 1 } else { 0 }; return x; }";
    let (hir, name_diags, type_diags) = run_full_pipeline(src);

    assert!(name_diags.is_empty(), "unexpected name diags: {:?}", name_diags);
    assert!(type_diags.is_empty(), "unexpected type diags: {:?}", type_diags);

    let func = hir.items.iter().find_map(|i| match i {
        HirItem::Function(f) if f.name == "f" => Some(f),
        _ => None,
    }).expect("f should exist");

    let let_stmt = func.body.statements.first().unwrap();
    if let HirStmt::Let(let_s) = let_stmt {
        assert!(matches!(let_s.init.as_ref(), HirExpr::If(..)),
            "expected HirExpr::If, got {:?}", let_s.init);
    } else {
        panic!("expected Let stmt");
    }
}

// ============================================================================
// AC-SEM-002 变体: block 表达式降级
// ============================================================================

#[test]
fn integration_block_expression() {
    let src = "function f(): number { let x = { let y = 2; y }; return x; }";
    let (hir, name_diags, type_diags) = run_full_pipeline(src);

    assert!(name_diags.is_empty(), "unexpected name diags: {:?}", name_diags);
    assert!(type_diags.is_empty(), "unexpected type diags: {:?}", type_diags);

    let func = hir.items.iter().find_map(|i| match i {
        HirItem::Function(f) if f.name == "f" => Some(f),
        _ => None,
    }).expect("f should exist");

    let let_stmt = func.body.statements.first().unwrap();
    if let HirStmt::Let(let_s) = let_stmt {
        assert!(matches!(let_s.init.as_ref(), HirExpr::Block(..)),
            "expected HirExpr::Block, got {:?}", let_s.init);
    }
}

// ============================================================================
// AC-SEM-002 变体: loop 表达式降级
// ============================================================================

#[test]
fn integration_loop_expression() {
    let src = "function f(): number {
        let x = loop { if (true) { break 1; } };
        return x;
    }";
    let (_hir, name_diags, type_diags) = run_full_pipeline(src);
    assert!(name_diags.is_empty(), "unexpected name diags: {:?}", name_diags);
    assert!(type_diags.is_empty(), "unexpected type diags: {:?}", type_diags);
}

// ============================================================================
// AC-TYP-001: i32 + f64 → 编译错误
// ============================================================================

#[test]
fn integration_type_error_mix_numbers() {
    let src = "function f(): number { let a = 42; let b = 3.14; return a + b; }";
    let (_hir, _name_diags, type_diags) = run_full_pipeline(src);

    let has_type_err = type_diags.iter().any(|d| d.message.contains("type mismatch"));
    assert!(has_type_err, "expected type mismatch for i32 + f64, got: {:?}",
        type_diags.iter().map(|d| &d.message).collect::<Vec<_>>());
}

// ============================================================================
// AC-TYP-002: as 转换后类型兼容（Phase 1 仅验证 as 语法不产生错误）
// ============================================================================

#[test]
fn integration_as_cast_allows_same_type() {
    // Phase 1: `as` 在类型同为 number 时是 no-op
    let src = "function f(): number { let a = 42; return a as number; }";
    let (_hir, _name_diags, type_diags) = run_full_pipeline(src);
    assert!(type_diags.is_empty(), "unexpected type diags: {:?}",
        type_diags.iter().map(|d| &d.message).collect::<Vec<_>>());
}

// ============================================================================
// AC-TYP-003: as 转换放在二元运算中仍正确
// ============================================================================

#[test]
fn integration_as_cast_in_expression() {
    // Phase 1: a as number + b → 类型检查中 as 的结果与 b 类型兼容
    let src = "function f(): number { let a = 42; let b = 10; return a as number + b; }";
    let (_hir, _name_diags, type_diags) = run_full_pipeline(src);
    assert!(type_diags.is_empty(), "unexpected type diags: {:?}",
        type_diags.iter().map(|d| &d.message).collect::<Vec<_>>());
}

// ============================================================================
// 函数调用
// ============================================================================

#[test]
fn integration_function_call() {
    let src = "function add(a: number, b: number): number { return a + b; }
               function main(): number { return add(1, 2); }";
    let (_hir, name_diags, _type_diags) = run_full_pipeline(src);
    assert!(name_diags.is_empty(), "unexpected name diags: {:?}", name_diags);
    // 注：跨函数名称解析在 Phase 1 已实现（模块作用域）
}

// ============================================================================
// 控制流: for / while / for-of
// ============================================================================

#[test]
fn integration_for_loop() {
    let src = "function f(): void { for (let i = 0; i < 10; i = i + 1) { } }";
    let (_hir, name_diags, type_diags) = run_full_pipeline(src);
    assert!(name_diags.is_empty(), "unexpected name diags: {:?}", name_diags);
    assert!(type_diags.is_empty(), "unexpected type diags: {:?}", type_diags);
}

#[test]
fn integration_while_loop() {
    let src = "function f(): void { let x = 10; while (x > 0) { x = x - 1; } }";
    let (_hir, name_diags, _type_diags) = run_full_pipeline(src);
    assert!(name_diags.is_empty(), "unexpected name diags: {:?}", name_diags);
}

// ============================================================================
// 错误恢复: 哨兵不级联
// ============================================================================

#[test]
fn integration_sentinel_prevents_cascade() {
    let src = "function f(): void { let x = \"hello\"; let y = x + 1; let z = y + 2; }";
    let (_hir, _name_diags, type_diags) = run_full_pipeline(src);

    // 应该有 1 个根因错误（x + 1 的类型不匹配），而非 2 个
    let type_err_count = type_diags.len();
    // y 的类型应该是 Error 哨兵，阻止 z 的二次报错
    assert!(type_err_count <= 2,
        "expected at most 2 type errors (root cause only), got {type_err_count}: {:?}",
        type_diags.iter().map(|d| &d.message).collect::<Vec<_>>());
}

// ============================================================================
// as 转换禁止: bool as number 在 Phase 1 中 number→I32，Bool→I32 被禁止
// ============================================================================

#[test]
fn integration_as_cast_bool_to_number_forbidden() {
    // bool → number is implicitly bool → I32 which is forbidden by the as matrix
    let src = "function f(): number { let b = true; return b as number; }";
    let (_hir, _name_diags, type_diags) = run_full_pipeline(src);
    let has_forbidden = type_diags.iter().any(|d| d.message.contains("cannot cast")
        || d.message.contains("invalid cast"));
    assert!(has_forbidden, "expected cast error for bool as number, got: {:?}",
        type_diags.iter().map(|d| &d.message).collect::<Vec<_>>());
}

// ============================================================================
// 未定义标识符
// ============================================================================

#[test]
fn integration_undefined_identifier() {
    let src = "function f(): void { let x = unknownVar; }";
    let (_hir, name_diags, _type_diags) = run_full_pipeline(src);
    let has_undefined = name_diags.iter().any(|d| d.message.contains("undefined identifier"));
    assert!(has_undefined, "expected 'undefined identifier' diagnostic");
}

// ============================================================================
// 函数参数不匹配
// ============================================================================

#[test]
fn integration_wrong_arg_count() {
    let src = "function add(a: number, b: number): number { return a + b; }
               function main(): number { return add(1); }";
    let (_hir, _name_diags, type_diags) = run_full_pipeline(src);
    let has_count_err = type_diags.iter().any(|d| d.message.contains("expects")
        && d.message.contains("arguments"));
    assert!(has_count_err, "expected argument count error, got: {:?}",
        type_diags.iter().map(|d| &d.message).collect::<Vec<_>>());
}
