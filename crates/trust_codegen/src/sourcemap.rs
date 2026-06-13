// §设计文档 §7.2 / constraints §7.2: Source Map 双向映射
//!
//! 记录 Trust 源码位置 ↔ 生成 Rust 源码位置的对应关系。
//! 支持回退模式 `// @trust:` 注释。

use std::collections::HashMap;
use trust_parser::ast::Span;

/// §设计文档 §7.2: Trust 源码 → Rust 源码 双向映射
#[derive(Debug, Clone, Default)]
pub struct SourceMapping {
    /// (Trust行, Trust列) → (Rust行, Rust列)
    pub trust_to_rust: HashMap<(u32, u32), (u32, u32)>,
    /// (Rust行, Rust列) → (Trust文件, Trust行, Trust列)
    pub rust_to_trust: HashMap<(u32, u32), (String, u32, u32)>,
}

impl SourceMapping {
    /// §7.2: 创建空映射表
    pub fn new() -> Self {
        SourceMapping {
            trust_to_rust: HashMap::new(),
            rust_to_trust: HashMap::new(),
        }
    }

    /// 插入一条 Trust → Rust 映射
    pub fn insert(&mut self, span: &Span, rust_line: u32, rust_col: u32) {
        let trust_key = (span.line_start, span.col_start);
        let rust_val = (rust_line, rust_col);
        self.trust_to_rust.insert(trust_key, rust_val);
        self.rust_to_trust
            .insert((rust_line, rust_col), (span.file.clone(), span.line_start, span.col_start));
    }

    /// 查询 Trust 位置对应的 Rust 位置
    pub fn lookup_trust(&self, line: u32, col: u32) -> Option<(u32, u32)> {
        self.trust_to_rust.get(&(line, col)).copied()
    }

    /// 查询 Rust 位置对应的 Trust 源位置
    pub fn lookup_rust(&self, line: u32, col: u32) -> Option<&(String, u32, u32)> {
        self.rust_to_trust.get(&(line, col))
    }

    /// 生成回退注释
    pub fn fallback_comment(file: &str, line: u32, col: u32) -> String {
        format!("// @trust: {}:{}:{}\n", file, line, col)
    }
}
