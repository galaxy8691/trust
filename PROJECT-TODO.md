[中文](PROJECT-TODO.zh-CN.md)

# trust long-term project TODO

This document tracks compiler and toolchain work over time, grouped by theme. Use `[ ]` / `[~]` / `[x]` in PRs or commits as appropriate. **Every item assumes the strong-typing (trust) model.**

**Code entry points**: [`README.md`](README.md) · [`crates/trust-hir`](crates/trust-hir) (`build.rs` / `sem.rs` / `codegen.rs` / `ir.rs`) · [`crates/trust-parser`](crates/trust-parser) · [`crates/trust-driver`](crates/trust-driver) · [`crates/trust-cli`](crates/trust-cli) · [`test-ts/main.ts`](test-ts/main.ts) (multi-file: [`test-ts/math.ts`](test-ts/math.ts)) · [`crates/trust-cli/tests/fixtures/`](crates/trust-cli/tests/fixtures/)

Chinese mirror: [`PROJECT-TODO.zh-CN.md`](PROJECT-TODO.zh-CN.md).

**Follow-up backlog (what to do next)**: [§14 Next steps](#14-next-steps-follow-up-backlog) — consolidated; older sections may still mention the same work in passing.

### Planning constraint: trust (strong typing)

**trust is strongly typed; there is no soft typing.** Long-term items and PR trade-offs must stay consistent: only extend syntax that can get **static** rules in HIR / [`sem.rs`](crates/trust-hir/src/sem.rs). **Do not** target implicit `any`, runtime reshaping, or un-annotated widen-in as goals. See [README — Trust: strong typing](README.md).  
Here, “narrowing”, “assignable”, and “structural / shape” mean **static rules inside HIR / sem**, not runtime reshaping, and not aligning with `tsc`’s default loose or progressive soft typing.

---

## 0. Vision and “1.0” acceptance (editable)

- [x] **Single-file subset**: every feature the README matrix marks as supported has fixtures and integration tests ([`crates/trust-cli/tests/fixtures/`](crates/trust-cli/tests/fixtures/) + [`cli_e2e.rs`](crates/trust-cli/tests/cli_e2e.rs)); `trust-lower` also has compile unit tests.
- [x] **Diagnostics**: common errors include line/column (`path:line:col`); messages are **English** (see README “Scope (1.0)”).
- [x] **Reproducible**: `cargo test --workspace`, `cargo clippy --workspace --all-targets`; [`.github/workflows/ci.yml`](.github/workflows/ci.yml) on push/PR.
- [x] **Multi-file (if in scope)**: **not** full project graphs for 1.0; **relative** `import { x } from "./dep.ts"` uses [`parse_module_graph`](crates/trust-parser/src/module_graph.rs) (no merged AST), CLI and [`compile_entrypoint_to_executable`](crates/trust-driver/src/lib.rs) via `validate_imports` → `lower_module_graph`; see §6.2.

---

## 1. Frontend: parsing and AST coverage

### 1.1 Robustness on supported paths

- [x] **Error recovery**: **single diagnostic** today (fail on first); multi-diagnostic collection is future work; see [README — Diagnostics (§1.1)](README.md).
- [x] **Preserving comments**: [`parse_typescript_file`](crates/trust-parser/src/lib.rs) populates [`ParsedSource::comments`](crates/trust-parser/src/lib.rs) (swc); frozen into HIR and optionally emitted as Rust `//` lines ([§14 — Comments vs generated Rust](PROJECT-TODO.md)); see README §1.1.
- [x] **`export` variants**: `export function`, top-level `function`, relative **`export * from "./…"`** / **`export { … } from "./…"`** (value re-exports; HIR skips); **`export default function main`**, **`export default async function main`**, and **`export default main`** when `main` is a top-level function (see §13.6); other `export default` shapes still rejected ([`build.rs`](crates/trust-hir/src/build.rs), [`module_graph.rs`](crates/trust-parser/src/module_graph.rs)); negatives `export_*_fail.ts` + `cli_e2e`.

### 1.2 Statements and declarations

- [x] **`import`**: relative `import { f } from "./x.ts"` via module graph ([`module_graph.rs`](crates/trust-parser/src/module_graph.rs)); old [`resolve_imports.rs`](crates/trust-parser/src/resolve_imports.rs) removed; non-relative paths still error (`import_fail.ts`).
- [x] **Nested `function`**: [`IRStmt::FnDecl`](crates/trust-hir/src/ir.rs) + no-capture subset; see `nested_fn.ts`.
- [x] **`const`**: aligned with `let`; reassignment forbidden; `const_ok.ts`, `const_reassign_fail.ts`.
- [x] **Assignment in expression statements**: `IRStmt::Assign` + mutable `let`; `assign_simple.ts`.
- [x] **`for` / `do-while`**: C-style `for` (including update assign), `do-while`; **`switch`**: lowered to `If` chain in `build` (see §13.5, `switch_ok.ts`).
- [x] **`break` / `continue`**: inside loops; no labels.
- [x] **Empty statement / blocks**: `Stmt::Empty`, `Block`; `empty_stmt.ts`.

### 1.3 Expressions

- [x] **`async` / `await` / HTTP `fetch` / `fetchText` (MVP)**: [`IRFunction::is_async`](crates/trust-hir/src/ir.rs), [`IRExpr::Await`](crates/trust-hir/src/ir.rs) / [`FetchText`](crates/trust-hir/src/ir.rs) / [`Fetch`](crates/trust-hir/src/ir.rs) / [`PromiseAll`](crates/trust-hir/src/ir.rs) (TS surface: **`async_all([...])`**), [`#[tokio::main]`](crates/trust-hir/src/codegen.rs), driver injects [`tokio` + `reqwest`](crates/trust-driver/src/crate_writer.rs) and **`futures-util`** when generated Rust uses streaming (`crate_writer` detects `futures_util` in source). **`await` in arbitrary control flow** is implemented; **no user `Promise<T>`** — `async function` annotations use the **awaited** type `T` (`number` / `string` / `void`); builtins **`fetchText`**, **`fetch`**, **`readFileTextAsync`**, **`async_all`** are await-only; **`response.body.getReader()`** + **`await reader.read()`** (chunked body via `bytes_stream()`), **optional `init`** on `fetch`; **`.then` / `Promise.all` / `Promise<T>`** are **rejected or absent** on the TS surface ([§13.8](PROJECT-TODO.md)); **`Headers` iteration / Web `Request` parity / byte-level TLS·HTTP2 parity with Node** remain out of scope (see §Async / HTTP backlog).
- [x] **Member access and call chains**: `string.length` (UTF-16), `string[i]` (single UTF-16 code unit as `string`), `number[]` / `string[]` index, `length` on objects; **`obj.m(args)`** → global `m(receiver,…)` ([`IRExpr::MethodCall`](crates/trust-hir/src/ir.rs)); **one-level** `f().prop` / `f().m()` ([`chain_call_ok.ts`](crates/trust-cli/tests/fixtures/chain_call_ok.ts)); optional **`?.` / `f?.()` / `recv?.m()`** ([`optional_call_ok.ts`](crates/trust-cli/tests/fixtures/optional_call_ok.ts)); fixtures `member_length_ok.ts`, `method_call_ok.ts`, `string_utf16_length.ts`, `stdlib_hir_ok.ts`.
- [x] **Optional chaining / nullish coalescing**: limited subset (`obj?.prop`, `??`; `optional_ok.ts`, `nullish_ok.ts`); full semantics tied to §3.3.
- [x] **Logical short-circuit**: `&&`, `||`; `boolean` and `number` truthiness (`!= 0`), result type `boolean` (`logical_bool.ts`, `logical_truthy_ok.ts`); differs from TS value-preserving `&&`/`||`; under **strong typing** result is `boolean`; more complex truthiness or unions still limited.
- [x] **Ternary**: `cond ? a : b` (`ternary_ok.ts`).
- [x] **Comma expression**: `comma_ok.ts`.
- [x] **Template literals**: no tag; `template_ok.ts`.
- [x] **Array / object literals**: limited subset (`number[]`, `{ k: number }`; `array_ok.ts`, `object_ok.ts`); runtime and full types in §1.4 / §2.1.

**§1.3 follow-ups (notes)**

- **Method / chain typing**: `obj.m` and one-level `f().g` are implemented; **richer receiver typing** (e.g. arbitrary class instance methods) remains limited — see class subset in README matrix.
- **`??` / `?.`**: same-family `Union` narrowing for `??` and optional call/member are implemented; **discriminated narrowing** implemented (D1; §3.3.1).
- **“Full” types for array/object literals**: richer elements/fields and `TsType`/IR evolution in §1.4, §2.1 — not only expression layer.

### 1.4 Type syntax (types only)

**Summary**

- [x] **Literal types**, **union types**, **`interface`**, **`type` aliases**: aligned with **strong-typing** checker roadmap (sub-items below; **literal types**, **primitive/literal unions**, **limited `interface`→`ObjectNum`**, **limited `type` alias→named table**). **Generics** are a separate sub-item (document “still rejected” milestone, not semantics).

**Relation to implemented subset**: §1.3 supports limited annotations `number[]`, `{ k: number }` ([`TsType::ArrayNumber`](crates/trust-hir/src/ir.rs) / [`ObjectNum`](crates/trust-hir/src/ir.rs)). **Literal types** (`NumberLit` / `StringLit` / `BoolLit`) and **unions** ([`TsType::Union`](crates/trust-hir/src/ir.rs) + normalization) below; **top-level `interface`** is nominal `ObjectNum` in the type layer (same rules as object type literals); **top-level `type` aliases** via [`collect_named_types_with_errors`](crates/trust-hir/src/build/build_types.rs) into the same named table; **generic semantics** still not implemented — rejection table in [README §1.4](README.md) and sub-items below; full object/interface shapes and IR in §2.1; **static** null and branch narrowing crosses §3.3.

**Sub-items**

- [x] **Literal types** (e.g. `42`, `"a"`, `true` in type position)  
  - **Deps**: extend [`TsType`](crates/trust-hir/src/ir.rs); **static** assignability to base types (with §3.3 explicit shapes / sem rules, not full TS structural subtyping).  
  - **Done**: [`build.rs`](crates/trust-hir/src/build.rs) parses `TsLitType`; [`sem.rs`](crates/trust-hir/src/sem.rs) `type_assignable` / literal inference; `literal_type_ok.ts`, `literal_type_fail.ts` + [`cli_e2e.rs`](crates/trust-cli/tests/cli_e2e.rs).

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
  - **Deps**: monomorphization or limited strategy still **future**; rejection and English diagnostics in [README §1.4 — Generics](README.md), [`build.rs`](crates/trust-hir/src/build.rs) generic checks.  
  - **Done**: document “still rejected” in stages — [README](README.md) table + `generic_function_fail`, `interface_generic_fail`, `type_alias_generic_fail` + e2e; **no** generic semantics in this milestone.

---

## 2. IR (`ir.rs`) evolution

### 2.1 Current structure

- [x] **Statements**: `Assign`, `Break`, `Continue`, `DoWhile`, `FnDecl`, `Empty`; `for` lowered to `while`; no `Switch` IR stmt.
- [x] **Expressions**: `LogicalAnd`/`LogicalOr`, `Conditional`, `Seq`, `Tpl`, `Member` / `OptionalMember`, [`Index`](crates/trust-hir/src/ir.rs) (array `number`/`string` elements, string UTF-16), [`MethodCall`](crates/trust-hir/src/ir.rs) / [`OptionalMethodCall`](crates/trust-hir/src/ir.rs), one-level chained `f().prop` / `f().m()`, [`JsonBuiltin`](crates/trust-hir/src/ir.rs) / [`UriBuiltin`](crates/trust-hir/src/ir.rs), math/string/http builtins as in README matrix; **computed** `obj[expr](…)` call still unsupported. `ObjectNum` / `interface` shapes: §1.4; strong typing, static checks.
- [x] **Top level**: multi-file graph — `parse_module_graph` + `validate_imports`; HIR merged to [`IRModule`](crates/trust-hir/src/ir.rs) (`build_program_multi` / `compile_graph`); `main` in entry file; global function names unique; negatives `import_missing_export_*`, `circular_*`, `dup_*`.

### 2.2 Metadata and debugging

- [x] **Span**: HIR nodes carry swc `Span`; [`diag`](crates/trust-hir/src/error.rs) and codegen errors use function `cm` + `source_path` + node `span` (see [`ir.rs`](crates/trust-hir/src/ir.rs) module docs); whole-file `span` if no top-level function in `build`; `sem` missing `main` anchors to first function `span`.
- [x] **Optional**: [`IRFunction::ir_id`](crates/trust-hir/src/ir.rs) (per function, including nested; monotonic per compile, see `build_fn`).

---

## 3. Semantic analysis (`sem.rs`)

### 3.1 Solidifying what exists

- [x] **Symbol table**: block scope and duplicate `let` (partial) — fixtures for nesting/shadowing: `let_dup_same_block_fail.ts`, `let_shadow_nested_ok.ts`, `param_let_same_name_fail.ts` + [`cli_e2e.rs`](crates/trust-cli/tests/cli_e2e.rs).
- [x] **Control flow**: simplified `stmts_return` — documented vs TS/tsc (**trust only guarantees static rules**; README). Done: [README — Control flow and return](README.md).
- [x] **`void` and `console.log`**: `BuiltinLog` void paths covered; branch-only-log case `void_log_in_branch.ts` + e2e.

### 3.2 Mutability and assignment

- [x] **Mutable `let`**: `IRStmt::Assign`; LHS must be a bound identifier.
- [x] **No assignment to `const`**.

### 3.3 Deeper type system

- [x] **Tie-in with §1.4**: literal and union types and **static** `??` / `?.` narrowing must stay consistent with §1.4 (avoid conflict with limited `TsType`). Unions are in HIR. **Implemented (sem)**: when `Union` minus `null`/`undefined` matches the right-hand side of `??` as the same “family” (`number` / `string` / `boolean`, or **structurally matching** `Fn`), [`infer_expr_mut`](crates/trust-hir/src/sem.rs) on `IRExpr::NullishCoalesce` calls `unify_ternary_branches` for a single result type. **Completed (D1)**: discriminated-union narrowing that relies on a **discriminant** via `Binding::narrow_ty` (§3.3.1). Done: README §3.3; `nullish_ok` / `optional_ok`; sem + `Fn` unions covered by `nullish_fn_ok.ts` (`trust check`).
- [x] **`null` / `undefined`**: [`TsType`](crates/trust-hir/src/ir.rs) has `Null`/`Undefined` variants; checks follow **current sem static rules**, not tsc’s default “everything nullable”. Done: README §3.3; **no** `strictNullChecks`-style switch. If added later, make it an **explicit** compiler mode, not implicit JS looseness.
- [x] **Structural vs nominal**: **trust** uses nominal table + static shape checks; Rust mapping strategy (mostly primitives today). Done: README §3.3; **not** implementing full TS structural subtyping as a goal.
- [x] **Function types and HOF**: Align with [§13.2](PROJECT-TODO.md) and README — a **restricted** subset is implemented (function types, arrow values, calls, passing/returning functions); closure codegen remains the strict `(number) => number` subset. **Do not** claim “no first-class functions / no HOF”; distinguish “supported HOF subset” vs “further generalization / codegen work”.

### 3.3.1 Extension roadmap — pillar pick and specifications (implementation TBD)

Single-PR rule: land **one** pillar at a time; each needs fixtures + README §3.3 / matrix touch-up.

**Recommended order**

1. **D1** — discriminated narrowing (localized to `sem`, high leverage for `if` + unions).
2. **R1** — nominal `interface` methods (build + sem; may touch codegen for call shape).
3. **G1 / G2** — broader monomorphization surface (touches `sem/mono.rs` + build rejects).
4. **C1 / C2** — TS comment fidelity (mostly `codegen` + parser comment maps; weak coupling to sem).

#### D1 — Discriminated union narrowing (spec)

- **Goal**: Inside `if (v.tag === 'a')` / `else`, refine static type of `v` when `v` is a `Union` of `ObjectNum`-like arms that share a **required** discriminant field (same name on each arm) with **pairwise-distinct literal** types (`string` / `boolean` / `number` literals only for v0).
- **Condition shape (v0)**: `Binary` with `Eq` / `StrictEq`, left = `Member` or `OptionalMember` on identifier `v` + literal property name, right = **literal** matching one arm’s discriminant value. (Computed member / arbitrary expr left side: later.)
- **Semantics**: In **then** branch, bind `v` (and copies?) to narrowed object type; **else** optional v0.1: narrow to remaining union arms. Merge with existing `??` / `?.` rules without breaking `nullish_ok.ts`, `optional_ok.ts`, `nullish_fn_ok.ts`.
- **Implementation sketch**: flow-sensitive map `ident -> TsType` per block stack in [`check_stmts`](crates/trust-hir/src/sem.rs) / `check_stmt` for `IRStmt::If`; or optional `narrow_ty` on [`Binding`](crates/trust-hir/src/sem.rs) entries.
- **Fixtures (planned)**: `discriminated_narrow_ok.ts`, `discriminated_narrow_else_ok.ts`, `discriminated_narrow_non_literal_fail.ts`; e2e `run_*` / `compile_*`.

#### G1 / G2 — Generics subset expansion (spec)

- [x] **G1 (explicit args)**: Nested generic calls with explicit type args `f<T>(g<U>(x))` supported.
- [x] **G2 (inference)**: Extended `synth_expr_ty` to infer from non-generic function return types; e.g., `id(getNumber())` where `getNumber(): number` infers `T=number`.
- [x] **Implementation**: Added `fn_ret_types: HashMap<String, TsType>` throughout monomorphization pipeline to enable function return type lookup during type argument inference.
- [x] **Fixtures**: `generic_fn_return_infer_ok.ts`, `generic_nested_call_ok.ts`, `generic_multi_param_ok.ts`.
- **Still out of scope**: Higher-kinded types, `extends` constraints, default type params, inference from heterogeneous unions, call-site partial application of type args.
- **Regression**: retained [`generic_function_*_fail.ts`](crates/trust-cli/tests/fixtures/), [`compile_generic_function_multi_infer_fail_reports_multiple_errors`](crates/trust-cli/tests/cli_e2e.rs).

#### R1 — `interface` instance methods (nominal; spec)

- [x] **Syntax subset (v0)**: Inside top-level `interface I { foo(): number; bar(x: number): void; }` — **no** overloads, **no** generic methods.
- [x] **Lowering strategy**: Global desugar `foo__I(receiver: &IShape, …)` via `inherent_rust` field in `IRExpr::MethodCall`; receiver type is `ObjectNum` with method signatures in `ObjectMemberKind::Method`.
- [x] **Interaction**: `obj.m(args)` checks interface method signature first, falls back to global function if not found; does not break existing method calls.
- [x] **Fixtures**: `interface_method_ok.ts`, `interface_method_bad_args_fail.ts`.

#### C1 / C2 — TS source comments in Rust (spec)

- **C1 (documentation)**: See README §1.1 — leading comments attach by **HIR statement span start**; many lowerings move or split spans.
- **C2 (future)**: Trailing / interior comments need range lookup (`span.lo` / `span.hi` vs swc comment tables) and policy for `switch`→`if`, `for`→`while`, etc.
- **Optional future warning**: emit [`CompileWarning`](crates/trust-hir/src/error.rs) when `--ts-source-comments` is on and a file has frozen comments that **no** emitted stmt consumed (needs cheap global pass); not implemented yet.

### 3.4 Control-flow analysis (advanced)

- [x] **Reachability**: unreachable warnings (`warning: path:line:col: unreachable code`); `early_return_unreachable.ts`, `unreachable_after_return.ts`, `break_unreachable.ts` + [`cli_e2e.rs`](crates/trust-cli/tests/cli_e2e.rs).
- [x] **Definite assignment**: `let x: number;` without init allowed; must assign before use (`if`/`else` merge, conservative loops); `definite_assign_ok.ts`, `definite_assign_if_ok.ts`, negative `definite_assign_fail.ts`.
- [x] **Finer return**: early exhaustive return in a sequence (`if`/`else` both return → following dead code warnings only); `while`/`do-while` still use tail rules (`tail_returns_while_body`); future `switch` may extend `stmt_fn_returns_complete`.

---

## 4. Code generation (`codegen.rs`)

### 4.1 Current behavior

- [x] **`console.log` multi-arg format**: [`emit_builtin_log`](crates/trust-hir/src/codegen.rs) uses spaced `"{}"`; test `console_log_multi_arg_uses_spaced_format`.
- [x] **Arithmetic and `/`**: TS `number` → Rust **`f64`**; **`/`** is IEEE-754 double division (unlike the former `i32` truncating division); README “Arithmetic, `/`, overflow” and matrix.
- [x] **NaN / ∞ and overflow**: possible; not identical to V8 `number` edge cases in every scenario; **no** runtime-checked Cargo feature yet.

### 4.2 Mapping new features

- [x] **Assignment**: `let mut` and blocks match Rust (`codegen_42_let_mut_block_and_assign`; [`emit_stmt`](crates/trust-hir/src/codegen.rs) `Let`/`Assign`/`Block`).
- [x] **Strings**: `String` + `format!`; large strings/perf TBD (`codegen_42_string_concat_uses_format`; [`emit_expr`](crates/trust-hir/src/codegen.rs) `StrConcat`/`Tpl`).
- [x] **Heap / GC**: objects are value `HashMap::from`, **no** `Rc`/`Arc` yet (`codegen_42_object_literal_hashmap_without_rc`; [`emit_expr`](crates/trust-hir/src/codegen.rs) `ObjectLit`).

### 4.3 Readability of emitted Rust

- [x] **Indent / line breaks**: comma (`Seq`) block lines and closing `})` align for rustfmt (`codegen_43_comma_seq_indented`; [`emit_seq_expr`](crates/trust-hir/src/codegen.rs) / `emit_expr` `stmt_level`).
- [x] **Comments**: optional `// ts: path:line:col` per statement (`codegen_43_span_comments_emits_ts_anchors`; `trust-cli` `compile_span_comments_writes_ts_anchors`; [`emit_stmt`](crates/trust-hir/src/codegen.rs), `CodegenOptions`; `trust compile --span-comments`). Optional TS leading comment text as Rust `//` lines (`emit_ts_source_comments_writes_frozen_leading`; `compile_ts_source_comments_writes_ts_text`; `CodegenOptions::emit_ts_source_comments`; `trust compile --ts-source-comments`).

---

## 5. Builtins and std mapping

### 5.1 `console`

- [x] **`console.log` / `console.error` / `console.debug`**: `log` → `println!`, `error`/`debug` → `eprintln!` (`console_error_and_debug_use_eprintln`; `compile_console_stderr_writes_eprintln`; [`build.rs`](crates/trust-hir/src/build.rs); [`emit_builtin_log`](crates/trust-hir/src/codegen.rs)).
- [x] **Formatting**: same as §4.1, spaced `"{}"`; shared [`emit_builtin_log`](crates/trust-hir/src/codegen.rs).

### 5.2 Minimal runtime ([`trust_rt`](crates/trust_rt))

- [x] **Strings**: `string.length` is **UTF-16 code units** (`encode_utf16().count()`); `number[].length` → `Vec::len`; object field `length` via `HashMap::get` ([`MemberLengthDispatch`](crates/trust-hir/src/ir.rs)) (`codegen_52_string_length_utf16`, `codegen_52_object_length_field_uses_get`; CLI tests). **`string` subscript `s[i]`**: UTF-16 index → single-code-unit `string` ([`IndexKind::StringUtf16`](crates/trust-hir/src/ir.rs); `stdlib_hir_ok.ts`).
- [x] **Math**: `Math.abs` / `min` / `max` / `floor` / `ceil` / `sign` / `trunc` / `round` / `pow` etc. lower to **`f64`** operations in codegen ([`MathBuiltinKind`](crates/trust-hir/src/ir.rs); [`build.rs`](crates/trust-hir/src/build.rs); [`emit_expr`](crates/trust-hir/src/codegen.rs); matches README matrix `Math.*` row).
- [x] **HIR stdlib (no `trust_rt` required)**: `Number.parseInt` / `parseFloat`; `String` methods `charAt`, `charCodeAt`, `slice`, `substring`, `indexOf`, `includes`; global `readLine()` via inlined `std::io` (rejected in `async` bodies). Default implementation now routes through **`trust_stdlib`** facade APIs (`json` / `uri` / `string`) with `--stdlib-mode legacy` fallback for inline helpers / direct `serde_json` + `urlencoding`; fixtures [`stdlib_hir_ok.ts`](crates/trust-cli/tests/fixtures/stdlib_hir_ok.ts), [`json_uri_trust_ok.ts`](crates/trust-cli/tests/fixtures/json_uri_trust_ok.ts).
- [x] **I/O**: [`trust_rt::read_stdin_line`](crates/trust_rt/src/lib.rs) placeholder (`std::io`); **optional** — sync `readLine()` is emitted in generated Rust **without** linking `trust_rt`; driver temp crate still does not depend on `trust_rt` unless `--link-trust-rt`.

---

## 6. Driver and build ([`trust-driver`](crates/trust-driver))

### 6.1 Single-file path

- [x] **Temp directory lifecycle**: documented [`TempDir`](https://docs.rs/tempfile) drop and `(TempDir, PathBuf)` return in [`compile_entrypoint_to_executable`](crates/trust-driver/src/lib.rs) / [`build_rust_to_executable`](crates/trust-driver/src/lib.rs) / [`build_rust_and_copy`](crates/trust-driver/src/lib.rs) (`lib.rs` docs; `cargo test --workspace`).
- [x] **Offline / no cargo**: [`DriverError::CargoNotFound`](crates/trust-driver/src/lib.rs) on `NotFound`; build failures → [`DriverError::CargoBuild`](crates/trust-driver/src/lib.rs) with stdout/stderr (`map_cargo_spawn_error_maps_not_found_to_cargo_not_found`).

### 6.2 Multi-file and modules ([`compile_entrypoint_to_executable`](crates/trust-driver/src/lib.rs))

- [x] **Multi-root parsing**: [`parse_module_graph_with_extra_roots`](crates/trust-parser/src/module_graph.rs); CLI multiple `.ts` or `--project` + simplified JSON (`extends`, `files`, `include` / `exclude`) via [`tsconfig_resolve`](crates/trust-cli/src/tsconfig_resolve.rs) ([`trust-cli`](crates/trust-cli/src/main.rs)) (`extra_root_includes_unreachable_file`; `run_multi_entry_extra_roots_prints_main`, `run_project_tsconfig_prints_main`, `run_project_tsconfig_extends_include_ok`).
- [x] **Dependency graph (subset)**: entry + relative `import` → [`parse_module_graph`](crates/trust-parser/src/module_graph.rs) (per-module AST) → `validate_imports` → [`lower_module_graph`](crates/trust-lower/src/lib.rs) → one Rust crate.
- [x] **Generating `Cargo.toml`**: [`RustBuildOptions`](crates/trust-driver/src/lib.rs) / [`build_rust_to_executable_with_options`](crates/trust-driver/src/lib.rs); optional path dep `trust_rt` + feature; CLI `--link-trust-rt` (`write_minimal_crate_with_link_trust_rt_contains_optional_path_dep`; `run_with_link_trust_rt_prints_main`).
- [x] **Cycles**: [`parse_module_graph`](crates/trust-parser/src/module_graph.rs) detects and errors (`circular_*.ts`).

---

## 7. CLI ([`trust-cli`](crates/trust-cli))

- [x] **Subcommands**: `compile` / `run` / `check`; README CLI table and `trust --help`; `check` is HIR+sem only ([`check_module_graph`](crates/trust-lower/src/lib.rs)) (`check_sample_ok`, `check_switch_fail_stderr`).
- [x] **Flags**: `compile -o`; `run` `-O`/`--release` and `--debug` ([`RustBuildOptions::release`](crates/trust-driver/src/lib.rs)); global `-q`/`--quiet`, `--color`, `--emit-ir` (`compile_emit_ir_stderr_contains_ir_module`, `debug_build_writes_binary_under_target_debug`).
- [x] **Exit codes**: as README; `run` forwards child `ExitStatus::code` (else `1`); trust errors `1` ([`main.rs`](crates/trust-cli/src/main.rs) `exit_code_for_failed_child`).

---

## 8. Testing and quality

### 8.1 Integration ([`cli_e2e.rs`](crates/trust-cli/tests/cli_e2e.rs) / `fixtures/`)

- [x] **Each matrix row** has a minimal fixture (or one large file with section comments). Done: README “[Matrix vs integration tests](README.md#matrix-vs-integration-tests)” maps rows to `fixtures/` + `cli_e2e`; extras `array_fail`, `optional_chain_fail`, `nullish_fail`, `object_fail`.
- [x] **Regression**: known bugs pinned under [`tests/regression/*.ts`](crates/trust-cli/tests/regression/) ([`tests/regression/README.md`](crates/trust-cli/tests/regression/README.md), [`switch_fallthrough_regression.ts`](crates/trust-cli/tests/regression/switch_fallthrough_regression.ts), `regression_switch_fallthrough_check_fails`).

### 8.2 Unit tests

- [x] **`trust-hir`**: `build`/`sem`/`codegen` `#[cfg(test)]` (`build_module_records_main`, `check_module_accepts_simple_main` / `check_module_rejects_missing_return`, `emit_rust_contains_ts_main_and_println`; dev-dep `trust-parser`).
- [x] **parser**: swc wrapper snapshots/minimal cases ([`lib.rs`](crates/trust-parser/src/lib.rs) `parse_rejects_unclosed_function_body`, `parses_module_with_import_and_export_main`).

### 8.3 Tooling

- [x] **CI** (GitHub Actions): `cargo test`, `clippy`, fmt ([`.github/workflows/ci.yml`](.github/workflows/ci.yml): rustfmt+clippy components, `cargo fmt --all --check` → `cargo test --workspace` → `cargo clippy --workspace --all-targets`).
- [x] **Fuzzing (optional)**: random AST mutations do not panic ([`parse_fuzz_inputs_do_not_panic`](crates/trust-parser/src/lib.rs) on `parse_typescript_file`).

---

## 9. Documentation and developer experience

- [x] **README**: matrix synced with implementation; “unsupported TS” summary (**also describes trust strong-type rejection**). **Done**: [`README.md`](README.md) (English default) and [`README.zh-CN.md`](README.zh-CN.md) — **Unsupported TypeScript (trust rejection boundary)** / **不支持的 TypeScript 特性（trust 强类型拒斥边界）** and language matrix.
- [x] **Architecture diagram**: parse → HIR → sem → codegen → driver (Mermaid). **Done**: Mermaid `flowchart LR` under **Architecture** / **架构** in both READMEs (`trust_parser` → HIR → sem → codegen → `trust_lower` → `trust_cli` / `trust_driver`).
- [x] **Contributing**: `CONTRIBUTING.md` (branch, test commands, MSRV). **Done**: [`CONTRIBUTING.md`](CONTRIBUTING.md) / [`CONTRIBUTING.zh-CN.md`](CONTRIBUTING.zh-CN.md); root [`Cargo.toml`](Cargo.toml) `rust-version = "1.74"` and per-crate `rust-version.workspace = true`.
- [x] **Changelog**: `CHANGELOG.md` for releases. **Done**: [`CHANGELOG.md`](CHANGELOG.md) / [`CHANGELOG.zh-CN.md`](CHANGELOG.zh-CN.md), Keep a Changelog with `[Unreleased]` and `[0.1.0]`.
- [x] **This roadmap**: English default [`PROJECT-TODO.md`](PROJECT-TODO.md), Chinese [`PROJECT-TODO.zh-CN.md`](PROJECT-TODO.zh-CN.md), cross-linked at the top of each file.

---

## 10. Performance and scale (later)

Multi-file **parallel semantic checking** is implemented (`rayon`; details under **§14 — Performance and security**). This section keeps only what is still open.

- [x] **Incremental compile**: multi-file, recompile only changed modules (opt-in `--incremental` on `compile` / `run`; HIR fragment cache under configurable dir, default `.trust-cache`; importers of changed files rebuilt). Implementation: [`incremental.rs`](crates/trust-cli/src/incremental.rs), [`ir_cache`](crates/trust-hir/src/ir_cache/mod.rs), [`forward_deps`](crates/trust-parser/src/module_graph.rs). Tests: `compile_incremental_rebuilds_only_changed_module`, `module_fragment_round_trip_bincode`.

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
- [x] **HIR**: no `IRStmt::Switch`; `switch` lowered in [`build.rs`](crates/trust-hir/src/build.rs) to nested [`IRStmt::If`](crates/trust-hir/src/ir.rs) + [`IRExpr::Binary`](crates/trust-hir/src/ir.rs) `Eq` (same idea as §2.1).
- [x] **sem**: reuses `if` conditions and `Binary` `Eq`; no dedicated `switch` arm analysis.
- [x] **codegen**: same `If`/`Eq` emission; no dedicated `match` for `switch`.
- [x] **Tests**: positive [`switch_ok.ts`](crates/trust-cli/tests/fixtures/switch_ok.ts) (`run_switch_ok_prints_seven`, `compile_switch_ok_writes_rust`); negative [`switch_fail.ts`](crates/trust-cli/tests/fixtures/switch_fail.ts) (`compile_switch_fallthrough_fails`).

### 13.6 `export default` (staged; not full `tsc`)

Trust keeps a **callable entry named `main`**. Default export is supported only when it is equivalent to that contract.

- [x] **A1 — default function**: `export default function main` / `export default async function main` → same IR as `export function main` ([`build.rs`](crates/trust-hir/src/build.rs)); module graph records `main` as export ([`module_graph.rs`](crates/trust-parser/src/module_graph.rs)); fixtures `export_default_function_main_ok.ts`, `export_default_async_main_ok.ts` + `cli_e2e`.
- [x] **A2 — default references `main`**: `export default main` when a top-level `function main` exists; validated after scan ([`build.rs`](crates/trust-hir/src/build.rs)); fixture `export_default_main_ref_ok.ts`.
- [x] **Default import**: `import main from "./dep.ts"` requires binding name `main` and target default export `main` ([`import_utils.rs`](crates/trust-parser/src/import_utils.rs)); negative `import_default_wrong_binding_fail.ts`.
- [x] **A3 — arbitrary default expressions** (`export default 42`, `export default () => {}`, anonymous `export default function`, etc.): **explicitly out of scope** (by product decision). Only **A1/A2** shapes are supported; see README [Unsupported TypeScript](README.md) and **`export` shapes** under Diagnostics (§1.1). No plan to add general expression default exports unless the entry contract changes.

### 13.7 Structural typing milestones (not full `tsc`)

**trust** uses static, codegen-friendly rules; **not** full TypeScript structural subtyping.

- [x] **B1 — nested `ObjectNum` + optional props**: [`ObjectProp`](crates/trust-hir/src/ir.rs), [`object_shape_assignable`](crates/trust-hir/src/sem/helpers.rs), object literals as `serde_json::Value` in codegen; fixture `nested_object_ok.ts`.
- [ ] **B2+ — cross-file interface names in type position, callable members on object types, richer `readonly`/index signatures**: **backlog**; README documents current limits and differences from `tsc`.
- **B2a (next milestone)** — cross-file **type-only** / nominal reuse: e.g. `import type { I } from "./dep.ts"` and using `I` in annotations **across files**. **Current boundary**: `import type` and type-only specifiers are **rejected** at import resolution ([`import_utils.rs`](crates/trust-parser/src/import_utils.rs)); negative fixture [`import_type_fail_main.ts`](crates/trust-cli/tests/fixtures/import_type_fail_main.ts) + e2e `compile_import_type_fails`. Implementing B2a requires extending the module graph + merged type table, not only parser tweaks.

### 13.8 Async surface — no user `Promise` / no `.then` (product decision)

**User-visible `Promise<T>`**, **`Promise.all`**, and **`.then` / `.catch` / `.finally`** callback chaining are **not part of trust**. Write **`async function …(): T`** with the **awaited** type `T` (`number` / `string` / `void`); use **`async_all([...])`** for homogeneous parallel awaits (same lowering as former `Promise.all`). The type name **`Promise`** in type position is **rejected** with a diagnostic ([`build_types.rs`](crates/trust-hir/src/build/build_types.rs)). **`.then`** on calls is **rejected** ([`build.rs`](crates/trust-hir/src/build.rs); [`promise_then_fail.ts`](crates/trust-cli/tests/fixtures/promise_then_fail.ts); e2e `compile_promise_then_fails`). HIR still uses an internal awaitable wrapper ([`TsType::Promise`](crates/trust-hir/src/ir.rs)) for codegen only. Same product stance as §13.6 A3: **permanent scope**, not a deferred milestone.

---

## 14. Next steps (follow-up backlog)

Consolidated **what to do next**. Items may overlap §1.3 notes, §10–§11, README “Partial” / “Unsupported”, and §1.3 follow-ups — **code wins**; update stale bullets when landing features. For a **single checkbox list** of remaining gaps (from README + matrix + roadmap discussion), see **[Consolidated product backlog](#consolidated-product-backlog-readme--partial--slow-track)** at the end of this section — work through it incrementally; some rows duplicate narrative elsewhere (e.g. §3.3.1, §13.7).

### Toolchain and UX

**Multi-diagnostic collection** (see [README §1.1 — Diagnostics and surface](README.md))

- [x] **Compile pipeline (build + sem)**: [`build_module`](crates/trust-hir/src/build.rs) and [`check_module`](crates/trust-hir/src/sem.rs) collect multiple [`CompileError`](crates/trust-hir/src/error.rs) into [`CompileError::Many`](crates/trust-hir/src/error.rs) (sorted). Top-level declaration / per-function sem errors are aggregated; **monomorphization** still aborts the rest of `sem` on failure; [`emit_rust_with_options`](crates/trust-hir/src/codegen.rs) remains fail-fast on the first codegen issue. **UX**: mono / codegen errors may append a short English note that further diagnostics can appear after fix + recompile ([`with_monomorphization_followup`](crates/trust-hir/src/error.rs) / [`with_codegen_followup`](crates/trust-hir/src/error.rs)); see README §1.1.
- [x] **Parser**: [`parse_typescript_file`](crates/trust-parser/src/lib.rs) reports **all** swc [`take_errors()`](crates/trust-parser/src/lib.rs) diagnostics (plus primary `parse_program` error when present), merged and sorted. **Module graph** still returns on the first file parse failure (per-file output can be multi-line).

**Comments vs generated Rust** (see [README §1.1 — Comments](README.md))

- [x] **Span anchors (supported)**: `trust compile --span-comments` sets [`CodegenOptions::span_comments`](crates/trust-hir/src/codegen.rs) to emit `// ts: path:line:col` before statements (§4.3; maps TS **positions**, not TS comment text).
- [x] **TS source comments** in generated Rust (opt-in): parser collects swc leading comments into [`ParsedSource::comments`](crates/trust-parser/src/lib.rs); [`build_module`](crates/trust-hir/src/build.rs) / [`build_program_multi`](crates/trust-hir/src/build.rs) freeze them into [`IRModule::ts_comments_by_path`](crates/trust-hir/src/ir.rs); [`CodegenOptions::emit_ts_source_comments`](crates/trust-hir/src/codegen.rs) emits Rust `//` lines before each statement and each top-level function. **Limitations**: leading only (no trailing / per-expression); lowered AST (e.g. large `switch` desugar) may shift or drop placement; [`compile_with_options`](crates/trust-hir/src/lib.rs) passes `None` for comments (no TS text unless using `build_module` / graph with parser output). CLI: `trust compile --ts-source-comments`. Tests: `emit_ts_source_comments_writes_frozen_leading`, `compile_ts_source_comments_writes_ts_text`.

**Project-scale tooling** (see [README — Scope (1.0)](README.md) “Not 1.0” and [Unsupported TypeScript](README.md))

- [x] **Simplified `tsconfig` (CLI `--project`)**: recursive **`extends`**, **`include` / `exclude` glob**, merged **`files`** (still **no** npm / `node_modules`; not full `tsc` merge semantics). Implementation: [`tsconfig_resolve`](crates/trust-cli/src/tsconfig_resolve.rs), [`graph_loader`](crates/trust-cli/src/graph_loader.rs). **`include`-only** projects: matched `.ts` files are sorted; **entry** is lexicographically first — use explicit **`files`** when order matters. Tests: `tsconfig_resolve` unit tests, `run_project_tsconfig_extends_include_ok`.
- [x] **npm / package-manager resolution**: **`node_modules`**, npm packages, and typical `compilerOptions.paths` → package layouts — **explicit non-goal; not planned.** Imports stay **relative** `./x.ts` (and CLI roots / `--project` roots).
- [x] **Relative re-exports**: `export * from "./x.ts"` and `export { a as b } from "./x.ts"` (value exports; **`export * as` / local `export { x }` without `from`** still rejected; **default export** is limited — see §13.6). Effective exports for `validate_imports`: [`effective_exported_function_names_by_path`](crates/trust-parser/src/module_graph.rs); module graph follows re-export edges. HIR skips these statements (no duplicate IR). Test: `validate_import_via_export_star_from`, `run_reexport_export_star_ok`.
- [x] **README / matrix alignment**: “Not 1.0”, unsupported table, and §1.1 updated for the above (this bullet).

### Performance and security (aligned with §10–§11; **parallelism / codegen safety / driver resources** are tracked here)

- [x] **Incremental compile** (multi-file, only rebuild changed modules; same item as §10 — done).
- [x] **Parallelize** multi-file semantic checks. (Per-function [`check_function`](crates/trust-hir/src/sem.rs) runs under **`rayon`** `par_iter_mut`; [`SendSourceMap`](crates/trust-hir/src/ir.rs) makes [`IRFunction`](crates/trust-hir/src/ir.rs) `Send` despite `swc` `Lrc`; warning order matches `module.fns`.)
- [x] **Generated-code safety**: string escaping / `println!` injection audit. (Class `__class_name` literal uses `Debug` escaping; [`emit_builtin_log`](crates/trust-hir/src/codegen.rs) documents fixed format templates; template literals already brace-escape in [`emit_tpl`](crates/trust-hir/src/codegen.rs).)
- [x] **Driver resource limits**: optional timeout / memory around `cargo` subprocess. ([`RustBuildOptions::cargo_timeout`](crates/trust-driver/src/lib.rs) + [`max_cargo_output_bytes`](crates/trust-driver/src/lib.rs); [`cargo_build`](crates/trust-driver/src/cargo_runner.rs) uses [`wait_timeout::ChildExt`].)

### Async / HTTP (MVP; residual backlog)

- [x] **`await` in arbitrary control flow** (not limited to current async MVP body rules). (Removed [`check_async_mvp_stmts`](crates/trust-hir/src/sem.rs); [`infer_expr_mut`](crates/trust-hir/src/sem.rs) `Await` accepts any internal awaitable operand; [`async_control_flow_ok.ts`](crates/trust-cli/tests/fixtures/async_control_flow_ok.ts), `compile_async_control_flow_if_while_await_ok`.)
- [x] **`async_all([...])`** (array literal only; homogeneous awaitables from `fetch` / `fetchText` / …). ([`IRExpr::PromiseAll`](crates/trust-hir/src/ir.rs), [`async_all_fetch_ok.ts`](crates/trust-cli/tests/fixtures/async_all_fetch_ok.ts), `compile_async_all_fetch_alias_ok`.)
- [x] **`fetchText(url)`** — awaitable `string` via [`IRExpr::FetchText`](crates/trust-hir/src/ir.rs) → `trust_stdlib::http::fetch_text` ([`crates/trust-stdlib/src/http.rs`](crates/trust-stdlib/src/http.rs)).
- [x] **`fetch(url, init?)`** — awaitable `HttpResponse` via [`IRExpr::Fetch`](crates/trust-hir/src/ir.rs) → `trust_stdlib::http::fetch` + `trust_stdlib::http::FetchInit`; Response members and `text`/`json` as above; fixtures [`fetch_response_ok.ts`](crates/trust-cli/tests/fixtures/fetch_response_ok.ts), [`fetch_post_init_ok.ts`](crates/trust-cli/tests/fixtures/fetch_post_init_ok.ts).
- [x] **Streaming response body (M3)** — `response.body.getReader()` / `await reader.read()` → `StreamReadResult` (`done`, `value` as `Uint8Array`); `reqwest::Response::bytes_stream()`; semantic mutex with `.text()` / `.json()`; built-in type names `HttpResponse`, `ReadableStreamDefaultReader`, `StreamReadResult`, `Uint8Array`; fixture [`fetch_stream_ok.ts`](crates/trust-cli/tests/fixtures/fetch_stream_ok.ts), test `compile_fetch_stream_ok`.
- [x] **TLS / HTTP stack (documented, not “Node parity”)**: generated crates use **reqwest** with **`rustls-tls`** ([`crate_writer.rs`](crates/trust-driver/src/crate_writer.rs)). **TLS 1.2+** and **HTTP/2** (when the server and stack negotiate ALPN) are provided by that stack; **root stores, cipher suites, and HTTP/2 prioritization are not guaranteed to match any specific Node or browser version**. **Still backlog**: full **WHATWG** `fetch` (browser `ReadableStream` / `Request`/`Headers` objects, duplex, CORS in non-browser hosts, etc.); trust subset already supports **chunked body** via `getReader`/`read` (see streaming item above).

### Language and typing (trust strong-typing subset)

- [x] **Optional call** `f?.()`; **static narrowing** for `??` / `?.` (decidable; §3.3). Done: [`OptionalCall` / `OptionalMethodCall`](crates/trust-hir/src/ir.rs), [`build_opt_chain_call_expr`](crates/trust-hir/src/build.rs)（含 `Expr::OptChain` callee）；[`optional_call_ok.ts`](crates/trust-cli/tests/fixtures/optional_call_ok.ts)、[`optional_chain_fail.ts`](crates/trust-cli/tests/fixtures/optional_chain_fail.ts)（非标识符 callee）；[`NullishCoalesce`](crates/trust-hir/src/sem.rs) 扩展 **同族** `Union` 去 `null`/`undefined` 后与右操作数合并。**完整** discriminated / 全联合收窄仍属后续。
- [x] **Chained member/calls** `f().g()`（一层 `expr.prop` / `expr.m()`）。Done: [`chain_call_ok.ts`](crates/trust-cli/tests/fixtures/chain_call_ok.ts)，`run_chain_call_ok_prints_six`；一般实例方法类型仍见 §1.3 follow-ups。
- [x] **Numeric model**: 全局 `number` → Rust **`f64`**（[`IRExpr::Number(f64)`](crates/trust-hir/src/ir.rs)，[`rust_ty_scalar`](crates/trust-hir/src/codegen/helpers.rs)，`Math`/`Number`/`JSON.parse` / `indexOf` 等）；下标与 UTF-16 内部仍 `as i32`。破坏性：原 `i32` 截断语义不再；与 Node/IEEE 细节见 README。
- [x] **HIR stdlib / JSON / strings**: `JSON.parse` — **string literal** args fold at build time via `serde_json` to trust-closed IR (`number` / `boolean` / `string` / `null` / homogeneous `number[]` \| `string[]` / flat `{ k: number }`); **dynamic** arg stays JSON **number** document → `f64` via `trust_stdlib::json::parse_number`. Global **`encodeURIComponent`** / **`decodeURIComponent`** through `trust_stdlib::uri`. Generated `Cargo.toml` injects `trust_stdlib` by default ([`crate_writer.rs`](crates/trust-driver/src/crate_writer.rs)); `serde_json` is still added when generated Rust references `serde_json::` (object literals / `JSON.stringify` on objects). Fixtures [`json_uri_trust_ok.ts`](crates/trust-cli/tests/fixtures/json_uri_trust_ok.ts), [`json_parse_hetero_array_fail.ts`](crates/trust-cli/tests/fixtures/json_parse_hetero_array_fail.ts); tests `run_json_uri_trust_ok_prints_expected`, `compile_json_uri_trust_ok_uses_trust_stdlib_json_and_uri`, `compile_json_parse_hetero_array_literal_fails`.

### Trust.toml / Rust extern (crates.io)

- [x] **Manifest**: parse `Trust.toml`; merge `[dependencies]` into the generated crate `Cargo.toml`; discover walking up from the entry `.ts` ([`trust-manifest`](crates/trust-manifest), [`crate_writer`](crates/trust-driver/src/crate_writer.rs), [`graph_loader`](crates/trust-cli/src/graph_loader.rs) / [`pipeline`](crates/trust-driver/src/pipeline.rs)).
- [x] **Module graph**: `import … from "crate_key"` when the key is in the manifest; `validate_imports` checks `[[rust_binding]]`; no filesystem DFS into a fake Rust module tree.
- [x] **HIR / sem / codegen**: `TsType::RustExtern`, `IRExpr::RustNew`, inherent `MethodCall` with `inherent_rust_str_ref` for `string` → `&str` (`.as_str()` at call sites).
- [x] **E2E**: [`tests/fixtures/trust_regex/`](crates/trust-cli/tests/fixtures/trust_regex/) — `run_trust_regex_ok_prints_one`, `compile_trust_regex_ok_emits_regex_crate`.
- [x] **Prebuilt binding shims** (starter patterns): no central registry; copy-paste templates are documented in [`examples/README.md`](examples/README.md) (Diesel/FFI examples + minimal [`tests/fixtures/trust_regex/`](crates/trust-cli/tests/fixtures/trust_regex/) `Trust.toml` pattern). Optional future: curated registry / `trust add` presets — still backlog if desired.

### Documentation and examples

- [x] **README + this file**: periodic sweep so matrix / §1.3 / §2.1 lines match shipped features (e.g. stdlib, `string[i]`, `Math.*` extensions). *(This sweep: §1.3 / §2.1 bullets above + `.json()`/`JSON.parse` serde_json wording; README matrix already lists member/`JSON`/URI/async — see [Language feature matrix](README.md).)*
- [x] **[`test-ts/main.ts`](test-ts/main.ts)**: kept within supported subset; header documents expected I/O and **intentionally omits** `async`/`fetch`/generic call sites (covered by `fixtures/` instead).

### Consolidated product backlog (README / Partial / slow track)

Checkbox list derived from [README — Unsupported TypeScript](README.md), matrix **Partial** rows, §1.3 follow-ups, [§3.3.1](PROJECT-TODO.md), and residual §14 notes. **Strong typing / decidable sem / codegen** still apply ([README — Trust: strong typing](README.md)). Prefer **one main pillar per PR** where possible. When an item is done, tick it here **and** update the section it duplicates (§3.3.1, §13.7, README matrix, etc.).

#### Types and semantics (vs full `tsc`)

- [x] **D1 — Discriminated narrowing** on object unions (`if (v.kind === 'a')` / `else`); coordinate with `??` / `?.` ([§3.3.1](PROJECT-TODO.md), [`sem.rs`](crates/trust-hir/src/sem.rs)). Completed: nested narrowing supported via `narrow_ty` on `Binding`.
- [ ] **D3-style — Broader unions / `normalize_union` / assignability** only where still **one** Rust type and statically decidable ([§3.3](PROJECT-TODO.md)).
- [x] **G1 / G2 — Generics subset expansion** (more explicit type-arg patterns; more arity / non-call monomorphization) ([§3.3.1](PROJECT-TODO.md), [`sem/mono.rs`](crates/trust-hir/src/sem/mono.rs)). Completed: nested generic calls, function return type inference.
- [ ] **G3 — Where-like constraints** (if introduced: must remain fully static).
- [ ] **Intersection types `A & B`** in type positions (currently rejected).
- [ ] **`bigint` / template literal types** in type positions (rejected today).
- [ ] **Full structural subtyping parity with `tsc`** — not a goal; only **documented safe chips** if any.
- [ ] **Heterogeneous unions** (`number | string`, …): clearer codegen strategy or diagnostics when a single Rust type is impossible ([README matrix — Union](README.md)).
- [ ] **`strictNullChecks`-equivalent mode** as an **explicit** compiler option (not implicit TS looseness).
- [ ] **R1 — Nominal methods on `interface` / object types** (static dispatch; global `m__I(receiver,…)` or inherent) ([§3.3.1](PROJECT-TODO.md)).
- [ ] **R2 — Deeper type-driven call/member chains** than one `f().g()` ([§1.3 follow-ups](PROJECT-TODO.md)).
- [ ] **§13.7 B2a + B2+** — `import type`, cross-file interface/type names in annotations, callable members on object types, `interface extends`, richer `readonly` / index signatures ([§13.7](PROJECT-TODO.md)).

#### Language surface

- [ ] **More `export` shapes** only if product scope changes — today: arbitrary `export default`, `export const`, `export * as`, local `export { x }` without `from`, etc. ([README Unsupported](README.md), §13.6).
- [ ] **Wider optional chaining** (callees / forms still rejected — see [`optional_chain_fail.ts`](crates/trust-cli/tests/fixtures/optional_chain_fail.ts)).
- [ ] **`&&` / `||` value-preserving** semantics vs current **`boolean`** result — requires an explicit **strong-typing** spec before implementation ([README matrix](README.md)).
- [ ] **`for..of`** loops.
- [ ] **Labeled `break` / `continue`**.
- [ ] **Computed member / call** `obj[expr]`, `obj[expr](…)` where decidable.

#### Async and closures

- [ ] **Closure codegen** beyond the current **`(number) => number`-style** strict subset ([README — HOF / matrix](README.md)).
- [ ] **WHATWG / browser `fetch` parity** (`Headers` iteration, full `Request`, duplex, etc.) — residual vs current reqwest subset ([README — Web fetch](README.md)).

#### Toolchain and parser UX

- [ ] **C1 — Optional warning** for TS comments not attached when `--ts-source-comments` ([§3.3.1](PROJECT-TODO.md), Comments in §14 above).
- [ ] **C2 / C3 — Trailing / inline comments** and **comment inheritance** after large lowerings (`switch` → `if`, `for` → `while`, …).
- [ ] **Module graph: collect diagnostics from multiple files** after a parse failure (vs stop at first bad file) ([§14 Toolchain](PROJECT-TODO.md)).
- [ ] **Parser / AST error recovery** beyond current swc + multi-line per file (if desired).

#### Ecosystem (non-goals unless policy changes)

- [ ] **npm / `node_modules` / `compilerOptions.paths` → packages** — today **not planned**; revisit only with an explicit product decision ([README — Scope (1.0)](README.md)).
- [ ] **Full `tsconfig` / `tsc` behavior parity** — same.

---

## Maintenance

- When an item is done, change `[ ]` to `[x]`, or add “completed in commit / PR #”.
- If scope changes, note **alternatives** or **why deprecated** in the item.
- If this list conflicts with the [`README.md`](README.md) language matrix, **code wins** — update the README (and [`README.zh-CN.md`](README.zh-CN.md) as needed).
