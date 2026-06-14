# Phase 2.1 — 已知失败清单

> 里程碑：2.1-CLEANUP · 创建：2026-06-14
> 用途：记录因 AST/运算符/关键字删除而禁用或失败的测试，由 Phase 2.5 统一恢复/改写。

## 已禁用的测试（编译失败——引用了已删除的 AST 节点/运算符）

| 测试 | 文件 | 原因 | 处理 |
|------|------|------|------|
| `e2e_bigint` | `crates/trustc/tests/integration.rs:85` | bigint 类型/字面量已移除 | 禁用 `#[test]`，待 2.5 重写为 number 整数测试 |
| `e2e_loop_break` | `crates/trustc/tests/integration.rs:117` | loop/LoopExpr 已移除 | 禁用 `#[test]`，待 2.5 重写为 while+break 测试 |
| `e2e_break_value` | `crates/trustc/tests/integration.rs:158` | loop 和 break value 已移除 | 禁用 `#[test]`，待 2.5 移除或重写 |
| `e2e_continue_loop` | `crates/trustc/tests/integration.rs:164` | loop 已移除 | 禁用 `#[test]`，待 2.5 重写为 while+continue 测试 |
| `snap_loop` | `crates/trust_parser/tests/snapshot_tests.rs:94` | LoopExpr/Loop AST 已删除 | 已删除测试函数 |
| `snap_bang` | `crates/trust_parser/tests/snapshot_tests.rs:114` | AssertUnwrap(expr!) 已移除 | 已删除测试函数 |
| `snap_try` | `crates/trust_parser/tests/snapshot_tests.rs:118` | TryPropagate(expr?) 已移除 | 已删除测试函数 |
| `syn037_bang` | `crates/trust_parser/src/parser.rs` | AssertUnwrap 已移除 | 已删除测试函数 |
| `syn038_try` | `crates/trust_parser/src/parser.rs` | TryPropagate 已移除 | 已删除测试函数 |

## 运行时失败（待 2.5 验证）

（编译通过后运行 `cargo test --workspace` 收集）

## 交叉核对记录（MS-2.1-9）

（待 Step 6 完成时填写）

## 章节冻结矩阵（MS-2.1-8）

| 规范章节 | 冻结时机 | 对应实现 |
|---------|---------|---------|
| 词法规范（关键字集、字面量） | 2.1 完成前 | 2.1.1 关键字重核 |
| 类型系统核心（`number`/基本类型） | 2.2/2.3 启动前 | 2.2 number=f64 |
| 函数声明规则 | 2.3 启动前 | 2.3 |
| 具名类型/泛型/`unknown` | Phase 3 各子任务启动前 | Phase 3 |
| 错误/`null` | Phase 4 各子任务启动前 | Phase 4 |
| 并发/FFI | Phase 5/7 启动前 | Phase 5/7 |
