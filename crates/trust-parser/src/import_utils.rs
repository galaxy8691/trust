use std::path::{Path, PathBuf};

use swc_ecma_ast::{ImportDecl, ImportSpecifier, ModuleExportName, Str};
use trust_manifest::TrustManifest;

use crate::ParseError;

/// `import` 说明符解析结果：相对 `.ts` 文件或 **Trust.toml 声明的 Rust crate**（如 `regex`）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModuleSpecifierResolution {
    Relative(PathBuf),
    RustCrate(String),
    BuiltinStd,
}

/// 解析 `import ... from "…"`：相对路径，或 `trust` 中 `[dependencies]` 的键名。
pub fn resolve_module_specifier(
    file: &Path,
    imp: &ImportDecl,
    trust: Option<&TrustManifest>,
) -> Result<ModuleSpecifierResolution, ParseError> {
    // `import type` 现在被支持，用于类型导入
    let raw = imp.src.value.to_string_lossy();
    let raw = raw.trim_matches(|c| c == '"' || c == '\'');
    if raw.starts_with("./") || raw.starts_with("../") {
        return Ok(ModuleSpecifierResolution::Relative(
            resolve_relative_ts_path(file, &imp.src)?,
        ));
    }
    if raw == "std" {
        return Ok(ModuleSpecifierResolution::BuiltinStd);
    }
    if let Some(t) = trust {
        if t.has_dependency(raw) {
            return Ok(ModuleSpecifierResolution::RustCrate(raw.to_string()));
        }
    }
    Err(ParseError::Message(format!(
        "only relative paths (`./` / `../`) or a Rust crate name from Trust.toml `[dependencies]` are supported as import specifiers, got `{raw}`"
    )))
}

/// Resolve `./` / `../` module specifier (import or re-export).
pub(crate) fn resolve_relative_ts_path(file: &Path, src: &Str) -> Result<PathBuf, ParseError> {
    let raw = src.value.to_string_lossy();
    let raw = raw.trim_matches(|c| c == '"' || c == '\'');
    if !(raw.starts_with("./") || raw.starts_with("../")) {
        return Err(ParseError::Message(format!(
            "only relative paths like `./file.ts` are supported, got `{raw}`"
        )));
    }
    let dir = file.parent().ok_or_else(|| {
        ParseError::Message(format!("cannot resolve parent of `{}`", file.display()))
    })?;
    Ok(dir.join(raw))
}

pub(crate) fn resolve_supported_import_path(
    file: &Path,
    imp: &ImportDecl,
) -> Result<PathBuf, ParseError> {
    match resolve_module_specifier(file, imp, None)? {
        ModuleSpecifierResolution::Relative(p) => Ok(p),
        ModuleSpecifierResolution::RustCrate(name) => Err(ParseError::Message(format!(
            "Rust crate import `{name}` requires a Trust.toml next to the project; use `trust` driver path that loads Trust.toml"
        ))),
        ModuleSpecifierResolution::BuiltinStd => Err(ParseError::Message(
            "builtin std import is virtual and has no filesystem path".to_string(),
        )),
    }
}

/// 与 [`resolve_supported_import_path`] 相同，但在提供 `trust` 时允许 `from \"crate\"`。
pub fn resolve_supported_import_path_with_trust(
    file: &Path,
    imp: &ImportDecl,
    trust: Option<&TrustManifest>,
) -> Result<ModuleSpecifierResolution, ParseError> {
    resolve_module_specifier(file, imp, trust)
}

pub(crate) fn named_import_target(spec: &ImportSpecifier) -> Result<String, ParseError> {
    match spec {
        ImportSpecifier::Named(named) => {
            // type-only import specifiers 现在被支持
            let want = match &named.imported {
                Some(ModuleExportName::Ident(id)) => id.sym.to_string(),
                Some(ModuleExportName::Str(s)) => s.value.to_string_lossy().into_owned(),
                None => named.local.sym.to_string(),
            };
            Ok(want)
        }
        ImportSpecifier::Default(d) => {
            if d.local.sym != "main" {
                return Err(ParseError::Message(
                    "default import must use the binding name `main` (trust maps it to the module default export)"
                        .to_string(),
                ));
            }
            Ok("main".to_string())
        }
        ImportSpecifier::Namespace(_) => Err(ParseError::Message(
            "only named imports `{ foo }` and `import main from \"./file.ts\"` are supported"
                .to_string(),
        )),
    }
}
