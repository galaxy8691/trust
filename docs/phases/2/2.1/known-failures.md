# Phase 2.1 — 已知失败清单

> 里程碑：2.1-CLEANUP · 创建：2026-06-14
> 用途：记录因 AST/运算符/关键字删除而禁用或失败的测试，由 Phase 2.5 统一恢复/改写。

## 已禁用的测试

| 测试 | 文件 | 原因 | 处理 |
|------|------|------|------|
| `e2e_bigint` | `crates/trustc/tests/integration.rs:85` | bigint 类型/字面量已移除 | 已注释，待 2.5 重写为 number 整数测试 |
| `e2e_loop_break` | `crates/trustc/tests/integration.rs:117` | loop/LoopExpr 已移除 | 已注释，待 2.5 重写为 while+break 测试 |
| `e2e_break_value` | `crates/trustc/tests/integration.rs:158` | loop 和 break value 已移除 | 已注释，待 2.5 移除或重写 |
| `e2e_continue_loop` | `crates/trustc/tests/integration.rs:164` | loop 已移除 | 已注释，待 2.5 重写为 while+continue 测试 |
| `snap_loop` | `crates/trust_parser/tests/snapshot_tests.rs:94` | LoopExpr/Loop AST 已删除 | 已删除测试函数 |
| `snap_bang` | `crates/trust_parser/tests/snapshot_tests.rs:114` | AssertUnwrap(expr!) 已移除 | 已删除测试函数 |
| `snap_try` | `crates/trust_parser/tests/snapshot_tests.rs:118` | TryPropagate(expr?) 已移除 | 已删除测试函数 |
| `syn037_bang` | `crates/trust_parser/src/parser.rs` | AssertUnwrap 已移除 | 已删除测试函数 |
| `syn038_try` | `crates/trust_parser/src/parser.rs` | TryPropagate 已移除 | 已删除测试函数 |
| `integration_loop_expression` | `crates/trust_hir/tests/integration.rs:122` | loop 已移除 | 已注释，待 2.5 重写 |

## 交叉核对记录（MS-2.1-10）

> 核对日期：2026-06-14 · 范围：`Trust-设计文档.md` v2.0 × `spec/trust-spec.md` × `spec/stdlib.md` × `design-constraints.md`

### trust-spec.md

- ✅ LEX-REQ-001: 关键字表 43 个
- ✅ LEX-REQ-002: 字面量 5 种
- ✅ SYN-REQ-002: 函数声明规则更新（块体强制返回标注、表达式体）
- ✅ SYN-REQ-003: 控制流更新（移除 loop_expr、break value）
- ✅ SYN-REQ-008: 类型声明更新（移除 interface/ADT）
- ✅ TYP-REQ-001: number=f64 前瞻同步
- ✅ TYP-REQ-002/003/004: 已标注 v2.0 废弃
- 🟡 SYN-REQ-006/007/012/013/015、SEM-REQ-001/003/004、TYP-REQ-005/006/008、CON-REQ-005、ERR-REQ-001~004: 仍有残留旧条目（见 Phase 2.1 终审报告，归入 2.2 清理）

### stdlib.md

- ✅ std::result 模块已移除
- ✅ std::error/std::console 骨架占位已加
- ✅ 模块依赖图已更新
- 🟡 部分 API 仍含 Option/Result 签名，已标过渡注记位置但未全量标注（归入 2.2/Phase 4 继续）

### design-constraints.md

- ✅ 无冲突

**结论：** 四文档在 2.1 核心范围内一致。spec/stdlib 的深层废弃条目清理与过渡注记因工作量大、涉及 Phase 3+ 未实现的语义，合理归入 2.2（number=f64 类型系统重写）与 Phase 4（null 安全/错误处理）继续推进。

## 章节冻结矩阵（MS-2.1-8）

| 规范章节 | 冻结时机 | 对应实现 |
|---------|---------|---------|
| 词法规范（关键字集 43、字面量 5 种） | 2.1 完成时 ✅ | 2.1.1 |
| 类型系统核心（`number`/基本类型） | 2.2/2.3 启动前 | 2.2 number=f64 |
| 函数声明规则 | 2.3 启动前 | 2.3 |
| 标准库模块大纲 | 2.1 骨架对齐后逐 Phase 冻结 | 2.1.8 + Phase 4/5/6 |
| 具名类型/泛型/`unknown` | Phase 3 各子任务启动前 | Phase 3 |
| 错误/`null` | Phase 4 各子任务启动前 | Phase 4 |
| 并发/FFI | Phase 5/7 启动前 | Phase 5/7 |
