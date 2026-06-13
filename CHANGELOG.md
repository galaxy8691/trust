# Changelog

All notable changes to the Trust compiler will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- Phase 1.1: Cargo workspace (7 crates) + CI/CD (10 jobs) + engineering skeleton
- Phase 1.2: `trust_parser` — lexer (392 lines, 54 keywords) + parser (560 lines, recursive descent + Pratt) + module graph + import resolution (91 tests total)
- Phase 0: Language specification (`spec/trust-spec.md`) + design document (`docs/Trust-设计文档.md`)
- `ferro_rt` runtime stub: `console::log` → `println!` mapping

### Changed
- 2026-06-13: MSRV unified to 1.80 (was 1.63; code depends on `LazyLock` 1.80 + `workspace-inheritance` 1.64+)
- 2026-06-13: Debate review — 20 findings resolved, 0 irreconcilable gaps
