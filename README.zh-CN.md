[English](README.md)

# ts2rs

将 **TypeScript** 编译为 **Rust 源码**，再经 **cargo/rustc** 生成可执行文件的实验性编译器（Rust 实现）。本仓库在工程上常作为 **trust** 子集使用。

另见 [CONTRIBUTING.zh-CN.md](CONTRIBUTING.zh-CN.md) 与 [CHANGELOG.zh-CN.md](CHANGELOG.zh-CN.md)。

## 类型立场：硬类型（trust）

**trust 是硬类型，不允许软类型。** 受支持的程序必须在编译期内具备**静态、确定**的类型信息：参数与返回值须注解（或本子集内等价地可判定）；`let` / `const` 须带类型注解（或明确初始化且类型可推断）；**不**提供隐式 `any`、运行期随意改型、或「先写后推断全局放宽」等软类型语义。路线与验收以**静态类型检查**为准，与完整 TypeScript / `tsc` 的渐进式宽松模式**不一致**。

## 架构

解析（swc）→ **HIR**（[`ts2rs-hir`](crates/ts2rs-hir)）→ **语义检查**（符号、类型、简化 return 路径）→ **Rust 代码生成** → **cargo** 链接。

可选运行时 [`ts2rs_rt`](crates/ts2rs_rt)：当前**生成代码不依赖**本 crate；提供 `read_stdin_line` 等占位 API。控制台：`console.log` → `println!`，`console.error` / `console.debug` → `eprintln!`。

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

[`ts2rs-lower`](crates/ts2rs-lower) 串联 HIR 构建、语义与代码生成。[`ts2rs-driver`](crates/ts2rs-driver) 负责临时 crate 与 `cargo`（`ts2rs run` 使用）。

## 不支持的 TypeScript 特性（trust 硬类型拒斥边界）

以下为常见**显式拒绝**形态（诊断为英文；详见 [`build.rs`](crates/ts2rs-hir/src/build.rs) / [`sem.rs`](crates/ts2rs-hir/src/sem.rs)）。与下文「泛型与类型参数」表及语言矩阵**互补**。

| 用户可见形态 | 说明 |
|--------------|------|
| 非 `export function` / 非顶层 `function` 的 `export` | 如 `export { }`、`export default`、`export * from`、`export const` / `class` 等 |
| 泛型 | `function f<T>`、`interface I<T>`、`type A<T>`、类型位置 `Foo<T>` |
| 可选调用 | `f?.()`；可选**成员** `obj?.prop` 为部分支持 |
| `interface extends`、跨文件导入接口名、可选属性 | 仅单文件具名表 |
| 交集 `A & B` | 拒绝 |
| `bigint`、类型位置模板字面量类型 | 拒绝 |
| 完整 `tsc` / 结构子类型全集 / 高阶函数值 | 未实现 |

## 1.0 范围（当前版本）

- **矩阵覆盖**：下表「支持」或「部分支持」行均有代表性 **fixture**（[`fixtures/`](crates/ts2rs-cli/tests/fixtures/)）与 **[`cli_e2e.rs`](crates/ts2rs-cli/tests/cli_e2e.rs)** 测试对应，详见下文 **[矩阵与集成测试对照](#矩阵与集成测试对照)**。手工大样例另见 [`test-ts/main.ts`](test-ts/main.ts)、[`test-ts/math.ts`](test-ts/math.ts)。**回归**用例目录见 [`tests/regression/`](crates/ts2rs-cli/tests/regression/)。
- **诊断**：编译**错误**为**英文**，格式为 `path:line:col: message`（[`CompileError`](crates/ts2rs-hir/src/error.rs)）。**警告**（如不可达代码）同为该格式，经 [`CompileWarning`](crates/ts2rs-hir/src/error.rs) 收集；成功编译时 CLI / driver 将警告打印到 **stderr**，不抬高退出码。
- **CI**：推送与 PR 在 GitHub Actions 上运行 `cargo fmt --all --check`、`cargo test --workspace` 与 `cargo clippy --workspace --all-targets`（[`.github/workflows/ci.yml`](.github/workflows/ci.yml)）。
- **非 1.0**：完整 `tsconfig`（`extends` / `include` glob）、包名解析、`export *` 等仍为后续目标；**相对路径** `import { x } from "./dep.ts"` 已支持；CLI 另支持**多入口**：多个 `.ts` 位置参数或极简 JSON（仅 `files` 数组）+ `--project`（[`parse_module_graph_with_extra_roots`](crates/ts2rs-parser/src/module_graph.rs) + `validate_imports`，HIR [`compile_graph`](crates/ts2rs-hir/src/lib.rs)；入口文件须含 `main`，全局函数名唯一）。

## 诊断与前端健壮性（§1.1）

- **单条错误**：[`ts2rs_hir::compile`](crates/ts2rs-hir/src/lib.rs) / [`compile_graph`](crates/ts2rs-hir/src/lib.rs) 在失败时**只报告第一条**错误（[`CompileError`](crates/ts2rs-hir/src/error.rs)），同一次运行不会继续收集后续错误；若需「一屏多条」错误诊断，见 [PROJECT-TODO.md](PROJECT-TODO.md) 长期条目。**成功时**可附带多条 [`CompileWarning`](crates/ts2rs-hir/src/error.rs)（返回 `(String, Vec<CompileWarning>)`；[`ts2rs_lower`](crates/ts2rs-lower/src/lib.rs) 同形）。
- **`export` 形态**：除 `export function …` 与顶层 `function …` 外，其余 `export`（如 `export { … }`、`export default`、`export * from`、`export const` / `class` 等）均**显式报错**（[`build.rs`](crates/ts2rs-hir/src/build.rs)）；负例样例见 `export_*_fail.ts`（与 [`cli_e2e.rs`](crates/ts2rs-cli/tests/cli_e2e.rs)）。
- **注释**：swc 产出的 `Program` **不携带**注释节点；[`ParsedSource`](crates/ts2rs-parser/src/lib.rs) 已含 `source_map` 供行列号。若要将 TS 注释反映到生成的 Rust，需在 parser 侧保留注释或扫描 token，并在 IR/codegen 中单独设计；**当前未实现**。

## 控制流与 return（简化语义 + §3.4）

实现见 [`sem.rs`](crates/ts2rs-hir/src/sem.rs)（`fn_body_returns`、`tail_returns_last_only`、`tail_returns_while_body`、`stmt_block_diverges` 等）。

- **非 `void` 函数**：在 [`check_function`](crates/ts2rs-hir/src/sem.rs) 末尾要求 `fn_body_returns(&f.body, &ret)` 为真，否则报错（「not all control paths return…」）。
- **提前穷尽返回**：若语句序列中**靠前**的某条语句已保证所有路径带值返回（例如完整的 `if { … } else { … }` 且两分支持 `fn_body_returns`），则视为整函数已返回；其后的语句仅触发**不可达代码警告**（仍生成 IR/Rust），见 `early_return_unreachable.ts`。
- **尾部规则（与旧版兼容）**：若不存在上述提前返回，则仍要求**最后一条可达语句**在简化意义下保证 return：
  - 最后一条是 `return`（带值）→ 通过；
  - 最后一条是 `if`，且 **then 与 else 两个分支都存在**时，要求**两个分支**各自满足与 `while` 体相同的**尾部** `tail_returns_while_body` 规则；**无 `else` 的 `if` 不能**单独满足「最后一条」的 return 检查（即使 then 内全部 return）；
  - 最后一条是块 `{ ... }` → 递归看块内语句；
  - 最后一条是 `while` / `do-while` → 看**循环体**是否 `tail_returns_while_body`（不分析循环是否执行；与旧 `stmts_return` 行为一致）。
- **不可达代码**：同一块内出现在 `return`、`break`/`continue`（循环体内）、或「`if`/`else` 两分支持提前函数返回」之后的语句会生成 **warning**（`unreachable code`），见 `unreachable_after_return.ts`、`break_unreachable.ts`。
- **`let` 无初始化**：`let x: T;` 允许（须类型注解）；首次读前须已赋值（含 `if`/`else` 分支合并）。循环体对外层变量的赋值采用**保守**策略（体执行次数不定则不视为循环后一定已赋值）。负例 `definite_assign_fail.ts`。
- **与 TypeScript / `tsc`**：仍**不等价**于完整可达性 / never / `tsc` return 检查；以编译器报错与警告为准。

符号表（块作用域、`let`/`const` 同块重复、嵌套遮蔽、与形参同名）的边界样例见 fixtures：`let_dup_same_block_fail.ts`、`let_shadow_nested_ok.ts`、`param_let_same_name_fail.ts`。`void` 与 `console.log` 在分支内见 `void_log_in_branch.ts`。

## 语言支持矩阵

| 特性 | 状态 | 说明 |
|------|------|------|
| 单文件 `.ts` | 支持 | |
| `function` 顶层声明 | 支持 | `export function` 同文件内支持；其它 `export` 形式见上文 §1.1 |
| `import` | 部分支持 | 仅 `import { name } from "./relative.ts"`；依赖须 `export function name`；解析为模块图（不合并 AST）；见 `import_add_main.ts` 与负例 `import_missing_export_*`、`circular_*` |
| `number` / `boolean` / `string` / `void` | 支持 | `void` 仅作返回类型；`let` 不可用 `void` |
| `let`（单声明） | 部分支持 | 须类型注解；可无初始化，但使用前须明确赋值（见上文 §3.4）；可变 `let` 可二次赋值（`IRStmt::Assign`）；见 `definite_assign_ok.ts` |
| `const` | 支持 | 与 `let` 同形，语义禁止赋值 |
| 块 `{ }`、多条语句 | 支持 | 含空语句 `;`、块语句 |
| `if` / `else`、`while`、`do-while` | 支持 | 条件为 `number`（非 0 为真）或 `boolean`；或**同一 primitive 族**的联合（如 `1 \| 2`、`true \| false`），不含 `number \| boolean` 等混合 |
| C 风格 `for(;;)` | 支持 | `init`/`update` 为单声明或表达式；`update` 可为 `i = i + 1` |
| `break` / `continue` | 支持 | 须在循环内；带 label 未支持 |
| 嵌套 `function` | 部分支持 | 无闭包捕获子集；见 `nested_fn.ts` |
| 逻辑与/或 | 部分支持 | `boolean` 与 `number`（`number` 按 `!= 0` 真值，与条件位置一致）；结果类型为 `boolean`；见 `logical_bool.ts`、`logical_truthy_ok.ts`；非 `boolean`/`number` 联合仍拒绝 |
| 三元 `?:` | 支持 | 两分支需同类型；条件为 `number` 或 `boolean`；见 `ternary_ok.ts` |
| 模板字符串 | 支持 | 无 tag；插值须非 `void`；见 `template_ok.ts` |
| 逗号表达式 | 支持 | 取最后一项类型与值；见 `comma_ok.ts` |
| 成员访问 | 部分支持 | `string.length` 为 JS **UTF-16 码元数**；`number[].length`；对象字段 `length` 为普通数字字段；**未**支持 `string` 下标（仅 `number[]` 下标）；**`obj.m(args)`** 脱糖为全局函数 **`m(receiver, ...args)`**（`receiver` 为 `obj` 的值；须存在对应顶层函数；与严格 `tsc` 对 interface 成员的检查可能不一致）；**未**支持 `obj[expr](...)`、`obj?.m(...)`；见 `string_utf16_length.ts`、`method_call_ok.ts`、`object_length_field.ts` |
| `?.` / `??` | 部分支持 | `?.` 仅支持成员访问 `obj?.prop`，`??` 为受限空值子集；见 `optional_ok.ts`、`nullish_ok.ts`；完整语义见 §3.3 |
| 数组 / 对象字面量 | 部分支持 | 仅 `number[]` 与 `{ k: number }` 子集；对象为值型 `HashMap`（无 `Rc`/`Arc`）；见 `array_ok.ts`、`object_ok.ts`；完整类型语法见 §1.4 / §2.1 |
| `switch` | 部分支持 | `case` 仅 `number`/`boolean` **字面量**；`default` 须**最后**；**无** `case` 间穿透（空 `case` 体拒绝）；`case` 末尾 `break` 在 build 剥离；判别式与 `if` 条件类型规则一致；见 [`switch_ok.ts`](crates/ts2rs-cli/tests/fixtures/switch_ok.ts)、负例 [`switch_fail.ts`](crates/ts2rs-cli/tests/fixtures/switch_fail.ts) |
| `return` | 支持 | 非 `void` 函数需满足 `fn_body_returns`（含提前穷尽返回 + 尾部规则，见上文「控制流与 return」） |
| `void` 函数 | 支持 | 不要求 `return` 路径检查 |
| `+ - * /`、比较、`!`、一元 `-` | 支持 | 字符串仅 `+` 拼接；`number` 运算见下文「算术、`/` 与溢出」 |
| `Math.abs` / `min` / `max` / `floor` / `ceil` | 部分支持 | 整数 `number`；`floor`/`ceil` 在纯 `i32` 下为恒等；见 `math_builtin.ts` |
| `console.log` / `console.error` / `console.debug` | 支持 | `log` → `println!`；`error` / `debug` → `eprintln!`；多参数均为 `"{}"` **空格分隔**（与 §4.1 一致） |
| 字面量类型 | 部分支持 | `42`、`"a"`、`true` 等类型位置；向 `number`/`string`/`boolean` 拓宽；见 `literal_type_ok.ts`；`bigint`/模板字面量类型位置拒绝 |
| 联合类型 `A \| B` | 部分支持 | 嵌套 `|` 扁平化、排序去重；成员须**映射到同一 Rust 类型**（如均为 `number` 字面量或 `number` 与字面量）；`number \| string` 等无法在单一 Rust 类型上 codegen 时会报错；**交集** `A & B` 拒绝；条件位置须为单族联合；见 `union_*`、`intersection_type_fail.ts` |
| `interface`（受限） | 部分支持 | 顶层 `interface` / `export interface`；声明体与 `{ k: number }` 相同规则，解析为 [`TsType::ObjectNum`](crates/ts2rs-hir/src/ir.rs)（`build.rs` 中具名表）；类型位置用 `Point` 形式引用；**单文件**内按出现顺序声明，引用尚未声明的接口名会报错；**不**从依赖模块导入接口名；`extends`、泛型、可选属性拒绝；见 `interface_ok.ts`、`export_interface_ok.ts`、负例 `interface_extends_fail.ts`、`interface_generic_fail.ts` |
| `type` 别名（受限） | 部分支持 | 顶层 `type Id = T` / `export type`；与 `interface` **共用**同一张具名表（[`collect_named_types`](crates/ts2rs-hir/src/build.rs)），按**出现顺序**解析右侧 `T`；可与 `interface` 交错；重复名（含与 `interface` 同名）拒绝；泛型 `type` 拒绝；见 `type_alias_ok.ts`、`type_alias_to_interface_ok.ts`、`export_type_alias_ok.ts`、负例 `type_alias_generic_fail.ts`、`type_alias_dup_fail.ts` |
| 泛型 / 类型实参 | 不支持 | 显式拒绝（见下文「泛型与类型参数（当前仍拒绝）」）；负例 `generic_function_fail.ts`、`interface_generic_fail.ts`、`type_alias_generic_fail.ts` |
| 完整 TypeScript / `tsc` 语义 | 未实现 | 长期目标 |

### 矩阵与集成测试对照

下列按主题归纳「语言支持矩阵」行与 **fixture** / **`cli_e2e` 测试**（命名前缀 `run_`、`compile_`、`check_` 等；完整列表以测试文件为准）。「完整 TS」等无单行 fixture。

| 主题 | 代表性 fixture | 代表性集成测试 |
|------|----------------|----------------|
| 单文件 / 算术 / 条件 / 字符串 | `sample.ts`、`ops.ts`、`boolean_if.ts`、`string_concat.ts` | `compile_writes_rust`、`run_prints_main_result`、`run_ops_prints_six` 等 |
| `import` / 多文件 / 导出 | `import_add_main.ts`+`add_dep.ts`、`multi_entry_*`、`export_main.ts` | `run_import_add_main_prints_three`、`run_multi_entry_extra_roots_prints_main`、`compile_export_main_writes_ts_main` |
| 负例（import/export/重复） | `import_missing_export_*`、`circular_*`、`dup_*`、`export_*_fail.ts`、`import_fail.ts` | `compile_import_missing_export_fails`、`compile_circular_import_fails` 等 |
| `let`/`const`/块/赋值 | `const_ok.ts`、`assign_simple.ts`、`empty_stmt.ts`、`let_if.ts` | `run_const_ok_prints_42`、`run_assign_simple_prints_five` 等 |
| 语义边界（重复/shadow/void 分支） | `let_dup_same_block_fail.ts`、`let_shadow_nested_ok.ts`、`param_let_same_name_fail.ts`、`void_log_in_branch.ts` | 对应 `compile_*` / `run_void_log_in_branch_prints_branch` |
| 控制流与 return / 不可达 | `while_early.ts`、`for_loop.ts`、`do_while_count.ts`、`break_while.ts`、`continue_while.ts`、`early_return_unreachable.ts`、`definite_assign_*.ts` | `run_while_early_prints_three`、`compile_early_return_unreachable_warns` 等 |
| 逻辑/三元/模板/逗号 | `logical_bool.ts`、`logical_truthy_ok.ts`、`ternary_ok.ts`、`template_ok.ts`、`comma_ok.ts` | `run_logical_bool_prints_one`、`run_ternary_ok_prints_one` 等 |
| 成员 / `Math` / 数组长度 | `string_utf16_length.ts`、`member_length_ok.ts`、`method_call_ok.ts`、`object_length_field.ts`、`array_length.ts`、`math_builtin.ts` | `run_string_utf16_length_prints_two`、`run_math_builtin_prints_sum` 等 |
| `?.` / `??`（支持子集） | `optional_ok.ts`、`nullish_ok.ts` | `run_optional_ok_prints_two`、`run_nullish_ok_prints_one` |
| 数组 / 对象字面量 | `array_ok.ts`、`object_ok.ts`；负例 `array_fail.ts` | `run_array_ok_prints_two`、`run_object_ok_prints_three`、`compile_array_return_type_mismatch_fails` |
| `switch` | `switch_ok.ts`、`switch_fail.ts` | `run_switch_ok_prints_seven`、`compile_switch_fallthrough_fails` |
| `console` | `console_stderr.ts`、`void_log.ts` | `compile_console_stderr_writes_eprintln`、`run_void_log_in_branch_prints_branch` |
| 字面量类型 / 联合 / 交集 | `literal_type_*.ts`、`union_*.ts`、`intersection_type_fail.ts` | `run_literal_type_ok_prints_eight`、`compile_union_heterogeneous_fail_errors` 等 |
| `interface` / `type` / 泛型拒绝 | `interface_*.ts`、`type_alias_*.ts`、`generic_function_fail.ts` | `run_interface_ok_prints_three`、`compile_generic_function_fails` 等 |
| 嵌套函数 | `nested_fn.ts` | `run_nested_fn_prints_nine` |
| 极简 tsconfig / `--project` | `multi_entry_tsconfig.json` + `multi_entry_*.ts` | `run_project_tsconfig_prints_main` |
| CLI：`check` / `--emit-ir` | `sample.ts`、`switch_fail.ts` | `check_sample_ok`、`compile_emit_ir_stderr_contains_ir_module` |
| 可选链 / `??` / 对象字段（负例边界） | `optional_chain_fail.ts`、`nullish_fail.ts`、`object_fail.ts` | `compile_optional_call_not_supported_fails` 等 |
| 回归锚点（与 fixture 重复语义） | [`tests/regression/switch_fallthrough_regression.ts`](crates/ts2rs-cli/tests/regression/switch_fallthrough_regression.ts) | `regression_switch_fallthrough_check_fails` |

## 类型层路线（§1.4）

**字面量类型**、**联合类型**、**受限 `interface`**、**受限 `type` 别名**与**泛型边界文档**子项已勾选（见 [PROJECT-TODO.md §1.4](PROJECT-TODO.md)）。与已实现子集（如注解中的 `number[]`、仅 `number` 字段的对象类型）的边界与拆分里程碑见 [PROJECT-TODO.md §1.4](PROJECT-TODO.md)。空值与收窄与下文「语义与类型路线（§3.3）」及 [PROJECT-TODO.md §3.3](PROJECT-TODO.md) 交叉：联合与 `??` / `?.` 的**完整**收窄仍待后续；当前 `??` 仍为受限子集（见 `nullish_ok.ts`），含 `null`/`undefined` 与非 primitive 的联合若无法映射到单一 Rust 类型会在 codegen 阶段拒绝。

### 泛型与类型参数（当前仍拒绝）

本版本**不实现**泛型语义（无单态化/类型实参代入）。下列形态在 [`build.rs`](crates/ts2rs-hir/src/build.rs) 中显式报错，诊断为英文：

| 语法形态 | 诊断摘要（节选） | 说明 |
|----------|------------------|------|
| 顶层 `function f<T>(…)` | `generic functions are not supported` | `build_fn` 检查 `func.type_params` |
| `interface I<T> { … }` / `export interface I<T>` | `generic interfaces are not supported` | `collect_one_interface` |
| `type A<T> = …` / `export type A<T> = …` | `generic type aliases are not supported` | `collect_one_type_alias` |
| 类型位置 `Name<T>`（`TsTypeRef` 带实参） | `type arguments on type references are not supported` | `ts_type_from_ast` 中 `TsTypeRef` |

后续若支持泛型，需单态化或等价受限策略；与上表无关的其它类型错误（如 `qualified type names are not supported`）仍按现有诊断为准。

## 语义与类型路线（§3.3）

与 [PROJECT-TODO.md §3.3](PROJECT-TODO.md) 对应；本小节描述**当前**边界与后续方向（矩阵中 `?.` / `??` 一行「完整语义」指本节）。

### 与 §1.4 的衔接

字面量类型与联合类型在 HIR 中已与 [`TsType`](crates/ts2rs-hir/src/ir.rs) 对齐；`??` 与 `?.` 仅在**受限子集**内实现（见 `nullish_ok.ts`、`optional_ok.ts`）。**完整** discriminated 收窄与空值收窄仍属后续工作；扩展时需与 §1.4 联合类型规则一致，避免与受限 `TsType` 语义冲突。

### `null` / `undefined` 与 `strictNullChecks`

[`TsType`](crates/ts2rs-hir/src/ir.rs) 已包含 `Null`、`Undefined` 等变体；本编译器**没有** TypeScript `strictNullChecks` 的等价开关。若未来对齐 `tsc` 空值规则，需单独定义策略（例如默认宽松与显式严格模式）。

### 结构类型与名义类型

顶层 `interface` / `type` 按**名字**在单文件具名表解析（名义侧）；对象字面量与赋值、成员访问的类型检查在 [`sem.rs`](crates/ts2rs-hir/src/sem.rs) 中按现有结构化规则进行。生成 Rust 为具名函数与值类型，**不**实现 TypeScript 结构子类型全集。

### 函数类型与高阶函数

仅支持 **`function` 声明**与调用；**无**「函数作为一等值」的类型（无箭头函数类型表达式作值、无函数类型注解传播到值）。高阶函数需后续扩展 IR 与 [`codegen.rs`](crates/ts2rs-hir/src/codegen.rs)。

更多进阶条目见 [PROJECT-TODO.md §3.3–3.4](PROJECT-TODO.md)。

## 算术、`/` 与溢出（codegen，§4.1）

- **`number` → `i32`**：`+`、`-`、`*` 为 `i32` 上的运算。
- **除法 `/`**：生成 Rust 整数除法，**向零截断**（与 `i32` / `/` 一致）。**与 TypeScript 不同**：TS 中 `number` 的 `/` 为 IEEE-754 **浮点**除法（例如 `1 / 2 === 0.5`），本子集不复现该行为。
- **范围与溢出**：可表示范围即 `i32`，与 TS `number` 双精度全集不一致。算术溢出遵循 **Rust** 对 `i32` 的语义（如在 Release 配置下溢出可为未定义行为）；**默认不**插入 `checked_*` 或运行时 panic；若需可检测溢出，属后续可选工作（见 [PROJECT-TODO.md §4.1](PROJECT-TODO.md)）。
- **`console.log` / `console.error` / `console.debug`**：多参数时格式串为 `"{}"` 之间带**空格**（如 `println!("{} {}", …)` / `eprintln!(…)`），与 [`emit_builtin_log`](crates/ts2rs-hir/src/codegen.rs) 实现一致。

## 构建

```bash
cargo build --release
cargo test
```

## 用法

```bash
cargo run -p ts2rs-cli -- compile path/to/app.ts -o out.rs
cargo run -p ts2rs-cli -- compile path/to/entry.ts path/to/extra.ts -o out.rs
cargo run -p ts2rs-cli -- run path/to/app.ts
cargo run -p ts2rs-cli -- run --project path/to/tsconfig.json
cargo run -p ts2rs-cli -- check path/to/app.ts
```

### CLI（子命令与选项）

| 子命令 | 作用 |
|--------|------|
| **`compile`** | 解析 → HIR → 语义 → 生成 Rust，写入 **`-o` / `--output`** 路径 |
| **`run`** | 同上后写入临时 crate，**`cargo build`**（默认 **`--release`**）并运行生成的可执行文件 |
| **`check`** | 仅解析 + HIR + **语义检查**，不写 `.rs`、**不**调用 `cargo` |

**全局**（可写在子命令前，如 `ts2rs -q run …`）：

| 选项 | 说明 |
|------|------|
| **`-q` / `--quiet`** | 成功时不打印 **warning**（错误仍输出） |
| **`--color`** | `auto` / `always` / `never`，帮助文本等着色；`never` / `always` 在解析前会同步 `NO_COLOR`（亦可直接设 `NO_COLOR=1`） |

**`compile`**：`--span-comments`、`--emit-ir`（将 [`IRModule`](crates/ts2rs-hir/src/ir.rs) 的 `Debug` 打到 **stderr**，输出可能很大，仅调试用）、`--link-ts2rs-rt`（无效果，与 `run` 对齐）。

**`run`**：`--link-ts2rs-rt`；**`--debug`** 使用非 release 的 `cargo build`（`target/debug/`）；**`-O` / `--release`** 显式要求 release 构建（与默认一致，与 `--debug` 互斥）。

**退出码**：**`0`** 表示 ts2rs 与子程序均成功；**`ts2rs` 自身失败**（解析、语义、找不到 `cargo` 等）为 **`1`**；**`run`** 在已生成并成功启动子进程后，若子进程非零退出，则 **ts2rs 以该进程的退出码退出**（无 `code()` 时如信号则 **`1`**）。Warning **不**抬高退出码（与上文「诊断」一致）。

- **多文件**：第一个位置参数为**入口**（须含 `export function main`），其余为**额外根**（入口 DFS 未覆盖的 `.ts` 仍会加入模块图）。
- **极简 tsconfig**：`--project foo.json` 解析 JSON 中的 **`files`** 数组（路径相对该文件所在目录）；**第一项为入口**。与多个 `.ts` 位置参数**互斥**。
- **`run --link-ts2rs-rt`**：在临时 crate 的 `Cargo.toml` 中加入可选 path 依赖 **`ts2rs_rt`**（须在本仓库源码树内构建；仅从 crates.io 安装时通常会失败）。生成代码默认仍不 `use ts2rs_rt`。
- **`compile --link-ts2rs-rt`**：仅为 CLI 一致性保留，不生成 `Cargo.toml`，无效果。

`compile` 可加 `--span-comments`，在生成的 Rust 中为每条语句前置 `// ts: path:line:col`（对照 TS 位置）。

## 仓库布局

| Crate | 说明 |
|-------|------|
| `ts2rs-parser` | swc 封装；`ParsedSource` 含 `source_map`；[`module_graph`](crates/ts2rs-parser/src/module_graph.rs) 多文件入口 |
| `ts2rs-hir` | IR、构建、语义、`emit_rust`；[`compile_graph`](crates/ts2rs-hir/src/lib.rs) 多模块；诊断与 codegen 共用节点 `Span` + 函数级 `ir_id`（见 [`ir.rs`](crates/ts2rs-hir/src/ir.rs)） |
| `ts2rs-lower` | `lower_program` / [`lower_module_graph`](crates/ts2rs-lower/src/lib.rs) |
| `ts2rs-driver` | 临时 crate + `cargo build`（需本机 `cargo` 在 `PATH`）；[`compile_entrypoint_to_executable`](crates/ts2rs-driver/src/lib.rs)；未找到 `cargo` 时 [`DriverError::CargoNotFound`](crates/ts2rs-driver/src/lib.rs) |
| `ts2rs_rt` | 可选运行时（预留） |
| `ts2rs-cli` | 命令行 `ts2rs` |

## 许可

MIT OR Apache-2.0
