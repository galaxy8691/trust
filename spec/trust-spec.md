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

**需求：** Trust 的关键字不可作为标识符。v2.0 关键字表共 43 个，分类如下：

| 分类 | 关键字 | 说明 |
|------|--------|------|
| 声明 | `let` `mut` `const` `shared` | 变量声明 |
| 函数 | `function` `fn` | `fn` 仅用于 `extern` 块 |
| 参数 | `inout` `move` | 借用/所有权标注 |
| 并发 | `spawn` `async` `await` | 异步与并发（`select` 已移除 §14） |
| 控制流 | `if` `else` `for` `of` `while` `break` `continue` `return` `throw` | 控制流（`loop` 已移除 §14） |
| 匹配 | `switch` `case` `default` `match` | 模式匹配 |
| 模块 | `import` `export` `from` `as` | 模块系统 |
| 类型 | `type` `this` | 类型系统（`interface`/`impl`/`extends`/`dyn` 已移除 §14） |
| 测试 | `test` | 测试函数 |
| 外部 | `extern` | FFI 声明 |
| 字面量 | `true` `false` `null` | 值字面量（`undefined`/`None` 已移除） |
| 新增预留 | `unknown` `try` `catch` `panic` | Phase 3/4 实现（仅 lexer 预留） |
| 类型名 | `number` `string` `boolean` `void` | 内置类型名（`bigint` 已移除 §14） |

**验收标准：**
- AC-LEX-001: `let async = 42` → 词法错误（`async` 是关键字）
- AC-LEX-002: `function void() {}` → 词法错误（`void` 是类型名）
- AC-LEX-003: `let x: number = 42` → 合法
- AC-LEX-004: `extern "rust" { fn sqlx_query(query: string): string throws { message: string }; }` → `fn` 被识别为关键字（仅在 `extern` 块内合法）。v2.0: `throws` 替代旧 `Result<T,E>` 返回标注

**设计决策——`fn` vs `function`：** `extern "rust"` 块内使用 `fn` 而非 `function`，视觉区分 FFI 声明（映射 Rust 函数签名，不经过 Trust 所有权检查）与 Trust 自身函数。`fn` 仅在 `extern` 块内合法，全局不可作为标识符。方案 B（统一用 `function`）被否决——会使 FFI 块的"不安全边界"在视觉上不够明显。

### LEX-REQ-002：字面量

**需求：** v2.0 字面量格式与承载类型（5 种，已移除 BigInt `Nn` 后缀）：

| 种类 | 词法格式 | 承载类型 | 示例 |
|------|---------|---------|------|
| 整数 | `[0-9]+` | `number`(f64) | `42` |
| 浮点 | `[0-9]+ "." [0-9]+` | `number`(f64) | `3.14` |
| 字符串 | `"\"" ... "\""` | `string` | `"hello"` |
| 模板 | `` "`" ... "${" expr "}" ... "`" `` | `string`（展开为 format!） | `` `Hello, ${name}` `` |
| 布尔 | `true` `false` | `boolean` | |
| null | `null` | `T \| null`（内部 `Option<T>`） | `null` |

**验收标准：**
- AC-LEX-005: `42` → 词法器输出 `IntLiteral(42)`，类型 `number`(f64)（v2.0）
- AC-LEX-006: `3.14` → `FloatLiteral(3.14)`，类型 `number`(f64)（v2.0）
- AC-LEX-007: `9007199254740991n` → 词法错误（v2.0 已移除 BigInt `Nn` 后缀）

### LEX-REQ-003：运算符优先级

**需求：** 运算符按优先级从高到低排序。高数字 = 高优先级（更紧绑定）。同一优先级内按结合性。

| 优先级 | 运算符 | 结合性 | 类别 |
|--------|--------|--------|------|
| 15 | `()` `[]` `::` `.` `++` `--` | 左 | 调用、索引、构造器、成员、后置自增自减 |
| 14 | `expr!` `expr?` | 左 | 断言解包、Result 传播 |
| 13 | `&expr` `*expr` `!expr` `await expr` `++expr` `--expr` | 右→左 | 引用、**解引用**、逻辑非、异步等待、前置自增自减 |
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

> **EBNF 阅读说明：** `ident` 为词法原子（见 LEX-REQ-001），`expr` 的完整优先级解析由 LEX-REQ-003 运算符优先级表定义。以下 EBNF 中未展开的原子非终结符定义：`block ::= "{" stmt* "}"`；`stmt` 汇总见下文 `stmt` 产生式；`pattern ::= ident | literal | "{" pattern_field ("," pattern_field)* "}" | ident "(" pattern ")"`；`param_list ::= param ("," param)*`；`closure ::= ("move")? "(" param_list? ")" "=>" (expr | block)`。

```ebnf
-- 汇总规则（以下非终结符的完整产生式）
stmt    ::= var_decl | const_decl | shared_decl | function_decl
         | if_expr | for_stmt | while_stmt | loop_expr
         | return_stmt | break_stmt | continue_stmt
         | switch_stmt | match_stmt | if_let_stmt
         | expr_stmt
block   ::= "{" stmt* "}"
pattern ::= ident | literal | "{" pattern_field ("," pattern_field)* "}" | ident "(" pattern ")"
param_list ::= param ("," param)*
closure ::= ("move")? "(" param_list? ")" "=>" (expr | block)
```

### SYN-REQ-001：变量声明

```ebnf
  var_decl ::= ("let" | "let" "mut") ident (":" type)? "=" expr ";?"
           | "const" ident (":" type)? "=" expr ";?"
           | "shared" ident (":" type)? "=" expr ";?"
```

**验收标准：**
- AC-SYN-001: `let x = 42` → 成功解析为 `LetStmt`
- AC-SYN-002: `let mut y: number = 10;` → 成功解析
- AC-SYN-003: `const MAX = 100;` → 成功解析为 `ConstStmt`
- AC-SYN-004: `shared counter = 0;` → 成功解析为 `SharedStmt`

### SYN-REQ-002：函数声明

> **v2.0:** 移除 `<T extends ...>` 显式泛型语法，隐式泛型由参数无标注驱动（Phase 3）。块体函数强制返回类型标注。

```ebnf
function_decl ::= "function" ident "(" param_list? ")" (":" type)? ("{" stmt* "}" | "=" expr ";")
param         ::= ("inout" | "move")? ident (":" type)?
```

**块体函数返回标注规则：** `function f(...) { ... }` 必须显式标注返回类型，无返回值使用 `:void`。缺失 → 编译错误。
**表达式体函数：** `function f(...) = expr` 返回类型由表达式推断，可不标注。
**箭头函数：** 返回类型可省略，由表达式推断；标注时以标注为准。

**验收标准：**
- AC-SYN-005: `function add(a: number, b: number): number { return a + b; }` → 成功解析
- AC-SYN-006: `function square(x: number) = x * x;` → 表达式体函数解析，返回类型由 `x * x` 推断
- AC-SYN-007: `function pushOne(inout arr: number[]): void { arr.push(1); }` → inout 参数 + 块体 `:void`
- AC-SYN-008: `function greet(name: string) = \`Hello, ${name}\`;` → 表达式体模板字符串

### SYN-REQ-003：控制流

> **v2.0:** `loop` 已移除（`while (true)` 替代）。`break` 不再带值。

```ebnf
if_expr    ::= "if" "(" expr ")" block ("else" ("if" "(" expr ")" block | block))?
for_stmt   ::= "for" "(" ("let" ident "=" expr ";" expr ";" expr) ")" block
             | "for" "(" "let" ident "of" expr ")" block
while_stmt ::= "while" "(" expr ")" block
return_stmt ::= "return" expr? ";?"
break_stmt ::= "break" ";?"
```

**设计决策——`if` 是表达式：** `let x = if (c) { a } else { b };` 合法。`for`/`while` 是语句，无返回值。

**验收标准：**
- AC-SYN-009: `let label = if (score >= 60) { "pass" } else { "fail" };` → 解析为 `IfExpr`（表达式）
- AC-SYN-010: `while (true) { if (done) { break; } }` → `while (true)` 替代旧 `loop`
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

**设计决策——`switch`（语句）vs `match`（表达式）：** `switch` 用于副作用，分支用 `case X:` + `break`。`match` 用于值映射，分支用 `case X => expr,`。

**验收标准：**
- AC-SYN-013: `switch (msg.kind) { case "quit": return; case "data": process(msg.payload); break; }` → 穷举检查
- AC-SYN-014: `let label = match (msg.kind) { case "quit" => "bye", case "data" => "data" };` → `match` 表达式
- AC-SYN-015: `if let Some(val) = opt { process(val); }` → 成功解析
- AC-SYN-016: `if let Some(val) = opt { process(val); } else { default(); }` → if let ... else 解析

### SYN-REQ-005：异步

```ebnf
async_fn   ::= "async" "function" ident generic_params? "(" param_list? ")" (":" type)? block
await_expr ::= "await" expr   -- expr 的优先级解析遵循 LEX-REQ-003；await 优先级(13)高于 +(10)，parser 按优先级表归约
spawn_expr ::= "spawn" "(" ("move")? "async"? "(" param_list? ")" "=>" (block | expr) ")"
```

**验收标准：**
- AC-SYN-017: `async function fetch(): Result<Data, Error> { ... }` → 解析
- AC-SYN-018: `let data = await fetch();` → 解析
- AC-SYN-019: `spawn(move async () => { ... })` → 解析

### SYN-REQ-006：模块

```ebnf
import_decl ::= "import" (import_named | import_default | import_namespace) "from" string ";?"
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
- AC-SYN-025: `counter.withLock(c => { c += 1; });` → 解析

### SYN-REQ-008：类型别名（v2.0）

> **v2.0:** `interface`/`impl`/ADT（`type X = | ...`）已移除。仅保留 `type` 具名结构别名（Phase 3 实现）。

```ebnf
type_decl ::= "type" ident "=" type ";?"
```

**验收标准：**
- AC-SYN-026: `type Point2D = { x: number; y: number };` → 成功解析结构别名（Phase 3 实现 receiver 方法绑定）

### SYN-REQ-009：箭头函数与闭包

```ebnf
arrow_fn ::= ("move")? "(" param_list? ")" "=>" (expr | block)
closure   ::= ("move")? "(" param_list? ")" "=>" block
```

**验收标准：**
- AC-SYN-030: `let f = (x: number) => x * 2;` → 箭头函数解析
- AC-SYN-031: `let c = move () => process(data);` → move 闭包解析

### SYN-REQ-010：FFI（extern 块）

```ebnf
extern_decl   ::= "extern" string "{" extern_fn* "}"
extern_fn     ::= "fn" ident "(" param_list? ")" (":" type)? (";" | "throws" type ";")
```

**验收标准：**
- AC-SYN-032: `extern "rust" { fn sha256(data: number[]): [number; 32]; }` → FFI 解析
- AC-SYN-033: `extern "rust" { fn sqlx_query<T>(query: string): Result<T, SqlxError>; }` → 泛型 FFI 解析

### SYN-REQ-011：属性与测试语法

```ebnf
attribute    ::= "#[" ident ("(" expr ")")? "]"
test_decl    ::= attribute? "test" "async"? "function" ident "(" param_list? ")" block
```

**验收标准：**
- AC-SYN-034: `#[test] function add_works() { assert(1 + 1 == 2); }` → 属性语法解析
- AC-SYN-035: `test function subtract_works() { assert(5 - 3 == 2); }` → test 关键字语法解析

### SYN-REQ-012：引用、空值糖与构造器

**验收标准（语法通过上文 EBNF 覆盖，此处仅追加补充验收）：**
- AC-SYN-036: `let r = &data;` → `&` 引用解析
- AC-SYN-037: `let val = maybeValue!;` → `!` 断言解析
- AC-SYN-038: `let file = fs.open("a.txt")?;` → `?` 传播解析
- AC-SYN-039: `let street = user?.address?.street;` → `?.` 可选链解析
- AC-SYN-040: `let name = maybeName ?? "anonymous";` → `??` 空值合并解析
- AC-SYN-041: `let pt = Box::new(Point { x: 1, y: 2 });` → `::` 构造器解析

### SYN-REQ-013：错误恢复

**需求：** Parser 采用 panic mode。遇到语法错误后，跳过 token 直到同步点。

**同步点：** `;`（可选语句分隔符） `}` `function` `import` `export` `type` `interface` `impl` `test` `async`

**验收标准：**
- AC-SYN-037: 包含语法错误的 `.trust` 文件产生 ≥2 个诊断信息（非首个错误停止）
- AC-SYN-038: 错误恢复后，后续合法代码仍产出正确的 AST 节点

### SYN-REQ-014：语句分隔规则

**需求：** Trust 采用**换行即分隔**的语句规则（与 Go/TypeScript 一致）。分号 `;` **可选**——推荐省略，仅在同行多语句时用于分隔。

| 上下文 | 规则 |
|--------|------|
| 语句 | 换行自动分隔，`;` 可选（推荐省略） |
| 同行多语句 | `;` 分隔（如 `let x = 1; let y = 2`，但不推荐此风格） |
| `for` 子句 | `;` 分隔 init / condition / update（语法要求，非语句分隔） |
| `match` 分支 | `,` 分隔，最后一个可省略 |
| `switch` 分支 | 分支内语句换行分隔，`break` 退出（无需 `break;`） |
| 块 `{ }` | 最后表达式作为返回值（无 `;`） |
| `return expr` | 换行即返回；`return` 后无表达式 = 返回 void |

**设计决策——为何不像 Rust 强制分号：** Rust 用 `;` 区分表达式（无分号=返回值）和语句（有分号=无返回值）。Trust 的 `if`/`loop`/`match` 是表达式，`for`/`while`/`switch` 是语句——这个区分在**语法层面就已确定**（EBNF 结构），不需要分号来消歧。去分号减少 TS/JS 开发者的迁移摩擦，且避免了 Rust 常见的"忘写分号导致类型不匹配"的困惑。

**验收标准：**
- AC-SYN-SEP-001: `let x = 42\nlet y = 10` → 换行分隔，两条语句正确解析
- AC-SYN-SEP-002: `let x = { let y = 2; y }` → 块内最后表达式省略 `;` 作为返回值（`;` 在此处合法但可选）
- AC-SYN-SEP-003: `let x = match (v) { case Some(n) => n, case None => 0 }` → `match` 分支 `,` 分隔
- AC-SYN-SEP-004: `return 42\n}` → 换行即返回，无需 `;`

### SYN-REQ-015：类型标注语法

**需求：** 类型标注的完整 EBNF，覆盖基本类型、数组、元组、泛型、trait object 等。

```ebnf
type ::= "number" | "string" | "boolean" | "bigint" | "void"
       | ident                               -- 名义类型/ADT
       | type "[]"                            -- 数组
       | "[" type ("," type)* "]"             -- 元组
       | "[" type ";" number "]"              -- 固定大小数组（如 [number; 32]）
       | ident "<" type ("," type)* ">"       -- 泛型实例化
       | "Box" "<" "dyn" ident ">"            -- trait object
       | "Option" "<" type ">"                -- Option 简写
       | "Result" "<" type "," type ">"       -- Result 简写
       | "&" "'"? ident? type                 -- 引用（含可选生命周期）
       | "(" type ")"                         -- 分组
```

**验收标准：**
- AC-SYN-041: `let x: number[]` → 解析为数组类型
- AC-SYN-042: `let x: Box<dyn Serializable>` → 解析为 trait object

---

## SEM：语义规范

**设计决策——三层 IR 架构（AST → HIR → TIR）：** Trust 不直接在 AST 上进行类型和所有权分析——AST 是语法糖的"源代码视图"，HIR 消除语法糖并完成名称解析和类型检查，TIR 将控制流简化为基本块后执行所有权/借用检查。方案 B（AST 直接分析）被否决——同等能力下 AST 层分析需要处理语法糖变换（如 `if let` 展开为 `match`、方法调用展开为 `Printable::print(&pt)`），导致分析逻辑与语法耦合，增加编译器维护成本。三层分离使每层专注一个任务。

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
| `ForStmt` | `init`, `condition`, `update`, `body` | C-style `for` |
| `ForOfStmt` | `iterator`, `item`, `body` | for-of `for (let item of items)` |
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

### TYP-REQ-001：`number`=f64 ✅ 2.2 已冻结

> 本节已在 Phase 2.2 实现并冻结。编译器已统一 `number`=f64。

**需求：** `number` 统一为 64 位浮点（f64）。整数和浮点自由运算，不需要 `as` 转换。

| 字面量 | 承载类型 | Rust 映射 |
|--------|---------|----------|
| `42` `0` `-7` | `number`(f64) | `f64` |
| `3.14` `0.0` | `number`(f64) | `f64` |

**整数语义：** `number` 存储为 f64，以下场景编译器自动进行整数转换：

| 场景 | Trust 写 | Codegen 生成 | 说明 |
|------|---------|-------------|------|
| 数组索引 | `arr[n]` | `arr[n as usize]` | 自动转 |
| 循环计数 | `for (let i=0; i<N; i++)` | `i: f64`，`i++` → `i += 1.0` | |
| 长度/容量 | `arr.length` / `Channel<T>(64)` | 返回/期望 `number`(f64) | 编译器自动装箱/拆箱 |

**精度边界：** 超过 2^53 的整数可能丢失精度。编译器在检测到超出安全整数范围的字面量或数组索引时发出 `Warning` 级别诊断，附 `Help` 子诊断说明精度风险。

**位运算：** 位运算 `&`/`|`/`^`/`<<`/`>>` 仅允许 `number` 类型。编译器不保证浮点值上的位运算行为——开发者需确保操作数为整数。codegen 生成 `f64::to_bits()`→`u64`→位运算→`f64::from_bits()` 转换链。

**验收标准（Phase 2.2 实现后生效）：**
- AC-TYP-001: `let a = 42; let b = 3.14; let c = a + b;` → 类型检查通过（number 之间自由运算）
- AC-TYP-002: `let arr = [1,2,3]; let x = arr[1];` → codegen 生成 `arr[1 as usize]`
- AC-TYP-003: `let big = 9007199254740993;` → 超 2^53 字面量，发出 `Warning`

### TYP-REQ-002：ADT（标签联合）— ⚠️ v2.0 已废弃

> ⚠️ v2.0 已废弃。ADT（`type X = | ...`）由 `unknown` + `match` 替代（Phase 3）。本节保留为历史参考。

### TYP-REQ-003：Dynamic 枚举 — ⚠️ v2.0 已废弃

> ⚠️ v2.0 已废弃。`Dynamic` 枚举由 `unknown` + 类型化装载/`match` 替代（Phase 3），内部使用带类型标签的 `Value` 载荷，非虚表分发。本节保留为历史参考。

### TYP-REQ-004：Box<dyn Trait> — ⚠️ v2.0 已废弃

> ⚠️ v2.0 已废弃。禁止动态分发（设计 §14）。`Box`/`dyn` 关键字已移除。本节保留为历史参考。

### TYP-REQ-005：?? 与 ?. 类型规则

**需求：**
- `expr1 ?? expr2` → `Option<T> ?? T → T` / `Result<T,E> ?? T → T`（映射 `unwrap_or`）
- `obj?.prop` → 字段类型为 `Option<U>` 时映射 `and_then`；字段类型非 `Option` 时映射 `map`
- `?.` 在 owned `Option` 上 move 原变量

**验收标准：**
- AC-TYP-010: `let name: string = maybeName ?? "anonymous";` → 类型检查通过，映射 `unwrap_or`
- AC-TYP-011: `let config: Config = loadConfig() ?? defaultConfig;` → Result 被 `??` 处理
- AC-TYP-012: `user?.name` → `name: string`（非 Option） → 映射 `map`
- AC-TYP-013: `user?.address?.street` → `address: Option<Address>`（Option） → 映射 `and_then`

### TYP-REQ-006：Send / Sync 推导

**需求：** 基于字段组成的自动推导：
- 所有字段 `Send` → 类型 `Send`
- 所有字段 `Sync` → 类型 `Sync`
- `Rc<T>` 不实现 `Send`；`Arc<T>` 实现 `Send`（若 `T: Send + Sync`）

**验收标准：**
- AC-TYP-012: struct 所有字段为 i32 → `Send + Sync` 自动推导
- AC-TYP-013: struct 包含 `Rc<i32>` → **不**推导 `Send`

### TYP-REQ-007：泛型单态化规则

**需求：** 泛型函数在调用点单态化。`identity<T>(value: T): T` 被 `identity(42)` 调用时生成 `identity_i32`。每个不同的类型参数组合产生独立的 Rust 函数副本。

**设计决策——隐式 trait 生成：** `T extends { length: number }` 结构化约束时，编译器在后台生成隐式 trait `HasLength`，为 `Vec<T>`（→ `len()`）、`String`（→ `len()`）、`[T; N]`（→ 编译时常量）自动 impl。用户自定义类型可手动 `impl HasLength` 满足约束。此方案是 Trust 名义类型系统中唯一的"结构子类型"例外——仅用于减少公开 API 的样板代码。

**验收标准：**
- AC-TYP-014: `function first<T extends { length: number }>(x: T): number { return x.length; }` → `first([1,2,3])` 类型检查通过
- AC-TYP-015: struct 无 `length` 字段 → `first(myStruct)` → 编译错误

### TYP-REQ-008：this 隐式参数

**需求：** `impl` 块内的方法默认接收隐式参数 `this: &Self`。若需可变借用，声明 `inout this`；若需消耗性方法，声明 `move this`。`interface` 中不写 `this`——编译器自动在两端注入。

**验收标准：**
- AC-TYP-016: `impl Printable for Point { function print() { console.log(this.x); } }` → `this` 隐式绑定为 `&Point`
- AC-TYP-017: `pt.print()` → 编译为 `Printable::print(&pt)`

---

## OWN：所有权规则规范

### OWN-REQ-001：移动语义

**需求：** `let b = a;` → `a` 所有权转移到 `b`，`a` 失效。后续访问 `a` → 错误 E0382。

**验收标准：**
- AC-OWN-001: `let a = [1,2,3]; let b = a; console.log(a.length);` → 编译错误 E0382
- AC-OWN-002: `let a = 42; let b = a; console.log(a);` → 合法（`number` 是 Copy 类型，不触发移动）

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

**设计决策——`move` 闭包统一为 FnOnce：** Rust 区分 `Fn`/`FnMut`/`FnOnce` 三态——`move ||` 仅决定捕获方式，不决定可调用性。Trust 简化：`move` 关键字 = 所有权转移（与函数参数 `move x: T` 语义一致），一旦变量 move 进闭包即无法再次调用（FnOnce）。方案 B（保留 Rust 三态）被否决——增加认知负担，与 Trust 的"一关键字一语义"哲学冲突。多次调用场景使用默认借用闭包即可。

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
- AC-OWN-015b: `let i = 0; i += 1;` → 编译错误：`let` 变量不可变（非 for 循环上下文中无隐式可变例外）

### OWN-REQ-008：Copy 类型判定

**需求：** 类型是否实现 `Copy` 由编译器自动判定。基本规则：
- 标量类型（`number`(f64)、`boolean`）→ `Copy`
- 引用 `&T` → `Copy`
- 元组/数组若所有元素 `Copy` → `Copy`
- `Vec<T>`、`String`、`Box<T>`、`Rc<T>`、`Arc<T>` → **非** `Copy`

**设计决策——`number[]`（Vec<number>）不实现 Copy：** 与 Rust 一致——堆分配类型不能位拷贝。`let a = [1,2,3]; let b = a;` 是移动语义（§OWN-REQ-001），非隐式复制。

**验收标准：**
- AC-OWN-016: `let a: number = 42; let b = a; console.log(a);` → `a` 仍可用（`number` 是 Copy）
- AC-OWN-017: `let a = [1,2,3]; let b = a; console.log(a);` → 编译错误（`Vec<number>` 非 Copy）

### OWN-REQ-009：生命周期省略规则

**需求：** 生命周期标注仅在以下场景需要手动标注 `'a`，其余全部自动推导：

| 场景 | 需要标注？ | 说明 |
|------|----------|------|
| 函数返回非引用类型（如 `number`、`Vec<T>`） | ❌ 自动推导 | `function getLen(arr: number[]): number` — 无引用，无需生命周期 |
| 函数参数是引用，返回值是引用 | ✅ 需标注 | `function getFirst<'a>(arr: &'a number[]): &'a number` — 返回值生命周期绑定到参数 |
| 函数返回引用但无参数引用 | ✅ 需标注 | 返回值必须绑定到某个参数或标注为 `'static` |
| 结构体包含引用字段 | ✅ 需标注 | `struct Ref<'a> { data: &'a number }` |
| 方法调用（`&self`）返回 `&self` 的字段 | ❌ 自动推导 | 编译器自动绑定返回值生命周期到 `self` |
| 闭包捕获引用 | ❌ 自动推导 | 编译器推断最小生命周期 |

**设计决策——默认省略 vs 显式标注：** Trust 在绝大多数场景省略生命周期（与 Rust 的 elision rules 一致但更激进——方法返回 `&self` 字段时自动绑定）。仅在函数签名层面需要标注——当返回引用且无明确参数引用来源时。这与 Trust 的"局部便利，全局显式"哲学一致。

**验收标准：**
- AC-OWN-018: `function getLen(arr: number[]): number { return arr.length; }` → 无需生命周期标注，编译通过
- AC-OWN-019: `function getFirst<'a>(arr: &'a number[]): &'a number { return &arr[0]; }` → 合法，`'a` 手动标注
- AC-OWN-020: 返回引用且无标记 → 若 TIR 层无法推断来源 → 编译错误，提示添加 `'a` 标注

---

## CON：并发规则规范

### CON-REQ-001：async 执行模型

**需求：** Trust 的 `async function` 返回惰性 Future——调用时零执行，状态机仅在 `.await` 或 `spawn` 时由 executor poll。

此决策由 Rust 编译目标物理约束决定（见设计文档 §5.1.1）。

**验收标准：**
- AC-CON-001: `let f1 = fetchUser(); let f2 = fetchConfig(); let u = await f1; let c = await f2;` → 两个操作**串行**执行（代码生成中 `f1` poll 时 `f2` 未被 poll）
- AC-CON-002: `let (u, c) = await join(fetchUser(), fetchConfig())?;` → 两个操作**并发**执行
- AC-CON-003: Trust 代码 `async function fetch()` 生成 Rust `async fn fetch()`（非 `tokio::spawn` 包装）

**设计决策——惰性 Future vs JS 热启动 Promise：** JS 的 `async function` 调用即启动（Promise 热启动）。Trust 选择惰性 Future（与 Rust 一致）——调用时仅创建状态机，不执行任何代码，仅在 `.await` 或 `spawn` 时由 executor poll。方案 B（热启动）在 Rust 代码生成层面不可行：自动 `tokio::spawn` 要求闭包 `'static`，预 `poll` 无 waker 会永久挂起，全局 executor 导致 Future 无法返回调用者。详见设计文档 §5.1.1。

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

**设计决策——Channel 分离 vs 共享引用：** `Channel<T>()` 返回 `(Sender, Receiver)` 而非单个 `Channel` 对象。方案 B（共享引用——`chan.send()` 和 `chan.receive()` 通过同一变量调用）被否决：`spawn(move || { chan.send(); })` 后 `chan` 被 move，无法再从另一个 `spawn` 调用 `receive()`。`(tx, rx)` 分离使发送端和接收端可分别 `move` 进不同线程/任务，`Sender: Clone` 支持多个发送方。

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

**设计决策——`!` 仅限 Option：** `!` 断言 "我知道这里有值"，适用于 `Option`（开发者可控制是否有值）。`Result` 的错误是不可控的（I/O 失败、网络断开）——允许 `Result!` 将训练开发者习惯性忽略错误。方案 B（`!` 同时用于 `Result`）被否决——与 §6.1 的显式 `?` 传播哲学和 Trust 的 "渐进式安全" 承诺冲突。`Result` 的断言使用 `.expect()` 携带错误消息。

### ERR-REQ-004：.expect() 方法

**需求：** `expr.expect("msg")` → `expr.unwrap()` 携带 message。适用于 `Option` 和 `Result`。

**验收标准：**
- AC-ERR-007: `let config = readConfig().expect("FATAL: config required");` → 编译通过
- AC-ERR-008: 若 `readConfig` 返回 `Err` → panic 并显示 `"FATAL: config required: <error details>"`

---

> **审计标记（Phase 0.3 审计后修正）：** 本规范覆盖设计文档 §2（保留/牺牲特性）、§3–§7（类型/所有权/并发/错误/模块）、§8 FFI 部分、§9.1 编译管线、§11（全部 20 子节语法参考）、§14.1（测试语法）、§15（被拒绝特性在 EBNF 中不存在）。  
> **未覆盖：** §1（设计哲学——非规范内容）、§7.2–§7.3（包管理/动态导入——工具链范畴）、§9.2–§9.5（ferro_rt/source map/trust eval/`--fix`——工具链范畴）、§10（标准库——由 `spec/stdlib.md` 覆盖，Phase 0.2）、§12–§14.2+（未来展望/AI 友好性/高级测试——辅助内容）。  
> **下一步：** `spec/stdlib.md`（Phase 0.2）。`docs/phases/0/TODO.md` 中 0.1.1–0.1.7 全部可勾选。
