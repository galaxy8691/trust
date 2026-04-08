[中文](CONTRIBUTING.zh-CN.md)

# Contributing to ts2rs

Thank you for your interest in this experimental TypeScript→Rust compiler. This document describes how to build the workspace, what to run before opening a PR, and toolchain expectations.

## Project roadmap

Long-term tracked work lives in [`PROJECT-TODO.md`](PROJECT-TODO.md) (English default); Chinese: [`PROJECT-TODO.zh-CN.md`](PROJECT-TODO.zh-CN.md).

## Build

From the repository root:

```bash
cargo build
cargo build --release
```

## Required checks (match CI)

CI runs the following on every push and PR ([`.github/workflows/ci.yml`](.github/workflows/ci.yml)). Run them locally before submitting changes:

```bash
cargo fmt --all --check
cargo test --workspace
cargo clippy --workspace --all-targets
```

## Module layout convention

- Prefer splitting large files by stage-oriented modules:
  - `ts2rs-hir`: `build/*`, `sem/*`, `codegen/*` helpers
  - `ts2rs-cli`: `cli_args.rs`, `commands.rs`, `graph_loader.rs`
  - `ts2rs-driver`: `pipeline.rs`, `cargo_runner.rs`, `crate_writer.rs`
- Keep behavior-preserving move-only refactors separate from semantic changes when possible.

## Rust toolchain

- **Edition**: Rust **2021** (workspace-wide).
- **MSRV**: The workspace declares **`rust-version = "1.74"`** in [`Cargo.toml`](Cargo.toml) (`[workspace.package]`). Use **Rust 1.74 or newer**.
- **CI**: GitHub Actions uses **`ubuntu-latest`** with the **latest stable** Rust from `actions-rust-lang/setup-rust-toolchain` (or equivalent), which satisfies the MSRV.

If you only have an older toolchain, install a newer stable via [rustup](https://rustup.rs/).

## Branches and PRs

- Open feature branches from the default branch (e.g. **`master`** or **`main`**, whichever this repo uses) and submit a PR when ready.
- Keep commits focused; mention user-visible behavior or diagnostic changes in the PR description.
- If you change language support or diagnostics, update [`README.md`](README.md) / [`README.zh-CN.md`](README.zh-CN.md) and, when appropriate, [`CHANGELOG.md`](CHANGELOG.md).

## License

By contributing, you agree that your contributions are licensed under the same terms as the project: **MIT OR Apache-2.0** (see [`README.md`](README.md)).
