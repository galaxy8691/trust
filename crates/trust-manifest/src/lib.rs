//! [`Trust.toml`]：为 TypeScript 入口声明 **Rust 依赖**（写入生成 crate 的 `Cargo.toml`）与 **extern 绑定**
//! （`import { T } from "crate"` 的符号与 `new` / 方法 codegen）。**不**从 Rust 源码反射 API。

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum TrustManifestError {
    #[error("io reading `{0}`: {1}")]
    Io(PathBuf, std::io::Error),
    #[error("parse Trust.toml `{0}`: {1}")]
    Toml(PathBuf, toml::de::Error),
    #[error("Trust.toml `{0}`: duplicate rust_binding for crate `{1}` type `{2}`")]
    DuplicateBinding(PathBuf, String, String),
}

/// 单条 `[[rust_binding]]`：从 `import { type_name } from \"<crate>\"` 映射到 Rust 类型与调用。
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct RustTypeBinding {
    /// Cargo 依赖键名，须与 `import ... from \"...\"` 的说明符一致（如 `regex`）。
    #[serde(rename = "crate")]
    pub crate_name: String,
    /// TS 导出名（如 `Regex`）。
    pub type_name: String,
    /// Rust 类型路径（如 `regex::Regex`）。
    pub rust_type: String,
    #[serde(default)]
    pub new: Option<RustNewBinding>,
    #[serde(default)]
    pub method: Vec<RustMethodBinding>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct RustNewBinding {
    /// 完整路径，如 `regex::Regex::new`
    pub rust: String,
    /// 为 `true` 时对 `Result` 使用 `.unwrap()`（trust 子集；失败时 panic）。
    #[serde(default)]
    pub unwrap: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct RustMethodBinding {
    pub name: String,
    /// 缺省与 `name` 相同
    #[serde(default)]
    pub rust: Option<String>,
    /// 目前支持：`string`
    #[serde(default)]
    pub args: Vec<String>,
    /// 返回：`boolean`、`number`、`string`、`void`
    pub returns: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrustManifest {
    path: PathBuf,
    /// `[dependencies]` 里的键集合（与 import 说明符一致）
    pub dependency_keys: BTreeSet<String>,
    /// `Cargo.toml` `[dependencies]` 逐行（已含换行）
    pub cargo_dependency_lines: String,
    /// `crate_name` → `type_export` → 绑定
    pub bindings_by_crate: BTreeMap<String, BTreeMap<String, RustTypeBinding>>,
}

#[derive(Debug, Deserialize)]
struct RawTrust {
    #[serde(default)]
    dependencies: Option<toml::Table>,
    #[serde(default)]
    rust_binding: Vec<RustTypeBinding>,
}

impl TrustManifest {
    /// 自 `path` 读取并解析（文件须存在）。
    pub fn load(path: &Path) -> Result<Self, TrustManifestError> {
        let text =
            fs::read_to_string(path).map_err(|e| TrustManifestError::Io(path.to_path_buf(), e))?;
        Self::parse(path.to_path_buf(), &text)
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// 合并进生成 crate 时追加在 `[dependencies]` 中（不含 `[dependencies]` 头）。
    pub fn format_cargo_dependency_lines(deps: &toml::Table) -> String {
        let mut out = String::new();
        let mut keys: Vec<_> = deps.keys().cloned().collect();
        keys.sort();
        for k in keys {
            let v = deps.get(&k).expect("key from iteration");
            let mut one = toml::map::Map::new();
            one.insert(k.clone(), v.clone());
            let s = toml::to_string(&toml::Value::Table(one)).unwrap_or_default();
            out.push_str(&s);
        }
        out
    }

    fn parse(path: PathBuf, text: &str) -> Result<Self, TrustManifestError> {
        let raw: RawTrust =
            toml::from_str(text).map_err(|e| TrustManifestError::Toml(path.clone(), e))?;
        let dependency_keys: BTreeSet<String> = raw
            .dependencies
            .as_ref()
            .map(|t| t.keys().cloned().collect())
            .unwrap_or_default();
        let cargo_dependency_lines = raw
            .dependencies
            .as_ref()
            .map(Self::format_cargo_dependency_lines)
            .unwrap_or_default();

        let mut bindings_by_crate: BTreeMap<String, BTreeMap<String, RustTypeBinding>> =
            BTreeMap::new();
        for b in raw.rust_binding {
            let ty = b.type_name.clone();
            let ck = b.crate_name.clone();
            let m = bindings_by_crate.entry(ck.clone()).or_default();
            if m.insert(ty.clone(), b).is_some() {
                return Err(TrustManifestError::DuplicateBinding(path, ck, ty));
            }
        }

        Ok(TrustManifest {
            path,
            dependency_keys,
            cargo_dependency_lines,
            bindings_by_crate,
        })
    }

    pub fn has_dependency(&self, crate_name: &str) -> bool {
        self.dependency_keys.contains(crate_name)
    }

    pub fn binding_for(&self, crate_name: &str, type_name: &str) -> Option<&RustTypeBinding> {
        self.bindings_by_crate
            .get(crate_name)
            .and_then(|m| m.get(type_name))
    }
}

/// 自入口 `.ts` 路径向上查找 `Trust.toml`（含入口所在目录）；找不到则 `None`。
pub fn discover_trust_toml(entry_ts: &Path) -> Option<PathBuf> {
    let mut dir = entry_ts.parent()?;
    loop {
        let candidate = dir.join("Trust.toml");
        if candidate.is_file() {
            return Some(candidate);
        }
        dir = dir.parent()?;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_sample_regex() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("Trust.toml");
        fs::write(
            &p,
            r#"
[dependencies]
regex = "1.10"

[[rust_binding]]
crate = "regex"
type_name = "Regex"
rust_type = "regex::Regex"
new = { rust = "regex::Regex::new", unwrap = true }
method = [
  { name = "is_match", args = ["string"], returns = "boolean" },
]
"#,
        )
        .unwrap();
        let m = TrustManifest::load(&p).unwrap();
        assert!(m.has_dependency("regex"));
        assert!(m.cargo_dependency_lines.contains("regex"));
        let b = m.binding_for("regex", "Regex").unwrap();
        assert_eq!(b.rust_type, "regex::Regex");
        assert!(b.new.as_ref().unwrap().unwrap);
        assert_eq!(b.method.len(), 1);
        assert_eq!(b.method[0].name, "is_match");
    }
}
