# Trust Language Specification v0.0

> 版本：v0.0-draft · 分支：phase0-spec · 对齐 `docs/Trust-设计文档.md`  
> 本文档是 Trust 编译器实现的单一真理来源。任何分歧以此规范为准。

---

## 0. 文件说明

本文档形式化定义 Trust 语言的词法、语法、语义、类型系统、所有权、并发和错误处理。

**目标读者：** 编译器实现者。本文档不解释"为什么这样设计"（见设计文档），只陈述"是什么"——实现者可直接据此编码。

**成功标准：** 一名不熟悉 Trust 的 Rust 编译器开发者，仅凭本文档即可实现 lexer + parser + type checker + borrow checker + codegen。

---

## LEX：词法规范

### LEX-REQ-001：关键字

**需求：** Trust 的关键字不可作为标识符。分类如下：

| 分类 | 关键字 | 说明 |
|------|--------|------|
| 声明 | `let` `mut` `const` `shared` | 变量声明 |
| 函数 | `function` `fn` | `fn` 仅用于 `extern` 块 |
| 参数 | `inout` `move` | 借用/所有权标注 |
| 并发 | `spawn` `async` `await` `select` | 异步与并发 |
| 控制流 | `if` `else` `for` `of` `while` `loop` `break` `continue` `return` `throw` | 控制流 |
| 匹配 | `switch` `case` `default` `match` | 模式匹配 |
| 模块 | `import` `export` `from` `as` | 模块系统 |
| 类型 | `interface` `type` `impl` `extends` `this` `dyn` | 类型系统 |
| 测试 | `test` | 测试函数 |
| 外部 | `extern` | FFI 声明 |
| 字面量 | `true` `false` `undefined` `None` | 值字面量 |
| 构造器 | `Some` `Ok` `Err` | Option/Result 构造 |
| 智能指针 | `Rc` `Arc` `Weak` `Box` | 标准库类型 |
| 类型名 | `number` `string` `boolean` `bigint` `void` | 内置类型名，不可用于变量/函数名 |

**验收标准：**
- AC-LEX-001: `let async = 42` → 词法错误（`async` 是关键字）
- AC-LEX-002: `function void() {}` → 词法错误（`void` 是类型名）
- AC-LEX-003: `let x: number = 42` → 合法
- AC-LEX-004: `extern "rust" { fn sqlx_query<T>(query: string): Result<T, SqlxError>; }` → `fn` 被识别为关键字（仅在 `extern` 块内合法）

### LEX-REQ-002：字面量

**需求：** 字面量格式与承载类型如下：

| 种类 | 词法格式 | 承载类型 | 示例 |
|------|---------|---------|------|
| 整数 | `[0-9]+` | `i32` | `42` |
| 浮点 | `[0-9]+ "." [0-9]+` | `f64` | `3.14` |
| BigInt | `[0-9]+ "n"` | `i64` | `9007199254740991n` |
| 字符串 | `"\"" ... "\""` | `String` | `"hello"` |
| 模板 | `` "`" ... "${" expr "}" ... "`" `` | `String`（展开为 format!） | `` `Hello, ${name}` `` |
| 布尔 | `true` `false` | `bool` | |

**验收标准：**
- AC-LEX-005: `42` → 词法器输出 `IntLiteral(42)`，类型 `i32`
- AC-LEX-006: `3.14` → `FloatLiteral(3.14)`，类型 `f64`
- AC-LEX-007: `9007199254740991n` → `BigIntLiteral(9007199254740991n)`，类型 `i64`

### LEX-REQ-003：运算符优先级

**需求：** 运算符按优先级从高到低排序。高数字 = 高优先级（更紧绑定）。同一优先级内按结合性。

| 优先级 | 运算符 | 结合性 | 类别 |
|--------|--------|--------|------|
| 15 | `()` `[]` `::` `.` | 左 | 调用、索引、构造器、成员 |
| 14 | `expr!` `expr?` | 左 | 断言解包、Result 传播 |
| 13 | `&expr` `!expr` `await expr` | 右→左 | 引用、逻辑非、异步等待 |
| 12 | `expr as T` | 左 | 显式类型转换 |
| 11 | `*` `/` `%` | 左 | 乘除取模 |
| 10 | `+` `-` | 左 | 加减 |
| 9 | `?.` | 左 | 可选链 |
| 8 | `??` | 左 | 空值合并 |
| 7 | `==` `!=` `<` `>` `<=` `>=` | 左 | 比较 |
| 6 | `&&` | 左 | 逻辑与 |
| 5 | `\|\|` | 左 | 逻辑或 |
| 4 | `=` `+=` `-=` `*=` `/=` `%=` | 右→左 | 赋值 |

**验收标准：**
- AC-LEX-008: `a + b * c` → 解析为 `a + (b * c)`（优先级 11 > 10）
- AC-LEX-009: `a ?? b ?? c` → 解析为 `(a ?? b) ?? c`（左结合）
- AC-LEX-010: `a = b = c` → 解析为 `a = (b = c)`（右→左结合）
- AC-LEX-011: `await a + b` → 解析为 `(await a) + b`

### LEX-REQ-004：注释

| 格式 | 语法 | 说明 |
|------|------|------|
| 行注释 | `// ...` | 到行尾止 |
| 块注释 | `/* ... */` | 可跨行，不可嵌套 |
| 文档注释 | `/// ...` | 仅允许在 `export` 声明上方 |

**验收标准：**
- AC-LEX-012: `// comment\nlet x = 1` → 注释被忽略，`x` 正常解析
- AC-LEX-013: `let /* inline */ x = 1` → 块注释不影响 `let` 解析
- AC-LEX-014: `/// doc\n export function f()` → 文档注释关联到 `f`

---

## SYN：语法规范

### SYN-REQ-001：变量声明

```ebnf
var_decl ::= ("let" | "let" "mut") ident (":" type)? "=" expr ";"
           | "const" ident (":" type)? "=" expr ";"
           | "shared" ident "=" expr ";"
```

**验收标准：**
- AC-SYN-001: `let x = 42` → 成功解析为 `LetStmt`
- AC-SYN-002: `let mut y: number = 10;` → 成功解析
- AC-SYN-003: `const MAX = 100;` → 成功解析为 `ConstStmt`
- AC-SYN-004: `shared counter = 0;` → 成功解析为 `SharedStmt`

### SYN-REQ-002：函数声明

```ebnf
function_decl ::= "function" ident generic_params? "(" param_list? ")" (":" type)? ("{" stmt* "}" | "=" expr ";")
param         ::= ("inout" | "move")? ident (":" type)?
generic_params ::= "<" ident ("," ident)* ("extends" (type | "{" field_list "}"))? ">"
```

**验收标准：**
- AC-SYN-005: `function add(a: number, b: number): number { return a + b; }` → 成功解析
- AC-SYN-006: `function square(x: number) = x * x;` → 单表达式简写解析
- AC-SYN-007: `function identity<T>(value: T): T { return value; }` → 泛型解析
- AC-SYN-008: `function pushOne(inout arr: number[]) { arr.push(1); }` → inout 参数解析

### SYN-REQ-003：控制流

```ebnf
if_stmt    ::= "if" "(" expr ")" block ("else" ("if" "(" expr ")" block | block))?
for_stmt   ::= "for" "(" ("let" ident "=" expr ";" expr ";" expr) ")" block
             | "for" "(" "let" ident "of" expr ")" block
while_stmt ::= "while" "(" expr ")" block
loop_stmt  ::= "loop" block
return_stmt ::= "return" expr? ";"
break_stmt ::= "break" expr? ";"
```

**设计决策——`if` 和 `loop` 是表达式：** `let x = if (c) { a } else { b };` 合法。`loop { break val; }` 返回 `val`。`for`/`while` 是语句，无返回值。`break` 仅在 `loop` 中可带值。

**验收标准：**
- AC-SYN-009: `let label = if (score >= 60) { "pass" } else { "fail" };` → 解析为 `IfExpr`（表达式）
- AC-SYN-010: `let result = loop { if (count >= 3) { break count * 2; } count += 1; };` → `break` 带值解析
- AC-SYN-011: `for (let i = 0; i < 10; i++) { console.log(i); }` → C-style for 解析
- AC-SYN-012: `for (let item of items) { process(item); }` → for-of 解析

### SYN-REQ-004：模式匹配

```ebnf
switch_stmt ::= "switch" "(" expr ")" "{" switch_case* "}"
switch_case ::= "case" pattern ":" stmt* ("break" ";")?
match_expr  ::= "match" "(" expr ")" "{" match_arm ("," match_arm)* ","? "}"
match_arm   ::= "case" pattern "=>" expr
if_let_stmt ::= "if" "let" pattern "=" expr block ("else" block)?
```

**设计决策——`switch`（语句）vs `match`（表达式）：** `switch` 用于副作用，分支用 `case X:` + `break;`。`match` 用于值映射，分支用 `case X => expr,`。

**验收标准：**
- AC-SYN-013: `switch (msg.kind) { case "quit": return; case "data": process(msg.payload); break; }` → 穷举检查
- AC-SYN-014: `let label = match (msg.kind) { case "quit" => "bye", case "data" => "data" };` → `match` 表达式
- AC-SYN-015: `if let Some(val) = opt { process(val); }` → 成功解析
- AC-SYN-016: `if let Some(val) = opt { process(val); } else { default(); }` → if let ... else 解析

### SYN-REQ-005：异步

```ebnf
async_fn   ::= "async" "function" ident generic_params? "(" param_list? ")" (":" type)? block
await_expr ::= "await" expr
spawn_expr ::= "spawn" "(" ("move")? "async"? "(" param_list? ")" "=>" (block | expr) ")"
```

**验收标准：**
- AC-SYN-017: `async function fetch(): Result<Data, Error> { ... }` → 解析
- AC-SYN-018: `let data = await fetch();` → 解析
- AC-SYN-019: `spawn(move async () => { ... })` → 解析

### SYN-REQ-006：模块

```ebnf
import_decl    ::= "import" (import_named | import_default | import_namespace) "from" string ";"
import_named   ::= "{" ident ("," ident)* "}"
import_default ::= ident
import_namespace ::= "*" "as" ident
export_decl    ::= "export" ("default")? (function_decl | var_decl | type_decl | interface_decl)
```

**验收标准：**
- AC-SYN-020: `import { foo, bar } from "./util";` → 解析
- AC-SYN-021: `import greet from "./greet";` → 默认导入解析
- AC-SYN-022: `import * as math from "./math";` → 命名空间导入解析
- AC-SYN-023: `export function baz() { }` → 解析

### SYN-REQ-007：并发

```ebnf
channel_expr ::= "Channel" "<" type ">" "(" expr? ")"
select_stmt  ::= "select" "{" select_branch+ "}"
select_branch ::= "case" ident "=" expr "=>" block
withlock_expr ::= ident "." "withLock" "(" closure ")"
```

**验收标准：**
- AC-SYN-024: `let (tx, rx) = Channel<number>(64);` → 解析
- AC-SYN-025: `select { case msg = rx.receive() => { console.log(msg); } }` → 解析（分支内**无** `await`）
- AC-SYN-026: `counter.withLock(c => { c += 1; });` → 解析

### SYN-REQ-008：类型与表达式补充

覆盖空值糖、泛型、ADT、闭包、引用、FFI、属性、生命周期。

**验收标准：**
- AC-SYN-027: `let val = maybeValue!;` → `!` 断言解析
- AC-SYN-028: `let file = fs.open("a.txt")?;` → `?` 传播解析
- AC-SYN-029: `let name = maybeName ?? "anonymous";` → `??` 解析
- AC-SYN-030: `let street = user?.address?.street;` → `?.` 解析
- AC-SYN-031: `type Msg = | { kind: "quit" } | { kind: "data"; payload: number[] };` → ADT 解析
- AC-SYN-032: `interface Printable { print(): void; } impl Printable for Point { function print() { ... } }` → interface+impl 解析
- AC-SYN-033: `let r = &data;` → `&` 引用解析
- AC-SYN-034: `extern "rust" { fn sha256(data: number[]): [number; 32]; }` → FFI 解析
- AC-SYN-035: `function getFirst<'a>(arr: &'a number[]): &'a number { return &arr[0]; }` → 生命周期标注解析
- AC-SYN-036: `#[test] function add_works() { assert(1 + 1 == 2); }` → 属性语法解析

### SYN-REQ-009：错误恢复

**需求：** Parser 采用 panic mode。遇到语法错误后，跳过 token 直到同步点。

**同步点：** `;` `}` `function` `import` `export` `type` `interface` `impl` `test` `async`

**验收标准：**
- AC-SYN-037: 包含语法错误的 `.trust` 文件产生 ≥2 个诊断信息（非首个错误停止）
- AC-SYN-038: 错误恢复后，后续合法代码仍产出正确的 AST 节点

### SYN-REQ-010：分隔符规则

| 上下文 | 规则 |
|--------|------|
| 语句 | 必须 `;` |
| `match` 分支 | `,` 分隔，最后一个可省略 |
| `switch` 分支 | 分支内语句 `;` + `break;` |
| 块 `{ }` | 最后表达式省略 `;` 作为返回值 |

**验收标准：**
- AC-SYN-039: `let x = { let y = 2; y };` → `y` 作为块返回值（省略 `;`）
- AC-SYN-040: `let x = match (v) { case Some(n) => n, case None => 0 };` → `match` 分支 `,` 分隔

---

## SEM：语义规范

### SEM-REQ-001：AST 节点定义

**需求：** Parser 产出以下 AST 节点：

| 节点 | 字段 | 说明 |
|------|------|------|
| `Program` | `imports: Vec<Import>`, `statements: Vec<Stmt>` | 文件顶层 |
| `LetStmt` | `name`, `type_ann`, `init`, `mutable: bool` | `let`/`let mut` |
| `ConstStmt` | `name`, `type_ann`, `init` | `const` |
| `SharedStmt` | `name`, `init` | `shared` |
| `FunctionDecl` | `name`, `generics`, `params`, `return_type`, `body` | `function` |
| `IfExpr` | `condition`, `then_branch`, `else_branch` | 表达式，可赋值 |
| `ForStmt` | `init`, `condition`, `update`, `body` | C-style / for-of |
| `LoopExpr` | `body` | 表达式，`break` 可带值 |
| `SwitchStmt` | `discriminant`, `cases` | 语句 |
| `MatchExpr` | `discriminant`, `arms` | 表达式 |
| `IfLetStmt` | `pattern`, `expr`, `then_branch`, `else_branch` | `if let` |
| `SelectStmt` | `branches` | `select` |

**验收标准：**
- AC-SEM-001: `let x = 42` → 产生 `LetStmt { name: "x", init: IntLiteral(42), mutable: false }`
- AC-SEM-002: `let x = if (c) { 1 } else { 0 }` → `LetStmt` 的 `init` 是 `IfExpr`

### SEM-REQ-002：AST → HIR 降级规则

**名称解析：**
- `import { foo } from "./bar"` → 解析 `bar.trust` → 验证 `foo` 导出存在 → 建立符号绑定
- `import * as ns from "./lib"` → `ns.foo` 解析为 `./lib` 中的 `foo` 导出

**作用域：**
- 函数参数和 `let` 变量 → 局部作用域
- `const` → 模块作用域
- `shared` → 模块作用域，`Arc` 包裹

**验收标准：**
- AC-SEM-003: `import { add } from "./math"; let x = add(1, 2);` → `add` 解析到 `./math.trust` 的导出
- AC-SEM-004: 未导入的标识符 → 编译错误

### SEM-REQ-003：类型检查规则

**名义类型：** `interface A { x: number }` 和 `interface B { x: number }` 不兼容。赋值/参数传递需显式构造或 trait 约束。

**类型推断：**
- 闭包参数：`nums.map(x => x * 2)` → 从 `nums` 的类型 `Vec<number>` 推断 `x: number`
- 单表达式体：`function square(x: number) = x * x` → 从 `x * x` 推断返回类型 `number`
- 泛型调用点：`identity(42)` → 从实参 `i32` 推断 `T = number`

**验收标准：**
- AC-SEM-005: `interface A { x: number }; interface B { x: number }; let a: A = { x: 1 }; let b: B = a;` → 编译错误（名义类型不兼容）
- AC-SEM-006: `let x = identity(42);` → `x` 推断为 `number`

### SEM-REQ-004：HIR → TIR 降级规则

**控制流图：** `if`/`for`/`loop` → 基本块 + 条件跳转

**表达式→语句：** `if` 和 `loop` 表达式的值通过临时变量持有

**方法调用展开：** `pt.print()` → `Printable::print(&pt)`

**闭包捕获提升：** 闭包体引用的外部变量提升为隐式参数

**验收标准：**
- AC-SEM-007: `let x = if (c) { 1 } else { 0 }` → TIR 中 `x` 的赋值通过临时变量持有
- AC-SEM-008: 闭包 `() => console.log(data)` → TIR 中 `data` 作为隐式只读参数传入

### SEM-REQ-005：编译管线编排

**需求：** 编译顺序：Parse → HIR（类型检查） → TIR（所有权检查） → 错误数=0? → Codegen → rustc

若 TIR 所有权检查有错误，跳过 codegen，通过 `--error-format=json` 输出错误。

**验收标准：**
- AC-SEM-009: TIR 错误 > 0 → codegen 不运行，JSON 错误输出
- AC-SEM-010: TIR 错误 = 0 → codegen 运行，生成可达 rustc 的 Rust 源码

---

## TYP：类型系统规范

### TYP-REQ-001：数字类型严格分离

**需求：** `number` 在词法阶段即区分整数（`i32`）和浮点（`f64`）。隐式混算禁止。

| 字面量 | 承载类型 | Rust 映射 |
|--------|---------|----------|
| `42` `0` `-7` | `i32` | `i32` |
| `3.14` `0.0` | `f64` | `f64` |
| `9007199254740991n` | `bigint` | `i64` |

**设计决策——方案 B（严格分离）：** 方案 A（`i32→f64` 自动提升）被否决——违反 §2.2 隐式转换禁止的安全承诺。方案 B 要求 `42 as f64 + 3.14`，TS 开发者付出的代价是显式 `as` 语法，换来"编译通过 = 数值无隐式精度丢失"。

**验收标准：**
- AC-TYP-001: `let a: i32 = 42; let b: f64 = 3.14; let c = a + b;` → 编译错误：`i32` 与 `f64` 不能混用
- AC-TYP-002: `let c = a as f64 + b;` → 类型检查通过，生成 Rust `a as f64 + b`
- AC-TYP-003: `let c = a + b as i32;` → 类型检查通过（b 截断为 3）

### TYP-REQ-002：ADT（标签联合）

**需求：** `type Msg = | { kind: "a" } | { kind: "b"; data: number }` 生成 Rust enum。

`switch` 和 `match` 在 ADT 上强制穷举检查——遗漏分支 → 编译错误。

**验收标准：**
- AC-TYP-004: ADT 定义后，`switch` 遗漏一个变体 → 编译错误（穷举检查）
- AC-TYP-005: `let label = match (msg.kind) { case "a" => 1, case "b" => 2 };` → 合法，穷举全部变体

### TYP-REQ-003：Dynamic 枚举

**需求：** `Dynamic` 是标准库类型，变体：
- `Dynamic.Number(n: i32)` / `Dynamic.Float(f: f64)`
- `Dynamic.String(s: String)` / `Dynamic.Boolean(b: bool)`
- `Dynamic.Array(arr: Vec<Dynamic>)` / `Dynamic.Null`
- `Dynamic.Object(map: HashMap<String, Dynamic>)`

模式匹配：`case Dynamic.Number(n) => n * 2`

**验收标准：**
- AC-TYP-006: `let val: Dynamic = 42; match (val) { case Dynamic.Number(n) => ..., case Dynamic.String(s) => ..., default => ... }` → 类型安全穷举

### TYP-REQ-004：Box<dyn Trait>

**需求：** `Box<dyn Trait>` 是 trait object。vtable 分发，不可穷举检查。与 `Dynamic` 的选择指南：已知集合 → Dynamic；开放集合 → `Box<dyn Trait>`。

**验收标准：**
- AC-TYP-007: `let pt: Box<dyn Serializable> = Box::new(Point { x: 1, y: 2 }); pt.serialize();` → vtable 分发，生成 Rust `Box<dyn Serializable>`

### TYP-REQ-005：?? 与 ?. 类型规则

**需求：**
- `expr1 ?? expr2` → `Option<T> ?? T → T` / `Result<T,E> ?? T → T`（映射 `unwrap_or`）
- `obj?.prop` → 字段类型为 `Option<U>` 时映射 `and_then`；字段类型非 `Option` 时映射 `map`
- `?.` 在 owned `Option` 上 move 原变量

**验收标准：**
- AC-TYP-008: `let name: string = maybeName ?? "anonymous";` → 类型检查通过，映射 `unwrap_or`
- AC-TYP-009: `let config: Config = loadConfig() ?? defaultConfig;` → Result 被 `??` 处理
- AC-TYP-010: `user?.name` → `name: string`（非 Option） → 映射 `map`
- AC-TYP-011: `user?.address?.street` → `address: Option<Address>`（Option） → 映射 `and_then`

### TYP-REQ-006：Send / Sync 推导

**需求：** 基于字段组成的自动推导：
- 所有字段 `Send` → 类型 `Send`
- 所有字段 `Sync` → 类型 `Sync`
- `Rc<T>` 不实现 `Send`；`Arc<T>` 实现 `Send`（若 `T: Send + Sync`）

**验收标准：**
- AC-TYP-012: struct 所有字段为 i32 → `Send + Sync` 自动推导
- AC-TYP-013: struct 包含 `Rc<i32>` → **不**推导 `Send`

---

## OWN：所有权规则规范

### OWN-REQ-001：移动语义

**需求：** `let b = a;` → `a` 所有权转移到 `b`，`a` 失效。后续访问 `a` → 错误 E0382。

**验收标准：**
- AC-OWN-001: `let a = [1,2,3]; let b = a; console.log(a.length);` → 编译错误 E0382

### OWN-REQ-002：三模式参数表

**需求：** 参数传递有三种模式。调用处必须对称标注：

| 声明 | 语义 | TIR 模式 | Rust 生成 | 调用示例 |
|------|------|---------|----------|---------|
| `f(x: T)` | 只读借用 | `ReadOnly` | `f(x: &T)` | `f(x)` |
| `f(inout x: T)` | 可变借用 | `Mutable` | `f(x: &mut T)` | `f(inout x)` |
| `f(move x: T)` | 所有权转移 | `Move` | `f(x: T)` | `f(move x)` |

**验收标准：**
- AC-OWN-002: `function pushOne(inout arr: number[]) { ... }; pushOne(inout data);` → 编译通过
- AC-OWN-003: `function pushOne(inout arr: number[]) { ... }; pushOne(data);` → 编译错误：缺少 `inout`
- AC-OWN-004: `function consume(move arr: number[]) { ... }; consume(move data);` → 编译通过

### OWN-REQ-003：借用规则

**需求：** 同一变量同时 ≤1 可变借用 或 ≥0 只读借用。`&` 运算符创建显式引用。

**验收标准：**
- AC-OWN-005: `let r1 = &data; let r2 = &data;` → 合法（多个只读借用）
- AC-OWN-006: `let r = &data; pushOne(inout data);` → 编译错误：同时存在只读和可变借用

### OWN-REQ-004：方法调用所有权

**需求：**
- `let obj = ...`（非 `mut`）→ 只能调用 `&self` 方法
- `let mut obj = ...` → 可调用 `&self` 和 `&mut self` 方法，调用期间独占借用

**验收标准：**
- AC-OWN-007: `let arr = [1,2,3]; arr.push(4);` → 编译错误：`push` 需要 `&mut self`，`arr` 不可变
- AC-OWN-008: `let mut arr = [1,2,3]; arr.push(4);` → 合法

### OWN-REQ-005：闭包捕获规则

**需求：**
- 默认闭包 `() => expr`：只读借用，`Fn`，可多次调用
- `move` 闭包 `move () => expr`：所有权转移，`FnOnce`，只能调用一次
- `spawn` 要求 `move` 闭包 + 捕获变量 `Send`

**验收标准：**
- AC-OWN-009: `let r = () => console.log(data); r(); r();` → 合法（默认借用，可多次调用）
- AC-OWN-010: `let c = move () => process(data); c(); c();` → 第二次调用编译错误（FnOnce）
- AC-OWN-011: `spawn(() => { ... })` → 编译错误：缺少 `move`
- AC-OWN-012: `spawn(move () => { ... })` → 合法

### OWN-REQ-006：引用计数

**需求：**
- `Rc::new(v)` → 引用计数 1；`clone()` → +1；`drop` → -1
- `Arc` 同 `Rc` 但原子计数；`Weak::upgrade()` → `Option<Rc<T>>`
- `Rc<T>` 不实现 `Send`；`Arc<T>` 实现

**验收标准：**
- AC-OWN-013: `spawn(move () => { let local = rc.clone(); })` → 编译错误（`Rc` 非 `Send`）
- AC-OWN-014: `spawn(move () => { let local = arc.clone(); })` → 通过（`Arc` 是 `Send`）

### OWN-REQ-007：for 循环隐式可变

**需求：** `for (let i = 0; i < N; i++)` 中 `i` 为隐式可变——唯一例外。

**验收标准：**
- AC-OWN-015: `for (let i = 0; i < 10; i++) { console.log(i); }` → 合法（无需 `let mut`）

---

## CON：并发规则规范

### CON-REQ-001：async 执行模型

**需求：** Trust 的 `async function` 返回惰性 Future——调用时零执行，状态机仅在 `.await` 或 `spawn` 时由 executor poll。

此决策由 Rust 编译目标物理约束决定（见设计文档 §5.1.1）。

**验收标准：**
- AC-CON-001: `let f1 = fetchUser(); let f2 = fetchConfig(); let u = await f1; let c = await f2;` → 两个操作**串行**执行（代码生成中 `f1` poll 时 `f2` 未被 poll）
- AC-CON-002: `let (u, c) = await join(fetchUser(), fetchConfig())?;` → 两个操作**并发**执行
- AC-CON-003: Trust 代码 `async function fetch()` 生成 Rust `async fn fetch()`（非 `tokio::spawn` 包装）

### CON-REQ-002：Send / Sync 使用侧检查

**需求：**
- `spawn`：闭包必须 `move` + `Send`
- `shared`：类型必须 `Sync`
- `Channel<T>`：`T: Send`

**验收标准：**
- AC-CON-004: `spawn(move () => { ... })` → 编译通过（所有捕获变量 Send）
- AC-CON-005: 包含 `Rc<T>` 的结构体在 `spawn` 中使用 → 编译错误

### CON-REQ-003：shared 优化

**需求：**
- `shared x: number` → `Arc<AtomicI32>`（原子操作，无锁）
- `shared x: T`（非 number）→ `Arc<Mutex<T>>`
- `withLock` 闭包接收 `&mut T`（auto-deref）

**验收标准：**
- AC-CON-006: `shared counter = 0; counter.withLock(c => { c += 1; });` → 生成 Rust `fetch_add(1, Ordering::Relaxed)`
- AC-CON-007: `shared data: Vec<number> = [...]; data.withLock(d => { d.push(1); });` → 生成 Rust `Arc<Mutex<Vec<i32>>>`

### CON-REQ-004：Channel 分离

**需求：** `Channel<T>(capacity)` → `(Sender<T>, Receiver<T>)`。`Sender: Clone`，`Receiver: !Clone`。

**验收标准：**
- AC-CON-008: `let (tx, rx) = Channel<number>(64);` → 返回元组，tx 和 rx 可分别 move
- AC-CON-009: `let tx2 = tx.clone();` → 合法（Sender 可 Clone）
- AC-CON-010: `let rx2 = rx.clone();` → 编译错误（Receiver 不可 Clone）

### CON-REQ-005：select

**需求：** `select { case x = future => { ... } }` 分支内**禁止** `await`。编译器在 `select` 上下文中自动 poll。`Result` 自动匹配 `Ok`，`Err` 视为分支不可用。全分支 `Err` → panic。

**验收标准：**
- AC-CON-011: `select { case msg = rx.receive() => { ... } }` → 合法（无 `await`）
- AC-CON-012: `select { case msg = await rx.receive() => { ... } }` → 编译错误：`select` 分支内不可 `await`
- AC-CON-013: `select { case msg = rx.receive() => ... }` 中 `receive()` 返回 `Err(ChannelClosed)` → 分支不被触发，继续等待其他分支

---

## ERR：错误处理规范

### ERR-REQ-001：Result 与 ? 传播

**需求：** `?` 作用在 `Result<T,E>`：`Ok(t)` → 提取 `t`；`Err(e)` → 立即返回 `Err(e)`。

**验收标准：**
- AC-ERR-001: `let file = fs.open("a.txt")?;` → 若 `open` 返回 `Ok`，`file` 类型为 `File`；若 `Err`，当前函数返回 `Err`
- AC-ERR-002: `?` 用于非 `Result` 类型 → 编译错误

### ERR-REQ-002：throw → panic! 映射

**需求：** `throw Error(msg)` 编译为 `panic!("{}", msg)`。不可捕获。仅用于逻辑不变量。

**验收标准：**
- AC-ERR-003: `throw Error("fatal")` → 生成 Rust `panic!("fatal")`
- AC-ERR-004: 无 `try/catch` 语法可用于捕获 `throw`

### ERR-REQ-003：! 断言操作符

**需求：** `expr!` → `expr.unwrap()`。**仅允许用于 `Option<T>`**。

**验收标准：**
- AC-ERR-005: `let val: Option<number> = None; let x = val!;` → 运行时 panic（编译通过）
- AC-ERR-006: `let res: Result<number, Error> = Err(...); let x = res!;` → 编译错误

### ERR-REQ-004：.expect() 方法

**需求：** `expr.expect("msg")` → `expr.unwrap()` 携带 message。适用于 `Option` 和 `Result`。

**验收标准：**
- AC-ERR-007: `let config = readConfig().expect("FATAL: config required");` → 编译通过
- AC-ERR-008: 若 `readConfig` 返回 `Err` → panic 并显示 `"FATAL: config required: <error details>"`

---

> **审计标记：** 本规范覆盖设计文档 §1–§11、§14.1、§15（被拒绝特性在 EBNF 中不存在）。  
> **下一步：** `spec/stdlib.md`（Phase 0.2）。`docs/phases/0/TODO.md` 中 0.1.1–0.1.7 全部可勾选。
