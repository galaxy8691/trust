//! Trust TIR crate — Phase 1.4
//!
//! 编译管线第三站：消费 `trust_hir` 产出的带类型标注的 HIR，
//! 执行 HIR→TIR 降级 + 移动语义检查 + 借用检查 + 区域推断，
//! 产出所有权注解完备的 TIR 控制流图。
//!
//! §design-constraints §3.2: 本 crate 零 unsafe。
//! §design-constraints §5.4: 所有 pub 函数必须有 doctest（P0 约束）。
//! §design-constraints §6.2 / §8.3: 错误信息映射到 Trust 源码变量名+行列号。
//!
//! # 模块
//!
//! - `tir` — TIR 节点定义 + HIR→TIR 降级（控制流→基本块、表达式→语句、闭包捕获提升）
//! - `moveck` — 移动语义分析 + Copy 类型判定 + 错误映射
//! - `borrowck` — 借用检查 + 三模式参数验证 + 区域推断

pub mod borrowck;
pub mod moveck;
pub mod tir;
