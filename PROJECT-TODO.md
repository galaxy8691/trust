# ts2rs 项目长期 TODO 清单

本文档用于**长期跟进**编译器与工具链的演进，按主题分层列出可验收项。状态建议用 `[ ]` / `[~]` 进行中 / `[x]` 在 PR 或提交中维护。**本清单所列特性均以硬类型（trust）为前提。**

**相关代码入口**：[`README.md`](README.md) · [`crates/ts2rs-hir`](crates/ts2rs-hir)（`build.rs` / `sem.rs` / `codegen.rs` / `ir.rs`）· [`crates/ts2rs-parser`](crates/ts2rs-parser) · [`crates/ts2rs-driver`](crates/ts2rs-driver) · [`crates/ts2rs-cli`](crates/ts2rs-cli) · [`test-ts/main.ts`](test-ts/main.ts)（多文件：`test-ts/math.ts`） · [`crates/ts2rs-cli/tests/fixtures/`](crates/ts2rs-cli/tests/fixtures/)

### 规划约束：硬类型（trust）

**trust 为硬类型，不允许软类型。** 长期条目与 PR 取舍须与此一致：只扩展能在 HIR / [`sem.rs`](crates/ts2rs-hir/src/sem.rs) 中给出**静态**规则的语法；**不**把「隐式 any、运行期改型、无注解宽进」等软类型能力列入本仓库目标。详细表述见 [README「类型立场：硬类型」](README.md)。  
本文中「收窄」「可赋值」「结构/形状」均指 **HIR / sem 内的静态规则**，**不**表示运行期改型，也**不**表示向 `tsc` 默认宽松或渐进式软类型靠拢。

---

## 0. 愿景与「1.0」验收标准（可删减）

- [x] **单文件子集**：对 README 矩阵中声明支持的特性，均有对应 fixture 与集成测试（[`crates/ts2rs-cli/tests/fixtures/`](crates/ts2rs-cli/tests/fixtures/) + [`cli_e2e.rs`](crates/ts2rs-cli/tests/cli_e2e.rs)）；`ts2rs-lower` 另有 compile 单元测试。
- [x] **诊断**：常见错误带行列号（`path:line:col`）；文案为**英文**（见 README「1.0 范围」）。
- [x] **可复现**：`cargo test --workspace`、`cargo clippy --workspace --all-targets`；[`.github/workflows/ci.yml`](.github/workflows/ci.yml) 在 push/PR 上执行。
- [x] **多文件（若纳入范围）**：**不纳入 1.0** 完整工程图；**相对路径** `import { x } from "./dep.ts"` 由 [`parse_module_graph`](crates/ts2rs-parser/src/module_graph.rs) 构建模块图（不合并 AST），CLI 与 [`compile_entrypoint_to_executable`](crates/ts2rs-driver/src/lib.rs) 走 `validate_imports` → `lower_module_graph`；见 §6.2。

---

## 1. 前端：解析与 AST 覆盖

### 1.1 已支持路径的健壮性

- [x] **错误恢复**：**当前策略为单条诊断**（首次失败即返回）；多诊断收集为后续增强，见 [README.md](README.md)「诊断与前端健壮性（§1.1）」。
- [x] **保留注释**：**已评估**：AST 不挂注释，`source_map` 已有；贯通注释需 parser/token 层扩展，结论写在 README §1.1。
- [x] **`export` 变体**：除 `export function` 外均已显式拒绝（[`build.rs`](crates/ts2rs-hir/src/build.rs)）；负例 fixtures `export_*_fail.ts` + `cli_e2e`。

### 1.2 语句与声明扩展

- [x] **`import`**：相对路径 `import { f } from "./x.ts"` 由模块图解析（[`module_graph.rs`](crates/ts2rs-parser/src/module_graph.rs)），旧实现 [`resolve_imports.rs`](crates/ts2rs-parser/src/resolve_imports.rs) 已废弃；非相对路径仍报错（见 `import_fail.ts`）。
- [x] **嵌套 `function`**：[`IRStmt::FnDecl`](crates/ts2rs-hir/src/ir.rs) + 无捕获子集；见 `nested_fn.ts`。
- [x] **`const`**：与 `let` 对齐，语义禁止对 `const` 赋值；见 `const_ok.ts`、`const_reassign_fail.ts`。
- [x] **表达式语句中的赋值**：`IRStmt::Assign` + 可变 `let`；见 `assign_simple.ts`。
- [x] **`for` / `do-while`**：C 风格 `for`（含 update 赋值）、`do-while`；**`switch`** 仍显式拒绝（`switch_fail.ts`）。
- [x] **`break` / `continue`**：循环内；label 未支持。
- [x] **空语句 / 块**：`Stmt::Empty`、`Block`；见 `empty_stmt.ts`。

### 1.3 表达式扩展

- [x] **成员访问与调用链**：受限子集；当前仅 `string` 的 `.length`（见 `member_length_ok.ts`）；一般 `obj.m()` / 链式调用待扩展。
- [x] **可选链 / 空值合并**：受限子集已支持（`obj?.prop`、`??`；见 `optional_ok.ts`、`nullish_ok.ts`）；完整语义依赖 §3.3。
- [x] **逻辑与短路**：`&&`、`||`；`boolean` 与 `number` 真值（`!= 0`）已支持，结果类型为 `boolean`（见 `logical_bool.ts`、`logical_truthy_ok.ts`）；与 TypeScript 值保留式 `&&`/`||` 仍不同；**硬类型下**结果类型固定为 `boolean`，更复杂真值或联合操作数仍受限。
- [x] **三元运算符**：`cond ? a : b`（见 `ternary_ok.ts`）。
- [x] **逗号表达式**：见 `comma_ok.ts`。
- [x] **模板字符串**：无 tag；见 `template_ok.ts`。
- [x] **数组 / 对象字面量**：受限子集已支持（`number[]`、`{ k: number }`；见 `array_ok.ts`、`object_ok.ts`）；运行时与完整类型见 §1.4 / §2.1。

**§1.3 仍待后续（原因备忘）**

- **`obj.m(args)`（成员调用脱糖）**：已实现 [`IRExpr::MethodCall`](crates/ts2rs-hir/src/ir.rs) → 全局函数 `m(receiver, ...args)`（须存在对应顶层函数；验收：`method_call_ok.ts`、`cli_e2e` `run_method_call_ok_prints_three`）。**链式方法调用** `f().g()`、**一般方法类型**仍待。
- **`?.()`（可选调用）与 `??` 的完整静态收窄**：须在 **硬类型、可静态判定** 前提下与 §3.3 对齐；可选调用仍显式拒绝；`??` 为受限实现。
- **数组/对象字面量的「完整」类型**：更丰富的元素与字段类型、`TsType`/IR 演进见 §1.4、§2.1，不单属表达式扩展层。

### 1.4 类型语法（仅类型层）

**摘要**

- [x] **字面量类型**、**联合类型**、**接口**、**type 别名**：与 **硬类型** checker 路线图对齐（拆分为下列子项；**字面量类型**、**primitive/字面量联合**、**受限 `interface`→`ObjectNum`** 与 **受限 `type` 别名→具名表** 已见子项）。**泛型**见下列独立子项（文档化「仍拒绝」里程碑，非实现语义）。

**与已实现子集的关系**：§1.3 已支持受限注解 `number[]`、`{ k: number }`（HIR 中 [`TsType::ArrayNumber`](crates/ts2rs-hir/src/ir.rs) / [`ObjectNum`](crates/ts2rs-hir/src/ir.rs)）。**字面量类型**（`NumberLit` / `StringLit` / `BoolLit`）与 **联合类型**（[`TsType::Union`](crates/ts2rs-hir/src/ir.rs) + 规范化）已见下项；**顶层 `interface`** 在类型层等价于具名 `ObjectNum`（与对象类型字面量同一规则）；**顶层 `type` 别名**经 [`collect_named_types`](crates/ts2rs-hir/src/build.rs) 解析为既有 `TsType` 并进入同一张具名表；**泛型语义**仍未实现，拒绝对照见下列子项与 [README §1.4](README.md)；完整对象/接口形状与 IR 演进见 §2.1；**静态**空值与分支收窄与 §3.3 交叉。

**子项（逐项勾选）**

- [x] **字面量类型**（如 `42`、`"a"`、`true` 出现在类型位置）  
  - **依赖**：扩展 [`TsType`](crates/ts2rs-hir/src/ir.rs) 或等价表示；字面量与基类型的**静态**可赋值关系（与 §3.3 **显式形状 / sem 规则**一致，非 TS 结构子类型全集）。  
  - **验收**：[`build.rs`](crates/ts2rs-hir/src/build.rs) 解析 `TsLitType`；[`sem.rs`](crates/ts2rs-hir/src/sem.rs) `type_assignable` / 推断字面量；`literal_type_ok.ts`、`literal_type_fail.ts` + [`cli_e2e.rs`](crates/ts2rs-cli/tests/cli_e2e.rs)。

- [x] **联合类型**（`A | B`，建议先 primitive / 字面量联合再扩展）  
  - **依赖**：类型规范化与可判定相等的并集表示；与 `??` / `?.` 的**静态收窄 / 分支类型**（硬类型下、须可静态判定）对齐 §3.3。  
  - **验收**：受限联合下的赋值与分支可给出一致诊断或生成；集成测试覆盖典型路径（`union_literal_ok`、`union_cond_ok`、负例 `union_heterogeneous_fail`、`intersection_type_fail`、`union_mixed_cond_fail`）。

- [x] **`interface` 与对象类型**（声明体、可选属性、`extends` 等按阶段）  
  - **依赖**：**显式字段形状**进入 IR（§2.1），经 sem **静态**检查；**非** TS 结构子类型全集；与现有 `ObjectNum` 子集的关系在实现 PR 中写清（兼容或迁移路径）。  
  - **验收**：至少一种 `interface` 形态可编译到等价 Rust 或明确诊断边界（`interface_ok`、`export_interface_ok`；`extends`/泛型负例 `interface_extends_fail`、`interface_generic_fail`；README 说明单文件与顺序）。

- [x] **`type` 别名**（`type Id = …`）  
  - **依赖**：顶层收集 `TsTypeAlias`（或等价）并入符号表；解析在 swc 侧已有，需进入 HIR/语义。  
  - **验收**：别名可在参数/变量注解中解析；fixture + e2e（`type_alias_ok`、`type_alias_to_interface_ok`、`export_type_alias_ok`；负例 `type_alias_generic_fail`、`type_alias_dup_fail`）。

- [x] **泛型**（函数 `function f<T>(…)` 与类型上参数）  
  - **依赖**：单态化（per-call 特化）或受限策略仍为**后续工作**；当前拒绝入口与英文诊断见 [README §1.4「泛型与类型参数」](README.md)、[`build.rs` 中 generic 相关检查](crates/ts2rs-hir/src/build.rs)。  
  - **验收**：分阶段文档化「仍拒绝」— [README](README.md) 对照表 + 负例 `generic_function_fail`、既有 `interface_generic_fail`、`type_alias_generic_fail` 与 e2e；**不**在本里程碑实现泛型语义。

---

## 2. IR（`ir.rs`）演进

### 2.1 当前结构补强

- [x] **语句**：已含 `Assign`、`Break`、`Continue`、`DoWhile`、`FnDecl`、`Empty`；`for` 展开为 `while`；`Switch` 未实现。
- [x] **表达式**：已含 `LogicalAnd` / `LogicalOr`、`Conditional`（三元）、`Seq`（逗号）、`Tpl`（模板）、受限 `Member`；`Index` 与完整成员链待扩展。由 §1.3 引入的数组下标与 `ObjectNum` 字段访问已覆盖；**完整** `interface` / **显式形状**对象类型见 §1.4 与后续 IR 扩展（硬类型、静态检查）。
- [x] **顶层**：多文件模块图 — `parse_module_graph` + `validate_imports`；HIR 合并为 [`IRModule`](crates/ts2rs-hir/src/ir.rs)（`build_program_multi` / `compile_graph`）；`main` 须在入口文件；全局函数名唯一；负例见 `import_missing_export_*`、`circular_*`、`dup_*` fixtures。

### 2.2 元数据与调试

- [x] **Span**：HIR 节点带 swc `Span`；[`diag`](crates/ts2rs-hir/src/error.rs) 与 codegen 错误均用所属函数的 `cm` + `source_path` + 节点 `span`（见 [`ir.rs`](crates/ts2rs-hir/src/ir.rs) 模块注释）；`build` 无顶层函数时用整文件 `span`；`sem` 缺 `main` 时锚点为第一函数 `span`。
- [x] **可选**：[`IRFunction::ir_id`](crates/ts2rs-hir/src/ir.rs)（函数级，含嵌套 `function`；单次编译内单调递增，见 `build_fn`）。

---

## 3. 语义分析（`sem.rs`）

### 3.1 已实现的巩固

- [x] **符号表**：块作用域与 `let` 重复绑定（已部分实现）— 增加用例覆盖边界（嵌套块、与参数同名等）。验收：`let_dup_same_block_fail.ts`、`let_shadow_nested_ok.ts`、`param_let_same_name_fail.ts` + [`cli_e2e.rs`](crates/ts2rs-cli/tests/cli_e2e.rs)。
- [x] **控制流**：`stmts_return` 简化规则— 文档化并与 TS/tsc 差异列表对照（**trust 仅保证静态规则**；见 README）。验收：[README.md「控制流与 return（简化语义）」](README.md)。
- [x] **`void` 与 `console.log`**：`BuiltinLog` 为 void 表达式路径已覆盖；补充「仅 log 的表达式语句」在分支中的用例。验收：`void_log_in_branch.ts` + e2e。

### 3.2 可变性与赋值

- [x] **`let` 可变**：`IRStmt::Assign`；语义检查 LHS 为已绑定标识符。
- [x] **禁止对 `const` 赋值**。

### 3.3 类型系统加深

- [x] **与 §1.4 的衔接**：字面量类型、联合类型与 `??` / `?.` **静态**收窄应在实现时与 §1.4 子项对齐（避免与当前受限 `TsType` 语义冲突）。联合类型已入 HIR；**均在硬类型、可静态实现前提下**，`??` / `?.` 的**完整** discriminated / 空值收窄仍待后续。验收：[README「语义与类型路线（§3.3）」](README.md) 已写明衔接关系与 `nullish_ok` / `optional_ok` 受限子集。
- [x] **`null` / `undefined`**：[`TsType`](crates/ts2rs-hir/src/ir.rs) 已含 `Null` / `Undefined` 等变体；检查以 **当前 sem 静态规则**为准，**不设** tsc 默认「万物可空」式软语义。验收：README §3.3；**未**实现 `strictNullChecks` 式开关。若将来增加模式，应为**显式编译选项**（如 strict 空值），**非**隐式放宽或兼容 JS 动态性。
- [x] **结构类型 vs 名义类型**：**trust 以名义表 + 静态形状检查为界**；与 Rust 后端映射策略（当前以基础类型为主）。验收：README §3.3 已说明具名表/语义检查与 Rust 生成侧边界；**未**实现 TS 结构子类型全集，**不**将其列为路线目标。
- [x] **函数类型**：高阶函数（函数作值）须先有**静态函数类型**与 IR，再 codegen。验收：README §3.3 已说明仅 `function` 声明与调用、无函数作一等值；**未**扩展 IR/codegen 支持高阶函数。

### 3.4 控制流分析（进阶）

- [x] **可达性**：不可达代码警告（英文 `warning: path:line:col: unreachable code`）；见 `early_return_unreachable.ts`、`unreachable_after_return.ts`、`break_unreachable.ts` + [`cli_e2e.rs`](crates/ts2rs-cli/tests/cli_e2e.rs)。
- [x] **明确赋值**：`let x: number;` 无初始化已允许；使用前须明确赋值（`if`/`else` 合并、循环保守策略）；正例 `definite_assign_ok.ts`、`definite_assign_if_ok.ts`，负例 `definite_assign_fail.ts`。
- [x] **更精确的 return**：序列内提前穷尽返回（如 `if`/`else` 均 `return` 后允许后续死代码仅警告）；`while`/`do-while` 仍沿用原尾部规则（`tail_returns_while_body`）；未来 `switch` 可在 `stmt_fn_returns_complete` 侧扩展。

---

## 4. 代码生成（`codegen.rs`）

### 4.1 当前行为改进

- [x] **`console.log` 多参数格式**：[`emit_builtin_log`](crates/ts2rs-hir/src/codegen.rs) 多参数已改为 `"{}"` 空格分隔；验收：`ts2rs-lower` 单测 `console_log_multi_arg_uses_spaced_format`。
- [x] **整数除法**：codegen 仍为 `i32` `/`（向零截断）；验收：README「算术、`/` 与溢出」与矩阵 `console.log` / 算术行。
- [x] **溢出**：README 已说明 `i32` 范围、与 TS `number` 差异及 Rust 溢出语义；**未**加运行时检查 Cargo feature（留待后续）。

### 4.2 新特性映射

- [x] **赋值**：`let mut` 与块作用域对齐 Rust。（验收：`ts2rs-lower` `codegen_42_let_mut_block_and_assign`；[`emit_stmt`](crates/ts2rs-hir/src/codegen.rs) `Let` / `Assign` / `Block`；`cargo test --workspace`。）
- [x] **字符串**：继续 `String` + `format!`；大字符串与性能另议。（验收：`ts2rs-lower` `codegen_42_string_concat_uses_format`；[`emit_expr`](crates/ts2rs-hir/src/codegen.rs) `StrConcat` / `Tpl`；`cargo test --workspace`。）
- [x] **堆对象 / GC**：当前对象为值型 `HashMap::from`，**未**引入 `Rc`/`Arc`；若将来引用类型再定策略。（验收：`ts2rs-lower` `codegen_42_object_literal_hashmap_without_rc`；[`emit_expr`](crates/ts2rs-hir/src/codegen.rs) `ObjectLit`；`cargo test --workspace`。）

### 4.3 生成代码可读性

- [x] **缩进与换行**：逗号表达式（`Seq`）块内行与闭合 `})` 与外层语句层级对齐，便于 rustfmt。（验收：`ts2rs-lower` `codegen_43_comma_seq_indented`；[`emit_seq_expr`](crates/ts2rs-hir/src/codegen.rs) / `emit_expr` 的 `stmt_level`；`cargo test --workspace`。）
- [x] **注释**：可选在每条语句前注入 `// ts: path:line:col`（与诊断同源 `lookup_char_pos`）。（验收：`ts2rs-lower` `codegen_43_span_comments_emits_ts_anchors`；`ts2rs-cli` `compile_span_comments_writes_ts_anchors`；[`emit_stmt`](crates/ts2rs-hir/src/codegen.rs) 与 `CodegenOptions`；`ts2rs compile --span-comments`；`cargo test --workspace`。）

---

## 5. 内建与标准库映射

### 5.1 `console`

- [x] **`console.log` / `console.error` / `console.debug`**：`log` → `println!`，`error` / `debug` → `eprintln!`。（验收：`ts2rs-lower` `console_error_and_debug_use_eprintln`；`ts2rs-cli` `compile_console_stderr_writes_eprintln`；[`build.rs`](crates/ts2rs-hir/src/build.rs) `console` 成员；[`emit_builtin_log`](crates/ts2rs-hir/src/codegen.rs)；`cargo test --workspace`。）
- [x] **格式化语义**：与 §4.1 相同，多参 `"{}"` 空格分隔；`log` 与 `error`/`debug` 共用 [`emit_builtin_log`](crates/ts2rs-hir/src/codegen.rs) 格式构造。（验收：同上 + 已有 `console_log_multi_arg_uses_spaced_format`；`cargo test --workspace`。）

### 5.2 最小运行时（[`ts2rs_rt`](crates/ts2rs_rt)）

- [x] **字符串操作**：`string.length` 为 **UTF-16 码元数**（`encode_utf16().count()`）；`number[].length` → `Vec::len`；对象数字字段名为 `length` 时走 `HashMap::get`（见 [`MemberLengthDispatch`](crates/ts2rs-hir/src/ir.rs)）。（验收：`ts2rs-lower` `codegen_52_string_length_utf16`、`codegen_52_object_length_field_uses_get`；`ts2rs-cli` `run_string_utf16_length_prints_two`、`run_array_length_prints_three`、`run_object_length_field_prints_value`；`cargo test --workspace`。）**未实现**：`string` 下标 `s[i]`（仅 `number[]` 下标；完整 UTF-16 单元语义留后续）。
- [x] **数学**：`Math.abs` / `Math.min` / `Math.max` / `Math.floor` / `Math.ceil` 整数子集（[`MathBuiltinKind`](crates/ts2rs-hir/src/ir.rs)；[`build.rs`](crates/ts2rs-hir/src/build.rs) `Math`；[`emit_expr`](crates/ts2rs-hir/src/codegen.rs)；`floor`/`ceil` 在纯 `i32` 下为恒等）。（验收：`ts2rs-lower` `codegen_52_math_builtins`；`ts2rs-cli` `run_math_builtin_prints_sum`；`cargo test --workspace`。）
- [x] **I/O**：[`ts2rs_rt::read_stdin_line`](crates/ts2rs_rt/src/lib.rs) 占位（`std::io`）；**生成代码与 driver 临时 crate 仍未依赖 `ts2rs_rt`**，全量接入留后续。（验收：crate 文档与 API 存在；`cargo test --workspace`。）

---

## 6. Driver 与构建（[`ts2rs-driver`](crates/ts2rs-driver)）

### 6.1 单文件路径

- [x] **临时目录生命周期**：crate 与 [`compile_entrypoint_to_executable`](crates/ts2rs-driver/src/lib.rs) / [`build_rust_to_executable`](crates/ts2rs-driver/src/lib.rs) / [`build_rust_and_copy`](crates/ts2rs-driver/src/lib.rs) 文档说明 [`TempDir`](https://docs.rs/tempfile) drop 删除目录、返回元组为 `(TempDir, PathBuf)`。（验收：文档见 [`lib.rs`](crates/ts2rs-driver/src/lib.rs) 顶部与上述函数；`cargo test --workspace`。）
- [x] **离线 / 无 cargo 环境**：`cargo` 无法启动（`NotFound`）时 [`DriverError::CargoNotFound`](crates/ts2rs-driver/src/lib.rs)；编译失败（含网络/依赖）仍为 [`DriverError::CargoBuild`](crates/ts2rs-driver/src/lib.rs) 并附 stdout/stderr。（验收：单测 `map_cargo_spawn_error_maps_not_found_to_cargo_not_found`；`cargo test --workspace`。）

### 6.2 多文件与模块（[`compile_entrypoint_to_executable`](crates/ts2rs-driver/src/lib.rs)）

- [x] **解析多入口**：[`parse_module_graph_with_extra_roots`](crates/ts2rs-parser/src/module_graph.rs)；CLI 多 `.ts` 位置参数或 `--project` + 极简 JSON `files`（[`ts2rs-cli`](crates/ts2rs-cli/src/main.rs)）。（验收：`module_graph::tests::extra_root_includes_unreachable_file`；`cli_e2e` `run_multi_entry_extra_roots_prints_main`、`run_project_tsconfig_prints_main`。）
- [x] **依赖图（子集）**：入口文件 + 相对 `import` → [`parse_module_graph`](crates/ts2rs-parser/src/module_graph.rs)（保留各模块 AST）→ `validate_imports` → [`lower_module_graph`](crates/ts2rs-lower/src/lib.rs) → 单 Rust crate。
- [x] **`Cargo.toml` 生成**：[`RustBuildOptions`](crates/ts2rs-driver/src/lib.rs) / [`build_rust_to_executable_with_options`](crates/ts2rs-driver/src/lib.rs)；可选 path 依赖 `ts2rs_rt` + feature `ts2rs_rt`；CLI `--link-ts2rs-rt`。（验收：`write_minimal_crate_with_link_ts2rs_rt_contains_optional_path_dep`；`cli_e2e` `run_with_link_ts2rs_rt_prints_main`；`cargo test --workspace`。）
- [x] **循环依赖**：[`parse_module_graph`](crates/ts2rs-parser/src/module_graph.rs) 检测并报错（见 `circular_*.ts`）。

---

## 7. CLI（[`ts2rs-cli`](crates/ts2rs-cli)）

- [ ] **子命令**：`compile` / `run` 文档化；可选 `check`（只语义不生成）。
- [ ] **选项**：`-O`、输出路径、`-q`、颜色、`--emit-ir`（调试用）。
- [ ] **退出码**：约定编译失败非零、与诊断数量（可选）。

---

## 8. 测试与质量

### 8.1 集成测试（[`cli_e2e.rs`](crates/ts2rs-cli/tests/cli_e2e.rs) / `fixtures/`）

- [ ] **每个矩阵行**一条最小 fixture（或合并大文件但注释分段）。
- [ ] **回归**：已知 bug 固定为 `tests/regression/*.ts`（若引入目录）。

### 8.2 单元测试

- [ ] **`ts2rs-hir`**：`build`/`sem`/`codegen` 分模块 `#[cfg(test)]`。
- [ ] **parser**：swc 封装层快照或最小片段。

### 8.3 工具链

- [ ] **CI 工作流**（GitHub Actions / 其他）：`cargo test`、`clippy`、格式化检查。
- [ ] **模糊测试**（可选）：随机 AST 片段不 panic。

---

## 9. 文档与开发者体验

- [ ] **README**：与实现同步更新矩阵；「不支持的 TS 特性」简表（**兼作 trust 硬类型拒斥边界说明**）。
- [ ] **架构图**：解析 → HIR → sem → codegen → driver（可 Mermaid）。
- [ ] **贡献指南**：`CONTRIBUTING.md`（分支、测试命令、MSRV）。
- [ ] **变更日志**：`CHANGELOG.md`（若对外发布）。

---

## 10. 性能与规模（后期）

- [ ] **增量编译**：多文件时只重编译变更模块。
- [ ] **并行**：多文件语义检查并行化。

---

## 11. 安全与边界

- [ ] **生成代码注入**：字符串字面量转义与 `println!` 安全。
- [ ] **资源限制**：driver 调用 `cargo` 超时/内存（可选）。

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
| P3     | 泛型、硬类型下静态类型系统深化          | 长期（**非** tsc / 软类型全集）；**细项与验收见 §13** |

---

## 13. 大型语言特性（分里程碑筹备）

下列条目均为**大工程**，实施时按 **解析（swc/AST）→ HIR → `sem` → `codegen` → 集成/单元测试** 分 PR 推进；语义须保持 **trust 硬类型**（可静态判定），与完整 `tsc` **不必**逐条等价。完成子里程碑后更新 [README.md](README.md) 语言矩阵与本节勾选。

### 13.1 泛型（函数 / 接口 / 类型别名 / 类型实参）

- [ ] **设计**：单态化、 erased、或受限策略（文档化与 README 泛型表对齐或替代）。
- [ ] **解析 + build**：`type_params`、泛型实例化边界、`TsTypeRef` 实参进入 HIR。
- [ ] **sem**：实参代入与一致性检查（硬类型下可判定子集）。
- [ ] **codegen**：单态化展开或等价 Rust 生成策略。
- [ ] **测试**：fixture + `cli_e2e` + 负例（过度宽泛的仍拒绝）。

### 13.2 高阶函数（函数作一等值、函数类型与调用）

- [ ] **设计**：捕获策略、栈闭包 vs 明确不捕获子集扩展、`fn` 类型在 HIR 中的表示。
- [ ] **HIR**：函数类型、`Callee` 扩展（含成员/变量调用路径）。
- [ ] **build + sem**：箭头函数与函数值、调用与赋值类型检查。
- [ ] **codegen**：`Fn`/`fn` 指针或生成结构体闭包（依设计）。
- [ ] **测试**：最小高阶用例 + 与现有 `nested_fn` 无捕获语义的关系说明。

### 13.3 完整 OO（`class`、`this`、构造/继承等）

- [ ] **设计**：与 Rust 映射（结构体 + impl、或显式拒绝部分 TS 语义）；`export class` 与模块交互。
- [ ] **build**：`ClassDecl`、方法、字段进入 HIR（或分阶段：仅类字段 + 方法）。
- [ ] **sem**：`this`、可见性、继承/重写（按采纳子集）。
- [ ] **codegen**：与方法分发、`super`（若纳入范围）。
- [ ] **测试**：类 fixture + 负例（不支持的修饰符仍诊断）。

### 13.4 `for..in`

- [ ] **设计**：迭代对象键的静态类型（`string` 键与 `ObjectNum` / 扩展对象模型）。
- [ ] **HIR**：`ForIn` 或 lowering 策略。
- [ ] **sem**：循环变量类型、与对象/字典表示一致。
- [ ] **codegen**：迭代 `HashMap` 键或约定运行时辅助。
- [ ] **测试**：fixture + 与 `for(;;)` 对照。

### 13.5 完整 `switch` / `case`

- [ ] **设计**：穿透、`default`、与联合/字面量收窄的配合范围（硬类型下可静态实现子集）。
- [ ] **HIR**：`IRStmt::Switch` 或等价（§2.1 曾记未实现）。
- [ ] **sem**：穷尽性（可选）、fallthrough 规则。
- [ ] **codegen**：`match` 映射与 `break` 行为。
- [ ] **测试**：替换/补充现有 `switch_fail` 正例路径 + 负例。

---

## 维护说明

- 完成一项后，将对应 `[ ]` 改为 `[x]`，或在项下追加「完成于 commit / PR #」。
- 若某项范围变化，在条目末尾用括号注明**替代方案**或**废弃原因**。
- 与 [`README.md`](README.md) 语言矩阵冲突时，以代码为准并更新 README。
