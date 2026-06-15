# JSON→serde 迁移评估

> 日期：2026-07-03 · 来源：`docs/phases/2/TODO.md §2.4.3` · Phase 2.4 产出物

## 当前方案

`crates/trust_error/src/json_fmt.rs` 使用**手写 JSON 格式化**（零外部依赖），输出 NDJSON 格式。

- 手动字符串拼接 + `escape_json_string`（覆盖 5 种控制字符）
- 当前诊断输出规模 < 100 errors/次编译，性能非瓶颈
- 不引入 `serde` / `serde_json` 依赖

## serde 成本估算

| 指标 | 手写（当前） | serde + serde_json |
|------|-------------|-------------------|
| 编译时间 | 基准 | +~3s（serde derive 宏展开） |
| 二进制增量 | 基准 | ~200KB（serde_json + serde 核心） |
| 依赖数 | 0 | 2（serde + serde_json） |
| 代码量 | ~150 行（json_fmt.rs） | ~30 行（#[derive] + serde_json::to_string） |

## 决策

**坚持手写 JSON，不引入 serde。**

理由：
1. 编译时间增量 ~3s 对冷启动体验有可感知影响
2. 二进制增量 ~200KB 突破 Phase 1 零依赖策略的容忍线（TODO 要求 <200KB）
3. 当前诊断输出简单（平坦 NDJSON），手写成本低且已稳定运行
4. 引入 serde 未带来实质功能增益（当前无需复杂嵌套序列化）

## 后续

- 若未来诊断格式需复杂嵌套（如 children 递归、多级 span），可在 Phase 4（错误处理落地后）重新评估
- H-X-01（各 crate 迁移至 `trust_error::Diagnostic`）不依赖 serde——Diagnostic 结构体已有手写 Display/JSON 路径
