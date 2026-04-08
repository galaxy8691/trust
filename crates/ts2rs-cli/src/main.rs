use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use clap::{Parser, Subcommand};
use serde::Deserialize;
use ts2rs_driver::{build_rust_to_executable_with_options, RustBuildOptions};
use ts2rs_hir::CodegenOptions;
use ts2rs_lower::{lower_module_graph, lower_module_graph_with_options};
use ts2rs_parser::{
    parse_module_graph_with_extra_roots, validate_imports, ParsedModuleGraph,
};

#[derive(Parser)]
#[command(name = "ts2rs")]
#[command(about = "TypeScript → Rust → executable (experimental)")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// 解析并降级为 Rust 源码，写入文件
    Compile {
        /// 输入 `.ts`：第一个为入口，其余为额外根（与 `--project` 互斥）
        #[arg(value_name = "TS")]
        inputs: Vec<PathBuf>,
        /// 从极简 JSON tsconfig 读取 `files`（相对本文件目录）；与位置参数互斥
        #[arg(long, value_name = "TSCONFIG")]
        project: Option<PathBuf>,
        /// 输出 .rs 路径
        #[arg(short, long)]
        output: PathBuf,
        /// 在每条语句前生成 `// ts: path:line:col` 注释
        #[arg(long)]
        span_comments: bool,
        /// 为 `run` 预留；`compile` 不写 Cargo.toml，无效果
        #[arg(long)]
        link_ts2rs_rt: bool,
    },
    /// 生成 Rust、在临时 crate 中 cargo build 并运行可执行文件
    Run {
        #[arg(value_name = "TS")]
        inputs: Vec<PathBuf>,
        #[arg(long, value_name = "TSCONFIG")]
        project: Option<PathBuf>,
        /// 在生成的临时 crate 的 Cargo.toml 中加入可选 path 依赖 `ts2rs_rt`（须在源码树内构建）
        #[arg(long)]
        link_ts2rs_rt: bool,
    },
}

#[derive(Deserialize)]
struct TsConfigFiles {
    files: Vec<String>,
}

fn main() {
    let cli = Cli::parse();
    if let Err(e) = run(cli) {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}

fn run(cli: Cli) -> Result<(), String> {
    match cli.command {
        Commands::Compile {
            inputs,
            project,
            output,
            span_comments,
            link_ts2rs_rt: _,
        } => cmd_compile(&inputs, project.as_deref(), &output, span_comments),
        Commands::Run {
            inputs,
            project,
            link_ts2rs_rt,
        } => cmd_run(&inputs, project.as_deref(), link_ts2rs_rt),
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
) -> Result<(), String> {
    let graph = load_module_graph(project, inputs)?;
    ensure_entry_nonempty(&graph)?;
    validate_imports(&graph).map_err(|e| e.to_string())?;
    let units = graph.compile_units();
    let codegen = CodegenOptions { span_comments };
    let (rust, warnings) = lower_module_graph_with_options(&units, &graph.entry_path_str(), &codegen)
        .map_err(|e| e.to_string())?;
    print_warnings(&warnings);
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
) -> Result<(), String> {
    let graph = load_module_graph(project, inputs)?;
    ensure_entry_nonempty(&graph)?;
    validate_imports(&graph).map_err(|e| e.to_string())?;
    let units = graph.compile_units();
    let (rust, warnings) =
        lower_module_graph(&units, &graph.entry_path_str()).map_err(|e| e.to_string())?;
    print_warnings(&warnings);

    let opts = RustBuildOptions { link_ts2rs_rt };
    let (_dir, exe) =
        build_rust_to_executable_with_options(&rust, &opts).map_err(|e| e.to_string())?;

    let status = Command::new(&exe)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .map_err(|e| e.to_string())?;

    if !status.success() {
        return Err(format!("program exited with {status}"));
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
