# Phase 2.2 交叉核对记录（MS-2.2-10）

> 核对日期：2026-06-15 · 范围：设计文档 × trust-spec × stdlib × design-constraints  
> 延期项追踪：[`docs/phases/DEFERRED-AND-HANDOFFS.md`](../../DEFERRED-AND-HANDOFFS.md)（H-P6-05、H-P2-13 等）

## trust-spec.md

- ✅ TYP-REQ-001: 已冻结（移除"前瞻"标记），number=f64
- ✅ OWN-REQ-008: Copy 类型改为 `number`(f64)，移除 i32/f64/bigint
- ✅ LEX-REQ-001: 关键字 43，字面量 5 种（2.1 完成时冻结）
- ✅ SYN-REQ-002/003: 函数声明/控制流规则已对齐 v2.0

## stdlib.md

- ✅ 模块依赖图无 std::result
- ✅ 无用户面 Option/Result 构造器
- ✅ number=f64 已同步（Phase 2.2 冻结声明已加入 header）
- 🔜 索引/容量等 number API 具体实现在 Phase 6 集合类型落地

## Trust-设计文档.md

- ✅ §2.2 "help 级别警告"→"Warning+Help 子诊断"措辞已同步（规范层）
- 🔜 §2.2 索引/`as usize`、`.length` 实现归 Phase 6（语言尚无索引语法；`MemberAccess` 仅 `console.log`）
- 🔜 2^53 精度：`trust_error` 尚无 `Warning` API，实现为 `DiagError` 占位（与 TODO §2.2.4 / `typeck.rs` 注释一致）；扩展 `Diagnostic` 后对齐设计
- ✅ number=f64、循环计数 f64 推断、位运算、as 收敛与实现一致

## design-constraints.md

- ✅ 无冲突
- ✅ P0:magic: 2^53 检查使用命名常量 MAX_SAFE_INTEGER
- ✅ P0:unsafe: 无新增 unsafe

**结论：** 2.2 四文档交叉核对通过（规范层）。Phase 6 集合类型落地后复查 stdlib number API 签名、索引 codegen、`Warning`+`Help` 诊断 API。
