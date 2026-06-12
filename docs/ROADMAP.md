# Trust 语言实现路线图

> 本文档定义 Trust 编译器及生态从零到可交付的完整实施路径。
> 每一步标注优先级（P0=阻塞、P1=重要、P2=增强）、预估工作量和依赖关系。

---

## Phase 0：语言规范 (v0.0 — 规范冻结)

**目标：** 将 `Trust-设计文档.md` 转化为可执行的、无歧义的语言规范文档。

### 0.1 编写 Trust Language Specification

**工作量：** 2-3 周  
**优先级：** P0（阻塞所有后续工作）

内容：
- 词法规范（token 定义、关键字列表、字面量格式、注释格式）
- 语法规范（形式化 EBNF 或类似格式的完整语法定义）
- 语义规范（每个语法制导翻译的规则——AST → HIR → TIR 的降级过程）
- 类型系统规范（名义类型、结构别名、ADT、泛型、trait bound 的完整图）
- 所有权规则规范（移动语义、借用规则、`inout`/`move`/`shared` 的形式化定义）
- 编译模型规范（TIR 节点定义、代码生成映射规则）

**产出物：** `spec/trust-spec.md`（或 `spec/` 目录下的多文件）

### 0.2 编写 Trust 标准库规范

**工作量：** 1 周  
**优先级：** P0  
**依赖：** 0.1

- `trust_std` 每个模块的 API 签名列表
- 模块间的依赖图
- 与 Rust 标准库的映射关系

### 0.3 设计文档与规范的一致性审计

**工作量：** 3 天  
**优先级：** P0  
**依赖：** 0.1, 0.2  
**状态：** ✅ 完成（审计报告：`docs/phases/0/0.3/audit-report.md`，条件通过——2 P0 死引用已修正，3 P1 记录在案）

- 确保 `Trust-设计文档.md`、`spec/trust-spec.md`、`design-constraints.md` 三者一致
- 所有示例代码在规范中用形式化语法标注

---

## Phase 1：编译器核心 (v0.1 — 最小可用编译器)

**目标：** 能编译最小的 Trust 程序（变量声明、函数、简单控制流）到 Rust 源码，并通过 rustc 编译为可执行二进制。

### 1.1 项目初始化

**工作量：** 2 天  
**优先级：** P0

- Cargo workspace 搭建（crate 结构按 `design-constraints.md` §1.2）
- CI/CD 配置（GitHub Actions，按 constraints §11.1）
- tarpaulin + miri CI job 配置
- `fuzz/` 目录初始化（cargo-fuzz，parser + codegen 目标）
- `rustfmt.toml`、`clippy.toml` 配置
- MSRV 声明（stable Rust ≥ 1.63）

### 1.2 `trust_parser` — 词法分析与语法分析

**工作量：** 3-4 周  
**优先级：** P0  
**依赖：** 0.1

| 子任务 | 说明 |
|--------|------|
| lexer | Tokenizer，支持 Trust 完整关键字和字面量 |
| parser | 递归下降解析器，覆盖 Phase 1 语法子集 |
| AST 定义 | `trust_parser/src/ast.rs` 的完整 AST 节点 |
| 错误恢复 | Parser panic mode（constraints §11.5） |

**Phase 1 语法子集：**
- `let` / `let mut` 变量声明
- `const` 编译时常量声明（等价于 Rust const）
- `function` 函数声明（无泛型）
- `if` / `else` / `for` / `while` / `loop`
- `return`、`break`、`continue`
- 基本类型：`number`、`string`、`boolean`、`void`
- 算术/比较/逻辑表达式
- 函数调用
- 模块导入/导出
- 注释（`//`、`/* */`、`///`）

### 1.3 `trust_hir` — HIR 与类型检查

**工作量：** 2-3 周  
**优先级：** P0  
**依赖：** 1.2

| 子任务 | 说明 |
|--------|------|
| HIR 节点定义 | AST → HIR 降级（名称解析、作用域分析） |
| 类型检查 | 基本类型检查、函数签名验证 |
| 名称解析 | `import` / `export` 跨文件引用 |
| 错误收集 | 函数级独立检查 + `Vec<Diagnostic>` 收集（constraints §3.1.1） |

### 1.4 `trust_tir` — TIR 与所有权检查

**工作量：** 4-6 周  
**优先级：** P0  
**依赖：** 1.3

| 子任务 | 说明 |
|--------|------|
| TIR 节点定义 | HIR → TIR 降级（控制流图、基本块） |
| 移动语义检查 | `let b = a;` 后 `a` 失效 |
| 借用检查 | 三模式参数表（默认借用/`inout`/`move`） |
| 闭包捕获分析 | 默认借用 vs `move` 闭包 |
| 区域推断 | 生命周期自动推导 |
| 错误映射 | TIR 错误 → Trust 源码行号列号 |

**Phase 1 所有权子集：**
- `let =` 移动语义
- 函数参数默认借用
- `inout` 可变借用
- `move` 所有权转移
- 闭包默认借用

### 1.5 `trust_codegen` — Rust 代码生成

**工作量：** 2-3 周  
**优先级：** P0  
**依赖：** 1.4

| 子任务 | 说明 |
|--------|------|
| 参数模式映射 | 默认借用 → `&T`、`inout` → `&mut T`、`move` → `T` |
| 函数生成 | Trust `function` → Rust `fn` |
| 控制流生成 | `if`/`for`/`while`/`loop` → Rust 等价物 |
| Source map | `SourceMapping` 结构体 + `// @trust:` 回退模式注释 |
| `fn main()` 包装 | Trust 入口 → Rust `fn main()` |

### 1.6 `trust_error` — 错误诊断

**工作量：** 1 周  
**优先级：** P0  
**依赖：** 1.4

| 子任务 | 说明 |
|--------|------|
| `Diagnostic` 结构体 | 错误/警告/帮助三级 |
| JSON 输出 | `--error-format=json`（对齐设计文档 §9.1.1） |
| `--fix` 模式 | 交互式确认的修复建议（constraints §3.1） |

### 1.7 `trustc` — 编译器入口

**工作量：** 1 周  
**优先级：** P0  
**依赖：** 1.5, 1.6

| 子任务 | 说明 |
|--------|------|
| CLI | `trustc compile`、`trustc check`、`trustc eval` |
| `trust eval` | 无状态表达式求值（设计文档 §9.4），包装为 `fn main()` 编译执行 |
| 编译管线编排 | Parse → HIR → TIR → **错误检查（TIR 错误数=0 才继续）** → Codegen → rustc |
| `Trust.toml` 解析 | 项目配置读取，桥接生成 `Cargo.toml` |

### 1.8 Phase 1 集成测试

**工作量：** 持续  
**优先级：** P0  
**依赖：** 1.7

- 每个语法特性至少一个端到端测试（`.trust` 输入 → `.rs` 快照比较 → `rustc` 编译验证）
- CI 覆盖率门控配置（tarpaulin，`trust_tir` 行覆盖 ≥85%，其余 ≥70%）
- `benches/` 基础目录 + CI 性能回归（基准：编译 5000 行 Trust 代码 ≤60 秒）
- 自举测试（Trust 编译器编译最小 Trust 程序）

**Phase 1 交付标准：** 编译以下程序并执行输出 `"Hello, Trust!"`
```ts
function main() {
    console.log("Hello, Trust!");
}
```

---

## Phase 2：类型系统与泛型 (v0.1.1)

### 2.1 `interface` 与 `type`

**工作量：** 2 周  
**优先级：** P1  
**依赖：** 1.3

- 名义类型检查（`interface` 不可互相赋值）
- 结构别名（`type` 透明等价）
- `{x, y}` 属性简写
- 类型上下文推断

### 2.2 泛型

**工作量：** 3 周  
**优先级：** P1  
**依赖：** 2.1

- 泛型函数声明与调用
- 泛型参数推断
- `extends` 约束（名义 trait + 结构化）
- 隐式 trait 生成（`HasLength` 等）
- 单态化代码生成

### 2.3 ADT（代数数据类型）

**工作量：** 2 周  
**优先级：** P1  
**依赖：** 2.1

- `type Msg = | { kind: "..." } ...` 语法解析
- `switch` 语句 + 穷举检查
- `match` 表达式
- `if let` 语法糖展开
- Rust `enum` 生成

### 2.4 `impl` — Trait 实现

**工作量：** 2 周  
**优先级：** P1  
**依赖：** 2.1, 2.3

- `impl Trait for Type` 语法
- `this: &Self` 隐式参数
- `inout this` / `move this` 方法
- vtable 生成（与 Rust 对齐）

---

## Phase 3：错误处理与 Option/Result (v0.1.2)

### 3.1 `Option<T>` 与 `Result<T,E>`

**工作量：** 1 周  
**优先级：** P1  
**依赖：** 2.3

### 3.2 语法糖

**工作量：** 1 周  
**优先级：** P1  
**依赖：** 3.1

- `?` 操作符（Result 传播）
- `??` 空值合并（`unwrap_or`）
- `?.` 可选链（`map`/`and_then` 自动选择）
- `!` 断言解包（仅 Option）
- `.expect()`

### 3.3 `throw` → `panic!`

**工作量：** 3 天  
**优先级：** P1  
**依赖：** 1.4

---

## Phase 4：并发模型 (v0.2)

### 4.1 `ferro_rt` 运行时库

**工作量：** 3-4 周  
**优先级：** P0  
**依赖：** 1.7, 3.1

| 子任务 | 说明 |
|--------|------|
| `Channel<T>` | `tokio::sync::mpsc` 封装，返回 `(Sender<T>, Receiver<T>)` |
| `shared<T>` | `Arc<Mutex<T>>` / `AtomicI32` 封装 |
| `spawn` | `std::thread::spawn` + `tokio::spawn` |
| `join()` | `tokio::join!` 包装，返回 `Result<(T1,T2), JoinError>` |
| Tokio runtime 初始化 | `#[tokio::main]` 等价物 |
| Feature gate | `default = ["tokio"]`，`sync` feature 用 crossbeam |
| Miri 测试 | `ferro_rt` 的 `unsafe` 块（constraints §3.2 P1） |

### 4.2 `spawn` / `spawn async`

**工作量：** 1 周  
**优先级：** P0  
**依赖：** 4.1, 1.4

- spawn 闭包 `move` 强制检查
- `Send` / `Sync` 自动推导
- 非 async 闭包 → `std::thread::spawn`
- async 闭包 → `tokio::spawn`

### 4.3 `select` 语法

**工作量：** 1 周  
**优先级：** P1  
**依赖：** 4.1, 3.2

- `select { case x = future => ... }` 解析
- 分支内隐式 poll（不写 `await`）
- `Result` 自动匹配 `Ok`，`Err` 静默跳过
- 全分支 `Err` → panic 行为

### 4.4 `shared` + `withLock`

**工作量：** 2 周  
**优先级：** P0  
**依赖：** 4.1

- `shared` 关键字 → `Arc<Mutex<T>>` / `AtomicI32`
- `withLock` 闭包 `&mut T` + auto-deref
- 原子类型优化（`shared number` → `AtomicI32`）
- 嵌套 `withLock` 死锁警告

---

## Phase 5：标准库 (v0.2 — v0.3)

### 5.1 `std::collections`

**工作量：** 2 周  
**优先级：** P1  
**依赖：** 2.2, 2.3

- `Vec`、`HashMap`、`HashSet`、`VecDeque`
- Trust 风格 API（贴近 JS Array/Map 的方法名）

### 5.2 `std::string`

**工作量：** 1 周  
**优先级：** P1  
**依赖：** 2.1

- `split`、`slice`、`replace`、`trim`、`toUpperCase` 等

### 5.3 `std::fs`

**工作量：** 1 周  
**优先级：** P1  
**依赖：** 3.1

- 文件读写、目录遍历、元数据
- 全部返回 `Result`

### 5.4 `std::net`（v0.2）

**工作量：** 2 周  
**优先级：** P2  
**依赖：** 4.2

- TCP/UDP 套接字、HTTP 客户端、TLS
- > **v0.2 阶段采用手写 extern 绑定，v0.2.1 迁移至 bindgen 自动生成。** 手写绑定 API 标注 `unstable` 直到 bindgen 接管。

### 5.5 `std::serde`（v0.2）

**工作量：** 2 周  
**优先级：** P2  
**依赖：** 2.2, 6.3

- JSON / MessagePack 序列化
- 基于 Rust serde 封装
- > **v0.2 阶段采用手写 extern 绑定，v0.2.1 迁移至 bindgen 自动生成。** 手写绑定 API 标注 `unstable` 直到 bindgen 接管。

### 5.6 `std::crypto`（v0.3）

**工作量：** 2 周  
**优先级：** P2  
**依赖：** 5.5

- SHA-256、BLAKE3、对称/非对称加密原语

### 5.7 `std::time` / `std::process`

**工作量：** 1 周  
**优先级：** P1  
**依赖：** 5.3

---

## Phase 6：开发者工具 (v0.2+)

### 6.1 `trust test` — 测试框架

**工作量：** 2 周  
**优先级：** P1  
**依赖：** 1.7

- `test function` → `#[test]` 生成
- `#[should_panic]` 支持
- `#[property]` 属性测试（v0.2+）
- `#[concurrent]` 并发压力测试（v0.3+）
- `trust test --filter` / `--threads`
- 集成测试快照比较

### 6.2 Doctest

**工作量：** 1 周  
**优先级：** P1  
**依赖：** 6.1

- `/// ```trust` 代码块提取
- 编译并作为测试运行

### 6.3 `trust bindgen` — Rust 绑定生成器

**工作量：** 4-6 周  
**优先级：** P1  
**依赖：** 2.2（泛型 + trait）

- 从 `rustdoc` JSON 生成 Trust 类型声明
- 自动推导参数模式（借用 vs move vs inout）
- 处理简单 trait、泛型、`Option`/`Result` 映射

### 6.4 Trust LSP（Language Server）

**工作量：** 6-8 周  
**优先级：** P2  
**依赖：** 1.3（HIR 名称解析），1.6（错误诊断）

- 语法高亮（TextMate grammar）
- 诊断（编译错误实时显示，覆盖 Phase 1 语法子集）
- 跳转定义、查找引用（基于 HIR）
- 自动补全
- Hover 类型信息（Phase 2+ 类型系统稳定后增强）
- `--fix` code action

### 6.4b VS Code 扩展原型

**工作量：** 2-3 周  
**优先级：** P2  
**依赖：** 6.4

### 6.5 `trust fmt` — 格式化工具

**工作量：** 3-4 周  
**优先级：** P2  
**依赖：** 1.2（parser 可用）

### 6.6 `trust doc` — 文档生成

**工作量：** 2-3 周  
**优先级：** P2  
**依赖：** 1.7

- `///` 注释提取，类似 rustdoc
- 标准库 API 文档生成

### 6.7 `trust generate` — 代码生成

**工作量：** 2-3 周  
**优先级：** P2  
**依赖：** 2.4

- 编译前代码生成模板（替代 proc macro，设计文档 §15.4）
- 生成的代码进入版本控制（类似 Go 的 `go generate`）

---

## Phase 7：生态与发布 (v0.3+)

### 7.1 自举

**依赖：** Phase 1-3

自举分三个里程碑：

| 子任务 | 内容 | 工作量 | 优先级 |
|--------|------|--------|--------|
| **7.1a 自举子集定义** | 确定 Trust 编译器中哪些模块优先用 Trust 重写，建立自举测试流水线（Trust 源码 → 阶段1编译器 → 阶段2编译器 → 对比字节码） | 1 周 | P1 |
| **7.1b ferro_rt 自举** | 将 ferro_rt 运行时库用 Trust 重写 | 3 周 | P1 |
| **7.1c 编译器前端自举** | 将 trust_parser + trust_hir 用 Trust 重写 | 8 周 | P2 |

### 7.2 crates.io 发布

**工作量：** 1 周  
**优先级：** P2  
**依赖：** Phase 1 完成

- `cargo publish` 各 workspace crate
- 版本同步（SemVer）
- CHANGELOG 自动生成

### 7.3 文档站

**工作量：** 3 周  
**优先级：** P2  
**依赖：** 6.6

- 《Trust 语言之旅》（tutorial）
- 标准库 API 文档（由 `trust doc` 生成）
- 从 TS 迁移指南
- 与 Rust 互操作指南

### 7.4 包注册中心（Trust Registry）

**工作量：** 远期  
**优先级：** P2

- Trust 原生的 crate 索引
- 直接引用 Cargo 依赖的 `[trust-dependencies]` 机制

---

## 总结：关键里程碑

| 里程碑 | 交付标准 | 预计时间 |
|--------|---------|---------|
| **v0.0 — 规范冻结** | `spec/trust-spec.md` 完成，设计文档/规范/constraints 三者一致 | 第 1 个月 |
| **v0.1 — 最小编译器** | 编译变量/函数/控制流 → Rust 源码 → 可执行二进制 | 第 3-4 个月 |
| **v0.1.1 — 类型系统** | interface、泛型、ADT、impl 可用 | 第 5 个月 |
| **v0.1.2 — 错误处理** | Option/Result + 语法糖（`?`/`??`/`?.`/`!`） | 第 6 个月 |
| **v0.2 — 并发模型** | ferro_rt 运行时 + spawn/Channel/shared/select | 第 8-9 个月 |
| **v0.3 — 生态就绪** | 标准库完整、bindgen 原型、LSP 原型、文档站 | 第 12+ 个月 |

---

> **下一步：** Phase 0.1 —— 编写 `spec/trust-spec.md`（形式化语言规范）。
