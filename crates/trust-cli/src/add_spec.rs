//! `trust add` 路径解析：`crate::Type`、`crate::Type::item`、`crate::*`。

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum ParsedAddSpec {
    /// `url::*` — rustdoc 批量绑定
    Wildcard { crate_name: String },
    /// `url::Url` — 仅类型（不写 `new`）
    TypeOnly {
        crate_name: String,
        type_name: String,
        /// 如 `url::Url`
        rust_type_path: String,
    },
    /// `url::Url::parse` 且无 `--returns` — 构造函数 `new`
    Constructor {
        crate_name: String,
        type_name: String,
        /// 类型路径，如 `url::Url`
        rust_type_path: String,
        /// 完整路径，如 `url::Url::parse`
        rust_ctor_path: String,
    },
    /// 带 `--returns` 时为方法（最后一段为方法名）
    Method {
        crate_name: String,
        type_name: String,
        method_name: String,
        /// 类型路径，如 `url::Url`
        rust_type_path: String,
        /// 完整路径，如 `url::Url::join`
        rust_method_path: String,
        returns: String,
        args: Vec<String>,
    },
}

fn split_parts(spec: &str) -> Vec<String> {
    spec.split("::")
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect()
}

fn parse_method_args(s: &str) -> Result<Vec<String>, String> {
    if s.trim().is_empty() {
        return Ok(Vec::new());
    }
    let mut out = Vec::new();
    for part in s.split(',') {
        let t = part.trim();
        if t.is_empty() {
            return Err("empty entry in --args (check commas)".to_string());
        }
        validate_trust_binding_type(t)?;
        out.push(t.to_string());
    }
    Ok(out)
}

fn validate_trust_binding_type(t: &str) -> Result<(), String> {
    match t {
        "string" | "number" | "boolean" | "void" => Ok(()),
        _ => Err(format!(
            "invalid trust binding type `{t}`: expected string|number|boolean|void"
        )),
    }
}

fn validate_returns(t: &str) -> Result<(), String> {
    validate_trust_binding_type(t)
}

/// 解析 `trust add` 的位置参数与 `--returns` / `--args`。
pub(crate) fn parse_add_spec(
    spec: &str,
    returns: Option<&str>,
    args: Option<&str>,
) -> Result<ParsedAddSpec, String> {
    let parts = split_parts(spec);
    if parts.is_empty() {
        return Err("empty rust path".to_string());
    }

    if parts.len() == 2 && parts[1] == "*" {
        if returns.is_some() || args.is_some() {
            return Err("`crate::*` cannot be combined with --returns or --args".to_string());
        }
        return Ok(ParsedAddSpec::Wildcard {
            crate_name: parts[0].clone(),
        });
    }

    if let Some(r) = returns {
        validate_returns(r)?;
    }
    if let Some(a) = args {
        if returns.is_none() {
            return Err("--args requires --returns (method binding)".to_string());
        }
        let _ = parse_method_args(a)?;
    }

    match parts.len() {
        1 => Err(format!(
            "invalid rust path `{spec}`: expected `crate::Type`, `crate::Type::item`, or `crate::*`"
        )),
        2 => {
            if returns.is_some() || args.is_some() {
                return Err(
                    "`crate::Type` cannot be combined with --returns / --args; use `crate::Type::method`"
                        .to_string(),
                );
            }
            Ok(ParsedAddSpec::TypeOnly {
                crate_name: parts[0].clone(),
                type_name: parts[1].clone(),
                rust_type_path: parts.join("::"),
            })
        }
        n if n >= 3 => {
            let crate_name = parts[0].clone();
            let type_name = parts[n - 2].clone();
            let last = parts[n - 1].clone();
            let rust_type_path = parts[..n - 1].join("::");
            let full_item_path = parts.join("::");

            if let Some(ret) = returns {
                let arg_vec = parse_method_args(args.unwrap_or(""))?;
                Ok(ParsedAddSpec::Method {
                    crate_name,
                    type_name,
                    method_name: last,
                    rust_type_path,
                    rust_method_path: full_item_path,
                    returns: ret.to_string(),
                    args: arg_vec,
                })
            } else {
                if args.is_some() {
                    return Err("--args is only valid with --returns (method binding)".to_string());
                }
                Ok(ParsedAddSpec::Constructor {
                    crate_name,
                    type_name,
                    rust_type_path,
                    rust_ctor_path: full_item_path,
                })
            }
        }
        _ => unreachable!(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn type_only() {
        let p = parse_add_spec("url::Url", None, None).unwrap();
        assert!(
            matches!(p, ParsedAddSpec::TypeOnly { ref crate_name, ref type_name, .. } if crate_name == "url" && type_name == "Url")
        );
    }

    #[test]
    fn ctor() {
        let p = parse_add_spec("url::Url::parse", None, None).unwrap();
        match p {
            ParsedAddSpec::Constructor { rust_ctor_path, .. } => {
                assert_eq!(rust_ctor_path, "url::Url::parse");
            }
            _ => panic!("expected ctor"),
        }
    }

    #[test]
    fn method_with_flags() {
        let p = parse_add_spec("url::Url::join", Some("string"), Some("string")).unwrap();
        match p {
            ParsedAddSpec::Method {
                method_name,
                rust_method_path,
                returns,
                args,
                ..
            } => {
                assert_eq!(method_name, "join");
                assert_eq!(rust_method_path, "url::Url::join");
                assert_eq!(returns, "string");
                assert_eq!(args, vec!["string".to_string()]);
            }
            _ => panic!("expected method"),
        }
    }

    #[test]
    fn wildcard() {
        let p = parse_add_spec("regex::*", None, None).unwrap();
        assert!(matches!(p, ParsedAddSpec::Wildcard { ref crate_name } if crate_name == "regex"));
    }
}
