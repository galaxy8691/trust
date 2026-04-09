//! `trust add crate::*`：通过 `cargo +nightly rustdoc` 生成 rustdoc JSON，启发式填充 `[[rust_binding]]`。

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;
use tempfile::TempDir;
use toml::Value as TomlValue;

/// 从已解析的 rustdoc JSON（`serde_json::Value`）提取可映射的绑定（单元测试与 `merge_wildcard` 共用）。
pub(crate) fn extract_bindings_from_rustdoc_json(
    json: &Value,
    crate_key: &str,
) -> Result<Vec<GeneratedBinding>, String> {
    let ver = json
        .get("format_version")
        .and_then(|v| v.as_u64())
        .ok_or_else(|| "rustdoc JSON missing format_version".to_string())?;
    if !(50..=100).contains(&ver) {
        eprintln!(
            "warning: rustdoc format_version {ver} may be unsupported; continuing heuristically"
        );
    }

    let index = json
        .get("index")
        .and_then(|v| v.as_object())
        .ok_or_else(|| "rustdoc JSON missing index".to_string())?;

    let mut by_type: HashMap<String, GeneratedBinding> = HashMap::new();

    for item in index.values() {
        let inner = match item.get("inner") {
            Some(i) => i,
            None => continue,
        };
        let impl_obj = match inner.get("impl") {
            Some(Value::Object(o)) => o,
            _ => continue,
        };

        if impl_obj.get("is_synthetic").and_then(|v| v.as_bool()) == Some(true) {
            continue;
        }
        if impl_obj.get("is_negative").and_then(|v| v.as_bool()) == Some(true) {
            continue;
        }
        // Trait impl — 跳过（非固有 impl）
        if let Some(t) = impl_obj.get("trait") {
            if !t.is_null() {
                continue;
            }
        }

        let for_ty = match impl_obj.get("for") {
            Some(t) => t,
            None => continue,
        };
        let Some(rust_type_path) = type_path_string(for_ty) else {
            continue;
        };
        let segments: Vec<&str> = rust_type_path.split("::").collect();
        if segments.len() < 2 {
            continue;
        }
        if segments[0] != crate_key {
            continue;
        }
        let type_name = segments[segments.len() - 1].to_string();

        let entry = by_type
            .entry(rust_type_path.clone())
            .or_insert_with(|| GeneratedBinding {
                crate_name: crate_key.to_string(),
                type_name: type_name.clone(),
                rust_type: rust_type_path.clone(),
                new: None,
                methods: Vec::new(),
            });

        let item_ids = impl_obj
            .get("items")
            .and_then(|v| v.as_array())
            .map(|a| a.as_slice())
            .unwrap_or(&[]);

        for id in item_ids {
            let id_str = match id.as_str() {
                Some(s) => s,
                None => continue,
            };
            let Some(fn_item) = index.get(id_str) else {
                continue;
            };
            let Some(name) = fn_item.get("name").and_then(|n| n.as_str()) else {
                continue;
            };
            let finner = match fn_item.get("inner") {
                Some(i) => i,
                None => continue,
            };
            let func = match finner.get("function") {
                Some(Value::Object(f)) => f,
                _ => continue,
            };
            let sig = match func.get("sig").and_then(|s| s.as_object()) {
                Some(s) => s,
                None => continue,
            };
            let inputs = match sig.get("inputs").and_then(|i| i.as_array()) {
                Some(i) => i,
                None => continue,
            };

            let header = func.get("header");
            if header
                .and_then(|h| h.get("is_async"))
                .and_then(|v| v.as_bool())
                == Some(true)
            {
                eprintln!("warning: rustdoc: skip async fn `{name}` (unsupported)");
                continue;
            }

            let first_is_self = inputs
                .first()
                .and_then(|tup| tup.as_array())
                .and_then(|a| a.first())
                .and_then(|nm| nm.as_str())
                .map(|n| n == "self" || n.starts_with("__self"))
                .unwrap_or(false);

            let rust_fn_path = format!("{}::{}", entry.rust_type, name);

            if first_is_self {
                let arg_tys: Vec<&Value> = inputs
                    .iter()
                    .skip(1)
                    .filter_map(|tup| tup.as_array())
                    .filter_map(|a| a.get(1))
                    .collect();
                let mut trust_args = Vec::new();
                let mut ok = true;
                for t in arg_tys {
                    match map_arg_type(t) {
                        Some(x) => trust_args.push(x),
                        None => {
                            ok = false;
                            break;
                        }
                    }
                }
                if !ok {
                    eprintln!("warning: rustdoc: skip method `{name}` (unmapped argument types)");
                    continue;
                }
                let out = match sig.get("output") {
                    None | Some(Value::Null) => Some("void".to_string()),
                    Some(o) => match map_return_type(o) {
                        Some(r) => Some(r),
                        None => {
                            eprintln!(
                                "warning: rustdoc: skip method `{name}` (unmapped return type)"
                            );
                            continue;
                        }
                    },
                };
                let returns = out.expect("mapped");
                entry.methods.push(MethodRow {
                    name: name.to_string(),
                    rust: None,
                    args: trust_args,
                    returns,
                });
            } else {
                // 关联函数：仅将 `new` / `parse` 记为 `new` 绑定（与现有 `trust add url::Url::parse` 一致）
                let output = sig.get("output");
                let looks_like_result = output
                    .map(|o| type_path_string(o).unwrap_or_default().contains("Result"))
                    .unwrap_or(false);
                if name == "new" || name == "parse" {
                    entry.new = Some(NewRow {
                        rust: rust_fn_path,
                        unwrap: looks_like_result,
                    });
                } else {
                    eprintln!(
                        "warning: rustdoc: skip associated fn `{name}` (only new/parse become [[rust_binding]].new)"
                    );
                }
            }
        }
    }

    let mut out: Vec<_> = by_type.into_values().collect();
    out.sort_by(|a, b| a.rust_type.cmp(&b.rust_type));
    if out.is_empty() {
        return Err(
            "rustdoc produced no mappable inherent items (check crate_key, visibility, or nightly rustdoc)"
                .to_string(),
        );
    }
    Ok(out)
}

pub(crate) struct NewRow {
    pub rust: String,
    pub unwrap: bool,
}

pub(crate) struct MethodRow {
    pub name: String,
    pub rust: Option<String>,
    pub args: Vec<String>,
    pub returns: String,
}

pub(crate) struct GeneratedBinding {
    pub crate_name: String,
    pub type_name: String,
    pub rust_type: String,
    pub new: Option<NewRow>,
    pub methods: Vec<MethodRow>,
}

fn type_path_string(ty: &Value) -> Option<String> {
    if let Some(p) = ty.get("resolved_path") {
        return p
            .get("path")
            .and_then(|x| x.as_str())
            .map(|s| s.to_string());
    }
    if let Some(q) = ty.get("qualified_path") {
        if let Some(p) = q.get("self_type") {
            return type_path_string(p);
        }
    }
    if let Some(b) = ty.get("borrowed_ref") {
        return type_path_string(b.get("type")?);
    }
    if let Some(p) = ty.get("primitive") {
        if let Some(s) = p.as_str() {
            return Some(s.to_string());
        }
    }
    None
}

fn map_arg_type(ty: &Value) -> Option<String> {
    let s = type_path_string(ty)?;
    map_path_str_to_trust(&s)
}

fn map_return_type(ty: &Value) -> Option<String> {
    let s = type_path_string(ty)?;
    if s == "()" {
        return Some("void".to_string());
    }
    map_path_str_to_trust(&s)
}

fn map_path_str_to_trust(path: &str) -> Option<String> {
    let tail = path.rsplit("::").next().unwrap_or(path);
    match tail {
        "bool" => Some("boolean".to_string()),
        "f64" | "f32" | "i8" | "i16" | "i32" | "i64" | "i128" | "u8" | "u16" | "u32" | "u64"
        | "u128" | "isize" | "usize" => Some("number".to_string()),
        "str" | "String" => Some("string".to_string()),
        "()" => Some("void".to_string()),
        _ => None,
    }
}

/// 运行 `cargo +nightly rustdoc -p <crate>`，返回 `target/doc/<crate>.json` 路径。
pub(crate) fn run_rustdoc_json(crate_name: &str, dep_spec: &str) -> Result<PathBuf, String> {
    let tmp = TempDir::new().map_err(|e| e.to_string())?;
    let root = tmp.path();
    let cargo_toml = format!(
        r#"[package]
name = "trust_rustdoc_probe"
version = "0.0.0"
edition = "2021"

[dependencies]
{crate_name} = "{dep}"
"#,
        crate_name = crate_name,
        dep = dep_spec.replace('"', "")
    );
    fs::create_dir_all(root.join("src")).map_err(|e| e.to_string())?;
    fs::write(root.join("Cargo.toml"), cargo_toml).map_err(|e| e.to_string())?;
    fs::write(root.join("src/main.rs"), "fn main() {}\n").map_err(|e| e.to_string())?;

    let st = Command::new("cargo")
        .current_dir(root)
        .args([
            "+nightly",
            "rustdoc",
            "-p",
            crate_name,
            "--",
            "-Z",
            "unstable-options",
            "--output-format=json",
        ])
        .status()
        .map_err(|e| {
            format!(
                "failed to spawn `cargo +nightly rustdoc` ({e}); install nightly: rustup toolchain install nightly"
            )
        })?;

    if !st.success() {
        return Err(
            "`cargo +nightly rustdoc` failed (rustdoc JSON needs nightly). Try: rustup toolchain install nightly"
                .to_string(),
        );
    }

    let doc_dir = root.join("target/doc");
    find_doc_json(&doc_dir, crate_name).ok_or_else(|| {
        format!(
            "rustdoc JSON not found under `{}` (expected {}.json)",
            doc_dir.display(),
            crate_name.replace('-', "_")
        )
    })
}

fn find_doc_json(doc_dir: &Path, crate_name: &str) -> Option<PathBuf> {
    let candidates = [
        doc_dir.join(format!("{}.json", crate_name.replace('-', "_"))),
        doc_dir.join(format!("{crate_name}.json")),
    ];
    for c in &candidates {
        if c.is_file() {
            return Some(c.clone());
        }
    }
    let read = fs::read_dir(doc_dir).ok()?;
    let mut jsons: Vec<PathBuf> = read
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "json"))
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| !n.starts_with('.') && n != "search-index.json")
        })
        .collect();
    if jsons.len() == 1 {
        return jsons.pop();
    }
    None
}

/// 读取 `Trust.toml` 中 `[dependencies].<crate>` 的版本说明，缺省为 `*`。
pub(crate) fn dependency_spec_for_crate(doc: &TomlValue, crate_name: &str) -> String {
    let Some(table) = doc.as_table() else {
        return "*".to_string();
    };
    let Some(deps) = table.get("dependencies").and_then(|d| d.as_table()) else {
        return "*".to_string();
    };
    match deps.get(crate_name) {
        Some(TomlValue::String(s)) => s.clone(),
        Some(TomlValue::Table(t)) => {
            if let Some(TomlValue::String(v)) = t.get("version") {
                v.clone()
            } else {
                "*".to_string()
            }
        }
        _ => "*".to_string(),
    }
}

/// 合并通配符绑定到已加载的 TOML 根表。
pub(crate) fn merge_wildcard_into_doc(crate_key: &str, doc: &mut TomlValue) -> Result<(), String> {
    let dep = dependency_spec_for_crate(doc, crate_key);
    let json_path = run_rustdoc_json(crate_key, &dep)?;
    let text = fs::read_to_string(&json_path).map_err(|e| e.to_string())?;
    let v: Value = serde_json::from_str(&text).map_err(|e| format!("rustdoc JSON parse: {e}"))?;
    let bindings = extract_bindings_from_rustdoc_json(&v, crate_key)?;
    apply_generated_bindings_to_toml(doc, crate_key, &bindings)
}

fn apply_generated_bindings_to_toml(
    doc: &mut TomlValue,
    crate_key: &str,
    bindings: &[GeneratedBinding],
) -> Result<(), String> {
    let table = doc
        .as_table_mut()
        .ok_or_else(|| "Trust.toml root must be a table".to_string())?;

    let deps_val = table
        .entry("dependencies".to_string())
        .or_insert_with(|| TomlValue::Table(toml::map::Map::new()));
    let deps = deps_val
        .as_table_mut()
        .ok_or_else(|| "`dependencies` must be a table".to_string())?;
    deps.entry(crate_key.to_string())
        .or_insert_with(|| TomlValue::String("*".to_string()));

    let rb_val = table
        .entry("rust_binding".to_string())
        .or_insert_with(|| TomlValue::Array(Vec::new()));
    let rb = rb_val
        .as_array_mut()
        .ok_or_else(|| "`rust_binding` must be an array".to_string())?;

    for gen in bindings {
        let mut found = false;
        for item in rb.iter_mut() {
            let Some(m) = item.as_table_mut() else {
                continue;
            };
            let same_crate = m
                .get("crate")
                .and_then(TomlValue::as_str)
                .map(|s| s == gen.crate_name)
                .unwrap_or(false);
            let same_type = m
                .get("type_name")
                .and_then(TomlValue::as_str)
                .map(|s| s == gen.type_name)
                .unwrap_or(false);
            if same_crate && same_type {
                m.insert(
                    "rust_type".to_string(),
                    TomlValue::String(gen.rust_type.clone()),
                );
                if let Some(n) = &gen.new {
                    let mut new_tbl = toml::map::Map::new();
                    new_tbl.insert("rust".to_string(), TomlValue::String(n.rust.clone()));
                    new_tbl.insert("unwrap".to_string(), TomlValue::Boolean(n.unwrap));
                    m.insert("new".to_string(), TomlValue::Table(new_tbl));
                }
                merge_method_table(m, &gen.methods)?;
                found = true;
                break;
            }
        }
        if !found {
            let mut m = toml::map::Map::new();
            m.insert(
                "crate".to_string(),
                TomlValue::String(gen.crate_name.clone()),
            );
            m.insert(
                "type_name".to_string(),
                TomlValue::String(gen.type_name.clone()),
            );
            m.insert(
                "rust_type".to_string(),
                TomlValue::String(gen.rust_type.clone()),
            );
            if let Some(n) = &gen.new {
                let mut new_tbl = toml::map::Map::new();
                new_tbl.insert("rust".to_string(), TomlValue::String(n.rust.clone()));
                new_tbl.insert("unwrap".to_string(), TomlValue::Boolean(n.unwrap));
                m.insert("new".to_string(), TomlValue::Table(new_tbl));
            }
            let mut arr = Vec::new();
            for met in &gen.methods {
                arr.push(method_to_toml(met));
            }
            m.insert("method".to_string(), TomlValue::Array(arr));
            rb.push(TomlValue::Table(m));
        }
    }
    Ok(())
}

fn merge_method_table(
    m: &mut toml::map::Map<String, TomlValue>,
    methods: &[MethodRow],
) -> Result<(), String> {
    let meth_arr = m
        .entry("method".to_string())
        .or_insert_with(|| TomlValue::Array(Vec::new()));
    let arr = meth_arr
        .as_array_mut()
        .ok_or_else(|| "method must be an array".to_string())?;
    let mut by_name: HashMap<String, usize> = HashMap::new();
    for (i, row) in arr.iter().enumerate() {
        if let Some(t) = row.as_table() {
            if let Some(n) = t.get("name").and_then(TomlValue::as_str) {
                by_name.insert(n.to_string(), i);
            }
        }
    }
    for met in methods {
        let v = method_to_toml(met);
        if let Some(&idx) = by_name.get(&met.name) {
            arr[idx] = v;
        } else {
            by_name.insert(met.name.clone(), arr.len());
            arr.push(v);
        }
    }
    Ok(())
}

fn method_to_toml(met: &MethodRow) -> TomlValue {
    let mut t = toml::map::Map::new();
    t.insert("name".to_string(), TomlValue::String(met.name.clone()));
    if let Some(r) = &met.rust {
        t.insert("rust".to_string(), TomlValue::String(r.clone()));
    }
    let args: Vec<TomlValue> = met.args.iter().cloned().map(TomlValue::String).collect();
    t.insert("args".to_string(), TomlValue::Array(args));
    t.insert(
        "returns".to_string(),
        TomlValue::String(met.returns.clone()),
    );
    TomlValue::Table(t)
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;

    #[test]
    fn extract_minimal_fixture() {
        let raw = include_str!("../tests/fixtures/rustdoc_minimal.json");
        let v: Value = serde_json::from_str(raw).unwrap();
        let binds = extract_bindings_from_rustdoc_json(&v, "demo").unwrap();
        assert_eq!(binds.len(), 1);
        let b = &binds[0];
        assert_eq!(b.type_name, "Demo");
        assert_eq!(b.rust_type, "demo::Demo");
        assert!(b.new.is_some(), "parse -> new");
        let mnames: HashSet<_> = b.methods.iter().map(|m| m.name.as_str()).collect();
        assert!(mnames.contains("path"));
    }
}
