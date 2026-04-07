# ts2rs

将 **TypeScript** 编译为 **Rust 源码**，再经 **cargo/rustc** 生成可执行文件的实验性编译器（Rust 实现）。

## 架构

解析（swc）→ **HIR**（[`ts2rs-hir`](crates/ts2rs-hir)）→ **语义检查**（符号、类型、简化 return 路径）→ **Rust 代码生成** → **cargo** 链接。

可选运行时 [`ts2rs_rt`](crates/ts2rs_rt) 预留扩展（当前 `console.log` 直接生成 `println!`）。

## 1.0 范围（当前版本）

- **矩阵覆盖**：README 下表中标为「支持」的特性，均有对应 TypeScript 样例与集成测试，目录为 [`crates/ts2rs-cli/tests/fixtures/`](crates/ts2rs-cli/tests/fixtures/)（含 §1.2：`empty_stmt`、`const_ok`、`assign_simple`、`nested_fn`、`for_loop`、`do_while_count`、`break_while`、`continue_while`、`import_add_main`+`add_dep`、多文件负例 `import_missing_export_*`、`circular_*`、`dup_*` 等；§1.3：`ternary_ok`、`template_ok`、`comma_ok`、`member_length_ok`、`logical_bool`、`logical_truthy_ok` 等；§1.4 字面量类型：`literal_type_ok`、`literal_type_fail`；§1.4 联合类型：`union_literal_ok`、`union_cond_ok`、`union_heterogeneous_fail`、`intersection_type_fail`、`union_mixed_cond_fail`；§1.4 `interface`：`interface_ok`、`export_interface_ok`、`interface_extends_fail`、`interface_generic_fail`；§1.4 `type` 别名：`type_alias_ok`、`type_alias_to_interface_ok`、`export_type_alias_ok`、`type_alias_generic_fail`、`type_alias_dup_fail`；§1.4 泛型拒绝：`generic_function_fail`；§3.1 语义边界：`let_dup_same_block_fail`、`let_shadow_nested_ok`、`param_let_same_name_fail`、`void_log_in_branch`；另有 `export_main`、`while_early`、`boolean_if`、`string_concat`、`ops`、`import_fail`）。手工大样例见 [`test-ts/main.ts`](test-ts/main.ts)（与同目录 [`test-ts/math.ts`](test-ts/math.ts) 多文件 `import`）。
- **诊断**：编译错误信息为**英文**，格式为 `path:line:col: message`（见 [`ts2rs-hir` 错误类型](crates/ts2rs-hir/src/error.rs)）。
- **CI**：推送与 PR 在 GitHub Actions 上运行 `cargo test --workspace` 与 `cargo clippy --workspace --all-targets`（[`.github/workflows/ci.yml`](.github/workflows/ci.yml)）。
- **非 1.0**：`tsconfig` 多入口、包名解析、`export *` 等仍为后续目标；**相对路径** `import { x } from "./dep.ts"` 已支持（[`parse_module_graph`](crates/ts2rs-parser/src/module_graph.rs) 构建模块图 + `validate_imports`，HIR [`compile_graph`](crates/ts2rs-hir/src/lib.rs)；入口文件须含 `main`，全局函数名唯一）。

## 诊断与前端健壮性（§1.1）

- **单条诊断**：[`ts2rs_hir::compile`](crates/ts2rs-hir/src/lib.rs) 与 CLI 在任一步失败时**只报告第一条**错误（[`CompileError`](crates/ts2rs-hir/src/error.rs)），同一次运行不会继续收集后续问题；若需「一屏多条」诊断，见 [PROJECT-TODO.md](PROJECT-TODO.md) 长期条目。
- **`export` 形态**：除 `export function …` 与顶层 `function …` 外，其余 `export`（如 `export { … }`、`export default`、`export * from`、`export const` / `class` 等）均**显式报错**（[`build.rs`](crates/ts2rs-hir/src/build.rs)）；负例样例见 `export_*_fail.ts`（与 [`cli_e2e.rs`](crates/ts2rs-cli/tests/cli_e2e.rs)）。
- **注释**：swc 产出的 `Program` **不携带**注释节点；[`ParsedSource`](crates/ts2rs-parser/src/lib.rs) 已含 `source_map` 供行列号。若要将 TS 注释反映到生成的 Rust，需在 parser 侧保留注释或扫描 token，并在 IR/codegen 中单独设计；**当前未实现**。

## 控制流与 return（简化语义）

实现见 [`sem.rs`](crates/ts2rs-hir/src/sem.rs) 中 `stmts_return` / `last_returns`。

- **非 `void` 函数**：在 [`check_function`](crates/ts2rs-hir/src/sem.rs) 末尾要求 `stmts_return(&f.body)` 为真，否则报错（「not all control paths return…」）。
- **判定方式（刻意简化）**：只看函数体**语句列表的最后一条**是否「在简化意义下保证 return」：
  - 最后一条是 `return`（带值）→ 通过；
  - 最后一条是 `if`，且 **then 与 else 两个分支都存在**时，要求**两个分支**各自 `stmts_return`；**无 `else` 的 `if` 不能**单独满足「最后一条」的 return 检查（即使 then 内全部 return）；
  - 最后一条是块 `{ ... }` → 递归看块内语句；
  - 最后一条是 `while` / `do-while` → 看**循环体**是否 `stmts_return`（不分析循环是否执行）。
- **与 TypeScript / `tsc`**：本规则**不等价**于可达性分析、never、或「所有分支均有值」的完整判定；可能拒绝在 TS 中可通过静态检查的程序，也可能接受在 TS 中更严格的写法。以编译器报错为准。

符号表（块作用域、`let`/`const` 同块重复、嵌套遮蔽、与形参同名）的边界样例见 fixtures：`let_dup_same_block_fail.ts`、`let_shadow_nested_ok.ts`、`param_let_same_name_fail.ts`。`void` 与 `console.log` 在分支内见 `void_log_in_branch.ts`。

## 语言支持矩阵

| 特性 | 状态 | 说明 |
|------|------|------|
| 单文件 `.ts` | 支持 | |
| `function` 顶层声明 | 支持 | `export function` 同文件内支持；其它 `export` 形式见上文 §1.1 |
| `import` | 部分支持 | 仅 `import { name } from "./relative.ts"`；依赖须 `export function name`；解析为模块图（不合并 AST）；见 `import_add_main.ts` 与负例 `import_missing_export_*`、`circular_*` |
| `number` / `boolean` / `string` / `void` | 支持 | `void` 仅作返回类型；`let` 不可用 `void` |
| `let`（单声明） | 支持 | 需类型注解与初始化；可变 `let` 可二次赋值（`IRStmt::Assign`） |
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
| 成员访问 | 部分支持 | 当前仅 `string` 的 `.length`（Rust `len()` 字节，ASCII 与 TS 一致） |
| `?.` / `??` | 部分支持 | `?.` 仅支持成员访问 `obj?.prop`，`??` 为受限空值子集；见 `optional_ok.ts`、`nullish_ok.ts`；完整语义见 §3.3 |
| 数组 / 对象字面量 | 部分支持 | 仅 `number[]` 与 `{ k: number }` 子集；见 `array_ok.ts`、`object_ok.ts`；完整类型语法见 §1.4 / §2.1 |
| `switch` | 不支持 | 显式诊断 |
| `return` | 支持 | 非 `void` 函数需满足简化 `stmts_return` 规则（见上文「控制流与 return」） |
| `void` 函数 | 支持 | 不要求 `return` 路径检查 |
| `+ - * /`、比较、`!`、一元 `-` | 支持 | 字符串仅 `+` 拼接 |
| `console.log(...)` | 支持 | 生成 `println!` |
| 字面量类型 | 部分支持 | `42`、`"a"`、`true` 等类型位置；向 `number`/`string`/`boolean` 拓宽；见 `literal_type_ok.ts`；`bigint`/模板字面量类型位置拒绝 |
| 联合类型 `A \| B` | 部分支持 | 嵌套 `|` 扁平化、排序去重；成员须**映射到同一 Rust 类型**（如均为 `number` 字面量或 `number` 与字面量）；`number \| string` 等无法在单一 Rust 类型上 codegen 时会报错；**交集** `A & B` 拒绝；条件位置须为单族联合；见 `union_*`、`intersection_type_fail.ts` |
| `interface`（受限） | 部分支持 | 顶层 `interface` / `export interface`；声明体与 `{ k: number }` 相同规则，解析为 [`TsType::ObjectNum`](crates/ts2rs-hir/src/ir.rs)（`build.rs` 中具名表）；类型位置用 `Point` 形式引用；**单文件**内按出现顺序声明，引用尚未声明的接口名会报错；**不**从依赖模块导入接口名；`extends`、泛型、可选属性拒绝；见 `interface_ok.ts`、`export_interface_ok.ts`、负例 `interface_extends_fail.ts`、`interface_generic_fail.ts` |
| `type` 别名（受限） | 部分支持 | 顶层 `type Id = T` / `export type`；与 `interface` **共用**同一张具名表（[`collect_named_types`](crates/ts2rs-hir/src/build.rs)），按**出现顺序**解析右侧 `T`；可与 `interface` 交错；重复名（含与 `interface` 同名）拒绝；泛型 `type` 拒绝；见 `type_alias_ok.ts`、`type_alias_to_interface_ok.ts`、`export_type_alias_ok.ts`、负例 `type_alias_generic_fail.ts`、`type_alias_dup_fail.ts` |
| 泛型 / 类型实参 | 不支持 | 显式拒绝（见下文「泛型与类型参数（当前仍拒绝）」）；负例 `generic_function_fail.ts`、`interface_generic_fail.ts`、`type_alias_generic_fail.ts` |
| 完整 TypeScript / `tsc` 语义 | 未实现 | 长期目标 |

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

## 构建

```bash
cargo build --release
cargo test
```

## 用法

```bash
cargo run -p ts2rs-cli -- compile path/to/app.ts -o out.rs
cargo run -p ts2rs-cli -- run path/to/app.ts
```

## 仓库布局

| Crate | 说明 |
|-------|------|
| `ts2rs-parser` | swc 封装；`ParsedSource` 含 `source_map`；[`module_graph`](crates/ts2rs-parser/src/module_graph.rs) 多文件入口 |
| `ts2rs-hir` | IR、构建、语义、`emit_rust`；[`compile_graph`](crates/ts2rs-hir/src/lib.rs) 多模块；诊断与 codegen 共用节点 `Span` + 函数级 `ir_id`（见 [`ir.rs`](crates/ts2rs-hir/src/ir.rs)） |
| `ts2rs-lower` | `lower_program` / [`lower_module_graph`](crates/ts2rs-lower/src/lib.rs) |
| `ts2rs-driver` | 临时 crate + `cargo build`；[`compile_entrypoint_to_executable`](crates/ts2rs-driver/src/lib.rs) 走模块图主路径 |
| `ts2rs_rt` | 可选运行时（预留） |
| `ts2rs-cli` | 命令行 `ts2rs` |

## 许可

MIT OR Apache-2.0
