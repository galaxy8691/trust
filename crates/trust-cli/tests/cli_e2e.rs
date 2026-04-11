//! 集成测试：TS → Rust → cargo → 运行可执行文件。

use std::path::PathBuf;
use std::process::Command;

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

fn fixture_subpath(relative: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(relative)
}

fn regression_case(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/regression")
        .join(name)
}

fn assert_run_stdout(name: &str, expected: &str) {
    let exe = PathBuf::from(env!("CARGO_BIN_EXE_trust"));
    let ts = fixture(name);
    let out = Command::new(exe)
        .args(["run", ts.to_str().unwrap()])
        .output()
        .expect("spawn trust run");
    assert!(
        out.status.success(),
        "stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&out.stdout), expected);
}

#[test]
fn run_prints_main_result() {
    let exe = PathBuf::from(env!("CARGO_BIN_EXE_trust"));
    let ts = fixture("sample.ts");
    let out = Command::new(exe)
        .args(["run", ts.to_str().unwrap()])
        .output()
        .expect("spawn trust run");

    assert!(
        out.status.success(),
        "stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&out.stdout), "3\n");
}

#[test]
fn compile_exec_writes_binary_and_runs() {
    let exe = PathBuf::from(env!("CARGO_BIN_EXE_trust"));
    let ts = fixture("sample.ts");
    let dir = tempfile::tempdir().expect("tempdir");
    let bin = dir
        .path()
        .join(if cfg!(windows) { "out.exe" } else { "out" });
    let status = Command::new(&exe)
        .args([
            "compile",
            "--exec",
            ts.to_str().unwrap(),
            "-o",
            bin.to_str().unwrap(),
        ])
        .status()
        .expect("spawn trust compile --exec");
    assert!(status.success(), "compile --exec should succeed");
    let out = Command::new(&bin).output().expect("spawn compiled binary");
    assert!(
        out.status.success(),
        "stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&out.stdout), "3\n");
}

#[test]
fn compile_exec_without_o_defaults_to_entry_stem_in_cwd() {
    let exe = PathBuf::from(env!("CARGO_BIN_EXE_trust"));
    let ts = fixture("sample.ts");
    let dir = tempfile::tempdir().expect("tempdir");
    let bin = dir.path().join(if cfg!(windows) {
        "sample.exe"
    } else {
        "sample"
    });
    let status = Command::new(&exe)
        .current_dir(dir.path())
        .args(["compile", "--exec", ts.to_str().unwrap()])
        .status()
        .expect("spawn trust compile --exec without -o");
    assert!(status.success(), "compile --exec without -o should succeed");
    let out = Command::new(&bin)
        .output()
        .expect("spawn default-named binary");
    assert!(
        out.status.success(),
        "stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&out.stdout), "3\n");
}

#[test]
fn compile_writes_rust() {
    let exe = PathBuf::from(env!("CARGO_BIN_EXE_trust"));
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
        .expect("spawn trust compile");
    assert!(status.success());
    let body = std::fs::read_to_string(&rs_path).expect("read out.rs");
    assert!(body.contains("fn ts_main"));
    assert!(body.contains("fn add"));
}

#[test]
fn compile_ts_source_comments_writes_ts_text() {
    let exe = PathBuf::from(env!("CARGO_BIN_EXE_trust"));
    let ts = fixture("ts_source_comment_ok.ts");
    let dir = tempfile::tempdir().expect("tempdir");
    let rs_path = dir.path().join("out.rs");
    let status = Command::new(exe)
        .args([
            "compile",
            ts.to_str().unwrap(),
            "-o",
            rs_path.to_str().unwrap(),
            "--ts-source-comments",
        ])
        .status()
        .expect("spawn trust compile");
    assert!(status.success());
    let body = std::fs::read_to_string(&rs_path).expect("read out.rs");
    assert!(
        body.contains("// __TRUST_SOURCE_COMMENT_TOP__"),
        "expected TS top comment in Rust: {body}"
    );
    assert!(
        body.contains("// __TRUST_SOURCE_COMMENT_BODY__"),
        "expected TS body comment in Rust: {body}"
    );
}

#[test]
fn compile_span_comments_writes_ts_anchors() {
    let exe = PathBuf::from(env!("CARGO_BIN_EXE_trust"));
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
        .expect("spawn trust compile");
    assert!(status.success());
    let body = std::fs::read_to_string(&rs_path).expect("read out.rs");
    assert!(
        body.contains("// ts:"),
        "expected // ts: span comments in generated Rust: {body}"
    );
}

#[test]
fn compile_console_stderr_writes_eprintln() {
    let exe = PathBuf::from(env!("CARGO_BIN_EXE_trust"));
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
        .expect("spawn trust compile");
    assert!(status.success());
    let body = std::fs::read_to_string(&rs_path).expect("read out.rs");
    assert!(
        body.contains("trust_stdlib::console::log_joined(true"),
        "expected console.error/debug to lower to trust_stdlib::console with stderr=true: {body}"
    );
}

#[test]
fn run_let_if_prints_ten() {
    let exe = PathBuf::from(env!("CARGO_BIN_EXE_trust"));
    let ts = fixture("let_if.ts");
    let out = Command::new(exe)
        .args(["run", ts.to_str().unwrap()])
        .output()
        .expect("spawn trust run");
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&out.stdout), "10\n");
}

#[test]
fn compile_void_main_has_no_println_value() {
    let exe = PathBuf::from(env!("CARGO_BIN_EXE_trust"));
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
        .expect("spawn trust compile");
    assert!(status.success());
    let body = std::fs::read_to_string(&rs_path).expect("read out.rs");
    assert!(body.contains("fn ts_main() -> ()"));
    assert!(body.contains("trust_stdlib::console::log_joined(false"));
    assert!(!body.contains("println!(\"{}\", ts_main())"));
}

#[test]
fn compile_async_mvp_writes_tokio_and_await() {
    let exe = PathBuf::from(env!("CARGO_BIN_EXE_trust"));
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
        .expect("spawn trust compile");
    assert!(status.success());
    let body = std::fs::read_to_string(&rs_path).expect("read out.rs");
    assert!(body.contains("#[tokio::main]"), "{body}");
    assert!(body.contains("async fn ts_main"), "{body}");
    assert!(body.contains(".await"), "{body}");
    assert!(body.contains("trust_stdlib::http::fetch_text"), "{body}");
}

#[test]
fn compile_async_control_flow_if_while_await_ok() {
    let exe = PathBuf::from(env!("CARGO_BIN_EXE_trust"));
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
        .expect("spawn trust compile");
    assert!(status.success());
    let body = std::fs::read_to_string(&rs_path).expect("read out.rs");
    assert!(body.contains("#[tokio::main]"), "{body}");
    assert!(body.contains("async fn ts_main"), "{body}");
    assert!(body.contains(".await"), "{body}");
    assert!(body.contains("if ("), "{body}");
    assert!(body.contains("while "), "{body}");
    assert!(body.contains("trust_stdlib::http::fetch_text"), "{body}");
}

#[test]
fn compile_async_all_fetch_alias_ok() {
    let exe = PathBuf::from(env!("CARGO_BIN_EXE_trust"));
    let ts = fixture("async_all_fetch_ok.ts");
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
        .expect("spawn trust compile");
    assert!(status.success());
    let body = std::fs::read_to_string(&rs_path).expect("read out.rs");
    assert!(body.contains("vec!["), "{body}");
    assert!(body.contains("trust_stdlib::http::fetch("), "{body}");
    assert!(body.contains(".await"), "{body}");
}

#[test]
fn compile_promise_then_fails() {
    assert_compile_fails_stderr("promise_then_fail.ts", ".then` callbacks are not supported");
}

#[test]
fn compile_fetch_response_ok() {
    let exe = PathBuf::from(env!("CARGO_BIN_EXE_trust"));
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
        .expect("spawn trust compile");
    assert!(status.success());
    let body = std::fs::read_to_string(&rs_path).expect("read out.rs");
    assert!(body.contains("trust_stdlib::http::fetch("), "{body}");
    assert!(body.contains(".status()"), "{body}");
    assert!(body.contains(".text()"), "{body}");
}

#[test]
fn compile_fetch_stream_ok() {
    let exe = PathBuf::from(env!("CARGO_BIN_EXE_trust"));
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
        .expect("spawn trust compile");
    assert!(status.success());
    let body = std::fs::read_to_string(&rs_path).expect("read out.rs");
    assert!(body.contains("bytes_stream()"), "{body}");
    assert!(body.contains("__TrustStreamReadResult"), "{body}");
    assert!(body.contains("futures_util"), "{body}");
}

#[test]
fn compile_fetch_post_init_ok() {
    let exe = PathBuf::from(env!("CARGO_BIN_EXE_trust"));
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
        .expect("spawn trust compile");
    assert!(status.success());
    let body = std::fs::read_to_string(&rs_path).expect("read out.rs");
    assert!(body.contains("trust_stdlib::http::FetchInit"), "{body}");
    assert!(body.contains("\"POST\""), "{body}");
}

#[test]
fn compile_file_read_text_sync_ok() {
    let exe = PathBuf::from(env!("CARGO_BIN_EXE_trust"));
    let ts = fixture("file_read_text_sync_compile_ok.ts");
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
        .expect("spawn trust compile");
    assert!(status.success());
    let body = std::fs::read_to_string(&rs_path).expect("read out.rs");
    assert!(body.contains("trust_stdlib::io::read_file_text"), "{body}");
}

#[test]
fn compile_file_read_text_async_ok() {
    let exe = PathBuf::from(env!("CARGO_BIN_EXE_trust"));
    let ts = fixture("file_read_text_async_compile_ok.ts");
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
        .expect("spawn trust compile");
    assert!(status.success());
    let body = std::fs::read_to_string(&rs_path).expect("read out.rs");
    assert!(
        body.contains("trust_stdlib::io::read_file_text_async"),
        "{body}"
    );
    assert!(body.contains(".await"), "{body}");
}

#[test]
fn compile_file_read_text_arg_type_fails() {
    assert_compile_fails_stderr(
        "file_read_text_arg_type_fail.ts",
        "`readFileText` argument must be `string`",
    );
}

#[test]
fn compile_file_read_text_async_without_await_fails() {
    assert_compile_fails_stderr("file_read_text_async_no_await_fail.ts", "readFileTextAsync");
}

#[test]
fn run_file_read_text_sync_ok() {
    let exe = PathBuf::from(env!("CARGO_BIN_EXE_trust"));
    let dir = tempfile::tempdir().expect("tempdir");
    let file_path = dir.path().join("in.txt");
    std::fs::write(&file_path, "hello").expect("write input");
    let ts_path = dir.path().join("main.ts");
    let ts_src = format!(
        "function main(): number {{ let s: string = readFileText({:?}); console.log(s); return s.length; }}\n",
        file_path.to_string_lossy()
    );
    std::fs::write(&ts_path, ts_src).expect("write ts");
    let out = Command::new(exe)
        .args(["run", ts_path.to_str().unwrap()])
        .output()
        .expect("spawn trust run");
    assert!(
        out.status.success(),
        "stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&out.stdout), "hello\n5\n");
}

#[test]
fn run_file_read_text_async_ok() {
    let exe = PathBuf::from(env!("CARGO_BIN_EXE_trust"));
    let dir = tempfile::tempdir().expect("tempdir");
    let file_path = dir.path().join("in.txt");
    std::fs::write(&file_path, "hello").expect("write input");
    let ts_path = dir.path().join("main.ts");
    let ts_src = format!(
        "export async function main(): number {{ let s: string = await readFileTextAsync({:?}); return s.length; }}\n",
        file_path.to_string_lossy()
    );
    std::fs::write(&ts_path, ts_src).expect("write ts");
    let out = Command::new(exe)
        .args(["run", ts_path.to_str().unwrap()])
        .output()
        .expect("spawn trust run");
    assert!(
        out.status.success(),
        "stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&out.stdout), "5\n");
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
    let exe = PathBuf::from(env!("CARGO_BIN_EXE_trust"));
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
        .expect("spawn trust compile");
    assert!(status.success());
    let body = std::fs::read_to_string(&rs_path).expect("read out.rs");
    assert!(body.contains("trust_stdlib::console::log_joined(false"));
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
        "intersection_hetero_fail.ts",
        "return type mismatch",
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
        "cannot infer type arguments for generic function",
    );
}

#[test]
fn compile_generic_function_infer_conflict_fails() {
    assert_compile_fails_stderr(
        "generic_function_infer_conflict_fail.ts",
        "conflicting inferred type arguments",
    );
}

#[test]
fn compile_generic_function_multi_infer_fail_reports_multiple_errors() {
    let exe = PathBuf::from(env!("CARGO_BIN_EXE_trust"));
    let ts = fixture("generic_function_multi_infer_fail.ts");
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
        .expect("spawn trust compile");
    assert!(!out.status.success());
    let stderr = String::from_utf8_lossy(&out.stderr);
    let n = stderr
        .matches("cannot infer type arguments for generic function")
        .count();
    assert!(
        n >= 2,
        "expected at least two monomorphization errors, got {n} in:\n{stderr}"
    );
}

#[test]
fn run_generic_method_call_infer_ok_prints_three() {
    assert_run_stdout("generic_method_call_infer_ok.ts", "3\n");
}

#[test]
fn compile_generic_method_call_infer_ok_writes_rust() {
    let exe = PathBuf::from(env!("CARGO_BIN_EXE_trust"));
    let ts = fixture("generic_method_call_infer_ok.ts");
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
        .expect("spawn trust compile");
    assert!(status.success());
    let body = std::fs::read_to_string(&rs_path).expect("read out.rs");
    assert!(
        body.contains("fn ts_main"),
        "expected generated Rust with ts_main: {body}"
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
    let exe = PathBuf::from(env!("CARGO_BIN_EXE_trust"));
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
        .expect("spawn trust compile");
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
    let exe = PathBuf::from(env!("CARGO_BIN_EXE_trust"));
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
        .expect("spawn trust compile");
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
    let exe = PathBuf::from(env!("CARGO_BIN_EXE_trust"));
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
        .expect("spawn trust compile");
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
    let exe = PathBuf::from(env!("CARGO_BIN_EXE_trust"));
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
        .expect("spawn trust compile");
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
    let exe = PathBuf::from(env!("CARGO_BIN_EXE_trust"));
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
        .expect("spawn trust compile");
    assert!(
        !out.status.success(),
        "expected compile to fail:\nstdout={}\nstderr={}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("relative") || stderr.contains("import"),
        "expected stderr to mention non-relative import, got:\n{stderr}"
    );
}

#[test]
fn run_import_fails() {
    let exe = PathBuf::from(env!("CARGO_BIN_EXE_trust"));
    let ts = fixture("import_fail.ts");
    let out = Command::new(exe)
        .args(["run", ts.to_str().unwrap()])
        .output()
        .expect("spawn trust run");
    assert!(!out.status.success());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("relative") || stderr.contains("import"),
        "stderr:\n{stderr}"
    );
}

fn assert_compile_fails_stderr(fixture_name: &str, needle: &str) {
    let exe = PathBuf::from(env!("CARGO_BIN_EXE_trust"));
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
        .expect("spawn trust compile");
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
fn compile_import_type_fails() {
    // import type 现在被支持，但当导入不存在的类型时会报错
    assert_compile_fails_stderr("import_type_fail_main.ts", "no exported type");
}

#[test]
fn run_export_default_function_main_prints_42() {
    assert_run_stdout("export_default_function_main_ok.ts", "42\n");
}

#[test]
fn run_export_default_main_ref_prints_7() {
    assert_run_stdout("export_default_main_ref_ok.ts", "7\n");
}

#[test]
fn compile_export_default_async_main_writes_tokio() {
    let exe = PathBuf::from(env!("CARGO_BIN_EXE_trust"));
    let ts = fixture("export_default_async_main_ok.ts");
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
        .expect("spawn trust compile");
    assert!(status.success());
    let body = std::fs::read_to_string(&rs_path).expect("read out.rs");
    assert!(body.contains("#[tokio::main]"), "{body}");
    assert!(body.contains("async fn ts_main"), "{body}");
}

#[test]
fn run_nested_object_ok_prints_three() {
    assert_run_stdout("nested_object_ok.ts", "3\n");
}

#[test]
fn compile_import_default_wrong_binding_fails() {
    assert_compile_fails_stderr(
        "import_default_wrong_binding_fail.ts",
        "binding name `main`",
    );
}

#[test]
fn compile_export_from_fails() {
    assert_compile_fails_stderr("export_from_fail.ts", "relative");
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
fn run_for_of_number_array_ok_prints_six() {
    assert_run_stdout("for_of_number_array_ok.ts", "6\n");
}

#[test]
fn run_for_of_string_array_ok_prints_three() {
    assert_run_stdout("for_of_string_array_ok.ts", "3\n");
}

#[test]
fn run_for_of_break_ok_prints_three() {
    assert_run_stdout("for_of_break_ok.ts", "3\n");
}

#[test]
fn compile_for_of_non_array_fails() {
    assert_compile_fails_stderr("for_of_non_array_fail.ts", "for..of target must be an array");
}

#[test]
fn compile_for_of_type_mismatch_fails() {
    assert_compile_fails_stderr(
        "for_of_type_mismatch_fail.ts",
        "binary arithmetic expects two `number`s",
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
    let exe = PathBuf::from(env!("CARGO_BIN_EXE_trust"));
    let ts = fixture("sample.ts");
    let out = Command::new(exe)
        .args(["check", ts.to_str().unwrap()])
        .output()
        .expect("spawn trust check");
    assert!(
        out.status.success(),
        "stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn check_nullish_fn_union_ok() {
    let exe = PathBuf::from(env!("CARGO_BIN_EXE_trust"));
    let ts = fixture("nullish_fn_ok.ts");
    let out = Command::new(exe)
        .args(["check", ts.to_str().unwrap()])
        .output()
        .expect("spawn trust check nullish_fn_ok");
    assert!(
        out.status.success(),
        "stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn check_switch_fail_stderr() {
    let exe = PathBuf::from(env!("CARGO_BIN_EXE_trust"));
    let ts = fixture("switch_fail.ts");
    let out = Command::new(exe)
        .args(["check", ts.to_str().unwrap()])
        .output()
        .expect("spawn trust check");
    assert!(!out.status.success());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("fall-through"),
        "expected fall-through diagnostic, got:\n{stderr}"
    );
}

#[test]
fn regression_switch_fallthrough_check_fails() {
    let exe = PathBuf::from(env!("CARGO_BIN_EXE_trust"));
    let ts = regression_case("switch_fallthrough_regression.ts");
    let out = Command::new(exe)
        .args(["check", ts.to_str().unwrap()])
        .output()
        .expect("spawn trust check regression");
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
    assert_compile_fails_stderr("object_fail.ts", "object literal field values must be");
}

#[test]
fn compile_emit_ir_stderr_contains_ir_module() {
    let exe = PathBuf::from(env!("CARGO_BIN_EXE_trust"));
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
        .expect("spawn trust compile --emit-ir");
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
    let exe = PathBuf::from(env!("CARGO_BIN_EXE_trust"));
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
        .expect("spawn trust compile");
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
fn compile_stdlib_hir_ok_uses_trust_stdlib_calls() {
    let exe = PathBuf::from(env!("CARGO_BIN_EXE_trust"));
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
        .expect("spawn trust compile");
    assert!(status.success());
    let body = std::fs::read_to_string(&rs_path).expect("read out.rs");
    assert!(body.contains("trust_stdlib::string::utf16_slice"));
    assert!(body.contains("trust_stdlib::string::utf16_index_of"));
    assert!(body.contains("trust_stdlib::"));
}

#[test]
fn compile_stdlib_hir_ok_legacy_mode_still_uses_trust_stdlib() {
    let exe = PathBuf::from(env!("CARGO_BIN_EXE_trust"));
    let ts = fixture("stdlib_hir_ok.ts");
    let dir = tempfile::tempdir().expect("tempdir");
    let rs_path = dir.path().join("out.rs");
    let status = Command::new(&exe)
        .args([
            "compile",
            ts.to_str().unwrap(),
            "-o",
            rs_path.to_str().unwrap(),
            "--stdlib-mode",
            "legacy",
        ])
        .status()
        .expect("spawn trust compile");
    assert!(status.success());
    let body = std::fs::read_to_string(&rs_path).expect("read out.rs");
    assert!(body.contains("trust_stdlib::string::utf16_slice"));
    assert!(body.contains("trust_stdlib::string::utf16_index_of"));
    assert!(body.contains("trust_stdlib::"));
}

#[test]
fn run_json_uri_trust_ok_prints_expected() {
    assert_run_stdout("json_uri_trust_ok.ts", "162.5\n");
}

#[test]
fn run_std_namespace_import_ok() {
    let exe = PathBuf::from(env!("CARGO_BIN_EXE_trust"));
    let dir = tempfile::tempdir().expect("tempdir");
    let ts = dir.path().join("main.ts");
    let src = r#"import std from "std";
export function main(): number {
  let n: number = std.json.parse("12" + "3");
  let s: string = std.string.slice("abcd", 1, 3);
  let u: string = std.uri.decodeComponent(std.uri.encodeURIComponent("a b"));
  std.console.log("std", s, u);
  let lenFn: number = std.string.length("ab");
  let m: number = std.math.abs(-2.0);
  let p: number = std.number.parseInt("10", 10);
  return n + s.length + u.length + lenFn + m + p;
}
"#;
    std::fs::write(&ts, src).expect("write ts");
    let out = Command::new(exe)
        .args(["run", ts.to_str().unwrap()])
        .output()
        .expect("spawn trust run");
    assert!(
        out.status.success(),
        "stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    // 128 + 2 (length fn) + 2 (abs) + 10 (parseInt) = 142
    assert_eq!(String::from_utf8_lossy(&out.stdout), "std bc a b\n142\n");
}

#[test]
fn compile_std_http_namespace_ok() {
    let exe = PathBuf::from(env!("CARGO_BIN_EXE_trust"));
    let dir = tempfile::tempdir().expect("tempdir");
    let ts = dir.path().join("main.ts");
    let rs_path = dir.path().join("out.rs");
    let src = r#"import std from "std";
export async function main(): number {
  const r: HttpResponse = await std.http.fetch("http://127.0.0.1:9/nope");
  let _t: string = await std.http.text(r);
  return 0;
}
"#;
    std::fs::write(&ts, src).expect("write ts");
    let status = Command::new(&exe)
        .args([
            "compile",
            ts.to_str().unwrap(),
            "-o",
            rs_path.to_str().unwrap(),
        ])
        .status()
        .expect("spawn trust compile");
    assert!(status.success(), "compile std.http should succeed");
    let body = std::fs::read_to_string(&rs_path).expect("read out.rs");
    assert!(
        body.contains("trust_stdlib::http::fetch("),
        "expected fetch helper in output: {body}"
    );
    assert!(
        body.contains(".text().await"),
        "expected response text await in output: {body}"
    );
}

#[test]
fn run_trust_regex_ok_prints_one() {
    let exe = PathBuf::from(env!("CARGO_BIN_EXE_trust"));
    let ts = fixture_subpath("trust_regex/main.ts");
    let out = Command::new(&exe)
        .args(["run", ts.to_str().unwrap()])
        .output()
        .expect("spawn trust run");
    assert!(
        out.status.success(),
        "stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&out.stdout), "1\n");
}

#[test]
fn compile_trust_regex_ok_emits_regex_crate() {
    let exe = PathBuf::from(env!("CARGO_BIN_EXE_trust"));
    let ts = fixture_subpath("trust_regex/main.ts");
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
        .expect("spawn trust compile");
    assert!(status.success());
    let body = std::fs::read_to_string(&rs_path).expect("read out.rs");
    assert!(
        body.contains("regex::"),
        "expected qualified regex path in output: {body}"
    );
    assert!(
        body.contains("is_match"),
        "expected inherent is_match call: {body}"
    );
}

#[test]
fn compile_json_uri_trust_ok_uses_trust_stdlib_json_and_uri() {
    let exe = PathBuf::from(env!("CARGO_BIN_EXE_trust"));
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
        .expect("spawn trust compile");
    assert!(status.success());
    let body = std::fs::read_to_string(&rs_path).expect("read out.rs");
    assert!(
        body.contains("trust_stdlib::json::"),
        "expected dynamic JSON.parse to use trust_stdlib::json: {body}"
    );
    assert!(
        body.contains("trust_stdlib::uri::"),
        "expected URI builtins to use trust_stdlib::uri: {body}"
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
    let exe = PathBuf::from(env!("CARGO_BIN_EXE_trust"));
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
        .expect("spawn trust compile");
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
    let exe = PathBuf::from(env!("CARGO_BIN_EXE_trust"));
    let main_ts = fixture("multi_entry_main.ts");
    let side_ts = fixture("multi_entry_side.ts");
    let out = Command::new(exe)
        .args(["run", main_ts.to_str().unwrap(), side_ts.to_str().unwrap()])
        .output()
        .expect("spawn trust run");
    assert!(
        out.status.success(),
        "stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&out.stdout), "42\n");
}

#[test]
fn run_project_tsconfig_prints_main() {
    let exe = PathBuf::from(env!("CARGO_BIN_EXE_trust"));
    let tsconfig = fixture("multi_entry_tsconfig.json");
    let out = Command::new(exe)
        .args(["run", "--project", tsconfig.to_str().unwrap()])
        .output()
        .expect("spawn trust run");
    assert!(
        out.status.success(),
        "stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&out.stdout), "42\n");
}

#[test]
fn run_reexport_export_star_ok() {
    let dir = tempfile::tempdir().expect("tempdir");
    let lib = dir.path().join("lib.ts");
    let barrel = dir.path().join("barrel.ts");
    let app = dir.path().join("app.ts");
    std::fs::write(
        &lib,
        "export function add(a: number, b: number): number { return a + b; }\n",
    )
    .unwrap();
    std::fs::write(&barrel, "export * from \"./lib.ts\";\n").unwrap();
    std::fs::write(
        &app,
        "import { add } from \"./barrel.ts\";\nexport function main(): number { return add(1, 2); }\n",
    )
    .unwrap();
    let exe = PathBuf::from(env!("CARGO_BIN_EXE_trust"));
    let out = Command::new(exe)
        .args(["run", app.to_str().unwrap()])
        .output()
        .expect("spawn trust run");
    assert!(
        out.status.success(),
        "stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&out.stdout), "3\n");
}

#[test]
fn run_project_tsconfig_extends_include_ok() {
    let dir = tempfile::tempdir().expect("tempdir");
    let base = dir.path().join("base.json");
    let cfg = dir.path().join("tsconfig.json");
    let main = dir.path().join("main.ts");
    let side = dir.path().join("side.ts");
    std::fs::write(&main, "export function main(): number { return 42; }\n").unwrap();
    std::fs::write(&side, "export function unused(): number { return 1; }\n").unwrap();
    std::fs::write(&base, r#"{"include": ["*.ts"]}"#).unwrap();
    std::fs::write(
        &cfg,
        r#"{"extends": "./base.json", "files": ["main.ts", "side.ts"]}"#,
    )
    .unwrap();
    let exe = PathBuf::from(env!("CARGO_BIN_EXE_trust"));
    let out = Command::new(exe)
        .args(["run", "--project", cfg.to_str().unwrap()])
        .output()
        .expect("spawn trust run");
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
fn run_chain_deep_ok_prints_forty_two() {
    assert_run_stdout("chain_deep_ok.ts", "42\n");
}

#[test]
fn run_interface_extends_ok_prints_six() {
    assert_run_stdout("interface_extends_ok.ts", "6\n");
}

#[test]
fn run_import_type_ok_prints_ten() {
    // B2a: import type 跨文件导入类型
    assert_run_stdout("import_type_main.ts", "10\n");
}

#[test]
fn compile_interface_extends_circular_fails() {
    assert_compile_fails_stderr("interface_extends_circular_fail.ts", "circular interface inheritance");
}

#[test]
fn run_optional_call_ok_prints_five() {
    assert_run_stdout("optional_call_ok.ts", "5\n");
}

#[test]
fn compile_method_call_ok_desugars_to_global_fn() {
    let exe = PathBuf::from(env!("CARGO_BIN_EXE_trust"));
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
        .expect("spawn trust compile");
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
fn run_with_link_trust_rt_prints_main() {
    let exe = PathBuf::from(env!("CARGO_BIN_EXE_trust"));
    let main_ts = fixture("multi_entry_main.ts");
    let side_ts = fixture("multi_entry_side.ts");
    let out = Command::new(exe)
        .args([
            "run",
            "--link-trust-rt",
            main_ts.to_str().unwrap(),
            side_ts.to_str().unwrap(),
        ])
        .output()
        .expect("spawn trust run");
    assert!(
        out.status.success(),
        "stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&out.stdout), "42\n");
}

fn parse_fragment_rebuilds(stderr: &str) -> u32 {
    for line in stderr.lines() {
        if let Some(rest) = line.strip_prefix("trust_fragment_rebuilds=") {
            return rest.trim().parse().expect("parse fragment rebuild count");
        }
    }
    panic!("missing trust_fragment_rebuilds in stderr:\n{stderr}");
}

#[test]
fn compile_incremental_rebuilds_only_changed_module() {
    let dir = tempfile::tempdir().expect("tempdir");
    let dep = dir.path().join("lib.ts");
    let app = dir.path().join("app.ts");
    std::fs::write(
        &dep,
        "export function add(a: number, b: number): number { return a + b; }\n",
    )
    .unwrap();
    std::fs::write(
        &app,
        "import { add } from \"./lib.ts\";\nexport function main(): number { return add(1, 2); }\n",
    )
    .unwrap();
    let cache = dir.path().join("incr-cache");
    let out_rs = dir.path().join("out.rs");
    let exe = PathBuf::from(env!("CARGO_BIN_EXE_trust"));
    let o1 = Command::new(&exe)
        .env("TRUST_TEST_FRAGMENT_STATS", "1")
        .args([
            "compile",
            app.to_str().unwrap(),
            "-o",
            out_rs.to_str().unwrap(),
            "--incremental",
            cache.to_str().unwrap(),
        ])
        .output()
        .expect("spawn trust compile incremental");
    assert!(
        o1.status.success(),
        "stderr:\n{}",
        String::from_utf8_lossy(&o1.stderr)
    );
    let s1 = String::from_utf8_lossy(&o1.stderr);
    assert_eq!(
        parse_fragment_rebuilds(&s1),
        2,
        "first compile should build both modules: {s1}"
    );

    std::fs::write(
        &app,
        "import { add } from \"./lib.ts\";\nexport function main(): number { return add(2, 3); }\n",
    )
    .unwrap();
    let o2 = Command::new(&exe)
        .env("TRUST_TEST_FRAGMENT_STATS", "1")
        .args([
            "compile",
            app.to_str().unwrap(),
            "-o",
            out_rs.to_str().unwrap(),
            "--incremental",
            cache.to_str().unwrap(),
        ])
        .output()
        .expect("spawn trust compile incremental 2");
    assert!(
        o2.status.success(),
        "stderr:\n{}",
        String::from_utf8_lossy(&o2.stderr)
    );
    let s2 = String::from_utf8_lossy(&o2.stderr);
    assert_eq!(
        parse_fragment_rebuilds(&s2),
        1,
        "second compile should rebuild only app (and not lib): {s2}"
    );
}
