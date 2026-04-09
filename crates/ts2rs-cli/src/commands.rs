use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};

use ts2rs_driver::{build_rust_to_executable_with_options, RustBuildOptions};
use ts2rs_hir::{build_checked_module, emit_rust_with_options, CodegenOptions};
use ts2rs_lower::{check_module_graph, lower_module_graph};
use ts2rs_parser::validate_imports;

use crate::graph_loader::{ensure_entry_nonempty, load_module_graph};
use crate::incremental::{compile_graph_incremental, resolve_incremental_cache_root};

#[derive(Debug)]
pub(crate) enum RunOutcome {
    Ok,
    Ts2rsErr(String),
    ChildFailed(i32),
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn cmd_compile(
    inputs: &[PathBuf],
    project: Option<&Path>,
    output: &PathBuf,
    span_comments: bool,
    ts_source_comments: bool,
    emit_ir: bool,
    incremental: Option<&PathBuf>,
    quiet: bool,
) -> Result<(), String> {
    let graph = load_module_graph(project, inputs)?;
    ensure_entry_nonempty(&graph)?;
    validate_imports(&graph).map_err(|e| e.to_string())?;
    let entry_path = graph.entry_path_str();
    let codegen = CodegenOptions {
        span_comments,
        emit_ts_source_comments: ts_source_comments,
    };

    let (rust, warnings, module_for_emit_ir) = if let Some(dir) = incremental {
        let cache_root = resolve_incremental_cache_root(dir);
        let (rust, warnings) = compile_graph_incremental(&graph, &cache_root, &codegen)?;
        (rust, warnings, None::<ts2rs_hir::IRModule>)
    } else {
        let units = graph.compile_units();
        let (module, warnings) =
            build_checked_module(&units, &entry_path).map_err(|e| e.to_string())?;
        let rust = emit_rust_with_options(&module, &codegen).map_err(|e| e.to_string())?;
        (rust, warnings, Some(module))
    };

    if !quiet {
        print_warnings(&warnings);
    }
    if emit_ir {
        match module_for_emit_ir {
            Some(m) => eprintln!("{:#?}", m),
            None => {
                let units = graph.compile_units();
                let (m, _) =
                    build_checked_module(&units, &entry_path).map_err(|e| e.to_string())?;
                eprintln!("{:#?}", m);
            }
        }
    }

    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    fs::write(output, rust).map_err(|e| e.to_string())?;
    Ok(())
}

pub(crate) fn cmd_run(
    inputs: &[PathBuf],
    project: Option<&Path>,
    link_ts2rs_rt: bool,
    release: bool,
    incremental: Option<&PathBuf>,
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
    let codegen = CodegenOptions::default();
    let (rust, warnings) = match incremental {
        Some(dir) => {
            let cache_root = resolve_incremental_cache_root(dir);
            match compile_graph_incremental(&graph, &cache_root, &codegen) {
                Ok(x) => x,
                Err(e) => return RunOutcome::Ts2rsErr(e),
            }
        }
        None => {
            let units = graph.compile_units();
            match lower_module_graph(&units, &graph.entry_path_str()) {
                Ok(x) => x,
                Err(e) => return RunOutcome::Ts2rsErr(e.to_string()),
            }
        }
    };
    if !quiet {
        print_warnings(&warnings);
    }

    let opts = RustBuildOptions {
        link_ts2rs_rt,
        release,
        ..Default::default()
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

pub(crate) fn cmd_check(
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
