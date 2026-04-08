use std::path::{Path, PathBuf};

use swc_ecma_ast::{ImportDecl, ImportSpecifier, ModuleExportName};

use crate::ParseError;

pub(crate) fn resolve_supported_import_path(
    file: &Path,
    imp: &ImportDecl,
) -> Result<PathBuf, ParseError> {
    if imp.type_only {
        return Err(ParseError::Message(
            "`import type` is not supported for import resolution".to_string(),
        ));
    }
    let raw = imp.src.value.to_string_lossy();
    let raw = raw.trim_matches(|c| c == '"' || c == '\'');
    if !(raw.starts_with("./") || raw.starts_with("../")) {
        return Err(ParseError::Message(format!(
            "only relative imports like `./file.ts` are supported, got `{raw}`"
        )));
    }
    let dir = file.parent().ok_or_else(|| {
        ParseError::Message(format!("cannot resolve parent of `{}`", file.display()))
    })?;
    Ok(dir.join(raw))
}

pub(crate) fn named_import_target(spec: &ImportSpecifier) -> Result<String, ParseError> {
    match spec {
        ImportSpecifier::Named(named) => {
            if named.is_type_only {
                return Err(ParseError::Message(
                    "type-only import specifiers are not supported".to_string(),
                ));
            }
            let want = match &named.imported {
                Some(ModuleExportName::Ident(id)) => id.sym.to_string(),
                Some(ModuleExportName::Str(s)) => s.value.to_string_lossy().into_owned(),
                None => named.local.sym.to_string(),
            };
            Ok(want)
        }
        ImportSpecifier::Default(_) | ImportSpecifier::Namespace(_) => Err(ParseError::Message(
            "only named imports `{ foo }` are supported".to_string(),
        )),
    }
}
