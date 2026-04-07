use std::fs;
use std::path::PathBuf;
use std::process::{Command, Stdio};

use clap::{Parser, Subcommand};
use ts2rs_driver::build_rust_to_executable;
use ts2rs_lower::lower_module_graph;
use ts2rs_parser::{parse_module_graph, validate_imports};

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
        /// 输入 .ts 文件
        input: PathBuf,
        /// 输出 .rs 路径
        #[arg(short, long)]
        output: PathBuf,
    },
    /// 生成 Rust、在临时 crate 中 cargo build 并运行可执行文件
    Run { input: PathBuf },
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
        Commands::Compile { input, output } => cmd_compile(&input, &output),
        Commands::Run { input } => cmd_run(&input),
    }
}

fn cmd_compile(input: &PathBuf, output: &PathBuf) -> Result<(), String> {
    let src = fs::read_to_string(input).map_err(|e| e.to_string())?;
    ensure_nonempty_source(input, &src)?;
    let graph = parse_module_graph(input).map_err(|e| e.to_string())?;
    validate_imports(&graph).map_err(|e| e.to_string())?;
    let units = graph.compile_units();
    let rust = lower_module_graph(&units, &graph.entry_path_str()).map_err(|e| e.to_string())?;
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    fs::write(output, rust).map_err(|e| e.to_string())?;
    Ok(())
}

fn cmd_run(input: &PathBuf) -> Result<(), String> {
    let src = fs::read_to_string(input).map_err(|e| e.to_string())?;
    ensure_nonempty_source(input, &src)?;
    let graph = parse_module_graph(input).map_err(|e| e.to_string())?;
    validate_imports(&graph).map_err(|e| e.to_string())?;
    let units = graph.compile_units();
    let rust = lower_module_graph(&units, &graph.entry_path_str()).map_err(|e| e.to_string())?;

    let (_dir, exe) = build_rust_to_executable(&rust).map_err(|e| e.to_string())?;

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

fn ensure_nonempty_source(path: &std::path::Path, src: &str) -> Result<(), String> {
    if src.trim().is_empty() {
        return Err(format!(
            "input file `{}` is empty — save it in the editor if you just typed code",
            path.display()
        ));
    }
    Ok(())
}
