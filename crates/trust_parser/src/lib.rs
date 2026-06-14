#![allow(dead_code)]

// trust_parser/src/lib.rs
// Phase 1.2 — Trust parser crate
//
// §LEX-REQ-001: 关键字识别
// §SYN-REQ-001: 变量声明解析

pub mod ast;
pub mod lexer;
pub mod module_graph;
pub mod parser;
pub mod resolve_imports;
