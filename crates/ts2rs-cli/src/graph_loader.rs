use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;
use ts2rs_parser::{parse_module_graph_with_extra_roots, ParsedModuleGraph};

#[derive(Deserialize)]
struct TsConfigFiles {
    files: Vec<String>,
}

pub(crate) fn load_module_graph(
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

pub(crate) fn ensure_entry_nonempty(graph: &ParsedModuleGraph) -> Result<(), String> {
    let p = &graph.entry;
    let src = fs::read_to_string(p).map_err(|e| e.to_string())?;
    ensure_nonempty_source(p, &src)
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
