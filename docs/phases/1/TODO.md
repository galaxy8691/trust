# Phase 1 — 编译器核心实现  TODO

> 目标：实现最小可用 Trust 编译器（Trust → Rust 源码 → 二进制）  
> 期限：第 2–4 个月  
> 优先级：P0（阻塞所有后续 Phase）

---

## 1.1 项目初始化

**产出物：** Cargo workspace + CI/CD + 工程骨架  
**工作量：** 2 天  
**优先级：** P0

- [ ] Cargo workspace 搭建（crate 结构按 `docs/design-constraints.md` §1.2）
- [ ] `rustfmt.toml`、`clippy.toml` 配置
- [ ] MSRV 声明（stable Rust ≥ 1.63，`Cargo.toml` 中 `rust-version` 字段）
- [ ] CI/CD 配置（GitHub Actions，按 constraints §11.1）
- [ ] CI job: `cargo test --workspace`
- [ ] CI job: `cargo clippy --workspace -- -D warnings`
- [ ] CI job: `cargo fmt --check --all`
- [ ] CI job: `cargo tarpaulin -p trust_tir --fail-under 85`（P1，nightly toolchain）
- [ ] CI job: `cargo miri test -p ferro_rt`（P1，unsafe 块验证，nightly）
- [ ] `fuzz/` 目录初始化（cargo-fuzz，parser + codegen 目标）
- [ ] `benches/` 基础目录（v0.1 性能基准占位，v0.2 正式启用）
- [ ] `.github/dependabot.yml` 依赖自动更新配置

---

## 1.2 `trust_parser` — 词法分析与语法分析

**产出物：** `crates/trust_parser/`  
**工作量：** 3–4 周  
**优先级：** P0  
**依赖：** 1.1

### 1.2.1 AST 节点定义

- [ ] `crates/trust_parser/src/ast.rs`：完整 AST 节点定义（按 `spec/trust-spec.md` §SEM-REQ-001）
- [ ] 表达式节点：`LetStmt`、`ConstStmt`、`FunctionDecl`、`IfExpr`、`ForStmt`、`ForOfStmt`、`LoopExpr`、`SwitchStmt`、`MatchExpr`、`IfLetStmt`、`SelectStmt`
- [ ] 类型节点：`Type::Number`、`Type::String`、`Type::Boolean`、`Type::BigInt`、`Type::Void`、`Type::Array`、`Type::Tuple`、`Type::Generic`、`Type::TraitObject`、`Type::Option`、`Type::Result`、`Type::Ref`
- [ ] Source span 附加到每个 AST 节点（文件路径 + 行列号）

### 1.2.2 Lexer（词法分析器）

- [ ] `crates/trust_parser/src/lexer.rs`：Tokenizer
- [ ] 关键字识别：全部 40 个关键字按 `spec/trust-spec.md` §LEX-REQ-001
- [ ] 字面量解析：整数（`i32`）、浮点（`f64`）、BigInt（`i64`）、字符串、模板字符串、布尔
- [ ] 注释跳过：`//` 行注释、`/* */` 块注释、`///` 文档注释
- [ ] 运算符/分隔符/箭头的 token 生成
- [ ] 验收标准：AC-LEX-001~014 全部通过

### 1.2.3 Parser（语法分析器）

- [ ] `crates/trust_parser/src/parser.rs`：递归下降解析器
- [ ] **Phase 1 语法子集**（按 ROADMAP §1.2）：
  - [ ] `let` / `let mut` 变量声明
  - [ ] `const` 编译时常量声明
  - [ ] `function` 函数声明（无泛型）
  - [ ] `if` / `else` / `for` / `while` / `loop`
  - [ ] `return`、`break`、`continue`
  - [ ] 基本类型标注：`number`、`string`、`boolean`、`void`
  - [ ] 算术/比较/逻辑表达式
  - [ ] 函数调用
  - [ ] `import` / `export` 模块声明
  - [ ] 注释（`//`、`/* */`、`///`）
- [ ] 错误恢复：panic mode + 同步点（`;`、`}`、`function`、`import`、`export`、`type`、`interface`、`impl`、`test`、`async`）
- [ ] 验收标准：AC-SYN-001~042 中 Phase 1 子集全部通过

### 1.2.4 模块图

- [ ] `crates/trust_parser/src/module_graph.rs`：跨文件依赖解析
- [ ] `crates/trust_parser/src/resolve_imports.rs`：`import { ... } from "..."` 路径解析
- [ ] 循环导入检测 + 报错

### 1.2.5 测试

- [ ] 单元测试（与源码同文件 `#[cfg(test)] mod tests`）
- [ ] 快照测试（`trust_parser/tests/snapshots/` — AST 输出比对）
- [ ] Fuzz 目标（`fuzz/fuzz_targets/parse.rs` — 随机 `.trust` 输入不 panic）

---

## 1.3 `trust_hir` — HIR 与类型检查

**产出物：** `crates/trust_hir/`  
**工作量：** 2–3 周  
**优先级：** P0  
**依赖：** 1.2

### 1.3.1 HIR 节点定义

- [ ] `crates/trust_hir/src/hir.rs`：HIR 节点（AST → HIR 降级）
- [ ] 名称解析后的符号绑定（`import` → 实际文件/导出）
- [ ] 作用域结构（函数参数、`let` 局部作用域、`const`/`shared` 模块作用域）

### 1.3.2 类型检查

- [ ] `crates/trust_hir/src/typeck.rs`：类型检查器
- [ ] 基本类型兼容性检查
- [ ] 函数签名验证（参数数量、类型、返回值）
- [ ] 二元运算类型检查（`i32 + f64` → 编译错误，TYP-REQ-001）
- [ ] `as` 显式类型转换检查
- [ ] 验收标准：AC-TYP-001~003 通过

### 1.3.3 名称解析

- [ ] `crates/trust_hir/src/name_res.rs`：跨文件名称解析
- [ ] `import` 目标验证（导出是否存在）
- [ ] `export` 冲突检测

### 1.3.4 错误收集

- [ ] 函数级独立检查：同一模块内不同函数间的错误互不影响
- [ ] `Vec<Diagnostic>` 收集，统一报告（constraints §3.1.1）
- [ ] `Type::Error` 哨兵占位（避免级联类型报错）

### 1.3.5 测试

- [ ] 单元测试（happy path + 错误路径）
- [ ] 集成测试：`.` 文件 → HIR 快照比对

---

## 1.4 `trust_tir` — TIR 与所有权检查

**产出物：** `crates/trust_tir/`  
**工作量：** 4–6 周  
**优先级：** P0  
**依赖：** 1.3

### 1.4.1 TIR 节点定义

- [ ] `crates/trust_tir/src/tir.rs`：TIR 控制流图节点
- [ ] HIR → TIR 降级：`if`/`for`/`loop` → 基本块 + 条件跳转
- [ ] 表达式→语句转换：`if`/`loop` 表达式的值通过临时变量持有
- [ ] 方法调用展开：`pt.print()` → `Printable::print(&pt)`（Phase 1 无方法，占位）
- [ ] 闭包捕获提升：闭包体引用的外部变量提升为隐式参数

### 1.4.2 移动语义检查（moveck）

- [ ] `crates/trust_tir/src/moveck.rs`：移动语义分析
- [ ] `let b = a;` 后 `a` 失效（OWN-REQ-001）
- [ ] `Copy` 类型判定：标量 Copy、堆类型非 Copy（OWN-REQ-008）
- [ ] 错误信息映射：TIR 内部名 → Trust 源码变量名 + 行列号

### 1.4.3 借用检查（borrowck）

- [ ] `crates/trust_tir/src/borrowck.rs`：借用检查器
- [ ] 三模式参数表（OWN-REQ-002）：默认只读借用、`inout` 可变借用、`move` 所有权转移
- [ ] 调用处对称标注检查：`pushOne(inout data)` vs `pushOne(data)` 错误
- [ ] 借用规则（OWN-REQ-003）：同一变量同时 ≤1 可变借用 或 ≥0 只读借用
- [ ] 方法调用所有权（OWN-REQ-004）：`let` 非 `mut` → 仅 `&self` 方法
- [ ] 闭包捕获规则（OWN-REQ-005）：默认只读借用 / `move` → FnOnce

### 1.4.4 区域推断（Region Inference）

- [ ] 生命周期自动推导（OWN-REQ-009）：函数参数→返回值生命周期绑定
- [ ] `for` 循环隐式可变例外（OWN-REQ-007）：`for (let i=0; i<N; i++)` 中 `i` 隐式可变
- [ ] 回退策略：TIR 推断不足时生成显式生命周期标注 Rust 代码，由 rustc 保底

### 1.4.5 测试

- [ ] 单元测试（每个 pub 函数必有）
- [ ] Doctest（`trust_tir` 所有 pub 函数必须有 doctest — P0 约束）
- [ ] 行覆盖率 ≥ 85%（tarpaulin CI 门控）
- [ ] 分支覆盖率 ≥ 60%

---

## 1.5 `trust_codegen` — Rust 代码生成

**产出物：** `crates/trust_codegen/`  
**工作量：** 2–3 周  
**优先级：** P0  
**依赖：** 1.4

### 1.5.1 Rust 源码生成

- [ ] `crates/trust_codegen/src/codegen.rs`：TIR → Rust 源码
- [ ] 参数模式映射：默认借用 → `&T`、`inout` → `&mut T`、`move` → `T`
- [ ] 函数生成：`function foo(x: number): number { ... }` → `fn foo(x: &i32) -> i32 { ... }`
- [ ] 控制流生成：`if`/`for`/`while`/`loop` → Rust 等价物
- [ ] `fn main()` 包装：Trust 入口 → Rust `fn main()`
- [ ] 禁止硬编码 Trust/Rust 语法字符串（constraints §2.2）

### 1.5.2 Source Map

- [ ] `crates/trust_codegen/src/sourcemap.rs`：`SourceMapping` 结构体
- [ ] 每个 TIR → Rust 映射保存（文件 + 行号 + 列号）
- [ ] 回退模式：生成 `// @trust: src/main.trust:42:15` 注释（v0.1）

### 1.5.3 运行时库接口

- [ ] `crates/trust_codegen/src/runtime.rs`：ferro_rt API 映射表
- [ ] 标准库路径生成（`use ferro_rt::...`）

### 1.5.4 测试

- [ ] 单元测试
- [ ] 集成测试：完整 `.trust` 文件 → Rust 输出 → 与 `.rs` 快照比对 → rustc 编译验证

---

## 1.6 `trust_error` — 错误诊断

**产出物：** `crates/trust_error/`  
**工作量：** 1 周  
**优先级：** P0  
**依赖：** 1.4

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
- [ ] JSON 输出格式快照测试

---

## 1.7 `trustc` — 编译器入口

**产出物：** `crates/trustc/`  
**工作量：** 1 周  
**优先级：** P0  
**依赖：** 1.5, 1.6

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

- [ ] Parse → HIR → TIR → 错误检查（TIR 错误数=0 才继续）→ Codegen → rustc
- [ ] 各阶段错误数 > 0 时终止后续阶段（constraints §11.5）

---

## 1.8 Phase 1 集成测试

**工作量：** 持续  
**优先级：** P0  
**依赖：** 1.7

### 1.8.1 端到端测试

- [ ] 每个语法特性至少一个端到端测试（`tests/integration/*.trust` → `*.rs` 快照 → `rustc` 编译）
- [ ] 测试运行器：编译 `.trust` → 比较生成 Rust 与快照 → 编译 Rust → 执行并验证输出

### 1.8.2 自举测试

- [ ] 最小自举：Trust 编译器源码中的 `console.log` 调用由 Trust 自己的 codegen 生成
- [ ] 交叉编译验证：`trustc` 自身可通过生成的 Rust 代码编译（即使不运行）

### 1.8.3 性能基准

- [ ] `benches/` 目录初始化
- [ ] 基准指标：编译 5000 行 Trust 代码 ≤ 60 秒（冷启动）
- [ ] CI 性能回归监控（基准记录在 `benches/BASELINE.md`）

### 1.8.4 Fuzzing

- [ ] Parser fuzz：随机 `.trust` 输入不 panic
- [ ] Codegen fuzz：随机 TIR 图不 panic
- [ ] 语料库从集成测试的 `.trust` 文件初始化

---

## Phase 1 交付标准

- [ ] 编译以下程序并执行输出 `"Hello, Trust!"`：

```ts
function main() {
    console.log("Hello, Trust!");
}
```

- [ ] `cargo test --workspace` 全部通过
- [ ] `cargo clippy --workspace -- -D warnings` 通过
- [ ] `cargo fmt --check --all` 通过
- [ ] `cargo tarpaulin -p trust_tir --fail-under 85` 通过
- [ ] 集成测试：≥ 20 个语法特性有端到端 `tests/integration/` 测试
- [ ] `docs/ROADMAP.md` 的 Phase 1 全部子项标记完成

---

> **下一步：** Phase 2 — 类型系统与泛型（`phase2-types` 分支）
