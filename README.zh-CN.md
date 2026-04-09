[English](README.md)

# trust

将 **TypeScript** 编译为 **Rust 源码**，再经 **cargo/rustc** 生成可执行文件的实验性编译器（Rust 实现）。本仓库在工程上常作为 **trust** 子集使用。

另见 [CONTRIBUTING.zh-CN.md](CONTRIBUTING.zh-CN.md)、[CHANGELOG.zh-CN.md](CHANGELOG.zh-CN.md)，以及长期路线图 [PROJECT-TODO.zh-CN.md](PROJECT-TODO.zh-CN.md)（[English](PROJECT-TODO.md)）。

## 快速上手（先看这里）

### 1）初始化项目

```bash
trust init --dir my-trust-app
cd my-trust-app
trust run main.ts
```

### 2）快速添加 Rust extern

```bash
trust add url::Url::parse
```

该命令会自动更新 `Trust.toml`：

- `[dependencies]`（缺失时自动补 `url = "*"`）
- `[[rust_binding]]`（自动写入 `crate`、`type_name`、`rust_type`、`new`）

### 3）CLI 常用命令

```bash
# 仅检查（解析 + HIR + 语义）
trust check main.ts

# 输出 Rust 源码
trust compile main.ts -o out.rs

# 生成可执行文件（自行运行）
trust compile main.ts --exec -o ./app

# 一步编译并运行
trust run main.ts

# 项目模式
trust run --project tsconfig.json
```

## Trust 与 TypeScript 的差异

- 仅强类型：不支持隐式 `any`，也没有“先写后放宽”的软类型模式。
- 子集编译器：目标是静态可判定/可 codegen 的子集，不追求与完整 `tsc` 全等。
- 运行时模型有差异：`number` 在 Rust 侧是 `f64`，部分 JS 边界行为可能不同。
- 模块/依赖模型不同：不做 npm/node_modules 解析；Rust 依赖统一走 `Trust.toml`。
- 导入导出受限：仅支持文档列出的形态，超出即编译报错。

## 开发注意事项（高频坑）

- 函数参数与返回值请明确注解；`let/const` 的类型信息要清晰。
- 入口文件必须有 `main`；多文件模式下第一个 `.ts` 参数就是入口。
- `--project` 与位置参数 `.ts` 互斥。
- `--link-trust-rt` 只在 `compile --exec` 或 `run` 时有意义。
- `trust add` 仅接受 `crate::Type::new_fn` 形态（如 `url::Url::parse`）。
- Rust extern 方法签名必须在 `Trust.toml` 显式声明，不会从 crate 源码自动反射。

## Standard Lib 清单（trust 语言表面）

当前内建/标准化可用 API：

- Console：`console.log`、`console.error`、`console.debug`
- String：`.length`、`charAt`、`charCodeAt`、`slice`、`substring`、`indexOf`、`includes`、UTF-16 下标
- Number/Math：`Number.parseInt`、`Number.parseFloat`、`Math.abs/min/max/floor/ceil/sign/trunc/round/pow`
- JSON/URI：`JSON.parse`、`JSON.stringify`、`encodeURIComponent`、`decodeURIComponent`
- IO/网络：
  - `readLine()`（同步；在 async 函数体内会被拒绝）
  - `readFileText(path)`（同步读取文本，UTF-8）
  - `readFileTextAsync(path)`（异步读取文本，必须 `await`）
  - `fetchText(url)`
  - `fetch(url, init?)`、`await response.text()`、`await response.json()`
  - `response.body.getReader()` + `await reader.read()`（流式子集）
- Promise 子集：`await`、`Promise.all([...])`（同质子集）；`.then` 拒绝

---

以下内容为详细技术参考（架构、矩阵、诊断与实现细节）。

## 类型立场：强类型（strong typing，trust）

**trust 采用强类型（strong typing）：** 与隐式 `any`、运行期随意改型等宽松语义相对，受支持的程序必须在编译期内具备**静态、确定**的类型信息：参数与返回值须注解（或本子集内等价地可判定）；`let` / `const` 须带类型注解（或明确初始化且类型可推断）；**不**提供隐式 `any`、运行期随意改型、或「先写后推断全局放宽」等软类型语义。路线与验收以**静态类型检查**为准，与完整 TypeScript / `tsc` 的渐进式宽松模式**不一致**。

## 架构

解析（swc）→ **HIR**（[`trust-hir`](crates/trust-hir)）→ **语义检查**（符号、类型、简化 return 路径）→ **Rust 代码生成** → **cargo** 链接。

可选运行时 [`trust_rt`](crates/trust_rt)：当前**生成代码不依赖**本 crate；提供 `read_stdin_line` 等占位 API。控制台：`console.log` → `println!`，`console.error` / `console.debug` → `eprintln!`。

```mermaid
flowchart LR
  TS[TypeScript_source]
  PR[trust_parser_swc]
  HB[HIR_build]
  SE[Semantic_check]
  CG[Codegen_Rust]
  LO[trust_lower]
  CLI[trust_cli]
  DRV[trust_driver]
  TS --> PR --> HB --> SE --> CG --> LO --> CLI
  CLI --> DRV
```

[`trust-lower`](crates/trust-lower) 串联 HIR 构建、语义与代码生成。[`trust-driver`](crates/trust-driver) 负责临时 crate 与 `cargo`（`trust run` 使用）。

## Rust 生态：`Trust.toml`（非 npm）

编译器可通过清单把 **crates.io（或 path/git）依赖** 链进生成 crate，并在 TypeScript 中调用。**不是** npm、`node_modules`，也**不是** `tsc` 的包解析。

- **发现规则**：自入口 `.ts` 所在目录**向上**查找 **`Trust.toml`**（见 [`discover_trust_toml`](crates/trust-manifest/src/lib.rs)）。
- **`[dependencies]`**：与 Cargo 子集对齐的表；条目会**合并**进生成 crate 的 `Cargo.toml`（见 [`crate_writer`](crates/trust-driver/src/crate_writer.rs)）。TS 里 `import … from "…"` 的说明符须与依赖**键名**一致（例如 `import { Regex } from "regex"` 需要存在 `regex = "…"`）。
- **`[[rust_binding]]`**：把从该 crate 键导入的 TS 符号映射到 Rust 类型路径、可选的 `new`（`rust` 路径 + 是否在 `Result` 上 `unwrap`）、以及 `method`（`name`、`args` 如 `string` / `number` / `boolean`、`returns`）。**不做 API 反射**：编译器**不会**扫 Rust 源码或 rustdoc，绑定由作者维护。
- **代码生成**：`new T("…")` 降为配置的构造路径；方法降为 Rust **固有方法**调用。TS `string` 在生成代码中为 Rust `String`；绑定里将形参标为 `string` 时，调用点会生成 `.as_str()`，以匹配如 `regex::Regex::is_match(&self, &str)` 等签名。

**serde 类 crate**：仅在 `[dependencies]` 中声明只会增加 Cargo 依赖（供后续 `#[derive]` 或传递依赖）；一般**不能**指望 `import { Serialize } from "serde"`，除非手写绑定，且纯过程宏 API 在 TS 侧通常无意义。

样例：[`tests/fixtures/trust_regex/`](crates/trust-cli/tests/fixtures/trust_regex/) — 测试 `run_trust_regex_ok_prints_one`、`compile_trust_regex_ok_emits_regex_crate`。

## 不支持的 TypeScript 特性（trust 强类型拒斥边界）

以下为常见**显式拒绝**形态（诊断为英文；详见 [`build.rs`](crates/trust-hir/src/build.rs) / [`sem.rs`](crates/trust-hir/src/sem.rs)）。与下文「泛型与类型参数」表及语言矩阵**互补**；矩阵中已标为支持/部分支持的特性（如受限 **`?.`**、单态化泛型、顶层 `class` 子集）**不在此列**。

| 用户可见形态 | 说明 |
|--------------|------|
| 非 `export function` / 非顶层 `function` / 非相对 **`export * from "./…"`** / 非 **`export { … } from "./…"`** / 非 **`export default function main`** / 非 **`export default main`**（须先有 `function main`）的 `export` | 如无 `from` 的 `export { }`、任意非 `main` 的 `export default`、`export * as`、`export const`、`export class` 等（**顶层、无 `export` 的 `class`** 见矩阵与 [PROJECT-TODO.zh-CN.md §13.3](PROJECT-TODO.zh-CN.md)） |
| 复杂泛型语义 | 高阶推导、复杂约束与完整 TS 泛型语义仍未实现；**调用处显式类型实参的单态化子集**见矩阵「泛型与类型参数」与 [§13.1](PROJECT-TODO.zh-CN.md) |
| 可选链（拒斥边界） | 受限 **`f?.()`** / **`recv?.m()`** 已支持（`optional_call_ok.ts`）；任意 callee、非标识符 callee 等仍可能拒绝（见 `optional_chain_fail.ts`） |
| `interface extends`、跨依赖文件导入接口/类型**名** | **单文件**具名表；同文件内已支持嵌套 `number` 对象与 `k?: number`（**非**完整 `tsc` 结构规则） |
| 交集 `A & B` | 拒绝 |
| `bigint`、类型位置模板字面量类型 | 拒绝 |
| 完整 `tsc` / TS 结构子类型全集 / 超出 §13.2 的 HOF | 完整类型检查与结构子类型全集未实现；**受限**函数类型与箭头值见矩阵与 [§13.2](PROJECT-TODO.zh-CN.md) |

## 1.0 范围（当前版本）

- **矩阵覆盖**：下表「支持」或「部分支持」行均有代表性 **fixture**（[`fixtures/`](crates/trust-cli/tests/fixtures/)）与 **[`cli_e2e.rs`](crates/trust-cli/tests/cli_e2e.rs)** 测试对应，详见下文 **[矩阵与集成测试对照](#矩阵与集成测试对照)**。手工大样例另见 [`test-ts/main.ts`](test-ts/main.ts)、[`test-ts/math.ts`](test-ts/math.ts)。**回归**用例目录见 [`tests/regression/`](crates/trust-cli/tests/regression/)。
- **诊断**：编译**错误**为**英文**，格式为 `path:line:col: message`（[`CompileError`](crates/trust-hir/src/error.rs)）。**警告**（如不可达代码）同为该格式，经 [`CompileWarning`](crates/trust-hir/src/error.rs) 收集；成功编译时 CLI / driver 将警告打印到 **stderr**，不抬高退出码。
- **CI**：推送与 PR 在 GitHub Actions 上运行 `cargo fmt --all --check`、`cargo test --workspace` 与 `cargo clippy --workspace --all-targets`（[`.github/workflows/ci.yml`](.github/workflows/ci.yml)）。
- **非 1.0**：与完整 `tsc` 的 `tsconfig` 行为对齐、任意 `export default` 表达式、`export * as` 等；**不计划支持 npm / `node_modules` / 包管理器式模块解析。** Rust crate 仅通过 **`Trust.toml`** 接入（见上文 **Rust 生态：`Trust.toml`**）。**相对路径** `import { x } from "./dep.ts"`、**`import main from "./dep.ts"`**（绑定名**必须**为 `main`，对应依赖的默认导出 `main`）与相对 **`export *` / `export { … } from`**（桶文件重导出）已支持；CLI 支持**多根**（位置参数 `.ts`）或 **`--project`** 简化 JSON（**`extends`**、**`files`**、**`include` / `exclude` glob**，见 [`tsconfig_resolve`](crates/trust-cli/src/tsconfig_resolve.rs)、[`graph_loader`](crates/trust-cli/src/graph_loader.rs)）+ [`parse_module_graph_with_extra_roots`](crates/trust-parser/src/module_graph.rs) + [`validate_imports`](crates/trust-parser/src/module_graph.rs)，HIR [`compile_graph`](crates/trust-hir/src/lib.rs)；入口须含 `main`，全局函数名唯一。**可选增量**：`compile` / `run` 的 **`--incremental [DIR]`** 将各模块 HIR 片段写入磁盘缓存（无参时默认 `.trust-cache`）；每次仍会解析全部 `.ts`，节省主要在 HIR 构建与 I/O；见 [`incremental.rs`](crates/trust-cli/src/incremental.rs)。

## 诊断与前端健壮性（§1.1）

- **多条编译错误**：build 与语义阶段可在**一次失败**中收集多条诊断（[`CompileError::Many`](crates/trust-hir/src/error.rs)），按行输出多条 `path:line:col: message`（已排序）。解析器 [`parse_typescript_file`](crates/trust-parser/src/lib.rs) 会输出 swc **`take_errors()` 的全部**诊断。**单态化**与 **codegen** 仍可能在首条内部错误处停止。**成功时**可附带多条 [`CompileWarning`](crates/trust-hir/src/error.rs)（[`trust_lower`](crates/trust-lower/src/lib.rs) 同形）。
- **`export` 形态**：`export function …`、顶层 `function …`、**`export default function main`**、**`export default main`**（同模块须有 `function main`）、相对 **`export * from "./…"`**、**`export { a as b } from "./…"`**（**函数**导出，见 [`build.rs`](crates/trust-hir/src/build.rs)、[`module_graph`](crates/trust-parser/src/module_graph.rs)）；`export class` / `export const` / 其它 `export default` / 无 `from` 的 `export { x }` 等仍**报错**；**顶层、无 `export` 的 `class`** 见矩阵。正例 `export_default_*_ok.ts`，负例 `export_*_fail.ts` 与 [`cli_e2e.rs`](crates/trust-cli/tests/cli_e2e.rs)。
- **注释**：swc 的 `Program` **仍无**注释节点；[`ParsedSource`](crates/trust-parser/src/lib.rs) 含 `source_map` 与解析器收集的 `comments`（swc 注释表）。**将 TS leading 注释写入生成 Rust** 为可选：[`CodegenOptions::emit_ts_source_comments`](crates/trust-hir/src/codegen.rs)，CLI `trust compile --ts-source-comments`，在语句与顶层函数前输出 `//` 行；trailing 与大粒度 lowering 后的位置不保证（见 [PROJECT-TODO.zh-CN.md §14](PROJECT-TODO.zh-CN.md)）。
- **后续与 backlog**（更细粒度注释映射、完整工程工具链等）：见 [PROJECT-TODO.zh-CN.md §14 — 工具链与体验](PROJECT-TODO.zh-CN.md)。

## 控制流与 return（简化语义 + §3.4）

实现见 [`sem.rs`](crates/trust-hir/src/sem.rs)（`fn_body_returns`、`tail_returns_last_only`、`tail_returns_while_body`、`stmt_block_diverges` 等）。

- **非 `void` 函数**：在 [`check_function`](crates/trust-hir/src/sem.rs) 末尾要求 `fn_body_returns(&f.body, &ret)` 为真，否则报错（「not all control paths return…」）。
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
| `import` | 部分支持 | `import { name } from "./relative.ts"` 与 **`import main from "./relative.ts"`**（绑定名须为 `main`）；依赖模块须在**有效导出**中含对应符号；**Rust crate**：当 `Trust.toml` 的 `[dependencies]` 含键名且 `[[rust_binding]]` 声明类型时，支持 `import { T } from "crate_key"`（见上文 **Rust 生态：`Trust.toml`**）；`trust_regex/main.ts`；负例 `import_missing_export_*`、`import_default_wrong_binding_fail.ts`、`circular_*` |
| `number` / `boolean` / `string` / `void` | 支持 | `void` 仅作返回类型；`let` 不可用 `void` |
| `let`（单声明） | 部分支持 | 须类型注解；可无初始化，但使用前须明确赋值（见上文 §3.4）；可变 `let` 可二次赋值（`IRStmt::Assign`）；见 `definite_assign_ok.ts` |
| `const` | 支持 | 与 `let` 同形，语义禁止赋值 |
| 块 `{ }`、多条语句 | 支持 | 含空语句 `;`、块语句 |
| `if` / `else`、`while`、`do-while` | 支持 | 条件为 `number`（非 0 为真）或 `boolean`；或**同一 primitive 族**的联合（如 `1 \| 2`、`true \| false`），不含 `number \| boolean` 等混合 |
| C 风格 `for(;;)` | 支持 | `init`/`update` 为单声明或表达式；`update` 可为 `i = i + 1` |
| `for..in` | 部分支持 | 支持对象/class-instance 键遍历与 `number[]` 下标字符串键；循环变量统一 `string`；`for-of` 仍不支持 |
| `break` / `continue` | 支持 | 须在循环内；带 label 未支持 |
| 嵌套 `function` | 部分支持 | 无闭包捕获子集；见 `nested_fn.ts` |
| 逻辑与/或 | 部分支持 | `boolean` 与 `number`（`number` 按 `!= 0` 真值，与条件位置一致）；结果类型为 `boolean`；见 `logical_bool.ts`、`logical_truthy_ok.ts`；非 `boolean`/`number` 联合仍拒绝 |
| 三元 `?:` | 支持 | 两分支需同类型；条件为 `number` 或 `boolean`；见 `ternary_ok.ts` |
| 模板字符串 | 支持 | 无 tag；插值须非 `void`；见 `template_ok.ts` |
| 逗号表达式 | 支持 | 取最后一项类型与值；见 `comma_ok.ts` |
| 成员访问 | 部分支持 | `string.length` 为 JS **UTF-16 码元数**；`string[i]` 为 UTF-16 下标（单码元 `string`）；`number[].length`；对象字段 `length`；**`obj.m(args)`** 脱糖为全局 **`m(receiver, ...args)`**；**一层链式** `f().prop` / `f().m()`（`chain_call_ok.ts`）；**未**支持 `obj[expr](...)`；见 `string_utf16_length.ts`、`method_call_ok.ts`、`stdlib_hir_ok.ts` 等 |
| `?.` / `??` | 部分支持 | `?.` **成员**与**调用** `f?.()` / `recv?.m()`（`optional_call_ok.ts`）；`??` 对同族 `Union` 去 `null`/`undefined` 合并扩展；`optional_ok.ts`、`nullish_ok.ts`；完整 discriminated 收窄见 §3.3 |
| 数组 / 对象字面量 | 部分支持 | `number[]`；对象类型为 **`number` 叶子**、**嵌套**对象与 **`k?: number`**（宽度/可选规则与完整 `tsc` 不同，见 [`sem/helpers.rs`](crates/trust-hir/src/sem/helpers.rs)）；运行时表示为 **`serde_json::Value`**；见 `array_ok.ts`、`object_ok.ts`、`nested_object_ok.ts` |
| `switch` | 部分支持 | `case` 仅 `number`/`boolean` **字面量**；`default` 须**最后**；**无** `case` 间穿透（空 `case` 体拒绝）；`case` 末尾 `break` 在 build 剥离；判别式与 `if` 条件类型规则一致；见 [`switch_ok.ts`](crates/trust-cli/tests/fixtures/switch_ok.ts)、负例 [`switch_fail.ts`](crates/trust-cli/tests/fixtures/switch_fail.ts) |
| `return` | 支持 | 非 `void` 函数需满足 `fn_body_returns`（含提前穷尽返回 + 尾部规则，见上文「控制流与 return」） |
| `void` 函数 | 支持 | 不要求 `return` 路径检查 |
| `+ - * /`、比较、`!`、一元 `-` | 支持 | 字符串仅 `+` 拼接；`number` 在 Rust 侧为 **`f64`**；见下文 §4.1 |
| `Math.*` 内建 | 部分支持 | `abs`、`min`、`max`、`floor`、`ceil`、`sign`、`trunc`、`round`、`pow`（`f64`；`pow` 非负指数）；见 `math_builtin.ts`、`stdlib_hir_ok.ts` |
| `Number.*` / `JSON.*` / `String` 方法 / `readLine` | 部分支持 | `Number.parseInt` / `parseFloat` → **`f64`**；`JSON.stringify`（`string` / `number` / `boolean` / trust **对象**形状）；`JSON.parse`（字面量折叠为 trust 闭合形状，含嵌套纯 number 对象；非常量仍为 JSON number 文档 → `f64`，`serde_json`）；`encodeURIComponent` / `decodeURIComponent`（`urlencoding`）；`charAt` 等（UTF-16）；`readLine()`；`readFileText(path)`（同步文本读取，UTF-8）；见 `stdlib_hir_ok.ts`、`json_uri_trust_ok.ts` |
| `console.log` / `console.error` / `console.debug` | 支持 | `log` → `println!`；`error` / `debug` → `eprintln!`；多参数均为 `"{}"` **空格分隔**（与 §4.1 一致） |
| 字面量类型 | 部分支持 | `42`、`"a"`、`true` 等类型位置；向 `number`/`string`/`boolean` 拓宽；见 `literal_type_ok.ts`；`bigint`/模板字面量类型位置拒绝 |
| 联合类型 `A \| B` | 部分支持 | 嵌套 `|` 扁平化、排序去重；成员须**映射到同一 Rust 类型**（如均为 `number` 字面量或 `number` 与字面量）；`number \| string` 等无法在单一 Rust 类型上 codegen 时会报错；**交集** `A & B` 拒绝；条件位置须为单族联合；见 `union_*`、`intersection_type_fail.ts` |
| `interface`（受限） | 部分支持 | 顶层 `interface` / `export interface`；声明体为嵌套/可选 `ObjectNum` 字段（**单编译单元**）；类型位置用 `Point` 形式引用；**不**从依赖模块导入接口/类型**名**；对象类型上**无**可调用的方法成员类型（`obj.m()` 仍靠全局函数脱糖）；`extends`、泛型仍拒绝；见 `interface_ok.ts`、`nested_object_ok.ts`、`export_interface_ok.ts`、负例 `interface_extends_fail.ts`、`interface_generic_fail.ts` |
| `type` 别名（受限） | 部分支持 | 顶层 `type Id = T` / `export type`；与 `interface` **共用**同一张具名表（[`collect_named_types_with_errors`](crates/trust-hir/src/build/build_types.rs)），按**出现顺序**解析右侧 `T`；可与 `interface` 交错；重复名（含与 `interface` 同名）拒绝；泛型 `type` 拒绝；见 `type_alias_ok.ts`、`type_alias_to_interface_ok.ts`、`export_type_alias_ok.ts`、负例 `type_alias_generic_fail.ts`、`type_alias_dup_fail.ts` |
| 泛型 / 类型实参 | 部分支持 | 单态化：可写显式 `f<number>(x)`，或在实参类型可合成时省略（字面量、已注解的 `let`/参数）；不可推、冲突或多处错误会分别报错；`obj.m` 脱糖后的泛型全局函数同样推断；Rust 侧符号为 `name__` + 16 位十六进制指纹 |
| 高阶函数 | 部分支持 | 函数类型与箭头闭包（`(number) => number` → `(f64) -> f64`）；变量调用 `f(...)` 等 |
| `async` / `await` / `Promise` / `fetch` / `fetchText` | 部分支持 | **`fetchText(url)`** → `Promise<string>`；**`readFileTextAsync(path)`** → `Promise<string>`（必须 `await`）；**`fetch(url, init?)`** → `Promise<Response>`（`status` / `ok` / `await .text()` / `await .json()` JSON number → `f64`，`serde_json`）；**`init`** 可为字面量 `method`、`headers`（值为字符串字面量）、可选 `body`；**`Promise.all`** 同质 `number` / `string` / `fetch` 的 Response（顺序 `.await`）；**`.then`** 拒绝；TLS 为 **rustls**；HTTP/2 由协商决定，**不保证**与某一 Node 版本完全一致；完整 WHATWG `fetch` 仍为 backlog；见 `fetch_response_ok.ts`、`fetch_post_init_ok.ts` 与各 `compile_async_*` / `compile_fetch_*` / `compile_promise_*` 测试 |
| class / this / extends / super | 部分支持 | class 子集已降级到构造函数/方法函数；sem 已校验继承关系、`super(...)` 位置与基础 `override`；见 `class_*` fixtures |
| 完整 TypeScript / `tsc` 语义 | 未实现 | 长期目标 |

### 矩阵与集成测试对照

下列按主题归纳「语言支持矩阵」行与 **fixture** / **`cli_e2e` 测试**（命名前缀 `run_`、`compile_`、`check_` 等；完整列表以测试文件为准）。「完整 TS」等无单行 fixture。

| 主题 | 代表性 fixture | 代表性集成测试 |
|------|----------------|----------------|
| 单文件 / 算术 / 条件 / 字符串 | `sample.ts`、`ops.ts`、`boolean_if.ts`、`string_concat.ts` | `compile_writes_rust`、`compile_exec_writes_binary_and_runs`、`compile_exec_without_o_defaults_to_entry_stem_in_cwd`、`run_prints_main_result`、`run_ops_prints_six` 等 |
| `import` / 多文件 / 导出 / default | `import_add_main.ts`+`add_dep.ts`、`export_default_*_ok.ts`、`multi_entry_*`、`export_main.ts` | `run_import_add_main_prints_three`、`run_export_default_function_main_prints_42`、`run_multi_entry_extra_roots_prints_main`、`compile_export_main_writes_ts_main`、`run_reexport_export_star_ok` |
| `Trust.toml` / Rust extern | `trust_regex/main.ts` + `trust_regex/Trust.toml` | `run_trust_regex_ok_prints_one`、`compile_trust_regex_ok_emits_regex_crate` |
| 增量 HIR（`--incremental`） | e2e 临时目录 `lib.ts` + `app.ts` | `compile_incremental_rebuilds_only_changed_module` |
| 负例（import/export/重复） | `import_missing_export_*`、`circular_*`、`dup_*`、`export_*_fail.ts`、`import_fail.ts` | `compile_import_missing_export_fails`、`compile_circular_import_fails` 等 |
| `let`/`const`/块/赋值 | `const_ok.ts`、`assign_simple.ts`、`empty_stmt.ts`、`let_if.ts` | `run_const_ok_prints_42`、`run_assign_simple_prints_five` 等 |
| 语义边界（重复/shadow/void 分支） | `let_dup_same_block_fail.ts`、`let_shadow_nested_ok.ts`、`param_let_same_name_fail.ts`、`void_log_in_branch.ts` | 对应 `compile_*` / `run_void_log_in_branch_prints_branch` |
| 控制流与 return / 不可达 | `while_early.ts`、`for_loop.ts`、`for_in_*.ts`、`do_while_count.ts`、`break_while.ts`、`continue_while.ts`、`early_return_unreachable.ts`、`definite_assign_*.ts` | `run_while_early_prints_three`、`run_for_in_object_keys_ok_prints_three`、`compile_for_in_non_object_fails` 等 |
| 逻辑/三元/模板/逗号 | `logical_bool.ts`、`logical_truthy_ok.ts`、`ternary_ok.ts`、`template_ok.ts`、`comma_ok.ts` | `run_logical_bool_prints_one`、`run_ternary_ok_prints_one` 等 |
| 成员 / `Math` / HIR 标准库 / 链式 | `string_utf16_length.ts`、`math_builtin.ts`、`stdlib_hir_ok.ts`、`json_uri_trust_ok.ts`、`chain_call_ok.ts` 等 | `run_stdlib_hir_ok_prints_expected`、`run_json_uri_trust_ok_prints_expected`、`run_chain_call_ok_prints_six`、`compile_stdlib_hir_ok_writes_utf16_and_json_helpers`、`compile_json_uri_trust_ok_emits_serde_json_and_urlencoding` 等 |
| `?.` / `??`（支持子集） | `optional_ok.ts`、`nullish_ok.ts`、`nullish_fn_ok.ts`（`check`）、`optional_call_ok.ts` | `run_optional_ok_prints_two`、`run_nullish_ok_prints_one`、`check_nullish_fn_union_ok`、`run_optional_call_ok_prints_five` |
| 数组 / 对象字面量 | `array_ok.ts`、`object_ok.ts`、`nested_object_ok.ts`；负例 `array_fail.ts` | `run_array_ok_prints_two`、`run_object_ok_prints_three`、`run_nested_object_ok_prints_three`、`compile_array_return_type_mismatch_fails` |
| `switch` | `switch_ok.ts`、`switch_fail.ts` | `run_switch_ok_prints_seven`、`compile_switch_fallthrough_fails` |
| `console` | `console_stderr.ts`、`void_log.ts` | `compile_console_stderr_writes_eprintln`、`run_void_log_in_branch_prints_branch` |
| 字面量类型 / 联合 / 交集 | `literal_type_*.ts`、`union_*.ts`、`intersection_type_fail.ts` | `run_literal_type_ok_prints_eight`、`compile_union_heterogeneous_fail_errors` 等 |
| `interface` / `type` / 泛型子集 | `interface_*.ts`、`type_alias_*.ts`、`generic_function_ok.ts`、`generic_method_call_infer_ok.ts`、`generic_function_*_fail.ts` | `run_interface_generic_ok_prints_zero`、`run_type_alias_generic_ok_prints_zero`、`run_generic_function_ok_prints_three`、`run_generic_method_call_infer_ok_prints_three`、`compile_generic_function_infer_conflict_fails` 等 |
| class 子集 | `class_basic_ok.ts`、`class_this_method_ok.ts`、`class_extends_ok.ts`、`class_super_ctor_ok.ts`、`class_*_fail.ts` | `run_class_basic_ok_prints_five`、`run_class_extends_ok_prints_seven`、`compile_class_super_invalid_fails`、`compile_class_override_mismatch_fails` |
| 嵌套函数 | `nested_fn.ts` | `run_nested_fn_prints_nine` |
| 极简 tsconfig / `--project` | `multi_entry_tsconfig.json` + `multi_entry_*.ts` | `run_project_tsconfig_prints_main`、`run_project_tsconfig_extends_include_ok` |
| async / `Promise` / HTTP | `async_mvp_compile_ok.ts`、`async_control_flow_ok.ts`、`promise_all_fetch_ok.ts`、`fetch_response_ok.ts`、`fetch_post_init_ok.ts` | `compile_async_mvp_writes_tokio_and_await`、`compile_async_control_flow_if_while_await_ok`、`compile_promise_all_fetch_alias_ok`、`compile_fetch_response_ok`、`compile_fetch_post_init_ok`、`compile_promise_then_fails` |
| CLI：`check` / `--emit-ir` | `sample.ts`、`switch_fail.ts` | `check_sample_ok`、`compile_emit_ir_stderr_contains_ir_module` |
| 可选链 / `??` / 对象字段（负例边界） | `optional_chain_fail.ts`、`nullish_fail.ts`、`object_fail.ts` | `compile_optional_call_bad_callee_fails` 等 |
| 回归锚点（与 fixture 重复语义） | [`tests/regression/switch_fallthrough_regression.ts`](crates/trust-cli/tests/regression/switch_fallthrough_regression.ts) | `regression_switch_fallthrough_check_fails` |

## 类型层路线（§1.4）

**字面量类型**、**联合类型**、**受限 `interface`**、**受限 `type` 别名**与**泛型边界文档**子项已勾选（见 [PROJECT-TODO.zh-CN.md §1.4](PROJECT-TODO.zh-CN.md)）。与已实现子集（如注解中的 `number[]`、仅 `number` 字段的对象类型）的边界与拆分里程碑见 [PROJECT-TODO.zh-CN.md §1.4](PROJECT-TODO.zh-CN.md)。空值与收窄与下文「语义与类型路线（§3.3）」及 [PROJECT-TODO.zh-CN.md §3.3](PROJECT-TODO.zh-CN.md) 交叉：**sem 已实现** `Union` 上去除空值后与 `??` 右侧在 primitive 同族或 **兼容 `Fn` 类型**时的结果类型合并；**完整** discriminated 收窄仍待后续。`nullish_ok.ts` 等覆盖基础子集；`null`/`undefined` 与**异质**成员（如 `null | Fn`）的联合若在具名绑定上无法映射到单一 Rust 类型，**compile** 仍可能在 codegen 拒绝——与「heterogeneous union」诊断一致；`nullish_fn_ok.ts` 以 `trust check` 验证 sem。

### 泛型与类型参数（单态化子集）

- 泛型函数声明可通过；**单态化**在 `sem` 中、逐函数类型检查之前运行。调用可写显式类型实参，也可在能从**实参表达式合成类型**（数字/字符串/布尔/`null`/`undefined` 字面量，或带类型注解的局部/参数）与形参签名对齐时**省略**实参。
- 无法合成类型、对同一类型参数**推断冲突**、或形参类型尚不支持推断时会报错；一次编译可收集**多条**单态化相关诊断。
- `obj.m(args)` 脱糖为 `m(obj, ...args)`；若 `m` 为泛型顶层函数，推断规则相同（含 receiver）。
- 生成的 Rust 函数名为 `原名__` + 16 位十六进制（对规范类型键做 FNV-1a）；IR 上 `mono_origin` 仍保留可读实例说明。
- 泛型 `interface` / `type` 声明可解析于当前受限类型子集。
- 完整 TS 式推导、丰富约束与高阶多态等仍在后续范围。

## 语义与类型路线（§3.3）

与 [PROJECT-TODO.zh-CN.md §3.3](PROJECT-TODO.zh-CN.md) 对应；本小节描述**当前**边界与后续方向（矩阵中 `?.` / `??` 一行「完整语义」指本节）。

### 与 §1.4 的衔接

字面量类型与联合类型在 HIR 中已与 [`TsType`](crates/trust-hir/src/ir.rs) 对齐；`??` 与 `?.` 在 **sem** 上已实现上述同族 / `Fn` 兼容合并（见 `nullish_ok.ts`、`optional_ok.ts`、`nullish_fn_ok.ts`）。**完整** discriminated 收窄仍属后续；扩展时需与 §1.4 联合类型规则一致，避免与受限 `TsType` 语义冲突。

### `null` / `undefined` 与 `strictNullChecks`

[`TsType`](crates/trust-hir/src/ir.rs) 已包含 `Null`、`Undefined` 等变体；本编译器**没有** TypeScript `strictNullChecks` 的等价开关。若未来对齐 `tsc` 空值规则，需单独定义策略（例如默认宽松与显式严格模式）。

### 结构类型与名义类型

顶层 `interface` / `type` 按**名字**在单文件具名表解析（名义侧）；对象字面量与赋值、成员访问的类型检查在 [`sem.rs`](crates/trust-hir/src/sem.rs) 中按现有结构化规则进行。生成 Rust 为具名函数与值类型，**不**实现 TypeScript 结构子类型全集。

### 函数类型与高阶函数

当前已支持受限高阶函数子集：函数类型注解、箭头函数值、变量调用 `f(...)`、函数参数传递与返回函数；codegen 侧闭包路径目前限制为 `(number) => number` 的严格子集。

更多进阶条目见 [PROJECT-TODO.zh-CN.md §3.3–3.4](PROJECT-TODO.zh-CN.md)。

## 算术、`/` 与溢出（codegen，§4.1）

- **`number` → `f64`**：算术、`Math.*`、`Number.*` 等与 IEEE-754 双精度更接近 TS。
- **除法 `/`**：`f64` 除法（与旧版 `i32` 向零截断不同）。
- **NaN / ∞**：可能出现；与 V8 的 `number` 边界情况未必逐位一致。
- **`console.log` / `console.error` / `console.debug`**：多参数时格式串为 `"{}"` 之间带**空格**（如 `println!("{} {}", …)` / `eprintln!(…)`），与 [`emit_builtin_log`](crates/trust-hir/src/codegen.rs) 实现一致。

## 构建

```bash
cargo build --release
cargo test
```

## 用法

```bash
cargo run -p trust-cli -- compile path/to/app.ts -o out.rs
cargo run -p trust-cli -- compile path/to/app.ts -o ./my-app --exec
# --exec 时可省略 -o：在当前工作目录生成与入口文件主文件名相同的可执行文件
cargo run -p trust-cli -- compile --exec path/to/app.ts
cargo run -p trust-cli -- compile path/to/entry.ts path/to/extra.ts -o out.rs
cargo run -p trust-cli -- run path/to/app.ts
cargo run -p trust-cli -- run --project path/to/tsconfig.json
cargo run -p trust-cli -- compile path/to/entry.ts -o out.rs --incremental .trust-cache
cargo run -p trust-cli -- check path/to/app.ts
cargo run -p trust-cli -- init --dir my-trust-app
cargo run -p trust-cli -- add url::Url::parse --dir my-trust-app
```

### CLI（子命令与选项）

| 子命令 | 作用 |
|--------|------|
| **`compile`** | 解析 → HIR → 语义 → 非 **`--exec`** 时 **`-o` 必填**（`.rs`）；**`--exec`** 时经临时 crate + **`cargo build`** 写可执行文件：可显式 **`-o`**，或省略 **`-o`** 则在**当前工作目录**生成与**入口文件主文件名**同名的文件 |
| **`run`** | 同上后写入临时 crate，**`cargo build`**（默认 **`--release`**）并运行生成的可执行文件 |
| **`check`** | 仅解析 + HIR + **语义检查**，不写 `.rs`、**不**调用 `cargo` |
| **`init`** | 初始化 trust 项目模板（`main.ts`、`math.ts`、`strutil.ts`、`Trust.toml`） |
| **`add`** | 根据 `crate::Type::new_fn` 快速写入 `Trust.toml` 的依赖与 `[[rust_binding]]` |

**全局**（可写在子命令前，如 `trust -q run …`）：

| 选项 | 说明 |
|------|------|
| **`-q` / `--quiet`** | 成功时不打印 **warning**（错误仍输出） |
| **`--color`** | `auto` / `always` / `never`，帮助文本等着色；`never` / `always` 在解析前会同步 `NO_COLOR`（亦可直接设 `NO_COLOR=1`） |

**`compile`**：`--span-comments`、`--ts-source-comments`（将 TS leading 注释写成 Rust `//` 行）、`--emit-ir`（将 [`IRModule`](crates/trust-hir/src/ir.rs) 的 `Debug` 打到 **stderr**，输出可能很大，仅调试用）、**`--exec`**（见上表：**`-o`** 为可执行文件路径，或省略则用入口主文件名写到 **cwd**；**Windows** 上默认名会加 **`.exe`**；与 `run` 的 `cargo build` 一致）、**`--incremental` / `--incremental DIR`**（多文件 HIR 片段缓存；单独写 `--incremental` 时默认目录 `.trust-cache`）。**仅在 `--exec` 时**：**`--link-trust-rt`**、**`--debug`**、**`-O` / `--release`**（含义与互斥关系同 `run`）。

**`run`**：`--link-trust-rt`；**`--debug`** 使用非 release 的 `cargo build`（`target/debug/`）；**`-O` / `--release`** 显式要求 release 构建（与默认一致，与 `--debug` 互斥）；**`--incremental`** 与 `compile` 相同。

**`init`**：

- `--dir <DIR>` 初始化目录（默认 `.`）
- `--force` 覆盖同名已有文件

**`add`**：

- 位置参数 `RUST_PATH`：必须是 `crate::Type::new_fn`（例如 `url::Url::parse`）
- `--dir <DIR>`：`Trust.toml` 所在目录（默认 `.`）
- 行为：
  - 若 `[dependencies]` 无该 crate，则自动补 `crate = "*"`
  - 创建或更新匹配的 `[[rust_binding]]`，写入：
    - `crate`、`type_name`、`rust_type`
    - `new = { rust = "...", unwrap = true }`
    - `method = []`（缺失时初始化）

**退出码**：**`0`** 表示 trust 与子程序均成功；**`trust` 自身失败**（解析、语义、找不到 `cargo` 等）为 **`1`**；**`run`** 在已生成并成功启动子进程后，若子进程非零退出，则 **trust 以该进程的退出码退出**（无 `code()` 时如信号则 **`1`**）。Warning **不**抬高退出码（与上文「诊断」一致）。

- **多文件**：第一个位置参数为**入口**（须含 `export function main`），其余为**额外根**（入口 DFS 未覆盖的 `.ts` 仍会加入模块图）。
- **`--project` tsconfig**（简化 JSON）：可选 **`extends`**、**`files`**、**`include`**、**`exclude`**（glob）；路径相对**书写该字段的配置文件**所在目录；**`exclude`** 在 `extends` 链上**累积**。若合并后 **`files` 非空**则只用 `files`；否则展开 **`include`**（仅 `.ts`）。**仅 `include`** 时匹配文件会排序，**入口**为字典序第一个——需固定顺序时用显式 **`files`**。与多个 `.ts` 位置参数**互斥**。
- **`run --link-trust-rt`**：在临时 crate 的 `Cargo.toml` 中加入可选 path 依赖 **`trust_rt`**（须在本仓库源码树内构建；仅从 crates.io 安装时通常会失败）。生成代码默认仍不 `use trust_rt`。
- **`compile --link-trust-rt`**：不加 **`--exec`** 时不生成 `Cargo.toml`，无效果；与 **`--exec`** 联用时与 **`run --link-trust-rt`** 相同。

#### 常见工作流

**1）初始化并直接运行**

```bash
trust init --dir my-trust-app
trust run my-trust-app/main.ts
```

**2）快速添加 Rust 绑定**

```bash
trust add url::Url::parse --dir my-trust-app
# 然后在 TS 中：
# import { Url } from "url";
# const u: Url = new Url("https://example.com");
```

**3）生成可执行文件**

```bash
trust compile my-trust-app/main.ts --exec -o ./my-trust-app-bin
```

`compile` 可加 `--span-comments`，在生成的 Rust 中为每条语句前置 `// ts: path:line:col`（对照 TS **位置**）。另可加 `--ts-source-comments`，写入 TS **注释正文**（leading，转为 `//` 行；与 `--span-comments` 独立）。

## 仓库布局

| Crate | 说明 |
|-------|------|
| `trust-parser` | swc 封装；`ParsedSource`（`program`、`source_map`、`comments`）；[`module_graph`](crates/trust-parser/src/module_graph.rs) 多文件入口；共享 import 解析见 [`import_utils`](crates/trust-parser/src/import_utils.rs) |
| `trust-hir` | IR、构建、语义、`emit_rust`；[`compile_graph`](crates/trust-hir/src/lib.rs) 多模块；[`ir_cache`](crates/trust-hir/src/ir_cache/mod.rs)（增量磁盘快照）；诊断与 codegen 共用节点 `Span` + 函数级 `ir_id`（见 [`ir.rs`](crates/trust-hir/src/ir.rs)）；拆分辅助模块：[`build/build_types.rs`](crates/trust-hir/src/build/build_types.rs)、[`sem/helpers.rs`](crates/trust-hir/src/sem/helpers.rs)、[`codegen/helpers.rs`](crates/trust-hir/src/codegen/helpers.rs) |
| `trust-lower` | `lower_program` / [`lower_module_graph`](crates/trust-lower/src/lib.rs) |
| `trust-driver` | 临时 crate + `cargo build`（需本机 `cargo` 在 `PATH`）；[`compile_entrypoint_to_executable`](crates/trust-driver/src/lib.rs)；未找到 `cargo` 时 [`DriverError::CargoNotFound`](crates/trust-driver/src/lib.rs)；流水线拆分：[`pipeline.rs`](crates/trust-driver/src/pipeline.rs)、[`cargo_runner.rs`](crates/trust-driver/src/cargo_runner.rs)、[`crate_writer.rs`](crates/trust-driver/src/crate_writer.rs) |
| `trust_rt` | 可选运行时（预留） |
| `trust-cli` | 命令行 `trust`；[`cli_args.rs`](crates/trust-cli/src/cli_args.rs)、[`commands.rs`](crates/trust-cli/src/commands.rs)、[`graph_loader.rs`](crates/trust-cli/src/graph_loader.rs)、[`tsconfig_resolve.rs`](crates/trust-cli/src/tsconfig_resolve.rs)、[`incremental.rs`](crates/trust-cli/src/incremental.rs) |

## 许可

MIT OR Apache-2.0
