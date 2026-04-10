[English](PROJECT-TODO.md)

# trust 项目长期 TODO 清单

本文档用于**长期跟进**编译器与工具链的演进，按主题分层列出可验收项。状态建议用 `[ ]` / `[~]` 进行中 / `[x]` 在 PR 或提交中维护。**本清单所列特性均以强类型（strong typing，trust）为前提。**

**相关代码入口**：[`README.zh-CN.md`](README.zh-CN.md) · [`crates/trust-hir`](crates/trust-hir)（`build.rs` / `sem.rs` / `codegen.rs` / `ir.rs`）· [`crates/trust-parser`](crates/trust-parser) · [`crates/trust-driver`](crates/trust-driver) · [`crates/trust-cli`](crates/trust-cli) · [`test-ts/main.ts`](test-ts/main.ts)（多文件：`test-ts/math.ts`） · [`crates/trust-cli/tests/fixtures/`](crates/trust-cli/tests/fixtures/)

**后续/backlog 总表**：见 **[§14 后续工作（backlog）](#14-后续工作backlog)**（与英文 [`PROJECT-TODO.md`](PROJECT-TODO.md) §14 对应）。

### 规划约束：强类型（strong typing，trust）

**trust 采用强类型：** 与隐式 any、运行期改型等宽松语义相对，长期条目与 PR 取舍须与此一致：只扩展能在 HIR / [`sem.rs`](crates/trust-hir/src/sem.rs) 中给出**静态**规则的语法；**不**把「隐式 any、运行期改型、无注解宽进」等能力列入本仓库目标。详细表述见 [README「类型立场：强类型」](README.zh-CN.md)。  
本文中「收窄」「可赋值」「结构/形状」均指 **HIR / sem 内的静态规则**，**不**表示运行期改型，也**不**表示向 `tsc` 默认宽松或渐进式软类型靠拢。

---

## 0. 愿景与「1.0」验收标准（可删减）

- [x] **单文件子集**：对 README 矩阵中声明支持的特性，均有对应 fixture 与集成测试（[`crates/trust-cli/tests/fixtures/`](crates/trust-cli/tests/fixtures/) + [`cli_e2e.rs`](crates/trust-cli/tests/cli_e2e.rs)）；`trust-lower` 另有 compile 单元测试。
- [x] **诊断**：常见错误带行列号（`path:line:col`）；文案为**英文**（见 README「1.0 范围」）。
- [x] **可复现**：`cargo test --workspace`、`cargo clippy --workspace --all-targets`；[`.github/workflows/ci.yml`](.github/workflows/ci.yml) 在 push/PR 上执行。
- [x] **多文件（若纳入范围）**：**不纳入 1.0** 完整工程图；**相对路径** `import { x } from "./dep.ts"` 由 [`parse_module_graph`](crates/trust-parser/src/module_graph.rs) 构建模块图（不合并 AST），CLI 与 [`compile_entrypoint_to_executable`](crates/trust-driver/src/lib.rs) 走 `validate_imports` → `lower_module_graph`；见 §6.2。

---

## 1. 前端：解析与 AST 覆盖

### 1.1 已支持路径的健壮性

- [x] **错误恢复**：解析与 build/sem 可在一次失败中输出**多条**诊断（见 README.zh-CN.md §1.1）；模块图仍可能在**首个无法解析的文件**处返回（单文件内可多条）。
- [x] **保留注释**：[`parse_typescript_file`](crates/trust-parser/src/lib.rs) 写入 [`ParsedSource::comments`](crates/trust-parser/src/lib.rs)（swc）；可冻结进 HIR 并可选生成 Rust `//` 行（[§14 — 注释与生成 Rust](PROJECT-TODO.zh-CN.md)）；详见 README.zh-CN.md §1.1。
- [x] **`export` 变体**：`export function`、顶层 `function`、相对 **`export * from "./…"`** / **`export { … } from "./…"`**（值重导出；HIR 跳过）；**`export default function main`**、**`export default async function main`**、以及顶层已有 **`function main`** 时的 **`export default main`**（见 §13.6）；其余 `export default` 形态仍拒绝（[`build.rs`](crates/trust-hir/src/build.rs)、[`module_graph.rs`](crates/trust-parser/src/module_graph.rs)）；负例 `export_*_fail.ts` + `cli_e2e`。

### 1.2 语句与声明扩展

- [x] **`import`**：相对路径 `import { f } from "./x.ts"` 由模块图解析（[`module_graph.rs`](crates/trust-parser/src/module_graph.rs)），旧实现 [`resolve_imports.rs`](crates/trust-parser/src/resolve_imports.rs) 已废弃；非相对路径仍报错（见 `import_fail.ts`）。
- [x] **嵌套 `function`**：[`IRStmt::FnDecl`](crates/trust-hir/src/ir.rs) + 无捕获子集；见 `nested_fn.ts`。
- [x] **`const`**：与 `let` 对齐，语义禁止对 `const` 赋值；见 `const_ok.ts`、`const_reassign_fail.ts`。
- [x] **表达式语句中的赋值**：`IRStmt::Assign` + 可变 `let`；见 `assign_simple.ts`。
- [x] **`for` / `do-while`**：C 风格 `for`（含 update 赋值）、`do-while`；**`switch`**：在 `build` 降为 `If` 链（见 §13.5、`switch_ok.ts`）。
- [x] **`break` / `continue`**：循环内；label 未支持。
- [x] **空语句 / 块**：`Stmt::Empty`、`Block`；见 `empty_stmt.ts`。

### 1.3 表达式扩展

- [x] **`async` / `await` / `fetch` / `fetchText`（MVP）**：TS 表面**无** `Promise<T>` / `Promise.all` — `async function` 注解写**兑现类型** `T`（`number` / `string` / `void`），并行等待用 **`async_all([...])`**（对应 HIR [`PromiseAll`](crates/trust-hir/src/ir.rs)）；**`fetchText`**、**`fetch`**、**`readFileTextAsync`** 等仅可 **`await`**；**`response.body.getReader()`** + **`await reader.read()`**；`fetch` 的 `init` 支持字面量 `method`、`headers`、可选 `body`；**`.then` / 类型名 `Promise`** **拒绝**（[§13.8](PROJECT-TODO.zh-CN.md)）；含流式时 driver 注入 **`futures-util`**；**完整 WHATWG `fetch` / 与某 Node 字节级 TLS·HTTP2 对齐**仍属后续。
- [x] **成员访问与调用链**：`string.length`（UTF-16）、`string[i]`、`number[]`/`string[]` 下标、对象 `length`；**`obj.m(args)`** → 全局 `m(receiver,…)`；**一层** `f().prop` / `f().m()`（`chain_call_ok.ts`）；可选 **`?.` / `f?.()` / `recv?.m()`**（`optional_call_ok.ts`）；见 `member_length_ok.ts`、`method_call_ok.ts`、`string_utf16_length.ts`、`stdlib_hir_ok.ts` 等。
- [x] **可选链 / 空值合并**：受限子集已支持（`obj?.prop`、`??`；见 `optional_ok.ts`、`nullish_ok.ts`）；完整语义依赖 §3.3。
- [x] **逻辑与短路**：`&&`、`||`；`boolean` 与 `number` 真值（`!= 0`）已支持，结果类型为 `boolean`（见 `logical_bool.ts`、`logical_truthy_ok.ts`）；与 TypeScript 值保留式 `&&`/`||` 仍不同；**强类型下**结果类型固定为 `boolean`，更复杂真值或联合操作数仍受限。
- [x] **三元运算符**：`cond ? a : b`（见 `ternary_ok.ts`）。
- [x] **逗号表达式**：见 `comma_ok.ts`。
- [x] **模板字符串**：无 tag；见 `template_ok.ts`。
- [x] **数组 / 对象字面量**：受限子集已支持（`number[]`、`{ k: number }`；见 `array_ok.ts`、`object_ok.ts`）；运行时与完整类型见 §1.4 / §2.1。

**§1.3 仍待后续（原因备忘）**

- **方法 / 链式类型**：`obj.m` 与一层 `f().g` 已实现；**更一般的实例方法类型**（任意 class 实例）仍受 class 子集限制 — 见 README 矩阵。
- **`??` / `?.`**：同族 `Union` 与可选调用/成员已支持；**discriminated 收窄**已实现（D1；§3.3.1）。
- **数组/对象字面量的「完整」类型**：更丰富的元素与字段类型、`TsType`/IR 演进见 §1.4、§2.1，不单属表达式扩展层。

### 1.4 类型语法（仅类型层）

**摘要**

- [x] **字面量类型**、**联合类型**、**接口**、**type 别名**：与 **强类型** checker 路线图对齐（拆分为下列子项；**字面量类型**、**primitive/字面量联合**、**受限 `interface`→`ObjectNum`** 与 **受限 `type` 别名→具名表** 已见子项）。**泛型**见下列独立子项（文档化「仍拒绝」里程碑，非实现语义）。

**与已实现子集的关系**：§1.3 已支持受限注解 `number[]`、`{ k: number }`（HIR 中 [`TsType::ArrayNumber`](crates/trust-hir/src/ir.rs) / [`ObjectNum`](crates/trust-hir/src/ir.rs)）。**字面量类型**（`NumberLit` / `StringLit` / `BoolLit`）与 **联合类型**（[`TsType::Union`](crates/trust-hir/src/ir.rs) + 规范化）已见下项；**顶层 `interface`** 在类型层等价于具名 `ObjectNum`（与对象类型字面量同一规则）；**顶层 `type` 别名**经 [`collect_named_types_with_errors`](crates/trust-hir/src/build/build_types.rs) 解析为既有 `TsType` 并进入同一张具名表；**泛型语义**仍未实现，拒绝对照见下列子项与 [README §1.4](README.zh-CN.md)；完整对象/接口形状与 IR 演进见 §2.1；**静态**空值与分支收窄与 §3.3 交叉。

**子项（逐项勾选）**

- [x] **字面量类型**（如 `42`、`"a"`、`true` 出现在类型位置）  
  - **依赖**：扩展 [`TsType`](crates/trust-hir/src/ir.rs) 或等价表示；字面量与基类型的**静态**可赋值关系（与 §3.3 **显式形状 / sem 规则**一致，非 TS 结构子类型全集）。  
  - **验收**：[`build.rs`](crates/trust-hir/src/build.rs) 解析 `TsLitType`；[`sem.rs`](crates/trust-hir/src/sem.rs) `type_assignable` / 推断字面量；`literal_type_ok.ts`、`literal_type_fail.ts` + [`cli_e2e.rs`](crates/trust-cli/tests/cli_e2e.rs)。

- [x] **联合类型**（`A | B`，建议先 primitive / 字面量联合再扩展）  
  - **依赖**：类型规范化与可判定相等的并集表示；与 `??` / `?.` 的**静态收窄 / 分支类型**（强类型下、须可静态判定）对齐 §3.3。  
  - **验收**：受限联合下的赋值与分支可给出一致诊断或生成；集成测试覆盖典型路径（`union_literal_ok`、`union_cond_ok`、负例 `union_heterogeneous_fail`、`intersection_type_fail`、`union_mixed_cond_fail`）。

- [x] **`interface` 与对象类型**（声明体、可选属性、`extends` 等按阶段）  
  - **依赖**：**显式字段形状**进入 IR（§2.1），经 sem **静态**检查；**非** TS 结构子类型全集；与现有 `ObjectNum` 子集的关系在实现 PR 中写清（兼容或迁移路径）。  
  - **验收**：至少一种 `interface` 形态可编译到等价 Rust 或明确诊断边界（`interface_ok`、`export_interface_ok`；`extends`/泛型负例 `interface_extends_fail`、`interface_generic_fail`；README 说明单文件与顺序）。

- [x] **`type` 别名**（`type Id = …`）  
  - **依赖**：顶层收集 `TsTypeAlias`（或等价）并入符号表；解析在 swc 侧已有，需进入 HIR/语义。  
  - **验收**：别名可在参数/变量注解中解析；fixture + e2e（`type_alias_ok`、`type_alias_to_interface_ok`、`export_type_alias_ok`；负例 `type_alias_generic_fail`、`type_alias_dup_fail`）。

- [x] **泛型**（函数 `function f<T>(…)` 与类型上参数）  
  - **依赖**：单态化（per-call 特化）或受限策略仍为**后续工作**；当前拒绝入口与英文诊断见 [README §1.4「泛型与类型参数」](README.zh-CN.md)、[`build.rs` 中 generic 相关检查](crates/trust-hir/src/build.rs)。  
  - **验收**：分阶段文档化「仍拒绝」— [README](README.zh-CN.md) 对照表 + 负例 `generic_function_fail`、既有 `interface_generic_fail`、`type_alias_generic_fail` 与 e2e；**不**在本里程碑实现泛型语义。

---

## 2. IR（`ir.rs`）演进

### 2.1 当前结构补强

- [x] **语句**：已含 `Assign`、`Break`、`Continue`、`DoWhile`、`FnDecl`、`Empty`；`for` 展开为 `while`；`Switch` 未实现。
- [x] **表达式**：已含 `LogicalAnd` / `LogicalOr`、`Conditional`、`Seq`、`Tpl`、`Member` / `OptionalMember`、`Index`（数组与 UTF-16 字符串下标）、`MethodCall` / `OptionalMethodCall`、一层链式 `f().prop` / `f().m()`、`JsonBuiltin` / `UriBuiltin` 及 README 矩阵所列内建；**计算属性调用** `obj[expr](…)` 仍不支持。`ObjectNum` / `interface` 见 §1.4（强类型、静态检查）。
- [x] **顶层**：多文件模块图 — `parse_module_graph` + `validate_imports`；HIR 合并为 [`IRModule`](crates/trust-hir/src/ir.rs)（`build_program_multi` / `compile_graph`）；`main` 须在入口文件；全局函数名唯一；负例见 `import_missing_export_*`、`circular_*`、`dup_*` fixtures。

### 2.2 元数据与调试

- [x] **Span**：HIR 节点带 swc `Span`；[`diag`](crates/trust-hir/src/error.rs) 与 codegen 错误均用所属函数的 `cm` + `source_path` + 节点 `span`（见 [`ir.rs`](crates/trust-hir/src/ir.rs) 模块注释）；`build` 无顶层函数时用整文件 `span`；`sem` 缺 `main` 时锚点为第一函数 `span`。
- [x] **可选**：[`IRFunction::ir_id`](crates/trust-hir/src/ir.rs)（函数级，含嵌套 `function`；单次编译内单调递增，见 `build_fn`）。

---

## 3. 语义分析（`sem.rs`）

### 3.1 已实现的巩固

- [x] **符号表**：块作用域与 `let` 重复绑定（已部分实现）— 增加用例覆盖边界（嵌套块、与参数同名等）。验收：`let_dup_same_block_fail.ts`、`let_shadow_nested_ok.ts`、`param_let_same_name_fail.ts` + [`cli_e2e.rs`](crates/trust-cli/tests/cli_e2e.rs)。
- [x] **控制流**：`stmts_return` 简化规则— 文档化并与 TS/tsc 差异列表对照（**trust 仅保证静态规则**；见 README）。验收：[README.zh-CN.md「控制流与 return（简化语义）」](README.zh-CN.md)。
- [x] **`void` 与 `console.log`**：`BuiltinLog` 为 void 表达式路径已覆盖；补充「仅 log 的表达式语句」在分支中的用例。验收：`void_log_in_branch.ts` + e2e。

### 3.2 可变性与赋值

- [x] **`let` 可变**：`IRStmt::Assign`；语义检查 LHS 为已绑定标识符。
- [x] **禁止对 `const` 赋值**。

### 3.3 类型系统加深

- [x] **与 §1.4 的衔接**：字面量类型、联合类型与 `??` / `?.` **静态**收窄应与 §1.4 子项一致（避免与受限 `TsType` 冲突）。联合类型已入 HIR。**已实现（sem）**：在 `Union` 上去除 `null`/`undefined` 后，若与 `??` 右侧为**同族**（`number`/`string`/`boolean` 或**结构一致**的 `Fn`），则 [`infer_expr_mut`](crates/trust-hir/src/sem.rs) 中 `IRExpr::NullishCoalesce` 路径会调用 `unify_ternary_branches` 得到单一结果类型。**已完成（D1）**：依赖 **discriminant** 的 discriminated union 收窄，通过 `Binding::narrow_ty` 实现（§3.3.1）。验收：README §3.3；`nullish_ok.ts` / `optional_ok.ts`；`??` 与函数类型联合的 sem 见 `nullish_fn_ok.ts`（`trust check`）。
- [x] **`null` / `undefined`**：[`TsType`](crates/trust-hir/src/ir.rs) 已含 `Null` / `Undefined` 等变体；检查以 **当前 sem 静态规则**为准，**不设** tsc 默认「万物可空」式软语义。验收：README §3.3；**未**实现 `strictNullChecks` 式开关。若将来增加模式，应为**显式编译选项**（如 strict 空值），**非**隐式放宽或兼容 JS 动态性。
- [x] **结构类型 vs 名义类型**：**trust 以名义表 + 静态形状检查为界**；与 Rust 后端映射策略（当前以基础类型为主）。验收：README §3.3 已说明具名表/语义检查与 Rust 生成侧边界；**未**实现 TS 结构子类型全集，**不**将其列为路线目标。
- [x] **函数类型与高阶函数**：与 [§13.2](PROJECT-TODO.zh-CN.md) 及 README 一致——已实现**受限**静态函数类型、箭头值、`f(...)`、传参/返回函数等；codegen 闭包仍为 `(number) => number` 的严格子集。**勿**再写「无一等函数值 / 未做 HOF」；应区分「已支持的 HOF 子集」与「仍为后续的泛化 / codegen 扩展」。

### 3.3.1 扩展路线 — 主线选择与规格包（实现待立项）

**单次 PR 原则**：每次只落地**一条**主线；每条需 fixture + 更新 README §3.3 / 矩阵。

**推荐顺序**

1. **D1** — discriminated 收窄（主要改 `sem`，对 `if` + 联合收益大）。
2. **R1** — `interface` 名义方法（build + sem，可能动 codegen 调用形态）。
3. **G1 / G2** — 泛型单态子集扩大（`sem/mono.rs` + build 拒绝表）。
4. **C1 / C2** — TS 注释落 Rust（以 `codegen` + 注释表为主，与 sem 耦合弱）。

#### D1 — discriminated union 收窄（规格）

- [x] **目标**：在 `if (v.tag === 'a')` / `else` 内，当 `v` 为若干 `ObjectNum` 形联合臂且共享**必填** discriminant 字段、各臂字面量**互斥**时，静态收窄 `v` 的类型。
- [x] **条件形态（v0）**：`Eq` / `StrictEq`，左侧为标识符 `v` 的 `Member` / `OptionalMember` + 字面量属性名，右侧为与某一臂 discriminant **匹配的字面量**。（任意左式 / 计算属性：后续。）
- [x] **字面量 discriminant**：仅 `string` / `boolean` / `number` **字面量类型**（与 §1.4 一致）。
- [x] **与现有 `??` / `?.` 协同**：不破坏 `nullish_ok.ts`、`optional_ok.ts`、`nullish_fn_ok.ts`。
- [x] **实现**：[`Binding`](crates/trust-hir/src/sem.rs) 上增加 `narrow_ty: Option<TsType>`；[`check_stmt`](crates/trust-hir/src/sem.rs) 对 `IRStmt::If` 通过 `try_extract_discriminant_narrowing` 提取条件并经由 `update_binding_narrow_ty` 应用收窄；嵌套收窄使用 effective type（`narrow_ty.unwrap_or(ty)`）。
- [x] **Fixture**：`discriminated_narrow_ok.ts`、`discriminated_narrow_else_ok.ts`、`discriminated_narrow_nested_ok.ts`；e2e `run_discriminated_narrow_ok_prints_42`、`run_discriminated_narrow_else_ok_prints_100`、`run_discriminated_narrow_nested_ok_prints_100`。

#### G1 / G2 — 泛型子集扩展（规格）

- **G1**：显式类型实参的**更多安全调用形态**（链式/嵌套），逐项在 `mono.rs` 证明可单态 + fixture。
- **G2**：推断仅当每个类型参数可从**已支持的合成实参类型**唯一确定；拒绝联合歧义、未知标识符等（保持现有多错误行为）。
- **仍非目标**：高阶类型参数、`extends` 约束、默认类型参数、从异质联合推断等。
- **回归**：保留现有 `generic_function_*_fail.ts` 与 `compile_generic_function_multi_infer_fail_reports_multiple_errors`。

#### R1 — `interface` 实例方法（名义；规格）

- [x] **语法子集（v0）**：顶层 `interface I { m(): number; … }`，**无**重载、**无**方法级泛型。
- [x] **Lowering**：全局脱糖 `m__I(receiver, …)` 通过 `IRExpr::MethodCall` 的 `inherent_rust` 字段；接收者为包含方法签名的 `ObjectNum`，使用 `ObjectMemberKind::Method`。
- [x] **兼容**：`obj.m(args)` 优先检查 interface 方法签名，找不到则回退到全局函数；不破坏现有方法调用。
- [x] **Fixture**：`interface_method_ok.ts`、`interface_method_bad_args_fail.ts`。

#### C1 / C2 — TS 注释写入生成 Rust（规格）

- **C1**：边界说明见 README.zh-CN §1.1 — 仅 **leading**，按 **HIR 语句 span 起点**查表；`switch`→`if`、`for`→`while` 等 lowering 会导致注释位置与用户预期不一致。
- **C2（后续）**：trailing / 行内注释需按 `span.hi` 与 swc 注释区间策略处理。
- **可选警告**：`--ts-source-comments` 下对「有冻结注释但未被任何生成语句消费」做一次性提示（需额外遍历，**尚未实现**）。

### 3.4 控制流分析（进阶）

- [x] **可达性**：不可达代码警告（英文 `warning: path:line:col: unreachable code`）；见 `early_return_unreachable.ts`、`unreachable_after_return.ts`、`break_unreachable.ts` + [`cli_e2e.rs`](crates/trust-cli/tests/cli_e2e.rs)。
- [x] **明确赋值**：`let x: number;` 无初始化已允许；使用前须明确赋值（`if`/`else` 合并、循环保守策略）；正例 `definite_assign_ok.ts`、`definite_assign_if_ok.ts`，负例 `definite_assign_fail.ts`。
- [x] **更精确的 return**：序列内提前穷尽返回（如 `if`/`else` 均 `return` 后允许后续死代码仅警告）；`while`/`do-while` 仍沿用原尾部规则（`tail_returns_while_body`）；未来 `switch` 可在 `stmt_fn_returns_complete` 侧扩展。

---

## 4. 代码生成（`codegen.rs`）

### 4.1 当前行为改进

- [x] **`console.log` 多参数格式**：[`emit_builtin_log`](crates/trust-hir/src/codegen.rs) 多参数已改为 `"{}"` 空格分隔；验收：`trust-lower` 单测 `console_log_multi_arg_uses_spaced_format`。
- [x] **算术与 `/`**：TS `number` → 生成 Rust **`f64`**；**`/`** 为 IEEE-754 双精度除法（与旧版 `i32` 向零截断不同）；验收：README「算术、`/` 与溢出」与矩阵算术 / `Math.*` 行。
- [x] **NaN / ∞ 与溢出**：可能出现；与 V8 `number` 边界情况未必逐位一致；**未**加运行时溢出检查 Cargo feature（留待后续）。

### 4.2 新特性映射

- [x] **赋值**：`let mut` 与块作用域对齐 Rust。（验收：`trust-lower` `codegen_42_let_mut_block_and_assign`；[`emit_stmt`](crates/trust-hir/src/codegen.rs) `Let` / `Assign` / `Block`；`cargo test --workspace`。）
- [x] **字符串**：继续 `String` + `format!`；大字符串与性能另议。（验收：`trust-lower` `codegen_42_string_concat_uses_format`；[`emit_expr`](crates/trust-hir/src/codegen.rs) `StrConcat` / `Tpl`；`cargo test --workspace`。）
- [x] **堆对象 / GC**：当前对象为值型 `HashMap::from`，**未**引入 `Rc`/`Arc`；若将来引用类型再定策略。（验收：`trust-lower` `codegen_42_object_literal_hashmap_without_rc`；[`emit_expr`](crates/trust-hir/src/codegen.rs) `ObjectLit`；`cargo test --workspace`。）

### 4.3 生成代码可读性

- [x] **缩进与换行**：逗号表达式（`Seq`）块内行与闭合 `})` 与外层语句层级对齐，便于 rustfmt。（验收：`trust-lower` `codegen_43_comma_seq_indented`；[`emit_seq_expr`](crates/trust-hir/src/codegen.rs) / `emit_expr` 的 `stmt_level`；`cargo test --workspace`。）
- [x] **注释**：可选在每条语句前注入 `// ts: path:line:col`（与诊断同源 `lookup_char_pos`）。（验收：`trust-lower` `codegen_43_span_comments_emits_ts_anchors`；`trust-cli` `compile_span_comments_writes_ts_anchors`；[`emit_stmt`](crates/trust-hir/src/codegen.rs) 与 `CodegenOptions`；`trust compile --span-comments`；`cargo test --workspace`。）另可选将 TS **leading** 注释正文写成 Rust `//` 行（`emit_ts_source_comments_writes_frozen_leading`；`compile_ts_source_comments_writes_ts_text`；`CodegenOptions::emit_ts_source_comments`；`trust compile --ts-source-comments`。）

---

## 5. 内建与标准库映射

### 5.1 `console`

- [x] **`console.log` / `console.error` / `console.debug`**：`log` → `println!`，`error` / `debug` → `eprintln!`。（验收：`trust-lower` `console_error_and_debug_use_eprintln`；`trust-cli` `compile_console_stderr_writes_eprintln`；[`build.rs`](crates/trust-hir/src/build.rs) `console` 成员；[`emit_builtin_log`](crates/trust-hir/src/codegen.rs)；`cargo test --workspace`。）
- [x] **格式化语义**：与 §4.1 相同，多参 `"{}"` 空格分隔；`log` 与 `error`/`debug` 共用 [`emit_builtin_log`](crates/trust-hir/src/codegen.rs) 格式构造。（验收：同上 + 已有 `console_log_multi_arg_uses_spaced_format`；`cargo test --workspace`。）

### 5.2 最小运行时（[`trust_rt`](crates/trust_rt)）

- [x] **字符串操作**：`string.length` 为 **UTF-16 码元数**（`encode_utf16().count()`）；`number[].length` → `Vec::len`；对象数字字段名为 `length` 时走 `HashMap::get`（见 [`MemberLengthDispatch`](crates/trust-hir/src/ir.rs)）。（验收：`trust-lower` `codegen_52_string_length_utf16`、`codegen_52_object_length_field_uses_get`；`trust-cli` 等；`cargo test --workspace`。）**`string` 下标 `s[i]`**：UTF-16 索引 → 单码元 `string`（[`IndexKind::StringUtf16`](crates/trust-hir/src/ir.rs)；`stdlib_hir_ok.ts`）。
- [x] **数学**：`Math.abs` / `min` / `max` / `floor` / `ceil` / `sign` / `trunc` / `round` / `pow` 等在 codegen 上对 **`f64`** 运算（[`MathBuiltinKind`](crates/trust-hir/src/ir.rs)；[`build.rs`](crates/trust-hir/src/build.rs)；[`emit_expr`](crates/trust-hir/src/codegen.rs)；与 README 矩阵「`Math.*` builtins」一致）。（验收：`trust-lower` `codegen_52_math_builtins`；`trust-cli` `run_math_builtin_prints_sum`、`run_stdlib_hir_ok_prints_expected`；`cargo test --workspace`。）
- [x] **HIR 标准库（可不链 `trust_rt`）**：`Number.parseInt` / `parseFloat`；`String` 方法 `charAt`、`charCodeAt`、`slice`、`substring`、`indexOf`、`includes`；全局 `readLine()` 内联 `std::io`（`async` 函数体中拒绝）。默认实现已切到 **`trust_stdlib`** 门面（`json` / `uri` / `string`），并保留 `--stdlib-mode legacy` 回退旧内联 helper / 直接 `serde_json` + `urlencoding`。（验收：`stdlib_hir_ok.ts`、`json_uri_trust_ok.ts`；`compile_stdlib_hir_ok_uses_trust_stdlib_calls`。）
- [x] **I/O**：[`trust_rt::read_stdin_line`](crates/trust_rt/src/lib.rs) 仍为可选占位；**同步** `readLine()` 已在生成 Rust 中实现且**无需**链接 `trust_rt`；driver 临时 crate 默认仍不依赖 `trust_rt`（除非 `--link-trust-rt`）。

---

## 6. Driver 与构建（[`trust-driver`](crates/trust-driver)）

### 6.1 单文件路径

- [x] **临时目录生命周期**：crate 与 [`compile_entrypoint_to_executable`](crates/trust-driver/src/lib.rs) / [`build_rust_to_executable`](crates/trust-driver/src/lib.rs) / [`build_rust_and_copy`](crates/trust-driver/src/lib.rs) 文档说明 [`TempDir`](https://docs.rs/tempfile) drop 删除目录、返回元组为 `(TempDir, PathBuf)`。（验收：文档见 [`lib.rs`](crates/trust-driver/src/lib.rs) 顶部与上述函数；`cargo test --workspace`。）
- [x] **离线 / 无 cargo 环境**：`cargo` 无法启动（`NotFound`）时 [`DriverError::CargoNotFound`](crates/trust-driver/src/lib.rs)；编译失败（含网络/依赖）仍为 [`DriverError::CargoBuild`](crates/trust-driver/src/lib.rs) 并附 stdout/stderr。（验收：单测 `map_cargo_spawn_error_maps_not_found_to_cargo_not_found`；`cargo test --workspace`。）

### 6.2 多文件与模块（[`compile_entrypoint_to_executable`](crates/trust-driver/src/lib.rs)）

- [x] **解析多入口**：[`parse_module_graph_with_extra_roots`](crates/trust-parser/src/module_graph.rs)；CLI 多 `.ts` 位置参数或 `--project` + 简化 JSON（**`extends`**、**`files`**、**`include` / `exclude`**，见 [`tsconfig_resolve`](crates/trust-cli/src/tsconfig_resolve.rs)）（[`trust-cli`](crates/trust-cli/src/main.rs)）。（验收：`module_graph::tests::extra_root_includes_unreachable_file`；`cli_e2e` `run_multi_entry_extra_roots_prints_main`、`run_project_tsconfig_prints_main`、`run_project_tsconfig_extends_include_ok`。）
- [x] **依赖图（子集）**：入口文件 + 相对 `import` → [`parse_module_graph`](crates/trust-parser/src/module_graph.rs)（保留各模块 AST）→ `validate_imports` → [`lower_module_graph`](crates/trust-lower/src/lib.rs) → 单 Rust crate。
- [x] **`Cargo.toml` 生成**：[`RustBuildOptions`](crates/trust-driver/src/lib.rs) / [`build_rust_to_executable_with_options`](crates/trust-driver/src/lib.rs)；可选 path 依赖 `trust_rt` + feature `trust_rt`；CLI `--link-trust-rt`。（验收：`write_minimal_crate_with_link_trust_rt_contains_optional_path_dep`；`cli_e2e` `run_with_link_trust_rt_prints_main`；`cargo test --workspace`。）
- [x] **循环依赖**：[`parse_module_graph`](crates/trust-parser/src/module_graph.rs) 检测并报错（见 `circular_*.ts`）。

---

## 7. CLI（[`trust-cli`](crates/trust-cli)）

- [x] **子命令**：`compile` / `run` / `check`；README「CLI」表与 `trust --help`；`check` 仅 HIR+语义（[`check_module_graph`](crates/trust-lower/src/lib.rs)）。（验收：`cli_e2e` `check_sample_ok`、`check_switch_fail_stderr`。）
- [x] **选项**：`compile -o`；`run` 的 `-O`/`--release` 与 `--debug`（[`RustBuildOptions::release`](crates/trust-driver/src/lib.rs)）；全局 `-q`/`--quiet`、`--color`、`--emit-ir`。（验收：`compile_emit_ir_stderr_contains_ir_module`、`driver` `debug_build_writes_binary_under_target_debug`。）
- [x] **退出码**：README 约定；`run` 传播子进程 `ExitStatus::code`（无则 `1`）；`trust` 错误统一 `1`。（验收：[`main.rs`](crates/trust-cli/src/main.rs) `exit_code_for_failed_child` 单元测试。）

---

## 8. 测试与质量

### 8.1 集成测试（[`cli_e2e.rs`](crates/trust-cli/tests/cli_e2e.rs) / `fixtures/`）

- [x] **每个矩阵行**一条最小 fixture（或合并大文件但注释分段）。（验收：README「[矩阵与集成测试对照](README.zh-CN.md#矩阵与集成测试对照)」按主题对照矩阵行与 `fixtures/` + `cli_e2e`；补测 `array_fail`、`optional_chain_fail`、`nullish_fail`、`object_fail`。）
- [x] **回归**：已知 bug 固定为 [`tests/regression/*.ts`](crates/trust-cli/tests/regression/)（验收：[`tests/regression/README.md`](crates/trust-cli/tests/regression/README.md)、[`switch_fallthrough_regression.ts`](crates/trust-cli/tests/regression/switch_fallthrough_regression.ts)、`regression_switch_fallthrough_check_fails`。）

### 8.2 单元测试

- [x] **`trust-hir`**：`build`/`sem`/`codegen` 分模块 `#[cfg(test)]`。（验收：`build_module_records_main`、`check_module_accepts_simple_main` / `check_module_rejects_missing_return`、`emit_rust_contains_ts_main_and_println`；`dev-dependencies`：`trust-parser`。）
- [x] **parser**：swc 封装层快照或最小片段。（验收：[`lib.rs`](crates/trust-parser/src/lib.rs) `parse_rejects_unclosed_function_body`、`parses_module_with_import_and_export_main`。）

### 8.3 工具链

- [x] **CI 工作流**（GitHub Actions / 其他）：`cargo test`、`clippy`、格式化检查。（验收：[`.github/workflows/ci.yml`](.github/workflows/ci.yml)：`rustfmt`+`clippy` 组件、`cargo fmt --all --check` → `cargo test --workspace` → `cargo clippy --workspace --all-targets`。）
- [x] **模糊测试**（可选）：随机 AST 片段不 panic。（验收：[`parse_fuzz_inputs_do_not_panic`](crates/trust-parser/src/lib.rs) 对 `parse_typescript_file` 施加确定性变异输入。）

---

## 9. 文档与开发者体验

- [x] **README**：与实现同步更新矩阵；「不支持的 TS 特性」简表（**兼作 trust 强类型拒斥边界说明**）。**验收**：[`README.md`](README.md)（英文默认）与 [`README.zh-CN.md`](README.zh-CN.md) 中 **Unsupported TypeScript (trust rejection boundary)** / **不支持的 TypeScript 特性（trust 强类型拒斥边界）** 小节及语言矩阵。
- [x] **架构图**：解析 → HIR → sem → codegen → driver（可 Mermaid）。**验收**：两 README 中 **Architecture** / **架构** 下的 Mermaid `flowchart LR`（`trust_parser` → HIR → sem → codegen → `trust_lower` → `trust_cli` / `trust_driver`）。
- [x] **贡献指南**：`CONTRIBUTING.md`（分支、测试命令、MSRV）。**验收**：[`CONTRIBUTING.md`](CONTRIBUTING.md) / [`CONTRIBUTING.zh-CN.md`](CONTRIBUTING.zh-CN.md)；根 [`Cargo.toml`](Cargo.toml) `[workspace.package] rust-version = "1.74"` 与各 crate `rust-version.workspace = true`。
- [x] **变更日志**：`CHANGELOG.md`（若对外发布）。**验收**：[`CHANGELOG.md`](CHANGELOG.md) / [`CHANGELOG.zh-CN.md`](CHANGELOG.zh-CN.md)，Keep a Changelog 风格含 `[Unreleased]` 与 `[0.1.0]`。
- [x] **本路线图双语**：默认英文 [`PROJECT-TODO.md`](PROJECT-TODO.md)，中文版 [`PROJECT-TODO.zh-CN.md`](PROJECT-TODO.zh-CN.md)，文首互相链接。**验收**：两文件存在且首行交叉链接。

---

## 10. 性能与规模（后期）

多文件 **语义检查并行** 已实现（`rayon`），详见下文 **§14「性能与安全」**；本节仅保留仍为 backlog 的项。

- [x] **增量编译**：多文件时只重编译变更模块（`compile` / `run` 可选 `--incremental`，HIR 片段缓存目录默认可为 `.trust-cache`；变更模块的 **importers** 一并重编）。实现：[`incremental.rs`](crates/trust-cli/src/incremental.rs)、[`ir_cache`](crates/trust-hir/src/ir_cache/mod.rs)、[`forward_deps`](crates/trust-parser/src/module_graph.rs)。验收：`compile_incremental_rebuilds_only_changed_module`、`module_fragment_round_trip_bincode`。

---

## 11. 安全与边界

与下文 **§14「性能与安全」** 对齐；**勾选与实现说明以 §14 为准**，本节便于检索。

- [x] **生成代码注入**：字符串字面量转义与 `println!` 安全。（已实现，见 §14。）
- [x] **资源限制**：driver 调用 `cargo` 超时/内存与输出上限（可选）。（已实现，见 §14。）

---

## 12. 优先级建议（可随项目调整）

| 优先级 | 主题                                    | 说明                                      |
| ------ | --------------------------------------- | ----------------------------------------- |
| P0     | 赋值 + 可变 `let`                       | 解锁真实循环与累加，与 `test-ts` 示例一致 |
| P0     | 诊断与测试覆盖                          | 稳定性基础                                |
| P1     | `console.log` 格式 / 小运行时字符串 API | 体验与示例可信度                          |
| P1     | 嵌套函数或明确不支持的长期策略          | 减少用户困惑                              |
| P2     | 多文件 + `import`                       | 与 driver 联动，工作量大                  |
| P2     | 逻辑运算与三元                          | 常见 TS 惯用法                            |
| P3     | 泛型、强类型下静态类型系统深化          | 长期（**非** tsc / 软类型全集）；**细项与验收见 §13** |

---

## 13. 大型语言特性（分里程碑筹备）

下列条目均为**大工程**，实施时按 **解析（swc/AST）→ HIR → `sem` → `codegen` → 集成/单元测试** 分 PR 推进；语义须保持 **trust 强类型**（可静态判定），与完整 `tsc` **不必**逐条等价。完成子里程碑后更新 [README.zh-CN.md](README.zh-CN.md) 语言矩阵与本节勾选。

### 13.1 泛型（函数 / 接口 / 类型别名 / 类型实参）

- [x] **设计**：单态化、 erased、或受限策略（文档化与 README 泛型表对齐或替代）。（本轮采用单态化子集）
- [x] **解析 + build**：`type_params`、泛型实例化边界、`TsTypeRef` 实参进入 HIR。（已接入泛型声明与显式类型实参解析）
- [x] **sem**：实参代入与一致性检查（强类型下可判定子集）。（调用处显式类型实参校验、类型替换与实例化改写）
- [x] **codegen**：单态化展开或等价 Rust 生成策略。（消费单态化结果；未实例化类型参数在 codegen 兜底报错）
- [x] **测试**：fixture + `cli_e2e` + 负例（过度宽泛的仍拒绝）。（新增 `generic_function_ok` / `generic_function_missing_type_args_fail` 及对应 e2e）

### 13.2 高阶函数（函数作一等值、函数类型与调用）

- [x] **设计**：捕获策略、栈闭包 vs 明确不捕获子集扩展、`fn` 类型在 HIR 中的表示。（当前实现采用带类型箭头闭包，codegen 走 `Rc<dyn Fn(i32) -> i32>` 路径。）
- [x] **HIR**：函数类型、`Callee` 扩展（含成员/变量调用路径）。（新增 `TsType::Fn` 与 `IRExpr::ArrowFn`；变量调用 `f(...)` 可按函数值校验。）
- [x] **build + sem**：箭头函数与函数值、调用与赋值类型检查。（build 支持箭头函数与函数类型注解；sem 支持函数值赋值/传参/返回与调用检查。）
- [x] **codegen**：`Fn`/`fn` 指针或生成结构体闭包（依设计）。（当前子集以 `Rc<dyn Fn(i32) -> i32>` 发射闭包调用。）
- [x] **测试**：最小高阶用例 + 与现有 `nested_fn` 无捕获语义的关系说明。（新增 `hof_apply_ok.ts`、`hof_return_closure_ok.ts` 与 e2e；`nested_fn` 仍保持可用。）

### 13.3 完整 OO（`class`、`this`、构造/继承等）

- [x] **设计**：与 Rust 映射（结构体 + impl、或显式拒绝部分 TS 语义）；`export class` 与模块交互。（已落地 class lowering + codegen 动态 trait 框架，在强类型约束子集内。）
- [x] **build**：`ClassDecl`、方法、字段进入 HIR（或分阶段：仅类字段 + 方法）。（已接入类收集/降级、`new`、`this` 重写与子类构造中的 `super(...)` 降级。）
- [x] **sem**：`this`、可见性、继承/重写（按采纳子集）。（已接入继承关系校验、`super(...)` 位置校验、基础 `override` 名称与签名校验。）
- [x] **codegen**：与方法分发、`super`（若纳入范围）。（已输出类动态 trait 代码框架，运行路径由降级后的构造函数/方法函数承接。）
- [x] **测试**：类 fixture + 负例（不支持的修饰符仍诊断）。（已新增 class 正负例与 `cli_e2e` 覆盖：basic/this/extends/super/override 诊断。）

### 13.4 `for..in`

- [x] **设计**：迭代对象键的静态类型（`string` 键与 `ObjectNum` / 扩展对象模型）。（已确定：`for..in` 循环变量统一为 `string`；右侧支持对象/class-instance 键与 `number[]` 下标字符串键。）
- [x] **HIR**：`ForIn` 或 lowering 策略。（已新增 `IRStmt::ForIn`，并在 build 从 `Stmt::ForIn` 构建该节点。）
- [x] **sem**：循环变量类型、与对象/字典表示一致。（已校验循环变量为 `string`，并限制右侧为对象/class-instance/数组。）
- [x] **codegen**：迭代 `HashMap` 键或约定运行时辅助。（已发射 `HashMap::keys()` 遍历与数组 `0..len` 下标转字符串遍历。）
- [x] **测试**：fixture + 与 `for(;;)` 对照。（已新增 `for_in_*` 正负例 fixture 与 `cli_e2e` 覆盖。）

### 13.5 完整 `switch` / `case`

- [x] **设计**：强类型子集——无穿透、`default` 须最后、`case` 仅数字/布尔字面量；完整 ECMA 穿透与 `default` 位置待后续。
- [x] **HIR**：无 `IRStmt::Switch`；`switch` 在 [`build.rs`](crates/trust-hir/src/build.rs) 降为嵌套 [`IRStmt::If`](crates/trust-hir/src/ir.rs) + [`IRExpr::Binary`](crates/trust-hir/src/ir.rs) `Eq`（与 §2.1「或等价」一致）。
- [x] **sem**：沿用 `if` 条件与 `Binary` `Eq` 推断；无单独 `switch` 分支。
- [x] **codegen**：沿用 `If`/`Eq` 发射；`switch` 专用 `match` 未做。
- [x] **测试**：正例 [`switch_ok.ts`](crates/trust-cli/tests/fixtures/switch_ok.ts)（`run_switch_ok_prints_seven`、`compile_switch_ok_writes_rust`）；负例 [`switch_fail.ts`](crates/trust-cli/tests/fixtures/switch_fail.ts)（`compile_switch_fallthrough_fails`，穿透诊断）。

### 13.6 `export default`（分阶段；非完整 `tsc`）

trust 仍要求**可调用入口名为 `main`**。默认导出仅在与此约定等价时支持。

- [x] **A1 — 默认函数**：`export default function main` / `export default async function main` → 与 `export function main` 同 IR（[`build.rs`](crates/trust-hir/src/build.rs)）；模块图记录导出 `main`（[`module_graph.rs`](crates/trust-parser/src/module_graph.rs)）；fixture `export_default_function_main_ok.ts`、`export_default_async_main_ok.ts` + `cli_e2e`。
- [x] **A2 — 默认指向 `main`**：存在顶层 `function main` 时的 `export default main`；扫描结束后校验（[`build.rs`](crates/trust-hir/src/build.rs)）；fixture `export_default_main_ref_ok.ts`。
- [x] **默认导入**：`import main from "./dep.ts"` 要求绑定名为 `main` 且目标默认导出为 `main`（[`import_utils.rs`](crates/trust-parser/src/import_utils.rs)）；负例 `import_default_wrong_binding_fail.ts`。
- [x] **A3 — 任意默认表达式**（`export default 42`、`export default () => {}`、匿名 `export default function` 等）：**明确不在范围内**（产品决策）。仅 **A1/A2** 形态受支持；见 [`README.zh-CN.md`](README.zh-CN.md)「不支持的 TypeScript」与诊断 §1.1 的 **`export` 形态**。除非入口契约变更，否则不计划支持一般「默认导出表达式」。

### 13.7 结构子类型里程碑（非完整 `tsc`）

**trust** 使用静态、可 codegen 的规则；**不等于** TypeScript 完整结构子类型。

- [x] **B1 — 嵌套 `ObjectNum` + 可选属性**：[`ObjectProp`](crates/trust-hir/src/ir.rs)、[`object_shape_assignable`](crates/trust-hir/src/sem/helpers.rs)、codegen 对象字面量为 `serde_json::Value`；fixture `nested_object_ok.ts`。
- [ ] **B2+ — 跨文件接口名在类型位置、对象上的可调用成员、`readonly`/索引签名等更丰富规则**：**待办**；README 写明当前限制及与 `tsc` 的差异。
- **B2a（下一里程碑）** — 跨文件**仅类型** / 具名复用：例如 `import type { I } from "./dep.ts"` 并在注解中使用 `I`。**当前边界**：`import type` 与 type-only 说明符在导入解析阶段即**拒绝**（[`import_utils.rs`](crates/trust-parser/src/import_utils.rs)）；负例 fixture [`import_type_fail_main.ts`](crates/trust-cli/tests/fixtures/import_type_fail_main.ts) 与 e2e `compile_import_type_fails`。实现 B2a 需扩展模块图与合并后的类型表，不仅是 parser。

### 13.8 异步表面 — 无用户侧 `Promise` / 无 `.then`（产品决策）

**用户可见的 `Promise<T>`、`Promise.all`、`.then` / `.catch` / `.finally` 回调链** 均**不属于** trust。应写 **`async function …(): T`**（`T` 为 `number` / `string` / `void`），并行 **`async_all([...])`**。类型位置出现 **`Promise`** 即**报错**（[`build_types.rs`](crates/trust-hir/src/build/build_types.rs)）。**`.then`** 调用**报错**（[`build.rs`](crates/trust-hir/src/build.rs)；[`promise_then_fail.ts`](crates/trust-cli/tests/fixtures/promise_then_fail.ts)；e2e `compile_promise_then_fails`）。HIR 内部仍用 [`TsType::Promise`](crates/trust-hir/src/ir.rs) 表示 awaitable，仅服务 codegen。与 §13.6 A3 相同：**范围决策**，非 backlog 里程碑。

---

## 14. 后续工作（backlog）

汇总「接下来要做什么」，可能与 §1.3 附注、§10–§11、README「部分支持/不支持」、§1.3 follow-ups 重复；**以代码为准**，落地后同步删改旧句。文末 **[汇总产品待办](#汇总产品待办-readme--partial--慢车道)** 提供**一条龙勾选清单**（来自 README / 矩阵 / 前文讨论），可逐项慢慢做；与 §3.3.1、§13.7 等处叙述可能重复，落地时请同步勾选并改对应小节。

### 工具链与体验

**多条诊断收集**（见 [README.zh-CN.md](README.zh-CN.md) §1.1）

- [x] **编译管线（build + sem）**：[`build_module`](crates/trust-hir/src/build.rs) 与 [`check_module`](crates/trust-hir/src/sem.rs) 可聚合多条 [`CompileError`](crates/trust-hir/src/error.rs) 为 [`CompileError::Many`](crates/trust-hir/src/error.rs)（排序后输出）。顶层声明 / 各函数语义错误会尽量收集；**单态化**一旦失败会**中止后续 sem**；[`emit_rust_with_options`](crates/trust-hir/src/codegen.rs) 在 codegen 首条内部错误处停止。**体验**：单态 / codegen 错误末尾可附英文短句，提示修复并重新编译后或出现更多诊断（[`with_monomorphization_followup`](crates/trust-hir/src/error.rs) / [`with_codegen_followup`](crates/trust-hir/src/error.rs)）；见 README.zh-CN §1.1。
- [x] **解析器**：[`parse_typescript_file`](crates/trust-parser/src/lib.rs) 汇总 swc [`take_errors()`](crates/trust-parser/src/lib.rs) **全部**诊断（并与 `parse_program` 主错误合并、排序）。**模块图**仍按文件顺序在首次解析失败时返回（单文件 stderr 可多行）。

**注释与生成 Rust**（见 [README.zh-CN.md](README.zh-CN.md) §1.1「注释」）

- [x] **位置锚点（已支持）**：`trust compile --span-comments` 设置 [`CodegenOptions::span_comments`](crates/trust-hir/src/codegen.rs)，在每条语句前生成 `// ts: path:line:col`（§4.3；映射 TS **位置**，非 TS 注释正文；用法见 README「Usage」中 `compile`）。
- [x] **TS 源码注释**进入生成 Rust（可选）：解析器将 swc leading 注释写入 [`ParsedSource::comments`](crates/trust-parser/src/lib.rs)；[`build_module`](crates/trust-hir/src/build.rs) / [`build_program_multi`](crates/trust-hir/src/build.rs) 冻结为 [`IRModule::ts_comments_by_path`](crates/trust-hir/src/ir.rs)；[`CodegenOptions::emit_ts_source_comments`](crates/trust-hir/src/codegen.rs) 在语句与顶层函数前输出 Rust `//` 行。**局限**：仅 leading（无 trailing / 表达式级）；大粒度 lowering（如 `switch`）可能导致位置偏移或丢失；[`compile_with_options`](crates/trust-hir/src/lib.rs) 仍传 `None` 注释（单文件 API 不带来 TS 正文，除非自行 `build_module` 并传入解析结果）。CLI：`trust compile --ts-source-comments`。验收：`emit_ts_source_comments_writes_frozen_leading`、`compile_ts_source_comments_writes_ts_text`。

**工程级工具链**（见 [README.zh-CN.md](README.zh-CN.md)「非 1.0」与「不支持」边界）

- [x] **简化 `tsconfig`（CLI `--project`）**：递归 **`extends`**、**`include` / `exclude` glob**、合并后的 **`files`**（仍**无** npm / `node_modules`；合并语义为简化子集，非完整 `tsc`）。实现：[`tsconfig_resolve`](crates/trust-cli/src/tsconfig_resolve.rs)、[`graph_loader`](crates/trust-cli/src/graph_loader.rs)。**仅 `include`** 时匹配的 `.ts` 会排序，**入口**为字典序第一个；需固定顺序时用显式 **`files`**。验收：`tsconfig_resolve` 单测、`run_project_tsconfig_extends_include_ok`。
- [x] **npm / 包管理器式解析**：**`node_modules`、npm 包、以及典型 `compilerOptions.paths` 指向包布局** — **明确非目标，不计划做。** 导入仍以**相对路径** `./x.ts` 为主（外加 CLI 多根 / `--project` 根列表）。
- [x] **相对路径重导出**：`export * from "./x.ts"`、`export { a as b } from "./x.ts"`（值导出；**`export * as` / 无 `from` 的 `export { x }`** 仍拒绝；**默认导出**为受限子集 — 见 §13.6）。有效导出与 `validate_imports`：[`effective_exported_function_names_by_path`](crates/trust-parser/src/module_graph.rs)；模块图沿 re-export 拉取依赖。HIR 跳过上述语句。验收：`validate_import_via_export_star_from`、`run_reexport_export_star_ok`。
- [x] **README / 矩阵对齐**：已同步「非 1.0」、不支持表与 §1.1（本条）。

### 性能与安全（与 §10–§11 对齐；**并行 / 代码安全 / driver 资源**以本节勾选为准）

- [x] **增量编译**（多文件、仅重编变更模块；与 §10 同条，已勾选）。
- [x] **并行**多文件语义检查。（各函数的 [`check_function`](crates/trust-hir/src/sem.rs) 经 **`rayon`** `par_iter_mut` 并行；[`SendSourceMap`](crates/trust-hir/src/ir.rs) 使 [`IRFunction`](crates/trust-hir/src/ir.rs) 在 `swc` `Lrc` 下仍可 `Send`；警告顺序与 `module.fns` 一致。）
- [x] **生成代码安全**：字符串转义、`println!` 注入等审计。（类 `__class_name` 字符串字面量用 `Debug` 转义；[`emit_builtin_log`](crates/trust-hir/src/codegen.rs) 标明格式串为固定模板；模板字面量在 [`emit_tpl`](crates/trust-hir/src/codegen.rs) 中已对 `{`/`}` 转义。）
- [x] **Driver 资源限制**：对子进程 `cargo` 的可选超时/内存上限。（[`RustBuildOptions::cargo_timeout`](crates/trust-driver/src/lib.rs)、[`max_cargo_output_bytes`](crates/trust-driver/src/lib.rs)；[`cargo_build`](crates/trust-driver/src/cargo_runner.rs) 使用 [`wait_timeout::ChildExt`]。）

### async / HTTP（MVP；残余 backlog）

- [x] **任意控制流中的 `await`**（不限于当前 async MVP 体约束）。（已移除 [`check_async_mvp_stmts`](crates/trust-hir/src/sem.rs)；[`infer_expr_mut`](crates/trust-hir/src/sem.rs) 中 `Await` 接受内部 awaitable 操作数；[`async_control_flow_ok.ts`](crates/trust-cli/tests/fixtures/async_control_flow_ok.ts)、`compile_async_control_flow_if_while_await_ok`。）
- [x] **`async_all([...])`**（仅数组字面量；同质 awaitable）。见 [`IRExpr::PromiseAll`](crates/trust-hir/src/ir.rs)、[`async_all_fetch_ok.ts`](crates/trust-cli/tests/fixtures/async_all_fetch_ok.ts)、`compile_async_all_fetch_alias_ok`。
- [x] **`fetchText`** → `trust_stdlib::http::fetch_text`；**`fetch`** → `trust_stdlib::http::fetch` + `FetchInit`（见 [`crates/trust-stdlib/src/http.rs`](crates/trust-stdlib/src/http.rs)）。
- [x] **流式响应 body（M3）**：`response.body.getReader()` / `await reader.read()` → `StreamReadResult`；与 `.text()` / `.json()` 互斥；[`fetch_stream_ok.ts`](crates/trust-cli/tests/fixtures/fetch_stream_ok.ts)、`compile_fetch_stream_ok`。
- [x] **TLS / HTTP 说明（非「与 Node 完全一致」）**：临时 crate 使用 **reqwest** + **rustls-tls**；**TLS 1.2+** 与 **HTTP/2**（ALPN 协商成功时）由该栈提供；**根证书、cipher、HTTP/2 细节不保证与某一 Node 或浏览器版本逐字节一致**。**仍为 backlog**：完整 **WHATWG `fetch`**（浏览器级 `ReadableStream`、`Request`/`Headers` 对象、duplex、在非浏览器宿主下的 CORS 等）；trust 子集已支持 **`getReader`/`read` 分块读 body**（见上条）。

### 语言与类型（trust 强类型子集）

- [x] **可选调用** `f?.()`；**`??` / `?.` 的静态收窄**（可判定；§3.3）。已实现：`OptionalCall` / `OptionalMethodCall`、`build_opt_chain_call_expr`；`optional_call_ok.ts`、`optional_chain_fail.ts`；`NullishCoalesce` 对同族 `Union` 去空值合并。**完整** discriminated 收窄仍属后续。
- [x] **链式调用** `f().g()`（一层）。`chain_call_ok.ts`，`run_chain_call_ok_prints_six`；更一般实例方法类型仍见 §1.3。
- [x] **数值模型**：全局 `number` → Rust **`f64`**（`IRExpr::Number(f64)`、codegen）；下标等仍 `as i32`。与旧 `i32` 截断不兼容；见 README。
- [x] **HIR 标准库 / JSON / 字符串**：`JSON.parse` 对**字符串字面量**在构建期用 `serde_json` 折叠为 trust 闭合 IR；**非常量**实参仍为 JSON **number** 文档 → `f64`（`trust_stdlib::json::parse_number`，与 `await response.json()` 一致）。全局 **`encodeURIComponent`** / **`decodeURIComponent`** 经 `trust_stdlib::uri`。生成 crate 默认注入 `trust_stdlib`；生成代码若含 `serde_json::`（对象字面量 / 对象 `JSON.stringify`）则仍写入 `serde_json` 依赖。见 `json_uri_trust_ok.ts`、`json_parse_hetero_array_fail.ts` 与对应 `run_` / `compile_` 测试。

### Trust.toml / Rust extern（crates.io）

- [x] **清单**：解析 `Trust.toml`；将 `[dependencies]` 合并进生成 crate 的 `Cargo.toml`；自入口 `.ts` 向上发现（[`trust-manifest`](crates/trust-manifest)、[`crate_writer`](crates/trust-driver/src/crate_writer.rs)、[`graph_loader`](crates/trust-cli/src/graph_loader.rs) / [`pipeline`](crates/trust-driver/src/pipeline.rs)）。
- [x] **模块图**：`import … from "crate_key"` 且键在清单中；`validate_imports` 校验 `[[rust_binding]]`；不对虚构的 Rust 模块做文件系统 DFS。
- [x] **HIR / sem / codegen**：`TsType::RustExtern`、`IRExpr::RustNew`、带 `inherent_rust_str_ref` 的固有 `MethodCall`（`string` 实参生成 `.as_str()` 以匹配 `&str`）。
- [x] **E2E**：[`tests/fixtures/trust_regex/`](crates/trust-cli/tests/fixtures/trust_regex/) — `run_trust_regex_ok_prints_one`、`compile_trust_regex_ok_emits_regex_crate`。
- [x] **常用 crate 的预置绑定 shim（入门模板）**：无中央 registry；可复制模板见 [`examples/README.md`](examples/README.md)（Diesel/FFI 示例 + 最小 [`tests/fixtures/trust_regex/`](crates/trust-cli/tests/fixtures/trust_regex/) `Trust.toml` 模式）。若需「精选列表 / `trust add` 预设」仍为后续可选工作。

### 文档与示例

- [x] **README + 本清单**：定期对照实现扫一遍（矩阵、§1.3、§2.1 等），避免与已交付特性（如 stdlib、`string[i]`、`Math` 扩展）矛盾。*（本次已对齐 §1.3 / §2.1 与 `.json` 语义；README 矩阵见 [语言功能矩阵](README.zh-CN.md)。）*
- [x] **[`test-ts/main.ts`](test-ts/main.ts)**：保持在支持子集内；文件头注明预期 I/O，并**刻意不包含** `async`/`fetch`/泛型调用等（由 `fixtures/` 覆盖）。

### 汇总产品待办（README / Partial / 慢车道）

由 [README.zh-CN — 不支持的 TypeScript](README.zh-CN.md)、功能矩阵 **Partial** 行、§1.3 附注、[§3.3.1](PROJECT-TODO.zh-CN.md)、§14 残余项归纳。**强类型、可静态判定、可 codegen** 仍须遵守（[README — Trust: strong typing](README.md)）。建议 **每次 PR 尽量只攻一条主线**。完成某项后，在此打 `[x]`，并更新重复叙述处（§3.3.1、§13.7、README 矩阵等）。

#### 类型与语义（相对完整 `tsc`）

- [x] **D1 — Discriminated 收窄**：对象联合上 `if (v.kind === 'a')` / `else`；与 `??` / `?.` 协同（[§3.3.1](PROJECT-TODO.zh-CN.md)、[`sem.rs`](crates/trust-hir/src/sem.rs)）。已完成：通过 `Binding` 上的 `narrow_ty` 支持嵌套收窄。
- [ ] **D3 向 — 更宽的联合 / `normalize_union` / 可赋值性**：仅当仍能落到**单一** Rust 类型且可静态判定（[§3.3](PROJECT-TODO.zh-CN.md)）。
- [ ] **G1 / G2 — 泛型子集扩大**（更多显式实参形态、更多元数或非调用位置单态）（[§3.3.1](PROJECT-TODO.zh-CN.md)、[`sem/mono.rs`](crates/trust-hir/src/sem/mono.rs)）。
- [ ] **G3 — 类 where 约束**（若做：须全程可静态检查）。
- [ ] **交集类型 `A & B`**（类型位置；当前拒绝）。
- [ ] **类型位置的 `bigint` / 模板字面量类型**（当前拒绝）。
- [ ] **与 `tsc` 对齐的完整结构子类型** — 非目标；若做只接受**有文档的安全子集**。
- [ ] **异质联合**（如 `number | string`）：codegen 策略或「无法单类型映射」时的诊断更清晰（见 README 矩阵「联合」行）。
- [ ] **等价于 `strictNullChecks` 的模式**：须为**显式**编译选项，而非默认宽松 JS 语义。
- [x] **R1 — `interface` / 对象名义方法**（静态分派；全局 `m__I(receiver,…)` 或固有方法）（[§3.3.1](PROJECT-TODO.zh-CN.md)）。已完成：interface 支持方法签名，方法调用类型检查。
- [ ] **R2 — 比一层 `f().g()` 更深的类型驱动链**（[§1.3](PROJECT-TODO.zh-CN.md) 附注）。
- [ ] **§13.7 B2a + B2+** — `import type`、跨文件 interface/type 名、`interface extends`、对象可调用成员、更丰富的 `readonly`/索引签名等（[§13.7](PROJECT-TODO.zh-CN.md)）。

#### 语言表面

- [ ] **更多 `export` 形态** — 仅当产品范围变更；当前仍拒绝任意 `export default` 表达式、`export const`、`export * as`、无 `from` 的 `export { x }` 等（[README 不支持表](README.zh-CN.md)、§13.6）。
- [ ] **更广的可选链**（仍被拒的 callee/形态 — 见 [`optional_chain_fail.ts`](crates/trust-cli/tests/fixtures/optional_chain_fail.ts)）。
- [ ] **`&&` / `||` 的值保留语义** vs 当前结果类型 **`boolean`** — 需先写清**强类型**规格再实现（README 矩阵）。
- [ ] **`for..of`** 循环。
- [ ] **带标签的 `break` / `continue`**。
- [ ] **计算属性成员与调用** `obj[expr]`、`obj[expr](…)`（在可判定前提下）。

#### 异步与闭包

- [ ] **闭包 codegen** 超出当前 **`(number) => number` 式**严格子集（README 矩阵「高阶函数」）。
- [ ] **WHATWG / 浏览器级 `fetch` 对齐**（`Headers` 遍历、完整 `Request`、duplex 等）— 相对现有 reqwest 子集的残余（README「Web fetch」非目标说明）。

#### 工具链与解析体验

- [ ] **C1 — `--ts-source-comments` 下未附着注释的可选警告**（[§3.3.1](PROJECT-TODO.zh-CN.md)、上文「注释与生成 Rust」）。
- [ ] **C2 / C3 — Trailing / 行内注释** 与 **大 lowering 后的注释继承**（`switch`→`if`、`for`→`while` 等）。
- [ ] **模块图：首个解析失败后仍收集其他文件诊断**（相对当前遇错即停）（§14 工具链）。
- [ ] **解析器 / AST 级 error recovery**（若需要，在 swc 现状之上扩展）。

#### 生态（除非改产品决策，否则多为非目标）

- [ ] **npm / `node_modules` / `paths` 指包** — 当前**不计划**；若做须单独立项（README「非 1.0」）。
- [ ] **完整 `tsconfig` / `tsc` 行为对齐** — 同上。

---

## 维护说明

- 完成一项后，将对应 `[ ]` 改为 `[x]`，或在项下追加「完成于 commit / PR #」。
- 若某项范围变化，在条目末尾用括号注明**替代方案**或**废弃原因**。
- 与 [`README.zh-CN.md`](README.zh-CN.md)（或英文 [`README.md`](README.md)）语言矩阵冲突时，以代码为准并更新 README。
