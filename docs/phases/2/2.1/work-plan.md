# Phase 2.1 工作计划

> 里程碑：2.1-CLEANUP · 分支：`phase2-v2-align`
> 规格书：`docs/phases/2/2.1/2.1-spec.md` · 约束：`docs/design-constraints.md`
> 期限：1.5–2 周 · 本计划：7 步、35 项任务

---

## 约束速查（design-constraints.md 关键条款）

本计划每项任务标注适用约束。关键字速查：

| 标签 | 约束 | 来源 |
|------|------|------|
| **P0:ref** | 每个关键结构上方注释设计文档章节号 | §1.3 |
| **P0:magic** | 字面量值必须命名常量 | §2.1 |
| **P0:unsafe** | trust_parser/hir/tir 禁止 unsafe | §3.2 |
| **P0:unwrap** | 禁止 unwrap/expect（不变量场景除外） | §3.1 |
| **P0:err-name** | 错误信息不暴露 TIR 内部名 | §8.3 |
| **P1:test** | 每个特性必须有端到端集成测试 | §5.5 |
| **P2:snake** | snake_case 函数/变量，PascalCase 类型 | §1.1 |
| **P2:cov** | trust_tir ≥85%，其余 ≥70% | §5.3 |

---

## Step 1：关键字表重核（lexer）— 0.5 天

### 1.1 更新 `static KEYWORDS`

**文件：** `crates/trust_parser/src/lexer.rs` L9–63

- [ ] 删除 16 个废弃条目的 `(k, v)` 元组：`loop`/`bigint`/`interface`/`impl`/`select`/`undefined`/`None`/`Some`/`Ok`/`Err`/`Rc`/`Arc`/`Weak`/`Box`/`dyn`/`extends`
- [ ] 新增 5 个条目：`("unknown", TokenKind::Unknown)`, `("try", TokenKind::Try)`, `("catch", TokenKind::Catch)`, `("null", TokenKind::Null)`, `("panic", TokenKind::Panic)`
- [ ] 每项附带 `// 设计 §X.X` 来源注释（**P0:ref**）
- [ ] 确认 5 个已存在关键字无需操作：`type`/`match`/`throw`/`shared`/`spawn`

### 1.2 更新 `TokenKind` 枚举

**文件：** `crates/trust_parser/src/lexer.rs` L67–167

- [ ] 删除 16 个废弃变体 + `BigIntLiteral`（字面量 token）
- [ ] 新增 5 个变体：`Unknown`, `Try`, `Catch`, `Null`, `Panic`
- [ ] 每个变体上方加 `/// 设计 §X.X` 文档注释（**P0:ref**）

### 1.3 更新文件头注释

**文件：** `crates/trust_parser/src/lexer.rs` L1–5

- [ ] `//! 54 个关键字` → `//! 43 个关键字`
- [ ] `//! 6 种字面量格式` → `//! 5 种字面量格式`

### 1.4 更新 `TokenKind::can_end_stmt`

**文件：** `crates/trust_parser/src/lexer.rs` L173–195

- [ ] 移除 `None_` / `BigIntLiteral`，替换为 `Null` 等
- [ ] **保留 `Bang`**（仅前缀 `!x`）
- [ ] **保留 `Break` / `Continue` / `Return` / `Throw`** 等活跃 token

### 1.5 移除 `BigIntLiteral` 字面量 token

**文件：** `crates/trust_parser/src/lexer.rs`

- [ ] 删除 `TokenKind::BigIntLiteral(i64)` 变体
- [ ] 删除 lexer 中 `Nn` 后缀的数字字面量识别路径
- [ ] 相关辅助函数中移除 `BigIntLiteral` 引用

### 1.6 更新 lexer 单元测试

**文件：** `crates/trust_parser/src/lexer.rs` L683+

- [ ] `lex_keyword_count_is_54` → `lex_keyword_count_is_43`，断言 `KEYWORDS.len() == 43`
- [ ] 删除 `lex_bigint_literal_returns_bigint_token`
- [ ] 新增 `lex_null_literal`：验证 `null` → `TokenKind::Null`

### 1.7 验证

- [ ] `cargo build -p trust_parser` 预期失败——记录错误清单到 `docs/phases/2/2.1/build-errors-step1.txt`

---

## Step 2：移除 `loop`（端到端）— 1 天

### 2.1 AST 层

**文件：** `crates/trust_parser/src/ast.rs`

- [ ] 删除 `Stmt::Loop(LoopExpr)` 变体（L44）
- [ ] 删除 `Expr::LoopExpr` 变体
- [ ] 删除 `LoopExpr` 结构体定义
- [ ] 删除/禁用 `BreakStmt.value: Option<Box<Expr>>` 字段（L158–163），保留 `span` 字段；`break` 仍合法，`break expr` 报错
- [ ] 若 `BreakStmt` 无 value 后还需 `struct`，保留最小化结构 `struct BreakStmt { span: Span }`

### 2.2 Parser 层

**文件：** `crates/trust_parser/src/parser.rs`

- [ ] 删除 `fn parse_loop`（L449）
- [ ] 删除 `parse_stmt` / `parse_expr` 中 `loop` 关键字的调用分支
- [ ] 修改 `fn parse_break`（L461）：若 `self.peek()` 是表达式开头 → 报错"break with value is no longer supported after loop removal"

### 2.3 HIR 层

**文件：** `crates/trust_hir/src/hir.rs`

- [ ] 删除 `HirExpr::Loop(HirLoop, Span)` 变体（L228）
- [ ] 删除 `HirLoop` 结构体定义
- [ ] 若存在 `HirStmt::Loop`，一并删除

**文件：** `crates/trust_hir/src/name_res.rs`

- [ ] 删除 `Expr::LoopExpr` / `Stmt::Loop` → `HirExpr::Loop` 降级分支

**文件：** `crates/trust_hir/src/typeck.rs`

- [ ] 删除 `HirExpr::Loop` 类型检查分支
- [ ] 删除 `infer_loop_type` 函数

### 2.4 TIR 层

**文件：** `crates/trust_tir/src/tir.rs`

- [ ] 删除 `lower_loop_stmt` 函数
- [ ] 删除 `HirStmt::Loop` / `HirExpr::Loop` 降级调用

**文件：** `crates/trust_tir/src/borrowck.rs` / `moveck.rs`

- [ ] 删除 Loop 相关 borrowck/moveck 路径

### 2.5 Codegen 层

**文件：** `crates/trust_codegen/src/codegen.rs`

- [ ] 删除 `loop` → Rust `loop { ... }` 生成分支

### 2.6 验证

- [ ] 执行 MS-2.1-2 中 loop 相关 grep（见 spec），确认残留仅限注释

---

## Step 3：移除 `bigint`（端到端）— 0.75 天

### 3.1 AST 层

**文件：** `crates/trust_parser/src/ast.rs`

- [ ] 删除 `Type::BigIntType` 变体
- [ ] 删除 `Expr::BigIntLiteral(i64)` 变体

### 3.2 Parser 层

**文件：** `crates/trust_parser/src/parser.rs`

- [ ] 删除 `bigint` 类型标注解析路径
- [ ] 删除 `Nn` 字面量解析（已在 Step 1.5 lexer 层移除）

### 3.3 HIR 层

**文件：** `crates/trust_hir/src/hir.rs`

- [ ] 删除 `HirExpr::BigIntLiteral(i64, Span)` 变体（L202）
- [ ] 删除 `HirType::I64` / `HirType::BigInt` 变体
- [ ] 删除 `from_ast_type` 中 `BigIntType` 分支
- [ ] 删除 `as_rust_type` 中 `I64` / `BigInt` 分支
- [ ] 删除 `Display for HirType` 中 `i64` / `bigint` 字符串

**文件：** `crates/trust_hir/src/name_res.rs`

- [ ] 删除 `Expr::BigIntLiteral` → `HirExpr::BigIntLiteral` 降级分支

**文件：** `crates/trust_hir/src/typeck.rs`

- [ ] 删除 `HirExpr::BigIntLiteral` 类型检查分支
- [ ] 删除 `I64` 类型的一元 `-` 允许规则
- [ ] 删除 `as` 转换中 `I64` 目标类型的处理

### 3.4 TIR 层

**文件：** `crates/trust_tir/src/tir.rs`

- [ ] 删除 `TirValue::BigIntLiteral(i64)` 变体
- [ ] 删除 `lower_expr_to_value` 中 `BigIntLiteral` 分支

**文件：** `crates/trust_tir/src/moveck.rs`

- [ ] 删除 `is_copy_type` 中 `BigInt` / `I64` 判定

### 3.5 Codegen 层

**文件：** `crates/trust_codegen/src/codegen.rs`

- [ ] 删除 `TYPE_I64` 常量
- [ ] 删除 `hir_type_to_rust` 中 `I64` / `BigInt` 分支
- [ ] 删除 `tir_value_type` 中 `BigIntLiteral` 分支
- [ ] 删除 `emit_value` 中 `BigIntLiteral` 分支

### 3.6 验证

- [ ] 执行 MS-2.1-2 中 bigint 相关 grep

---

## Step 4：移除 `interface`/`impl`/`select` 残留 — 0.5 天

### 4.1 Parser 层

**文件：** `crates/trust_parser/src/parser.rs`

- [ ] 删除 `interface` / `impl` 关键字同步点（Phase 1 未实现语义，仅清理预留分支）
- [ ] 确认 `parse_stmt` 等函数中无 `TokenKind::Interface` / `TokenKind::Impl` / `TokenKind::Select` 引用

### 4.2 AST 层

**文件：** `crates/trust_parser/src/ast.rs`

- [ ] 删除 `select` 相关 AST 节点/转义槽（若有）

### 4.3 验证

- [ ] `grep -rni "TokenKind::Interface\|TokenKind::Impl\|TokenKind::Select" crates/trust_parser/src/` 返回 0

---

## Step 5：移除其余残留 + `null` 映射 + 旧后缀运算符 — 0.75 天

### 5.1 更新 `Parser::can_expr_start`

**文件：** `crates/trust_parser/src/parser.rs` L609–630

- [ ] 移除 `None_` / `BigIntLiteral` / `Loop`
- [ ] 替换为 `Null` / `Panic` 等新 token
- [ ] **保留 `Bang`**（用于前缀 `!x`；后缀 `expr!` 已在 5.2 移除）

### 5.2 移除旧后缀运算符 `expr!` / `expr?`

**文件：** `crates/trust_parser/src/ast.rs` L265–267

- [ ] 删除 `Expr::AssertUnwrap(Box<Expr>)` 变体
- [ ] 删除 `Expr::TryPropagate(Box<Expr>)` 变体
- [ ] 删除 `ast.rs` 单元测试中的引用（L431–434）

**文件：** `crates/trust_parser/src/parser.rs` L670–676

- [ ] 删除 `parse_binary` 中后缀 `!` → `Expr::AssertUnwrap` 分支
- [ ] 删除后缀 `?` → `Expr::TryPropagate` 分支
- [ ] **保留** `TokenKind::QuestionDot` → `MemberAccess`（`?.` 空值链）
- [ ] **保留** `TokenKind::QuestionQuestion` → `BinOp::QuestionQuestion`（`??` 空值合并）

**测试清理（本步禁用/删除，记录到 known-failures.md）：**
- `snap_bang` (`snapshot_tests.rs:114`): 删除 `assert_ast_snapshot("let v=maybeValue!", ...)` 
- `snap_try` (`snapshot_tests.rs:118`): 删除
- `syn037_bang` (`parser.rs:1120`): 删除
- `syn038_try` (`parser.rs:1128`): 删除

### 5.3 HIR 层旧运算符清理

**文件：** `crates/trust_hir/src/hir.rs` L225–227

- [ ] 删除 `HirExpr::AssertUnwrap(Box<HirExpr>, Span)` 变体
- [ ] 删除 `HirExpr::TryPropagate(Box<HirExpr>, Span)` 变体

**文件：** `crates/trust_hir/src/name_res.rs` L421–428, L933–936

- [ ] 删除 `ast::Expr::AssertUnwrap` → `HirExpr::AssertUnwrap` 降级分支
- [ ] 删除 `ast::Expr::TryPropagate` → `HirExpr::TryPropagate` 降级分支
- [ ] 删除 `name_res` 中 `HirExpr::AssertUnwrap` / `TryPropagate` 的处理分支

**文件：** `crates/trust_hir/src/typeck.rs` L360–364, L633

- [ ] 删除 `HirExpr::AssertUnwrap` / `TryPropagate` 类型检查分支
- [ ] 删除哨兵分支中的 `AssertUnwrap | TryPropagate => HirType::Error`

### 5.4 Codegen 层旧运算符清理

**文件：** `crates/trust_codegen/src/codegen.rs`

- [ ] 删除 `HirExpr::AssertUnwrap` / `TryPropagate` 代码生成分支（若存在）

### 5.5 `null` 关键字映射

**文件：** `crates/trust_parser/src/parser.rs`

- [ ] `null` 关键字 → `Expr::Null`（取代旧 `None_ → Expr::Null` 路径）
- [ ] 新增 `snap_null_literal` 快照测试（`snapshot_tests.rs`）：`assert_ast_snapshot("let n=null", &["LetStmt", "Null"])`

### 5.6 trust_error 清理

**文件：** `crates/trust_error/src/diagnostic.rs`

- [ ] 清理仅服务于已删 AST/运算符的废弃错误码（如有引用上述变体的错误码）

### 5.7 全仓验证

- [ ] 执行 MS-2.1-2 全部 grep（含 AssertUnwrap/TryPropagate）

---

## Step 6：规范对齐 v2.0 — 0.75 天

### 6.1 删除 spec 废弃条目

**文件：** `spec/trust-spec.md`

- [ ] 删除/标注废弃：`interface`/`impl` 词法+语法+语义、ADT、`Option`/`Result`/`?`/`!`、`select`、`loop`/`bigint`

### 6.2 前瞻同步新增规范（写入 spec，实现归 2.2/2.3）

**文件：** `spec/trust-spec.md`

- [ ] `number`=f64 类型规则（2.2）
- [ ] `number` 整数语义（索引/循环/长度/FFI + 2^53 警告）（2.2）
- [ ] `number` 位运算约束（2.2）
- [ ] 块体函数强制返回标注（2.3）
- [ ] 表达式体函数语法（2.3）

### 6.3 废止旧审计

**文件：** `docs/phases/0/0.3/audit-report.md`

- [ ] 顶部插入废止声明（见 spec MS-2.1-7）

### 6.4 建立章节冻结矩阵

**文件：** 记录在本计划文件末尾或 spec 前言

### 6.5 交叉核对（MS-2.1-9）

- [ ] 对 `Trust-设计文档.md` v2.0 / `spec/trust-spec.md` / `design-constraints.md` 交叉核对
- [ ] 记录结论到 `known-failures.md`

---

## Step 7：编译验证 + 失败清单 — 0.75 天

### 7.1 创建失败清单文件

- [ ] 创建 `docs/phases/2/2.1/known-failures.md`

### 7.2 禁用/移除无法编译的测试

**集成测试** (`crates/trustc/tests/integration.rs`)：
- [ ] 禁用 `e2e_bigint` (L85)
- [ ] 禁用 `e2e_loop_break` (L117)
- [ ] 禁用 `e2e_break_value` (L158)

**快照测试** (`crates/trust_parser/tests/snapshot_tests.rs`)：
- [ ] 禁用 `snap_loop` (L94)
- [ ] `snap_bang` / `snap_try`——已在 Step 5.2 删除

**parser 内嵌测试** (`crates/trust_parser/src/parser.rs`)：
- [ ] `syn037_bang` / `syn038_try`——已在 Step 5.2 删除
- [ ] 检查是否有引用 `Stmt::Loop` / `LoopExpr` 的内嵌测试，一并禁用

所有禁用项记录到 `known-failures.md`，标记为「待 2.5 恢复/改写」。

### 7.3 编译验证

- [ ] `cargo build --workspace` 必须通过

### 7.4 测试运行

- [ ] `cargo test --workspace` 运行，收集运行时失败到 `known-failures.md`

### 7.5 全量验收

- [ ] MS-2.1-1：grep 关键字 43 个 + 头注释
- [ ] MS-2.1-2：全仓 grep 无残留
- [ ] MS-2.1-3：`can_end_stmt` / `can_expr_start` 验证
- [ ] MS-2.1-4：`BreakStmt.value` 验证
- [ ] MS-2.1-5：`null` 映射 + 测试
- [ ] MS-2.1-6：spec 废弃条目删除
- [ ] MS-2.1-7：旧审计废止声明
- [ ] MS-2.1-8：章节冻结矩阵
- [ ] MS-2.1-9：交叉核对记录
- [ ] MS-2.1-10：`cargo build --workspace` + `known-failures.md`

### 7.6 CI 验证（与 spec 一致）

- [ ] `cargo clippy --workspace -- -D warnings` 通过
- [ ] `cargo fmt --check --all` 通过
- [ ] `grep -r "unsafe" crates/trust_parser crates/trust_hir crates/trust_tir` 返回空（**P0:unsafe**）

---

## 约束合规检查清单

| 约束 | 本里程碑适用性 | 核查点 |
|------|-------------|--------|
| P0:ref | ✅ 所有新增/修改的 TokenKind 变体需有设计文档注释 | Step 1.2, 1.1 |
| P0:magic | ⚠️ 本里程碑以删除为主，新增代码极少；若有新增字面量则命名 | — |
| P0:unsafe | ✅ 不移除任何 unsafe 块 | Step 7.6 |
| P0:unwrap | ✅ 不新增 unwrap/expect | 全步骤 |
| P0:err-name | ⚠️ 新增的 `break value` 错误信息、`null` 解析错误需用 Trust 源码名 | Step 2.2 |
| P1:test | ✅ 新增 `snap_null_literal` 快照测试 | Step 5.5 |
| P2:snake | ✅ 新增 token 命名遵循 CamelCase（enum 变体自动符合） | Step 1.2 |
| P2:cov | ⚠️ 本里程碑以删除代码为主，覆盖率可能略微波动——不设硬性下降阈值 | — |

---

## 风险跟踪

| 风险 | 当前状态 | 触发条件 | 应急措施 |
|------|---------|---------|---------|
| lexer 删变体后 parser 大面积编译失败 | 🔴 必然 | Step 1 完成 | 按 Step 1→7 顺序修复，清单记录到 build-errors-step1.txt |
| `!`/`?` 混淆（前后缀） | 🟡 留意 | Step 5.2 | 保留前缀 Bang+QuestionDot/QuestionQuestion，仅删后缀 AssertUnwrap/TryPropagate |
| 测试禁用后覆盖率下降超 5% | 🟢 低 | Step 7.4 | 2.5 恢复测试时补齐 |
| spec 修改范围大导致遗漏 | 🟡 留意 | Step 6 | MS-2.1-9 交叉核对兜底 |

---

## 耗时预估

| Step | 描述 | 预估 |
|------|------|------|
| 1 | 关键字表重核 | 0.5 天 |
| 2 | 移除 loop（5 crate） | 1 天 |
| 3 | 移除 bigint（5 crate） | 0.75 天 |
| 4 | 移除 interface/impl/select | 0.5 天 |
| 5 | 其余残留 + null 映射 + 旧后缀运算符 | 0.75 天 |
| 6 | spec 对齐 v2.0 | 0.75 天 |
| 7 | 编译验证 + 失败清单 + 验收 | 0.75 天 |
| **合计** | | **5 天**（1 工作周，含 buffer 1.5–2 周） |
