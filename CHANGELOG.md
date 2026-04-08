[中文](CHANGELOG.zh-CN.md)

# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [Unreleased]

### Added

- (none yet)

## [0.1.0] - 2026-04-08

### Added

- Experimental TypeScript→Rust compiler (`ts2rs-parser`, `ts2rs-hir`, `ts2rs-lower`, `ts2rs-driver`, `ts2rs-cli`, optional `ts2rs_rt`).
- CLI subcommands `compile`, `run`, and `check`; multi-file / minimal `--project` workflow.
- Trust hard-typing subset with English diagnostics, fixtures, and `cli_e2e` integration tests.
- CI: `cargo fmt --all --check`, `cargo test --workspace`, `cargo clippy --workspace --all-targets`.
- Documentation: English-default `README.md`, Chinese `README.zh-CN.md`, contributing and changelog pairs, architecture Mermaid diagram, and unsupported-TS / trust rejection summary table.
