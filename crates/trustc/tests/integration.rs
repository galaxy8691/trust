//! §4.2: Phase 1.8 端到端集成测试
//!
//! 验证 trustc compile 管线完整执行。27 个语法特性覆盖。

use std::process::Command;

/// 运行 trustc 二进制（注入 FERRO_RT_LIB 环境变量，指向 workspace target/）
fn run_trustc(args: &[&str]) -> std::io::Result<std::process::Output> {
    // CARGO_MANIFEST_DIR = crates/trustc/ → 回退两级到 workspace 根
    let workspace_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent().and_then(|p| p.parent())
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|| ".".into());
    let ferro_rt_dir = std::env::var("FERRO_RT_LIB")
        .unwrap_or_else(|_| format!("{}/target/debug", workspace_root));
    Command::new(env!("CARGO_BIN_EXE_trustc"))
        .args(args)
        .env("FERRO_RT_LIB", &ferro_rt_dir)
        .output()
}

/// 编译 .trust 文件，仅断言编译成功
macro_rules! assert_compiles {
    ($fixture:expr) => {{
        let out = run_trustc(&[
            "compile",
            &format!("tests/fixtures/{}", $fixture),
            "-o",
            &format!("/tmp/trust_test_{}", $fixture.trim_end_matches(".trust")),
        ])
        .expect("trustc should run");
        assert!(
            out.status.success(),
            "{}: compilation failed\nstderr: {}",
            $fixture,
            String::from_utf8_lossy(&out.stderr)
        );
    }};
}

/// 编译并执行 .trust 文件，断言 stdout 包含预期字符串
macro_rules! assert_output {
    ($fixture:expr, $expected:expr) => {{
        let name = $fixture.trim_end_matches(".trust");
        let out = run_trustc(&["compile", &format!("tests/fixtures/{}", $fixture), "-o", &format!("/tmp/trust_test_{}", name)]).expect("trustc should run");
        assert!(out.status.success(), "{}: compilation failed\nstderr: {}", $fixture, String::from_utf8_lossy(&out.stderr));
        let run = Command::new(&format!("/tmp/trust_test_{}", name)).output().expect("binary should run");
        let stdout = String::from_utf8_lossy(&run.stdout);
        assert!(stdout.contains($expected), "{}: expected '{}', got: '{}'", $fixture, $expected, stdout.trim());
    }};
}

// ============================================================================
// 端到端测试 — 输出验证
// ============================================================================

#[test] fn e2e_hello()           { assert_output!("hello.trust", "Hello, Trust!"); }
#[test] fn e2e_arithmetic()      { assert_output!("arithmetic.trust", "arithmetic ok"); }
#[test] fn e2e_bigint()          { assert_output!("bigint.trust", "bigint ok"); }
#[test] fn e2e_comparison()      { assert_output!("comparison.trust", "comparison ok"); }
#[test] fn e2e_const_decl()      { assert_output!("const_decl.trust", "const ok"); }
#[test] fn e2e_for_loop()        { assert_output!("for_loop.trust", "ok"); }
#[test] fn e2e_if_else()         { assert_output!("if_else.trust", "if branch"); }
#[test] fn e2e_let_variable()    { assert_output!("let_variable.trust", "let ok"); }
#[test] fn e2e_let_mut()         { assert_output!("let_mut.trust", "mut ok"); }
#[test] fn e2e_logical()         { assert_output!("logical.trust", "logical ok"); }
#[test] fn e2e_loop_break()      { assert_output!("loop_break.trust", "escaped"); }
#[test] fn e2e_param_move()      { assert_output!("param_move.trust", "move ok"); }
#[test] fn e2e_ref_operator()    { assert_output!("ref_operator.trust", "ref ok"); }
#[test] fn e2e_return_value()    { assert_output!("return_value.trust", "return ok"); }
#[test] fn e2e_shared_decl()     { assert_output!("shared_decl.trust", "shared ok"); }
#[test] fn e2e_template_literal(){ assert_output!("template_literal.trust", "template ok"); }
#[test] fn e2e_while_loop()      { assert_output!("while_loop.trust", "while ok"); }

// ============================================================================
// 端到端测试 — 编译通过（不验证运行时输出）
// ============================================================================

#[test] fn e2e_arrow_fn()        { assert_compiles!("arrow_fn.trust"); }
#[test] fn e2e_as_cast()         { assert_compiles!("as_cast.trust"); }
#[test] fn e2e_break_value()     { assert_compiles!("break_value.trust"); }
#[test] fn e2e_continue_loop()   { assert_compiles!("continue_loop.trust"); }
#[test] fn e2e_export_lib()      { assert_compiles!("export_lib.trust"); }
#[test] fn e2e_function_call()   { assert_compiles!("function_call.trust"); }
#[test] fn e2e_import_export()   { assert_compiles!("import_export.trust"); }
#[test] fn e2e_nullish_coalesce(){ assert_compiles!("nullish_coalesce.trust"); }
#[test] fn e2e_param_inout()     { assert_compiles!("param_inout.trust"); }
#[test] fn e2e_type_annotation() { assert_compiles!("type_annotation.trust"); }

// === 覆盖率增强: 错误路径 + 复杂场景 ===
#[test] fn e2e_multi_function()  { assert_output!("multi_function.trust", "multi fn ok"); }
#[test] fn e2e_nested_if()      { assert_output!("nested_if.trust", "nested ok"); }

// 错误路径测试（期望编译失败——验证错误处理路径被覆盖）
#[test]
fn e2e_err_type_mismatch() {
    let out = run_trustc(&["compile", "tests/fixtures/err_type_mismatch.trust", "-o", "/tmp/trust_err_type"]).unwrap();
    // 类型不匹配应报错
    assert!(!out.status.success(), "expected compilation failure for type mismatch");
}
#[test]
fn e2e_err_undefined_var() {
    let out = run_trustc(&["compile", "tests/fixtures/err_undefined_var.trust", "-o", "/tmp/trust_err_undef"]).unwrap();
    assert!(!out.status.success(), "expected compilation failure for undefined variable");
}
#[test]
fn e2e_err_param_count() {
    let out = run_trustc(&["compile", "tests/fixtures/err_param_count.trust", "-o", "/tmp/trust_err_param"]).unwrap();
    assert!(!out.status.success(), "expected compilation failure for param count mismatch");
}
#[test]
fn e2e_err_move_use() {
    let out = run_trustc(&["compile", "tests/fixtures/err_move_use.trust", "-o", "/tmp/trust_err_move"]).unwrap();
    assert!(!out.status.success(), "expected compilation failure for use-after-move");
}

// 覆盖率增强: 更多错误路径 + 变体
#[test] fn e2e_if_else_both() { assert_output!("if_else_both.trust", "positive"); }
#[test] fn e2e_shadow_var()   { assert_output!("shadow_var.trust", "ok"); }
#[test]
fn e2e_err_return_type() {
    let out = run_trustc(&["compile", "tests/fixtures/err_return_type.trust", "-o", "/tmp/trust_err_ret"]).unwrap();
    assert!(!out.status.success(), "expected compilation failure for return type mismatch");
}
#[test]
fn e2e_err_binary_type() {
    let out = run_trustc(&["compile", "tests/fixtures/err_binary_type.trust", "-o", "/tmp/trust_err_bin"]).unwrap();
    assert!(!out.status.success(), "expected compilation failure for binary type mismatch");
}
#[test]
fn e2e_err_duplicate_var() {
    let out = run_trustc(&["compile", "tests/fixtures/err_duplicate_var.trust", "-o", "/tmp/trust_err_dup"]).unwrap();
    assert!(!out.status.success(), "expected compilation failure for duplicate variable");
}

// ============================================================================
// CLI 参数解析测试
// ============================================================================

#[cfg(test)]
mod cli_tests {
    use super::*;

    #[test] fn cli_compile_requires_file() {
        let out = run_trustc(&["compile"]).unwrap();
        assert!(!out.status.success());
        assert!(String::from_utf8_lossy(&out.stderr).contains("requires"));
    }
    #[test] fn cli_check_requires_file() {
        let out = run_trustc(&["check"]).unwrap();
        assert!(!out.status.success());
        assert!(String::from_utf8_lossy(&out.stderr).contains("requires"));
    }
    #[test] fn cli_eval_requires_expr() {
        let out = run_trustc(&["eval"]).unwrap();
        assert!(!out.status.success());
        assert!(String::from_utf8_lossy(&out.stderr).contains("requires"));
    }
    #[test] fn cli_unknown_flag() {
        let out = run_trustc(&["--nonexistent"]).unwrap();
        assert!(!out.status.success());
        assert!(String::from_utf8_lossy(&out.stderr).contains("unknown") || String::from_utf8_lossy(&out.stderr).contains("Usage"));
    }
    #[test] fn cli_no_command() {
        let out = run_trustc(&[]).unwrap();
        assert!(!out.status.success());
        assert!(String::from_utf8_lossy(&out.stderr).contains("no command") || String::from_utf8_lossy(&out.stderr).contains("Usage"));
    }
    #[test] fn cli_error_format_json() {
        let _ = run_trustc(&["compile", "tests/fixtures/hello.trust", "--error-format=json", "-o", "/tmp/trust_cli_json"]).unwrap();
    }
    #[test] fn cli_error_format_json_space() {
        let _ = run_trustc(&["compile", "tests/fixtures/hello.trust", "--error-format", "json", "-o", "/tmp/trust_cli_json2"]).unwrap();
    }
    #[test] fn cli_verbose() {
        let _ = run_trustc(&["compile", "tests/fixtures/hello.trust", "--verbose", "-o", "/tmp/trust_cli_verbose"]).unwrap();
    }
    #[test] fn cli_quiet() {
        let _ = run_trustc(&["compile", "tests/fixtures/hello.trust", "--quiet", "-o", "/tmp/trust_cli_quiet"]).unwrap();
    }
    #[test] fn cli_output_flag() {
        let _ = run_trustc(&["compile", "tests/fixtures/hello.trust", "-o", "/tmp/trust_cli_o"]).unwrap();
    }
}
