use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};

use clap::{ArgAction, Args, ColorChoice, Parser, Subcommand};
use serde::Deserialize;
use ts2rs_driver::{build_rust_to_executable_with_options, RustBuildOptions};
use ts2rs_hir::{build_checked_module, emit_rust_with_options, CodegenOptions};
use ts2rs_lower::{check_module_graph, lower_module_graph};
use ts2rs_parser::{parse_module_graph_with_extra_roots, validate_imports, ParsedModuleGraph};

/// Apply `--color never|always` before [`Cli::parse`] so subcommand `--help` respects it (via `NO_COLOR`).
fn preapply_color_from_argv() {
    let args: Vec<String> = std::env::args().collect();
    let mut i = 0usize;
    while i < args.len() {
        if args[i] == "--color" && i + 1 < args.len() {
            match args[i + 1].as_str() {
                "never" => {
                    std::env::set_var("NO_COLOR", "1");
                }
                "always" => {
                    std::env::remove_var("NO_COLOR");
                }
                _ => {}
            }
            i += 2;
        } else if let Some(rest) = args[i].strip_prefix("--color=") {
            match rest {
                "never" => std::env::set_var("NO_COLOR", "1"),
                "always" => std::env::remove_var("NO_COLOR"),
                _ => {}
            }
            i += 1;
        } else {
            i += 1;
        }
    }
}

#[derive(Parser)]
#[command(
    name = "ts2rs",
    about = "TypeScript → Rust → executable (experimental)",
    version,
    color = ColorChoice::Auto
)]
struct Cli {
    #[command(flatten)]
    global: GlobalOpts,
    #[command(subcommand)]
    command: Commands,
}

#[derive(Args)]
struct GlobalOpts {
    /// 成功时不打印 warning（错误仍输出到 stderr）
    #[arg(short, long, global = true)]
    quiet: bool,
    /// 帮助文本等着色：`auto` | `always` | `never`（也可设 `NO_COLOR=1`）
    #[arg(long, global = true, value_enum, default_value_t = ColorChoice::Auto)]
    color: ColorChoice,
}

#[derive(Subcommand)]
enum Commands {
    /// 解析、HIR、语义检查，生成 Rust 写入 `-o`
    Compile(CompileCmd),
    /// 生成 Rust、临时 crate 中 cargo build 并运行入口 `main`
    Run(RunCmd),
    /// 仅解析 + HIR + 语义检查，不写文件、不调用 cargo
    Check(CheckCmd),
}

#[derive(Args)]
struct GraphInput {
    /// 入口 `.ts` 在前，其余为额外根（与 `--project` 互斥）
    #[arg(value_name = "TS")]
    inputs: Vec<PathBuf>,
    /// 极简 JSON tsconfig（仅 `files` 数组），路径相对该文件目录；与位置参数互斥
    #[arg(long, value_name = "TSCONFIG")]
    project: Option<PathBuf>,
}

#[derive(Args)]
struct CompileCmd {
    #[command(flatten)]
    graph: GraphInput,
    /// 输出 `.rs` 路径
    #[arg(short, long)]
    output: PathBuf,
    /// 每条语句前生成 `// ts: path:line:col`
    #[arg(long)]
    span_comments: bool,
    /// 为与 `run` 对齐保留；`compile` 不写 Cargo.toml，无效果
    #[arg(long)]
    link_ts2rs_rt: bool,
    /// 将 [`ts2rs_hir::IRModule`] 的 `Debug` 打到 stderr（调试用，输出可能很大）
    #[arg(long)]
    emit_ir: bool,
}

#[derive(Args)]
struct RunCmd {
    #[command(flatten)]
    graph: GraphInput,
    /// 临时 crate 的 Cargo.toml 中加入可选 path 依赖 `ts2rs_rt`（须在仓库源码树内构建）
    #[arg(long)]
    link_ts2rs_rt: bool,
    /// `cargo build` 不用 `--release`（更快，未优化）
    #[arg(long, conflicts_with = "release_flag")]
    debug: bool,
    /// 显式 `cargo build --release`（默认已开启；与 `--debug` 互斥）
    #[arg(
        short = 'O',
        long = "release",
        action = ArgAction::SetTrue,
        conflicts_with = "debug"
    )]
    release_flag: bool,
}

#[derive(Args)]
struct CheckCmd {
    #[command(flatten)]
    graph: GraphInput,
    #[arg(long)]
    emit_ir: bool,
}

#[derive(Deserialize)]
struct TsConfigFiles {
    files: Vec<String>,
}

#[derive(Debug)]
enum RunOutcome {
    Ok,
    Ts2rsErr(String),
    ChildFailed(i32),
}

fn main() {
    preapply_color_from_argv();
    let cli = Cli::parse();
    match run(cli) {
        RunOutcome::Ok => {}
        RunOutcome::Ts2rsErr(e) => {
            eprintln!("error: {e}");
            std::process::exit(1);
        }
        RunOutcome::ChildFailed(code) => std::process::exit(code),
    }
}

fn run(cli: Cli) -> RunOutcome {
    let quiet = cli.global.quiet;
    let _ = cli.global.color;

    match cli.command {
        Commands::Compile(c) => match cmd_compile(
            &c.graph.inputs,
            c.graph.project.as_deref(),
            &c.output,
            c.span_comments,
            c.emit_ir,
            quiet,
        ) {
            Ok(()) => RunOutcome::Ok,
            Err(e) => RunOutcome::Ts2rsErr(e),
        },
        Commands::Run(r) => {
            let release = r.release_flag || !r.debug;
            cmd_run(
                &r.graph.inputs,
                r.graph.project.as_deref(),
                r.link_ts2rs_rt,
                release,
                quiet,
            )
        }
        Commands::Check(c) => match cmd_check(
            &c.graph.inputs,
            c.graph.project.as_deref(),
            c.emit_ir,
            quiet,
        ) {
            Ok(()) => RunOutcome::Ok,
            Err(e) => RunOutcome::Ts2rsErr(e),
        },
    }
}

fn load_module_graph(
    project: Option<&Path>,
    inputs: &[PathBuf],
) -> Result<ParsedModuleGraph, String> {
    if let Some(tsconfig) = project {
        if !inputs.is_empty() {
            return Err(
                "cannot use --project together with positional .ts files; use one or the other"
                    .to_string(),
            );
        }
        let base = tsconfig
            .parent()
            .ok_or_else(|| "invalid tsconfig path (no parent directory)".to_string())?;
        let text = fs::read_to_string(tsconfig).map_err(|e| e.to_string())?;
        let cfg: TsConfigFiles = serde_json::from_str(&text).map_err(|e| e.to_string())?;
        if cfg.files.is_empty() {
            return Err("tsconfig `files` must be a non-empty array".to_string());
        }
        let paths: Vec<PathBuf> = cfg.files.iter().map(|f| base.join(f)).collect();
        let entry = &paths[0];
        let extra: Vec<PathBuf> = paths[1..].to_vec();
        parse_module_graph_with_extra_roots(entry, &extra).map_err(|e| e.to_string())
    } else {
        if inputs.is_empty() {
            return Err("expected at least one .ts file, or use --project".to_string());
        }
        let entry = &inputs[0];
        let extra: Vec<PathBuf> = inputs[1..].to_vec();
        parse_module_graph_with_extra_roots(entry, &extra).map_err(|e| e.to_string())
    }
}

fn ensure_entry_nonempty(graph: &ParsedModuleGraph) -> Result<(), String> {
    let p = &graph.entry;
    let src = fs::read_to_string(p).map_err(|e| e.to_string())?;
    ensure_nonempty_source(p, &src)
}

fn cmd_compile(
    inputs: &[PathBuf],
    project: Option<&Path>,
    output: &PathBuf,
    span_comments: bool,
    emit_ir: bool,
    quiet: bool,
) -> Result<(), String> {
    let graph = load_module_graph(project, inputs)?;
    ensure_entry_nonempty(&graph)?;
    validate_imports(&graph).map_err(|e| e.to_string())?;
    let units = graph.compile_units();
    let entry_path = graph.entry_path_str();
    let codegen = CodegenOptions { span_comments };

    let (module, warnings) =
        build_checked_module(&units, &entry_path).map_err(|e| e.to_string())?;
    if !quiet {
        print_warnings(&warnings);
    }
    if emit_ir {
        eprintln!("{:#?}", module);
    }
    let rust = emit_rust_with_options(&module, &codegen).map_err(|e| e.to_string())?;

    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    fs::write(output, rust).map_err(|e| e.to_string())?;
    Ok(())
}

fn cmd_run(
    inputs: &[PathBuf],
    project: Option<&Path>,
    link_ts2rs_rt: bool,
    release: bool,
    quiet: bool,
) -> RunOutcome {
    let graph = match load_module_graph(project, inputs) {
        Ok(g) => g,
        Err(e) => return RunOutcome::Ts2rsErr(e),
    };
    if let Err(e) = ensure_entry_nonempty(&graph) {
        return RunOutcome::Ts2rsErr(e);
    }
    if let Err(e) = validate_imports(&graph) {
        return RunOutcome::Ts2rsErr(e.to_string());
    }
    let units = graph.compile_units();
    let (rust, warnings) = match lower_module_graph(&units, &graph.entry_path_str()) {
        Ok(x) => x,
        Err(e) => return RunOutcome::Ts2rsErr(e.to_string()),
    };
    if !quiet {
        print_warnings(&warnings);
    }

    let opts = RustBuildOptions {
        link_ts2rs_rt,
        release,
    };
    let (_dir, exe) = match build_rust_to_executable_with_options(&rust, &opts) {
        Ok(x) => x,
        Err(e) => return RunOutcome::Ts2rsErr(e.to_string()),
    };

    let status = match Command::new(&exe)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
    {
        Ok(s) => s,
        Err(e) => return RunOutcome::Ts2rsErr(e.to_string()),
    };

    if !status.success() {
        return RunOutcome::ChildFailed(exit_code_for_failed_child(&status));
    }
    RunOutcome::Ok
}

fn cmd_check(
    inputs: &[PathBuf],
    project: Option<&Path>,
    emit_ir: bool,
    quiet: bool,
) -> Result<(), String> {
    let graph = load_module_graph(project, inputs)?;
    ensure_entry_nonempty(&graph)?;
    validate_imports(&graph).map_err(|e| e.to_string())?;
    let units = graph.compile_units();
    let entry_path = graph.entry_path_str();

    if emit_ir {
        let (module, warnings) =
            build_checked_module(&units, &entry_path).map_err(|e| e.to_string())?;
        if !quiet {
            print_warnings(&warnings);
        }
        eprintln!("{:#?}", module);
        return Ok(());
    }

    let warnings = check_module_graph(&units, &entry_path).map_err(|e| e.to_string())?;
    if !quiet {
        print_warnings(&warnings);
    }
    Ok(())
}

fn print_warnings(warnings: &[ts2rs_hir::CompileWarning]) {
    for w in warnings {
        eprintln!("warning: {w}");
    }
}

fn ensure_nonempty_source(path: &std::path::Path, src: &str) -> Result<(), String> {
    if src.trim().is_empty() {
        return Err(format!(
            "input file `{}` is empty — save it in the editor if you just typed code",
            path.display()
        ));
    }
    Ok(())
}

/// Exit code when the generated program fails; `None` from [`ExitStatus::code`] (e.g. signal) maps to `1`.
fn exit_code_for_failed_child(status: &ExitStatus) -> i32 {
    status.code().unwrap_or(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exit_code_for_failed_child_maps_some() {
        let st = Command::new("sh")
            .arg("-c")
            .arg("exit 42")
            .status()
            .expect("sh");
        assert!(!st.success());
        assert_eq!(exit_code_for_failed_child(&st), 42);
    }

    #[test]
    fn exit_code_for_failed_child_maps_none_to_one() {
        // Normal platforms give Some for exit; unwrap_or(1) still holds for hypothetical None.
        let st = Command::new("false").status().expect("false");
        assert!(!st.success());
        assert_eq!(exit_code_for_failed_child(&st), 1);
    }
}
