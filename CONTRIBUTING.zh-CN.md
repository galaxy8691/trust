[English](CONTRIBUTING.md)

# 参与 trust 贡献

感谢关注本实验性 TypeScript→Rust 编译器。本文说明如何构建工作区、提 PR 前应运行的命令，以及工具链约定。

## 项目路线图

长期可验收项见 [`PROJECT-TODO.md`](PROJECT-TODO.md)（英文默认）与 [`PROJECT-TODO.zh-CN.md`](PROJECT-TODO.zh-CN.md)（中文版）。

## 构建

在仓库根目录执行：

```bash
cargo build
cargo build --release
```

## 必跑命令（与 CI 一致）

推送与 PR 在 GitHub Actions 上执行（见 [`.github/workflows/ci.yml`](.github/workflows/ci.yml)）。提交前请在本地运行：

```bash
cargo fmt --all --check
cargo test --workspace
cargo clippy --workspace --all-targets
```

## 模块拆分约定

- 大文件优先按“编译阶段”拆分：
  - `trust-hir`：`build/*`、`sem/*`、`codegen/*` 辅助模块
  - `trust-cli`：`cli_args.rs`、`commands.rs`、`graph_loader.rs`
  - `trust-driver`：`pipeline.rs`、`cargo_runner.rs`、`crate_writer.rs`
- 纯搬移（不改行为）与语义变更尽量分开提交，便于回归定位。

## Rust 工具链

- **Edition**：全工作区 **Rust 2021**。
- **MSRV**：根目录 [`Cargo.toml`](Cargo.toml) 在 `[workspace.package]` 中声明 **`rust-version = "1.74"`**，请使用 **Rust 1.74 及以上**。
- **CI**：使用 **`ubuntu-latest`** 上的 **latest stable** Rust，满足上述 MSRV。

若本地工具链较旧，可通过 [rustup](https://rustup.rs/) 安装更新的 stable。

## 分支与 PR

- 从默认分支（如 **`master`** 或 **`main`**，以本仓库为准）开 feature 分支，就绪后发 PR。
- 提交尽量聚焦；若影响用户可见行为或诊断信息，请在 PR 说明中写明。
- 若变更语言支持或诊断，请同步更新 [`README.md`](README.md) / [`README.zh-CN.md`](README.zh-CN.md)，必要时更新 [`CHANGELOG.md`](CHANGELOG.md)。

## 许可

参与贡献即表示你同意将贡献以与项目相同方式授权：**MIT OR Apache-2.0**（见 [`README.md`](README.md)）。
