# Trust Language Specification — Phase 0.1

> 版本：v0.0-draft · 分支：phase0-spec · 对 `docs/Trust-设计文档.md` 的完整形式化  
> 本文档是 Trust 编译器的单一真理来源。任何分歧以此规范为准。

---

## 0. 文件说明

本文档定义 Trust 语言的完整形式化规范，包括词法、语法、语义、类型系统、所有权、并发和错误处理。

**存放路径：** `spec/trust-spec.md` · **对齐设计文档：** `docs/Trust-设计文档.md`

<dl class="spec-grid">
  <dt>里程碑代号</dt><dd>v0.0 — 规范冻结</dd>
  <dt>前置</dt><dd>设计文档完成辩论审计</dd>
  <dt>下一里程碑</dt><dd>Phase 1 — 编译器核心实现</dd>
  <dt>产出物</dt><dd>本文件 + spec/stdlib.md + spec/p0/TODO.md 全部勾选</dd>
</dl>

---

## 1. 成功标准

> **规范冻结的验收条件：**
> 1. 0.1.1–0.1.7 全部子项形式化完成，无 TODO 残留
> 2. 所有设计文档 §11 的代码示例可以使用本规范的 EBNF 解析
> 3. Ownership rules (§0.1.5) 的算法伪代码可用于实现 borrow checker
> 4. 三方审计（设计文档 ↔ 本规范 ↔ design-constraints）无未解决的差异

---

## 2. 范围

### 2.1 纳入（本规范覆盖）

| # | 域 | 对应 TODO |
|---|------|----------|
| LEX | 词法规范 | 0.1.1 — 关键字、字面量、运算符、注释 |
| SYN | 语法规范 | 0.1.2 — 完整 EBNF、错误恢复、分号/分隔符 |
| SEM | 语义规范 | 0.1.3 — AST/HIR 节点定义、降级规则、类型检查 |
| TYP | 类型系统 | 0.1.4 — 名义类型、ADT、泛型、Dynamic、Box<dyn Trait>、??、?. |
| OWN | 所有权规则 | 0.1.5 — 移动、借用、方法调用、闭包、生命周期 |
| CON | 并发规则 | 0.1.6 — Send/Sync、spawn、shared、Channel、select |
| ERR | 错误处理 | 0.1.7 — Result+?、throw→panic!、!、expect |

### 2.2 排除（不在本规范）

| 项目 | 归属 |
|------|------|
| 编译器 Rust 实现规范 | `docs/design-constraints.md` |
| 标准库 API 签名 | `spec/stdlib.md`（Phase 0.2） |
| 工程里程碑时间线 | `docs/ROADMAP.md` |
| 被拒绝特性（try/catch、defer、\|\>等 8 项） | `docs/Trust-设计文档.md` §15；本规范 §0.3 审计验证 EBNF 中不存在 |
| `#[test]` / `#[should_panic]` 等属性语法 | 本规范 §0.1.2 仅定义语法形式；语义细节见设计文档 §14 |

---

## 0.1.1 词法规范

### LEX-REQ-001：关键字与保留字

**需求：** 词法器必须识别以下关键字，不允许作为标识符使用。

**分类：**

| 分类 | 关键字 | 说明 |
|------|--------|------|
| **保留关键字** | `let` `mut` `const` `function` `inout` `move` `shared` `spawn` `async` `await` `if` `else` `for` `of` `while` `loop` `break` `continue` `return` `throw` `switch` `case` `default` `match` `select` `import` `export` `from` `as` `interface` `type` `impl` `test` `extern` `this` `extends` `fn` | 不可作为标识符 |
| **布尔字面量** | `true` `false` | 值关键字 |
| **空值字面量** | `undefined` `None` | 值关键字 |
| **构造器关键字** | `Some` `Ok` `Err` | 类型构造器值 |
| **类型名** | `number` `string` `boolean` `bigint` `void` | 内置类型标识符，不可用于变量/函数名 |
| **智能指针** | `Rc` `Arc` `Weak` `Box` | 标准库类型，保留 |
| **trait 关键字** | `dyn` | trait object 语法 |

**设计决策——`fn` vs `function`：** `extern "rust" { fn ... }` 块内使用 `fn` 而非 `function`，视觉区分 FFI 声明与 Trust 函数。`fn` 仅在 `extern` 块内合法，全局不可作为标识符。

**验收标准：**
- [ ] 词法器拒绝 `let async = 42`
- [ ] 词法器拒绝 `function void() {}`
- [ ] 词法器接受 `let x: number = 42`
- [ ] `extern "rust" { fn sqlx_query<T>(...) -> ...; }` 中 `fn` 不被报错

### LEX-REQ-002：字面量格式

**需求：** 词法器必须识别以下字面量种类。

| 种类 | 格式 | 示例 | Rust 映射 |
|------|------|------|----------|
| 整数 | 十进制数字序列 | `42`, `0`, `999` | `i32` |
| 浮点 | 十进制数字序列、小数点、更多数字 | `3.14`, `0.0`, `1.0e10` | `f64` |
| BigInt | 十进制数字序列后跟 `n` | `9007199254740991n` | `i64` |
| 字符串 | `"..."` 双引号包裹 | `"hello"` | `String` |
| 模板字符串 | `` `...${expr}...` `` | `` `Hello, ${name}` `` | Rust `format!` 展开 |
| 布尔 | `true` `false` | | `bool` |

**设计决策——`number` 不是统一承载类型：** §2.2 禁止隐式类型转换。因此 `number` 在词法阶段即区分为整数（`i32`）和浮点（`f64`）。`42 + 3.14` 是**编译错误**——必须写 `42 as f64 + 3.14` 或 `42 + 3.14 as i32`。BigInt 通过 `n` 后缀独立识别，不参与 `i32`/`f64` 的自动选择。

**撤销 TS 习惯：** `42 + 3.14` 在 TypeScript 中返回 `44.14`（隐式 widening）。Trust 严格禁止——见 TYP-REQ-001。

**验收标准：**
- [ ] `42` 被词法器识别为 `IntLiteral(42)`
- [ ] `3.14` 被词法器识别为 `FloatLiteral(3.14)`
- [ ] `9007199254740991n` 被词法器识别为 `BigIntLiteral(9007199254740991)`
- [ ] `` `Hello, ${name}!` `` 被词法器识别为模板字符串
- [ ] `42 + 3.14` 在类型检查阶段报错（非词法阶段）

### LEX-REQ-003：运算符与优先级

**需求：** 词法器识别运算符 token，语法层通过 EBNF 定义优先级与结合性。

| 优先级（高→低） | 运算符 | 结合性 | 说明 |
|----------------|--------|--------|------|
| 15 | `()` `[]` `::` `.` | 左 | 调用、索引、构造器访问、成员访问 |
| 14 | `!`（后缀） `?`（后缀传播） | 左 | 断言解包、Result 传播 |
| 13 | `&` `!`（前缀） `await` | 右→左 | 引用、逻辑非、异步等待 |
| 12 | `as` | 左 | 显式类型转换 |
| 11 | `*` `/` `%` | 左 | 乘除取模 |
| 10 | `+` `-` | 左 | 加减 |
| 9 | `?.` | 左 | 可选链 |
| 8 | `??` | 左 | 空值合并 |
| 7 | `==` `!=` `<` `>` `<=` `>=` | 左 | 比较 |
| 6 | `&&` | 左 | 逻辑与 |
| 5 | `\|\|` | 左 | 逻辑或 |
| 4 | `=` `+=` `-=` `*=` `/=` `%=` | 右→左 | 赋值 |

**设计决策——`await` 是中缀而非前缀运算符语义，但在优先级表中归类为前缀：** `await expr` 紧绑定右侧表达式。`await a + b` 解析为 `(await a) + b`，非 `await (a + b)`。若意图后者，必须加括号。

**验收标准：**
- [ ] `a + b * c` 解析为 `a + (b * c)`
- [ ] `a ?? b ?? c` 解析为 `(a ?? b) ?? c`（左结合）
- [ ] `a = b = c` 解析为 `a = (b = c)`（右→左结合）
- [ ] `await a + b` 解析为 `(await a) + b`

### LEX-REQ-004：注释格式

| 格式 | 语法 | 说明 |
|------|------|------|
| 行注释 | `// ...` | 到行尾止 |
| 块注释 | `/* ... */` | 可跨行，不支持嵌套 |
| 文档注释 | `/// ...` | 仅允许在 `export function`、`export interface`、`export type` 上方 |

**验收标准：**
- [ ] `// comment` 被忽略，不影响后续 token
- [ ] `/* comment */` 可跨行
- [ ] `/// doc` 在 `export function` 上方正确关联

---

## 0.1.2 语法规范

### SYN-REQ-001：变量声明

```ebnf
var_decl ::= ("let" | "let" "mut") ident (":" type)? "=" expr ";"
          |  "const" ident (":" type)? "=" expr ";"
          |  "shared" ident "=" expr ";"
```

**验收标准：**
- [ ] `let x = 42` 成功解析
- [ ] `let mut y: number = 10` 成功解析
- [ ] `const MAX = 100` 成功解析
- [ ] `shared counter = 0` 成功解析

### SYN-REQ-002：函数声明

```ebnf
function_decl ::= "function" ident generic_params? "(" param_list? ")" (":" type)? (block | "=" expr ";")
param_list    ::= param ("," param)*
param         ::= ("inout" | "move")? ident (":" type)?
generic_params ::= "<" ident ("," ident)* (("extends" type) | ("extends" "{" field_list "}"))? ">"
```

**验收标准：**
- [ ] `function add(a: number, b: number): number { return a + b; }` 解析
- [ ] `function square(x: number) = x * x;` 单表达式简写解析
- [ ] `function identity<T>(value: T): T { return value; }` 泛型解析
- [ ] `function pushOne(inout arr: number[]) { arr.push(1); }` inout 参数解析

### SYN-REQ-003：控制流

```ebnf
if_stmt    ::= "if" "(" expr ")" block ("else" "if" "(" expr ")" block)* ("else" block)?
for_stmt   ::= "for" "(" ("let" ident "=" expr ";" expr ";" expr) ")" block
            |  "for" "(" "let" ident "of" expr ")" block
while_loop ::= "while" "(" expr ")" block
loop_stmt  ::= "loop" block
return_stmt ::= "return" expr? ";"
break_stmt   ::= "break" expr? ";"
```

**设计决策——`if` 和 `loop` 是表达式：** `let x = if (c) { a } else { b };` 合法。`loop { break val; }` 返回 `val`。`for` 和 `while` 是语句，无返回值。`break` 仅在 `loop` 中可携带值。

**验收标准：**
- [ ] `let label = if (score >= 60) { "pass" } else { "fail" };` 解析
- [ ] `let result = loop { if (count >= 3) { break count * 2; } count += 1; };` 解析
- [ ] `for (let i = 0; i < 10; i++) { console.log(i); }` 解析
- [ ] `for (let item of items) { process(item); }` 解析

### SYN-REQ-004：模式匹配

```ebnf
switch_stmt ::= "switch" "(" expr ("," expr)? ")" "{" switch_case* "}"
switch_case ::= "case" pattern ("," pattern)* ":" statement* ("break" ";")?
match_expr  ::= "match" "(" expr ")" "{" match_arm ("," match_arm)* ","? "}"
match_arm   ::= "case" pattern "=>" expr
if_let_stmt ::= "if" "let" pattern "=" expr block ("else" block)?
```

**验收标准：**
- [ ] `switch (msg.kind) { case "quit": return; case "data": process(msg.payload); break; }` 穷举检查
- [ ] `let label = match (msg.kind) { case "quit" => "bye", case "data" => "data" };` 表达式
- [ ] `if let Some(val) = opt { process(val); }` 解析

### SYN-REQ-005：Channel 与 select

```ebnf
channel_expr ::= "Channel" "<" type ">" "(" expr? ")"
select_stmt  ::= "select" "{" select_branch+ "}"
select_branch ::= "case" ident "=" expr "=>" block
```

**验收标准：**
- [ ] `let (tx, rx) = Channel<number>(64)` 解析
- [ ] `select { case msg = rx.receive() => { ... } }` 解析（分支内不写 await）

### SYN-REQ-006：Parser 错误恢复

**策略：** panic mode。遇到语法错误后，跳过 token 直到同步点，继续解析收集后续错误。

**同步点（按恢复优先级）：**
1. `;` — 语句分隔
2. `}` — 块结束
3. `function` `import` `export` `type` `interface` `impl` `test` `async` — 声明关键字

**验收标准：**
- [ ] 包含语法错误的 `.trust` 文件产生 **≥2 个诊断信息**，而非仅在第一个错误停止
- [ ] 错误恢复后，后续合法代码仍产生正确的 AST 节点

### SYN-REQ-007：分号与分隔符

| 上下文 | 规则 |
|--------|------|
| 语句结尾 | 必须 `;` |
| `match` 分支 | 以 `,` 分隔，最后一个可省略 |
| `switch` 分支 | 分支内语句以 `;` 结尾，`break;` 必需 |
| 块 `{ ... }` | 最后一条表达式省略 `;` 则作为块返回值 |

---

## 0.1.3 语义规范

### SEM-REQ-001：AST 节点定义

必须定义以下 AST 节点（parser 输出）：

- `Program`：文件顶层，包含 import 列表 + statement 列表
- `LetStmt`：`let` / `let mut` / `const` 声明
- `SharedStmt`：`shared` 声明
- `FunctionDecl`：函数声明（含泛型参数、参数列表、返回类型、体）
- `IfExpr`：if-else 表达式（可赋值给变量）
- `ForStmt`：C-style / for-of 循环语句
- `LoopExpr`：loop 表达式（可带 break 值）
- `WhileStmt`：while 循环语句
- `SwitchStmt`：switch 语句
- `MatchExpr`：match 表达式
- `IfLetStmt`：if let 语句
- `CallExpr`：函数调用
- `SelectStmt`：select 语句

### SEM-REQ-002：HIR 节点定义

HIR = AST + 名称解析 + 类型信息。定义：
- `HirModule`：全局符号表、import 图
- `HirFunction`：名称解析后的函数（每个标识符绑定到 def）
- `HirType`：解析后的类型（不含推断变量）

### SEM-REQ-003：AST → HIR 降级规则

- 名称解析：`import { foo } from "./bar"` → 解析 `bar.trust` → 验证 `foo` 存在 → 建立符号绑定
- 作用域分析：函数参数和 `let` 变量进入局部作用域；`const` 进入模块作用域
- 泛型实例化：调用点推断 `T`（见 SEM-REQ-004）

### SEM-REQ-004：类型检查与推断

- 名义类型检查：`interface A { x: number }` 和 `interface B { x: number }` 是不同的类型，不能互相赋值
- 类型推断增强：闭包参数从上下文推断（`nums.map(x => x * 2)` → `x: number`）；单表达式函数返回值推断（`function square(x: number) = x * x` → 返回类型为 `number`）；泛型参数从调用点实参推断（`identity(42)` → `T = number`）

### SEM-REQ-005：HIR → TIR 降级规则

- 控制流图转换（if/for/loop → 基本块 + 条件跳转）
- `if` 和 `loop` 表达式→语句转换（表达式值通过临时变量持有）
- 闭包捕获变量提升为隐式参数
- 方法调用语法糖展开（`pt.print()` → `Printable::print(&pt)`）

---

## 0.1.4 类型系统规范

### TYP-REQ-001：数字类型规则

> **数字类型体系：** `number` 是抽象类型族，编译时根据字面量形式选择承载类型。`i32` 与 `f64` 之间**禁止隐式混用**。此规则是 §2.2 "隐式类型转换禁止" 的核心实例。

| 字面量 | 承载类型 | Rust 映射 |
|--------|---------|----------|
| `42` `0` `-7` | `i32` | `i32` |
| `3.14` `0.0` `1e10` | `f64` | `f64` |
| `9007199254740991n` | `bigint` | `i64` |

编译器行为：
- `let x = 42; let y = 3.14; let z = x + y;` → **编译错误**："类型不匹配：i32 与 f64 不能混用"
- `let z = x as f64 + y;` → ✅ 合法
- `let z = x + y as i32;` → ✅ 合法（y 截断为 3）

**设计决策——为什么不是 `i32→f64` 自动提升？（方案 A vs B）：** Trust 的类型系统核心理念：编译通过 = 无隐式信息丢失。如果允许 `i32 + f64` 自动提升，则 `f64` 的精度问题（IEEE 754）会静默注入整数运算——与 JavaScript 的 `Number` 安全隐患同源。TS 开发者习惯的 `42 + 3.14` 需要加一个 `as f64`，这是可接受的"语法税"。

### TYP-REQ-002：名义类型（interface）

两个 `interface` 即使字段类型完全相同，也被视为不同的类型。赋值和参数传递必须通过显式构造或 trait 约束。

```ts
interface Point { x: number; y: number; }
interface Vec2 { x: number; y: number; }
let p: Point = { x: 1, y: 2 };
// let v: Vec2 = p;  // ❌ 编译错误：Point 不能赋值给 Vec2
```

### TYP-REQ-003：结构别名（type { }）

`type Alias = { x: number; y: number }` 是结构体的透明别名。`Alias` 与 `{ x: number; y: number }` 等价，不引入新的类型身份。

### TYP-REQ-004：ADT（type | ）标签联合

`type Msg = | { kind: "a" } | { kind: "b"; data: number }` 生成带标签的枚举（Rust enum）。每个变体有新的类型身份。穷举检查在 `switch` 和 `match` 中生效。

### TYP-REQ-005：Dynamic 枚举

标准库类型 `Dynamic` 包含以下变体：`Number(n: i32)` `Float(f: f64)` `String(s: String)` `Boolean(b: bool)` `Array(arr: Vec<Dynamic>)` `Null` `Object(map: HashMap<String, Dynamic>)`。

模式匹配时每个变体可绑定内部值：`case Dynamic.Number(n) => n * 2`。

### TYP-REQ-006：Box<dyn Trait>

`Box<dyn Trait>` 是 trait object 的类型语法。映射为 Rust `Box<dyn Trait + 'static>`。vtable 分发，不能穷举检查。与泛型的选择指南：已知类型集合 → Dynamic；开放类型集合 → `Box<dyn Trait>`。

### TYP-REQ-007：Send / Sync 类型推导

基于字段组成的自动推导：
- 所有字段为 `Send` → 类型为 `Send`
- 所有字段为 `Sync` → 类型为 `Sync`
- `Rc<T>` 不实现 `Send`
- `Arc<T>` 实现 `Send`（若 `T: Send + Sync`）

---

## 0.1.5 所有权规则规范

### OWN-REQ-001：移动语义

形式化定义：`let b = a;` 后，`a` 的所有权转移到 `b`，`a` 变为"已移动"状态。后续读取 `a` → 编译错误 E0382。

### OWN-REQ-002：三模式参数表

| 声明 | IR 表示 | Rust 生成 | 调用处 |
|------|---------|----------|--------|
| `f(x: T)` | `ReadOnly(x: T)` | `f(x: &T)` | `f(x)` |
| `f(inout x: T)` | `Mutable(x: T)` | `f(x: &mut T)` | `f(inout x)` |
| `f(move x: T)` | `Move(x: T)` | `f(x: T)` | `f(move x)` |

### OWN-REQ-003：借用规则

同一变量在同一时刻最多有一个可变借用，或任意数量个只读借用。

### OWN-REQ-004：方法调用与所有权

- `let obj = ...`（非 `mut`） → 只能调用 `&self` 方法。调用 `&mut self` 方法 → 编译错误。
- `let mut obj = ...` → 可调用任何方法，但调用期间独占借用。
- `inout this` 方法 → 调用期间冻结原变量。
- `move this` 方法 → 调用期间消耗原变量。

### OWN-REQ-005：闭包捕获规则

闭包捕获外部变量与函数参数保持一致——默认只读借用，`move` 关键字转移所有权。

- 默认闭包 `() => expr`：`Fn` 类型，可多次调用
- `move` 闭包 `move () => expr`：`FnOnce` 类型，只能调用一次
- `spawn` 要求 `move` 闭包 + 捕获变量满足 `Send`

### OWN-REQ-006：引用计数

| 类型 | 线程 | 规则 |
|------|------|------|
| `Rc<T>` | 单线程 | `Rc::new(v)` → 引用计数 1；`clone()` → +1；`drop` → -1，到 0 释放。**不实现 Send**。 |
| `Arc<T>` | 多线程 | 同 `Rc` 但原子引用计数。**实现 Send**（若 `T: Send + Sync`）。 |
| `Weak<T>` | 单/多 | `Rc::downgrade` / `Arc::downgrade` 创建；`upgrade()` → `Option<Rc<T>>`。 |

**验收：** `spawn` 内部使用 `Rc<T>` → 编译错误（`Rc` 非 `Send`）；用 `Arc<T>` → 通过。

### OWN-REQ-007：for 循环隐式可变例外

`for (let i = 0; i < N; i++)` 中的迭代变量 `i` 为**隐式可变**——这是 Trust 中唯一允许 `let` 声明的变量被修改的场景。

---

## 0.1.6 并发规则规范

### CON-REQ-001：Send/Sync 使用侧检查

- `spawn(move || ...)`：闭包必须 `Send`（所有捕获变量 Send）
- `spawn(move async { ... })`：闭包必须 `Send + 'static`
- `shared x = expr`：`T` 必须 `Send`（因内部使用 `Arc<Mutex<T>>`）
- `Channel<T>(cap)`：`T` 必须 `Send`

### CON-REQ-002：spawn

`spawn` 关键字：非 async 闭包 → `std::thread::spawn`；async 闭包 → `tokio::spawn`。两者都必须 `move` 闭包。

### CON-REQ-003：shared → 编译时实现选择

| `shared` 类型 | Rust 实现 | 优化条件 |
|---------------|----------|---------|
| `shared x: number = 0` | `Arc<AtomicI32>` | 类型为 `number`（i32） |
| `shared x: T`（非 number） | `Arc<Mutex<T>>` | 默认路径 |

**验收：** `shared counter = 0; counter.withLock(c => { c += 1; })` → 原子 `fetch_add`，无锁。

### CON-REQ-004：Channel 分离

`Channel<T>(capacity)` 返回 `(Sender<T>, Receiver<T>)` 元组。`Sender` 实现 `Clone`，`Receiver` 不实现 `Clone`（唯一接收方）。

### CON-REQ-005：select 隐式 poll

`select { case x = rx1.receive() => { ... } }` 分支内**禁止写** `await`。编译器在 `select` 上下文中自动 poll。`receive()` 返回 `Result<T, ChannelClosed>`，`select` 自动匹配 `Ok(T)` 并绑定到变量；`Err` 视为该分支不可用。若所有分支同时不可用 → 编译为 `panic!("all select branches disabled")`。

---

## 0.1.7 错误处理规则规范

### ERR-REQ-001：Result<T, E> 与 ? 传播

`?` 操作符作用在 `Result<T, E>` 上：若 `Ok(t)` → 提取 `t` 继续执行；若 `Err(e)` → 立即从当前函数返回 `Err(e)`。

### ERR-REQ-002：throw → panic! 映射

`throw Error(msg)` 编译为 `panic!("{}", msg)`。不可捕获（无 try/catch）。仅用于逻辑不变量，不可用于可恢复业务错误。

### ERR-REQ-003：! 断言操作符

`expr!` 等价于 `expr.unwrap()`。仅允许用于 `Option<T>`。`Result<T,E>` 使用 `!` → 编译错误。

### ERR-REQ-004：.expect()

`expr.expect("message")` 等价于 `expr.unwrap()` 但 panic 时携带消息。用于 `Option<T>` 和 `Result<T,E>`。

---

> **下一步：** `spec/stdlib.md`（Phase 0.2）覆盖标准库 API 签名。  
> **审计：** 三方交叉验证（Phase 0.3）检查本规范 × 设计文档 × design-constraints 的一致性。
