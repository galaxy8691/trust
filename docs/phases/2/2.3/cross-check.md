# Phase 2.3 交叉核对记录（MS-2.3-8）

> 核对日期：2026-07-03 · 范围：设计文档 × trust-spec × stdlib × design-constraints
> 延期项追踪：[`docs/phases/DEFERRED-AND-HANDOFFS.md`](../../DEFERRED-AND-HANDOFFS.md)（H-P3-07a 等）

## Trust-设计文档.md

- ✅ §4.1 三条规则与实现一致（含 `export function` 块体标注 + typeck body 检查）
- ✅ 箭头语法为 `(x: T): R =>`，与实现一致
- ✅ 无冲突

## trust-spec.md

- ✅ SYN-REQ-002：标为 **2.3 已冻结**；新增 EBNF `arrow_fn` 规则 + AC-SYN-008a/b/c
- 🔜 SEM-REQ-003：函数返回类型检查条目已对齐（箭头使用自身返回类型检查 body）；全文 interface 等旧示例清理归 Phase 3
- ✅ LEX-REQ-001：关键字 43 个（2.1 冻结），函数相关关键字（`function`/`inout`/`move`/`this`）均已保留

## stdlib.md

- ✅ 无冲突——2.3 不涉及 stdlib 变更（函数声明规则属语言核心，stdlib 无函数声明语法）
- ✅ 模块大纲与设计 §13 一致（2.1 对齐）

## design-constraints.md

- ✅ P0:unsafe：`trust_hir`/`trust_parser` 无新增 `unsafe`
- ✅ P0:magic：`MAX_SAFE_INTEGER` 等常量已有命名（2.2 引入）
- ✅ 所有新增结构体字段均为 `bool`/`Option<Type>`，无需命名常量

**结论：** 2.3 四文档交叉核对通过，里程碑可关闭。

> **遗留（非阻塞 2.3）：** SEM-REQ-003 interface 名义类型旧示例 → Phase 3；parser 边界 trustc e2e → §2.5.3；`(name) =>` 参数推断 → H-P3-07a。
