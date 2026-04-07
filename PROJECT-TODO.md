# ts2rs 项目长期 TODO 清单

本文档用于**长期跟进**编译器与工具链的演进，按主题分层列出可验收项。状态建议用 `[ ]` / `[~]` 进行中 / `[x]` 在 PR 或提交中维护。

**相关代码入口**：[`README.md`](README.md) · [`crates/ts2rs-hir`](crates/ts2rs-hir)（`build.rs` / `sem.rs` / `codegen.rs` / `ir.rs`）· [`crates/ts2rs-parser`](crates/ts2rs-parser) · [`crates/ts2rs-driver`](crates/ts2rs-driver) · [`crates/ts2rs-cli`](crates/ts2rs-cli) · [`test-ts/main.ts`](test-ts/main.ts)（多文件：`test-ts/math.ts`） · [`crates/ts2rs-cli/tests/fixtures/`](crates/ts2rs-cli/tests/fixtures/)

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
- [x] **逻辑与短路**：`&&`、`||`；`boolean` 与 `number` 真值（`!= 0`）已支持，结果类型为 `boolean`（见 `logical_bool.ts`、`logical_truthy_ok.ts`）；与 TypeScript 值保留式 `&&`/`||` 仍不同；更复杂真值或联合操作数仍受限。
- [x] **三元运算符**：`cond ? a : b`（见 `ternary_ok.ts`）。
- [x] **逗号表达式**：见 `comma_ok.ts`。
- [x] **模板字符串**：无 tag；见 `template_ok.ts`。
- [x] **数组 / 对象字面量**：受限子集已支持（`number[]`、`{ k: number }`；见 `array_ok.ts`、`object_ok.ts`）；运行时与完整类型见 §1.4 / §2.1。

**§1.3 仍待后续（原因备忘）**

- **一般 `obj.m()` / 链式方法调用**：需扩展 IR（例如 `Callee` 为成员表达式时的方法调用）、方法/函数类型与 codegen；当前 [`build.rs`](crates/ts2rs-hir/src/build.rs) 仅接受 `f(...)` 与 `console.log(...)`。
- **`?.()`（可选调用）与 `??` 的完整收窄**：与 §3.3「类型系统加深」空值/联合收窄路线一致；可选调用仍显式拒绝；`??` 为受限实现。
- **数组/对象字面量的「完整」类型**：更丰富的元素与字段类型、`TsType`/IR 演进见 §1.4、§2.1，不单属表达式扩展层。

### 1.4 类型语法（仅类型层）

**摘要**

- [x] **字面量类型**、**联合类型**、**接口**、**type 别名**：与 checker 路线图对齐（拆分为下列子项；**字面量类型**、**primitive/字面量联合**、**受限 `interface`→`ObjectNum`** 与 **受限 `type` 别名→具名表** 已见子项）。**泛型**见下列独立子项（文档化「仍拒绝」里程碑，非实现语义）。

**与已实现子集的关系**：§1.3 已支持受限注解 `number[]`、`{ k: number }`（HIR 中 [`TsType::ArrayNumber`](crates/ts2rs-hir/src/ir.rs) / [`ObjectNum`](crates/ts2rs-hir/src/ir.rs)）。**字面量类型**（`NumberLit` / `StringLit` / `BoolLit`）与 **联合类型**（[`TsType::Union`](crates/ts2rs-hir/src/ir.rs) + 规范化）已见下项；**顶层 `interface`** 在类型层等价于具名 `ObjectNum`（与对象类型字面量同一规则）；**顶层 `type` 别名**经 [`collect_named_types`](crates/ts2rs-hir/src/build.rs) 解析为既有 `TsType` 并进入同一张具名表；**泛型语义**仍未实现，拒绝对照见下列子项与 [README §1.4](README.md)；完整对象/接口形状与 IR 演进见 §2.1，空值与收窄与 §3.3 交叉。

**子项（逐项勾选）**

- [x] **字面量类型**（如 `42`、`"a"`、`true` 出现在类型位置）  
  - **依赖**：扩展 [`TsType`](crates/ts2rs-hir/src/ir.rs) 或等价表示；字面量与基类型的可赋值关系（与 §3.3「子类型 / 结构化」一致）。  
  - **验收**：[`build.rs`](crates/ts2rs-hir/src/build.rs) 解析 `TsLitType`；[`sem.rs`](crates/ts2rs-hir/src/sem.rs) `type_assignable` / 推断字面量；`literal_type_ok.ts`、`literal_type_fail.ts` + [`cli_e2e.rs`](crates/ts2rs-cli/tests/cli_e2e.rs)。

- [x] **联合类型**（`A | B`，建议先 primitive / 字面量联合再扩展）  
  - **依赖**：类型规范化与可判定相等的并集表示；与 `??` / `?.` 收窄策略对齐（§3.3）。  
  - **验收**：受限联合下的赋值与分支可给出一致诊断或生成；集成测试覆盖典型路径（`union_literal_ok`、`union_cond_ok`、负例 `union_heterogeneous_fail`、`intersection_type_fail`、`union_mixed_cond_fail`）。

- [x] **`interface` 与对象类型**（声明体、可选属性、`extends` 等按阶段）  
  - **依赖**：结构化类型进入 IR（§2.1）；与现有 `ObjectNum` 子集的关系在实现 PR 中写清（兼容或迁移路径）。  
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
- [x] **表达式**：已含 `LogicalAnd` / `LogicalOr`、`Conditional`（三元）、`Seq`（逗号）、`Tpl`（模板）、受限 `Member`；`Index` 与完整成员链待扩展。由 §1.3 引入的数组下标与 `ObjectNum` 字段访问已覆盖；**完整** `interface` / 结构化对象类型见 §1.4 与后续 IR 扩展。
- [x] **顶层**：多文件模块图 — `parse_module_graph` + `validate_imports`；HIR 合并为 [`IRModule`](crates/ts2rs-hir/src/ir.rs)（`build_program_multi` / `compile_graph`）；`main` 须在入口文件；全局函数名唯一；负例见 `import_missing_export_*`、`circular_*`、`dup_*` fixtures。

### 2.2 元数据与调试

- [x] **Span**：HIR 节点带 swc `Span`；[`diag`](crates/ts2rs-hir/src/error.rs) 与 codegen 错误均用所属函数的 `cm` + `source_path` + 节点 `span`（见 [`ir.rs`](crates/ts2rs-hir/src/ir.rs) 模块注释）；`build` 无顶层函数时用整文件 `span`；`sem` 缺 `main` 时锚点为第一函数 `span`。
- [x] **可选**：[`IRFunction::ir_id`](crates/ts2rs-hir/src/ir.rs)（函数级，含嵌套 `function`；单次编译内单调递增，见 `build_fn`）。

---

## 3. 语义分析（`sem.rs`）

### 3.1 已实现的巩固

- [x] **符号表**：块作用域与 `let` 重复绑定（已部分实现）— 增加用例覆盖边界（嵌套块、与参数同名等）。验收：`let_dup_same_block_fail.ts`、`let_shadow_nested_ok.ts`、`param_let_same_name_fail.ts` + [`cli_e2e.rs`](crates/ts2rs-cli/tests/cli_e2e.rs)。
- [x] **控制流**：`stmts_return` 简化规则— 文档化并与 TypeScript 差异列表对照（见 README）。验收：[README.md「控制流与 return（简化语义）」](README.md)。
- [x] **`void` 与 `console.log`**：`BuiltinLog` 为 void 表达式路径已覆盖；补充「仅 log 的表达式语句」在分支中的用例。验收：`void_log_in_branch.ts` + e2e。

### 3.2 可变性与赋值

- [x] **`let` 可变**：`IRStmt::Assign`；语义检查 LHS 为已绑定标识符。
- [x] **禁止对 `const` 赋值**。

### 3.3 类型系统加深

- [x] **与 §1.4 的衔接**：字面量类型、联合类型与 `??` / `?.` 收窄应在实现时与 §1.4 子项对齐（避免与当前受限 `TsType` 语义冲突）。联合类型已入 HIR；`??` / `?.` 的**完整** discriminated / 空值收窄仍待后续。验收：[README「语义与类型路线（§3.3）」](README.md) 已写明衔接关系与 `nullish_ok` / `optional_ok` 受限子集。
- [x] **`null` / `undefined`**：若支持，与 `strictNullChecks` 策略。验收：README §3.3 已说明当前无 `strictNullChecks` 等价开关及后续若对齐 `tsc` 的策略方向；**未**在本里程碑实现该开关。
- [x] **结构类型 vs 名义类型**：与 Rust 后端映射策略（当前以基础类型为主）。验收：README §3.3 已说明具名表/语义检查与 Rust 生成侧边界；**未**实现 TS 结构子类型全集。
- [x] **函数类型**：高阶函数（函数作值）需 IR + codegen。验收：README §3.3 已说明仅 `function` 声明与调用、无函数作一等值；**未**扩展 IR/codegen 支持高阶函数。

### 3.4 控制流分析（进阶）

- [ ] **可达性**：不可达代码警告。
- [ ] **明确赋值**：`let x: number;` 后使用（若放宽「必须初始化」）。
- [ ] **更精确的 return**：与 `switch`/多分支统一。

---

## 4. 代码生成（`codegen.rs`）

### 4.1 当前行为改进

- [ ] **`console.log` 多参数格式**：当前为连续 `{}` 无分隔符（见 [`emit_builtin_log`](crates/ts2rs-hir/src/codegen.rs)）；可选改为空格或 `Debug` 列表以贴近 TS 观感。
- [ ] **整数除法**：TS `/` 与 Rust 整数除法对齐；文档说明截断行为。
- [ ] **溢出**：`i32` 边界与 TS `number` 差异说明或运行时检查（可选 feature）。

### 4.2 新特性映射

- [ ] **赋值**：`let mut` 与块作用域对齐 Rust。
- [ ] **字符串**：继续 `String` + `format!`；大字符串与性能另议。
- [ ] **堆对象 / GC**：若引入引用类型，需 `Rc`/`Arc` 策略或明确不支持。

### 4.3 生成代码可读性

- [ ] **缩进与换行**：稳定 rustfmt 友好输出。
- [ ] **注释**：可选注入原始 TS span 注释，便于调试。

---

## 5. 内建与标准库映射

### 5.1 `console`

- [ ] **`console.log`**：已映射 `println!`；评估 `console.error`、`console.debug`。
- [ ] **格式化语义**：与第 4.1 节一致。

### 5.2 最小运行时（[`ts2rs_rt`](crates/ts2rs_rt)）

- [ ] **字符串操作**：`length`、索引（UTF-8 边界语义）。
- [ ] **数学**：`Math.*` 子集到 `std`/`libm`。
- [ ] **I/O**：与 `std::io` 的受控封装（若范围允许）。

---

## 6. Driver 与构建（[`ts2rs-driver`](crates/ts2rs-driver)）

### 6.1 单文件路径

- [ ] **临时目录生命周期**：API 文档化（已有 `TempDir` + 可执行路径）。
- [ ] **离线 / 无 cargo 环境**：明确错误提示。

### 6.2 多文件与模块（[`compile_entrypoint_to_executable`](crates/ts2rs-driver/src/lib.rs)）

- [ ] **解析多入口**：CLI 接收多 `.ts` 或 `tsconfig` 式列表。
- [x] **依赖图（子集）**：入口文件 + 相对 `import` → [`parse_module_graph`](crates/ts2rs-parser/src/module_graph.rs)（保留各模块 AST）→ `validate_imports` → [`lower_module_graph`](crates/ts2rs-lower/src/lib.rs) → 单 Rust crate。
- [ ] **`Cargo.toml` 生成**：依赖 `ts2rs_rt` 可选 feature。
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

- [ ] **README**：与实现同步更新矩阵；「不支持的 TS 特性」简表。
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
| P3     | 泛型、完整类型系统                      | 长期                                      |

---

## 维护说明

- 完成一项后，将对应 `[ ]` 改为 `[x]`，或在项下追加「完成于 commit / PR #」。
- 若某项范围变化，在条目末尾用括号注明**替代方案**或**废弃原因**。
- 与 [`README.md`](README.md) 语言矩阵冲突时，以代码为准并更新 README。
