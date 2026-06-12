# Phase 0 — 语言规范冻结  TODO

> 目标：将 `docs/Trust-设计文档.md` 转化为可执行的、无歧义的形式化语言规范。  
> 期限：第 1 个月  
> 优先级：P0（阻塞所有后续 Phase）

---

## 0.1 编写 Trust Language Specification

**产出物：** `spec/trust-spec.md`（或 `spec/` 目录下多文件）  
**工作量：** 2-3 周

### 0.1.1 词法规范

- [x] Token 定义（关键字列表、字面量格式、注释格式）
- [x] 关键字完整列表与分类：
  - **保留关键字：** `let` `mut` `const` `function` `inout` `move` `shared` `spawn` `async` `await` `if` `else` `for` `of` `while` `loop` `break` `continue` `return` `throw` `switch` `case` `default` `match` `select` `import` `export` `from` `as` `interface` `type` `impl` `test` `extern` `this` `extends` `fn` `true` `false` `undefined` `None` `Some` `Ok` `Err` `Rc` `Arc` `Weak` `Box` `dyn`
  - **类型名标识符：** `number` `string` `boolean` `bigint` `void`
  - **值关键字：** `undefined` `true` `false` `None` `Some` `Ok` `Err`
- [x] 字面量格式：
  - `number`：整数默认 `i32`，浮点默认 `f64`，禁止隐式混用
  - `bigint`：十进制数字后跟 `n`（如 `9007199254740991n`）
  - `string`：双引号 + 模板字符串
  - `boolean`：`true` / `false`
- [x] 注释格式：`//` 行注释、`/* */` 块注释、`///` 文档注释
- [x] 运算符和分隔符完整列表
- [x] **运算符优先级与结合性表**（含 `await` 中缀、`?.` 链式、`??` 空值合并、`!` 后缀断言、`!` 前缀逻辑非、`&` 引用、`=>` 箭头、`as` 转换等）
- [x] **显式类型转换语法**：`expr as T`

### 0.1.2 语法规范

- [x] 形式化 EBNF 语法定义（或类似形式化格式）
- [x] **Parser 错误恢复策略与同步点定义**（panic mode，同步点：`;` `}` `function` `import` `export` `type` `interface` `impl` `test` `async`）
- [x] 变量声明：`let` / `let mut` / `const` / **`shared`**（独立条目）
- [x] 函数声明：`function` 签名 + 单表达式简写 `=`
- [x] 箭头函数：`(params) => expr` / `(params) => { body }`
- [x] 控制流：`if` / `else` / `for` / `for-of` / `while` / `loop`
- [x] 模式匹配：`switch` 语句 + `match` 表达式 + `if let`
- [x] 异步：`async function` / `await` / `spawn` / `spawn async`
- [x] 模块：`import` / `export` / `export default`
- [x] 类型标注语法：`x: Type`
- [x] 属性简写：`{ x, y }`
- [x] 空值糖：`??` / `?.` / `!`
- [x] 泛型：`<T>` / `extends` 约束
- [x] ADT：`type X = | { kind: "A" } | { kind: "B" }`
- [x] 结构体/接口：`interface` / `type` 别名 / `impl ... for ...`
- [x] 闭包：`() =>` / `move () =>`
- [x] 并发：`shared` / `withLock` / `Channel` / `select`
- [x] FFI：`extern "rust" { fn ... }`（含 `fn` 语法、`...args` 变长、泛型参数细节）
- [x] 属性：`#[test]` / `#[should_panic]` / `#[property]` / `#[concurrent]`
- [x] 生命周期标注：`<'a>`（返回引用、返回含引用结构、关联参数与返回值生存期时使用；绝大多数场景自动推导）
- [x] 引用：`&` 运算符
- [x] 测试：`test function` / `#[test]`
- [x] **`::` 构造器/关联函数访问**：`Type::ident`（`Box::new`、`Dynamic.Number`、`Vec::new` 等）
- [x] **分号与分隔符规则**：换行即分隔（`;` 可选）；`for` 子句 `;` 分隔（语法要求）；`match` 分支 `,` 分隔（末尾可选）；块内最后表达式省略 `;` 作为返回值

### 0.1.3 语义规范

- [x] **AST 节点完整定义**（parser 输出，含 `if`/`loop`/`match` 表达式 vs `switch`/`for`/`while` 语句区分、`break` 带返回值仅限 `loop`）
- [x] **HIR 节点完整定义**（符号表、作用域结构、名称解析）
- [x] **AST → HIR 降级规则**（名称解析、模块路径解析、`import`/`export` 语义）
- [x] **HIR 层类型检查与推断规则**（名义类型检查、结构别名等价、闭包参数推断、单表达式返回值推断、泛型参数调用点推断）
- [x] **HIR → TIR 降级规则**（控制流图、基本块转换、表达式→语句转换）

### 0.1.4 类型系统规范

- [x] 名义类型（`interface`）规则
- [x] 结构别名（`type { ... }`）透明等价规则
- [x] ADT（`type | ...`）标签联合规则
- [x] **数字类型规则**：`number` 单一类型，整数默认 `i32`，浮点默认 `f64`，**禁止混合运算**，用 `as` 显式转换
- [x] 泛型单态化规则
- [x] trait bound（`extends Interface` vs `extends { ... }` 结构化约束）
- [x] 隐式 trait 生成规则（`HasLength` 等）
- [x] `this: &Self` / `inout this` / `move this` 隐式参数规则
- [x] **`Dynamic` 枚举类型规则**（栈分配、变体构造、模式匹配穷举）
- [x] **`Box<dyn Trait>` 类型规则**（vtable 分发、与泛型单态化区分场景）
- [x] **`??` 运算符类型规则**（`Option<T> ?? T → T`、`Result<T,E> ?? T → T`，映射 `unwrap_or`）
- [x] **`?.` 运算符类型规则**（`Option` 字段时 `and_then` vs 非 `Option` 字段时 `map` 自动选择；owned 上下文 move 语义）
- [x] **`Send` / `Sync` 的自动推导类型规则**（基于字段组成的类型推导）

### 0.1.5 所有权规则规范

- [x] 移动语义形式化定义：`let b = a;` → `a` 失效
- [x] 三模式参数表形式化定义：

  | 声明 | 语义 | Rust mapping |
  |------|------|-------------|
  | `f(x: T)` | 只读借用 | `f(x: &T)` |
  | `f(inout x: T)` | 可变借用 | `f(x: &mut T)` |
  | `f(move x: T)` | 所有权转移 | `f(x: T)` |

- [x] 借用规则：同一时刻 ≤1 可变借用 或 ≥0 只读借用
- [x] **不可变绑定的方法调用限制**：`let`（非 `mut`）变量只能调用 `&self` 方法
- [x] **可变绑定的方法调用规则**：`let mut` 可调用所有方法，调用期间独占借用
- [x] 闭包捕获规则：默认只读借用，`move` → FnOnce
- [x] **引用计数所有权规则**：`Rc::new`/`Arc::new` 创建；`clone()` 增引用；`Weak::upgrade`；`Rc<T>` 非 `Send` 不能跨线程
- [x] 区域推断（Region Inference）算法伪代码
- [x] `Copy` 类型判定规则
- [x] **`for (let i = 0; ...; i++)` 隐式可变例外**：`i` 允许 `i++`，不可变默认的唯一例外

### 0.1.6 并发规则形式化

- [x] **`Send` / `Sync` 的并发使用侧检查规则**：`spawn` 要求 `move` + `Send`；`shared` 要求 `Sync`；`Channel<T>` 要求 `T: Send`
- [x] `spawn` 要求 `move` + `Send` 的规则
- [x] `shared` → `Arc<Mutex<T>>` / `AtomicI32` 优化规则
- [x] `Channel` → `(Sender<T>, Receiver<T>)` 分离规则
- [x] `select` 隐式 poll + `Result` auto-unwrap 规则

### 0.1.7 错误处理规则

- [x] `Result<T, E>` 与 `?` 传播规则
- [x] `throw Error` → `panic!` 映射
- [x] `!` 仅限 `Option<T>` 的编译器检查规则
- [x] `.expect()` 语义

---

## 0.2 编写 Trust 标准库规范

**产出物：** `spec/stdlib.md`  
**工作量：** 1 周  
**依赖：** 0.1 完成基本类型定义

- [x] `std::collections` API 签名（`Vec` `HashMap` `HashSet` `VecDeque`）
- [x] `std::sync` API 签名（`Channel` `shared` `spawn` `Mutex` `RwLock` `Atomic`）
- [x] `std::async` API 签名（`join` `sleep`、异步 I/O 原语、Tokio 绑定）
- [x] `std::result` 类型定义（`Option<T>` / `Result<T,E>`）
- [x] `std::string` API 签名（`split` `slice` `replace` `trim` `toUpperCase`）
- [x] `std::fs` API 签名（读写、目录遍历、元数据）
- [x] `std::rc` API 签名（`Rc` `Arc` `Weak`）
- [x] `std::time` API 签名（`Duration` 定时器）
- [x] `std::process` API 签名（子进程管理、环境变量）— v0.2 目标
- [x] `std::net` API 签名（TCP/UDP/HTTP/TLS）— v0.2 目标
- [x] `std::serde` API 签名（JSON/MessagePack）— v0.2 目标
- [x] `std::crypto` API 签名（SHA/BLAKE3/加密）— v0.3 目标
- [x] 模块间依赖图
- [x] Trust API → Rust 标准库映射关系表

---

## 0.3 设计文档与规范一致性审计

**工作量：** 3 天  
**依赖：** 0.1, 0.2

- [ ] `docs/Trust-设计文档.md` ↔ `spec/trust-spec.md` 交叉验证
  - [ ] 所有设计文档中描述的语法特性在规范中有形式化定义
  - [ ] 所有代码示例的语法与规范一致
  - [ ] 所有权规则描述与规范中的形式化定义一致
  - [ ] 并发模型描述与规范中的形式化定义一致
- [ ] `spec/trust-spec.md` ↔ `docs/design-constraints.md` 交叉验证
  - [ ] 规范中的 TIR 节点与 constraints 中的实现节点对应
  - [ ] 规范中的 API 映射与 constraints §9.2 的 ferros_rt 映射一致
  - [ ] 错误格式与 constraints §8 的结构体一致
- [ ] `docs/Trust-设计文档.md` ↔ `docs/design-constraints.md` 直接交叉验证：设计文档中每个"编译器应…"的描述在 constraints 中有实现规范
- [ ] 三方交叉引用一致性（所有 "详见 §X.Y" 引用可解析）
- [ ] 所有示例代码在规范中用形式化语法标注
- [ ] **验证被拒绝的 8 个特性（§15）的语法在 EBNF 中不存在**

---

## Phase 0 交付标准

- [x] `spec/trust-spec.md` 完成，覆盖 0.1.1–0.1.7 全部子项
- [x] `spec/stdlib.md` 完成，覆盖 12 个模块
- [ ] 0.3 三方一致性审计通过，无需修正的差异记录在案
- [ ] `docs/ROADMAP.md` 的 Phase 0.3 "一致性审计" 条目标记完成
- [ ] 创建 PR 合并 `phase0-spec` → `main`（Phase 0 冻结）

---

> **下一步：** Phase 1 — 编译器核心实现（`phase1-compiler` 分支）
