//! Trust 运行时库 —— ferro_rt
//!
//! 提供 Trust 标准库到 Rust 标准库的映射。
//!
//! Phase 1 仅包含 `console` 模块。完整运行时（Channel/shared/spawn/join）
//! 在 Phase 4 实现。

pub mod console;
