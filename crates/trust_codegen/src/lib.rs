// §设计文档 §7: TIR → Rust 源码生成
//!
//! `trust_codegen` 是编译管线的第四站，消费 `trust_tir` 产出的所有权注解完备的 TIR，
//! 机械映射为可通过 `rustc` 编译的 Rust 源码。
//!
//! # 模块
//!
//! - `codegen` — §7.1: 主代码生成器（参数映射、函数签名、TirOp→Rust语句、控制流重构）
//! - `sourcemap` — §7.2: Source Map 双向映射 + 回退注释
//! - `runtime` — §7.3: ferro_rt 运行时库 API 映射

pub mod codegen;
pub mod runtime;
pub mod sourcemap;
