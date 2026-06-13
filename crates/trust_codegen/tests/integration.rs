//! 集成测试 — Phase 1.5 trust_codegen
//!
//! .trust 源码 → parse → HIR → TIR → codegen → Rust 源码 → rustc 编译验证

use std::process::Command;
use trust_codegen::codegen::generate_rust;
use trust_hir::name_res::resolve_names;
use trust_hir::typeck::check_types;
use trust_parser::module_graph::ModuleGraph;
use trust_tir::borrowck::check_borrows;
use trust_tir::moveck::check_moves;
use trust_tir::tir::{lower_hir, TirProgram};

fn run_pipeline(src: &str) -> (TirProgram, String) {
    let mut p = trust_parser::parser::Parser::new(src, "test.trust");
    let prog = p.parse_program();
    let mg = ModuleGraph::new();
    let mut diags = vec![];
    let mut hir = resolve_names(&prog, &mg, &mut diags);
    let _ = check_types(&mut hir, &mut diags);

    if !diags.is_empty() {
        eprintln!("HIR diagnostics: {:?}", diags);
        panic!("HIR phase failed");
    }

    let tir = lower_hir(&hir).unwrap_or_else(|e| {
        eprintln!("lower_hir errors: {:?}", e);
        panic!("lower_hir failed");
    });

    // 运行所有权检查（TIR 错误 = 0 才 codegen）
    let move_ok = check_moves(&tir).is_ok();
    let borrow_ok = check_borrows(&tir).is_ok();
    if !move_ok || !borrow_ok {
        eprintln!("TIR ownership check failed");
        panic!("TIR ownership check failed");
    }

    let (rust_code, _source_map) = generate_rust(&tir).unwrap_or_else(|e| {
        eprintln!("codegen errors: {:?}", e);
        panic!("codegen failed");
    });

    (tir, rust_code)
}

/// 验证生成的 Rust 代码可通过 rustc 编译
fn verify_compiles(rust_code: &str) -> bool {
    let temp_dir = std::env::temp_dir().join("trust_codegen_test");
    let _ = std::fs::create_dir_all(&temp_dir);
    let rs_file = temp_dir.join("test_output.rs");
    std::fs::write(&rs_file, rust_code).expect("write test_output.rs");

    // 找到 ferro_rt 的构建输出目录
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let ferro_rt_lib = std::path::Path::new(manifest_dir)
        .join("../../target/debug")
        .canonicalize()
        .unwrap_or_else(|_| {
            std::path::Path::new(manifest_dir)
                .join("../target/debug")
                .canonicalize()
                .unwrap_or_else(|_| std::path::Path::new("target/debug").to_path_buf())
        });

    let output = Command::new("rustc")
        .args([
            "--edition", "2021",
            "--crate-type", "bin",
            "-L", ferro_rt_lib.to_str().unwrap(),
            "--extern", &format!("ferro_rt={}/libferro_rt.rlib", ferro_rt_lib.display()),
            "-o", temp_dir.join("test_output").to_str().unwrap(),
            rs_file.to_str().unwrap(),
        ])
        .output()
        .expect("run rustc");

    if !output.status.success() {
        eprintln!(
            "rustc failed:\n{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    output.status.success()
}

// ============================================================================
// 基础测试
// ============================================================================

#[test]
fn gen_empty_function() {
    let src = "function f(): void {}";
    let (_tir, rust) = run_pipeline(src);
    assert!(rust.contains("fn f()"), "should contain fn f(): {}", rust);
    assert!(rust.contains("}"), "should have closing brace");
}

#[test]
fn gen_variable_decl_i32() {
    let src = "function f(): void { let x = 42; }";
    let (_tir, rust) = run_pipeline(src);
    assert!(rust.contains("let "), "should have let: {}", rust);
    assert!(rust.contains("42"), "should have 42: {}", rust);
}

#[test]
fn gen_variable_decl_f64() {
    let src = "function f(): void { let x = 3.14; }";
    let (_tir, rust) = run_pipeline(src);
    assert!(rust.contains("3.1"), "should have float literal: {}", rust);
}

#[test]
fn gen_variable_reference_copy() {
    let src = "function f(): void { let x = 42; let y = x; }";
    let (_tir, rust) = run_pipeline(src);
    assert!(rust.contains("let "), "should have let bindings: {}", rust);
}

#[test]
fn gen_string_move() {
    let src = "function f(): void { let a = \"hello\"; let b = a; }";
    let (_tir, rust) = run_pipeline(src);
    assert!(rust.contains("\"hello\""), "should have string: {}", rust);
}

#[test]
fn gen_borrow_shared() {
    let src = "function f(): void { let x = 42; let r = &x; }";
    let (_tir, rust) = run_pipeline(src);
    assert!(rust.contains("&"), "should have reference: {}", rust);
}

#[test]
// N2 fix: 原 gen_borrow_shared (L130) 与本测试重复，删除此重复测试

#[test]
fn gen_binary_op() {
    let src = "function f(): void { let x = 10; let y = 20; let z = x + y; }";
    let (_tir, rust) = run_pipeline(src);
    assert!(rust.contains("+"), "should have addition: {}", rust);
}

#[test]
fn gen_function_call() {
    let src = "function add(a: number, b: number): number { return a + b; }
               function f(): void { let x = add(1, 2); }";
    let (_tir, rust) = run_pipeline(src);
    assert!(rust.contains("fn add"), "should have fn add: {}", rust);
    assert!(rust.contains("fn f"), "should have fn f: {}", rust);
}

#[test]
fn gen_if_expr() {
    let src = "function f(): number { let x = if (true) { 1 } else { 0 }; return x; }";
    let (_tir, rust) = run_pipeline(src);
    assert!(rust.contains("if"), "should have if: {}", rust);
    assert!(rust.contains("else"), "should have else: {}", rust);
}

#[test]
fn gen_return_stmt() {
    let src = "function f(): number { return 42; }";
    let (_tir, rust) = run_pipeline(src);
    assert!(rust.contains("return"), "should have return: {}", rust);
}

#[test]
fn gen_param_default() {
    let src = "function echo(x: number): number { return x; }";
    let (_tir, rust) = run_pipeline(src);
    assert!(rust.contains("&"), "default param should be &: {}", rust);
    assert!(rust.contains("i32"), "should have i32 type: {}", rust);
}

#[test]
fn gen_param_inout() {
    let src = "function inc(inout x: number): void {}";
    let (_tir, rust) = run_pipeline(src);
    assert!(rust.contains("&mut"), "inout param should be &mut: {}", rust);
}

#[test]
fn gen_param_move() {
    let src = "function consume(move x: string): void {}";
    let (_tir, rust) = run_pipeline(src);
    assert!(rust.contains("String"), "should have String type: {}", rust);
    // move 参数不加 &
    assert!(rust.contains("x: String"), "move param should be bare T: {}", rust);
}

#[test]
fn gen_multiple_params() {
    let src = "function f(a: number, inout b: number, move c: string): void {}";
    let (_tir, rust) = run_pipeline(src);
    assert!(rust.contains("&"), "should have ref: {}", rust);
    assert!(rust.contains("&mut"), "should have mut ref: {}", rust);
    assert!(rust.contains("String"), "should have String: {}", rust);
}

#[test]
fn gen_console_log() {
    let src = "function main(): void { console.log(\"Hello, Trust!\"); }";
    let (_tir, rust) = run_pipeline(src);
    assert!(
        rust.contains("ferro_rt::console::log") || rust.contains("console.log"),
        "should have console.log mapping: {}",
        rust
    );
}

#[test]
fn gen_empty_main_wrapper() {
    // 没有 main 函数时，应生成空的 fn main()
    let src = "function f(): void {}";
    let (_tir, rust) = run_pipeline(src);
    assert!(rust.contains("fn main()"), "should have fn main wrapper: {}", rust);
}

#[test]
fn gen_main_wrapper_present() {
    // B3 fix: 原 rustc_compile_simple 名误导（不做实际 rustc 编译）。
    // 重命名为 gen_main_wrapper_present，验证 fn main() 包装生成。
    let src = "function main(): void {}";
    let (_tir, rust) = run_pipeline(src);
    assert!(rust.contains("fn main"), "should have fn main: {}", rust);
    assert!(rust.contains("{"), "should have braces");
}

#[test]
fn gen_console_log_mapping() {
    // B3 fix: 原 rustc_compile_console_log 名误导。
    // 重命名为 gen_console_log_mapping，验证 console.log → ferro_rt 映射。
    let src = "function main(): void { console.log(\"hello\"); }";
    let (_tir, rust) = run_pipeline(src);
    assert!(
        rust.contains("ferro_rt::console::log") || rust.contains("console.log"),
        "should map console.log: {}",
        rust
    );
}

/// A4 fix: 真正验证 rustc 编译（需要 cargo build ferro_rt 先）
#[test]
#[ignore = "requires cargo build ferro_rt first"]
fn verify_rustc_compiles() {
    let src = "function main(): void { console.log(\"hello\"); }";
    let (_tir, rust) = run_pipeline(src);
    assert!(verify_compiles(&rust), "generated code should compile with rustc");
}
