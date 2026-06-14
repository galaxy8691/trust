# Changelog

All notable changes to the Trust compiler will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.0] — 2026-06-13

### Added
- Phase 1.1: Cargo workspace (7 crates) + CI/CD (10 jobs) + engineering skeleton
- Phase 1.2: `trust_parser` — lexer (392 lines, 54 keywords) + parser (560 lines, recursive descent + Pratt) + module graph + import resolution (91 tests total)
- Phase 1.3: `trust_hir` — HIR lowering + type checking + name resolution
- Phase 1.4: `trust_tir` — TIR control flow graph + move semantics + borrow checker + region inference
- Phase 1.5: `trust_codegen` — TIR → Rust source generation + source map + ferro_rt API mapping
- Phase 1.6: `trust_error` — unified Diagnostic struct + JSON output + fix suggestion engine
- Phase 1.7: `trustc` — compiler entry point (CLI + pipeline orchestration + end-to-end tests)
- Phase 1.8: 47 end-to-end test fixtures (56 tests), benchmark skeleton (113ms), fuzzing infrastructure (3 targets)
- Phase 0: Language specification (`spec/trust-spec.md`) + design document (`docs/Trust-设计文档.md`)
- `ferro_rt` runtime stub: `console::log` → `println!` mapping

### Changed
- MSRV unified to 1.80 (was 1.63; code depends on `LazyLock` 1.80 + `workspace-inheritance` 1.64+)
- Debate review — Phase 1.7: 29 findings resolved (4 critical, 9 major, 16 minor), 0 irreconcilable gaps

### Fixed
- Codegen: uninitialized temporary variable declarations (for/while loop compilation fix)
- Clippy: `collapsible_else_if` warning in trust_tir
- Parser: bigint literal parsing (i64 range)
- Error conversion: TIR/Move/Borrow errors now use concrete types with source spans (was generic + hardcoded "unknown")
