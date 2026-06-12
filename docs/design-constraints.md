# Trust 编译器实现规范与约束

> **作用域声明：本文档约束 Trust 编译器的 Rust 实现代码（`.rs` 文件），不约束 Trust 语言源码（`.trust` 文件）。**  
> Trust 语言的语法和语义由 `Trust-设计文档.md` 定义，本文档描述如何用 Rust 正确、一致地实现该规范。  
> 违反 P0/P1 约束的代码不得合并；违反 P2 约束的代码应在 review 中指出并要求修正。

---

## 1. 命名与项目结构

### 1.1 Rust 命名规范

遵循 Rust 社区标准（`rustfmt` + `clippy`）：

| 分类 | 规范 | 示例 |
|------|------|------|
| **crate 名** | `snake_case` | `trust_parser`, `trust_tir`, `ferro_rt` |
| **模块/文件** | `snake_case` | `src/borrow_check/region_inference.rs` |
| **函数/变量** | `snake_case` | `fn check_move_semantics()`, `let hir_node` |
| **类型/trait/enum** | `PascalCase` | `struct HirFunction`, `enum TirOp`, `trait CodegenBackend` |
| **常量/static** | `SCREAMING_SNAKE_CASE` | `const MAX_GENERIC_DEPTH: usize = 32` |
| **宏** | `snake_case!` | `macro_rules! tir_assert` |

### 1.2 Crate 结构

```
crates/
├── trust_parser/          # 词法分析 + 语法分析 → AST
│   ├── src/
│   │   ├── lexer.rs       # Tokenizer
│   │   ├── parser.rs      # 递归下降解析器
│   │   └── ast.rs         # AST 节点定义
├── trust_hir/             # High-level IR（类型检查、名称解析）
│   ├── src/
│   │   ├── hir.rs         # HIR 节点
│   │   ├── typeck.rs      # 类型检查
│   │   └── name_res.rs    # 名称解析
├── trust_tir/             # Trust Intermediate Representation（所有权检查、借用分析）
│   ├── src/
│   │   ├── tir.rs         # TIR 节点（控制流图、基本块）
│   │   ├── borrowck.rs    # 借用检查器
│   │   └── moveck.rs      # 移动语义分析
├── trust_codegen/         # TIR → Rust 源码生成
│   ├── src/
│   │   ├── codegen.rs     # 主代码生成器
│   │   ├── runtime.rs     # ferro_rt API 映射
│   │   └── sourcemap.rs   # Source map 生成
├── trust_error/           # 错误诊断与格式化
│   ├── src/
│   │   ├── diagnostic.rs  # 错误数据结构
│   │   ├── json_fmt.rs    # --error-format=json
│   │   └── fix_suggest.rs # 修复建议引擎
├── ferro_rt/              # 运行时库
│   ├── src/
│   │   ├── channel.rs     # Channel / Sender / Receiver
│   │   ├── shared.rs      # Shared<T> / Atomic 封装
│   │   └── join.rs        # join() 并发等待
└── trustc/                # 编译器入口（main binary）
    └── src/
        └── main.rs        # CLI、编译管线编排
```

> 每个 crate 有独立的 `Cargo.toml`。`trustc` 依赖所有其他 crate。`ferro_rt` 零依赖（或仅依赖 Tokio/crossbeam 的选定 feature）。

### 1.3 设计文档引用约定

所有实现代码必须通过注释引用 `Trust-设计文档.md` 的对应章节号：

```rust
// §3.2.2: match 是表达式，switch 是语句。两者在 HIR 中通过不同的 HirNode 变体表示。
enum HirControlFlow {
    MatchExpr { arms: Vec<HirMatchArm> },   // case X => expr,
    SwitchStmt { cases: Vec<HirSwitchCase> }, // case X: stmt; break;
}
```

**规则：** `trust_tir`、`trust_codegen` 和 `trust_hir` 中的每个 pub 函数或关键 match 分支上方，必须标注对应设计文档的章节引用。

---

## 2. 禁止硬编码与 Magic Number

### 2.1 core

所有字面量数值（除 `0`, `1`, `-1` 用于循环/索引/位移）、字符串字面量必须定义为命名常量：

```rust
// ❌ 错误
if self.generic_depth > 32 { return Err(...); }
let output = format!("fn {}()", name);

// ✅ 正确
const MAX_GENERIC_DEPTH: usize = 32;
if self.generic_depth > MAX_GENERIC_DEPTH { return Err(...); }
const FN_KEYWORD: &str = "fn";
let output = format!("{} {}()", FN_KEYWORD, name);
```

**豁免：** 以下公认常量可裸写，不需命名：
- 数组/向量初始容量：`Vec::with_capacity(16)`
- 缓冲区大小（Rust 标准约定）：`[0u8; 1024]`、`4096`
- 错误码偏移：`ExitCode::from(1)`
- 测试断言值：`assert_eq!(result, 42)`

### 2.2 禁止硬编码 Trust 语法字符串

```rust
// ❌ 错误 — 硬编码 Trust 语法
let output = "function main(): void {".to_string();
let output = "import { Channel } from \"std::sync\";".to_string();

// ✅ 正确 — 通过 codegen 模块的模板方法生成
let output = codegen.emit_function_decl(&hir_func)?;
let output = codegen.emit_import(&["Channel"], "std::sync")?;
```

**规则：** 所有 Trust 语法字符串的拼接必须在 `trust_codegen` crate 中通过结构化方法生成。禁止在 `trust_tir`、`trust_hir`、`trust_parser` 中直接拼接 Trust 或 Rust 语法字符串。

---

## 3. 编译器 Rust 代码规范

### 3.1 错误处理

编译器内部使用 `Result<T, E>` 传播错误。使用 `thiserror` 定义错误类型：

```rust
// ✅ 正确
#[derive(Debug, thiserror::Error)]
pub enum TirError {
    #[error("variable `{name}` moved at {at_line} and used at {use_line}")]
    UseAfterMove { name: String, at_line: u32, use_line: u32 },
    
    #[error("cannot borrow `{name}` as mutable: already borrowed at {at_line}")]
    MutableBorrowConflict { name: String, at_line: u32 },
}

fn check_move(&self, node: &TirNode) -> Result<(), TirError> { ... }
```

**规则：**
- `trust_hir`、`trust_tir`、`trust_codegen` 的错误类型必须实现 `std::error::Error` + `Display`
- 禁止在这些 crate 中使用 `unwrap()` 或 `expect()`。使用 `?` 传播错误
- 仅在以下场景允许 `expect()`：编译器内部逻辑不变量（如"parser 必须产出完整的 AST"），并附带 `// SAFETY:` 注释

#### 3.1.1 错误收集策略

编译器不应在第一个错误处立即终止。各阶段采用不同的收集策略：

- **Parser 层：** 遇到语法错误后进入 **panic mode**，同步到下一个 statement/function 边界，继续解析以收集更多错误。
- **Typeck / TIR 层：** 函数级独立检查，错误收集到 `Vec<Diagnostic>` 后统一报告。同一个函数内的多个错误一次性暴露，不同函数之间互不影响。

```rust
// ✅ 正确 —— 收集所有错误后统一报告
fn check_module(&mut self, module: &HirModule) -> Result<(), Vec<TypeError>> {
    let mut errors = Vec::new();
    for func in &module.functions {
        if let Err(e) = self.check_function(func) {
            errors.push(e);
        }
    }
    if errors.is_empty() { Ok(()) } else { Err(errors) }
}
```

> 单个函数内部遇到首个不可恢复的错误（如类型不匹配导致后续分析无意义）可用 `?` 提前终止该函数的检查，但不影响其他函数。

### 3.2 Unsafe 使用

```rust
// ✅ 正确
/// 直接写入 DWARF 调试信息缓冲区。
/// 
/// # Safety
/// 
/// 调用者必须保证 `buf` 长度 >= `DWARF_LINE_PROGRAM_HEADER_SIZE`，
/// 且当前 TIR 节点的 source span 已通过 `validate_span` 校验。
unsafe fn write_dwarf_header(buf: &mut [u8], span: SourceSpan) { ... }
```

**规则：**
- `unsafe` 块/函数必须附带 `// Safety:` 段落，说明被满足的前提条件
- `unsafe` 仅允许在 `trust_codegen`（DWARF 写入）和 `ferro_rt`（异步 executor 绑定）中使用
- `trust_hir`、`trust_tir`、`trust_parser` 中**禁止任何 `unsafe`**

**`unsafe` 测试分级：**
| Crate | `unsafe` 是否允许 | Miri 测试要求 | 替代验证 |
|-------|------------------|-------------|---------|
| `ferro_rt` | 是 | **P1：每个 `unsafe` 必须有 Miri 测试** | — |
| `trust_codegen` | 是（DWARF 写入） | P2：推荐 Miri | 单元测试 + 代码审查（DWARF 字节操作 Miri 检测价值有限） |
| 其余 5 个 crate | 否 | 不需要 | — |

### 3.3 分配感知

```rust
// ✅ 正确 — 预分配已知大小的容器
fn collect_errors(diagnostics: &[Diagnostic]) -> Vec<TirError> {
    let mut errors = Vec::with_capacity(diagnostics.len());
    ...
}

// ⚠️ 避免 — 热路径上的 Box<dyn Trait>
// TIR 遍历使用 enum 分发而非 trait object
fn visit_tir_node(node: &TirNode) -> Result<(), TirError> {
    match node.kind {
        TirKind::Assign(lhs, rhs) => { ... }  // 静态分发
        TirKind::Call(func, args) => { ... }
    }
}
```

### 3.4 依赖管理

- `ferro_rt` 运行时库：允许依赖 `tokio`（默认）、`crossbeam`（可选 feature）
- 编译器其余 crate：禁止引入 `ferro_rt` 以外的外部 crate，除非经过架构评审
- 所有 crate 版本在 workspace `Cargo.toml` 中统一管理，禁止独立引入不同版本的同一依赖

---

## 4. 所有权与并发

### 4.1 Rust 所有权使用

编译器代码是 Rust 代码，遵循 Rust 所有权规则：

```rust
// ✅ 正确 — 借用而非 clone（编译器内部热路径）
fn check_function_signature(func: &HirFunction) -> Result<(), TypeError> {
    for param in &func.params {          // 借用
        self.check_type(&param.ty)?;     // 借用
    }
    Ok(())
}

// ❌ 避免 — 热路径上的 clone
fn check_function_signature(func: HirFunction) -> Result<(), TypeError> {
    let params = func.params.clone();    // 不必要的 clone
    ...
}
```

### 4.2 并行编译

```rust
// TIR 检查可对每个函数独立运行，天然适合并行
use rayon::prelude::*;

// 收集所有函数的错误，而非遇到第一个错误就终止
fn check_all_functions(tir: &TirModule) -> Vec<TirError> {
    tir.functions
        .par_iter()                        // rayon 并行迭代
        .filter_map(|func| self.check_function(func).err())
        .collect()
}
```

**规则：** 利用 Rayon 在 TIR 层并行检查独立的函数/模块。使用 `filter_map` + `collect` 收集所有错误（非短路），确保用户一次性看到全部问题。不在 `trust_parser` 阶段并行（解析是顺序瓶颈）。

### 4.3 禁止事项

- ❌ 禁止在编译器代码中使用全局可变状态（`static mut`、`lazy_static!` + `Mutex`）
- ❌ 禁止在测试之外使用 `unwrap()` 或 `expect()` 处理 `Result`

---

## 5. 编译器测试

### 5.1 测试类型

| 测试类型 | 位置 | 覆盖目标 |
|---------|------|---------|
| 单元测试 | 与源码同文件 `#[cfg(test)] mod tests` | 每个 pub 函数的 happy path + 错误路径 |
| 集成测试 | `tests/` 目录 | 完整编译管线（`.trust` 输入 → Rust 输出比较） |
| 快照测试 | `trust_parser/tests/snapshots/` | AST/HIR/TIR 输出与预期快照比对 |
| 模糊测试 | `fuzz/` 目录 | 随机 `.trust` 输入验证编译器不崩溃 |

### 5.2 测试命名

```rust
// ✅ 正确
#[test]
fn parse_function_with_inout_param_returns_correct_ast() { ... }

#[test]
fn borrowck_rejects_mutable_borrow_while_immutable_exists() { ... }

// ❌ 错误
#[test]
fn test1() { ... }
```

**规则：** `{subject}_{condition}_{expected}` 模式。编译器测试不需要 `snake_case` 对 `camelCase` — 所有标识符遵循 Rust 标准 `snake_case`。

### 5.3 覆盖率

| 要求 | 阈值 |
|------|------|
| `trust_tir` 的行覆盖率 | ≥ 85%（所有权/借用检查是正确性关键） |
| 其余 crate 的行覆盖率 | ≥ 70% |
| 分支覆盖率 | ≥ 60% |

### 5.4 Doctest

```rust
/// 验证函数参数默认借用语义（对应 Trust-设计文档.md §4.3）
///
/// ```
/// use trust_hir::typeck::check_param_mode;
/// use trust_hir::hir::{HirParam, Type};
/// let param = HirParam::new("x", Type::i32(), None); // 无 inout/move 标注
/// let mode = check_param_mode(&param).unwrap();
/// assert_eq!(mode, ParamMode::ReadOnlyBorrow);
/// ```
pub fn check_param_mode(param: &HirParam) -> Result<ParamMode, TypeError> { ... }
```

**规则：** `trust_tir` 中的所有 pub 函数必须有 doctest。其余 crate 的 pub 函数推荐有。

### 5.5 集成测试规范

每个 `.trust` 编译器特性必须有一个端到端集成测试：

```
tests/
├── integration/
│   ├── basic_variable.trust      # 输入
│   ├── basic_variable.rs         # 期望的 Rust 输出
│   ├── closure_move.trust
│   ├── closure_move.rs
│   ├── channel_spawn.trust
│   └── channel_spawn.rs
```

测试运行器编译 `.trust` 文件，比较生成的 Rust 代码与 `.rs` 快照文件，再编译 Rust 代码验证可通过 `rustc`。

---

## 6. TIR 层实现规范

### 6.1 TIR 节点

```rust
/// TIR 控制流图节点（对应 Trust-设计文档.md §9.1，spec SEM-REQ-001）
enum TirNode {
    /// let x = expr; （对应 设计文档 §4.1 移动语义，spec OWN-REQ-001）
    Let { name: Symbol, init: TirExpr, mutable: bool },
    /// function foo(inout x: T) { ... } （对应 设计文档 §4.3，spec OWN-REQ-002）
    Function { name: Symbol, params: Vec<TirParam>, body: TirBlock },
    /// spawn(move || { ... }) （对应 设计文档 §5.1，spec CON-REQ-002）
    Spawn { closure: TirClosure, is_async: bool },
    /// shared counter = 0; counter.withLock(c => ...) （对应 设计文档 §5.3，spec CON-REQ-003）
    Shared { name: Symbol, init: TirExpr },
    WithLock { shared: Symbol, body: TirClosure },
}
```

**规则：** 每个 TIR 节点变体上方必须注释对应的设计文档章节号。

> **范围说明：** 以上仅列出所有权/并发相关的 TIR 节点（5 个变体）。完整的 AST→TIR 降级覆盖 spec SEM-REQ-001 的全部 13 个 AST 节点——控制流（`IfExpr`、`ForStmt`、`LoopExpr` 等）在 HIR→TIR 时降级为基本块 + 条件跳转（见 spec SEM-REQ-004），非所有权相关的节点不在此列出。

### 6.2 Borrow Checker

借用检查器实现 Trust 的三模式参数表（§4.3）：默认只读借用、`inout` 可变借用、`move` 所有权转移。核心算法：对每个 TIR 基本块进行**区域推断（region inference）**和**数据流分析**。

```rust
/// 借用检查入口（设计文档 §4.3 三模式参数表，spec OWN-REQ-002）
fn check_function_borrows(&self, func: &TirFunction) -> Result<(), TirError> {
    for param in &func.params {
        match param.mode {
            ParamMode::ReadOnly => self.check_immutable_borrow(param)?,   // OWN-REQ-003
            ParamMode::InOut => self.check_mutable_borrow(param)?,       // OWN-REQ-003
            ParamMode::Move => self.check_ownership_transfer(param)?,     // OWN-REQ-001
        }
    }
    Ok(())
}
```

**规则：** 
- 借用检查器的每个检查步骤必须注释对应的 spec REQ-ID（OWN-REQ-001~008）
- 错误信息必须映射回 Trust 源文件的**行号和列号**（通过 source span）
- 不允许暴露 TIR 内部名称（如 `_tir_borrow_14`）到错误信息中

#### 6.2.1 for 循环隐式可变（spec OWN-REQ-007）

`for (let i = 0; i < N; i++)` 中 `i` 为隐式可变——这是 Trust 中唯一允许 `let` 声明的变量被修改的场景。TIR 层在对 `for` 循环降级时，自动将 `i` 标记为 `mutable = true`：

```rust
fn lower_for_loop(init: &TirExpr, cond: &TirExpr, update: &TirExpr, body: &TirBlock) -> TirBlock {
    // C-style for 的迭代变量强制标记为 mutable（OWN-REQ-007）
    if let TirExpr::Let { name, init, .. } = init {
        self.scope.insert(name, TirVar { mutable: true, .. });
    }
    // ... 降级为 loop + if break 的基本块
}
```

**规则：** `for-of` 和 `while` 循环不受此例外影响——它们的迭代变量遵循标准 `let` 不可变规则。

#### 6.2.2 Copy 类型判定（spec OWN-REQ-008）

编译器在 HIR→TIR 降级时自动判定类型是否实现 `Copy` trait：

```rust
fn is_copy_type(ty: &Type) -> bool {
    match ty {
        Type::I32 | Type::F64 | Type::Bool | Type::BigInt => true,  // 标量类型
        Type::Ref(_) => true,                                        // 引用总是 Copy
        Type::Tuple(elems) => elems.iter().all(|e| is_copy_type(e)), // 元素全 Copy
        Type::Array(elem, _) => is_copy_type(elem),                  // 固定数组元素 Copy
        Type::Vec(_) | Type::String | Type::Box(_)
            | Type::Rc(_) | Type::Arc(_) => false,                   // 堆分配类型非 Copy
        _ => false,                                                  // 保守：用户类型默认非 Copy
    }
}
```

`Copy` 类型在 `let b = a` 时不触发移动语义（`a` 后续仍可用），非 `Copy` 类型触发 OWN-REQ-001。

### 6.3 区域推断（Region Inference）

生命周期自动推导实现（设计文档 §4.4，spec OWN-REQ-009）：

- 函数参数 → 返回值：如果返回引用类型，自动将返回值生命周期绑定到参数
- 大多数场景不需要标注，仅在返回引用或自引用结构中需要
- TIR 层检测生命周期不足时，生成 rustc 风格的生命周期标注并提示用户

**回退策略：** 当 TIR 层无法完全推断生命周期关系时（如高阶生命周期多态、自引用结构），编译器不放弃——回退为生成带有显式生命周期标注的 Rust 代码，依赖 rustc 进行最终验证。错误信息通过 source map（§7.2）映射回 Trust 源码行。此策略确保 Trust 编译器在 v0.1 即可覆盖 95%+ 的场景，剩余极端情况由 rustc 保底，而非阻塞编译。随着 TIR 成熟度提升，回退触发频率应逐步降低至 0。

---

## 7. 代码生成规范

### 7.1 Rust 代码生成

`trust_codegen` 从经过 TIR 验证的 TIR 图生成 Rust 源码。核心约束：**生成的 Rust 代码必须保证通过 rustc 检查（soundness by construction）**。

```rust
fn emit_param(param: &TirParam) -> String {
    match param.mode {
        ParamMode::ReadOnly => format!("{}: &{}", param.name, emit_type(&param.ty)),       // &T
        ParamMode::InOut    => format!("{}: &mut {}", param.name, emit_type(&param.ty)),   // &mut T
        ParamMode::Move     => format!("{}: {}", param.name, emit_type(&param.ty)),        // T
    }
}
```

#### 7.1.1 隐式 Trait 生成（对应 spec TYP-REQ-007）

当 Trust 泛型约束使用结构化类型 `T extends { field: Type }` 时，编译器在 codegen 阶段自动生成隐式 trait 并实现：

```rust
// Trust 源：function first<T extends { length: number }>(x: T): number { ... }
//
// 编译器自动生成：
trait HasLength {           // 隐式 trait，命名规则：Has{FieldName}
    fn length(&self) -> usize;
}

impl<T> HasLength for Vec<T> {             // 向量 → len()
    fn length(&self) -> usize { self.len() }
}
impl HasLength for String {                // 字符串 → len()
    fn length(&self) -> usize { self.len() }
}
impl<T, const N: usize> HasLength for [T; N] { // 固定数组 → 编译时常量
    fn length(&self) -> usize { N }
}
```

**规则：**
- 隐式 trait 仅用于结构化 `extends` 约束（`T extends { field: Type }`），不用于名义 `extends Interface` 约束
- 内置类型的自动 `impl` 仅覆盖 `Vec<T>`、`String`、`[T; N]`、`&[T]`、`HashMap<K,V>`
- 用户自定义类型若需满足该约束，可手动 `impl HasFieldName for MyType`（孤儿规则适用）
- 隐式 trait 生成在 HIR 类型检查阶段（`trust_hir::typeck`），非 codegen 阶段——codegen 仅消费已解析的 trait 信息

### 7.2 Source Map

```rust
struct SourceMapping {
    trust_file: PathBuf,
    trust_line: u32,
    trust_col: u32,
    rust_file: PathBuf,
    rust_line: u32,
    rust_col: u32,
}
```

**规则：** 每个 TIR → Rust 的映射必须保存在 source map 中。生成的 Rust 代码中嵌入 `// @trust: src/main.trust:42:15` 风格的注释（回退模式，v0.1），后续版本生成 DWARF。

### 7.3 禁止特性

- ❌ 禁止生成包含 `unsafe` 块的 Rust 代码，除非经过架构评审且附带安全论证
- ❌ 禁止生成依赖特定 Rust 编译器版本（nightly only）的代码——目标为 stable Rust
- ❌ 禁止生成硬编码路径或特定环境的 Rust 代码

---

## 8. 错误诊断规范

### 8.1 错误结构

```rust
struct Diagnostic {
    level: Level,           // Error | Warning | Help
    code: ErrorCode,        // 如 E0382（移动后使用）
    message: String,        // 人类可读
    spans: Vec<SourceSpan>, // 主错误位置
    children: Vec<Diagnostic>, // 辅助信息（如修复建议）
    fix_suggestion: Option<String>, // --fix 模式的修复建议
}
```

### 8.2 JSON 格式（§9.1.1）

输出格式与设计文档声明一致：

```json
{
  "message": "变量 `data` 在第 12 行被移动后在第 15 行被使用",
  "level": "error",
  "code": "E0382",
  "spans": [
    {
      "file": "src/main.trust",
      "line_start": 12,
      "line_end": 12,
      "col_start": 5,
      "col_end": 9,
      "label": "data 在此处被移动"
    }
  ],
  "children": [
    { "message": "考虑在此处使用 data.clone()", "level": "help" }
  ]
}
```

### 8.3 错误信息映射

错误信息中**只能**引用 Trust 源码中的变量名、函数名和行号。禁止在用户可见的错误信息中暴露 TIR 内部名、Rust 生成的变量名或 rustc 的内部结构。

---

## 9. ferro_rt 运行时库

### 9.1 依赖策略

```toml
[features]
default = ["tokio"]                    # 默认启用 Tokio 异步运行时
tokio = ["dep:tokio"]
sync = ["dep:crossbeam"]               # 同步 Channel 可选后端

[dependencies]
tokio = { version = "1", features = ["sync", "rt", "macros"], optional = true }
crossbeam = { version = "0.8", optional = true }
```

**规则：** Tokio 作为默认异步运行时（`default = ["tokio"]`）。`crossbeam` 作为可选同步 Channel 后端（feature `sync`）。未启用任何 feature 时 ferro_rt 不提供 Channel 实现（编译时报错提示启用 feature）。

### 9.2 核心 API 实现映射

| Trust API | Rust 实现 | 设计文档 |
|-----------|----------|---------|
| `Channel<T>(cap)`（async） | `tokio::sync::mpsc::channel(cap)` — 返回 `(Sender<T>, Receiver<T>)` | §5.2 |
| `Channel<T>(cap)`（sync） | `crossbeam::channel::bounded(cap)`（feature = `sync`） | §5.2 |
| `shared x = init` | `Arc<AtomicI32>` / `Arc<Mutex<T>>` | §5.3 |
| `spawn(move \|\| ...)` | `std::thread::spawn` | §5.1 |
| `spawn(move async { ... })` | `tokio::spawn` | §5.1 |
| `join(f1, f2)` | `ferro_rt::join(f1, f2)` — 包装 `tokio::join!`，返回 `Result<(T1,T2), JoinError>` | §5.1.1 |

> **`join` 说明：** `ferro_rt::join` 是自定义函数，内部调用 `tokio::join!` 并发 poll 两个 Future。对外暴露 `Result<(T1, T2), JoinError>` 接口以支持 `?` 操作符。若所有 Future 成功返回 `Ok((t1, t2))`；若任一 panic 或被取消，返回 `Err(JoinError)`。
>
> **Channel 运行时说明：** 设计文档 §9.2 早期提及 crossbeam / std::sync::mpsc，但 Trust 的 `await tx.send()`（§5.2）需要 async channel。因此默认 async 场景使用 Tokio，同步场景通过 `sync` feature 使用 crossbeam。

### 9.3 Unsafe 使用

仅在以下场景允许 `unsafe`：
- Tokio runtime 初始化（调用 `tokio::runtime::Builder`）
- 原子操作封装（`AtomicI32` 等，实际由标准库实现，ferro_rt 仅暴露安全封装）
- 自定义异步 waker（如需要）

`ferro_rt` 中的每个 `unsafe` 必须附带 `// SAFETY:` 注释并经过 Miri 测试（P1，见 §3.2 分级表）。`trust_codegen` 中的 `unsafe`（DWARF 写入）推荐 Miri 测试，但可用单元测试 + 代码审查替代（P2）。

---

## 10. 约束优先级速查

| 优先级 | 类别 | 示例 |
|--------|------|------|
| **P0** | 错误处理 | 编译器中禁止 `unwrap()`/`expect()` 在非不变量场景；使用 `?` |
| **P0** | Unsafe | `unsafe` 必须附带 `// SAFETY:` 注释；`trust_hir`/`trust_tir`/`trust_parser` 中禁止 `unsafe` |
| **P0** | Doctest | `trust_tir` 的 pub 函数必须有 doctest |
| **P0** | 设计文档引用 | TIR/Codegen 关键结构必须有设计文档章节注释 |
| **P0** | 错误映射 | 编译器错误信息不能暴露 TIR 内部名或 Rust 内部名 |
| **P0** | Magic Number | 编译器 Rust 代码的所有字面量必须是命名常量（公认常量豁免） |
| **P1** | Unsafe 测试 | `ferro_rt` 的 `unsafe` 块必须有 Miri 测试（`trust_codegen` 推荐，见 §3.2） |
| **P1** | 集成测试 | 每个 Trust 语言特性必须有端到端集成测试 |
| **P1** | 依赖 | 禁止独立引入不同版本的同名依赖 |
| **P1** | 并行 | TIR 检查必须使用 Rayon 并行 |
| **P2** | 命名 | `snake_case` 函数/变量，`PascalCase` 类型，`SCREAMING_SNAKE_CASE` 常量 |
| **P2** | 覆盖率 | `trust_tir` 行覆盖 ≥ 85%，其余 ≥ 70% |
| **P2** | 快照测试 | AST/HIR/TIR 输出需有快照 |

---

> **本文档是 Trust 编译器 Rust 实现的工程宪法。** 所有 PR 必须通过 `cargo test`、`cargo clippy -- -D warnings`、`cargo fmt --check`。
> - `ferro_rt` 的 PR 额外需要 `cargo miri test`（针对 `unsafe` 块，P1）。
> - `trust_tir` 的 PR 额外需要 `cargo tarpaulin`（行覆盖率 ≥ 85%）。
> - 对于 tarpaulin 在复杂 workspace 中的已知问题（fork、多 crate 聚合），使用 `--skip-clean` 和手动 crate-by-crate 覆盖率收集。

---

## 11. 工程基础设施

### 11.1 CI/CD

```yaml
# .github/workflows/ci.yml 核心 job
jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - run: cargo test --workspace
      - run: cargo clippy --workspace -- -D warnings
      - run: cargo fmt --check --all
      - run: cargo miri test -p ferro_rt          # P1: unsafe 块
      - run: cargo tarpaulin -p trust_tir --fail-under 85
```

**规则：** PR merge 必须通过 CI 所有 job。`miri` 和 `tarpaulin` job 允许在 nightly toolchain 下运行。

### 11.2 MSRV（最低支持的 Rust 版本）

编译器自身使用 **stable Rust** 编译。MSRV 跟随 workspace `Cargo.toml` 中的 `rust-version` 字段。`ferro_rt` 的 Tokio feature 要求 Rust ≥ 1.63（`tokio::sync::mpsc` 稳定化版本）。

### 11.3 版本号与发布

- 编译器版本遵循 **SemVer**（`MAJOR.MINOR.PATCH`）
- workspace 所有 crate 版本同步（发布时统一 bump）
- 发布到 crates.io 前必须：
  - `cargo publish --dry-run` 通过
  - 自举测试通过（Trust 编译器用自己编译 `trust_std`）
  - `CHANGELOG.md` 更新

### 11.4 CHANGELOG

遵循 [Keep a Changelog](https://keepachangelog.com/) 格式。分类：`Added`、`Changed`、`Fixed`、`Removed`。每个条目引用相关 issue/PR 号。

### 11.5 错误恢复策略

- **Parser：** panic mode — 跳过 token 直到同步点（`;`、`}`、`function`、`import`），然后恢复解析。
- **Typeck：** 类型错误后继续检查函数剩余部分（错误类型用 `Type::Error` 哨兵占位以避免级联报错）。
- **TIR：** 所有权错误后函数级终止（后续分析已无意义），但继续检查其他函数。
- **Codegen：** 仅在 TIR 无错误时运行。若 TIR 有错误，不生成代码。

### 11.6 Fuzzing

```toml
# fuzz/Cargo.toml
[package]
name = "trust-fuzz"

[dependencies]
libfuzzer-sys = "0.4"

[dependencies.trust_parser]
path = "../crates/trust_parser"

[[bin]]
name = "parse"
path = "fuzz_targets/parse.rs"
```

**规则：** Fuzz 目标至少覆盖 parser（随机 `.trust` 输入不 panic）和 codegen（随机 TIR 图不 panic）。语料库从现有集成测试的 `.trust` 文件初始化。

### 11.7 性能目标

v0.1 阶段无具体毫秒目标。唯一要求：编译器自身编译 `trust_std`（约 5000 行 Trust 代码）在 60 秒内完成（冷启动）。具体性能基准在 v0.2 引入 `benches/` 目录后制定。
