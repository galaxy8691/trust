//! Trust 导入解析 — import 路径解析

/// 解析 import 路径为实际文件路径
pub fn resolve_import_path(import_path: &str, current_file: &str) -> Option<String> {
    if import_path.starts_with("std::") {
        // 标准库路径 — Phase 2+
        None
    } else if import_path.starts_with('/') {
        // 绝对路径（相对项目根）
        let mut p = import_path.to_string();
        if !p.ends_with(".trust") { p.push_str(".trust"); }
        Some(p)
    } else if import_path.starts_with("./") || import_path.starts_with("../") {
        let current_dir = std::path::Path::new(current_file).parent()?;
        // 去掉 ./ 前缀再 join，避免产生 src/./math 路径
        let clean_path = import_path.strip_prefix("./").unwrap_or(import_path);
        let resolved = current_dir.join(clean_path);
        let mut p = resolved.to_string_lossy().to_string();
        if !p.ends_with(".trust") { p.push_str(".trust"); }
        Some(p)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_relative_import_returns_correct_path() {
        let result = resolve_import_path("./math", "src/main.trust");
        assert_eq!(result, Some("src/math.trust".into()));
    }

    #[test]
    fn resolve_missing_file_returns_none_for_std() {
        let result = resolve_import_path("std::collections", "main.trust");
        assert!(result.is_none()); // Phase 2+
    }

    #[test]
    fn resolve_absolute_import_appends_extension() {
        let result = resolve_import_path("/lib/util", "src/main.trust");
        assert_eq!(result, Some("/lib/util.trust".into()));
    }

    #[test]
    fn resolve_parent_import_handles_dotdot_path() {
        let result = resolve_import_path("../lib/math", "src/sub/main.trust");
        let path = result.expect("../ import should resolve (dotdot branch)");
        assert!(path.contains("lib/math.trust"), "path should contain lib/math.trust, got: {path}");
        assert!(path.contains(".."), "Phase 1 does not canonicalize '..', got: {path}");
    }
}
