mod cli_args;
mod commands;
mod graph_loader;

use clap::Parser;

use cli_args::{preapply_color_from_argv, Cli, Commands};
use commands::{cmd_check, cmd_compile, cmd_run, RunOutcome};

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
            c.ts_source_comments,
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
