[English](CHANGELOG.md)

# 变更日志

本文件记录本项目的重要变更，格式参考 [Keep a Changelog](https://keepachangelog.com/zh-CN/1.1.0/)。

## [Unreleased]

### 新增

- （尚无）

## [0.1.0] - 2026-04-08

### 新增

- 实验性 TypeScript→Rust 编译器（`ts2rs-parser`、`ts2rs-hir`、`ts2rs-lower`、`ts2rs-driver`、`ts2rs-cli`、可选 `ts2rs_rt`）。
- CLI：`compile` / `run` / `check`；多文件与极简 `--project` 工作流。
- trust 强类型子集、英文诊断、fixture 与 `cli_e2e` 集成测试。
- CI：`cargo fmt --all --check`、`cargo test --workspace`、`cargo clippy --workspace --all-targets`。
- 文档：默认英文 `README.md`、中文 `README.zh-CN.md`、贡献指南与变更日志双语、架构 Mermaid 图与不支持的 TS / trust 拒斥简表。
