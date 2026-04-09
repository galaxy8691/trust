//! 简化版 `tsconfig`：`extends`、`files`、`include` / `exclude`（glob），无 npm / `paths`。
//!
//! 合并规则：自根向叶浅覆盖——后出现的配置若提供 `files` / `include` / `exclude` 键则整项替换前一值；
//! `exclude` 在链上**累积**（每层定义的 exclude 均参与过滤）。路径相对**书写该字段的配置文件**所在目录。
//! 若最终 `files` 非空则只用 `files`；否则展开 `include`；二者皆空则报错。`include` 模式下匹配文件按路径排序，
//! 入口为排序后第一个（请用 `files` 显式控制入口顺序）。

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use glob::glob;
use serde::Deserialize;

#[derive(Debug, Deserialize, Default)]
struct TsConfigJson {
    extends: Option<String>,
    #[serde(default)]
    files: Option<Vec<String>>,
    #[serde(default)]
    include: Option<Vec<String>>,
    #[serde(default)]
    exclude: Option<Vec<String>>,
}

#[derive(Clone, Default)]
struct Merged {
    /// Last layer that set `files` (key present in JSON).
    files: Option<(PathBuf, Vec<String>)>,
    /// Last layer that set `include` (key present).
    include: Option<(PathBuf, Vec<String>)>,
    /// Every layer's exclude patterns with that config's directory.
    exclude_layers: Vec<(PathBuf, Vec<String>)>,
}

fn parse_config_file(path: &Path) -> Result<TsConfigJson, String> {
    let text = fs::read_to_string(path).map_err(|e| e.to_string())?;
    serde_json::from_str(&text)
        .map_err(|e| format!("invalid JSON in tsconfig `{}`: {e}", path.display()))
}

/// Returns chain from **root (extends base)** to **leaf** (the file at `path`).
fn load_extends_chain(
    path: &Path,
    visiting: &mut Vec<PathBuf>,
) -> Result<Vec<(PathBuf, TsConfigJson)>, String> {
    let canon = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    if visiting.iter().any(|v| v == &canon) {
        return Err(format!(
            "circular `extends` in tsconfig involving `{}`",
            path.display()
        ));
    }
    visiting.push(canon);

    let cfg_dir = path
        .parent()
        .ok_or_else(|| format!("invalid tsconfig path (no parent): `{}`", path.display()))?
        .to_path_buf();
    let cfg = parse_config_file(path)?;

    let mut out = Vec::new();
    if let Some(ext) = &cfg.extends {
        let parent_path = cfg_dir.join(ext);
        let parent_path = if parent_path.exists() {
            parent_path
        } else {
            return Err(format!(
                "tsconfig extends `{}` not found (from `{}`)",
                ext,
                path.display()
            ));
        };
        out.extend(load_extends_chain(&parent_path, visiting)?);
    }

    out.push((cfg_dir, cfg));
    visiting.pop();
    Ok(out)
}

fn merge_chain(chain: &[(PathBuf, TsConfigJson)]) -> Merged {
    let mut m = Merged::default();
    for (dir, cfg) in chain {
        if cfg.files.is_some() {
            m.files = Some((dir.clone(), cfg.files.clone().unwrap_or_default()));
        }
        if cfg.include.is_some() {
            m.include = Some((dir.clone(), cfg.include.clone().unwrap_or_default()));
        }
        if let Some(ex) = &cfg.exclude {
            if !ex.is_empty() {
                m.exclude_layers.push((dir.clone(), ex.clone()));
            }
        }
    }
    m
}

fn expand_exclude_set(
    exclude_layers: &[(PathBuf, Vec<String>)],
) -> Result<HashSet<PathBuf>, String> {
    let mut excluded = HashSet::new();
    for (base, patterns) in exclude_layers {
        for pat in patterns {
            let full = base.join(pat);
            let g = full.to_string_lossy().to_string();
            for entry in glob(&g).map_err(|e| format!("invalid exclude glob `{g}`: {e}"))? {
                let p = entry.map_err(|e| e.to_string())?;
                if p.is_file() {
                    let c = p.canonicalize().unwrap_or(p);
                    excluded.insert(c);
                }
            }
        }
    }
    Ok(excluded)
}

fn resolve_ts_files(merged: Merged) -> Result<Vec<PathBuf>, String> {
    let excluded = expand_exclude_set(&merged.exclude_layers)?;

    let use_files = merged
        .files
        .as_ref()
        .map(|(_, v)| !v.is_empty())
        .unwrap_or(false);

    let mut paths: Vec<PathBuf> = if use_files {
        let (dir, rels) = merged.files.as_ref().unwrap();
        let mut v = Vec::new();
        for rel in rels {
            let p = dir.join(rel);
            if !p.exists() {
                return Err(format!(
                    "tsconfig `files` entry does not exist: `{}`",
                    p.display()
                ));
            }
            if !p.is_file() {
                return Err(format!(
                    "tsconfig `files` entry is not a file: `{}`",
                    p.display()
                ));
            }
            if p.extension().and_then(|e| e.to_str()) != Some("ts") {
                return Err(format!(
                    "tsconfig `files` must list `.ts` files, got `{}`",
                    p.display()
                ));
            }
            v.push(p.canonicalize().unwrap_or(p));
        }
        v
    } else {
        let Some((dir, patterns)) = merged.include else {
            return Err(
                "tsconfig must set non-empty `files` or `include` after merging `extends`"
                    .to_string(),
            );
        };
        if patterns.is_empty() {
            return Err(
                "tsconfig must set non-empty `files` or `include` after merging `extends`"
                    .to_string(),
            );
        }
        let mut set: HashSet<PathBuf> = HashSet::new();
        for pat in &patterns {
            let full = dir.join(pat);
            let g = full.to_string_lossy().to_string();
            for entry in glob(&g).map_err(|e| format!("invalid include glob `{g}`: {e}"))? {
                let p = entry.map_err(|e| e.to_string())?;
                if p.is_file() && p.extension().and_then(|e| e.to_str()) == Some("ts") {
                    set.insert(p.canonicalize().unwrap_or(p));
                }
            }
        }
        let mut v: Vec<PathBuf> = set.into_iter().collect();
        v.sort();
        v
    };

    paths.retain(|p| !excluded.contains(p));

    if paths.is_empty() {
        return Err(
            "tsconfig produced an empty file list (check `files` / `include` / `exclude`)"
                .to_string(),
        );
    }

    Ok(paths)
}

/// Resolve all root `.ts` paths for `--project`: ordered list, first is compile **entry**.
pub(crate) fn resolve_project_ts_roots(tsconfig_path: &Path) -> Result<Vec<PathBuf>, String> {
    let mut visiting = Vec::new();
    let chain = load_extends_chain(tsconfig_path, &mut visiting)?;
    let merged = merge_chain(&chain);
    resolve_ts_files(merged)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn extends_merges_include_and_child_overrides_files() {
        let dir = tempdir().unwrap();
        let base = dir.path().join("base.json");
        let leaf = dir.path().join("tsconfig.json");
        fs::write(&base, r#"{"include": ["should_not_be_used_alone.ts"]}"#).unwrap();
        fs::write(
            dir.path().join("a.ts"),
            "export function a(): number { return 1; }\n",
        )
        .unwrap();
        fs::write(
            dir.path().join("b.ts"),
            "export function b(): number { return 2; }\n",
        )
        .unwrap();
        fs::write(
            &leaf,
            r#"{ "extends": "./base.json", "files": ["a.ts", "b.ts"] }"#,
        )
        .unwrap();
        let roots = resolve_project_ts_roots(&leaf).unwrap();
        assert_eq!(roots.len(), 2);
        assert!(roots[0].ends_with("a.ts"));
        assert!(roots[1].ends_with("b.ts"));
    }

    #[test]
    fn include_glob_sorted_entry_is_first_sorted() {
        let dir = tempdir().unwrap();
        let cfg = dir.path().join("tsconfig.json");
        fs::write(
            dir.path().join("z.ts"),
            "export function z(): number { return 1; }\n",
        )
        .unwrap();
        fs::write(
            dir.path().join("a.ts"),
            "export function a(): number { return 0; }\n",
        )
        .unwrap();
        fs::write(&cfg, r#"{"include": ["*.ts"]}"#).unwrap();
        let roots = resolve_project_ts_roots(&cfg).unwrap();
        assert_eq!(roots.len(), 2);
        assert!(
            roots[0].file_name().unwrap() == "a.ts",
            "expected lexicographic first as entry"
        );
    }

    #[test]
    fn exclude_removes_matching_files() {
        let dir = tempdir().unwrap();
        let cfg = dir.path().join("tsconfig.json");
        fs::write(
            dir.path().join("keep.ts"),
            "export function main(): number { return 0; }\n",
        )
        .unwrap();
        fs::write(
            dir.path().join("skip.ts"),
            "export function x(): number { return 1; }\n",
        )
        .unwrap();
        fs::write(
            &cfg,
            r#"{"files": ["keep.ts", "skip.ts"], "exclude": ["skip.ts"]}"#,
        )
        .unwrap();
        let roots = resolve_project_ts_roots(&cfg).unwrap();
        assert_eq!(roots.len(), 1);
        assert!(roots[0].ends_with("keep.ts"));
    }

    #[test]
    fn circular_extends_errors() {
        let dir = tempdir().unwrap();
        let a = dir.path().join("a.json");
        let b = dir.path().join("b.json");
        fs::write(&a, r#"{"extends": "./b.json", "files": ["x.ts"]}"#).unwrap();
        fs::write(&b, r#"{"extends": "./a.json", "files": ["x.ts"]}"#).unwrap();
        let e = resolve_project_ts_roots(&a).unwrap_err();
        assert!(
            e.contains("circular") || e.contains("extends"),
            "unexpected: {e}"
        );
    }
}
