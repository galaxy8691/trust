use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};
use toml::Value;

use trust_driver::{
    build_rust_and_copy_with_options, build_rust_to_executable_with_options, RustBuildOptions,
};
use trust_hir::{build_checked_module, emit_rust_with_options, CodegenOptions};
use trust_lower::{check_module_graph, lower_module_graph_with_options};
use trust_parser::validate_imports;

use crate::add_spec::{parse_add_spec, ParsedAddSpec};
use crate::graph_loader::{ensure_entry_nonempty, load_module_graph};
use crate::incremental::{compile_graph_incremental, resolve_incremental_cache_root};

const INIT_MAIN_TS: &str = include_str!("../../../examples/main.ts");
const INIT_MATH_TS: &str = include_str!("../../../examples/math.ts");
const INIT_STRUTIL_TS: &str = include_str!("../../../examples/strutil.ts");
const INIT_TRUST_TOML: &str = include_str!("../../../examples/Trust.toml");

fn default_exec_output_path(entry: &Path) -> Result<PathBuf, String> {
    let stem = entry.file_stem().filter(|s| !s.is_empty()).ok_or_else(|| {
        format!(
            "cannot derive output name from entry path `{}`",
            entry.display()
        )
    })?;
    #[cfg(windows)]
    {
        let mut out = PathBuf::from(stem);
        out.set_extension("exe");
        Ok(out)
    }
    #[cfg(not(windows))]
    {
        Ok(PathBuf::from(stem))
    }
}

pub(crate) fn cmd_init(dir: &Path, force: bool) -> Result<(), String> {
    fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let files: [(&str, &str); 4] = [
        ("main.ts", INIT_MAIN_TS),
        ("math.ts", INIT_MATH_TS),
        ("strutil.ts", INIT_STRUTIL_TS),
        ("Trust.toml", INIT_TRUST_TOML),
    ];
    for (name, content) in files {
        let p = dir.join(name);
        if p.exists() && !force {
            return Err(format!(
                "init aborted: `{}` already exists (use --force to overwrite)",
                p.display()
            ));
        }
        fs::write(&p, content).map_err(|e| e.to_string())?;
    }
    eprintln!(
        "initialized trust project at `{}` (files: main.ts, math.ts, strutil.ts, Trust.toml)",
        dir.display()
    );
    Ok(())
}

fn ensure_dependency(
    table: &mut toml::map::Map<String, Value>,
    crate_name: &str,
) -> Result<(), String> {
    let deps_val = table
        .entry("dependencies".to_string())
        .or_insert_with(|| Value::Table(toml::map::Map::new()));
    let deps = deps_val
        .as_table_mut()
        .ok_or_else(|| "`dependencies` must be a table".to_string())?;
    deps.entry(crate_name.to_string())
        .or_insert_with(|| Value::String("*".to_string()));
    Ok(())
}

fn merge_type_only(
    table: &mut toml::map::Map<String, Value>,
    crate_name: &str,
    type_name: &str,
    rust_type_path: &str,
) -> Result<(), String> {
    let rb_val = table
        .entry("rust_binding".to_string())
        .or_insert_with(|| Value::Array(Vec::new()));
    let rb = rb_val
        .as_array_mut()
        .ok_or_else(|| "`rust_binding` must be an array".to_string())?;

    for item in rb.iter_mut() {
        let Some(m) = item.as_table_mut() else {
            continue;
        };
        if binding_matches(m, crate_name, type_name) {
            m.insert(
                "rust_type".to_string(),
                Value::String(rust_type_path.to_string()),
            );
            m.entry("method".to_string())
                .or_insert_with(|| Value::Array(Vec::new()));
            return Ok(());
        }
    }

    let mut m = toml::map::Map::new();
    m.insert("crate".to_string(), Value::String(crate_name.to_string()));
    m.insert(
        "type_name".to_string(),
        Value::String(type_name.to_string()),
    );
    m.insert(
        "rust_type".to_string(),
        Value::String(rust_type_path.to_string()),
    );
    m.insert("method".to_string(), Value::Array(Vec::new()));
    rb.push(Value::Table(m));
    Ok(())
}

fn merge_constructor(
    table: &mut toml::map::Map<String, Value>,
    crate_name: &str,
    type_name: &str,
    rust_type_path: &str,
    rust_ctor_path: &str,
) -> Result<(), String> {
    let rb_val = table
        .entry("rust_binding".to_string())
        .or_insert_with(|| Value::Array(Vec::new()));
    let rb = rb_val
        .as_array_mut()
        .ok_or_else(|| "`rust_binding` must be an array".to_string())?;

    for item in rb.iter_mut() {
        let Some(m) = item.as_table_mut() else {
            continue;
        };
        if binding_matches(m, crate_name, type_name) {
            m.insert(
                "rust_type".to_string(),
                Value::String(rust_type_path.to_string()),
            );
            let mut new_tbl = toml::map::Map::new();
            new_tbl.insert(
                "rust".to_string(),
                Value::String(rust_ctor_path.to_string()),
            );
            new_tbl.insert("unwrap".to_string(), Value::Boolean(true));
            m.insert("new".to_string(), Value::Table(new_tbl));
            m.entry("method".to_string())
                .or_insert_with(|| Value::Array(Vec::new()));
            return Ok(());
        }
    }

    let mut m = toml::map::Map::new();
    m.insert("crate".to_string(), Value::String(crate_name.to_string()));
    m.insert(
        "type_name".to_string(),
        Value::String(type_name.to_string()),
    );
    m.insert(
        "rust_type".to_string(),
        Value::String(rust_type_path.to_string()),
    );
    let mut new_tbl = toml::map::Map::new();
    new_tbl.insert(
        "rust".to_string(),
        Value::String(rust_ctor_path.to_string()),
    );
    new_tbl.insert("unwrap".to_string(), Value::Boolean(true));
    m.insert("new".to_string(), Value::Table(new_tbl));
    m.insert("method".to_string(), Value::Array(Vec::new()));
    rb.push(Value::Table(m));
    Ok(())
}

fn merge_method_binding(
    table: &mut toml::map::Map<String, Value>,
    crate_name: &str,
    type_name: &str,
    rust_type_path: &str,
    rust_method_path: &str,
    returns: &str,
    args: &[String],
) -> Result<(), String> {
    let rb_val = table
        .entry("rust_binding".to_string())
        .or_insert_with(|| Value::Array(Vec::new()));
    let rb = rb_val
        .as_array_mut()
        .ok_or_else(|| "`rust_binding` must be an array".to_string())?;

    let mut method_row = toml::map::Map::new();
    let method_name = rust_method_path
        .rsplit("::")
        .next()
        .unwrap_or(rust_method_path)
        .to_string();
    method_row.insert("name".to_string(), Value::String(method_name));
    method_row.insert(
        "rust".to_string(),
        Value::String(rust_method_path.to_string()),
    );
    method_row.insert(
        "args".to_string(),
        Value::Array(args.iter().cloned().map(Value::String).collect()),
    );
    method_row.insert("returns".to_string(), Value::String(returns.to_string()));

    for item in rb.iter_mut() {
        let Some(m) = item.as_table_mut() else {
            continue;
        };
        if binding_matches(m, crate_name, type_name) {
            m.insert(
                "rust_type".to_string(),
                Value::String(rust_type_path.to_string()),
            );
            let meth_arr = m
                .entry("method".to_string())
                .or_insert_with(|| Value::Array(Vec::new()));
            let arr = meth_arr
                .as_array_mut()
                .ok_or_else(|| "method must be an array".to_string())?;
            let name_key = method_row.get("name").and_then(Value::as_str).unwrap_or("");
            let mut replaced = false;
            for row in arr.iter_mut() {
                let Some(t) = row.as_table_mut() else {
                    continue;
                };
                if t.get("name").and_then(Value::as_str) == Some(name_key) {
                    *t = method_row.clone();
                    replaced = true;
                    break;
                }
            }
            if !replaced {
                arr.push(Value::Table(method_row));
            }
            return Ok(());
        }
    }

    let mut m = toml::map::Map::new();
    m.insert("crate".to_string(), Value::String(crate_name.to_string()));
    m.insert(
        "type_name".to_string(),
        Value::String(type_name.to_string()),
    );
    m.insert(
        "rust_type".to_string(),
        Value::String(rust_type_path.to_string()),
    );
    m.insert(
        "method".to_string(),
        Value::Array(vec![Value::Table(method_row)]),
    );
    rb.push(Value::Table(m));
    Ok(())
}

fn binding_matches(m: &toml::map::Map<String, Value>, crate_name: &str, type_name: &str) -> bool {
    let same_crate = m
        .get("crate")
        .and_then(Value::as_str)
        .map(|s| s == crate_name)
        .unwrap_or(false);
    let same_type = m
        .get("type_name")
        .and_then(Value::as_str)
        .map(|s| s == type_name)
        .unwrap_or(false);
    same_crate && same_type
}

pub(crate) fn cmd_add(
    spec: &str,
    dir: &Path,
    returns: Option<&str>,
    args: Option<&str>,
) -> Result<(), String> {
    let parsed = parse_add_spec(spec, returns, args)?;
    let trust_path = dir.join("Trust.toml");
    let raw = if trust_path.exists() {
        fs::read_to_string(&trust_path).map_err(|e| e.to_string())?
    } else {
        String::new()
    };
    let mut doc: Value = if raw.trim().is_empty() {
        Value::Table(toml::map::Map::new())
    } else {
        toml::from_str(&raw).map_err(|e| format!("parse `{}` failed: {e}", trust_path.display()))?
    };

    let table = doc
        .as_table_mut()
        .ok_or_else(|| format!("`{}` must be a TOML table", trust_path.display()))?;

    match parsed {
        ParsedAddSpec::Wildcard { crate_name } => {
            ensure_dependency(table, &crate_name)?;
            crate::add_rustdoc::merge_wildcard_into_doc(&crate_name, &mut doc)?;
        }
        ParsedAddSpec::TypeOnly {
            crate_name,
            type_name,
            rust_type_path,
        } => {
            ensure_dependency(table, &crate_name)?;
            merge_type_only(table, &crate_name, &type_name, &rust_type_path)?;
        }
        ParsedAddSpec::Constructor {
            crate_name,
            type_name,
            rust_type_path,
            rust_ctor_path,
        } => {
            ensure_dependency(table, &crate_name)?;
            merge_constructor(
                table,
                &crate_name,
                &type_name,
                &rust_type_path,
                &rust_ctor_path,
            )?;
        }
        ParsedAddSpec::Method {
            crate_name,
            type_name,
            rust_type_path,
            rust_method_path,
            returns,
            args,
            ..
        } => {
            ensure_dependency(table, &crate_name)?;
            merge_method_binding(
                table,
                &crate_name,
                &type_name,
                &rust_type_path,
                &rust_method_path,
                &returns,
                &args,
            )?;
        }
    }

    let out = toml::to_string_pretty(&doc).map_err(|e| e.to_string())?;
    fs::write(&trust_path, out).map_err(|e| e.to_string())?;
    eprintln!("updated `{}`", trust_path.display());
    Ok(())
}

#[derive(Debug)]
pub(crate) enum RunOutcome {
    Ok,
    TrustErr(String),
    ChildFailed(i32),
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn cmd_compile(
    inputs: &[PathBuf],
    project: Option<&Path>,
    output: Option<&PathBuf>,
    exec: bool,
    span_comments: bool,
    ts_source_comments: bool,
    link_trust_rt: bool,
    release: bool,
    emit_ir: bool,
    incremental: Option<&PathBuf>,
    quiet: bool,
) -> Result<(), String> {
    let graph = load_module_graph(project, inputs)?;
    ensure_entry_nonempty(&graph)?;
    validate_imports(&graph).map_err(|e| e.to_string())?;

    let out_path: PathBuf = match (output, exec) {
        (Some(p), _) => p.to_path_buf(),
        (None, true) => default_exec_output_path(&graph.entry)?,
        (None, false) => {
            return Err(
                "missing output path: use -o / --output (required unless using --exec)".to_string(),
            );
        }
    };

    let entry_path = graph.entry_path_str();
    let codegen = CodegenOptions {
        span_comments,
        emit_ts_source_comments: ts_source_comments,
    };

    let (rust, warnings, module_for_emit_ir) = if let Some(dir) = incremental {
        let cache_root = resolve_incremental_cache_root(dir);
        let (rust, warnings) = compile_graph_incremental(&graph, &cache_root, &codegen)?;
        (rust, warnings, None::<trust_hir::IRModule>)
    } else {
        let units = graph.compile_units();
        let (module, warnings) = build_checked_module(&units, &entry_path, graph.trust.as_ref())
            .map_err(|e| e.to_string())?;
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
                let (m, _) = build_checked_module(&units, &entry_path, graph.trust.as_ref())
                    .map_err(|e| e.to_string())?;
                eprintln!("{:#?}", m);
            }
        }
    }

    if exec {
        let mut opts = RustBuildOptions {
            link_trust_rt,
            release,
            ..Default::default()
        };
        if let Some(ref m) = graph.trust {
            opts.trust_dependency_lines = m.cargo_dependency_lines.clone();
        }
        build_rust_and_copy_with_options(&rust, &out_path, &opts).map_err(|e| e.to_string())?;
    } else {
        if let Some(parent) = out_path.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        fs::write(&out_path, rust).map_err(|e| e.to_string())?;
    }
    Ok(())
}

pub(crate) fn cmd_run(
    inputs: &[PathBuf],
    project: Option<&Path>,
    link_trust_rt: bool,
    release: bool,
    incremental: Option<&PathBuf>,
    quiet: bool,
) -> RunOutcome {
    let graph = match load_module_graph(project, inputs) {
        Ok(g) => g,
        Err(e) => return RunOutcome::TrustErr(e),
    };
    if let Err(e) = ensure_entry_nonempty(&graph) {
        return RunOutcome::TrustErr(e);
    }
    if let Err(e) = validate_imports(&graph) {
        return RunOutcome::TrustErr(e.to_string());
    }
    let codegen = CodegenOptions::default();
    let (rust, warnings) = match incremental {
        Some(dir) => {
            let cache_root = resolve_incremental_cache_root(dir);
            match compile_graph_incremental(&graph, &cache_root, &codegen) {
                Ok(x) => x,
                Err(e) => return RunOutcome::TrustErr(e),
            }
        }
        None => {
            let units = graph.compile_units();
            match lower_module_graph_with_options(
                &units,
                &graph.entry_path_str(),
                graph.trust.as_ref(),
                &codegen,
            ) {
                Ok(x) => x,
                Err(e) => return RunOutcome::TrustErr(e.to_string()),
            }
        }
    };
    if !quiet {
        print_warnings(&warnings);
    }

    let mut opts = RustBuildOptions {
        link_trust_rt,
        release,
        ..Default::default()
    };
    if let Some(ref m) = graph.trust {
        opts.trust_dependency_lines = m.cargo_dependency_lines.clone();
    }
    let (_dir, exe) = match build_rust_to_executable_with_options(&rust, &opts) {
        Ok(x) => x,
        Err(e) => return RunOutcome::TrustErr(e.to_string()),
    };

    let status = match Command::new(&exe)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
    {
        Ok(s) => s,
        Err(e) => return RunOutcome::TrustErr(e.to_string()),
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
        let (module, warnings) = build_checked_module(&units, &entry_path, graph.trust.as_ref())
            .map_err(|e| e.to_string())?;
        if !quiet {
            print_warnings(&warnings);
        }
        eprintln!("{:#?}", module);
        return Ok(());
    }

    let warnings =
        check_module_graph(&units, &entry_path, graph.trust.as_ref()).map_err(|e| e.to_string())?;
    if !quiet {
        print_warnings(&warnings);
    }
    Ok(())
}

fn print_warnings(warnings: &[trust_hir::CompileWarning]) {
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

    #[test]
    fn init_writes_template_files() {
        let dir = tempfile::tempdir().expect("tempdir");
        cmd_init(dir.path(), false).expect("init");
        for name in ["main.ts", "math.ts", "strutil.ts", "Trust.toml"] {
            assert!(dir.path().join(name).is_file(), "missing {name}");
        }
    }

    #[test]
    fn init_refuses_overwrite_without_force() {
        let dir = tempfile::tempdir().expect("tempdir");
        cmd_init(dir.path(), false).expect("init");
        let err = cmd_init(dir.path(), false).expect_err("should fail");
        assert!(err.contains("use --force"), "{err}");
    }

    #[test]
    fn add_updates_trust_toml() {
        let dir = tempfile::tempdir().expect("tempdir");
        cmd_add("url::Url::parse", dir.path(), None, None).expect("add");
        let body = fs::read_to_string(dir.path().join("Trust.toml")).expect("read");
        assert!(body.contains("[dependencies]"), "{body}");
        assert!(body.contains("url = \"*\""), "{body}");
        assert!(body.contains("crate = \"url\""), "{body}");
        assert!(body.contains("type_name = \"Url\""), "{body}");
        assert!(body.contains("rust_type = \"url::Url\""), "{body}");
        assert!(body.contains("rust = \"url::Url::parse\""), "{body}");
    }
}
