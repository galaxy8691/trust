# Phase 0 — 语言规范冻结  TODO

> 目标：将 `docs/Trust-设计文档.md` 转化为可执行的、无歧义的形式化语言规范。  
> 期限：第 1 个月  
> 优先级：P0（阻塞所有后续 Phase）

---

## 0.1 编写 Trust Language Specification

**产出物：** `spec/trust-spec.md`（或 `spec/` 目录下多文件）  
**工作量：** 2-3 周

### 0.1.1 词法规范

- [ ] Token 定义（关键字列表、字面量格式、注释格式）
- [ ] 关键字完整列表：`let` `mut` `const` `function` `inout` `move` `shared` `spawn` `async` `await`
  `if` `else` `for` `while` `loop` `break` `continue` `return` `throw` `switch` `case` `default`
  `match` `select` `import` `export` `from` `as` `interface` `type` `impl` `test` `extern`
  `true` `false` `undefined` `None` `Some` `Ok` `Err` `Rc` `Arc` `Weak` `Box` `dyn`
- [ ] 字面量格式：`number`（`i32`/`f64`/`bigint`）、`string`（双引号+模板字符串）、`boolean`
- [ ] 注释格式：`//` 行注释、`/* */` 块注释、`///` 文档注释
- [ ] 运算符和分隔符完整列表

### 0.1.2 语法规范

- [ ] 形式化 EBNF 语法定义（或类似形式化格式）
- [ ] 变量声明：`let` / `let mut` / `const`
- [ ] 函数声明：`function` 签名 + 单表达式简写 `=`
- [ ] 箭头函数：`(params) => expr` / `(params) => { body }`
- [ ] 控制流：`if` / `else` / `for` / `for-of` / `while` / `loop`
- [ ] 模式匹配：`switch` 语句 + `match` 表达式 + `if let`
- [ ] 异步：`async function` / `await` / `spawn` / `spawn async`
- [ ] 模块：`import` / `export` / `export default`
- [ ] 类型标注语法：`x: Type`
- [ ] 属性简写：`{ x, y }`
- [ ] 空值糖：`??` / `?.` / `!`
- [ ] 泛型：`<T>` / `extends` 约束
- [ ] ADT：`type X = | { kind: "A" } | { kind: "B" }`
- [ ] 结构体/接口：`interface` / `type` 别名 / `impl ... for ...`
- [ ] 闭包：`() =>` / `move () =>`
- [ ] 并发：`shared` / `withLock` / `Channel` / `select`
- [ ] FFI：`extern "rust" { fn ... }`
- [ ] 属性：`#[test]` / `#[should_panic]` / `#[property]` / `#[concurrent]`
- [ ] 生命周期标注：`<'a>`（仅在返回引用时）
- [ ] 引用：`&` 运算符
- [ ] 测试：`test function` / `#[test]`

### 0.1.3 语义规范

- [ ] AST → HIR 降级规则（每个 AST 节点到 HIR 的翻译）
- [ ] HIR → TIR 降级规则（控制流图、基本块转换）
- [ ] TIR 节点完整定义（参照 `design-constraints.md` §6.1）
- [ ] 代码生成映射规则（TIR → Rust 源码，参照 `design-constraints.md` §9.2 API 映射表）

### 0.1.4 类型系统规范

- [ ] 名义类型（`interface`）规则
- [ ] 结构别名（`type { ... }`）透明等价规则
- [ ] ADT（`type | ...`）标签联合规则
- [ ] 泛型单态化规则
- [ ] trait bound（`extends Interface` vs `extends { ... }` 结构化约束）
- [ ] 隐式 trait 生成规则（`HasLength` 等）
- [ ] `this: &Self` / `inout this` / `move this` 隐式参数规则
- [ ] `Send` / `Sync` 自动推导规则

### 0.1.5 所有权规则规范

- [ ] 移动语义形式化定义：`let b = a;` → `a` 失效
- [ ] 三模式参数表形式化定义：

  | 声明 | 语义 | Rust mapping |
  |------|------|-------------|
  | `f(x: T)` | 只读借用 | `f(x: &T)` |
  | `f(inout x: T)` | 可变借用 | `f(x: &mut T)` |
  | `f(move x: T)` | 所有权转移 | `f(x: T)` |

- [ ] 借用规则：同一时刻 ≤1 可变借用 或 ≥0 只读借用
- [ ] 闭包捕获规则：默认只读借用，`move` → FnOnce
- [ ] 区域推断（Region Inference）算法伪代码
- [ ] `Copy` 类型判定规则

### 0.1.6 并发规则形式化

- [ ] `Send` / `Sync` 自动推导算法
- [ ] `spawn` 要求 `move` + `Send` 的规则
- [ ] `shared` → `Arc<Mutex<T>>` / `AtomicI32` 优化规则
- [ ] `Channel` → `(Sender<T>, Receiver<T>)` 分离规则
- [ ] `select` 隐式 poll + `Result` auto-unwrap 规则

### 0.1.7 错误处理规则

- [ ] `Result<T, E>` 与 `?` 传播规则
- [ ] `throw Error` → `panic!` 映射
- [ ] `!` 仅限 `Option<T>` 的编译器检查规则
- [ ] `.expect()` 语义

---

## 0.2 编写 Trust 标准库规范

**产出物：** `spec/stdlib.md`  
**工作量：** 1 周  
**依赖：** 0.1 完成基本类型定义

- [ ] `std::collections` API 签名（`Vec` `HashMap` `HashSet` `VecDeque`）
- [ ] `std::sync` API 签名（`Channel` `shared` `spawn` `Mutex` `RwLock` `Atomic`）
- [ ] `std::async` API 签名（`join` `sleep`）
- [ ] `std::result` 类型定义（`Option<T>` / `Result<T,E>`）
- [ ] `std::string` API 签名（`split` `slice` `replace` `trim` `toUpperCase`）
- [ ] `std::fs` API 签名（读写、目录遍历、元数据）
- [ ] `std::rc` API 签名（`Rc` `Arc` `Weak`）
- [ ] `std::time` API 签名（`Duration` 定时器）
- [ ] `std::net` API 签名（TCP/UDP/HTTP/TLS）— v0.2 目标
- [ ] `std::serde` API 签名（JSON/MessagePack）— v0.2 目标
- [ ] `std::crypto` API 签名（SHA/BLAKE3/加密）— v0.3 目标
- [ ] 模块间依赖图
- [ ] Trust API → Rust 标准库映射关系表

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
- [ ] 三方交叉引用一致性（所有 "详见 §X.Y" 引用可解析）
- [ ] 所有示例代码在规范中用形式化语法标注

---

## Phase 0 交付标准

- [ ] `spec/trust-spec.md` 完成，覆盖 0.1.1–0.1.7 全部子项
- [ ] `spec/stdlib.md` 完成
- [ ] 三方一致性审计通过，无需修正的差异记录在案
- [ ] `docs/ROADMAP.md` 的 Phase 0.3 "一致性审计" 条目标记完成
- [ ] 创建 PR 合并 `phase0-spec` → `main`（Phase 0 冻结）

---

> **下一步：** Phase 1 — 编译器核心实现（`phase1-compiler` 分支）
