//! Trust HIR crate — Phase 1.3
//!
//! 编译管线第二站：消费 `trust_parser` 产出的 AST，
//! 执行名称解析 + 类型检查，产出带类型标注的 HIR。
//!
//! §设计文档 §3.1: HIR 是 AST→TIR 的中间表示，携带类型信息。
//! §设计文档 §3.2: 名称解析将符号引用绑定到声明。
//! §design-constraints §3.2: 本 crate 零 unsafe。
//!
//! # 模块
//!
//! - `hir` — HIR 节点定义（HirProgram、HirStmt、HirExpr、HirType 等）
//! - `name_res` — AST→HIR 降级 + 跨文件名称解析 + 作用域构造
//! - `typeck` — 类型检查器（二元运算类型 / as 转换 / 函数签名验证）

pub mod hir;
pub mod name_res;
pub mod typeck;
