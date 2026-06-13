# Phase 1 — 编译器核心实现  TODO

> 目标：实现最小可用 Trust 编译器（Trust → Rust 源码 → 二进制）  
> 期限：第 2–4 个月  
> 优先级：P0（阻塞所有后续 Phase）  
> 最后辩论：与 OpenClaw 三轮对抗审查，R1 发现 23 条 → R2 共识 20 项独立发现（🔴2 🟠6 🟡12），全部达成共识裂口清零

---

## 1.1 项目初始化

**产出物：** Cargo workspace + CI/CD + 工程骨架  
**工作量：** 2 天  **状态：** ✅ 完成 (2026-04-14)
**优先级：** P0

### Cargo workspace

- [x] Cargo workspace 搭建（crate 结构按 `docs/design-constraints.md` §1.2）
- [x] `rustfmt.toml`、`clippy.toml` 配置
- [x] MSRV 声明（stable Rust ≥ 1.80，`Cargo.toml` 中 `rust-version` 字段；对齐代码依赖 `LazyLock` 1.80）

### CI/CD

- [x] CI/CD 配置（GitHub Actions，按 constraints §11.1）
- [x] CI job: `cargo test --workspace`
- [x] CI job: `cargo clippy --workspace -- -D warnings`
- [x] CI job: `cargo fmt --check --all`
- [x] CI job: `grep -r "unsafe" crates/trust_parser crates/trust_hir crates/trust_tir && exit 1 || echo "OK"`（P0：前三 crate 零 unsafe，constraints §3.2）
- [x] CI job: `cargo tarpaulin -p trust_tir --fail-under 85`（P1，nightly toolchain）
- [x] CI job: `cargo tarpaulin -p trust_parser --fail-under 70`（P1，constraints §5.3）
- [x] CI job: `cargo tarpaulin -p trust_hir --fail-under 70`（P1）
- [x] CI job: `cargo tarpaulin -p trust_codegen --fail-under 70`（P1）
- [x] CI job: `cargo miri test -p ferro_rt`（P1，unsafe 块验证，nightly）
- [x] CI job: `cargo test --workspace` on MSRV（Rust 1.80，constraints §11.2 P1）
- [x] `.github/dependabot.yml` 依赖自动更新配置

### Clippy P0 约束

- [x] `clippy.toml` 启用 `clippy::unwrap_used`、`clippy::expect_used`（P0：编译器 crate 禁止 unwrap/expect，constraints §3.1）
- [x] `clippy.toml` 启用 `clippy::unnecessary_cast`、`clippy::cast_lossless`（数字类型规范）

### Fuzz + Bench + 工程规范

- [x] `fuzz/` 目录初始化（cargo-fuzz：parser + TIR + codegen 目标）
- [x] `benches/` 基础目录（criterion benchmark，v0.1 基础，v0.2 正式启用）
- [x] `CHANGELOG.md` 初始化（Keep a Changelog 格式，constraints §11.4）
- [x] Workspace 所有 crate 版本同步声明（SemVer `0.1.0`，constraints §11.3）
- [x] `cargo publish --dry-run` 通过（workspace 成员统一 bump）
- [x] 交叉编译目标声明：`wasm32-unknown-unknown`（P2，Phase 1 仅声明不实现；声明见 `.cargo/config.toml`）

---

## 1.2 `trust_parser` — 词法分析与语法分析

**产出物：** `crates/trust_parser/`  
**工作量：** 3–4 周  **状态：** ✅ 完成 (2026-04-14)
**优先级：** P0  
**依赖：** 1.1

> **P0 约束：** 本 crate 零 `unsafe` 块（constraints §3.2）。

### 1.2.1 AST 节点定义

- [x] `crates/trust_parser/src/ast.rs`：完整 AST 节点定义（按 `spec/trust-spec.md` §SEM-REQ-001）
- [x] Stmt 13 变体：`Let`/`Const`/`Shared`/`Function`/`If`/`For`/`ForOf`/`While`/`Loop`/`Return`/`Break`/`Continue`/`Expr`。Expr 20 变体含 `IfExpr`/`LoopExpr`/`ArrowFn`/`TemplateLiteral` 等。
  > *Phase 1 实际解析全部 13 Stmt。Phase 2+ Stmt（Switch/Match/IfLet/Select）预留于 Expr 枚举中转义。*
- [x] 类型节点：9 Type 变体（NumberType/StringType/BooleanType/BigIntType/VoidType/Named/Array/Tuple/Ref），Phase 2+ 追加 Generic/TraitObject/Option/Result
- [x] Source span 附加到每个 AST 节点（文件路径 + 真实行列号，parser `self.span()` 读取 lexer 坐标）

### 1.2.2 Lexer（词法分析器）

- [x] `crates/trust_parser/src/lexer.rs`：Tokenizer（392 行）
- [x] 关键字识别：54 关键字（LazyLock 静态缓存）
- [x] 字面量解析：整数（`i32`）、浮点（`f64`）、BigInt（`i64`）、字符串、模板（3-token 拆分 + in_template 状态机 + TemplateInterpolation 产出）、布尔
- [x] 注释跳过：`//` 行注释、`/* */` 块注释、`///` 文档注释
- [x] 运算符/分隔符/箭头（`=>`）token 生成 + ASI 换行分隔（含续行 `=` `{` `(` 等阻止）
- [x] 验收标准：17 AC-LEX tests 全部通过（超覆蓋 14 AC-LEX）

### 1.2.3 Parser（语法分析器）

- [x] `crates/trust_parser/src/parser.rs`：递归下降 + Pratt + postfix 解析器（560 行）
- [x] **Phase 1 语法子集**（对齐 ROADMAP §1.2 + §1.4）：
  - [x] `let` / `let mut` / `shared` 变量声明
  - [x] `const` 编译时常量声明
  - [x] `function` 函数声明 — 含 `inout` / `move` 参数标注
  - [x] `if` / `else` / `for` / `for-of` / `while` / `loop`
  - [x] `return`、`break`、`continue`
  - [x] 基本类型标注：`number`、`string`、`boolean`、`void`、`bigint`
  - [x] 算术/比较/逻辑表达式 + `??` 空值合并
  - [x] `as` 显式类型转换（TYP-REQ-001）
  - [x] `&` 显式引用创建（OWN-REQ-003）
  - [x] `() => expr` 箭头函数 / `move () => expr` 闭包（SYN-REQ-009）
  - [x] 函数调用 — 含 `inout` / `move` 调用处标注（OWN-REQ-002）
  - [x] `import` / `export` 模块声明
  - [x] 模板字面量 `` `...${expr}...` ``
- [x] 错误恢复：panic mode + 同步点（`;` `}` `function` `import` `export` `type` `interface` `impl` `test` `async`；**MVP 限制**：不保证恢复全部后续语句，完整恢复见 Phase 1.3）
- [x] 验收标准：25 AC-SYN + 2 AC-ERR-REC 全部通过（30 unit tests + 34 snapshot tests）
  > **Phase 1 AC-SYN 覆盖：** AC-SYN-001~006（变量/函数基本声明）、AC-SYN-009~012（控制流）、AC-SYN-020~023（模块）、AC-SYN-030~031（箭头函数/闭包）、AC-SYN-036~042（引用/运算符/错误恢复/分隔规则）
  > **不覆盖：** 007~008（泛型）、013~016（match/switch/if let）、017~019（async/await/spawn）、024~026（Channel/select/withLock）、027~029（interface/type/ADT）、032~033（FFI）、034~035（test/属性）

### 1.2.4 模块图

- [x] `crates/trust_parser/src/module_graph.rs`：跨文件依赖解析（117 行，DAG + 循环检测 + 拓扑排序，3 AC-MOD）
- [x] `crates/trust_parser/src/resolve_imports.rs`：4 种路径格式（`./` `../` `/` `std::`）+ 3 种导入语法格式（Named/Default/Namespace）（47 行，3 AC-IMP）
- [x] 循环导入检测 + 报错

### 1.2.5 测试

- [x] 单元测试：57 lib tests（4 AST + 17 LEX + 30 SYN + 3 MOD + 3 IMP）
- [x] 测试命名遵循 `{subject}_{condition}_{expected}` 模式（constraints §5.2）
- [x] 快照测试：34 snapshot tests（`tests/snapshot_tests.rs`，覆盖全部 25 AC-SYN）
- [x] Fuzz 目标：`fuzz/fuzz_targets/parse.rs` 调用 `parser::parse()`，nightly fuzz 可启动

---

## 1.3 `trust_hir` — HIR 与类型检查

**产出物：** `crates/trust_hir/`  
**工作量：** 2–3 周  **状态：** ✅ 完成 (2026-06-13)
**优先级：** P0  
**依赖：** 1.2

> **P0 约束：** 零 `unsafe`（constraints §3.2）。所有 pub 函数和关键结构上方标注 `// §X.Y.Z: ...` 设计文档章节引用（constraints §1.3）。

### 1.3.1 HIR 节点定义

- [x] `crates/trust_hir/src/hir.rs`：HIR 节点（AST → HIR 降级）
- [x] 名称解析后的符号绑定（`import` → 实际文件/导出）
- [x] 作用域结构（函数参数、`let` 局部作用域、`const`/`shared` 模块作用域）

### 1.3.2 类型检查

- [x] `crates/trust_hir/src/typeck.rs`：类型检查器
- [x] 基本类型兼容性检查
- [x] 函数签名验证（参数数量、类型、返回值）
- [x] 二元运算类型检查（`i32 + f64` → 编译错误，TYP-REQ-001）
- [x] `as` 显式类型转换检查
- [x] 验收标准：AC-TYP-001~003 通过

### 1.3.3 名称解析

- [x] `crates/trust_hir/src/name_res.rs`：跨文件名称解析
- [x] `import` 目标验证（导出是否存在）
- [x] `export` 冲突检测

### 1.3.4 错误收集

- [x] 函数级独立检查：同一模块内不同函数间的错误互不影响
- [x] `Vec<Diagnostic>` 收集，统一报告（constraints §3.1.1）
- [x] `Type::Error` 哨兵占位（避免级联类型报错）

### 1.3.5 测试

- [x] 单元测试（happy path + 错误路径）
- [x] 测试命名遵循 `{subject}_{condition}_{expected}` 模式（constraints §5.2）
- [x] Doctest（pub 函数推荐有，constraints §5.4）
- [x] 集成测试：`.` 文件 → HIR 快照比对
- [x] 验收标准：AC-SEM-001~010 中 Phase 1 相关项通过（AST→HIR 降级、名称解析、作用域、`if`/`loop` 表达式→语句转换）

---

## 1.4 `trust_tir` — TIR 与所有权检查

**产出物：** `crates/trust_tir/`  
**工作量：** 4–6 周  **状态：** ✅ 完成 (2026-06-14)
**优先级：** P0  
**依赖：** 1.3

> **P0 约束：** 零 `unsafe`（constraints §3.2）。所有 pub 函数和关键结构上方标注 `// §X.Y.Z: ...` 设计文档章节引用（constraints §1.3 P0）。
> 
> **Phase 1 并发范围：** 不实现 `spawn` / `Channel` / `shared` / `select`（押后 Phase 4）。`move` 闭包仅用于局部语义（FnOnce），不涉及跨线程。AC-CON-001~013 和 AC-OWN-011~014（spawn/Rc/Arc）押后 Phase 4。

### 1.4.1 TIR 节点定义

- [x] `crates/trust_tir/src/tir.rs`：TIR 控制流图节点
- [x] HIR → TIR 降级：`if`/`for`/`loop` → 基本块 + 条件跳转
- [x] 表达式→语句转换：`if`/`loop` 表达式的值通过临时变量持有
- [x] 方法调用展开：`pt.print()` → `Printable::print(&pt)`（Phase 1 无方法，占位）
- [x] 闭包捕获提升：闭包体引用的外部变量提升为隐式参数

### 1.4.2 移动语义检查（moveck）

- [x] `crates/trust_tir/src/moveck.rs`：移动语义分析
- [x] `let b = a;` 后 `a` 失效（OWN-REQ-001）
- [x] `Copy` 类型判定：标量 Copy、堆类型非 Copy（OWN-REQ-008）
- [x] 错误信息映射：**TIR 内部名 → Trust 源码变量名 + 行列号**（constraints §6.2, §8.3 P0）

### 1.4.3 借用检查（borrowck）

- [x] `crates/trust_tir/src/borrowck.rs`：借用检查器
- [x] 三模式参数表（OWN-REQ-002）：默认只读借用、`inout` 可变借用、`move` 所有权转移
- [x] 调用处对称标注检查：`pushOne(inout data)` vs `pushOne(data)` 错误
- [x] 借用规则（OWN-REQ-003）：同一变量同时 ≤1 可变借用 或 ≥0 只读借用
- [x] 方法调用所有权（OWN-REQ-004）：`let` 非 `mut` → 仅 `&self` 方法
- [x] 闭包捕获规则（OWN-REQ-005）：默认只读借用 / `move` → FnOnce（**不涉及 `spawn`**）

### 1.4.4 区域推断（Region Inference）

- [x] 生命周期自动推导（OWN-REQ-009）：函数参数→返回值生命周期绑定
- [x] `for` 循环隐式可变例外（OWN-REQ-007）：`for (let i=0; i<N; i=i+1)` 中 `i` 隐式可变
- [x] 回退策略：TIR 推断不足时生成显式生命周期标注 Rust 代码，由 rustc 保底

### 1.4.5 测试

- [x] 单元测试（每个 pub 函数必有）
- [x] 测试命名遵循 `{subject}_{condition}_{expected}` 模式（constraints §5.2）
- [x] Doctest（`trust_tir` **所有** pub 函数必须有 doctest — P0 约束，constraints §5.4）
- [x] 行覆盖率 ≥ 85%（tarpaulin CI 门控）
- [x] 分支覆盖率 ≥ 60%
- [x] Fuzz 目标：`fuzz/fuzz_targets/tir_borrowck.rs` — 随机 TIR 图不 panic（P1，constraints §11.6）
- [x] 验收标准：AC-OWN-001~006（移动/借用基本规则）、AC-OWN-007~008（方法调用所有权，Phase 2 启用）、AC-OWN-009~010（闭包捕获）、AC-OWN-015~017（for 隐式可变/Copy 判定）、AC-OWN-018~020（生命周期省略）通过

---

## 1.5 `trust_codegen` — Rust 代码生成 + ferro_rt Stub

**产出物：** `crates/trust_codegen/` + `crates/ferro_rt/`  
**工作量：** 2–3 周  
**优先级：** P0  
**依赖：** 1.4

> **P0 约束：** 所有 pub 函数和关键结构上方标注 `// §X.Y.Z: ...` 设计文档章节引用（constraints §1.3 P0）。代码生成中禁止硬编码 Trust/Rust 语法字符串（constraints §2.2）。

### 1.5.1 Rust 源码生成

- [x] `crates/trust_codegen/src/codegen.rs`：TIR → Rust 源码 (~550行，含常量表/类型映射/TirOp映射/控制流重构/可变性分析)
- [x] 参数模式映射：默认借用 → `&T`、`inout` → `&mut T`、`move` → `T`
- [x] 函数生成：`function foo(x: number): number { ... }` → `fn foo(x: &i32) -> i32 { ... }`
- [x] 控制流生成：`if`/`for`/`while`/`loop` → Rust 等价物
- [x] `fn main()` 包装：Trust 入口 → Rust `fn main()`
- [x] 代码生成中所有字面量（除 0/1/公认常量）使用命名常量，禁止硬编码（constraints §2.1 P0）

### 1.5.2 Source Map

- [x] `crates/trust_codegen/src/sourcemap.rs`：`SourceMapping` 结构体（双向 HashMap + 回退注释）
- [x] 每个 TIR → Rust 映射保存（文件 + 行号 + 列号）
- [x] 回退模式：生成 `// @trust: src/main.trust:42:15` 注释（v0.1）

### 1.5.3 运行时库接口

- [x] `crates/trust_codegen/src/runtime.rs`：ferro_rt API 映射表
- [x] `console.log("...")` 生成 `ferro_rt::console::log("...")`（非硬编码，constraints §2.2）
- [x] 标准库路径生成（`use ferro_rt::...`）

### 1.5.4 `ferro_rt` 最小 Stub（Phase 1）

> **优先级：P0**（阻塞交付标准——`console.log` 依赖 ferro_rt 实现）

- [x] `crates/ferro_rt/Cargo.toml` 创建（零依赖，Phase 1 无 tokio/crossbeam）
- [x] `crates/ferro_rt/src/console.rs`：`pub fn log(msg: &str)` 函数（→ `println!("{}", msg)`）
- [x] `crates/ferro_rt/src/lib.rs`：导出 `console` 模块
- [x] 无 `unsafe`（Phase 1 的 ferro_rt 是纯安全 Rust 包装）

### 1.5.5 测试

- [x] 单元测试：pub fn 通过集成测试覆盖（端到端 .trust→TIR→Rust），无独立 `#[cfg(test)]` 模块
- [x] 测试命名遵循 `{subject}_{condition}_{expected}` 模式（constraints §5.2）
- [x] Doctest（generate_rust 入口有 doctest，constraints §5.4）
- [x] 集成测试：19 个端到端测试（含函数/变量/调用/控制流/参数/console.log/main包装）全部通过
- [ ] Fuzz 目标：`fuzz/fuzz_targets/codegen.rs` — 随机 TIR 图生成 Rust 源码不 panic（P1，constraints §11.6）→ **下沉 Phase 1.6（TODO.md §1.6.5）**

---

## 1.6 `trust_error` — 错误诊断

**产出物：** `crates/trust_error/`  
**工作量：** 1 周  
**优先级：** P0  
**依赖：** 1.1（独立基础 crate，`Diagnostic` 结构不依赖任何 IR——被 1.2~1.5 共用）

### 1.6.1 错误数据结构

- [ ] `crates/trust_error/src/diagnostic.rs`：`Diagnostic` 结构体
- [ ] 三级分类：`Error` / `Warning` / `Help`
- [ ] `ErrorCode` 枚举（E0382 移动后使用等）
- [ ] `SourceSpan` 结构体（文件 + 行列号 + label）

### 1.6.2 JSON 错误输出

- [ ] `crates/trust_error/src/json_fmt.rs`：`--error-format=json`
- [ ] JSON schema 对齐设计文档 §9.1.1 和 constraints §8.2
- [ ] 多 span 支持（如移动处 + 使用处）
- [ ] children 辅助信息（如修复建议）

### 1.6.3 修复建议引擎

- [ ] `crates/trust_error/src/fix_suggest.rs`：`--fix` 模式
- [ ] 简单错误修复建议（`.clone()`、`inout`、`mut` 等）
- [ ] 交互式确认（`应用此修复？(y/N)`），默认不自动修复

### 1.6.4 测试

- [ ] 单元测试（每种 ErrorCode 至少一个触发用例）
- [ ] 测试命名遵循 `{subject}_{condition}_{expected}` 模式（constraints §5.2）
- [ ] Doctest（pub 函数推荐有，constraints §5.4）
- [ ] JSON 输出格式快照测试
- [ ] 验收标准：AC-ERR-001~002（Result/`?` 传播）、AC-ERR-005~006（`!` 断言）在 Phase 1 端到端测试中覆盖。AC-ERR-003~004（throw/panic）和 AC-ERR-007~008（`.expect`）押后 Phase 3。

### 1.6.5 承接 Phase 1.5 下沉项

> 以下项由 Phase 1.5 下沉——codegen 实现已就位，但端到端测试被 TIR 所有权检查拦截。1.6 优先尝试在本阶段补齐；若仍受阻则继续下沉到 1.7。

- [ ] **bigint 字面量**端到端：`let x = 9223372036854775807;` → 生成 `i64`
- [ ] **for 循环**端到端：`for (let i = 0; i < 10; i = i + 1) { ... }` → Rust `for`
- [ ] **while 循环**端到端：`while (x > 0) { x = x - 1; }` → Rust `while`
- [ ] **loop + break**端到端：`loop { if (c) { break; } }` → Rust `loop { break; }`
- [ ] **break 带值**端到端：`let x = loop { break 42; };` → Rust `loop { break 42; }`
- [ ] **Codegen fuzz**：`fuzz/fuzz_targets/codegen.rs` — 随机 TIR 图生成 Rust 源码不 panic（P1）← 下沉自 1.5
- [ ] **可变引用 `&mut x`** 端到端：需要 parser `let mut` + TIR 放行 → **本 Phase 无法完成，→ 延伸 Phase 2**
- [ ] **闭包调用 `r()`** 端到端：需要 name_res 保留 ArrowFn + K5 闭包 TirFunction → **本 Phase 无法完成，→ 延伸 Phase 2**

---

## 1.7 `trustc` — 编译器入口

**产出物：** `crates/trustc/`  
**工作量：** 1 周  
**优先级：** P0  
**依赖：** 1.2, 1.3, 1.4, 1.5, 1.6（直接依赖全部编译器 crate）

### 1.7.1 CLI

- [ ] `crates/trustc/src/main.rs`：编译管线编排
- [ ] 子命令：`trustc compile <file>` — 编译 Trust → Rust → 二进制
- [ ] 子命令：`trustc check <file>` — 仅类型/所有权检查，无代码生成
- [ ] 子命令：`trustc eval <expr>` — 无状态表达式求值（包装为 `fn main()` 编译执行）
- [ ] `--error-format=json` flag
- [ ] `--fix` flag（交互式修复）
- [ ] `--verbose` / `--quiet` flag

### 1.7.2 Trust.toml 解析

- [ ] `Trust.toml` 配置读取（项目名、版本、依赖、异步运行时选项）
- [ ] 桥接生成 `Cargo.toml`：Trust.toml → Cargo.toml + workspace 成员

### 1.7.3 编译管线编排

对齐 `design-constraints.md` §11.5 的错误恢复策略：

- [ ] **Parse**：panic mode 收集全部语法错误，不阻塞后续阶段
- [ ] **HIR**：类型错误用 `Type::Error` 哨兵继续，函数级收集全部错误
- [ ] **TIR**：所有权错误函数级终止，但继续检查其他函数（收集全部错误）
- [ ] **Codegen**：仅在 TIR 总错误数 = 0 时运行
- [ ] 各阶段错误汇总 → 结构化 JSON 输出

---

## 1.8 Phase 1 集成测试

**工作量：** 持续  
**优先级：** P0  
**依赖：** 1.7

### 1.8.1 端到端测试

- [ ] 每个语法特性至少一个端到端测试：

```
tests/integration/
├── basic_variable.trust      # 输入
├── basic_variable.rs         # 期望 Rust 输出（快照）
├── function_call.trust
├── function_call.rs
├── if_expr.trust
├── if_expr.rs
├── for_loop.trust
├── for_loop.rs
├── closure_move.trust
├── closure_move.rs
└── ...                       # ≥ 20 个特性
```

- [ ] 测试运行器：编译 `.trust` → 比较生成 Rust 与快照 → 编译 Rust → 执行并验证输出
- [ ] 测试命名遵循 `{subject}_{condition}_{expected}` 模式（constraints §5.2）

### 1.8.2 端到端验证（替换原"自举测试"）

- [ ] 编译器能将包含 `console.log` 的 `.trust` 文件编译为可执行 Rust 二进制并运行
- [ ] 交叉编译验证：生成的 Rust 代码可通过 `rustc` 独立编译（即使不运行 `trustc`）
- [ ] 真正自举（Trust 编译器用 Trust 重写）— 押后 Phase 7

### 1.8.3 性能基准

- [x] `benches/` 目录初始化（criterion；BASELINE.md 已就位）
- [ ] 基准指标：编译 **100 行** Trust 代码（含函数、变量、控制流、函数调用）≤ 5 秒（冷启动）
- [ ] 5000 行基准移至 Phase 2（v0.1.1，届时 `trust_std` 可用作为输入源）
- [ ] CI 性能回归监控：基准记录在 `benches/BASELINE.md`，`±10%` 视为回归（P1）
- [ ] CI job: `cargo bench` 运行（criterion 基准比较）

### 1.8.4 Fuzzing

- [ ] Parser fuzz：`fuzz/fuzz_targets/parse.rs` — 随机 `.trust` 输入不 panic
- [ ] TIR fuzz：`fuzz/fuzz_targets/tir_borrowck.rs` — 随机 TIR 图不 panic（P1，constraints §11.6）
- [ ] Codegen fuzz：`fuzz/fuzz_targets/codegen.rs` — 随机 TIR 图生成 Rust 源码不 panic（P1）← 承接自 1.6.5
- [ ] 语料库从集成测试的 `.trust` 文件初始化

---

## Phase 1 交付标准

- [ ] 编译以下程序并执行输出 `"Hello, Trust!"`：

```ts
// console 为 Phase 1 隐式全局绑定，codegen 自动映射到 ferro_rt::console::log
// Phase 2 后要求显式 import { console } from "trust_std"
function main() {
    console.log("Hello, Trust!");
}
```

- [ ] `cargo test --workspace` 全部通过
- [ ] `cargo clippy --workspace -- -D warnings` 通过（含 `unwrap_used`/`expect_used` lint）
- [ ] `cargo fmt --check --all` 通过
- [ ] `grep -r "unsafe" crates/trust_parser crates/trust_hir crates/trust_tir` 结果为空（P0）
- [ ] `cargo tarpaulin -p trust_tir --fail-under 85` 通过
- [ ] `cargo tarpaulin -p trust_parser --fail-under 70` 通过
- [ ] `cargo tarpaulin -p trust_hir --fail-under 70` 通过
- [ ] `cargo tarpaulin -p trust_codegen --fail-under 70` 通过
- [ ] `cargo miri test -p ferro_rt` 通过（unsafe 块验证）
- [ ] 集成测试：≥ 20 个语法特性有端到端 `tests/integration/` 测试
- [ ] `docs/ROADMAP.md` 的 Phase 1 全部子项标记完成

---

> **下一步：** Phase 2 — 类型系统与泛型（`phase2-types` 分支）  
> **辩论记录：** 2026-06-13 与 OpenClaw 三轮对抗审查，R1 发现 23 条 → 闸门裁决 → R2 共识 20 项独立发现（🔴2 🟠6 🟡12），P0/P1/P2 全部修复，裂口清零。
