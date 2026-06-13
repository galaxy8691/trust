// §设计文档 §8: trust_error — 统一错误诊断基础 crate
//!
//! 零依赖，被 parser / HIR / TIR / codegen / trustc 共用。
//! 提供 `Diagnostic` 结构体 + JSON 格式化 + 修复建议引擎。
//!
//! # 模块
//!
//! - `diagnostic` — §8.1: Diagnostic + ErrorCode(19变体) + Severity + SourceSpan + FixSuggestion
//! - `json_fmt`    — §9.1.1: 手动 JSON 格式化（NDJSON）+ 字符串转义
//! - `fix_suggest` — §8.4: 启发式修复建议引擎（3 种规则）

pub mod diagnostic;
pub mod fix_suggest;
pub mod json_fmt;
