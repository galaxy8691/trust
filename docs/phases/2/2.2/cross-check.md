# Phase 2.2 交叉核对记录（MS-2.2-10）

> 核对日期：2026-06-15 · 范围：设计文档 × trust-spec × stdlib × design-constraints

## trust-spec.md

- ✅ TYP-REQ-001: 已冻结（移除"前瞻"标记），number=f64
- ✅ OWN-REQ-008: Copy 类型改为 `number`(f64)，移除 i32/f64/bigint
- ✅ LEX-REQ-001: 关键字 43，字面量 5 种（2.1 完成时冻结）
- ✅ SYN-REQ-002/003: 函数声明/控制流规则已对齐 v2.0

## stdlib.md

- ✅ 模块依赖图无 std::result
- ✅ 无用户面 Option/Result 构造器
- 🔜 索引/容量等 number API 签名在 Phase 6 集合类型落地时同步

## Trust-设计文档.md

- 🔜 §2.2 "help 级别警告"还需同步为 "Warning + Help 子诊断"（低优先级，措辞层面）
- ✅ number=f64、整数语义、位运算、as 收敛均与实现一致

## design-constraints.md

- ✅ 无冲突（2.2 变更约束编译器代码，不改变约束本身）
- ✅ P0:magic: 2^53 检查使用命名常量 MAX_SAFE_INTEGER
- ✅ P0:unsafe: 无新增 unsafe

**结论：** 2.2 通过四文档交叉核对。设计文档 §2.2 措辞微调归入后续 PR。
