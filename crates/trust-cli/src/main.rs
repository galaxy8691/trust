mod add_rustdoc;
mod add_spec;
mod cli_args;
mod commands;
mod graph_loader;
mod incremental;
mod tsconfig_resolve;

use clap::Parser;

use cli_args::{preapply_color_from_argv, Cli, Commands};
use commands::{cmd_add, cmd_check, cmd_compile, cmd_init, cmd_run, RunOutcome};

fn main() {
    preapply_color_from_argv();
    let cli = Cli::parse();
    match run(cli) {
        RunOutcome::Ok => {}
        RunOutcome::TrustErr(e) => {
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
        Commands::Compile(c) => {
            let release = c.release_flag || !c.debug;
            match cmd_compile(
                &c.graph.inputs,
                c.graph.project.as_deref(),
                c.output.as_ref(),
                c.exec,
                c.span_comments,
                c.ts_source_comments,
                c.stdlib_mode,
                c.link_trust_rt,
                release,
                c.emit_ir,
                c.incremental.as_ref(),
                quiet,
            ) {
                Ok(()) => RunOutcome::Ok,
                Err(e) => RunOutcome::TrustErr(e),
            }
        }
        Commands::Run(r) => {
            let release = r.release_flag || !r.debug;
            cmd_run(
                &r.graph.inputs,
                r.graph.project.as_deref(),
                r.link_trust_rt,
                release,
                r.incremental.as_ref(),
                r.stdlib_mode,
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
            Err(e) => RunOutcome::TrustErr(e),
        },
        Commands::Init(i) => match cmd_init(&i.dir, i.force) {
            Ok(()) => RunOutcome::Ok,
            Err(e) => RunOutcome::TrustErr(e),
        },
        Commands::Add(a) => match cmd_add(
            &a.rust_path,
            &a.dir,
            a.returns.as_deref(),
            a.args.as_deref(),
        ) {
            Ok(()) => RunOutcome::Ok,
            Err(e) => RunOutcome::TrustErr(e),
        },
    }
}
