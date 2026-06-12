# Trust 编译器性能基准

> 工具：criterion (https://github.com/bheisler/criterion.rs)
> 触发：PR 时 `cargo bench` 运行，±10% 视为回归
> 记录：每行 = `<date> <commit> <bench_name> <mean_time> <std_dev>`

## Phase 1 基准 (v0.1)

| 日期 | 提交 | 基准名称 | 均值 | 标准差 |
|------|------|---------|------|--------|
| (待定) | (待定) | compile_100_lines | ? ms | ? ms |

**Phase 1 目标**：编译 100 行 Trust 代码 ≤ 5 秒（冷启动）
**Phase 2 目标**：编译 5000 行 Trust 代码 ≤ 60 秒（冷启动）
