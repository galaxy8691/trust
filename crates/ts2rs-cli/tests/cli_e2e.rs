//! 集成测试：TS → Rust → cargo → 运行可执行文件。

use std::path::PathBuf;
use std::process::Command;

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

fn regression_case(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/regression")
        .join(name)
}

fn assert_run_stdout(name: &str, expected: &str) {
    let exe = PathBuf::from(env!("CARGO_BIN_EXE_ts2rs"));
    let ts = fixture(name);
    let out = Command::new(exe)
        .args(["run", ts.to_str().unwrap()])
        .output()
        .expect("spawn ts2rs run");
    assert!(
        out.status.success(),
        "stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&out.stdout), expected);
}

#[test]
fn run_prints_main_result() {
    let exe = PathBuf::from(env!("CARGO_BIN_EXE_ts2rs"));
    let ts = fixture("sample.ts");
    let out = Command::new(exe)
        .args(["run", ts.to_str().unwrap()])
        .output()
        .expect("spawn ts2rs run");

    assert!(
        out.status.success(),
        "stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&out.stdout), "3\n");
}

#[test]
fn compile_writes_rust() {
    let exe = PathBuf::from(env!("CARGO_BIN_EXE_ts2rs"));
    let ts = fixture("sample.ts");
    let dir = tempfile::tempdir().expect("tempdir");
    let rs_path = dir.path().join("out.rs");
    let status = Command::new(exe)
        .args([
            "compile",
            ts.to_str().unwrap(),
            "-o",
            rs_path.to_str().unwrap(),
        ])
        .status()
        .expect("spawn ts2rs compile");
    assert!(status.success());
    let body = std::fs::read_to_string(&rs_path).expect("read out.rs");
    assert!(body.contains("fn ts_main"));
    assert!(body.contains("fn add"));
}

#[test]
fn compile_span_comments_writes_ts_anchors() {
    let exe = PathBuf::from(env!("CARGO_BIN_EXE_ts2rs"));
    let ts = fixture("sample.ts");
    let dir = tempfile::tempdir().expect("tempdir");
    let rs_path = dir.path().join("out.rs");
    let status = Command::new(exe)
        .args([
            "compile",
            ts.to_str().unwrap(),
            "-o",
            rs_path.to_str().unwrap(),
            "--span-comments",
        ])
        .status()
        .expect("spawn ts2rs compile");
    assert!(status.success());
    let body = std::fs::read_to_string(&rs_path).expect("read out.rs");
    assert!(
        body.contains("// ts:"),
        "expected // ts: span comments in generated Rust: {body}"
    );
}

#[test]
fn compile_console_stderr_writes_eprintln() {
    let exe = PathBuf::from(env!("CARGO_BIN_EXE_ts2rs"));
    let ts = fixture("console_stderr.ts");
    let dir = tempfile::tempdir().expect("tempdir");
    let rs_path = dir.path().join("out.rs");
    let status = Command::new(exe)
        .args([
            "compile",
            ts.to_str().unwrap(),
            "-o",
            rs_path.to_str().unwrap(),
        ])
        .status()
        .expect("spawn ts2rs compile");
    assert!(status.success());
    let body = std::fs::read_to_string(&rs_path).expect("read out.rs");
    assert!(
        body.contains("eprintln!"),
        "expected console.error/debug to lower to eprintln!: {body}"
    );
}

#[test]
fn run_let_if_prints_ten() {
    let exe = PathBuf::from(env!("CARGO_BIN_EXE_ts2rs"));
    let ts = fixture("let_if.ts");
    let out = Command::new(exe)
        .args(["run", ts.to_str().unwrap()])
        .output()
        .expect("spawn ts2rs run");
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&out.stdout), "10\n");
}

#[test]
fn compile_void_main_has_no_println_value() {
    let exe = PathBuf::from(env!("CARGO_BIN_EXE_ts2rs"));
    let ts = fixture("void_log.ts");
    let dir = tempfile::tempdir().expect("tempdir");
    let rs_path = dir.path().join("out.rs");
    let status = Command::new(exe)
        .args([
            "compile",
            ts.to_str().unwrap(),
            "-o",
            rs_path.to_str().unwrap(),
        ])
        .status()
        .expect("spawn ts2rs compile");
    assert!(status.success());
    let body = std::fs::read_to_string(&rs_path).expect("read out.rs");
    assert!(body.contains("fn ts_main() -> ()"));
    assert!(body.contains("println!"));
    assert!(!body.contains("println!(\"{}\", ts_main())"));
}

#[test]
fn compile_async_mvp_writes_tokio_and_await() {
    let exe = PathBuf::from(env!("CARGO_BIN_EXE_ts2rs"));
    let ts = fixture("async_mvp_compile_ok.ts");
    let dir = tempfile::tempdir().expect("tempdir");
    let rs_path = dir.path().join("out.rs");
    let status = Command::new(exe)
        .args([
            "compile",
            ts.to_str().unwrap(),
            "-o",
            rs_path.to_str().unwrap(),
        ])
        .status()
        .expect("spawn ts2rs compile");
    assert!(status.success());
    let body = std::fs::read_to_string(&rs_path).expect("read out.rs");
    assert!(body.contains("#[tokio::main]"), "{body}");
    assert!(body.contains("async fn ts_main"), "{body}");
    assert!(body.contains(".await"), "{body}");
    assert!(body.contains("__ts2rs_fetch_text"), "{body}");
}

#[test]
fn compile_async_control_flow_if_while_await_ok() {
    let exe = PathBuf::from(env!("CARGO_BIN_EXE_ts2rs"));
    let ts = fixture("async_control_flow_ok.ts");
    let dir = tempfile::tempdir().expect("tempdir");
    let rs_path = dir.path().join("out.rs");
    let status = Command::new(exe)
        .args([
            "compile",
            ts.to_str().unwrap(),
            "-o",
            rs_path.to_str().unwrap(),
        ])
        .status()
        .expect("spawn ts2rs compile");
    assert!(status.success());
    let body = std::fs::read_to_string(&rs_path).expect("read out.rs");
    assert!(body.contains("#[tokio::main]"), "{body}");
    assert!(body.contains("async fn ts_main"), "{body}");
    assert!(body.contains(".await"), "{body}");
    assert!(body.contains("if ("), "{body}");
    assert!(body.contains("while "), "{body}");
    assert!(body.contains("__ts2rs_fetch_text"), "{body}");
}

#[test]
fn compile_promise_all_fetch_alias_ok() {
    let exe = PathBuf::from(env!("CARGO_BIN_EXE_ts2rs"));
    let ts = fixture("promise_all_fetch_ok.ts");
    let dir = tempfile::tempdir().expect("tempdir");
    let rs_path = dir.path().join("out.rs");
    let status = Command::new(exe)
        .args([
            "compile",
            ts.to_str().unwrap(),
            "-o",
            rs_path.to_str().unwrap(),
        ])
        .status()
        .expect("spawn ts2rs compile");
    assert!(status.success());
    let body = std::fs::read_to_string(&rs_path).expect("read out.rs");
    assert!(body.contains("vec!["), "{body}");
    assert!(body.contains("__ts2rs_fetch("), "{body}");
    assert!(body.contains(".await"), "{body}");
}

#[test]
fn compile_promise_then_fails() {
    assert_compile_fails_stderr("promise_then_fail.ts", "Promise.prototype.then");
}

#[test]
fn compile_fetch_response_ok() {
    let exe = PathBuf::from(env!("CARGO_BIN_EXE_ts2rs"));
    let ts = fixture("fetch_response_ok.ts");
    let dir = tempfile::tempdir().expect("tempdir");
    let rs_path = dir.path().join("out.rs");
    let status = Command::new(exe)
        .args([
            "compile",
            ts.to_str().unwrap(),
            "-o",
            rs_path.to_str().unwrap(),
        ])
        .status()
        .expect("spawn ts2rs compile");
    assert!(status.success());
    let body = std::fs::read_to_string(&rs_path).expect("read out.rs");
    assert!(body.contains("__ts2rs_fetch("), "{body}");
    assert!(body.contains(".status()"), "{body}");
    assert!(body.contains(".text()"), "{body}");
}

#[test]
fn compile_fetch_stream_ok() {
    let exe = PathBuf::from(env!("CARGO_BIN_EXE_ts2rs"));
    let ts = fixture("fetch_stream_ok.ts");
    let dir = tempfile::tempdir().expect("tempdir");
    let rs_path = dir.path().join("out.rs");
    let status = Command::new(exe)
        .args([
            "compile",
            ts.to_str().unwrap(),
            "-o",
            rs_path.to_str().unwrap(),
        ])
        .status()
        .expect("spawn ts2rs compile");
    assert!(status.success());
    let body = std::fs::read_to_string(&rs_path).expect("read out.rs");
    assert!(body.contains("bytes_stream()"), "{body}");
    assert!(body.contains("__Ts2rsStreamReadResult"), "{body}");
    assert!(body.contains("futures_util"), "{body}");
}

#[test]
fn compile_fetch_post_init_ok() {
    let exe = PathBuf::from(env!("CARGO_BIN_EXE_ts2rs"));
    let ts = fixture("fetch_post_init_ok.ts");
    let dir = tempfile::tempdir().expect("tempdir");
    let rs_path = dir.path().join("out.rs");
    let status = Command::new(exe)
        .args([
            "compile",
            ts.to_str().unwrap(),
            "-o",
            rs_path.to_str().unwrap(),
        ])
        .status()
        .expect("spawn ts2rs compile");
    assert!(status.success());
    let body = std::fs::read_to_string(&rs_path).expect("read out.rs");
    assert!(body.contains("__Ts2rsFetchInit"), "{body}");
    assert!(body.contains("\"POST\""), "{body}");
}

// --- §3.1 符号表 / void+log 分支 ---

#[test]
fn compile_let_dup_same_block_fails() {
    assert_compile_fails_stderr("let_dup_same_block_fail.ts", "duplicate");
}

#[test]
fn run_let_shadow_nested_ok() {
    assert_run_stdout("let_shadow_nested_ok.ts", "2\n");
}

#[test]
fn compile_param_let_duplicate_fails() {
    assert_compile_fails_stderr("param_let_same_name_fail.ts", "duplicate");
}

#[test]
fn compile_void_log_in_branch_ok() {
    let exe = PathBuf::from(env!("CARGO_BIN_EXE_ts2rs"));
    let ts = fixture("void_log_in_branch.ts");
    let dir = tempfile::tempdir().expect("tempdir");
    let rs_path = dir.path().join("out.rs");
    let status = Command::new(exe)
        .args([
            "compile",
            ts.to_str().unwrap(),
            "-o",
            rs_path.to_str().unwrap(),
        ])
        .status()
        .expect("spawn ts2rs compile");
    assert!(status.success());
    let body = std::fs::read_to_string(&rs_path).expect("read out.rs");
    assert!(body.contains("println!"));
}

#[test]
fn run_void_log_in_branch_prints_branch() {
    assert_run_stdout("void_log_in_branch.ts", "branch\n");
}

#[test]
fn run_literal_type_ok_prints_eight() {
    assert_run_stdout("literal_type_ok.ts", "8\n");
}

#[test]
fn compile_literal_type_narrowing_fails() {
    assert_compile_fails_stderr("literal_type_fail.ts", "initializer type mismatch");
}

#[test]
fn run_union_literal_ok_prints_two() {
    assert_run_stdout("union_literal_ok.ts", "2\n");
}

#[test]
fn run_union_cond_ok_prints_two() {
    assert_run_stdout("union_cond_ok.ts", "2\n");
}

#[test]
fn compile_union_heterogeneous_main_fails() {
    assert_compile_fails_stderr("union_heterogeneous_fail.ts", "heterogeneous");
}

#[test]
fn compile_intersection_type_fails() {
    assert_compile_fails_stderr(
        "intersection_type_fail.ts",
        "intersection types are not supported",
    );
}

#[test]
fn compile_union_mixed_cond_fails() {
    assert_compile_fails_stderr("union_mixed_cond_fail.ts", "union of one primitive family");
}

#[test]
fn run_interface_ok_prints_three() {
    assert_run_stdout("interface_ok.ts", "3\n");
}

#[test]
fn run_export_interface_ok_prints_twelve() {
    assert_run_stdout("export_interface_ok.ts", "12\n");
}

#[test]
fn compile_interface_extends_fails() {
    assert_compile_fails_stderr(
        "interface_extends_fail.ts",
        "interface extends clauses are not supported",
    );
}

#[test]
fn run_interface_generic_ok_prints_zero() {
    assert_run_stdout("interface_generic_fail.ts", "0\n");
}

#[test]
fn run_generic_function_ok_prints_three() {
    assert_run_stdout("generic_function_ok.ts", "3\n");
}

#[test]
fn compile_generic_function_missing_type_args_fails() {
    assert_compile_fails_stderr(
        "generic_function_missing_type_args_fail.ts",
        "requires explicit type arguments",
    );
}

#[test]
fn run_type_alias_ok_prints_three() {
    assert_run_stdout("type_alias_ok.ts", "3\n");
}

#[test]
fn run_type_alias_to_interface_ok_prints_three() {
    assert_run_stdout("type_alias_to_interface_ok.ts", "3\n");
}

#[test]
fn run_export_type_alias_ok_prints_ten() {
    assert_run_stdout("export_type_alias_ok.ts", "10\n");
}

#[test]
fn run_type_alias_generic_ok_prints_zero() {
    assert_run_stdout("type_alias_generic_fail.ts", "0\n");
}

#[test]
fn compile_type_alias_dup_fails() {
    assert_compile_fails_stderr("type_alias_dup_fail.ts", "duplicate type name `A`");
}

#[test]
fn compile_multi_fn_sem_errors_reports_two_diagnostic_lines() {
    let exe = PathBuf::from(env!("CARGO_BIN_EXE_ts2rs"));
    let ts = fixture("multi_diag_two_fn_return_fail.ts");
    let dir = tempfile::tempdir().expect("tempdir");
    let rs_path = dir.path().join("out.rs");
    let out = Command::new(exe)
        .args([
            "compile",
            ts.to_str().unwrap(),
            "-o",
            rs_path.to_str().unwrap(),
        ])
        .output()
        .expect("spawn ts2rs compile");
    assert!(
        !out.status.success(),
        "expected compile to fail for multi_diag_two_fn_return_fail.ts"
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    let needle = "multi_diag_two_fn_return_fail.ts";
    let lines_with_path = stderr
        .lines()
        .filter(|l| l.contains(needle) && l.matches(':').count() >= 3)
        .count();
    assert!(
        lines_with_path >= 2,
        "expected at least two path:line:col lines in stderr, got:\n{stderr}"
    );
}

// --- 1.0 matrix fixtures ---

#[test]
fn run_export_main_prints_one() {
    assert_run_stdout("export_main.ts", "1\n");
}

#[test]
fn compile_export_main_writes_ts_main() {
    let exe = PathBuf::from(env!("CARGO_BIN_EXE_ts2rs"));
    let ts = fixture("export_main.ts");
    let dir = tempfile::tempdir().expect("tempdir");
    let rs_path = dir.path().join("out.rs");
    let status = Command::new(exe)
        .args([
            "compile",
            ts.to_str().unwrap(),
            "-o",
            rs_path.to_str().unwrap(),
        ])
        .status()
        .expect("spawn ts2rs compile");
    assert!(status.success());
    let body = std::fs::read_to_string(&rs_path).expect("read out.rs");
    assert!(body.contains("fn ts_main"));
    assert!(body.contains("println!(\"{}\", ts_main())"));
}

#[test]
fn run_while_early_prints_three() {
    assert_run_stdout("while_early.ts", "3\n");
}

#[test]
fn compile_while_early_writes_loop() {
    let exe = PathBuf::from(env!("CARGO_BIN_EXE_ts2rs"));
    let ts = fixture("while_early.ts");
    let dir = tempfile::tempdir().expect("tempdir");
    let rs_path = dir.path().join("out.rs");
    let status = Command::new(exe)
        .args([
            "compile",
            ts.to_str().unwrap(),
            "-o",
            rs_path.to_str().unwrap(),
        ])
        .status()
        .expect("spawn ts2rs compile");
    assert!(status.success());
    let body = std::fs::read_to_string(&rs_path).expect("read out.rs");
    assert!(body.contains("while "));
}

#[test]
fn run_boolean_if_prints_42() {
    assert_run_stdout("boolean_if.ts", "42\n");
}

#[test]
fn run_string_concat_prints_99() {
    assert_run_stdout("string_concat.ts", "99\n");
}

#[test]
fn compile_string_concat_uses_strings() {
    let exe = PathBuf::from(env!("CARGO_BIN_EXE_ts2rs"));
    let ts = fixture("string_concat.ts");
    let dir = tempfile::tempdir().expect("tempdir");
    let rs_path = dir.path().join("out.rs");
    let status = Command::new(exe)
        .args([
            "compile",
            ts.to_str().unwrap(),
            "-o",
            rs_path.to_str().unwrap(),
        ])
        .status()
        .expect("spawn ts2rs compile");
    assert!(status.success());
    let body = std::fs::read_to_string(&rs_path).expect("read out.rs");
    assert!(body.contains("to_string()") || body.contains("format!"));
}

#[test]
fn run_ops_prints_six() {
    assert_run_stdout("ops.ts", "6\n");
}

#[test]
fn compile_import_fails_with_message() {
    let exe = PathBuf::from(env!("CARGO_BIN_EXE_ts2rs"));
    let ts = fixture("import_fail.ts");
    let dir = tempfile::tempdir().expect("tempdir");
    let rs_path = dir.path().join("out.rs");
    let out = Command::new(exe)
        .args([
            "compile",
            ts.to_str().unwrap(),
            "-o",
            rs_path.to_str().unwrap(),
        ])
        .output()
        .expect("spawn ts2rs compile");
    assert!(
        !out.status.success(),
        "expected compile to fail:\nstdout={}\nstderr={}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("import"),
        "expected stderr to mention import, got:\n{stderr}"
    );
}

#[test]
fn run_import_fails() {
    let exe = PathBuf::from(env!("CARGO_BIN_EXE_ts2rs"));
    let ts = fixture("import_fail.ts");
    let out = Command::new(exe)
        .args(["run", ts.to_str().unwrap()])
        .output()
        .expect("spawn ts2rs run");
    assert!(!out.status.success());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("import"), "stderr:\n{stderr}");
}

fn assert_compile_fails_stderr(fixture_name: &str, needle: &str) {
    let exe = PathBuf::from(env!("CARGO_BIN_EXE_ts2rs"));
    let ts = fixture(fixture_name);
    let dir = tempfile::tempdir().expect("tempdir");
    let rs_path = dir.path().join("out.rs");
    let out = Command::new(exe)
        .args([
            "compile",
            ts.to_str().unwrap(),
            "-o",
            rs_path.to_str().unwrap(),
        ])
        .output()
        .expect("spawn ts2rs compile");
    assert!(
        !out.status.success(),
        "expected compile to fail for {fixture_name}"
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains(needle),
        "expected stderr to contain {needle:?}, got:\n{stderr}"
    );
}

#[test]
fn compile_export_named_fails() {
    assert_compile_fails_stderr("export_named_fail.ts", "export { ... }");
}

#[test]
fn compile_export_default_fails() {
    assert_compile_fails_stderr("export_default_fail.ts", "export default");
}

#[test]
fn compile_export_from_fails() {
    assert_compile_fails_stderr("export_from_fail.ts", "export * from");
}

#[test]
fn compile_export_var_fails() {
    assert_compile_fails_stderr("export_var_fail.ts", "unsupported export declaration");
}

// --- §1.2 statement extensions ---

#[test]
fn run_empty_stmt_prints_seven() {
    assert_run_stdout("empty_stmt.ts", "7\n");
}

#[test]
fn run_const_ok_prints_42() {
    assert_run_stdout("const_ok.ts", "42\n");
}

#[test]
fn run_assign_simple_prints_five() {
    assert_run_stdout("assign_simple.ts", "5\n");
}

#[test]
fn run_nested_fn_prints_nine() {
    assert_run_stdout("nested_fn.ts", "9\n");
}

#[test]
fn run_hof_apply_ok_prints_five() {
    assert_run_stdout("hof_apply_ok.ts", "5\n");
}

#[test]
fn run_hof_return_closure_ok_prints_seven() {
    assert_run_stdout("hof_return_closure_ok.ts", "7\n");
}

#[test]
fn run_for_loop_prints_four() {
    assert_run_stdout("for_loop.ts", "4\n");
}

#[test]
fn run_for_in_object_keys_ok_prints_three() {
    assert_run_stdout("for_in_object_keys_ok.ts", "3\n");
}

#[test]
fn run_for_in_object_keys_sum_ok_prints_six() {
    assert_run_stdout("for_in_object_keys_sum_ok.ts", "6\n");
}

#[test]
fn compile_for_in_non_object_fails() {
    assert_compile_fails_stderr("for_in_non_object_fail.ts", "for..in right side must be");
}

#[test]
fn compile_for_in_key_type_mismatch_fails() {
    assert_compile_fails_stderr(
        "for_in_key_type_mismatch_fail.ts",
        "initializer type mismatch",
    );
}

#[test]
fn run_do_while_prints_three() {
    assert_run_stdout("do_while_count.ts", "3\n");
}

#[test]
fn run_break_while_prints_two() {
    assert_run_stdout("break_while.ts", "2\n");
}

#[test]
fn run_continue_while_prints_13() {
    assert_run_stdout("continue_while.ts", "13\n");
}

#[test]
fn run_import_add_main_prints_three() {
    assert_run_stdout("import_add_main.ts", "3\n");
}

#[test]
fn compile_import_missing_export_fails() {
    assert_compile_fails_stderr(
        "import_missing_export_main.ts",
        "no exported function `foo`",
    );
}

#[test]
fn compile_circular_import_fails() {
    assert_compile_fails_stderr("circular_a.ts", "circular import");
}

#[test]
fn compile_duplicate_exported_name_fails() {
    assert_compile_fails_stderr("dup_main.ts", "duplicate function");
}

#[test]
fn compile_const_reassign_fails() {
    assert_compile_fails_stderr("const_reassign_fail.ts", "cannot assign to `const`");
}

#[test]
fn compile_switch_fallthrough_fails() {
    assert_compile_fails_stderr("switch_fail.ts", "fall-through");
}

#[test]
fn check_sample_ok() {
    let exe = PathBuf::from(env!("CARGO_BIN_EXE_ts2rs"));
    let ts = fixture("sample.ts");
    let out = Command::new(exe)
        .args(["check", ts.to_str().unwrap()])
        .output()
        .expect("spawn ts2rs check");
    assert!(
        out.status.success(),
        "stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn check_nullish_fn_union_ok() {
    let exe = PathBuf::from(env!("CARGO_BIN_EXE_ts2rs"));
    let ts = fixture("nullish_fn_ok.ts");
    let out = Command::new(exe)
        .args(["check", ts.to_str().unwrap()])
        .output()
        .expect("spawn ts2rs check nullish_fn_ok");
    assert!(
        out.status.success(),
        "stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn check_switch_fail_stderr() {
    let exe = PathBuf::from(env!("CARGO_BIN_EXE_ts2rs"));
    let ts = fixture("switch_fail.ts");
    let out = Command::new(exe)
        .args(["check", ts.to_str().unwrap()])
        .output()
        .expect("spawn ts2rs check");
    assert!(!out.status.success());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("fall-through"),
        "expected fall-through diagnostic, got:\n{stderr}"
    );
}

#[test]
fn regression_switch_fallthrough_check_fails() {
    let exe = PathBuf::from(env!("CARGO_BIN_EXE_ts2rs"));
    let ts = regression_case("switch_fallthrough_regression.ts");
    let out = Command::new(exe)
        .args(["check", ts.to_str().unwrap()])
        .output()
        .expect("spawn ts2rs check regression");
    assert!(!out.status.success());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("fall-through"),
        "expected fall-through diagnostic, got:\n{stderr}"
    );
}

#[test]
fn compile_array_return_type_mismatch_fails() {
    assert_compile_fails_stderr("array_fail.ts", "return type mismatch");
}

#[test]
fn compile_optional_call_bad_callee_fails() {
    assert_compile_fails_stderr("optional_chain_fail.ts", "optional call (`?.()`)");
}

#[test]
fn compile_nullish_operands_mismatch_fails() {
    assert_compile_fails_stderr("nullish_fail.ts", "nullish coalescing");
}

#[test]
fn compile_object_literal_non_number_field_fails() {
    assert_compile_fails_stderr("object_fail.ts", "object literal currently supports only");
}

#[test]
fn compile_emit_ir_stderr_contains_ir_module() {
    let exe = PathBuf::from(env!("CARGO_BIN_EXE_ts2rs"));
    let ts = fixture("sample.ts");
    let dir = tempfile::tempdir().expect("tempdir");
    let rs_path = dir.path().join("out.rs");
    let out = Command::new(exe)
        .args([
            "compile",
            ts.to_str().unwrap(),
            "-o",
            rs_path.to_str().unwrap(),
            "--emit-ir",
        ])
        .output()
        .expect("spawn ts2rs compile --emit-ir");
    assert!(
        out.status.success(),
        "stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("IRModule"),
        "expected IRModule Debug on stderr, got:\n{stderr}"
    );
}

#[test]
fn run_switch_ok_prints_seven() {
    assert_run_stdout("switch_ok.ts", "7\n");
}

#[test]
fn compile_switch_ok_writes_rust() {
    let exe = PathBuf::from(env!("CARGO_BIN_EXE_ts2rs"));
    let ts = fixture("switch_ok.ts");
    let dir = tempfile::tempdir().expect("tempdir");
    let rs_path = dir.path().join("out.rs");
    let status = Command::new(&exe)
        .args([
            "compile",
            ts.to_str().unwrap(),
            "-o",
            rs_path.to_str().unwrap(),
        ])
        .status()
        .expect("spawn ts2rs compile");
    assert!(status.success());
    let body = std::fs::read_to_string(&rs_path).expect("read out.rs");
    assert!(
        body.contains("=="),
        "expected lowered switch to use equality in Rust: {body}"
    );
}

// --- §1.3 expression extensions ---

#[test]
fn run_logical_bool_prints_one() {
    assert_run_stdout("logical_bool.ts", "1\n");
}

#[test]
fn run_logical_truthy_ok_prints_twelve() {
    assert_run_stdout("logical_truthy_ok.ts", "12\n");
}

#[test]
fn run_ternary_ok_prints_one() {
    assert_run_stdout("ternary_ok.ts", "1\n");
}

#[test]
fn run_template_ok_prints_three() {
    assert_run_stdout("template_ok.ts", "3\n");
}

#[test]
fn run_comma_ok_prints_three() {
    assert_run_stdout("comma_ok.ts", "3\n");
}

#[test]
fn run_string_utf16_length_prints_two() {
    assert_run_stdout("string_utf16_length.ts", "2\n");
}

#[test]
fn run_object_length_field_prints_value() {
    assert_run_stdout("object_length_field.ts", "42\n");
}

#[test]
fn run_array_length_prints_three() {
    assert_run_stdout("array_length.ts", "3\n");
}

#[test]
fn run_math_builtin_prints_sum() {
    assert_run_stdout("math_builtin.ts", "17\n");
}

#[test]
fn run_stdlib_hir_ok_prints_expected() {
    assert_run_stdout("stdlib_hir_ok.ts", "318.9\n");
}

#[test]
fn compile_stdlib_hir_ok_writes_utf16_and_json_helpers() {
    let exe = PathBuf::from(env!("CARGO_BIN_EXE_ts2rs"));
    let ts = fixture("stdlib_hir_ok.ts");
    let dir = tempfile::tempdir().expect("tempdir");
    let rs_path = dir.path().join("out.rs");
    let status = Command::new(&exe)
        .args([
            "compile",
            ts.to_str().unwrap(),
            "-o",
            rs_path.to_str().unwrap(),
        ])
        .status()
        .expect("spawn ts2rs compile");
    assert!(status.success());
    let body = std::fs::read_to_string(&rs_path).expect("read out.rs");
    assert!(body.contains("__ts2rs_utf16_slice"));
    assert!(body.contains("__ts2rs_utf16_index_of"));
    assert!(body.contains("__ts2rs_json_escape_string"));
}

#[test]
fn run_json_uri_trust_ok_prints_expected() {
    assert_run_stdout("json_uri_trust_ok.ts", "162.5\n");
}

#[test]
fn compile_json_uri_trust_ok_emits_serde_json_and_urlencoding() {
    let exe = PathBuf::from(env!("CARGO_BIN_EXE_ts2rs"));
    let ts = fixture("json_uri_trust_ok.ts");
    let dir = tempfile::tempdir().expect("tempdir");
    let rs_path = dir.path().join("out.rs");
    let status = Command::new(&exe)
        .args([
            "compile",
            ts.to_str().unwrap(),
            "-o",
            rs_path.to_str().unwrap(),
        ])
        .status()
        .expect("spawn ts2rs compile");
    assert!(status.success());
    let body = std::fs::read_to_string(&rs_path).expect("read out.rs");
    assert!(
        body.contains("serde_json::"),
        "expected dynamic JSON.parse to use serde_json: {body}"
    );
    assert!(
        body.contains("urlencoding::"),
        "expected URI builtins to use urlencoding: {body}"
    );
}

#[test]
fn compile_json_parse_hetero_array_literal_fails() {
    assert_compile_fails_stderr("json_parse_hetero_array_fail.ts", "homogeneous");
}

#[test]
fn run_member_length_ok_prints_two() {
    assert_run_stdout("member_length_ok.ts", "2\n");
}

#[test]
fn run_optional_ok_prints_two() {
    assert_run_stdout("optional_ok.ts", "2\n");
}

#[test]
fn run_nullish_ok_prints_one() {
    assert_run_stdout("nullish_ok.ts", "1\n");
}

#[test]
fn run_array_ok_prints_two() {
    assert_run_stdout("array_ok.ts", "2\n");
}

#[test]
fn run_object_ok_prints_three() {
    assert_run_stdout("object_ok.ts", "3\n");
}

// --- §3.4 不可达警告、明确赋值、提前 return ---

fn assert_compile_ok_stderr_contains(name: &str, needle: &str) {
    let exe = PathBuf::from(env!("CARGO_BIN_EXE_ts2rs"));
    let ts = fixture(name);
    let dir = tempfile::tempdir().expect("tempdir");
    let rs_path = dir.path().join("out.rs");
    let out = Command::new(exe)
        .args([
            "compile",
            ts.to_str().unwrap(),
            "-o",
            rs_path.to_str().unwrap(),
        ])
        .output()
        .expect("spawn ts2rs compile");
    assert!(
        out.status.success(),
        "stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains(needle),
        "expected stderr to contain {needle:?}, got:\n{stderr}"
    );
}

#[test]
fn compile_early_return_unreachable_warns() {
    assert_compile_ok_stderr_contains("early_return_unreachable.ts", "unreachable code");
}

#[test]
fn compile_unreachable_after_return_warns() {
    assert_compile_ok_stderr_contains("unreachable_after_return.ts", "unreachable code");
}

#[test]
fn compile_break_unreachable_warns() {
    assert_compile_ok_stderr_contains("break_unreachable.ts", "unreachable code");
}

#[test]
fn run_early_return_unreachable_prints_one() {
    assert_run_stdout("early_return_unreachable.ts", "1\n");
}

#[test]
fn run_unreachable_after_return_prints_one() {
    assert_run_stdout("unreachable_after_return.ts", "1\n");
}

#[test]
fn run_definite_assign_ok_prints_one() {
    assert_run_stdout("definite_assign_ok.ts", "1\n");
}

#[test]
fn run_definite_assign_if_ok_prints_one() {
    assert_run_stdout("definite_assign_if_ok.ts", "1\n");
}

#[test]
fn compile_definite_assign_fail_errors() {
    assert_compile_fails_stderr("definite_assign_fail.ts", "before being assigned");
}

#[test]
fn run_multi_entry_extra_roots_prints_main() {
    let exe = PathBuf::from(env!("CARGO_BIN_EXE_ts2rs"));
    let main_ts = fixture("multi_entry_main.ts");
    let side_ts = fixture("multi_entry_side.ts");
    let out = Command::new(exe)
        .args(["run", main_ts.to_str().unwrap(), side_ts.to_str().unwrap()])
        .output()
        .expect("spawn ts2rs run");
    assert!(
        out.status.success(),
        "stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&out.stdout), "42\n");
}

#[test]
fn run_project_tsconfig_prints_main() {
    let exe = PathBuf::from(env!("CARGO_BIN_EXE_ts2rs"));
    let tsconfig = fixture("multi_entry_tsconfig.json");
    let out = Command::new(exe)
        .args(["run", "--project", tsconfig.to_str().unwrap()])
        .output()
        .expect("spawn ts2rs run");
    assert!(
        out.status.success(),
        "stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&out.stdout), "42\n");
}

#[test]
fn run_method_call_ok_prints_three() {
    assert_run_stdout("method_call_ok.ts", "3\n");
}

#[test]
fn run_chain_call_ok_prints_six() {
    assert_run_stdout("chain_call_ok.ts", "6\n");
}

#[test]
fn run_optional_call_ok_prints_five() {
    assert_run_stdout("optional_call_ok.ts", "5\n");
}

#[test]
fn compile_method_call_ok_desugars_to_global_fn() {
    let exe = PathBuf::from(env!("CARGO_BIN_EXE_ts2rs"));
    let ts = fixture("method_call_ok.ts");
    let dir = tempfile::tempdir().expect("tempdir");
    let rs_path = dir.path().join("out.rs");
    let status = Command::new(exe)
        .args([
            "compile",
            ts.to_str().unwrap(),
            "-o",
            rs_path.to_str().unwrap(),
        ])
        .status()
        .expect("spawn ts2rs compile");
    assert!(status.success());
    let body = std::fs::read_to_string(&rs_path).expect("read out.rs");
    assert!(
        body.contains("sum_xy(") && body.contains("ts_main"),
        "expected desugared call to sum_xy in generated Rust: {body}"
    );
}

#[test]
fn run_class_basic_ok_prints_five() {
    assert_run_stdout("class_basic_ok.ts", "5\n");
}

#[test]
fn run_class_this_method_ok_prints_eight() {
    assert_run_stdout("class_this_method_ok.ts", "8\n");
}

#[test]
fn run_class_extends_ok_prints_seven() {
    assert_run_stdout("class_extends_ok.ts", "7\n");
}

#[test]
fn run_class_super_ctor_ok_prints_seven() {
    assert_run_stdout("class_super_ctor_ok.ts", "7\n");
}

#[test]
fn compile_class_super_invalid_fails() {
    assert_compile_fails_stderr(
        "class_super_invalid_fail.ts",
        "must start with `super(...)`",
    );
}

#[test]
fn compile_class_override_mismatch_fails() {
    assert_compile_fails_stderr(
        "class_override_mismatch_fail.ts",
        "override` method `score` not found in base class",
    );
}

#[test]
fn compile_class_this_scope_fails() {
    assert_compile_fails_stderr("class_this_scope_fail.ts", "only valid inside class method");
}

#[test]
fn run_with_link_ts2rs_rt_prints_main() {
    let exe = PathBuf::from(env!("CARGO_BIN_EXE_ts2rs"));
    let main_ts = fixture("multi_entry_main.ts");
    let side_ts = fixture("multi_entry_side.ts");
    let out = Command::new(exe)
        .args([
            "run",
            "--link-ts2rs-rt",
            main_ts.to_str().unwrap(),
            side_ts.to_str().unwrap(),
        ])
        .output()
        .expect("spawn ts2rs run");
    assert!(
        out.status.success(),
        "stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&out.stdout), "42\n");
}
