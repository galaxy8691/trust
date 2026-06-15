# Phase 2 — 修正 Phase 1 对齐 v2.0 设计 TODO

> **目标：** 在不重写编译管线骨架的前提下，把 Phase 1 实现修正到 v2.0 设计。
> **分支：** `phase2-v2-align`
> **基准设计：** `docs/Trust-设计文档.md` v2.0（唯一权威规范）
> **期限：** 5–6 周
> **优先级：** P0（阻塞 Phase 3+）
> **前置：** Phase 1 ✅ 完成  
> **承接追踪：** 各 spec 中的延期/承接项统一登记于 [`docs/phases/DEFERRED-AND-HANDOFFS.md`](DEFERRED-AND-HANDOFFS.md)（新增延期时请同步更新）。

---

## 背景

Phase 1 在 v2.0 设计重构前交付，基于旧设计实现。本 Phase 的使命：
1. **移除旧设计残留**（`loop`/`bigint`/`interface`/`impl`/`select` 等）
2. **修正现存实现到 v2.0 语义**（`number`=f64、函数声明规则、`&mut`/闭包调用）
3. **重核关键字表**（54→43）
4. **语言规范对齐**（`spec/trust-spec.md` 随实现增量修正，非一次性整篇重写）
5. **标准库规范对齐**（`spec/stdlib.md` 随实现增量修正，与语言规范同步推进）
6. **测试迁移**（56 集成测试在 v2.0 语义下重新通过）

### 规范与标准库对齐策略（v2.0）

Phase 0 产出的 `spec/trust-spec.md` 与 `spec/stdlib.md` 基于**旧设计**并已审计。v2.0 对齐采用**活文档**策略——随各子里程碑删旧、写新，不在 Phase 2 内一次性整篇重写。

| 文档 | 2.1 | 2.2 | 2.3 | Phase 3+ |
|------|-----|-----|-----|----------|
| `spec/trust-spec.md` | 删废弃条目；关键字表 43；废止旧审计；章节冻结矩阵 | `number`=f64、整数语义、位运算 | 函数声明规则 | receiver、具名类型、`unknown`+`match` 等随实现补齐 |
| `spec/stdlib.md` | 移除 `std::result` 用户面 API；更新模块依赖图；清除 Option/Result 暴露 | `number` 相关 API 参数/返回值 | — | `std::error`（Phase 4）、`throws` 签名（Phase 4）、并发模块（Phase 5+） |

**交叉核对：** 2.1 / 2.2 / 2.3 各完成时已记录（`known-failures.md`、`2.2/cross-check.md`、`2.3/cross-check.md`）；Phase 2 收尾时做总核对。

---

## 2.1 移除已废弃的语法与类型 + 规范对齐 v2.0

**产出物：** 清理后的 parser/HIR/TIR/codegen + 关键字表重核（54→43）+ `trust-spec` / `stdlib` 初步对齐  
**工作量：** 1.5–2 周  
**优先级：** P0  
**依赖：** Phase 1 完成  
**分支：** `phase2-v2-align`

### 2.1.1 关键字表重核（lexer）✅

**涉及 crate：** `trust_parser`（`crates/trust_parser/src/lexer.rs`）

- [x] **移除 16 个关键字：**
  - `loop` → 用 `while (true)` 替代（设计 §6.2）
  - `bigint` → `number`=f64 足够（设计 §14）
  - `interface` / `impl` → 纯结构类型 + Go 风格 receiver（设计 §14）
  - `select` → 多通道竞速取消（设计 §14）
  - `undefined` → 只有 `null`（设计 §2.2）
  - `None` / `Some` → `Option` 不暴露给用户（设计 §14）
  - `Ok` / `Err` → `Result` 不暴露给用户（设计 §14）
  - `Rc` / `Arc` / `Weak` / `Box` → 用户不接触底层 Rust 类型（设计 §3.7）
  - `dyn` → 禁止动态分发（设计 §14）
  - `extends` → 无 `<T extends ...>` 语法（设计 §2.5）
- [x] **净新增 5 个关键字（以下均为"仅关键字预留"，表达式/语句实现归后续 Phase）：**
  - `unknown` → 仅关键字预留，类型/表达式实现归 Phase 3（设计 §2.6）
  - `try` / `catch` → 仅关键字预留，语句实现归 Phase 4（设计 §5.1）
  - `null` → 唯一空值（设计 §2.7）。**注意：** 移除 `None_` 后需将 `null` 关键字映射到 `Expr::Null`（取代旧 `None_→Expr::Null` 路径）
  - `panic` → 仅关键字预留，`panic!("msg")` 表达式实现归 Phase 4（设计 §5.2）
- [x] **确认已存在无需操作：** `type` / `match` / `throw` / `shared` / `spawn`（均已存在于当前 lexer）
- [x] 更新 `static KEYWORDS` 映射表（54→43），每项附带 `// 设计 §X.X` 来源注释
- [x] 更新 lexer 文件头注释：`//! 54 个关键字` → `//! 43 个关键字`
- [x] 更新 `TokenKind` 枚举——移除上述 16 个废弃变体（含 `BigIntLiteral`），新增 5 个变体

### 2.1.2 移除 `loop`（端到端）✅

**涉及 crate：** `trust_parser`、`trust_hir`、`trust_tir`、`trust_codegen`

- [x] **lexer：** 移除 `TokenKind::Loop` 变体（由 2.1.1 覆盖）
- [x] **parser/ast：** 删除 `Stmt::Loop` 与 `Expr::LoopExpr` AST 节点
- [x] **parser：** 移除 `loop { ... }` 解析路径（含 `break` 带值在 `LoopExpr` 中的处理）
- [x] **parser/ast：** 删除/禁用 `BreakStmt.value` 字段（`loop` 移除后 `break value` 失去合法语境）
- [x] **HIR：** 移除 `HirLoop` / `HirStmt::Loop` / `HirExpr::Loop` 及 `LoopExpr` 降级逻辑；删除 `infer_loop_type`；`while (true)` 已在 Phase 1 支持，无需新增
- [x] **TIR：** 移除 `Loop` 对应的 TIR 节点与 borrowck 路径
- [x] **codegen：** 移除 `loop` → Rust 代码生成分支
- [x] **验证：** `grep -r "Loop" crates/trust_parser/src/ crates/trust_hir/src/ crates/trust_tir/src/ crates/trust_codegen/src/` 确认非注释引用已全部清理

### 2.1.3 移除 `bigint` ✅

**涉及 crate：** `trust_parser`、`trust_hir`、`trust_tir`、`trust_codegen`

- [x] **lexer：** 移除 `TokenKind::BigIntType` 变体 + i64 字面量相关 token（由 2.1.1 覆盖）
- [x] **parser/ast：** 删除 `Type::BigIntType` 与 `Expr::BigIntLiteral` AST 节点
- [x] **parser：** 移除 `bigint` 类型标注与 i64 字面量解析路径
- [x] **HIR typeck：** 移除 `BigInt` 类型检查规则
- [x] **codegen：** 移除 `bigint` → Rust `i64` 映射
- [x] **验证：** `grep -ri "bigint" crates/` 确认仅剩注释/文档

### 2.1.4 移除 `interface` / `impl` 关键字 ✅

**涉及 crate：** `trust_parser`

- [x] **lexer：** 移除 `TokenKind::Interface` / `TokenKind::Impl`（由 2.1.1 覆盖）
- [x] **parser：** 移除 parser 中 `interface`/`impl` 关键字同步点（Phase 1 未实现语义，仅清理残留）
- [x] **验证：** `grep -r "interface\|impl" crates/trust_parser/src/` 确认仅剩注释

### 2.1.5 移除 `select` 预留 ✅

**涉及 crate：** `trust_parser`

- [x] **lexer：** 移除 `TokenKind::Select`（由 2.1.1 覆盖）
- [x] **parser/ast：** 删除 AST 中 `select` 转义槽
- [x] **验证：** `grep -ri "select" crates/trust_parser/src/` 确认仅剩注释

### 2.1.6 移除其余旧设计残留关键字 + 更新辅助函数 + 旧后缀运算符 ✅

**涉及 crate：** `trust_parser`、`trust_hir`、`trust_tir`、`trust_codegen`、`trust_error`（若有）

- [x] **更新 `TokenKind::can_end_stmt`**（`lexer.rs`）：移除 `None_` / `BigIntLiteral` 引用，替换为 `null` 等新 token（**保留 `Bang`**——仅前缀逻辑非 `!x`）
- [x] **更新 `Parser::can_expr_start`**（`parser.rs`）：移除 `None_` / `BigIntLiteral` / `Loop` 引用，替换为 `null` 等新 token（**保留 `Bang`**）
- [x] **移除旧后缀运算符：** 删除 `Expr::AssertUnwrap` / `Expr::TryPropagate` 及 parser 后缀 `expr!` / `expr?` 分支（**保留** `?.` / `??` 与前缀 `!x`）
- [x] **HIR/TIR/Codegen：** 删除 `HirExpr::AssertUnwrap` / `HirExpr::TryPropagate` 降级路径
- [x] **验证残留引用：** `grep -rni "tokenkind::undefined\|None_\|Some_\|Ok_\|Err_\|Rc\|Arc\|Weak\|Box_\|Dyn\|Extends\|BigIntLiteral\|AssertUnwrap\|TryPropagate" crates/` 确认所有引用已清理（注释除外）
- [x] 若 HIR/codegen 中有对用户暴露的 `Option`/`Result` 类型翻译，一并移除

### 2.1.7 语言规范对齐 v2.0（`trust-spec.md`，随实现增量推进）✅

**产出物：** `spec/trust-spec.md` 中已废弃条目清除 + 前瞻规范条目 + 章节冻结矩阵

- [x] **删除已废弃规范条目：**
  - `interface` / `impl` 词法+语法+语义条目
  - ADT（`type X = | ...`）语法+语义条目
  - 旧 `Option` / `Result` 用户暴露语义、**后缀** `expr?`（Result 传播）、**后缀** `expr!`（Option 断言）条目
  - `select` 并发条目
  - `loop` / `bigint` 词法+语法+语义条目
- [x] **重写 LEX-REQ-001 关键字表**为 43 个（与 lexer 一致），更新字面量说明（5 种，无 `bigint`/`Nn`）
- [x] **前瞻同步**（写入 spec，实现归 2.2/2.3，不要求 2.1 实现）：
  - `number`=f64 类型规则（TYP-REQ-001 重写，对应 2.2）
  - `number` 整数语义（循环 + 2^53 占位，对应 2.2；索引/长度归 Phase 6）
  - `number` 位运算约束（`&`/`|`/`^`/`<<`/`>>` 仅允许 `number`，对应 2.2）
  - 块体函数强制返回标注规则（SYN-REQ-002 更新，对应 2.3）
  - 表达式体函数（`function f(...) = expr`）语法（SYN-REQ-002 更新，对应 2.3）
- [x] **不在 2.1 写入：** 具名类型、receiver、隐式泛型、`unknown`+`match`、`throw`/`try-catch` 穷举、完整 `null` 安全——归 Phase 3+（见上表）
- [x] **废止旧审计：** 在 `docs/phases/0/0.3/audit-report.md` 顶部添加废止声明：
  ```
  > ⚠️ 本审计报告基于旧设计（pre-v2.0），已被 v2.0 设计取代。
  > 请以 `docs/Trust-设计文档.md` v2.0 为唯一权威规范。
  > v2.0 重新审计随 Phase 2+ 逐 Phase 推进。
  ```
- [x] **章节冻结矩阵**（记入 spec 前言或 `2.1-spec.md`）：
  | 规范章节 | 冻结时机 | 对应实现 |
  |---------|---------|---------|
  | 词法规范（关键字集、字面量） | 2.1 完成前 | 2.1.1 关键字重核 |
  | 类型系统核心（`number`/基本类型） | 2.2/2.3 启动前 | 2.2 number=f64 |
  | 函数声明规则 | 2.3 启动前 | 2.3 |
  | 标准库模块大纲 | 2.1 骨架对齐后逐 Phase 冻结 | 2.1.8 + Phase 4/5/6 |
  | 具名类型/泛型/`unknown` | Phase 3 各子任务启动前 | Phase 3 |
  | 错误/`null` | Phase 4 各子任务启动前 | Phase 4 |
  | 并发/FFI | Phase 5/7 启动前 | Phase 5/7 |
- [x] **验证：** 2.1 完成时四文档交叉核对（设计文档 / `trust-spec` / `stdlib` / `design-constraints`）

### 2.1.8 标准库规范对齐 v2.0（`stdlib.md`，2.1 骨架修正）✅

**产出物：** `spec/stdlib.md` 移除旧设计用户面 API，模块大纲对齐设计 §13 骨架

- [x] **删除/废止 `std::result` 模块**（`Option`/`Result` 不暴露给用户；`Some`/`None`/`Ok`/`Err` 构造器从 stdlib 移除）
- [x] **更新模块依赖图**：去除以 `result` 为根的依赖；对齐设计 §13 模块列表（`error`、`console`、`collections`、`string`、`sync`、`async`、`fs`、`time` 等）
- [x] **清除 stdlib 设计决策中的旧语义**：删除「`!` 仅限 Option」「`??` 用于 Result」等 pre-v2.0 表述
- [x] **API 签名过渡标注**：仍含 `Result<T,E>` / `Option<T>` 的函数（如 `fs.readToString`）标注 `> ⚠️ v2.0 待改 — Phase 4 改为 `throws` 或 `T | null``，不在 2.1 一次性改完全部 API
- [x] **新增骨架模块占位**（仅模块标题 + 对齐设计 §13 的一句话说明，API 随 Phase 4+ 补齐）：`std::error`、`std::console`
- [x] **验证：** `grep -ri 'Option::\|Result::\|std::result' spec/stdlib.md` 仅剩废弃标注或过渡标注；模块列表与设计 §13 一致

---

## 2.2 `number` 统一为 f64

**产出物：** HIR typeck 规则更新 + codegen 映射修改 + 整数语义（循环/2^53 占位；索引/length 归 Phase 6）+ 位运算约束  
**工作量：** 1–1.5 周  
**优先级：** P0  
**依赖：** 2.1（关键字移除完成后，避免旧类型残留干扰）

### 2.2.1 类型统一（HIR typeck）✅

**涉及 crate：** `trust_hir`

- [x] 删除 i32 / f64 类型区分——`Type::Number` 统一为单一 f64 类型
- [x] 删除 `i32 + f64 → error` 类型不匹配规则
- [x] `number` 之间运算（`+`/`-`/`*`/`/`/`%`/`**`）自由通过，不报类型错误
- [x] 字面量类型推断：`404` → `number`(f64)（当前推断为 i32，需修正）
- [x] 验证：`cargo test -p trust_hir` 中与 number 类型相关的测试用例更新通过

### 2.2.2 codegen 映射 ✅

**涉及 crate：** `trust_codegen`

- [x] `Type::Number` → Rust `f64`（替换现有 i32 映射）
- [x] 字面量生成 f64 后缀：整数字面量 `404` → `404.0_f64`，浮点字面量 `3.14` → `3.14_f64`
- [x] 二元运算生成：`a + b` 保持 `a + b`（f64 原生运算，无需转换）
- [x] 更新所有 codegen 快照（`.snap` 文件中的 `i32` → `f64`）
- [x] 验证：`cargo test -p trust_codegen` 快照测试更新通过

### 2.2.3 `as` 收敛 ✅

**涉及 crate：** `trust_parser`、`trust_hir`

- [x] `number` 之间移除 `as` 转换需求（设计 §2.2："`number` 之间可以自由运算，不需要 `as` 转换"）
- [x] 移除 parser 中 `as number` / `as f64` / `as i32` 的数字转换解析路径
- [x] `as` 仅保留用于非 `number` 的必要转换（如 `unknown`→具体类型）
- [x] 验证：`cargo test -p trust_parser` 中 as 相关测试用例更新通过

### 2.2.4 整数语义

**涉及 crate：** `trust_hir`、`trust_codegen`

- [ ] **数组索引：** `arr[n]` → 语言尚不支持索引语法，延期 Phase 6（集合类型落地时一并实现 `as usize` 转换）
- [x] **循环计数：** `for (let i = 0; i < N; i = i + 1)` — 迭代变量 `i` 类型为 `number`(f64)。2.2 不实现 `i++`/`+=` 语法（归 Phase 3）
- [ ] **长度/容量：** `.length` — `MemberAccess` 仅支持 `console.log`，延期 Phase 6
- [x] **FFI 整数（Phase 2 仅建立默认映射）：** `number` → Rust `f64` 默认映射。具体 FFI 整数转换机制待 Phase 7
- [x] **超 2^53 精度警告：** 字面量范围检查已落地（`DiagError` 占位，完整 `Warning+Help` 待 `trust_error::Diagnostic` 扩展）
- [ ] 验证：e2e 测试延期 Phase 6（依赖索引/length 语法）；循环计数已在现有 for 测试中覆盖

### 2.2.5 位运算（新增 token/AST/parser/typeck/codegen 完整路径）

**涉及 crate：** `trust_parser`、`trust_hir`、`trust_codegen`

> **背景：** 设计 §2.2 要求位运算 `&`/`|`/`^`/`<<`/`>>` 仅允许 `number`。当前 `&` 仅用于 `Expr::Reference`（`TokenKind::Amp`），`|` 退化为 `Ident`，`^`/`<<`/`>>` 无对应 token。需新增完整路径。

- [x] **lexer token 新增：**
  - `|` → 从当前退化为 `Ident` 改为识别为 `TokenKind::Pipe`
  - `&` → 在 parser 中按上下文区分 `TokenKind::Amp`（前缀 `&x` = Reference）与 `TokenKind::BitAnd`（中缀 `a & b`）
  - `^` → 新增 `TokenKind::Caret`
  - `<<` → 新增 `TokenKind::Shl`
  - `>>` → 新增 `TokenKind::Shr`
- [x] **AST：** `BinOp` 枚举新增 `BitAnd` / `BitOr` / `BitXor` / `Shl` / `Shr` 变体
- [x] **parser：** 在 `parse_binary` 中新增位运算中缀解析（运算符优先级：`<<`/`>>` > `&` > `^` > `|`）
- [x] **HIR typeck：** 位运算操作数类型检查——仅允许 `number`
- [x] **codegen：** Rust `f64` 不支持位运算——生成 `f64::to_bits()`→`u64`→位运算→`f64::from_bits()` 转换链，并附加注释 `/* bitwise on f64: behavior per IEEE 754 */`
- [x] **验证：** `cargo test -p trust_parser` 位运算解析 + `cargo test -p trust_hir` 位运算类型检查 + 端到端测试（在 2.5 中集成）

---

## 2.3 函数声明规则对齐 ✅

> **状态：** 里程碑已完成（2026-07）。MS-2.3-1～8 验收通过；详见 `docs/phases/2/2.3/2.3-spec.md` §6、`cross-check.md`。

**产出物：** parser 表达式体 + name_res 块体标注 + typeck 表达式体/箭头推断 + `trust-spec` 冻结同步  
**工作量：** 1 周  
**优先级：** P0  
**依赖：** 2.1（关键字重核完成，避免语法歧义）

### 2.3.1 块体函数强制返回标注 ✅

**涉及 crate：** `trust_parser`、`trust_hir`（name_res）

- [x] **parser：** 解析 `function f(...) { ... }` 时，若函数签名无 `: ReturnType` → **不在 parser 报错**（与 2.3-spec 一致），由 name_res `lower_function` 统一检查
- [x] **HIR name_res：**
  - `function f(...) { ... }` 无返回类型标注且非表达式体 → **编译错误**
  - 错误信息：`"块体函数必须显式标注返回类型。无返回值时使用 :void"`（`trust_error::ErrorCode::E0062` 登记；诊断消息占位同 2.3-spec）
  - `function f(...): void { ... }` → 合法
  - `function main(): void { ... }` → 合法
  - `export function f() { ... }` → 经 `lower_exports`/`lower_function` 报错，并纳入 `items` 供 typeck
- [x] 验证：`cargo test --workspace` 通过；name_res `lower_function` 含块体标注检查

### 2.3.2 表达式体函数 ✅

**涉及 crate：** `trust_parser`、`trust_hir`（typeck）

> **现状：** parser 已支持 `function f(...) = expr`（`snap_fn_single`）。2.3 增加 `is_expression_body` 标记 + typeck 推断。

- [x] **parser：** 确认表达式体路径；`is_expression_body: true` 已设置
- [ ] **parser 边界 e2e 夹具**（嵌套表达式体、多场景模板字符串体、箭头作表达式体）— 核心路径已由 `trust_hir` 集成测试覆盖（`expr_body_infer_string` 等）；**trustc 端到端夹具归 §2.5.3**
- [x] **HIR typeck：** 表达式体无标注时由 `infer_return_type` 推断；有标注时校验一致
- [x] 验证：`cargo test --workspace` 通过；`check_function` 含 `is_expression_body` 推断分支

### 2.3.3 箭头函数返回类型推断与标注 ✅

**涉及 crate：** `trust_parser`、`trust_hir`（name_res、typeck）

- [x] **parser：** 扩展箭头语法——`(param_list) (: ReturnType)? => body`（`try_parse_arrow_params`；设计 §4.1：`(x: number): number => x * 2`）；`LParen` 和 `Move` 路径均已支持
- [x] **name_res：** 有返回标注时写入 `ArrowFn.ret`（`HirType::from_ast_type`）；无标注保持 `Error` 哨兵供推断
- [x] **typeck：** `(x) => expr` 推断返回类型（已有逻辑）；有 `: ReturnType` 时标注优先
- [x] **延期：** `(name) => expr` 参数类型从上下文推断 → Phase 3 隐式泛型（H-P3-06），已在 DEFERRED-AND-HANDOFFS 登记为 H-P3-07a
- [x] 验证：`snap_arrow_typed_return` parser 快照通过；`arrow_*` typeck 测试归 2.5 集成覆盖

---

## 2.4 承接 Phase 1 遗留项

**产出物：** `&mut x` 可变引用 + 闭包调用 `r()` + JSON→serde 评估  
**工作量：** 1 周  
**优先级：** P0（交付标准强制要求 `&mut`/闭包调用可用，故为阻塞项）  
**依赖：** 2.1

### 2.4.1 #7 可变引用 `&mut x`

**涉及 crate：** `trust_parser`、`trust_tir`

- [ ] **parser：**
  - Phase 1 已支持 `let mut`，补齐 `&mut x` 表达式解析
  - `&mut x` → AST 节点 `Expr::RefMut(Box<Expr>)`
  - `&x` → 已有 `Expr::Ref`，确认与设计 §3.5 一致
- [ ] **TIR borrowck：**
  - `&mut x` → 可变借用路径：检查 `x` 是否已存在活跃可变借用或只读借用
  - 错误信息：已有活跃借用时输出 borrowck 错误（含修复建议——如缩小作用域或 clone）
  - `&x` → 只读借用路径（Phase 1 可能已部分支持）
- [ ] 验证：添加端到端测试——`&mut` 在 borrowck 正确场景通过，冲突场景报错

### 2.4.2 #8 闭包调用 `r()`

**涉及 crate：** `trust_hir`（name_res）、`trust_tir`

- [ ] **HIR name_res：**
  - 箭头函数绑定 → 保留为 `ArrowFn`（Phase 1 可能仅支持声明不支持调用）
  - `let f = (x) => x + 1; f(5)` → name_res 将 `f` 解析为闭包类型，调用处生成 `Call(Ident("f"), args)`
- [ ] **TIR：**
  - 利用现有 `TirFunction` 结构，新增 `captures: Vec<Capture>` 字段表示闭包捕获（与现有 `TirFunction`/`TirOp`/`TirValue` 架构对齐）
  - 捕获分析（与 §3.4 一致）：默认只读借用 + `move` 闭包
  - 闭包调用 → 编译为闭包体的内联/函数调用
- [ ] **为 Phase 3 打好基础：** 闭包类型推断机制与隐式泛型共享——确保 `TirFunction.captures` 设计可扩展至泛型闭包
- [ ] 验证：添加端到端测试——闭包定义+调用通过编译并正确执行

### 2.4.3 #10 JSON→serde 迁移评估

**涉及 crate：** `trust_error`

- [ ] **评估：** `trust_error` 的 JSON 输出（`--error-format=json`）当前是否用 `serde` 还是手写 JSON
- [ ] **决策：**
  - 若手写 JSON：评估引入 `serde` + `serde_json` 的成本（二进制大小增量、编译时间）
  - 若已用 serde：确认版本与 features 合理
- [ ] **原则：** 零依赖策略——若 serde 增量 < 5% 编译时间且二进制增量 < 200KB，可引入；否则坚持手写
- [ ] 产出物：1 页评估文档 `docs/phases/2/2.4/serde-evaluation.md`

---

## 2.5 测试与夹具迁移

**产出物：** 56 集成测试在 v2.0 语义下重新全部通过 + 快照更新  
**工作量：** 0.5 周  
**优先级：** P0  
**依赖：** 2.1, 2.2, 2.3, 2.4（`&mut`/闭包调用的 e2e 测试依赖 2.4 完成）

### 2.5.1 移除废弃特性的测试夹具

- [ ] 移除/改写依赖 `loop` 的夹具（如 `loop_break.trust`）
- [ ] 移除/改写依赖 `bigint` 的夹具（如 `bigint_literal.trust`）
- [ ] 移除/改写依赖 `i32`-`f64` 类型区分的夹具（如混合运算报错测试）
- [ ] 移除/改写依赖 `as` 数字转换的夹具
- [ ] 若有 `interface`/`impl`/`select` 相关测试夹具，移除

### 2.5.2 更新 v2.0 语义的快照

- [ ] **codegen 测试预期输出：** 检查 `assert_compiles!` / `assert_output!` 宏中的预期 Rust 代码文本，将 `i32` → `f64`（字面量后缀、类型标注、函数签名）
- [ ] **HIR typeck 测试：** 类型推断结果从 i32/f64 → `number`(f64)
- [ ] **错误信息测试：** 移除 `loop`/`bigint` 相关错误信息，新增块体函数无返回标注等新错误预期

### 2.5.3 端到端验证

- [ ] 47 个 `.trust` 夹具全部重新编译通过
- [ ] 56 个集成测试（46 e2e + 10 CLI）全部通过（与 `crates/trustc/tests/integration.rs` 中 `#[test]` 数量一致）
- [ ] `cargo test --workspace` 零失败
- [ ] 新增 v2.0 语义端到端测试：
  - `number`=f64 运算（整数+浮点混合）
  - 块体函数 `:void` 返回标注（含 `export function` 无标注报错）
  - 表达式体函数 `function square(x) = x * x`
  - 表达式体边界：嵌套表达式、模板字符串、箭头函数作表达式体（承接 2.3 §2.3.2 延期项）
  - `&mut` 可变引用（正确+冲突场景）
  - 闭包调用（简单闭包 + move 闭包）

### 2.5.4 CI 验证

- [ ] `cargo clippy --workspace -- -D warnings` 通过
- [ ] `cargo fmt --check --all` 通过
- [ ] `grep -r "unsafe" crates/trust_parser crates/trust_hir crates/trust_tir` 结果为空（P0）
- [ ] `cargo test --workspace` 通过

---

## 2.6 Phase 1 下沉的工程项

**优先级：** P1（非阻塞 Phase 3，但应在 Phase 2 收尾）  
**依赖：** 2.1

### 2.6.1 Trust.toml 解析与 Cargo.toml 桥接（原 1.7.2）

**涉及 crate：** `trustc`

- [ ] `Trust.toml` 配置读取（TOML 解析）
  - `[runtime]` 节：`async = "tokio"` 等（设计 §8.1）
  - `[dependencies]` → 最终映射到 Cargo.toml `[dependencies]`
  - `[trust-dependencies]` → 远期（Phase 8），Phase 2 仅占位
- [ ] 桥接生成 `Cargo.toml`：从 `Trust.toml` + 编译器内建模板生成
- [ ] 验证：`trustc compile --project` 自动检测 `Trust.toml` 并生成/更新 `Cargo.toml`

### 2.6.2 CI 性能回归监控（原 1.8.3）

- [ ] CI job: `cargo bench --bench compile_bench` 运行（criterion 基准比较）
- [ ] 基准记录在 `benches/BASELINE.md`，`±10%` 视为回归
- [ ] 5000 行基准：准备一个 5000 行的 `.trust` 合成输入文件作为编译基准

### 2.6.3 Fuzz 语料库初始化（原 1.8.4）

- [ ] 从集成测试的 `.trust` 文件初始化 fuzz 语料库
- [ ] 语料库路径：`fuzz/corpus/parse/`、`fuzz/corpus/tir_borrowck/`、`fuzz/corpus/codegen/`
- [ ] 验证：`cargo fuzz list` 列出目标，`cargo fuzz run parse -- -max_total_time=30` 无 panic

### 2.6.4 代码覆盖率门控（可选 P2）

> ⚠️ 不在 ROADMAP Phase 2 范围，属新增可选任务。若时间允许则设基线；否则押后 Phase 3。

**涉及 crate：** 全 workspace

- [ ] 设定 Phase 2 覆盖率基线目标（承接 Phase 1 的 68.99%）：若时间允许则记录新基线到 `benches/BASELINE.md`；不设硬性门控
  - `trust_parser`：≥ 90%（Phase 1: 89%+）
  - `trust_error`：≥ 95%（Phase 1: 95%+）
  - `trust_hir`：≥ 65%（Phase 1: 60.70%）
  - `trust_tir`：≥ 55%（Phase 1: 48.65%）
  - `trust_codegen`：≥ 50%（Phase 1: 46.15%）
- [ ] CI job: `cargo tarpaulin --workspace --fail-under 70`（总覆盖目标）
- [ ] 若未达标：记录差距（非阻塞），在后续 Phase 补齐

---

## Phase 2 交付标准

> 子里程碑进度：2.1 ✅ · 2.2 ✅ · 2.3 ✅ · 2.4 🔜 · 2.5 🔜 · 2.6 🔜

- [ ] `number`=f64，整数/浮点自由运算（2.2 核心已实现；2.5 全量 e2e 验收）
- [x] 关键字表 43 个（移除 16 + 新增 5）—— 2.1
- [ ] 无 `loop` / `bigint` / `interface` / `impl` / `select` / `undefined` / `Option` / `Result` / `Box` / `dyn` 等旧设计残留（2.1 核心已清；2.5 夹具扫描）
- [x] 块体函数强制返回类型标注（含 `:void`）—— **2.3 已实现**（`lower_function` + export）；2.5 trustc e2e 全量验收
- [x] 表达式体函数（`function f(...) = expr`）可用 —— **2.3**
- [ ] `&mut x` 可变引用可用（parser + borrowck）—— 2.4
- [ ] 闭包调用 `r()` 可用（name_res + TIR）—— 2.4
- [ ] `as` 仅保留非 number 的必要转换 —— 2.2（typeck 已拒 number `as`；2.5 夹具验收）
- [x] 位运算 token/AST/parser/typeck/codegen 完整落地 —— **2.2**
- [ ] 超 2^53 字面量/索引发 `Warning` 级诊断（2.2 `DiagError` 占位；正式 Warning + 索引归 Phase 6）
- [ ] `spec/trust-spec.md`：废弃条目已删除，LEX-REQ-001 43 关键字 —— 2.1 ✅；2.2/2.3 条目已冻结 ✅；全文废弃清理随 Phase 3+
- [ ] `spec/stdlib.md`：无用户面 `Option`/`Result`/`std::result`；模块大纲对齐；`Result` 过渡注记 —— 2.1 骨架 ✅
- [x] 四文档交叉核对已记录 —— 2.1 `known-failures.md` · 2.2 `2.2/cross-check.md` · 2.3 `2.3/cross-check.md` ✅（Phase 2 收尾时再总核对）
- [ ] 旧审计报告已标注废止
- [ ] 56 个集成测试全部通过（v2.0 语义）；2.4 完成后 `&mut`/闭包调用增量测试通过 —— **2.5**
- [ ] `cargo clippy --workspace -- -D warnings` 通过
- [ ] `cargo fmt --check --all` 通过
- [ ] `grep -r "unsafe" crates/trust_parser crates/trust_hir crates/trust_tir` 结果为空
- [ ] `Trust.toml` 解析与 `Cargo.toml` 桥接可用
- [ ] CI 性能回归监控就位
- [ ] Fuzz 语料库已初始化

---

> **规范冻结矩阵（本 Phase 交付时）：**
> - ✅ 词法规范（关键字集 43 个、字面量、注释格式）—— 2.1 完成时冻结
> - ✅ 标准库模块大纲（无用户面 Option/Result）—— 2.1 骨架对齐后逐 Phase 冻结
> - ✅ `number`=f64 类型规则 + 位运算约束 + 循环计数/2^53 检查（`DiagError` 占位）—— 2.2 完成时冻结；索引/`.length` 整数语义 API 归 Phase 6
> - ✅ 函数声明规则（块体强制返回标注、表达式体函数、箭头函数推断）—— 2.3 完成时冻结
> - 🔜 具名类型/纯结构类型/隐式泛型/`unknown`+`match` —— Phase 3 逐子任务冻结
> - 🔜 错误处理/`null` 安全 —— Phase 4
> - 🔜 并发/`async`/FFI —— Phase 5/7

> **下一步：** Phase 3 — 类型系统与方法（`phase3-types` 分支）
