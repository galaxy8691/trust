[中文](README.zh-CN.md)

# ts2rs

Experimental **TypeScript → Rust source** compiler (implemented in Rust), then **cargo/rustc** to produce executables. This repository is often used as the **trust** subset in engineering.

See also [CONTRIBUTING.md](CONTRIBUTING.md), [CHANGELOG.md](CHANGELOG.md), and the long-term roadmap [PROJECT-TODO.md](PROJECT-TODO.md) ([中文](PROJECT-TODO.zh-CN.md)).

## Trust: strong typing

**trust is strongly typed; there is no soft typing.** Supported programs must have **static, definite** type information at compile time: parameters and return types must be annotated (or equivalently decidable in this subset); `let` / `const` require type annotations (or definite initialization with inferable types). There is **no** implicit `any`, runtime reshaping, or “infer later and widen globally” soft semantics. Validation is **static type checking**, not full TypeScript / `tsc` progressive looseness.

## Architecture

Parse (swc) → **HIR** ([`ts2rs-hir`](crates/ts2rs-hir)) → **semantic checks** (symbols, types, simplified return paths) → **Rust codegen** → **cargo** link.

Optional runtime [`ts2rs_rt`](crates/ts2rs_rt): generated code does **not** depend on this crate today; it exposes placeholder APIs such as `read_stdin_line`. Console: `console.log` → `println!`, `console.error` / `console.debug` → `eprintln!`.

```mermaid
flowchart LR
  TS[TypeScript_source]
  PR[ts2rs_parser_swc]
  HB[HIR_build]
  SE[Semantic_check]
  CG[Codegen_Rust]
  LO[ts2rs_lower]
  CLI[ts2rs_cli]
  DRV[ts2rs_driver]
  TS --> PR --> HB --> SE --> CG --> LO --> CLI
  CLI --> DRV
```

[`ts2rs-lower`](crates/ts2rs-lower) wires HIR build, semantics, and codegen. [`ts2rs-driver`](crates/ts2rs-driver) builds a temporary crate and runs `cargo` (used by `ts2rs run`).

## Unsupported TypeScript (trust rejection boundary)

Common forms that are **explicitly rejected** (diagnostics are English; see [`build.rs`](crates/ts2rs-hir/src/build.rs) / [`sem.rs`](crates/ts2rs-hir/src/sem.rs)). This table complements the **generics** table below and the language matrix. Features marked Supported / Partially supported there (restricted **`?.`**, monomorphized generics, top-level `class` subset, etc.) are **not** listed here as “unsupported.”

| User-visible form | Notes |
|-------------------|--------|
| `export` other than `export function` / top-level `function` / relative **`export * from "./x.ts"`** / **`export { … } from "./x.ts"`** / **`export default function main`** / **`export default main`** (after `function main`) | e.g. `export { }` without `from`, default export of non-`main`, `export * as`, `export const`, `export class` (**top-level `class` without `export`** is in the matrix and [PROJECT-TODO.md §13.3](PROJECT-TODO.md)) |
| Advanced generics | Full TS inference/constraints remain out of scope; **simple monomorphization** with optional explicit type args or **local inference** from argument types (literals + `let`/`param` annotations) — see matrix “Generics” and [§13.1](PROJECT-TODO.md) |
| Optional chaining (rejection boundary) | Restricted **`f?.()`** / **`recv?.m()`** are supported (`optional_call_ok.ts`); other callees / shapes may still be rejected (`optional_chain_fail.ts`) |
| `interface` `extends`, imported interface / type names **across dependency modules** | Single-file nominal table only; nested `number` objects and `k?: number` in **one file** are supported — see matrix “Array / object literals” / `interface` (**not** full `tsc` structural rules) |
| Intersection `A & B` | Rejected |
| `bigint`, template literal types in type positions | Rejected |
| Full `tsc` / full structural typing / HOF beyond §13.2 | Full checker and structural subtyping are not implemented; **restricted** function types and arrows — matrix and [§13.2](PROJECT-TODO.md) |

## Scope (1.0)

- **Matrix coverage**: rows marked Supported / Partially supported have representative **fixtures** ([`fixtures/`](crates/ts2rs-cli/tests/fixtures/)) and **[`cli_e2e.rs`](crates/ts2rs-cli/tests/cli_e2e.rs)** tests; see **[Matrix vs integration tests](#matrix-vs-integration-tests)**. Larger examples: [`test-ts/main.ts`](test-ts/main.ts), [`test-ts/math.ts`](test-ts/math.ts). **Regression** cases: [`tests/regression/`](crates/ts2rs-cli/tests/regression/).
- **Diagnostics**: compile **errors** are **English**, `path:line:col: message` ([`CompileError`](crates/ts2rs-hir/src/error.rs)). **Warnings** (e.g. unreachable code) use the same shape via [`CompileWarning`](crates/ts2rs-hir/src/error.rs); on success the CLI prints warnings to **stderr** and does **not** change exit code.
- **CI**: pushes and PRs run `cargo fmt --all --check`, `cargo test --workspace`, and `cargo clippy --workspace --all-targets` ([`.github/workflows/ci.yml`](.github/workflows/ci.yml)).
- **Not 1.0**: full `tsc` tsconfig parity, arbitrary `export default` expressions, `export * as`, etc. **npm / `node_modules` / package-manager resolution is not planned.** **Relative** `import { x } from "./dep.ts"` and **`import main from "./dep.ts"`** (binding **must** be the identifier `main`, mapped to the dependency’s default export of `main`) and relative **`export *` / `export { … } from`** (barrel files) are supported; the CLI supports **multiple roots** (positional `.ts`) or **`--project`** JSON with simplified **`extends`**, **`files`**, **`include` / `exclude` glob** ([`tsconfig_resolve`](crates/ts2rs-cli/src/tsconfig_resolve.rs), [`graph_loader`](crates/ts2rs-cli/src/graph_loader.rs), [`parse_module_graph_with_extra_roots`](crates/ts2rs-parser/src/module_graph.rs), [`validate_imports`](crates/ts2rs-parser/src/module_graph.rs), HIR [`compile_graph`](crates/ts2rs-hir/src/lib.rs)); entry must define `main`, global function names unique. **Optional incremental** (`compile` / `run --incremental [DIR]`): caches per-module HIR to disk (default dir `.ts2rs-cache`); still parses all `.ts` each run; see [`incremental.rs`](crates/ts2rs-cli/src/incremental.rs).

## Diagnostics and surface (§1.1)

- **Multiple compile errors**: build and semantic phases may collect **several** diagnostics in one failed run ([`CompileError::Many`](crates/ts2rs-hir/src/error.rs)), printed as multiple `path:line:col: message` lines (sorted). Parser [`parse_typescript_file`](crates/ts2rs-parser/src/lib.rs) surfaces **all** swc `take_errors()` diagnostics. **Monomorphization** and **codegen** can still stop at the first internal error. On success, multiple [`CompileWarning`](crates/ts2rs-hir/src/error.rs) may be returned (same shape in [`ts2rs_lower`](crates/ts2rs-lower/src/lib.rs)).
- **`export` shapes**: `export function …`, top-level `function …`, **`export default function main`**, **`export default main`** (with `function main` in the module), relative **`export * from "./…"`**, and **`export { a as b } from "./…"`** for **function** exports (see [`build.rs`](crates/ts2rs-hir/src/build.rs), [`module_graph.rs`](crates/ts2rs-parser/src/module_graph.rs)); `export class` / `export const` / other `export default` / local `export { x }` without `from` / etc. are still rejected; **top-level `class` without `export`** is in the matrix. Fixtures `export_default_*_ok.ts`, negatives `export_*_fail.ts`, and [`cli_e2e.rs`](crates/ts2rs-cli/tests/cli_e2e.rs).
- **Comments**: swc `Program` has **no** comment nodes; [`ParsedSource`](crates/ts2rs-parser/src/lib.rs) includes `source_map`, `comments` (swc leading/trailing tables via [`SingleThreadedComments`](https://rustdoc.swc.rs/swc_common/comments/struct.SingleThreadedComments.html)), and the parser always collects comments for downstream use. **TS comment text in generated Rust** is opt-in: [`CodegenOptions::emit_ts_source_comments`](crates/ts2rs-hir/src/codegen.rs) (CLI `ts2rs compile --ts-source-comments`) emits leading comments as Rust `//` lines before statements and top-level functions; trailing comments and exact placement after large desugarings are not guaranteed (see [PROJECT-TODO.md §14](PROJECT-TODO.md)).
- **Follow-up backlog** (finer-grained comment mapping, project-scale tooling): see [PROJECT-TODO.md §14 — Toolchain and UX](PROJECT-TODO.md).

## Control flow and return (§3.4)

Implemented in [`sem.rs`](crates/ts2rs-hir/src/sem.rs) (`fn_body_returns`, `tail_returns_last_only`, `tail_returns_while_body`, `stmt_block_diverges`, etc.).

- **Non-void functions**: [`check_function`](crates/ts2rs-hir/src/sem.rs) requires `fn_body_returns(&f.body, &ret)` or errors (“not all control paths return…”).
- **Early exhaustive return**: if an earlier statement guarantees a value return on all paths (e.g. full `if` / `else` with `fn_body_returns` on both), the rest may only produce **unreachable** warnings; see `early_return_unreachable.ts`.
- **Tail rules**: if no such early return, the **last reachable statement** must satisfy the simplified return rules (return, `if` with both branches, block, `while` / `do-while` body per `tail_returns_while_body`). An `if` **without** `else` cannot satisfy the “last statement” rule by itself.
- **Unreachable code**: statements after `return`, `break`/`continue` (in loops), or after an `if`/`else` that exhaustively returns — warning `unreachable code`; see `unreachable_after_return.ts`, `break_unreachable.ts`.
- **`let` without init**: `let x: T;` allowed; must be assigned before read. Loops use a **conservative** assignment model. Negative: `definite_assign_fail.ts`.

Fixture pointers: `let_dup_same_block_fail.ts`, `let_shadow_nested_ok.ts`, `param_let_same_name_fail.ts`, `void_log_in_branch.ts`.

## Language feature matrix

| Feature | Status | Notes |
|---------|--------|-------|
| Single `.ts` file | Supported | |
| Top-level `function` | Supported | `export function` in-file; other `export` §1.1 |
| `import` | Partial | Only `import { name } from "./relative.ts"`; deps need that name in the module’s **effective** exports (`export function` and/or relative re-exports); module graph; `import_add_main.ts`, `run_reexport_export_star_ok`, negatives `import_missing_export_*`, `circular_*` |
| `number` / `boolean` / `string` / `void` | Supported | `void` only as return; `let` cannot be `void` |
| `let` (single decl) | Partial | Type annotation required; may omit init but must assign before use (§3.4); mutable `let` → `IRStmt::Assign`; `definite_assign_ok.ts` |
| `const` | Supported | Same shape as `let`; no reassignment |
| Blocks, multiple statements | Supported | Empty `;`, blocks |
| `if` / `else`, `while`, `do-while` | Supported | Condition: `number` (truthy non-zero) or `boolean`, or same **primitive family** union (`1 \| 2`, `true \| false`), not `number \| boolean` mixed |
| C-style `for(;;)` | Supported | |
| `for..in` | Partial | Supports object/class-instance keys and `number[]` index keys; loop variable is `string`; `for-of` unsupported |
| `break` / `continue` | Supported | Must be inside a loop; no labels |
| Nested `function` | Partial | No closure capture subset; `nested_fn.ts` |
| `&&` / `\|\|` | Partial | `boolean` and `number` truthiness; result `boolean`; `logical_bool.ts`, `logical_truthy_ok.ts` |
| Ternary `?:` | Supported | Same type branches; `ternary_ok.ts` |
| Template literals | Supported | No tag; `template_ok.ts` |
| Comma expression | Supported | `comma_ok.ts` |
| Member access | Partial | `string.length` UTF-16 code units; `string[i]` UTF-16 index (single code unit as `string`); `number[].length`; `length` on objects; `obj.m(args)` → global `m(receiver,…)`; **chained** `f().prop` / `f().m()` (`chain_call_ok.ts`); no computed `obj[expr](…)`; fixtures `string_utf16_length.ts`, `method_call_ok.ts`, `object_length_field.ts`, `stdlib_hir_ok.ts` |
| `?.` / `??` | Partial | `?.` **member** and **call** `f?.()` / `recv?.m()` (`optional_call_ok.ts`); `??` extended for same-family unions with `null`/`undefined`; `optional_ok.ts`, `nullish_ok.ts`; §3.3 |
| Array / object literals | Partial | `number[]`; object types with **`number` leaves**, **nested** `{ inner: { x: number } }`, and **`k?: number`** (width subtyping + optional rules differ from full `tsc`; see [`sem/helpers.rs`](crates/ts2rs-hir/src/sem/helpers.rs)); runtime values use **`serde_json::Value`**; `array_ok.ts`, `object_ok.ts`, `nested_object_ok.ts` |
| `switch` | Partial | `case` only `number`/`boolean` literals; `default` last; no fall-through; `switch_ok.ts`, `switch_fail.ts` |
| `return` | Supported | `fn_body_returns` |
| `void` functions | Supported | No return-path requirement |
| `+ - * /`, compares, `!`, unary `-` | Supported | String only `+` concat; numeric ops lower to **`f64`** (Rust); §4.1 |
| `Math.*` builtins | Partial | `abs`, `min`, `max`, `floor`, `ceil`, `sign`, `trunc`, `round`, `pow` (`f64` semantics; `pow` non-negative exponent); `math_builtin.ts`, `stdlib_hir_ok.ts` |
| `Number.*` / `JSON.*` / string methods | Partial | `Number.parseInt` / `parseFloat` → **`f64`**; `JSON.stringify` (`string` \| `number` \| `boolean` \| trust **object** shapes); `JSON.parse`: **literal** string folds to closed shapes (`number` / `bool` / `string` / `null` / homogeneous `number[]` \| `string[]` / nested **number-only** objects); **non-literal** → JSON **number** document → `f64` (`serde_json`); `encodeURIComponent` / `decodeURIComponent` (`urlencoding`); `String` builtins: `charAt`, `charCodeAt`, `slice`, `substring`, `indexOf`, `includes` (UTF-16); `readLine()` sync stdin (not in `async`); `stdlib_hir_ok.ts`, `json_uri_trust_ok.ts` |
| `console.log` / `error` / `debug` | Supported | §4.1 |
| Literal types | Partial | `literal_type_ok.ts`; `bigint` / template literal types in type position rejected |
| Union `A \| B` | Partial | Normalization; must map to one Rust type; `number \| string` heterogeneous fails; `A & B` rejected; `union_*`, `intersection_type_fail.ts` |
| `interface` | Partial | Top-level; shapes as nested/optional `ObjectNum` props in **one compilation unit**; **no** importing an interface **name** from another `.ts` file; **no** callable/`( ) =>` members in object types; `interface_ok.ts`, `nested_object_ok.ts`, negatives |
| `type` alias | Partial | Shared table with `interface`; `type_alias_*.ts` |
| Generics / type args | Partial | Monomorphization: explicit `f<number>(x)` or inferred from args where each parameter type is inferable (`id(3)`, `p.m(...)` with generic `m`); conflicting/uninferable calls rejected; mangled Rust symbols use a stable fingerprint; multiple mono diagnostics may be reported in one run |
| Higher-order functions | Partial | Function type annotations and typed arrow closures are supported in current subset (`(number) => number` → `(f64) -> f64`); variable-call `f(...)`, function args/returns covered by e2e fixtures |
| `async` / `await` / `Promise` / `fetch` / `fetchText` | Partial | `async function` with return `Promise<T>` (`T` is `number` \| `string` \| `void`); **`fetchText(url)`** → `Promise<string>` (`__ts2rs_fetch_text`); **`fetch(url, init?)`** → `Promise<Response>` (`reqwest::Response`: `status`, `ok`, `await .text()`, `await .json()` JSON **number** body → `f64` via `serde_json`); **`response.body.getReader()`** + **`await reader.read()`** → `{ done, value }` with **`Uint8Array`** as `Vec<u8>` (`bytes_stream()` + `futures-util` `StreamExt`); optional **`init`** with string-literal `method`, `headers` map (string literal values), optional `body` string; **`Promise.all([...])`** homogeneous `number` / `string` / `fetch` responses (sequential `.await`); **`.then`** rejected; TLS via **rustls**; HTTP/2 when negotiated — **not** byte-parity with a specific Node release; full WHATWG `fetch` (`Headers`, duplex, etc.) still backlog; see `fetch_response_ok.ts`, `fetch_stream_ok.ts`, `fetch_post_init_ok.ts`, `compile_fetch_response_ok`, `compile_fetch_stream_ok`, `compile_fetch_post_init_ok`, and other `compile_async_*` / `promise_*` tests |
| Class / this / extends / super | Partial | Class subset is lowered to constructor/method functions, with sem checks for extends graph, `super(...)` placement, and baseline `override`; e2e: `class_*` fixtures |
| Full TypeScript / `tsc` | Not implemented | Long-term |

### Matrix vs integration tests

Theme → fixture → `cli_e2e` test names (`run_*`, `compile_*`, `check_*`). Full list lives in the test file.

| Theme | Representative fixtures | Representative tests |
|-------|-------------------------|-------------------------|
| Single file / ops / strings | `sample.ts`, `ops.ts`, `boolean_if.ts`, `string_concat.ts` | `compile_writes_rust`, `compile_exec_writes_binary_and_runs`, `compile_exec_without_o_defaults_to_entry_stem_in_cwd`, `run_prints_main_result`, … |
| Import / multi-file / default export | `import_add_main.ts` + `add_dep.ts`, `export_default_*_ok.ts`, `multi_entry_*`, `export_main.ts` | `run_import_add_main_prints_three`, `run_export_default_function_main_prints_42`, … |
| Incremental HIR cache (`--incremental`) | ad hoc `lib.ts` + `app.ts` in e2e tempdir | `compile_incremental_rebuilds_only_changed_module` |
| Negative import/export | `import_missing_export_*`, `circular_*`, `dup_*`, `export_*_fail.ts` | `compile_import_missing_export_fails`, … |
| `let` / `const` / blocks | `const_ok.ts`, `assign_simple.ts`, `empty_stmt.ts`, `let_if.ts` | `run_const_ok_prints_42`, … |
| Semantics (shadow, void branch) | `let_dup_same_block_fail.ts`, `void_log_in_branch.ts`, … | `compile_*`, `run_void_log_in_branch_prints_branch` |
| Control flow / unreachable | `while_early.ts`, `for_loop.ts`, `for_in_*.ts`, `early_return_unreachable.ts`, … | `run_while_early_prints_three`, `run_for_in_object_keys_ok_prints_three`, `compile_for_in_non_object_fails`, … |
| Logic / ternary / template / comma | `logical_bool.ts`, `ternary_ok.ts`, … | … |
| Members / Math / length / HIR stdlib / chain | `string_utf16_length.ts`, `math_builtin.ts`, `stdlib_hir_ok.ts`, `json_uri_trust_ok.ts`, `chain_call_ok.ts`, … | `run_stdlib_hir_ok_prints_expected`, `run_json_uri_trust_ok_prints_expected`, `run_chain_call_ok_prints_six`, `compile_stdlib_hir_ok_writes_utf16_and_json_helpers`, `compile_json_uri_trust_ok_emits_serde_json_and_urlencoding` |
| `?.` / `??` | `optional_ok.ts`, `nullish_ok.ts`, `nullish_fn_ok.ts` (`check`), `optional_call_ok.ts` | …, `check_nullish_fn_union_ok` |
| Arrays / objects | `array_ok.ts`, `object_ok.ts`, `array_fail.ts` | `compile_array_return_type_mismatch_fails` |
| `switch` | `switch_ok.ts`, `switch_fail.ts` | … |
| Console | `console_stderr.ts`, `void_log.ts` | … |
| Literal / union / intersection | `literal_type_*.ts`, `union_*.ts` | … |
| Interface / type / generic subset | `interface_*.ts`, `type_alias_*.ts`, `generic_function_ok.ts`, `generic_method_call_infer_ok.ts`, `generic_function_*_fail.ts` | `run_interface_generic_ok_prints_zero`, `run_type_alias_generic_ok_prints_zero`, `run_generic_function_ok_prints_three`, `run_generic_method_call_infer_ok_prints_three`, `compile_generic_function_infer_conflict_fails`, … |
| Class subset | `class_basic_ok.ts`, `class_this_method_ok.ts`, `class_extends_ok.ts`, `class_super_ctor_ok.ts`, `class_*_fail.ts` | `run_class_basic_ok_prints_five`, `run_class_extends_ok_prints_seven`, `compile_class_super_invalid_fails`, `compile_class_override_mismatch_fails` |
| Nested function | `nested_fn.ts` | `run_nested_fn_prints_nine` |
| Minimal tsconfig / `--project` | `multi_entry_tsconfig.json`, `multi_entry_*.ts` | `run_project_tsconfig_prints_main`, `run_project_tsconfig_extends_include_ok` |
| Async / `Promise` / HTTP | `async_mvp_compile_ok.ts`, `async_control_flow_ok.ts`, `promise_all_fetch_ok.ts`, `fetch_response_ok.ts`, `fetch_stream_ok.ts`, `fetch_post_init_ok.ts` | `compile_async_mvp_writes_tokio_and_await`, `compile_async_control_flow_if_while_await_ok`, `compile_promise_all_fetch_alias_ok`, `compile_fetch_response_ok`, `compile_fetch_stream_ok`, `compile_fetch_post_init_ok`, `compile_promise_then_fails` |
| CLI `check` / `--emit-ir` | `sample.ts`, `switch_fail.ts` | `check_sample_ok`, `compile_emit_ir_stderr_contains_ir_module` |
| Negative optional / nullish / object | `optional_chain_fail.ts`, `nullish_fail.ts`, `object_fail.ts` | `compile_optional_call_bad_callee_fails`, … |
| Regression anchor | [`tests/regression/switch_fallthrough_regression.ts`](crates/ts2rs-cli/tests/regression/switch_fallthrough_regression.ts) | `regression_switch_fallthrough_check_fails` |

## Type roadmap (§1.4)

Literal types, unions, limited `interface` / `type`, and generics roadmap: [PROJECT-TODO.md §1.4](PROJECT-TODO.md). Nullable / `??` narrowing: sem implements same-family and compatible-`Fn` merge (see §3.3); full discriminated narrowing is still future work.

### Generics (monomorphization subset)

- Generic function declarations are accepted; **monomorphization** runs in `sem` before per-function typecheck. Calls may use **explicit** type arguments (`id<number>(3)`) or **omit** them when each type parameter can be fixed from the callee signature and **synthesized argument types** (numeric/string/boolean/`null`/`undefined` literals, or locals/parameters with known annotations).
- Uninferable arguments (e.g. unknown identifier), **conflicting** constraints on the same type parameter, or parameter types that still contain unsupported shapes for inference produce errors; several such errors can be reported in one compile.
- Method-call sugar `obj.m(args)` lowers to `m(obj, ...args)`; if `m` is generic, the same inference rules apply (receiver plus arguments).
- Instance Rust names are `name__` + 16 hex digits (FNV-1a over a canonical type key); `mono_origin` on IR still records a readable instantiation label.
- Generic `interface` / `type` declarations are accepted in the current restricted type subset.
- Broader TypeScript generic semantics (full inference, rich constraints, higher-order polymorphism) remain out of scope.

## Semantics roadmap (§3.3)

See [PROJECT-TODO.md §3.3](PROJECT-TODO.md). **Implemented in `sem`**: after stripping `null`/`undefined` from a `Union`, nullish coalescing merges with the right operand when types are the same primitive family or **mutually assignable `Fn` types** (`unify_ternary_branches`). **Still future**: discriminated narrowing driven by a discriminant; no `strictNullChecks` switch; nominal `interface`/`type` vs full structural TS. HOFs are a **restricted** typed subset (see README above), not “no first-class functions.” Heterogeneous unions may still fail Rust codegen when a binding cannot map to one Rust type; `nullish_fn_ok.ts` is validated with `ts2rs check`.

## Arithmetic, `/`, overflow (§4.1)

- **`number` → `f64`** in generated Rust (`+`, `-`, `*`, `/`, compares, `Math.*`, etc.).
- **`/`**: IEEE-754 double semantics via `f64` (closer to TS than the former `i32` division).
- **Overflow / NaN**: `f64` infinity and NaN are possible; not identical to V8’s `number` edge cases in every scenario.
- **`console.*` multi-arg**: spaced `"{}"` formatting ([`emit_builtin_log`](crates/ts2rs-hir/src/codegen.rs)).

## Build

```bash
cargo build --release
cargo test
```

## Usage

```bash
cargo run -p ts2rs-cli -- compile path/to/app.ts -o out.rs
cargo run -p ts2rs-cli -- compile path/to/app.ts -o ./my-app --exec
# --exec 可省略 -o：在当前目录生成与入口同名的可执行文件（如 app）
cargo run -p ts2rs-cli -- compile --exec path/to/app.ts
cargo run -p ts2rs-cli -- compile path/to/entry.ts path/to/extra.ts -o out.rs
cargo run -p ts2rs-cli -- run path/to/app.ts
cargo run -p ts2rs-cli -- run --project path/to/tsconfig.json
cargo run -p ts2rs-cli -- compile path/to/entry.ts -o out.rs --incremental .ts2rs-cache
cargo run -p ts2rs-cli -- check path/to/app.ts
```

### CLI

| Subcommand | Role |
|------------|------|
| **`compile`** | Parse → HIR → sem → Rust written to **`-o` / `--output`** (**required** unless **`--exec`**); with **`--exec`**, temp crate + **`cargo build`** and copy the **executable** to **`-o`**, or to **`<entry-stem>`** in the **current directory** if **`-o`** is omitted |
| **`run`** | Same, then temp crate, **`cargo build`** (default **`--release`**) and run |
| **`check`** | Parse + HIR + **semantics only**; no `.rs`, no `cargo` |

**Global** (before subcommand, e.g. `ts2rs -q run …`):

| Flag | Role |
|------|------|
| **`-q` / `--quiet`** | Suppress warnings on success (errors still stderr) |
| **`--color`** | `auto` / `always` / `never` for help styling; interacts with `NO_COLOR` |

**`compile`**: `--span-comments`, `--ts-source-comments` (emit TS leading comments as Rust `//` lines), `--emit-ir` (dumps [`IRModule`](crates/ts2rs-hir/src/ir.rs) `Debug` to stderr), **`--exec`** (see table: **`-o`** is the executable path, or default **entry file stem** under **cwd** — on **Windows** the default name gets a **`.exe`** suffix; uses `cargo` like `run`), **`--incremental` / `--incremental DIR`** (multi-file HIR fragment cache; default dir `.ts2rs-cache` when flag is present without value). With **`--exec`** only: **`--link-ts2rs-rt`**, **`--debug`**, **`-O` / `--release`** (same semantics and mutual exclusion as `run`).

**`run`**: `--link-ts2rs-rt`; **`--debug`** → debug `cargo build`; **`-O` / `--release`** (conflicts with `--debug`); **`--incremental`** same as `compile`.

**Exit codes**: **0** success; **1** ts2rs/driver errors; **`run`** propagates child process exit code when the binary fails; warnings do not change exit code.

- **Multi-file**: first path is **entry** (`export function main`), rest are **extra roots**.
- **`--project` tsconfig** (simplified JSON): optional **`extends`**, **`files`**, **`include`**, **`exclude`** (globs); paths relative to the config file that contains them; **`exclude`** accumulates along the `extends` chain. If merged **`files`** is non-empty, only those entries are used; otherwise **`include`** globs are expanded (`.ts` only). **`include`-only**: roots are sorted; first path is entry — use **`files`** to pin entry order. Mutually exclusive with positional `.ts` args.
- **`--link-ts2rs-rt`**: optional path dep on **`ts2rs_rt`** (build from this repo).
- **`compile --link-ts2rs-rt`**: has no effect without **`--exec`**; with **`--exec`**, same as **`run --link-ts2rs-rt`**.

## Crate layout

| Crate | Role |
|-------|------|
| `ts2rs-parser` | swc wrapper; `ParsedSource` (`program`, `source_map`, `comments`); [`module_graph`](crates/ts2rs-parser/src/module_graph.rs); shared import parsing in [`import_utils`](crates/ts2rs-parser/src/import_utils.rs) |
| `ts2rs-hir` | IR, build, sem, `emit_rust`; [`compile_graph`](crates/ts2rs-hir/src/lib.rs); [`ir_cache`](crates/ts2rs-hir/src/ir_cache/mod.rs) (incremental disk snapshots); split helpers: [`build/build_types.rs`](crates/ts2rs-hir/src/build/build_types.rs), [`sem/helpers.rs`](crates/ts2rs-hir/src/sem/helpers.rs), [`codegen/helpers.rs`](crates/ts2rs-hir/src/codegen/helpers.rs) |
| `ts2rs-lower` | [`lower_module_graph`](crates/ts2rs-lower/src/lib.rs) |
| `ts2rs-driver` | Temp crate + `cargo` ([`compile_entrypoint_to_executable`](crates/ts2rs-driver/src/lib.rs)); pipeline split: [`pipeline.rs`](crates/ts2rs-driver/src/pipeline.rs), [`cargo_runner.rs`](crates/ts2rs-driver/src/cargo_runner.rs), [`crate_writer.rs`](crates/ts2rs-driver/src/crate_writer.rs) |
| `ts2rs_rt` | Optional runtime |
| `ts2rs-cli` | `ts2rs` binary; [`cli_args.rs`](crates/ts2rs-cli/src/cli_args.rs), [`commands.rs`](crates/ts2rs-cli/src/commands.rs), [`graph_loader.rs`](crates/ts2rs-cli/src/graph_loader.rs), [`tsconfig_resolve.rs`](crates/ts2rs-cli/src/tsconfig_resolve.rs), [`incremental.rs`](crates/ts2rs-cli/src/incremental.rs) |

## License

MIT OR Apache-2.0
