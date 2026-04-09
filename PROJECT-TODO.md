[中文](PROJECT-TODO.zh-CN.md)

# ts2rs long-term project TODO

This document tracks compiler and toolchain work over time, grouped by theme. Use `[ ]` / `[~]` / `[x]` in PRs or commits as appropriate. **Every item assumes the strong-typing (trust) model.**

**Code entry points**: [`README.md`](README.md) · [`crates/ts2rs-hir`](crates/ts2rs-hir) (`build.rs` / `sem.rs` / `codegen.rs` / `ir.rs`) · [`crates/ts2rs-parser`](crates/ts2rs-parser) · [`crates/ts2rs-driver`](crates/ts2rs-driver) · [`crates/ts2rs-cli`](crates/ts2rs-cli) · [`test-ts/main.ts`](test-ts/main.ts) (multi-file: [`test-ts/math.ts`](test-ts/math.ts)) · [`crates/ts2rs-cli/tests/fixtures/`](crates/ts2rs-cli/tests/fixtures/)

Chinese mirror: [`PROJECT-TODO.zh-CN.md`](PROJECT-TODO.zh-CN.md).

**Follow-up backlog (what to do next)**: [§14 Next steps](#14-next-steps-follow-up-backlog) — consolidated; older sections may still mention the same work in passing.

### Planning constraint: trust (strong typing)

**trust is strongly typed; there is no soft typing.** Long-term items and PR trade-offs must stay consistent: only extend syntax that can get **static** rules in HIR / [`sem.rs`](crates/ts2rs-hir/src/sem.rs). **Do not** target implicit `any`, runtime reshaping, or un-annotated widen-in as goals. See [README — Trust: strong typing](README.md).  
Here, “narrowing”, “assignable”, and “structural / shape” mean **static rules inside HIR / sem**, not runtime reshaping, and not aligning with `tsc`’s default loose or progressive soft typing.

---

## 0. Vision and “1.0” acceptance (editable)

- [x] **Single-file subset**: every feature the README matrix marks as supported has fixtures and integration tests ([`crates/ts2rs-cli/tests/fixtures/`](crates/ts2rs-cli/tests/fixtures/) + [`cli_e2e.rs`](crates/ts2rs-cli/tests/cli_e2e.rs)); `ts2rs-lower` also has compile unit tests.
- [x] **Diagnostics**: common errors include line/column (`path:line:col`); messages are **English** (see README “Scope (1.0)”).
- [x] **Reproducible**: `cargo test --workspace`, `cargo clippy --workspace --all-targets`; [`.github/workflows/ci.yml`](.github/workflows/ci.yml) on push/PR.
- [x] **Multi-file (if in scope)**: **not** full project graphs for 1.0; **relative** `import { x } from "./dep.ts"` uses [`parse_module_graph`](crates/ts2rs-parser/src/module_graph.rs) (no merged AST), CLI and [`compile_entrypoint_to_executable`](crates/ts2rs-driver/src/lib.rs) via `validate_imports` → `lower_module_graph`; see §6.2.

---

## 1. Frontend: parsing and AST coverage

### 1.1 Robustness on supported paths

- [x] **Error recovery**: **single diagnostic** today (fail on first); multi-diagnostic collection is future work; see [README — Diagnostics (§1.1)](README.md).
- [x] **Preserving comments**: **assessed** — AST has no comments, `source_map` exists; end-to-end comments need parser/token work; conclusion in README §1.1.
- [x] **`export` variants**: everything except `export function` is explicitly rejected ([`build.rs`](crates/ts2rs-hir/src/build.rs)); negative fixtures `export_*_fail.ts` + `cli_e2e`.

### 1.2 Statements and declarations

- [x] **`import`**: relative `import { f } from "./x.ts"` via module graph ([`module_graph.rs`](crates/ts2rs-parser/src/module_graph.rs)); old [`resolve_imports.rs`](crates/ts2rs-parser/src/resolve_imports.rs) removed; non-relative paths still error (`import_fail.ts`).
- [x] **Nested `function`**: [`IRStmt::FnDecl`](crates/ts2rs-hir/src/ir.rs) + no-capture subset; see `nested_fn.ts`.
- [x] **`const`**: aligned with `let`; reassignment forbidden; `const_ok.ts`, `const_reassign_fail.ts`.
- [x] **Assignment in expression statements**: `IRStmt::Assign` + mutable `let`; `assign_simple.ts`.
- [x] **`for` / `do-while`**: C-style `for` (including update assign), `do-while`; **`switch`**: lowered to `If` chain in `build` (see §13.5, `switch_ok.ts`).
- [x] **`break` / `continue`**: inside loops; no labels.
- [x] **Empty statement / blocks**: `Stmt::Empty`, `Block`; `empty_stmt.ts`.

### 1.3 Expressions

- [x] **`async` / `await` / `Promise` / HTTP `fetch` / `fetchText` (MVP)**: [`IRFunction::is_async`](crates/ts2rs-hir/src/ir.rs), [`IRExpr::Await`](crates/ts2rs-hir/src/ir.rs) / [`FetchText`](crates/ts2rs-hir/src/ir.rs) / [`Fetch`](crates/ts2rs-hir/src/ir.rs) / [`PromiseAll`](crates/ts2rs-hir/src/ir.rs), [`#[tokio::main]`](crates/ts2rs-hir/src/codegen.rs), driver injects [`tokio` + `reqwest`](crates/ts2rs-driver/src/crate_writer.rs) and **`futures-util`** when generated Rust uses streaming (`crate_writer` detects `futures_util` in source). **`await` in arbitrary control flow** is implemented; **`fetchText(url)`** → `Promise<string>`; **`fetch(url, init?)`** → `Promise<Response>` with **`status`**, **`ok`**, **`await .text()`**, **`await .json()`** (JSON **number** body → `f64` via **`serde_json`**, same as dynamic `JSON.parse`), **`response.body.getReader()`** + **`await reader.read()`** (chunked body via `bytes_stream()`), and **optional `init`** (`method` string literal, `headers` object with string-literal values, optional `body` string); **`.then`** is rejected with a diagnostic; **`Headers` iteration / Web `Request` parity / byte-level TLS·HTTP2 parity with Node** remain out of scope (see §Async / HTTP backlog).
- [x] **Member access and call chains**: `string.length` (UTF-16), `string[i]` (single UTF-16 code unit as `string`), `number[]` / `string[]` index, `length` on objects; **`obj.m(args)`** → global `m(receiver,…)` ([`IRExpr::MethodCall`](crates/ts2rs-hir/src/ir.rs)); **one-level** `f().prop` / `f().m()` ([`chain_call_ok.ts`](crates/ts2rs-cli/tests/fixtures/chain_call_ok.ts)); optional **`?.` / `f?.()` / `recv?.m()`** ([`optional_call_ok.ts`](crates/ts2rs-cli/tests/fixtures/optional_call_ok.ts)); fixtures `member_length_ok.ts`, `method_call_ok.ts`, `string_utf16_length.ts`, `stdlib_hir_ok.ts`.
- [x] **Optional chaining / nullish coalescing**: limited subset (`obj?.prop`, `??`; `optional_ok.ts`, `nullish_ok.ts`); full semantics tied to §3.3.
- [x] **Logical short-circuit**: `&&`, `||`; `boolean` and `number` truthiness (`!= 0`), result type `boolean` (`logical_bool.ts`, `logical_truthy_ok.ts`); differs from TS value-preserving `&&`/`||`; under **strong typing** result is `boolean`; more complex truthiness or unions still limited.
- [x] **Ternary**: `cond ? a : b` (`ternary_ok.ts`).
- [x] **Comma expression**: `comma_ok.ts`.
- [x] **Template literals**: no tag; `template_ok.ts`.
- [x] **Array / object literals**: limited subset (`number[]`, `{ k: number }`; `array_ok.ts`, `object_ok.ts`); runtime and full types in §1.4 / §2.1.

**§1.3 follow-ups (notes)**

- **Method / chain typing**: `obj.m` and one-level `f().g` are implemented; **richer receiver typing** (e.g. arbitrary class instance methods) remains limited — see class subset in README matrix.
- **`??` / `?.`**: same-family `Union` narrowing for `??` and optional call/member are implemented; **full discriminated narrowing** still future (§3.3).
- **“Full” types for array/object literals**: richer elements/fields and `TsType`/IR evolution in §1.4, §2.1 — not only expression layer.

### 1.4 Type syntax (types only)

**Summary**

- [x] **Literal types**, **union types**, **`interface`**, **`type` aliases**: aligned with **strong-typing** checker roadmap (sub-items below; **literal types**, **primitive/literal unions**, **limited `interface`→`ObjectNum`**, **limited `type` alias→named table**). **Generics** are a separate sub-item (document “still rejected” milestone, not semantics).

**Relation to implemented subset**: §1.3 supports limited annotations `number[]`, `{ k: number }` ([`TsType::ArrayNumber`](crates/ts2rs-hir/src/ir.rs) / [`ObjectNum`](crates/ts2rs-hir/src/ir.rs)). **Literal types** (`NumberLit` / `StringLit` / `BoolLit`) and **unions** ([`TsType::Union`](crates/ts2rs-hir/src/ir.rs) + normalization) below; **top-level `interface`** is nominal `ObjectNum` in the type layer (same rules as object type literals); **top-level `type` aliases** via [`collect_named_types_with_errors`](crates/ts2rs-hir/src/build/build_types.rs) into the same named table; **generic semantics** still not implemented — rejection table in [README §1.4](README.md) and sub-items below; full object/interface shapes and IR in §2.1; **static** null and branch narrowing crosses §3.3.

**Sub-items**

- [x] **Literal types** (e.g. `42`, `"a"`, `true` in type position)  
  - **Deps**: extend [`TsType`](crates/ts2rs-hir/src/ir.rs); **static** assignability to base types (with §3.3 explicit shapes / sem rules, not full TS structural subtyping).  
  - **Done**: [`build.rs`](crates/ts2rs-hir/src/build.rs) parses `TsLitType`; [`sem.rs`](crates/ts2rs-hir/src/sem.rs) `type_assignable` / literal inference; `literal_type_ok.ts`, `literal_type_fail.ts` + [`cli_e2e.rs`](crates/ts2rs-cli/tests/cli_e2e.rs).

- [x] **Union types** (`A | B`, primitives/literals first)  
  - **Deps**: normalization and decidable union equality; **static** narrowing / branch types for `??` / `?.` (strong typing, decidable) with §3.3.  
  - **Done**: assign/branches consistent under limited unions; tests (`union_literal_ok`, `union_cond_ok`, negatives `union_heterogeneous_fail`, `intersection_type_fail`, `union_mixed_cond_fail`).

- [x] **`interface` and object types** (body, optional props, `extends` staged)  
  - **Deps**: **explicit field shapes** in IR (§2.1), **static** sem checks; **not** full TS structural subtyping; relationship to existing `ObjectNum` documented in implementing PRs.  
  - **Done**: at least one `interface` shape compiles to equivalent Rust or clear errors (`interface_ok`, `export_interface_ok`; negatives `interface_extends_fail`, `interface_generic_fail`; README on single-file ordering).

- [x] **`type` aliases** (`type Id = …`)  
  - **Deps**: collect top-level `TsTypeAlias` into symbol table; swc already parses, wired into HIR/sem.  
  - **Done**: aliases in param/var annotations; fixtures + e2e (`type_alias_ok`, `type_alias_to_interface_ok`, `export_type_alias_ok`; negatives `type_alias_generic_fail`, `type_alias_dup_fail`).

- [x] **Generics** (`function f<T>(…)` and type parameters)  
  - **Deps**: monomorphization or limited strategy still **future**; rejection and English diagnostics in [README §1.4 — Generics](README.md), [`build.rs`](crates/ts2rs-hir/src/build.rs) generic checks.  
  - **Done**: document “still rejected” in stages — [README](README.md) table + `generic_function_fail`, `interface_generic_fail`, `type_alias_generic_fail` + e2e; **no** generic semantics in this milestone.

---

## 2. IR (`ir.rs`) evolution

### 2.1 Current structure

- [x] **Statements**: `Assign`, `Break`, `Continue`, `DoWhile`, `FnDecl`, `Empty`; `for` lowered to `while`; no `Switch` IR stmt.
- [x] **Expressions**: `LogicalAnd`/`LogicalOr`, `Conditional`, `Seq`, `Tpl`, `Member` / `OptionalMember`, [`Index`](crates/ts2rs-hir/src/ir.rs) (array `number`/`string` elements, string UTF-16), [`MethodCall`](crates/ts2rs-hir/src/ir.rs) / [`OptionalMethodCall`](crates/ts2rs-hir/src/ir.rs), one-level chained `f().prop` / `f().m()`, [`JsonBuiltin`](crates/ts2rs-hir/src/ir.rs) / [`UriBuiltin`](crates/ts2rs-hir/src/ir.rs), math/string/http builtins as in README matrix; **computed** `obj[expr](…)` call still unsupported. `ObjectNum` / `interface` shapes: §1.4; strong typing, static checks.
- [x] **Top level**: multi-file graph — `parse_module_graph` + `validate_imports`; HIR merged to [`IRModule`](crates/ts2rs-hir/src/ir.rs) (`build_program_multi` / `compile_graph`); `main` in entry file; global function names unique; negatives `import_missing_export_*`, `circular_*`, `dup_*`.

### 2.2 Metadata and debugging

- [x] **Span**: HIR nodes carry swc `Span`; [`diag`](crates/ts2rs-hir/src/error.rs) and codegen errors use function `cm` + `source_path` + node `span` (see [`ir.rs`](crates/ts2rs-hir/src/ir.rs) module docs); whole-file `span` if no top-level function in `build`; `sem` missing `main` anchors to first function `span`.
- [x] **Optional**: [`IRFunction::ir_id`](crates/ts2rs-hir/src/ir.rs) (per function, including nested; monotonic per compile, see `build_fn`).

---

## 3. Semantic analysis (`sem.rs`)

### 3.1 Solidifying what exists

- [x] **Symbol table**: block scope and duplicate `let` (partial) — fixtures for nesting/shadowing: `let_dup_same_block_fail.ts`, `let_shadow_nested_ok.ts`, `param_let_same_name_fail.ts` + [`cli_e2e.rs`](crates/ts2rs-cli/tests/cli_e2e.rs).
- [x] **Control flow**: simplified `stmts_return` — documented vs TS/tsc (**trust only guarantees static rules**; README). Done: [README — Control flow and return](README.md).
- [x] **`void` and `console.log`**: `BuiltinLog` void paths covered; branch-only-log case `void_log_in_branch.ts` + e2e.

### 3.2 Mutability and assignment

- [x] **Mutable `let`**: `IRStmt::Assign`; LHS must be a bound identifier.
- [x] **No assignment to `const`**.

### 3.3 Deeper type system

- [x] **Tie-in with §1.4**: literal and union types and **static** `??` / `?.` narrowing must stay consistent with §1.4 (avoid conflict with limited `TsType`). Unions are in HIR. **Implemented (sem)**: when `Union` minus `null`/`undefined` matches the right-hand side of `??` as the same “family” (`number` / `string` / `boolean`, or **structurally matching** `Fn`), [`infer_expr_mut`](crates/ts2rs-hir/src/sem.rs) on `IRExpr::NullishCoalesce` calls `unify_ternary_branches` for a single result type. **Still future**: discriminated-union narrowing that relies on a **discriminant** (large scope). Done: README §3.3; `nullish_ok` / `optional_ok`; sem + `Fn` unions covered by `nullish_fn_ok.ts` (`ts2rs check`).
- [x] **`null` / `undefined`**: [`TsType`](crates/ts2rs-hir/src/ir.rs) has `Null`/`Undefined` variants; checks follow **current sem static rules**, not tsc’s default “everything nullable”. Done: README §3.3; **no** `strictNullChecks`-style switch. If added later, make it an **explicit** compiler mode, not implicit JS looseness.
- [x] **Structural vs nominal**: **trust** uses nominal table + static shape checks; Rust mapping strategy (mostly primitives today). Done: README §3.3; **not** implementing full TS structural subtyping as a goal.
- [x] **Function types and HOF**: Align with [§13.2](PROJECT-TODO.md) and README — a **restricted** subset is implemented (function types, arrow values, calls, passing/returning functions); closure codegen remains the strict `(number) => number` subset. **Do not** claim “no first-class functions / no HOF”; distinguish “supported HOF subset” vs “further generalization / codegen work”.

### 3.4 Control-flow analysis (advanced)

- [x] **Reachability**: unreachable warnings (`warning: path:line:col: unreachable code`); `early_return_unreachable.ts`, `unreachable_after_return.ts`, `break_unreachable.ts` + [`cli_e2e.rs`](crates/ts2rs-cli/tests/cli_e2e.rs).
- [x] **Definite assignment**: `let x: number;` without init allowed; must assign before use (`if`/`else` merge, conservative loops); `definite_assign_ok.ts`, `definite_assign_if_ok.ts`, negative `definite_assign_fail.ts`.
- [x] **Finer return**: early exhaustive return in a sequence (`if`/`else` both return → following dead code warnings only); `while`/`do-while` still use tail rules (`tail_returns_while_body`); future `switch` may extend `stmt_fn_returns_complete`.

---

## 4. Code generation (`codegen.rs`)

### 4.1 Current behavior

- [x] **`console.log` multi-arg format**: [`emit_builtin_log`](crates/ts2rs-hir/src/codegen.rs) uses spaced `"{}"`; test `console_log_multi_arg_uses_spaced_format`.
- [x] **Arithmetic and `/`**: TS `number` → Rust **`f64`**; **`/`** is IEEE-754 double division (unlike the former `i32` truncating division); README “Arithmetic, `/`, overflow” and matrix.
- [x] **NaN / ∞ and overflow**: possible; not identical to V8 `number` edge cases in every scenario; **no** runtime-checked Cargo feature yet.

### 4.2 Mapping new features

- [x] **Assignment**: `let mut` and blocks match Rust (`codegen_42_let_mut_block_and_assign`; [`emit_stmt`](crates/ts2rs-hir/src/codegen.rs) `Let`/`Assign`/`Block`).
- [x] **Strings**: `String` + `format!`; large strings/perf TBD (`codegen_42_string_concat_uses_format`; [`emit_expr`](crates/ts2rs-hir/src/codegen.rs) `StrConcat`/`Tpl`).
- [x] **Heap / GC**: objects are value `HashMap::from`, **no** `Rc`/`Arc` yet (`codegen_42_object_literal_hashmap_without_rc`; [`emit_expr`](crates/ts2rs-hir/src/codegen.rs) `ObjectLit`).

### 4.3 Readability of emitted Rust

- [x] **Indent / line breaks**: comma (`Seq`) block lines and closing `})` align for rustfmt (`codegen_43_comma_seq_indented`; [`emit_seq_expr`](crates/ts2rs-hir/src/codegen.rs) / `emit_expr` `stmt_level`).
- [x] **Comments**: optional `// ts: path:line:col` per statement (`codegen_43_span_comments_emits_ts_anchors`; `ts2rs-cli` `compile_span_comments_writes_ts_anchors`; [`emit_stmt`](crates/ts2rs-hir/src/codegen.rs), `CodegenOptions`; `ts2rs compile --span-comments`).

---

## 5. Builtins and std mapping

### 5.1 `console`

- [x] **`console.log` / `console.error` / `console.debug`**: `log` → `println!`, `error`/`debug` → `eprintln!` (`console_error_and_debug_use_eprintln`; `compile_console_stderr_writes_eprintln`; [`build.rs`](crates/ts2rs-hir/src/build.rs); [`emit_builtin_log`](crates/ts2rs-hir/src/codegen.rs)).
- [x] **Formatting**: same as §4.1, spaced `"{}"`; shared [`emit_builtin_log`](crates/ts2rs-hir/src/codegen.rs).

### 5.2 Minimal runtime ([`ts2rs_rt`](crates/ts2rs_rt))

- [x] **Strings**: `string.length` is **UTF-16 code units** (`encode_utf16().count()`); `number[].length` → `Vec::len`; object field `length` via `HashMap::get` ([`MemberLengthDispatch`](crates/ts2rs-hir/src/ir.rs)) (`codegen_52_string_length_utf16`, `codegen_52_object_length_field_uses_get`; CLI tests). **`string` subscript `s[i]`**: UTF-16 index → single-code-unit `string` ([`IndexKind::StringUtf16`](crates/ts2rs-hir/src/ir.rs); `stdlib_hir_ok.ts`).
- [x] **Math**: `Math.abs` / `min` / `max` / `floor` / `ceil` / `sign` / `trunc` / `round` / `pow` etc. lower to **`f64`** operations in codegen ([`MathBuiltinKind`](crates/ts2rs-hir/src/ir.rs); [`build.rs`](crates/ts2rs-hir/src/build.rs); [`emit_expr`](crates/ts2rs-hir/src/codegen.rs); matches README matrix `Math.*` row).
- [x] **HIR stdlib (no `ts2rs_rt` required)**: `Number.parseInt` / `parseFloat`; **`JSON.stringify` / `JSON.parse`**: **string literal** args folded at build time with **`serde_json`** into trust-closed IR; **dynamic** strings match `await response.json()` via **`serde_json::from_str`** into `f64` and the trust subset (see §14 “HIR stdlib / JSON / strings”); `String` methods `charAt`, `charCodeAt`, `slice`, `substring`, `indexOf`, `includes`; global `readLine()` via inlined `std::io` (rejected in `async` bodies); [`stdlib_hir_ok.ts`](crates/ts2rs-cli/tests/fixtures/stdlib_hir_ok.ts), [`json_uri_trust_ok.ts`](crates/ts2rs-cli/tests/fixtures/json_uri_trust_ok.ts).
- [x] **I/O**: [`ts2rs_rt::read_stdin_line`](crates/ts2rs_rt/src/lib.rs) placeholder (`std::io`); **optional** — sync `readLine()` is emitted in generated Rust **without** linking `ts2rs_rt`; driver temp crate still does not depend on `ts2rs_rt` unless `--link-ts2rs-rt`.

---

## 6. Driver and build ([`ts2rs-driver`](crates/ts2rs-driver))

### 6.1 Single-file path

- [x] **Temp directory lifecycle**: documented [`TempDir`](https://docs.rs/tempfile) drop and `(TempDir, PathBuf)` return in [`compile_entrypoint_to_executable`](crates/ts2rs-driver/src/lib.rs) / [`build_rust_to_executable`](crates/ts2rs-driver/src/lib.rs) / [`build_rust_and_copy`](crates/ts2rs-driver/src/lib.rs) (`lib.rs` docs; `cargo test --workspace`).
- [x] **Offline / no cargo**: [`DriverError::CargoNotFound`](crates/ts2rs-driver/src/lib.rs) on `NotFound`; build failures → [`DriverError::CargoBuild`](crates/ts2rs-driver/src/lib.rs) with stdout/stderr (`map_cargo_spawn_error_maps_not_found_to_cargo_not_found`).

### 6.2 Multi-file and modules ([`compile_entrypoint_to_executable`](crates/ts2rs-driver/src/lib.rs))

- [x] **Multi-root parsing**: [`parse_module_graph_with_extra_roots`](crates/ts2rs-parser/src/module_graph.rs); CLI multiple `.ts` or `--project` + minimal JSON `files` ([`ts2rs-cli`](crates/ts2rs-cli/src/main.rs)) (`extra_root_includes_unreachable_file`; `run_multi_entry_extra_roots_prints_main`, `run_project_tsconfig_prints_main`).
- [x] **Dependency graph (subset)**: entry + relative `import` → [`parse_module_graph`](crates/ts2rs-parser/src/module_graph.rs) (per-module AST) → `validate_imports` → [`lower_module_graph`](crates/ts2rs-lower/src/lib.rs) → one Rust crate.
- [x] **Generating `Cargo.toml`**: [`RustBuildOptions`](crates/ts2rs-driver/src/lib.rs) / [`build_rust_to_executable_with_options`](crates/ts2rs-driver/src/lib.rs); optional path dep `ts2rs_rt` + feature; CLI `--link-ts2rs-rt` (`write_minimal_crate_with_link_ts2rs_rt_contains_optional_path_dep`; `run_with_link_ts2rs_rt_prints_main`).
- [x] **Cycles**: [`parse_module_graph`](crates/ts2rs-parser/src/module_graph.rs) detects and errors (`circular_*.ts`).

---

## 7. CLI ([`ts2rs-cli`](crates/ts2rs-cli))

- [x] **Subcommands**: `compile` / `run` / `check`; README CLI table and `ts2rs --help`; `check` is HIR+sem only ([`check_module_graph`](crates/ts2rs-lower/src/lib.rs)) (`check_sample_ok`, `check_switch_fail_stderr`).
- [x] **Flags**: `compile -o`; `run` `-O`/`--release` and `--debug` ([`RustBuildOptions::release`](crates/ts2rs-driver/src/lib.rs)); global `-q`/`--quiet`, `--color`, `--emit-ir` (`compile_emit_ir_stderr_contains_ir_module`, `debug_build_writes_binary_under_target_debug`).
- [x] **Exit codes**: as README; `run` forwards child `ExitStatus::code` (else `1`); ts2rs errors `1` ([`main.rs`](crates/ts2rs-cli/src/main.rs) `exit_code_for_failed_child`).

---

## 8. Testing and quality

### 8.1 Integration ([`cli_e2e.rs`](crates/ts2rs-cli/tests/cli_e2e.rs) / `fixtures/`)

- [x] **Each matrix row** has a minimal fixture (or one large file with section comments). Done: README “[Matrix vs integration tests](README.md#matrix-vs-integration-tests)” maps rows to `fixtures/` + `cli_e2e`; extras `array_fail`, `optional_chain_fail`, `nullish_fail`, `object_fail`.
- [x] **Regression**: known bugs pinned under [`tests/regression/*.ts`](crates/ts2rs-cli/tests/regression/) ([`tests/regression/README.md`](crates/ts2rs-cli/tests/regression/README.md), [`switch_fallthrough_regression.ts`](crates/ts2rs-cli/tests/regression/switch_fallthrough_regression.ts), `regression_switch_fallthrough_check_fails`).

### 8.2 Unit tests

- [x] **`ts2rs-hir`**: `build`/`sem`/`codegen` `#[cfg(test)]` (`build_module_records_main`, `check_module_accepts_simple_main` / `check_module_rejects_missing_return`, `emit_rust_contains_ts_main_and_println`; dev-dep `ts2rs-parser`).
- [x] **parser**: swc wrapper snapshots/minimal cases ([`lib.rs`](crates/ts2rs-parser/src/lib.rs) `parse_rejects_unclosed_function_body`, `parses_module_with_import_and_export_main`).

### 8.3 Tooling

- [x] **CI** (GitHub Actions): `cargo test`, `clippy`, fmt ([`.github/workflows/ci.yml`](.github/workflows/ci.yml): rustfmt+clippy components, `cargo fmt --all --check` → `cargo test --workspace` → `cargo clippy --workspace --all-targets`).
- [x] **Fuzzing (optional)**: random AST mutations do not panic ([`parse_fuzz_inputs_do_not_panic`](crates/ts2rs-parser/src/lib.rs) on `parse_typescript_file`).

---

## 9. Documentation and developer experience

- [x] **README**: matrix synced with implementation; “unsupported TS” summary (**also describes trust strong-type rejection**). **Done**: [`README.md`](README.md) (English default) and [`README.zh-CN.md`](README.zh-CN.md) — **Unsupported TypeScript (trust rejection boundary)** / **不支持的 TypeScript 特性（trust 强类型拒斥边界）** and language matrix.
- [x] **Architecture diagram**: parse → HIR → sem → codegen → driver (Mermaid). **Done**: Mermaid `flowchart LR` under **Architecture** / **架构** in both READMEs (`ts2rs_parser` → HIR → sem → codegen → `ts2rs_lower` → `ts2rs_cli` / `ts2rs_driver`).
- [x] **Contributing**: `CONTRIBUTING.md` (branch, test commands, MSRV). **Done**: [`CONTRIBUTING.md`](CONTRIBUTING.md) / [`CONTRIBUTING.zh-CN.md`](CONTRIBUTING.zh-CN.md); root [`Cargo.toml`](Cargo.toml) `rust-version = "1.74"` and per-crate `rust-version.workspace = true`.
- [x] **Changelog**: `CHANGELOG.md` for releases. **Done**: [`CHANGELOG.md`](CHANGELOG.md) / [`CHANGELOG.zh-CN.md`](CHANGELOG.zh-CN.md), Keep a Changelog with `[Unreleased]` and `[0.1.0]`.
- [x] **This roadmap**: English default [`PROJECT-TODO.md`](PROJECT-TODO.md), Chinese [`PROJECT-TODO.zh-CN.md`](PROJECT-TODO.zh-CN.md), cross-linked at the top of each file.

---

## 10. Performance and scale (later)

Multi-file **parallel semantic checking** is implemented (`rayon`; details under **§14 — Performance and security**). This section keeps only what is still open.

- [ ] **Incremental compile**: multi-file, recompile only changed modules.

---

## 11. Security and boundaries

Aligned with **§14 — Performance and security**; **status and pointers are authoritative in §14** (this heading remains for navigation).

- [x] **Generated-code injection**: string literal escaping and `println!` safety. (Done; see §14.)
- [x] **Resource limits**: optional timeout / memory / output cap around driver `cargo`. (Done; see §14.)

---

## 12. Priority suggestions (adjust as needed)

| Priority | Theme | Notes |
| -------- | ----- | ----- |
| P0 | Assignment + mutable `let` | Real loops/accumulation, matches `test-ts` |
| P0 | Diagnostics + test coverage | Stability baseline |
| P1 | `console.log` formatting / small runtime string APIs | UX and examples |
| P1 | Nested functions or explicit long-term “unsupported” story | Less user confusion |
| P2 | Multi-file + `import` | Driver-heavy |
| P2 | Logic + ternary | Common TS patterns |
| P3 | Generics, deeper static typing under strong typing | Long-term (**not** full tsc / soft typing); details in §13 |

---

## 13. Large language features (milestones)

Large efforts; land as **parse (swc/AST) → HIR → `sem` → `codegen` → integration/unit tests** PRs. Semantics stay **trust strong typing** (statically decidable); **no** need for full `tsc` equivalence. Update [README.md](README.md) matrix and this section when sub-milestones land.

### 13.1 Generics (functions / interfaces / type aliases / type arguments)

- [x] **Design**: monomorphization, erasure, or limited strategy (document vs README generics table).（本轮采用单态化子集）
- [x] **Parse + build**: `type_params`, generic bounds, `TsTypeRef` args into HIR.（已接入泛型声明与显式类型实参解析）
- [x] **sem**: argument substitution and consistency (decidable subset under strong typing).（调用处显式类型实参校验、类型替换与实例化改写）
- [x] **codegen**: monomorph expansion or equivalent Rust emission.（消费单态化结果；未实例化类型参数在 codegen 兜底报错）
- [x] **Tests**: fixtures + `cli_e2e` + negatives (still reject overly broad cases).（新增 `generic_function_ok` / `generic_function_missing_type_args_fail` 与对应 e2e）

### 13.2 Higher-order functions (first-class functions, types, calls)

- [x] **Design**: capture strategy, stack closures vs extending no-capture subset, `fn` types in HIR. (Current implementation uses typed arrow closures with `Rc<dyn Fn(i32) -> i32>` codegen path.)
- [x] **HIR**: function types, `Callee` for member/var call paths. (Added `TsType::Fn` and `IRExpr::ArrowFn`; variable call path `f(...)` is type-checked as callable function value.)
- [x] **build + sem**: arrow functions and function values, call/assign typing. (Build parses arrow functions and function type annotations; sem checks function-value assign/call and function-typed arguments/returns.)
- [x] **codegen**: `Fn`/`fn` pointers or struct closures (per design). (Codegen emits typed Rust closures via `Rc<dyn Fn(i32) -> i32>` for the current strongly typed subset.)
- [x] **Tests**: minimal HOF + relation to existing `nested_fn` no-capture semantics. (Added `hof_apply_ok.ts` and `hof_return_closure_ok.ts` plus corresponding `cli_e2e` tests; existing `nested_fn` remains valid.)

### 13.3 Full OO (`class`, `this`, ctor/inheritance)

- [x] **Design**: Rust mapping (struct + impl, or explicit rejection of some TS); `export class` and modules. (Landed OO subset with class lowering + dyn-trait scaffold in codegen, while preserving strong-typing constraints.)
- [x] **build**: `ClassDecl`, methods, fields in HIR (possibly staged). (Added class collection/lowering, `new` calls, `this` rewriting, and subclass constructor `super(...)` lowering path.)
- [x] **sem**: `this`, visibility, inheritance/override (per subset). (Added class validation: extends graph checks, `super(...)` placement checks, and baseline `override` signature/name validation.)
- [x] **codegen**: dispatch, `super` (if in scope). (Added class dyn-trait emission scaffold and kept runtime path through lowered constructor/method functions.)
- [x] **Tests**: class fixtures + negatives for unsupported modifiers. (Added class success/failure fixtures + `cli_e2e` coverage for basic class, this-method, extends/super, and override diagnostics.)

### 13.4 `for..in`

- [x] **Design**: static type for iterating keys (`string` keys vs `ObjectNum` / extended object model). (Chosen rule: `for..in` loop variable is `string`; right side supports object/class-instance keys and `number[]` index keys.)
- [x] **HIR**: `ForIn` or lowering strategy. (Added `IRStmt::ForIn` with sem-filled iteration kind and integrated build construction from `Stmt::ForIn`.)
- [x] **sem**: loop variable type vs object/dict representation. (Sem enforces `string` loop variable and validates RHS as object/class-instance/array.)
- [x] **codegen**: iterate `HashMap` keys or runtime helper. (Codegen emits object-key iteration via `HashMap::keys()` and array index iteration via `0..len` converted to string keys.)
- [x] **Tests**: fixture + compare with `for(;;)`. (Added `for_in_object_keys_ok`, `for_in_object_keys_sum_ok`, `for_in_non_object_fail`, `for_in_key_type_mismatch_fail` and corresponding `cli_e2e` cases.)

### 13.5 Full `switch` / `case`

- [x] **Design**: strong-typing subset — no fall-through, `default` last, `case` only numeric/boolean literals; full ECMA fall-through/`default` placement TBD.
- [x] **HIR**: no `IRStmt::Switch`; `switch` lowered in [`build.rs`](crates/ts2rs-hir/src/build.rs) to nested [`IRStmt::If`](crates/ts2rs-hir/src/ir.rs) + [`IRExpr::Binary`](crates/ts2rs-hir/src/ir.rs) `Eq` (same idea as §2.1).
- [x] **sem**: reuses `if` conditions and `Binary` `Eq`; no dedicated `switch` arm analysis.
- [x] **codegen**: same `If`/`Eq` emission; no dedicated `match` for `switch`.
- [x] **Tests**: positive [`switch_ok.ts`](crates/ts2rs-cli/tests/fixtures/switch_ok.ts) (`run_switch_ok_prints_seven`, `compile_switch_ok_writes_rust`); negative [`switch_fail.ts`](crates/ts2rs-cli/tests/fixtures/switch_fail.ts) (`compile_switch_fallthrough_fails`).

---

## 14. Next steps (follow-up backlog)

Consolidated **what to do next**. Items may overlap §1.3 notes, §10–§11, README “Partial” / “Unsupported”, and §1.3 follow-ups — **code wins**; update stale bullets when landing features.

### Toolchain and UX

**Multi-diagnostic collection** (see [README §1.1 — Diagnostics and surface](README.md))

- [x] **Compile pipeline (build + sem)**: [`build_module`](crates/ts2rs-hir/src/build.rs) and [`check_module`](crates/ts2rs-hir/src/sem.rs) collect multiple [`CompileError`](crates/ts2rs-hir/src/error.rs) into [`CompileError::Many`](crates/ts2rs-hir/src/error.rs) (sorted). Top-level declaration / per-function sem errors are aggregated; **monomorphization** may still stop at the first error; [`emit_rust_with_options`](crates/ts2rs-hir/src/codegen.rs) remains fail-fast on first codegen issue.
- [x] **Parser**: [`parse_typescript_file`](crates/ts2rs-parser/src/lib.rs) reports **all** swc [`take_errors()`](crates/ts2rs-parser/src/lib.rs) diagnostics (plus primary `parse_program` error when present), merged and sorted. **Module graph** still returns on the first file parse failure (per-file output can be multi-line).

**Comments vs generated Rust** (see [README §1.1 — Comments](README.md))

- [x] **Span anchors (supported)**: `ts2rs compile --span-comments` sets [`CodegenOptions::span_comments`](crates/ts2rs-hir/src/codegen.rs) to emit `// ts: path:line:col` before statements (§4.3; maps TS **positions**, not TS comment text).
- [ ] **TS source comments** reflected in generated Rust: needs comment/token preservation in the parse pipeline (swc `Program` has no comment nodes); still **not** implemented.

**Project-scale tooling** (see [README — Scope (1.0)](README.md) “Not 1.0” and [Unsupported TypeScript](README.md))

- [ ] **Full `tsconfig`**: `extends`, `include` glob, beyond minimal `--project` JSON with `files` only.
- [ ] **Package resolution** (e.g. `node_modules`, `paths` mapping as in typical TS projects).
- [ ] **`export *` from** and **more `export` shapes** than the supported subset.
- [ ] Keep this list aligned with README “Not 1.0” / unsupported table when scope changes.

### Performance and security (aligned with §10–§11; **parallelism / codegen safety / driver resources** are tracked here)

- [ ] **Incremental compile** (multi-file, only rebuild changed modules; same open item as §10).
- [x] **Parallelize** multi-file semantic checks. (Per-function [`check_function`](crates/ts2rs-hir/src/sem.rs) runs under **`rayon`** `par_iter_mut`; [`SendSourceMap`](crates/ts2rs-hir/src/ir.rs) makes [`IRFunction`](crates/ts2rs-hir/src/ir.rs) `Send` despite `swc` `Lrc`; warning order matches `module.fns`.)
- [x] **Generated-code safety**: string escaping / `println!` injection audit. (Class `__class_name` literal uses `Debug` escaping; [`emit_builtin_log`](crates/ts2rs-hir/src/codegen.rs) documents fixed format templates; template literals already brace-escape in [`emit_tpl`](crates/ts2rs-hir/src/codegen.rs).)
- [x] **Driver resource limits**: optional timeout / memory around `cargo` subprocess. ([`RustBuildOptions::cargo_timeout`](crates/ts2rs-driver/src/lib.rs) + [`max_cargo_output_bytes`](crates/ts2rs-driver/src/lib.rs); [`cargo_build`](crates/ts2rs-driver/src/cargo_runner.rs) uses [`wait_timeout::ChildExt`].)

### Async / HTTP (MVP; residual backlog)

- [x] **`await` in arbitrary control flow** (not limited to current async MVP body rules). (Removed [`check_async_mvp_stmts`](crates/ts2rs-hir/src/sem.rs); [`infer_expr_mut`](crates/ts2rs-hir/src/sem.rs) `Await` accepts any `Promise<T>` operand; [`async_control_flow_ok.ts`](crates/ts2rs-cli/tests/fixtures/async_control_flow_ok.ts), `compile_async_control_flow_if_while_await_ok`.)
- [x] **`Promise.all([...])`** (array literal only; homogeneous `Promise<number>` / `Promise<string>` / `Promise<Response>` from `fetch`). ([`IRExpr::PromiseAll`](crates/ts2rs-hir/src/ir.rs), [`promise_all_fetch_ok.ts`](crates/ts2rs-cli/tests/fixtures/promise_all_fetch_ok.ts), `compile_promise_all_fetch_alias_ok`.)
- [x] **`fetchText(url)`** — `Promise<string>` via [`IRExpr::FetchText`](crates/ts2rs-hir/src/ir.rs) / `__ts2rs_fetch_text`.
- [x] **`fetch(url, init?)`** — `Promise<Response>` via [`IRExpr::Fetch`](crates/ts2rs-hir/src/ir.rs) / `__ts2rs_fetch` + [`__Ts2rsFetchInit`](crates/ts2rs-hir/src/codegen.rs); Response members and `text`/`json` as above; fixtures [`fetch_response_ok.ts`](crates/ts2rs-cli/tests/fixtures/fetch_response_ok.ts), [`fetch_post_init_ok.ts`](crates/ts2rs-cli/tests/fixtures/fetch_post_init_ok.ts).
- [x] **Streaming response body (M3)** — `response.body.getReader()` / `await reader.read()` → `StreamReadResult` (`done`, `value` as `Uint8Array`); `reqwest::Response::bytes_stream()`; semantic mutex with `.text()` / `.json()`; built-in type names `HttpResponse`, `ReadableStreamDefaultReader`, `StreamReadResult`, `Uint8Array`; fixture [`fetch_stream_ok.ts`](crates/ts2rs-cli/tests/fixtures/fetch_stream_ok.ts), test `compile_fetch_stream_ok`.
- [x] **Reject `.then`** calls with a clear diagnostic (`Promise.prototype.then` is not supported). ([`build.rs`](crates/ts2rs-hir/src/build.rs) call lowering, [`promise_then_fail.ts`](crates/ts2rs-cli/tests/fixtures/promise_then_fail.ts), `compile_promise_then_fails`.)
- [x] **TLS / HTTP stack (documented, not “Node parity”)**: generated crates use **reqwest** with **`rustls-tls`** ([`crate_writer.rs`](crates/ts2rs-driver/src/crate_writer.rs)). **TLS 1.2+** and **HTTP/2** (when the server and stack negotiate ALPN) are provided by that stack; **root stores, cipher suites, and HTTP/2 prioritization are not guaranteed to match any specific Node or browser version**. **Still backlog**: full **WHATWG** `fetch` (browser `ReadableStream` / `Request`/`Headers` objects, duplex, CORS in non-browser hosts, etc.); trust subset already supports **chunked body** via `getReader`/`read` (see streaming item above).

### Language and typing (trust strong-typing subset)

- [x] **Optional call** `f?.()`; **static narrowing** for `??` / `?.` (decidable; §3.3). Done: [`OptionalCall` / `OptionalMethodCall`](crates/ts2rs-hir/src/ir.rs), [`build_opt_chain_call_expr`](crates/ts2rs-hir/src/build.rs)（含 `Expr::OptChain` callee）；[`optional_call_ok.ts`](crates/ts2rs-cli/tests/fixtures/optional_call_ok.ts)、[`optional_chain_fail.ts`](crates/ts2rs-cli/tests/fixtures/optional_chain_fail.ts)（非标识符 callee）；[`NullishCoalesce`](crates/ts2rs-hir/src/sem.rs) 扩展 **同族** `Union` 去 `null`/`undefined` 后与右操作数合并。**完整** discriminated / 全联合收窄仍属后续。
- [x] **Chained member/calls** `f().g()`（一层 `expr.prop` / `expr.m()`）。Done: [`chain_call_ok.ts`](crates/ts2rs-cli/tests/fixtures/chain_call_ok.ts)，`run_chain_call_ok_prints_six`；一般实例方法类型仍见 §1.3 follow-ups。
- [x] **Numeric model**: 全局 `number` → Rust **`f64`**（[`IRExpr::Number(f64)`](crates/ts2rs-hir/src/ir.rs)，[`rust_ty_scalar`](crates/ts2rs-hir/src/codegen/helpers.rs)，`Math`/`Number`/`JSON.parse` / `indexOf` 等）；下标与 UTF-16 内部仍 `as i32`。破坏性：原 `i32` 截断语义不再；与 Node/IEEE 细节见 README。
- [x] **HIR stdlib / JSON / strings**: `JSON.parse` — **string literal** args fold at build time via `serde_json` to trust-closed IR (`number` / `boolean` / `string` / `null` / homogeneous `number[]` \| `string[]` / flat `{ k: number }`); **dynamic** arg stays JSON **number** document → `f64` via **`serde_json::from_str`** (same as `await response.json()`). Global **`encodeURIComponent`** / **`decodeURIComponent`** → `urlencoding` (`string` → `string`). Generated `Cargo.toml` injects `serde_json` / `urlencoding` when needed ([`crate_writer.rs`](crates/ts2rs-driver/src/crate_writer.rs)). Fixtures [`json_uri_trust_ok.ts`](crates/ts2rs-cli/tests/fixtures/json_uri_trust_ok.ts), [`json_parse_hetero_array_fail.ts`](crates/ts2rs-cli/tests/fixtures/json_parse_hetero_array_fail.ts); tests `run_json_uri_trust_ok_prints_expected`, `compile_json_uri_trust_ok_emits_serde_json_and_urlencoding`, `compile_json_parse_hetero_array_literal_fails`.

### Documentation and examples

- [x] **README + this file**: periodic sweep so matrix / §1.3 / §2.1 lines match shipped features (e.g. stdlib, `string[i]`, `Math.*` extensions). *(This sweep: §1.3 / §2.1 bullets above + `.json()`/`JSON.parse` serde_json wording; README matrix already lists member/`JSON`/URI/async — see [Language feature matrix](README.md).)*
- [x] **[`test-ts/main.ts`](test-ts/main.ts)**: kept within supported subset; header documents expected I/O and **intentionally omits** `async`/`fetch`/generic call sites (covered by `fixtures/` instead).

---

## Maintenance

- When an item is done, change `[ ]` to `[x]`, or add “completed in commit / PR #”.
- If scope changes, note **alternatives** or **why deprecated** in the item.
- If this list conflicts with the [`README.md`](README.md) language matrix, **code wins** — update the README (and [`README.zh-CN.md`](README.zh-CN.md) as needed).
