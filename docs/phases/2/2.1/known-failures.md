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

## 交叉核对记录（MS-2.1-10）

> 核对日期：2026-06-14 · 范围：`Trust-设计文档.md` v2.0 × `spec/trust-spec.md` × `spec/stdlib.md` × `design-constraints.md`

### trust-spec.md 对齐

- ✅ LEX-REQ-001：关键字表 43 个，与设计 §2.2/§14 一致。`interface`/`impl`/`select`/`loop`/`bigint`/`undefined`/`None`/`Some`/`Ok`/`Err`/`Rc`/`Arc`/`Weak`/`Box`/`dyn`/`extends` 已移除
- ✅ LEX-REQ-002：字面量 5 种，BigInt `Nn` 后缀已移除。`number`=f64 已标注
- ✅ AC-LEX-004：`throws` 替代旧 `Result<T,E>` 返回标注
- ✅ `fn` vs `function` 设计决策保留（v2.0 仍适用）

### stdlib.md 对齐

- ✅ `std::result` → `std::error` + `std::console`，旧 Option/Result API 已移除
- ✅ `std::rc`（Box/Rc/Arc/Weak）已移除——用户不接触（设计 §3.7）
- ✅ 模块依赖图更新：`error`/`console` 为新的基础模块
- ✅ FsError ADT（`type X = | ...`）替换为结构类型
- ✅ 映射表已更新，旧条目加注「已移除」
- ✅ 仍含 `Result<T,E>` 的 API（如 fs/net）加注「过渡→throws Phase 4」

### design-constraints.md 对齐

- ✅ constraints §9.2 ferro_rt API 映射表不受影响（stdlib.md 映射表独立）

### 未覆盖事项

- 🔜 `null` 安全具体语义（Phase 4）
- 🔜 `throw`/`try-catch` 穷举检查（Phase 4）
- 🔜 `unknown` + `match`（Phase 3）
- 🔜 位运算 codegen（2.2.5）

**结论：** 四文档在 2.1 范围内一致，Phase 3+ 特性已在 spec 中标注为前瞻条目。

## 章节冻结矩阵（MS-2.1-8）

| 规范章节 | 冻结时机 | 对应实现 |
|---------|---------|---------|
| 词法规范（关键字集、字面量） | 2.1 完成前 | 2.1.1 关键字重核 |
| 类型系统核心（`number`/基本类型） | 2.2/2.3 启动前 | 2.2 number=f64 |
| 函数声明规则 | 2.3 启动前 | 2.3 |
| 具名类型/泛型/`unknown` | Phase 3 各子任务启动前 | Phase 3 |
| 错误/`null` | Phase 4 各子任务启动前 | Phase 4 |
| 并发/FFI | Phase 5/7 启动前 | Phase 5/7 |
