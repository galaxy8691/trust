// §9.1: trustc — 编译器入口。Phase 1.7。
//!
//! 子命令: compile / check / eval
//! Flags: --error-format=json --fix --verbose --quiet -o <path>
//!
//! 编译管线: Parse → HIR → TIR → Codegen → rustc
//! 错误恢复策略: constraints §11.5

use std::process;

use trust_codegen::codegen::generate_rust;
use trust_error::diagnostic::{Diagnostic, ErrorCode, FixSuggestion, Severity, SourceSpan};
use trust_error::fix_suggest::suggest_fixes;
use trust_error::json_fmt::format_diagnostics;
use trust_hir::name_res::{self, DiagError};
use trust_hir::typeck;
use trust_parser::module_graph::ModuleGraph;
use trust_parser::parser::{self, Parser};
use trust_tir::borrowck;
use trust_tir::moveck;
use trust_tir::tir;

// ============================================================================
// §3.1.1: CLI 数据结构
// ============================================================================

enum Command {
    Compile { file: String },
    Check { file: String },
    Eval { expr: String },
}

enum OutputFormat {
    Text,
    Json,
}

struct CliOptions {
    command: Command,
    error_format: OutputFormat,
    fix_mode: bool,
    verbose: bool,
    quiet: bool,
    output_path: String,
}

// ============================================================================
// §3.1.2: 手写参数解析（零外部依赖）
// ============================================================================

fn parse_args(args: &[String]) -> Result<CliOptions, String> {
    let mut command = None;
    let mut error_format = OutputFormat::Text;
    let mut fix_mode = false;
    let mut verbose = false;
    let mut quiet = false;
    let mut output_path = String::from("./output");

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "compile" => {
                i += 1;
                if i >= args.len() {
                    return Err("compile requires <file>".into());
                }
                command = Some(Command::Compile { file: args[i].clone() });
            }
            "check" => {
                i += 1;
                if i >= args.len() {
                    return Err("check requires <file>".into());
                }
                command = Some(Command::Check { file: args[i].clone() });
            }
            "eval" => {
                i += 1;
                if i >= args.len() {
                    return Err("eval requires <expr>".into());
                }
                command = Some(Command::Eval { expr: args[i].clone() });
            }
            "--error-format" => {
                i += 1;
                if i < args.len() && args[i] == "json" {
                    error_format = OutputFormat::Json;
                } else {
                    return Err("--error-format requires `json`".into());
                }
            }
            s if s.starts_with("--error-format=") => {
                if s == "--error-format=json" {
                    error_format = OutputFormat::Json;
                } else {
                    return Err(format!("unknown error format: {}", &s[15..]));
                }
            }
            "--fix" => fix_mode = true,
            "--verbose" | "-v" => verbose = true,
            "--quiet" | "-q" => quiet = true,
            "--output" | "-o" => {
                i += 1;
                if i >= args.len() {
                    return Err("-o requires <path>".into());
                }
                output_path = args[i].clone();
            }
            other => return Err(format!("unknown flag: {}", other)),
        }
        i += 1;
    }

    let command = command.ok_or("no command specified (compile/check/eval)")?;
    Ok(CliOptions { command, error_format, fix_mode, verbose, quiet, output_path })
}

// ============================================================================
// §3.2.1: CompileSession — 错误收集
// ============================================================================

struct CompileSession {
    diagnostics: Vec<Diagnostic>,
    error_count: usize,
    format: OutputFormat,
    verbose: bool,
    quiet: bool,
}

impl CompileSession {
    fn new(format: OutputFormat, verbose: bool, quiet: bool) -> Self {
        CompileSession { diagnostics: vec![], error_count: 0, format, verbose, quiet }
    }

    fn add(&mut self, diag: Diagnostic) {
        if diag.level == Severity::Error {
            self.error_count += 1;
        }
        self.diagnostics.push(diag);
    }

    fn has_errors(&self) -> bool {
        self.error_count > 0
    }

    fn emit(&self) {
        if self.diagnostics.is_empty() {
            return;
        }
        match self.format {
            OutputFormat::Json => {
                eprintln!("{}", format_diagnostics(&self.diagnostics, false));
            }
            OutputFormat::Text => {
                for diag in &self.diagnostics {
                    eprintln!("{}: {}", diag.code, diag.message);
                }
            }
        }
    }

    fn log_stage(&self, name: &str) {
        if self.verbose && !self.quiet {
            eprintln!("[trustc] {} ...", name);
        }
    }
}

// ============================================================================
// §3.2.2: 管线主函数
// ============================================================================

fn run(opts: CliOptions) -> Result<(), String> {
    let mut session = CompileSession::new(opts.error_format, opts.verbose, opts.quiet);

    let result = match opts.command {
        Command::Compile { file } => {
            let rust_code = run_pipeline(&file, &mut session, true)?;
            if !session.has_errors() {
                compile_with_rustc(&rust_code, &opts.output_path, &mut session)?;
            }
            Ok(())
        }
        Command::Check { file } => {
            run_pipeline(&file, &mut session, false)?;
            Ok(())
        }
        Command::Eval { expr } => {
            let wrapped = wrap_eval_expr(&expr);
            let tmp_file = format!("{}/__trust_eval.trust", opts.output_path);
            std::fs::create_dir_all(&opts.output_path)
                .map_err(|e| format!("mkdir {}: {}", opts.output_path, e))?;
            std::fs::write(&tmp_file, &wrapped)
                .map_err(|e| format!("write {}: {}", tmp_file, e))?;
            let rust_code = run_pipeline(&tmp_file, &mut session, true)?;
            if !session.has_errors() {
                let eval_output = format!("{}/__trust_eval_out", opts.output_path);
                compile_with_rustc(&rust_code, &eval_output, &mut session)?;
                run_binary(&eval_output)?;
            }
            Ok(())
        }
    };

    // §3.2.6: --fix 模式在 emit 之前调用
    if opts.fix_mode && session.has_errors() {
        run_fix_mode(&session.diagnostics);
    }

    session.emit();
    if session.has_errors() {
        Err(format!("{} error(s) found", session.error_count))
    } else {
        result
    }
}

// ============================================================================
// §3.2.3: 四阶段编译管线
// ============================================================================

fn run_pipeline(file: &str, session: &mut CompileSession, codegen: bool) -> Result<String, String> {
    let src = std::fs::read_to_string(file).map_err(|e| format!("read {}: {}", file, e))?;

    // 1. Parse
    session.log_stage("parse");
    let mut p = Parser::new(&src, file);
    let prog = p.parse_program();
    for diag in &p.diagnostics {
        session.add(convert_parser_diag(diag));
    }
    if session.has_errors() {
        return Err("parse errors".into());
    }

    // 2. HIR
    session.log_stage("hir");
    let mut hir_diags: Vec<DiagError> = vec![];
    let mg = ModuleGraph::new();
    let mut hir = name_res::resolve_names(&prog, &mg, &mut hir_diags);
    let _ = typeck::check_types(&mut hir, &mut hir_diags);
    for d in &hir_diags {
        session.add(convert_hir_diag(d));
    }

    // 3. TIR
    session.log_stage("tir");
    let tir = match tir::lower_hir(&hir) {
        Ok(t) => t,
        Err(diags) => {
            for d in &diags {
                session.add(convert_tir_diag(d));
            }
            return Err("TIR lowering errors".into());
        }
    };
    // 移动/借用检查
    if let Err(move_errors) = moveck::check_moves(&tir) {
        for e in &move_errors {
            session.add(convert_move_error(e));
        }
    }
    if let Err(borrow_errors) = borrowck::check_borrows(&tir) {
        for e in &borrow_errors {
            session.add(convert_borrow_error(e));
        }
    }

    // 4. Codegen（零错误时运行）
    if codegen && !session.has_errors() {
        session.log_stage("codegen");
        match generate_rust(&tir) {
            Ok((rust_code, _source_map)) => Ok(rust_code),
            Err(errors) => {
                for e in &errors {
                    session.add(convert_codegen_error(e));
                }
                Err("codegen errors".into())
            }
        }
    } else {
        Ok(String::new())
    }
}

// ============================================================================
// §3.2.4: eval 表达式包装
// ============================================================================

fn wrap_eval_expr(expr: &str) -> String {
    let escaped = expr
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('{', "\\{")
        .replace('}', "\\}")
        .replace('\n', " ");
    format!("function main(): void {{\n    console.log({escaped});\n}}\n", escaped = escaped)
}

// ============================================================================
// §3.2.5: rustc 编译 + 二进制执行
// ============================================================================

fn compile_with_rustc(
    rust_code: &str,
    output: &str,
    _session: &mut CompileSession,
) -> Result<(), String> {
    let temp_rs = format!("{}.rs", output);
    std::fs::write(&temp_rs, rust_code).map_err(|e| format!("write {}: {}", temp_rs, e))?;

    let ferro_rt_dir = std::env::var("FERRO_RT_LIB").unwrap_or_else(|_| "target/debug".to_string());

    let status = process::Command::new("rustc")
        .args([
            "--edition",
            "2021",
            "-L",
            &ferro_rt_dir,
            "--extern",
            &format!("ferro_rt={}/libferro_rt.rlib", ferro_rt_dir),
            "-o",
            output,
            &temp_rs,
        ])
        .status()
        .map_err(|e| format!("rustc: {}", e))?;

    if !status.success() {
        return Err("rustc compilation failed".into());
    }
    Ok(())
}

fn run_binary(path: &str) -> Result<(), String> {
    let output =
        process::Command::new(path).output().map_err(|e| format!("run {}: {}", path, e))?;
    print!("{}", String::from_utf8_lossy(&output.stdout));
    Ok(())
}

// ============================================================================
// §3.2.6: --fix 交互模式
// ============================================================================

fn run_fix_mode(diagnostics: &[Diagnostic]) {
    for diag in diagnostics {
        let fixes: Vec<FixSuggestion> = suggest_fixes(diag);
        for fix in &fixes {
            eprintln!("{}: {}", diag.code, diag.message);
            eprintln!("help: {}", fix.message);
            eprintln!("  → {}", fix.replacement);
            eprintln!("应用此修复？(y/N): ");

            let mut input = String::new();
            if std::io::stdin().read_line(&mut input).is_ok() && input.trim().to_lowercase() == "y"
            {
                eprintln!("[trustc] 修复建议已记录（Phase 1 不自动修改源文件）");
                eprintln!("  建议内容：{}", fix.replacement);
            }
        }
    }
}

// ============================================================================
// §3.2.7: 跨 crate 错误类型转换
// ============================================================================

fn convert_parser_diag(diag: &parser::Diagnostic) -> Diagnostic {
    let level = match diag.level {
        parser::DiagLevel::Error => Severity::Error,
        parser::DiagLevel::Warning => Severity::Warning,
    };
    let span = SourceSpan {
        file: diag.span.file.clone(),
        line_start: diag.span.line_start,
        col_start: diag.span.col_start,
        line_end: diag.span.line_end,
        col_end: diag.span.col_end,
        label: None,
    };
    Diagnostic::error(ErrorCode::E0001, format!("parse: {}", diag.message), span)
}

fn convert_hir_diag(diag: &DiagError) -> Diagnostic {
    let span = SourceSpan {
        file: diag.span.file.clone(),
        line_start: diag.span.line_start,
        col_start: diag.span.col_start,
        line_end: diag.span.line_end,
        col_end: diag.span.col_end,
        label: None,
    };
    Diagnostic::error(ErrorCode::E0425, format!("hir: {}", diag.message), span)
}

// TIR/Move/Borrow 错误类型为 private——用内联转换
fn convert_tir_diag<E: std::fmt::Debug>(diag: &E) -> Diagnostic {
    Diagnostic::error(
        ErrorCode::E9999,
        format!("tir: {:?}", diag),
        SourceSpan {
            file: "unknown".into(),
            line_start: 1,
            col_start: 1,
            line_end: 1,
            col_end: 1,
            label: None,
        },
    )
}

fn convert_move_error<E: std::fmt::Debug>(e: &E) -> Diagnostic {
    Diagnostic::error(
        ErrorCode::E0382,
        format!("move: {:?}", e),
        SourceSpan {
            file: "unknown".into(),
            line_start: 1,
            col_start: 1,
            line_end: 1,
            col_end: 1,
            label: None,
        },
    )
}

fn convert_borrow_error<E: std::fmt::Debug>(e: &E) -> Diagnostic {
    Diagnostic::error(
        ErrorCode::E0501,
        format!("borrow: {:?}", e),
        SourceSpan {
            file: "unknown".into(),
            line_start: 1,
            col_start: 1,
            line_end: 1,
            col_end: 1,
            label: None,
        },
    )
}

fn convert_codegen_error(e: &trust_codegen::codegen::CodegenError) -> Diagnostic {
    let span = SourceSpan {
        file: e.span.file.clone(),
        line_start: e.span.line_start,
        col_start: e.span.col_start,
        line_end: e.span.line_end,
        col_end: e.span.col_end,
        label: None,
    };
    Diagnostic::error(ErrorCode::E9999, format!("codegen: {}", e.message), span)
}

// ============================================================================
// §3.1.3: main() 入口
// ============================================================================

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let opts = match parse_args(&args) {
        Ok(o) => o,
        Err(e) => {
            eprintln!("trustc: {}", e);
            eprintln!("Usage: trustc <compile|check|eval> [flags]");
            process::exit(1);
        }
    };

    if let Err(e) = run(opts) {
        eprintln!("trustc: fatal: {}", e);
        process::exit(1);
    }
}
