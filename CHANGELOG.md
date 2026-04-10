[中文](CHANGELOG.zh-CN.md)

# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [Unreleased]

### Changed

- **Breaking:** Removed user-visible `Promise<T>` and `Promise.all` from the trust TS surface. `async function` return types use the awaited type (`number` / `string` / `void`); use builtin **`async_all([...])`** instead of `Promise.all`. Type `Promise<...>` is rejected with a diagnostic; `.then` errors no longer mention `Promise.prototype`.

### Added

- Added new workspace crate `trust_stdlib` as the default stdlib facade for generated Rust (`json`, `uri`, `string` helpers).
- Added CLI compatibility switch `--stdlib-mode trust_stdlib|legacy` on `compile` and `run` for migration fallback.
- Driver now injects `trust_stdlib` dependency by default in generated temporary `Cargo.toml`.

## [0.1.0] - 2026-04-08

### Added

- Experimental TypeScript→Rust compiler (`trust-parser`, `trust-hir`, `trust-lower`, `trust-driver`, `trust-cli`, optional `trust_rt`).
- CLI subcommands `compile`, `run`, and `check`; multi-file / minimal `--project` workflow.
- Trust strong-typing subset with English diagnostics, fixtures, and `cli_e2e` integration tests.
- CI: `cargo fmt --all --check`, `cargo test --workspace`, `cargo clippy --workspace --all-targets`.
- Documentation: English-default `README.md`, Chinese `README.zh-CN.md`, contributing and changelog pairs, architecture Mermaid diagram, and unsupported-TS / trust rejection summary table.
