[English](CHANGELOG.md)

# 变更日志

本文件记录本项目的重要变更，格式参考 [Keep a Changelog](https://keepachangelog.com/zh-CN/1.1.0/)。

## [Unreleased]

### 变更

- **不兼容：** TS 表面移除用户可见的 `Promise<T>` 与 `Promise.all`。`async function` 返回类型直接写兑现类型（`number` / `string` / `void`）；并行等待改用内置 **`async_all([...])`**。类型位置写 `Promise<...>` 会报错；`.then` 相关诊断不再出现 `Promise.prototype` 字样。

### 新增

- 新增 workspace crate `trust_stdlib`，作为生成 Rust 的默认标准库门面（`json`、`uri`、`string` helper）。
- CLI 新增兼容开关 `--stdlib-mode trust_stdlib|legacy`（`compile` / `run`）用于迁移期回退。
- `trust-driver` 生成临时 `Cargo.toml` 时默认注入 `trust_stdlib` 依赖。

## [0.1.0] - 2026-04-08

### 新增

- 实验性 TypeScript→Rust 编译器（`trust-parser`、`trust-hir`、`trust-lower`、`trust-driver`、`trust-cli`、可选 `trust_rt`）。
- CLI：`compile` / `run` / `check`；多文件与极简 `--project` 工作流。
- trust 强类型子集、英文诊断、fixture 与 `cli_e2e` 集成测试。
- CI：`cargo fmt --all --check`、`cargo test --workspace`、`cargo clippy --workspace --all-targets`。
- 文档：默认英文 `README.md`、中文 `README.zh-CN.md`、贡献指南与变更日志双语、架构 Mermaid 图与不支持的 TS / trust 拒斥简表。
