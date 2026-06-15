# Phase 2.4 四文档交叉核对

> 日期：2026-07 · 核对范围：MS-2.4-8

## 核对结论

**2.4 核对通过（有条件）**——`&mut` 解析与 `BorrowKind::Mutable` 发射、闭包 `let f = …; f(5)` HIR typeck 与 `Trust-设计文档.md` §3.4/§3.5、`OWN-REQ-003/004` 方向一致。

## 对齐项

| 文档 | 条目 | 结论 |
|------|------|------|
| `Trust-设计文档.md` §3.4 | 闭包捕获、只读默认 | TIR `captured_vars` + `ArrowFn` 提升路径一致 |
| `Trust-设计文档.md` §3.5 | `&` / `&mut` 借用规则 | borrowck `Shared/Mutable` 冲突规则一致；2.4 补 `RefMut` 发射 |
| `spec/trust-spec.md` OWN-REQ-003/004 | 借用与闭包 | 语义对齐；**一元 `*expr` 解引用 parser 未实现**（见下） |
| `spec/stdlib.md` | — | 2.4 无 stdlib API 变更 |
| `docs/design-constraints.md` §3.2 | 零 unsafe | 2.4 变更 crate 仍无 unsafe 块 |

## 已知差距（不归 2.4 阻塞）

1. **一元解引用 `*r = 2`：** `LEX-REQ-003` / trust-spec 列出 `*expr`，但 `parse_unary` 未实现前缀 `*`。2.4 验收以 `&mut x` 解析 + borrowck 路径为准；通过 `*r` 赋值归后续（或与 H-P3-07 自增运算符一并落地）。
2. **borrowck 专项测试 / trustc e2e：** `borrowck_ref_mut_*`、`e2e_closure_call` 未加；HIR `closure_call_*` + `snap_ref_mut` 已覆盖核心路径。完整 e2e 可收 §2.5.3。
3. **闭包调用返回类型：** typeck 验证参数匹配；`f(5)` 的返回类型完整推断归后续 Phase（与 2.4-spec MS-2.4-5 注记一致）。

## 登记

- H-P2-01 / H-P2-02 / H-P2-03 → ✅（见 `DEFERRED-AND-HANDOFFS.md`）
- 一元 `*` 解引用 → 建议登记 H-P2-16 或并入 Phase 3 运算符项
