use std::path::PathBuf;

use clap::{ArgAction, Args, ColorChoice, Parser, Subcommand};

/// Apply `--color never|always` before [`Cli::parse`] so subcommand `--help` respects it (via `NO_COLOR`).
pub(crate) fn preapply_color_from_argv() {
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
pub(crate) struct Cli {
    #[command(flatten)]
    pub(crate) global: GlobalOpts,
    #[command(subcommand)]
    pub(crate) command: Commands,
}

#[derive(Args)]
pub(crate) struct GlobalOpts {
    /// 成功时不打印 warning（错误仍输出到 stderr）
    #[arg(short, long, global = true)]
    pub(crate) quiet: bool,
    /// 帮助文本等着色：`auto` | `always` | `never`（也可设 `NO_COLOR=1`）
    #[arg(long, global = true, value_enum, default_value_t = ColorChoice::Auto)]
    pub(crate) color: ColorChoice,
}

#[derive(Subcommand)]
pub(crate) enum Commands {
    /// 解析、HIR、语义检查，生成 Rust 写入 `-o`
    Compile(CompileCmd),
    /// 生成 Rust、临时 crate 中 cargo build 并运行入口 `main`
    Run(RunCmd),
    /// 仅解析 + HIR + 语义检查，不写文件、不调用 cargo
    Check(CheckCmd),
}

#[derive(Args)]
pub(crate) struct GraphInput {
    /// 入口 `.ts` 在前，其余为额外根（与 `--project` 互斥）
    #[arg(value_name = "TS")]
    pub(crate) inputs: Vec<PathBuf>,
    /// JSON tsconfig：`extends`、`files`、`include`、`exclude`（简化合并，无 npm）；路径相对各字段所在配置目录；与位置参数互斥
    #[arg(long, value_name = "TSCONFIG")]
    pub(crate) project: Option<PathBuf>,
}

#[derive(Args)]
pub(crate) struct CompileCmd {
    #[command(flatten)]
    pub(crate) graph: GraphInput,
    /// 输出 `.rs` 路径
    #[arg(short, long)]
    pub(crate) output: PathBuf,
    /// 每条语句前生成 `// ts: path:line:col`
    #[arg(long)]
    pub(crate) span_comments: bool,
    /// 将 TS 源码中的 leading 注释（`//` / `/* */`）写入生成的 Rust 行注释
    #[arg(long)]
    pub(crate) ts_source_comments: bool,
    /// 为与 `run` 对齐保留；`compile` 不写 Cargo.toml，无效果
    #[arg(long)]
    pub(crate) link_ts2rs_rt: bool,
    /// 将 [`ts2rs_hir::IRModule`] 的 `Debug` 打到 stderr（调试用，输出可能很大）
    #[arg(long)]
    pub(crate) emit_ir: bool,
}

#[derive(Args)]
pub(crate) struct RunCmd {
    #[command(flatten)]
    pub(crate) graph: GraphInput,
    /// 临时 crate 的 Cargo.toml 中加入可选 path 依赖 `ts2rs_rt`（须在仓库源码树内构建）
    #[arg(long)]
    pub(crate) link_ts2rs_rt: bool,
    /// `cargo build` 不用 `--release`（更快，未优化）
    #[arg(long, conflicts_with = "release_flag")]
    pub(crate) debug: bool,
    /// 显式 `cargo build --release`（默认已开启；与 `--debug` 互斥）
    #[arg(
        short = 'O',
        long = "release",
        action = ArgAction::SetTrue,
        conflicts_with = "debug"
    )]
    pub(crate) release_flag: bool,
}

#[derive(Args)]
pub(crate) struct CheckCmd {
    #[command(flatten)]
    pub(crate) graph: GraphInput,
    #[arg(long)]
    pub(crate) emit_ir: bool,
}
