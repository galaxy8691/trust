# 回归测试用例（`tests/regression/`）

在修复 **已知 bug** 或 **防止回归** 时，在此目录添加**最小复现** `*.ts`，并在 [`cli_e2e.rs`](../cli_e2e.rs)（或单独 `tests/regression.rs`）中增加断言。

约定：

- 文件名建议 `regression_<topic>_<short>.ts` 或沿用问题简述。
- 文件头用注释说明：关联 issue / PR、行为预期（编译失败或 `run` 输出）。
- 与 [`fixtures/`](../fixtures/) 的关系：`fixtures/` 覆盖语言矩阵与日常集成；本目录用于**锚定**曾出问题的场景（可与 `fixtures/` 中样例语义重复，但独立维护意图）。

当前示例：`switch_fallthrough_regression.ts`（与 `fixtures/switch_fail.ts` 等价语义：空 `case` 穿透须拒绝）。
