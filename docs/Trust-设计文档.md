# Trust 语言设计文档

**代号:** Trust  
**理念:** TypeScript 的优雅，Rust 的安全，编译到原生代码。  
**目标:** 提供一种在语法和心智模型上贴近 TypeScript，但在编译时能彻底消除内存错误和数据竞争的系统编程语言。

---

## 1. 引言

### 1.1 背景与动机

TypeScript 以其出色的类型系统、现代语法和开发体验征服了前端与全栈领域。但在系统编程、高性能计算、嵌入式或多线程高并发场景下，JavaScript 引擎的 GC 停顿、单线程模型和动态类型所带来的性能天花板与安全隐患始终存在。Rust 提供了零成本抽象、所有权系统和无畏并发，但其学习曲线陡峭，生命周期标注、借用检查等概念对大量开发者形成了障碍。

Trust 尝试融合二者之长：用 TypeScript 的语法写逻辑，用 Rust 的所有权和并发模型保安全。编译器将 Trust 源码直接翻译为 Rust 源码，再由 Rust 工具链编译为原生二进制。

### 1.2 设计哲学

- **渐进式安全：** 默认行为即安全（不可变、所有权移动），不安全操作需显式标记。
- **零运行时开销：** 所有并发安全、内存安全在编译期保证，不引入隐式 GC 或重量级运行时。
- **错误信息友好：** 编译错误映射回原始 Trust 代码，并提供符合开发者心智的修复建议。
- **语法亲和：** 尽可能保持 TypeScript/JavaScript 的语法风格，降低迁移成本。

---

## 2. 核心设计决策：保留与牺牲

### 2.1 从 TypeScript 保留的优雅特性

- `let`、`const` 变量声明（语义重新定义）
- 箭头函数、解构、模板字符串
- `async`/`await` 异步语法
- `interface`、泛型、联合类型（加强为代数数据类型）
- 模块化导入/导出语法
- 类型推断与上下文推导
- `#[...]` 属性语法（用于测试标注、条件编译等元编程场景）

### 2.2 必须牺牲的特性

为了获得 Rust 级别的静态安全，以下 TS 特性被完全移除或严格限制：

| 特性 | 处理方式 |
|------|----------|
| `any` / `unknown` 的动态派发 | 移除。动态值必须用 tagged union 或 `Box<dyn Trait>` 模拟，且需穷举匹配 |
| 对象动态增删属性 | 移除。对象编译为固定字段的结构体，禁止运行时添加新属性 |
| 垃圾回收（GC） | 移除。采用所有权 + 引用计数（可选），无 GC 停顿 |
| 原型继承、`prototype`、`__proto__` | 移除。用组合与接口代替，不再支持动态修改原型链 |
| `eval`、`new Function` | 完全禁止 |
| `Proxy`、`Reflect` | 不支持。底层操作无钩子可挂载 |
| 可抛出任意值的异常 | 限制为错误类型，用 `Result` 传递错误 |
| 隐式类型转换（如 `"5" * 2`） | 禁止。数字类型明确，转换需显式 |
| 完全动态的 `import()` | 路径必须静态可分析，动态部分受限 |
| 循环引用自动回收 | 引用计数可能导致泄漏，需用弱引用或重构 |

这些牺牲换来的是完全静态可分析的内存图，这是并发安全的基础。

---

## 3. 类型系统

### 3.1 静态强类型与名义类型

Trust 采用**名义类型系统**为主，但允许通过 `type` 定义结构别名。对象的兼容性由显式声明或自动推导的 trait 决定，而非鸭子类型。

> **`type` 的完整语义：** `type` 是类型定义关键字，右侧语法决定语义——
> - 右侧为对象字面量或结构体类型（`{ ... }`） → 生成**透明结构别名**（type alias），等价于直接书写右侧类型。
> - 右侧以 `|` 开头（联合语法） → 生成**标签联合**（编译为 Rust enum），这是一个具有独立类型身份的新类型。
> 
> TypeScript 中 `type` 也是这样工作的：`type X = {a: number}` 和 `type X = "a" | "b"` 均由右侧语法决定产物。Trust 的差异在于 `|` 联合被提升为带标签的枚举（ADT）而非 TS 的字面量联合——这是有意设计，详见 §3.2。
>
> **`type` vs `interface` 语义差异：** 在 TypeScript 中，`type` 和 `interface` 在许多场景下可以互换（均为结构类型）。在 Trust 中：
> - **`interface`** 定义的是一个**名义类型（nominal type）**——两个字段完全相同的 interface 被视为不同的类型，不能互相赋值，除非显式声明实现了对方的 trait。
> - **`type`** 用作结构别名时是**透明的**——`type Point2 = {x: number, y: number}` 等价于直接书写 `{x: number, y: number}`；用作 ADT 定义时是**不透明的**（新类型身份）。
> 
> 这与 TS 的行为不同：TS 中 `interface Point {x:number; y:number}` 和 `type Point = {x:number; y:number}` 在大多数场景等价。Trust 的差异设计是有意的：`interface` 用于需要类型身份的场景（如跨模块 API 边界），`type` 用于便利的局部类型缩写和 ADT 定义。

```ts
interface Point {
    x: number;
    y: number;
}

function distance(p: Point) { /* ... */ }

// 必须显式构造或标注符合 Point 的对象
let pt: Point = { x: 1, y: 2 };
```

#### 3.1.1 类型推断增强

Trust 在 TypeScript 的局部类型推断基础上进一步减少标注需求：

- **闭包参数推断：** 当闭包作为高阶函数参数时，参数类型从上下文自动推导。
```ts
let nums = [1, 2, 3];
let doubled = nums.map(x => x * 2);       // x 推断为 number
let filtered = nums.filter(x => x > 0);   // x 推断为 number
```

- **函数返回值推断（单表达式体）：** 当函数体为单个表达式时，返回值类型自动推导。
```ts
function add(a: number, b: number) = a + b;  // 等号语法 = 单表达式体，返回值推断为 number
```

- **泛型参数推断：** 调用泛型函数时，类型参数从实参自动推导。
```ts
function identity<T>(value: T): T { return value; }
let result = identity(42);  // T 推断为 number
```

> **边界规则：** 函数签名（多语句体、公开 API）仍需显式标注返回值类型——这是有意设计，确保公开 API 的契约显式可读。仅单表达式私有函数和闭包参数享受全推断。这与 Trust 的"名义类型 + 显式契约"哲学一致：局部便利，全局显式。

#### 3.1.2 结构体字面量简写 `{ x, y }`

当变量名与字段名一致时，Trust 支持 TypeScript 风格的属性简写：

```ts
let x = 10;
let y = 20;
let pt: Point = { x, y };    // 等价于 { x: x, y: y }

// 也支持混合写法
let name = "Alice";
let user = { name, age: 30 };
```

> **约束：** 简写仅在**类型上下文明确**时有效——即赋值目标、函数参数、返回值类型三者之一已知。对于 `let obj = { x, y }` 且无法从上下文推断目标类型时，编译器将推断为匿名结构体（`type` 别名），后续传给名义类型接口时需显式标注。这避免了简写语法破坏名义类型安全。

### 3.2 代数数据类型（增强版联合类型）

TS 的字面量联合类型被提升为带标签的枚举，保证穷举检查。

```ts
type Msg =
    | { kind: "quit" }
    | { kind: "data"; payload: number[] }
    | { kind: "error"; message: string };

function handle(msg: Msg) {
    switch (msg.kind) {
        case "quit": return;
        case "data": process(msg.payload); break;
        case "error": log(msg.message); break;
    } // 遗漏分支 → 编译错误
}
```

#### 3.2.1 `if let` 单分支模式匹配

当只需匹配一个变体且不关心其他分支时，`if let` 避免全量 `match` 的穷举负担：

```ts
// 替代仅有单分支的 match
let msg: Msg = { kind: "data", payload: [1, 2, 3] };

if let { kind: "data", payload } = msg {
    process(payload);
}

// 对 Option 的解包
let opt: Option<number> = Some(42);
if let Some(val) = opt {
    console.log(val);         // val 绑定到 42
}

// 对 Result 的错误处理
let result = readConfig();
if let Err(e) = result {
    console.log(`warning: ${e}`);  // 仅处理错误，成功不做事
}

// if let ... else —— 匹配失败时的分支
if let Some(val) = opt {
    console.log(`found: ${val}`);
} else {
    console.log("not found");
}

// if let 结合所有权检查
let msg: Option<number[]> = Some([1, 2, 3]);
if let Some(arr) = msg {
    process(arr);        // arr 被 move 出 Option
}
// console.log(msg);     // ❌ msg 内容已被 move
```

> **设计说明：** `if let` 是 Trust 中 `match` 的语法糖——当编译发现 `if let` 时，TIR 层展开为完整 `match` 并在未匹配分支插入空操作。穷举检查在此处不适用（未匹配分支是隐式空操作，有意为之），但如果你在 `if let` 匹配后继续使用原变量，所有权分析照常进行——匹配成功的变体内容被 move，原变量部分不可用。`if let ... else` 支持完整的双分支路径，else 分支中可安全使用未匹配的剩余字段。

#### 3.2.2 `match` vs `switch`——表达式与语句

Trust 从 TypeScript 继承 `switch`，同时引入 Rust 风格的 `match` 表达式。两者的关键区别：

| | `switch` | `match` |
|---|---------|---------|
| **角色** | 语句（无返回值） | 表达式（返回值） |
| **分支语法** | `case X: statement; break;` | `case X => expr,` |
| **穷举检查** | 对 ADT 强制穷举 | 强制穷举所有变体 |
| **适用场景** | 多分支副作用（I/O、赋值） | 多分支值映射 |

```ts
// switch —— 语句（适合副作用）
switch (msg.kind) {
    case "quit":
        shutdown();
        break;
    case "data":
        process(msg.payload);
        break;
} // 遗漏分支 → 编译错误

// match —— 表达式（适合值映射）
let label = match (msg.kind) {
    case "quit" => "bye",
    case "data" => `got ${msg.payload.length} items`,
};
```

#### 3.2.3 `impl`——Trait 实现

`impl` 关键字为类型实现 `interface` 中定义的 trait。`impl` 块内的方法自动接收隐式参数 `this: &Self`（只读借用），等价于 Rust 的 `&self`。若需可变借用，声明 `inout this`；若需消耗性方法，声明 `move this`。`this` 是关键字，仅在 `impl` 块的方法体内可用。

```ts
interface Printable {
    print(): void;
}

// 为 Point 实现 Printable
// 方法 print() 隐式接收 this: &Point（不可变借用）
impl Printable for Point {
    function print() {
        console.log(`Point(${this.x}, ${this.y})`);
    }
}

let pt: Point = { x: 1, y: 2 };
pt.print();  // 编译器编译为 Printable::print(&pt)
// "Point(1, 2)"
```

> **签名同步说明：** `interface` 中声明 `print(): void` 不需要也不允许写 `this` 参数；`impl` 块中的方法签名同样不写 `this`——编译器在两端自动注入。这确保了接口声明与实现签名在视觉上一致，同时底层 vtable 布局与 Rust 的 `&self` 完全对应。

`impl` 支持泛型实现、条件实现（`impl<T: Clone> Clone for Box<T>` 等），编译为 Rust 的 `impl Trait for Type` 块。

### 3.3 泛型与约束

泛型语法与 TS 一致，支持 `extends` 约束，可映射到 Rust 的 trait bound。

```ts
function firstElement<T extends { length: number }>(arr: T): number {
    return arr.length;
}
```

**编译模型：** 当泛型约束使用结构化类型（如 `{ length: number }`）时，Trust 编译器在后台自动生成一个隐式 trait（如 `HasLength`），并为所有满足该结构的内置类型（`Vec<T>` → `len()`、`String` → `len()`、`[T; N]` → 编译时常量）自动实现该 trait。用户自定义类型若需要满足该约束，可以手动 `impl` 该隐式 trait，或使用 `interface` 声明显式 trait。

> **注意：** 这是 Trust 类型系统中唯一的"结构子类型"场景，与 §3.1 的总体名义类型策略形成了**受控的例外**。如果滥用结构化约束（在大型泛型函数中使用多字段的结构化 `extends`），隐式 trait 的自动实现会显著增加编译时间。推荐在公开 API 中使用 `interface` 声明显式 trait，仅在内部便利函数中使用结构化约束。

### 3.4 无 `any` 的替代方案

需要动态类型时，Trust 提供两种机制，分别适用于不同场景：

#### 3.4.1 `Dynamic` 枚举（tagged union）

标准库提供 `Dynamic` 枚举，囊括常见内置类型。适用于已知类型集合有限的场景。

```ts
let val: Dynamic = 42;
match (val) {
    case Dynamic.Number(n) => console.log(n * 2);
    case Dynamic.String(s) => console.log(s);
    default => throw Error("unexpected type");
}
```

- **优点：** 栈上分配（无堆开销）、模式匹配强制穷举、类型安全。
- **缺点：** 内存布局为 `tag + union`（等于最大变体大小），无法容纳用户自定义类型，每次访问需 match（O(n) 分发）。
- **适用场景：** JSON 值、配置文件解析、简单的事件载荷。

#### 3.4.2 `Box<dyn Trait>`（动态分发）

当类型集合在编译时未知或需要容纳用户自定义类型时，使用 trait 对象。

```ts
interface Serializable {
    serialize(): string;
}

function log(value: Box<dyn Serializable>) {
    console.log(value.serialize());  // vtable 分发，O(1)
}
```

- **优点：** 可容纳任意实现 trait 的类型（包括用户自定义类型），vtable 分发为 O(1)。
- **缺点：** 堆分配开销，无法在编译时穷举所有可能的类型变体。
- **适用场景：** 插件系统、回调注册、GUI 事件处理、依赖注入。

> **选择指南：** 如果你能枚举所有可能的类型（如 JSON 的 `null | bool | number | string | array | object`），用 `Dynamic`。如果类型集合是开放的需要扩展的，用 `Box<dyn Trait>`。

### 3.5 空值与可选链语法糖

Trust 为 `Option<T>` 提供两个贴近 TS 开发习惯的语法糖，映射到安全的 `Option` 方法：

#### 3.5.1 空值合并 `??`

`expr1 ?? expr2` 映射为 `expr1.unwrap_or(expr2)`。当 `expr1` 为 `Option::None` 或 `Result::Err` 时返回 `expr2`。同时适用于 `Option<T>` 和 `Result<T,E>`。

```ts
let config = loadConfig() ?? defaultConfig;
let name = maybeName ?? "anonymous";

// 与 ? 操作符组合
let user = db.findUser(id)?;
let display = user.nickname ?? user.username;
```

> **安全检查：** `??` 映射为 `unwrap_or`，不涉及 `unwrap`（panic）。用于 `Result` 时提供默认值，但**静默丢弃错误信息**——适合错误可忽略的场景。若需保留或处理错误，应使用 `match` 或 `?`。这是与 `!` 操作符的关键区别：`!` 发生 panic，`??` 优雅降级。

#### 3.5.2 可选链 `?.`（受限版）

`obj?.prop` 根据目标字段类型自动选择映射方法——若字段类型为 `Option<U>`，映射为 `obj.and_then(|o| o.prop)`；若字段类型为非 `Option`（如 `string`、`number`），映射为 `obj.map(|o| o.prop)`。链式 `?.` 中每个环节独立判断，编译器在 TIR 层根据类型信息自动选择。

```ts
// TS 风格
let street = user?.address?.street;

// 编译为 Trust 的 Option 链
let street = user.and_then(u => u.address).and_then(a => a.street);
```

> **所有权约束：** `?.` 在 Trust 中的行为取决于上下文——
> - 如果 `obj` 是 `&Option<T>`（只读借用），`?.` 不消耗所有权，`obj` 后续仍可使用。
> - 如果 `obj` 是 `Option<T>`（owned），`?.` 会 **move** `obj`，之后 `obj` 失效。这是 Trust 与 TS 的关键差异：TS 中 `obj?.prop` 后 `obj` 仍可用，Trust 中如果传递了所有权则不可用。
> - 推荐：在只读借用上下文（函数参数默认即只读借用）中使用 `?.`，或在 move 后不再需要原变量时使用。
>
> 编译器在 move 场景中会给出明确提示："`obj` 已被 `?.` move，如果你需要保留 `obj`，使用 `obj.clone()?.prop` 或先取 `&obj` 的引用。"

---

## 4. 所有权与内存管理

### 4.1 移动语义作为默认赋值

`let b = a;` 后 `a` 所有权转移给 `b`，`a` 失效。编译器会在后续使用 `a` 时给出建议："`a` 已被移动，你可能需要 `.clone()`"。

> **与 TypeScript 的语义断裂：** 这是 Trust 与 TypeScript 之间最根本的语义差异。TS 中 `let b = a` 后 `a` 和 `b` 共享同一份数据（引用语义）；Trust 中 `a` 立即失效（所有权转移）。`let b = a; b.push(1); console.log(a.length)` 这种在 TS 中极其常见的模式，在 Trust 中是编译错误。对于不需要转移所有权的场景，使用 `let b = a.clone()`（显式克隆）或将 `a` 作为只读借用传入（不消耗所有权）。这一差异是 Trust 获得内存安全所付出的"语法税"——它强制开发者显式思考数据的生命周期，而这正是消除 use-after-free 和 double-free 的代价。

### 4.2 不可变默认与 `mut`

变量默认不可变（即使使用 `let`），需用 `let mut` 声明可变绑定。

```ts
let x = 5;      // 不可变
let mut y = 10; // 可变
y += 1;
```

> **C-style for 循环例外：** `for (let i = 0; i < N; i++)` 中的迭代变量 `i` 隐式可变（`i++` 合法）。这是 Trust 中唯一允许 `let` 声明的变量被修改的场景——for 循环的迭代语义隐含了对计数器的修改需求，强制 `let mut` 会产生不必要的样板代码。`for-of` 循环和 `while` 循环不受此例外影响。

### 4.3 借用与 `inout` 关键字

函数参数默认是只读借用，不消耗所有权。若需要修改传入变量，使用 `inout` 关键字；若需要消耗所有权，使用 `move` 关键字。每个模式直观表达调用处的所有权意图。

```ts
function pushOne(inout arr: number[]) {
    arr.push(1);
}

let data = [1,2,3];
pushOne(inout data);  // data 被修改，调用期间独占借用
```

编译器在后台应用 Rust 的借用规则：同一时刻只能有一个可变借用或多个只读借用。

参数所有权有三种传递模式：

| 声明 | 语义 | 对应 Rust |
|------|------|----------|
| `function f(x: T)`（默认） | 只读借用，不消耗所有权，`x` 在调用后可继续使用 | `fn f(x: &T)` |
| `function f(inout x: T)` | 可变借用，独占访问，调用期间原始变量被冻结 | `fn f(x: &mut T)` |
| `function f(move x: T)` | 所有权转移，`x` 在调用后失效（仅当需要存储或转发数据时使用） | `fn f(x: T)` |

**显式引用：** Trust 提供 `&` 运算符创建只读引用（对应 Rust `&`）。绝大多数情况下隐式借用足够——函数参数默认只读借用，方法调用自动取引用。仅在需要显式声明引用变量时使用 `&`：

```ts
let data = [1, 2, 3];
let r1 = &data;    // 显式只读引用
let r2 = &data;    // ✅ 多个只读引用 OK
// let r3 = inout data;  // ❌ 同时存在只读引用和可变引用
```

#### 4.3.2 闭包捕获规则

闭包捕获外部变量的行为与函数参数保持一致——默认只读借用，不消耗所有权。若需要将变量所有权转移进闭包，使用 `move` 关键字：

```ts
let data = [1, 2, 3];

// 默认只读借用 —— 闭包可多次调用，data 仍可用
let read = () => console.log(data.length);
read();
read();              // ✅ 借用闭包可多次调用
console.log(data);    // ✅ data 仍可用

// move 闭包 —— 所有权转移，闭包变为 FnOnce
let data2 = [4, 5, 6];
let consume = move () => process(data2);
consume();
// consume();         // ❌ move 闭包只能调用一次
// console.log(data2); // ❌ data2 已被 move
```

> `spawn` 要求闭包为 `move` 且捕获变量满足 `Send`——这与跨线程所有权转移的要求一致。调用 `spawn(move () => ...)` 在调用处可视化所有权转移，符合 Trust 的显式所有权哲学。

> **设计取舍——`move` 闭包统一为 FnOnce：** 与 Rust 不同，Trust 将 `move` 闭包统一视为 `FnOnce`（只能调用一次），不区分 `Fn`/`FnMut`/`FnOnce` 三态。理由：① `move` 关键字在函数参数中意味着"所有权转移，原变量失效"，闭包的 `move` 是同一语义的延伸——一旦将变量 move 进闭包，闭包就是唯一拥有者，多次调用会引入隐式借用语义，破坏"一关键字一语义"的简化原则；② `spawn` 场景本就只调用一次闭包；③ 对于需要多次调用的回调场景，使用默认借用闭包（不写 `move`）即可。这一简化牺牲了 Rust 的灵活性，但使 `move` 的语义对 TS 开发者更可预测。

#### 4.3.1 方法调用与所有权

Trust 中对象方法调用的可变性受变量绑定控制，规则如下：

- **`let obj = ...`（不可变绑定）：** 对象方法默认以 `&self`（只读借用）调用。可以调用只读方法（如 `.len()`、`.get()`），但**不能调用需要 `&mut self` 的方法**（如 `.push()`、`.insert()`）。
- **`let mut obj = ...`（可变绑定）：** 对象方法可以 `&mut self` 调用所有方法。但与 Rust 的借用规则一致：调用期间 obj 被独占借用，其他引用失效。
- **临时可变借用：** 对于 `let` 声明的不可变数组、Map 等容器，若需要调用可变方法，有两种方式：① 声明为 `let mut`；② 将对象传入 `inout` 参数的函数，在函数内部进行可变操作。

```ts
// 不可变绑定：只能调用只读方法
let arr = [1, 2, 3];
console.log(arr.length);   // ✅ 合法（只读）
// arr.push(4);            // ❌ 编译错误：push 需要 &mut self，但 arr 是不可变的

// 可变绑定：可以调用所有方法
let mut arr2 = [1, 2, 3];
arr2.push(4);              // ✅ 合法
arr2.sort();               // ✅ 合法
```

> **设计权衡：** 这与 TypeScript 的默认行为截然不同（TS 中 `const arr = []` 的 `arr.push()` 完全合法）。Trust 牺牲了这种便利性以换取编译期对可变状态的精确追踪。对于习惯了 TS/JS 的开发者，这是迁移过程中最需要适应的差异之一——但换来的是在多线程场景下可以**静态证明**没有任何代码在你不期望的时候修改了你的数据。

### 4.4 生命周期自动推导

绝大多数情况下生命周期无需标注。仅在返回引用或复杂结构中需要轻量标注，编译器会提供自动修复提示，引导开发者添加类似 `'a` 的标记。

### 4.5 引用计数与克隆

标准库提供 `Rc<T>` / `Arc<T>` 用于共享所有权，但循环引用需自行使用 `Weak` 打破。对简单值类型或需要显式复制的场景，调用 `.clone()` 方法（编译器可自动推导 `Clone`）。

---

## 5. 并发模型

Trust 的并发模型是设计的核心亮点，它从语言层面消灭数据竞争。

### 5.1 线程与异步任务

Trust 区分两种并发执行单元：

- **`spawn`（OS 线程）：** 启动一个操作系统线程，适用于 CPU 密集型任务。闭包必须为 `move` 闭包，捕获变量必须满足 `Send` 约束（自动推导），因为数据会被移动到另一个线程。

```ts
spawn(move () => {
    doHeavyWork();  // CPU 密集型
});
```

- **`spawn async`（异步任务）：** 在异步运行时上启动一个用户态任务（绿色线程），适用于 I/O 密集型、高并发场景（可同时运行成千上万个任务）。闭包同样必须为 `move` 闭包，捕获变量必须满足 `Send` 约束（因为任务可能在多线程 executor 上调度）。

```ts
spawn(move async () => {
    let data = await fetchData();  // I/O 密集型
    process(data);
});
```

两者共享同一个 `spawn` 关键字和 `move` 闭包语法，由编译器根据闭包是否为 `async` 自动选择目标：非 async 闭包编译为 `std::thread::spawn`，async 闭包编译为 Tokio（或配置的异步运行时）的 `task::spawn`。

#### 5.1.1 Async 执行模型：惰性 Future

Trust 的 `async function` 返回**惰性 Future**，与 Rust 语义一致——调用时创建 Future 状态机但不执行任何代码，仅在 `.await` 或 `spawn` 时由运行时 poll 推进。这与 TypeScript 的 Promise（调用即启动）有本质差异：

```ts
// TS 习惯（Trust 中仍是串行！）
let f1 = fetchUser(42);     // 创建惰性 Future，未执行
let f2 = fetchConfig();     // 创建惰性 Future，未执行
let user = await f1;        // poll f1，期间 f2 从未被 poll
let config = await f2;      // poll f2
// 结果：两个操作**串行执行**

// Trust 并发写法：使用 join() 或 spawn
let (user, config) = await join(fetchUser(42), fetchConfig())?;
// 结果：两个操作**并发执行**
```

> **为什么是惰性 Future 而非 JS 热启动？** Trust 编译到 Rust，Rust 的 Future 是基于协作式调度的惰性状态机。没有 executor 的 poll 循环，Future 就是死代码——这是 Rust 语言本身的物理约束，不是偏好。Trust 通过 `join()` 函数和 `spawn` 提供并发替代方案，并计划提供编译器 lint 在检测到串行 await 模式时给出并发提示。

编译器 lint（计划 v0.2+）：当检测到 `let f1 = asyncFn(); let f2 = asyncFn(); await f1; await f2;` 模式时发出 `help` 级别提示："f1 和 f2 是惰性 Future，此处串行执行。若需并发，使用 join(f1, f2) 或 spawn"。

### 5.2 消息传递：`Channel<T>`

引入类型安全的通道，与 `async`/`await` 无缝集成，消息发送即所有权转移，实现无共享并发。`Channel<T>(capacity?: number)` 返回 `(Sender<T>, Receiver<T>)` 元组——发送端和接收端可各自独立 `move` 进不同线程/任务。

```ts
let (tx, rx) = Channel<{ user: string; score: number }>(64);

// 线程 A：持有 tx（发送端）
spawn(move async () => {
    let data = await fetchData();
    await tx.send(data);  // data 所有权转移进通道
});

// 线程 B：持有 rx（接收端）
spawn(move async () => {
    let result = await rx.receive()?; // 所有权取出，? 处理 ChannelClosed
    process(result);
});
```

编译器保证发送后原变量失效，杜绝竞态。多通道选择可通过 `select` 语法实现（类似 `Promise.race`）。

**`select` 语法：** 同时等待多个异步操作，第一个就绪的分支被执行，其余被取消。

```ts
let (tx1, rx1) = Channel<string>(64);
let (tx2, rx2) = Channel<number>(64);
// 发送方（spawn 到其他任务中，此处简化展示）
spawn(move async () => { await tx1.send("hello"); });
spawn(move async () => { await tx2.send(42); });
select {
    case msg = rx1.receive() => {
        console.log(`rx1: ${msg}`);
    }
    case val = rx2.receive() => {
        console.log(`rx2: ${val}`);
    }
}
```

> **`select` 内不使用 `await`：** 在 `select { case ... }` 分支中，表达式（如 `rx.receive()`）隐式注册为异步等待点，**不需要**显式 `await`。`select` 的非阻塞竞速语义与 `await` 的阻塞语义互斥——编译器在 `select` 上下文中自动对 future 表达式进行轮询。此规则仅在 `select` 分支内生效，普通代码中 `rx.receive()` 仍须 `await`。

> **`select` 分支返回 `Result`：** `select` 分支表达式若返回 `Result<T,E>`，编译器自动在 `Ok(T)` 变体上匹配——仅当结果为 `Ok` 时触发该分支，`T` 直接绑定到变量。`Err` 视为该分支不可用（静默跳过），不触发分支体。若所有分支的 future 同时返回 `Err`（如所有通道关闭），`select` 抛出 `Error("all select branches disabled")`（编译为 `panic!`）。

**Channel API 语义规范：**

`Channel<T>(capacity?: number)` 返回 `(Sender<T>, Receiver<T>)` 元组。`Sender` 实现 `Clone`（可多个发送方），`Receiver` 不实现 `Clone`（唯一接收方）。

| 特性 | 默认行为 | 可选配置 |
|------|---------|---------|
| **容量** | 默认有界（bounded），容量为 64。传入 `0` 或无界标记创建无界通道。 | 有界通道在满时 `send` 会 await 直到有空间；无界通道永不阻塞发送方。 |
| **取消（Cancellation）** | 当接收端被 drop 时，后续 `send` 返回 `Result.Err(ChannelClosed)`。发送端全部 drop 时，`receive` 返回 `Result.Err(ChannelClosed)`。 | — |
| **超时** | `rx.receive()` 无限等待。 | `rx.receiveTimeout(ms)` 返回 `Result<T, ChannelError>`，超时返回 `Err(Timeout)`。 |
| **背压** | 有界通道天然提供背压：当通道满时，`send` 的 await 挂起发送方，直到接收方消费。 | 无界通道无背压，需谨慎使用以避免内存无限增长。 |
| **关闭** | `tx.close()` 关闭通道，之后 `send` 返回 `Err`，`receive` 仍可取出剩余消息。 | — |

```ts
// 示例：有界通道 + 超时
let (tx, rx) = Channel<number>(32);  // 容量 32

spawn(move async () => {
    for (let i = 0; i < 100; i++) {
        match (await tx.send(i)) {
            case Ok => continue;           // Ok(()) 简写（send 返回 Result<(), ...>）
            case Err(ChannelClosed) => break;  // 接收端已关闭
        }
    }
});

// 接收端：带超时
match (await rx.receiveTimeout(5000)) {
    case Ok(val) => process(val);
    case Err(Timeout) => console.log("超时，没有收到消息");
    case Err(ChannelClosed) => console.log("通道已关闭");
}
```

### 5.3 共享可变状态：`shared` 与 `withLock`

当确实需要多线程修改同一数据时，使用 `shared` 关键字。编译器自动将其包裹为 `Arc<Mutex<T>>` 或原子类型。

```ts
shared counter = 0;

// 任何线程中修改
counter.withLock((c) => {
    c += 1;
});
```

- `withLock` 提供闭包内的独占访问，锁自动获取和释放。闭包接收 `&mut T` 参数，通过 `auto-deref` 透明读写内部值——`c += 1` 直接修改数值，`c => c` 返回 `T`（若 `T: Copy`）或 `&mut T`（否则）。
- 对于整数等支持原子操作的类型，编译器优化为原子指令，`withLock` 仍作为安全语法糖（内部变为 `AtomicI32` 操作）。
- **死锁风险声明：** 静态死锁检测在通用语言中是不可判定的（Rice 定理推论）。Trust 编译器**不声称能检测所有嵌套死锁**。编译器可以检测同一线程内对同一个 `Mutex` 的重入（会产生编译警告），但跨多个不同 `shared` 变量的循环等待无法在编译期保证检测。建议遵循以下最佳实践：① 避免嵌套 `withLock`；② 若必须嵌套，始终按固定顺序获取锁；③ 在 debug 构建中启用运行时死锁检测（基于 `parking_lot` 的 deadlock detection）。

### 5.4 `Send` 与 `Sync` 自动推导

类型是否可跨线程发送（`Send`）或共享（`Sync`）由编译器分析内部字段自动确定。如果某个类包含了非线程安全字段（如 `Rc`），编译器会拒绝在 `spawn` 中使用，并提示："类型 `MyClass` 不是 `Send`，因为字段 `cache` 是 `Rc`。考虑使用 `Arc` 或重新设计"。

### 5.5 数据竞争彻底根除

由于以下规则的组合，**数据竞争（data race）**——即两个线程同时访问同一内存且至少有一个是写操作——在编译期被物理消除：

1. **不可变数据**可自由跨线程共享（只读无竞争）。
2. **可变数据**通过 `inout` 保证独占访问。
3. **跨线程共享的可变数据**必须包裹在 `shared` + `withLock` 或通过 `Channel` 转移所有权。
4. **所有检查均发生在编译期**，无运行时开销。

> **重要区分——数据竞争 vs 逻辑竞态：** Trust 消除的是**数据竞争（data race）**——即因非同步内存访问导致的未定义行为、内存损坏。但 Trust **无法消除逻辑竞态（race condition）**——即多个线程的执行顺序导致程序行为不确定。例如，两个线程同时通过 `withLock` 读取-修改-写入同一个 `shared` 计数器，虽然每次操作是原子的（无数据竞争），但最终的计数器值取决于线程调度顺序（存在逻辑竞态）。消除逻辑竞态需要应用层设计（如使用消息传递而非共享状态），Trust 通过 Channel 和 select 语法为这种设计提供了原生支持。

---

## 6. 错误处理

### 6.1 可恢复错误：`Result` 类型与 `?` 操作符

函数返回 `Result<T, E>` 表示可能失败。使用 `?` 操作符传播错误，语法与 TS 可选链不同但同样简洁。

```ts
function readConfig(): Result<Config, IoError> {
    let file = fs.open("config.json")?;  // 若出错立即返回 Err
    let data = file.readAll()?;
    return parse(data);
}
```

### 6.2 不可恢复错误：`panic!` 映射

对于不可恢复错误，Trust 提供 `throw Error` 语法，但编译为 Rust 的 `panic!`。它不应被用于常规控制流，且无法捕获除特定场景外的 panic。鼓励用 `Result` 处理业务异常。

#### 6.2.1 `!` 断言操作符（仅限 `Option`）

在原型开发阶段或逻辑上不可能为 `None` 的场景中，使用 `!` 后缀操作符解包 `Option`：

```ts
let val = maybeValue!;  // 等价于 maybeValue.unwrap()；若为 None 则 panic
let first = list.first()!;  // "我知道这个列表非空"
```

> **设计约束：** `!` **仅允许用于 `Option<T>`，禁止用于 `Result<T,E>`**。这是有意限制——`Option::None` 代表"值缺失"（开发者自己知道是否有值），`Result::Err` 代表"操作失败"（开发者无法在编译时预知）。如果允许 `Result!`，将训练开发者习惯性忽略错误，与 §6.1 的显式 `?` 传播哲学冲突。Trust 的立场是：空白状态可以断言，错误状态必须处理。

`.expect()` 方法可用于 `Result<T,E>` 和 `Option<T>`，语义与 `!` 相同（失败时 panic），但允许提供错误消息：

```ts
let config = readConfig().expect("FATAL: config.json is required");
let user = findUser(42).expect("user 42 must exist");
```

> `.expect()` 与 `!` 的区别：`!` 是轻量语法糖（仅 `Option`），`.expect(msg)` 提供 panic 消息（`Option` 和 `Result` 均可用），适合在"逻辑上不可能失败但需要给未来维护者留下线索"的场景。

#### 6.2.2 不与 `try/catch` 兼容的明确声明

Trust **永久拒绝** `try/catch` 语法。原因有三：

1. **两套错误系统互斥：** Trust 已选择 `Result<T,E>` + `?` 的显式传播模型。引入 `try/catch` 将同时拥有隐式异常和显式错误，造成认知分裂——这正是 Go 社区被 `error` + `panic` 困扰十年的问题，Trust 不重蹈。
2. **所有权黑洞：** `try` 块内如果 move 了某个变量，`catch` 块中该变量是否可用？Rust 社区讨论 `try` 块超过 5 年至今未稳定，核心阻塞点就是所有权语义。Trust 继承 Rust 的所有权模型，无法绕开同一难题。
3. **背离核心承诺：** §2.2 已将"可抛出任意值的异常"列为必须牺牲的特性。`try/catch` 是异常捕获机制——它重新引入被牺牲的特性，会摧毁文档的诚实性和 Trust 的安全承诺。

> 对于习惯了 `try/catch` 的 TS 开发者，Trust 的替代方案是：用 `?` 传播错误，用 `match (result) { case Ok(v) => ...; case Err(e) => ... }` 处理错误分支。两者在表达力上等价，且显式控制流使所有权意图可见。

---

## 7. 模块系统与包管理

### 7.1 模块语法

完全沿用 ES 模块语法：

```ts
import { foo } from "./bar";
export function baz() { }
```

编译后映射为 Rust 的 `mod`、`use` 和文件模块结构。

### 7.2 包管理

使用 Rust 的 Cargo 生态。`Trust.toml` 配置文件声明依赖，可直接引用 [crates.io](https://crates.io) 上的 Rust 包。Trust 标准库作为预置 crate 提供。

#### 7.2.1 类型绑定策略

直接使用 Rust crate 需要对应的 Trust 类型声明。Trust 提供两级绑定方案：

| 级别 | 方式 | 适用场景 |
|------|------|---------|
| **自动绑定（计划中）** | `trust bindgen` 工具，从 Rust crate 的 `rustdoc` JSON 输出自动生成 Trust 类型声明 | 大多数纯数据/函数 crate（如 `serde`、`serde_json`） |
| **手写绑定** | 通过 `extern` 块声明外部 Rust 函数和类型，附带 Trust 类型签名 | 复杂宏 API、异步 trait、或自动绑定无法处理的场景 |

```ts
// 自动绑定（生成自 serde_json 的 rustdoc）
import { fromStr, toPrettyString } from "serde_json";

// 手写绑定
extern "rust" {
    fn sqlx_query<T>(query: string, ...args): Result<Vec<T>, SqlxError>;
}
```

> **`extern` 块使用 `fn` 而非 `function`：** `extern "rust"` 是 FFI 声明块，描述的是 Rust 侧的函数签名。使用 `fn` 关键字提醒读者"此处映射的是 Rust 函数"，与 Trust 自身的 `function` 关键字做视觉区分。`extern` 块内的所有声明不经过 Trust 所有权检查——正确性是开发者的责任。

> **现实预期：** 自动绑定工具是 Trust 生态建设的关键基础设施——没有它，手动为每个 Rust crate 编写类型声明的成本将严重阻碍生态发展。`trust bindgen` 计划在编译器 v0.2 阶段提供原型。在工具成熟之前，标准库将优先覆盖最常用的功能（网络、序列化、加密），以减少对第三方 crate 的依赖。

### 7.3 动态导入限制

`await import("...")` 仅支持静态字符串路径，用于惰性加载模块，但无法在运行时根据变量拼接路径。

---

## 8. 与外部生态交互

- **无法使用 npm 包：** Trust 不兼容 JavaScript 运行时，所有依赖必须来自 Rust 生态或重新实现。
- **调用 Rust 库：** 通过 `extern` 块声明外部 Rust 函数，附带类型签名，编译器生成对应的 `extern crate` 调用。
- **生成 Rust 绑定：** 工具链可将 Trust 编译为 Rust crate，供其他 Rust 项目使用。

---

## 9. 编译策略

### 9.1 工作流

Trust 编译器采用多层中间表示（IR）的架构，而非直接在 AST 层完成所有分析：

1. **解析阶段：** 解析 Trust 源码，生成带类型信息的 AST。
2. **IR 降级阶段：** 将 AST 降级为 Trust 中间表示（TIR —— Trust Intermediate Representation）。TIR 是 Trust 专用的、语义上等价于 Rust MIR 的控制流图表示，消除了语法糖，将控制流简化为基本块。**所有权检查、借用检查、并发安全分析在 TIR 层完成**，错误直接映射回原始 Trust 源码行。
3. **代码生成阶段：** 通过代码生成模块，将经过验证的 TIR 翻译为语义正确的 Rust 源码。
4. **Rust 编译阶段：** 调用 `rustc` 或 Cargo 将生成的 Rust 代码编译为最终二进制。

> **实现策略说明：** TIR 层的 borrow checker 不需要完全复刻 rustc 的 MIR borrow checker——Trust 的类型系统和所有权规则是 Rust 的受限于集，TIR 只需验证 Trust 语义层面的安全性。生成的 Rust 代码在语义上保证通过 rustc 的检查（soundness by construction）。对于 Trust 编译器暂未覆盖的复杂场景（如高阶生命周期多态），将回退为生成带有显式生命周期标注的 Rust 代码并依赖 rustc 进行最终验证，错误信息通过 source map 映射回 Trust 源码。

#### 9.1.1 结构化错误输出

Trust 编译器支持 `--error-format=json` 模式，输出机器可读的 JSON 错误信息，专为 AI 工具和 IDE 集成设计：

```json
{
  "message": "变量 `data` 在第 12 行被移动后在第 15 行被使用",
  "level": "error",
  "code": "E0382",
  "spans": [
    {
      "file": "src/main.trust",
      "line_start": 12, "line_end": 12,
      "label": "data 在此处被移动"
    },
    {
      "file": "src/main.trust",
      "line_start": 15, "line_end": 15,
      "label": "移动后使用"
    }
  ],
  "children": [
    {
      "message": "考虑在此处使用 data.clone()",
      "level": "help"
    }
  ]
}
```

> 结构化错误仅包含 Trust 源码层的变量名和位置，不暴露 TIR 中间名。AI 编码助手（Copilot、Cline 等）可直接消费此 JSON，实现"编译失败 → 自动修复建议 → 用户确认"的闭环。

### 9.2 运行时库（`ferro_rt`）

为支持 `Channel`、`shared`、`spawn` 等特性，提供一个轻量运行时库，包含：

- 多生产者多消费者通道（基于 crossbeam 或 `std::sync::mpsc`）
- `Shared<T>` 类型封装（`Arc<Mutex<T>>` 的 Trust 语法糖）
- 异步运行时绑定

**异步运行时策略：** Trust 的 `spawn async` 和 `Channel` 的异步 API 需要底层异步运行时。Trust 选择 **Tokio** 作为默认异步运行时（因其生态最成熟、社区最大），但通过 trait 抽象允许未来切换到其他运行时（如 async-std、smol）。运行时选择在 `Trust.toml` 中配置：

```toml
[runtime]
async = "tokio"  # 默认；可选 "async-std" | "smol"
```

单线程异步执行器（用于嵌入式或无 OS 环境）作为实验性特性计划在 v0.3+ 支持。

### 9.3 Source Map 与调试支持

生成 Rust 代码的同时输出 source map，将 Trust 源码的每个表达式映射到生成的 Rust 代码行。

**调试方案：**

| 层级 | 方案 | 说明 |
|------|------|------|
| **IDE 内联调试** | Trust Language Server 在 IDE 中直接展示 Trust 源码，将生成的 Rust 代码作为中间产物隐藏。断点、单步执行通过 source map + DWARF 信息在 Trust 源码行展示。 | 首选体验，需要 Trust LSP + IDE 插件支持（VS Code 优先）。 |
| **gdb/lldb 命令行** | 生成 Rust 代码时嵌入 `#line` 指令和 DWARF 调试信息，使调试器直接引用 Trust 源文件路径和行号，无需查看中间 Rust 代码。 | 需要自定义 rustc 的 codegen 参数或事后 DWARF 路径重写。 |
| **回退模式** | 在 Trust LSP 和 DWARF 重写工具成熟之前，开发者可通过编译选项生成"带 Trust 注释的 Rust 代码"，在调试器中查看带有原始 Trust 源码行号注释的 Rust 代码。 | 渐进式交付，v0.1 即可提供。 |

> Source map 在原生二进制调试领域不如 JS/浏览器生态成熟。Trust 采用渐进式策略：v0.1 提供带注释的 Rust 代码（回退模式），v0.2+ 开发 DWARF 路径重写工具和 IDE 插件。

### 9.4 `trust eval`：受控表达式求值

Trust 不提供完整的 REPL（交互式解释器），因为 REPL 的"会话状态持续性"与 move 语义的"一次性消耗"在物理上矛盾。替代方案是 `trust eval` 子命令：

```bash
# 求值单个表达式
$ trust eval "2 + 3"
5

# 求值语句块
$ trust eval "let x = 5; let y = x * 2; y"
10

# 从标准输入读取（notebook 风格）
$ echo 'let arr = [1,2,3]; arr.map(x => x * 2)' | trust eval -
[2, 4, 6]
```

**工作原理：** `trust eval` 将输入包装为 `fn main() { ... }`，走完整的 Trust → Rust → 二进制编译流程，执行后输出结果。每次求值是**独立的编译单元**，无状态共享，因此不存在 REPL 的所有权矛盾。

> **AI 编程代理的交互模式：** AI 工具使用 `trust eval` 快速验证生成的代码片段——生成代码 → 求值 → 读取结构化错误 JSON → 修复。这形成了比 REPL 更适合所有权模型的迭代闭环。Notebook 风格的内核可以在 Jupyter 中提供多 cell 体验，每个 cell 之间通过显式导出/导入传递状态。

### 9.5 `--fix` 编译器辅助修复

Trust 编译器提供 `trust check --fix` 模式，针对简单且确定的错误提供修复建议：

```bash
$ trust check --fix src/main.trust
# 交互式确认每个修复
src/main.trust:15 — 变量 `data` 在第 12 行被移动
  建议：在此处添加 .clone()
  应用此修复？(y/N)
```

> **设计原则：** `--fix` **默认关闭**，每次修复需开发者手动确认。这与"编译器永远不隐藏所有权决策"的哲学一致——编译器告诉你**为什么**代码不安全、**建议如何修**，但最终决定权在你。这确保了 Trust 作为 Rust onboarding 路径的角色：你会在这个过程中理解所有权，而不是被编译器偷偷保护。

---

## 10. 标准库初稿

| 模块 | 内容 | 优先级 |
|------|------|--------|
| `std::collections` | `Vec`、`HashMap`、`HashSet`、`VecDeque` 等，API 风格贴近 TS 数组/Map | v0.1 |
| `std::sync` | `Channel`、`shared`、`spawn`、`Mutex`、`RwLock`、`Atomic` 原子类型，对用户暴露简化接口 | v0.1 |
| `std::async` | `join`、`sleep`、异步 I/O 原语、Tokio 绑定 | v0.1 |
| `std::result` | `Result` 与 `Option` 类型 | v0.1 |
| `std::string` | 字符串类型（UTF-8），提供类似 JS 的常用方法（`split`、`slice`、`replace`、`trim`、`toUpperCase` 等） | v0.1 |
| `std::fs` | 文件系统操作（读/写/目录遍历/元数据），返回 `Result` | v0.1 |
| `std::net` | TCP/UDP 套接字、HTTP 客户端、TLS 支持（计划中，初期可依赖 Rust 生态 binding） | v0.2 |
| `std::serde` | 序列化/反序列化（JSON、MessagePack 等），基于 Rust 的 serde 生态封装 | v0.2 |
| `std::crypto` | 哈希（SHA-256、BLAKE3）、对称/非对称加密原语（计划中） | v0.3 |
| `std::time` | 时间戳、Duration、定时器 | v0.1 |
| `std::process` | 子进程管理、环境变量 | v0.2 |

> 标准库的优先级根据常见应用需求排序。v0.1 覆盖了 Trust 语言核心特性运行所需的最小集合。v0.2+ 的模块逐步填补网络、序列化等生产级应用的基础需求。在标准库覆盖不足的过渡期，开发者可通过 `extern` 绑定直接使用 Rust 生态的相应 crate。

---

## 11. 语法参考与范例

本章以范例形式覆盖 Trust 的完整语法，从基础到高级，每个示例突出 Trust 与 TypeScript/Rust 的差异点。

### 11.1 变量与常量

```ts
// let —— 默认不可变绑定
let x = 42;
// x = 43;              // ❌ 编译错误：x 不可变

// let mut —— 可变绑定
let mut y = 10;
y += 1;                 // ✅ 合法

// const —— 编译时常量（等价于 Rust const）
const MAX = 100;
const PI: number = 3.14159;

// 类型标注
let name: string = "Alice";
let scores: number[] = [95, 87, 92];
let maybe: Option<number> = None;
```

### 11.2 基本类型与字面量

```ts
// 标量类型
let a: number = 42;           // i32 默认
let b: number = 3.14;         // f64
let c: bigint = 9007199254740991n;  // i64
let d: string = "hello";
let e: string = `template ${a}`;
let f: boolean = true;
let g: void = undefined;

// 数组
let arr: number[] = [1, 2, 3];
let matrix: number[][] = [[1, 0], [0, 1]];

// 元组
let pair: [string, number] = ["age", 30];
```

### 11.3 控制流

```ts
// if / else if / else
if (x > 0) {
    console.log("positive");
} else if (x < 0) {
    console.log("negative");
} else {
    console.log("zero");
}

// if 是表达式（返回值）
let label = if (score >= 60) { "pass" } else { "fail" };

// for 循环
for (let i = 0; i < 10; i++) {
    console.log(i);
}

// for-of 遍历（等价于 Rust for-in）
for (let item of items) {
    process(item);
}

// while 循环
let mut n = 5;
while (n > 0) {
    n -= 1;
}

// loop 无限循环（等价于 Rust loop）
let mut count = 0;
let result = loop {
    count += 1;
    if (count >= 3) {
        break count * 2;    // break 带返回值
    }
};
// result == 6
```

> **`break` 带返回值：** Trust 的 `loop { break expr; }` 允许 break 携带一个表达式，该表达式成为整个 `loop` 的返回值。这与 Rust 的 `loop { break value; }` 语义一致。注意仅在 `loop`（无限循环）中 `break` 可带值，`for` 和 `while` 中的 `break` 不返回值。

### 11.4 函数

```ts
// 标准函数声明
function add(a: number, b: number): number {
    return a + b;
}

// 单表达式简写（等号语法，返回值自动推导）
function square(x: number) = x * x;

// 箭头函数
let double = (x: number): number => x * 2;
let greet = (name: string) => `Hello, ${name}`;

// 泛型函数
function identity<T>(value: T): T {
    return value;
}
let val = identity(42);          // T 推断为 number
let str = identity("hello");     // T 推断为 string

// inout 参数 —— 可变借用
function pushOne(inout arr: number[]) {
    arr.push(1);
}
let mut data = [1, 2, 3];
pushOne(inout data);             // data 被修改

// 默认只读借用（参数不消耗所有权）
function printLen(arr: number[]) {
    console.log(arr.length);     // 只读访问
}
printLen(data);                  // data 所有权未转移
```

### 11.5 结构体与接口

```ts
// interface —— 名义类型
interface Point {
    x: number;
    y: number;
}

function distance(p: Point): number {
    return Math.sqrt(p.x * p.x + p.y * p.y);
}

let pt: Point = { x: 3, y: 4 };

// type —— 结构别名
type Point3D = { x: number, y: number, z: number };

// {x, y} 属性简写（仅限类型上下文明确时）
let x = 10;
let y = 20;
let pt2: Point = { x, y };       // 等价 { x: 10, y: 20 }

// 实现 trait
interface Printable {
    print(): void;
}

impl Printable for Point {
    function print() {
        console.log(`Point(${this.x}, ${this.y})`);
    }
}
```

### 11.6 代数数据类型（ADT）

> `switch` 是语句（无返回值），`match` 是表达式（返回值）。语法区分：`switch` 使用 `case X: statement; break;`，`match` 使用 `case X => expr,`。

```ts
// 可辨识联合 —— 编译为 Rust enum
type Msg =
    | { kind: "quit" }
    | { kind: "data"; payload: number[] }
    | { kind: "error"; message: string };

// switch 穷举匹配
function handle(msg: Msg) {
    switch (msg.kind) {
        case "quit":
            return;
        case "data":
            process(msg.payload);
            break;
        case "error":
            log(msg.message);
            break;
    } // 遗漏任何分支 → 编译错误
}

// match 表达式 —— 与传统模式匹配
let label = match (msg.kind) {
    case "quit" => "bye",
    case "data" => `got ${msg.payload.length} items`,
    case "error" => msg.message,
};

// if let —— 单变体匹配，免除穷举
if let { kind: "data", payload } = msg {
    process(payload);
}
```

### 11.7 所有权：移动、克隆、借用

```ts
// === 移动语义 ===

// let = 移动，原变量失效
let a = [1, 2, 3];
let b = a;
// console.log(a.length);  // ❌ a 已被移动

// move 参数 —— 函数消耗所有权
function consume(move arr: number[]) {
    console.log(arr);          // arr 所有权归此函数
}
let c = [4, 5, 6];
consume(move c);
// consume(move c);           // ❌ c 已被移动

// === 显式克隆 ===
let d = [1, 2, 3];
let e = d.clone();           // 深拷贝，d 仍可用
console.log(d.length);       // ✅ 合法
console.log(e.length);       // ✅ 合法

// Copy 类型自动复制（number, boolean 等简单值）
let f = 42;
let g = f;
console.log(f);              // ✅ number 是 Copy 类型

// === 借用 ===

// 函数参数默认只读借用（无需 inout 或 move 关键字）
function inspect(arr: number[]) {
    console.log(arr.length); // 只读
}
let h = [7, 8, 9];
inspect(h);                  // h 借用，不消耗所有权
console.log(h.length);       // ✅ 仍可用
inspect(h);                  // ✅ 可再次借用

// inout —— 可变借用
function doubleAll(inout arr: number[]) {
    for (let i = 0; i < arr.length; i++) {
        arr[i] *= 2;
    }
}
let mut scores = [1, 2, 3];
doubleAll(inout scores);     // scores 被修改
// scores == [2, 4, 6]

// 借用规则：同一时刻只能有一个可变借用或多个只读借用
let mut data = [1, 2, 3];
let r1 = &data;              // 显式只读引用
let r2 = &data;              // ✅ 多个只读引用 OK
// doubleAll(inout data);    // ❌ 同时存在只读引用和可变引用
```

### 11.8 `Option` 与 `Result`

```ts
// Option —— 可能缺失的值
function findUser(id: number): Option<User> {
    if (id > 0) {
        return Some(User { id, name: "Alice" });
    }
    return None;
}

// match 处理 Option
match (findUser(42)) {
    case Some(user) => console.log(user.name);
    case None => console.log("not found");
}

// ?? —— 空值合并（映射 unwrap_or）
let name = findUser(42).map(u => u.name) ?? "guest";

// ?. —— 可选链（Option / &Option 均支持，owned 时 move 原值）
let street = user?.address?.street;

// ! —— 断言解包（仅限 Option，原型阶段使用）
let val = maybeValue!;       // 若 None 则 panic

// if let —— 单分支 match 的语法糖
if let Some(user) = findUser(42) {
    console.log(user.name);    // 只处理有值的情况
}

// if let 处理 Result 的错误分支
if let Err(e) = readConfig() {
    console.log(`skip config: ${e}`);
}

// if let ... else —— 双分支路径
if let Some(user) = findUser(42) {
    console.log(`hello, ${user.name}`);
} else {
    console.log("anonymous");
}

// === Result —— 可恢复错误 ===
function readConfig(): Result<Config, IoError> {
    let file = fs.open("config.json")?;   // ? 传播错误
    let data = file.readAll()?;
    return Ok(parse(data));
}

// 处理 Result
match (readConfig()) {
    case Ok(config) => apply(config);
    case Err(e) => console.log(`error: ${e}`);
}
```

### 11.9 字符串与模板

```ts
let name = "Alice";
let age = 30;

// 模板字符串
let msg = `Hello, ${name}. You are ${age} years old.`;

// 字符串方法（UTF-8，接近 JS API）
let parts = "a,b,c".split(",");         // ["a", "b", "c"]
let upper = "hello".toUpperCase();       // "HELLO"
let trimmed = "  hi  ".trim();           // "hi"
let replaced = "hello".replace("l", "L"); // "heLlo"
let sliced = "hello".slice(1, 4);        // "ell"
```

### 11.10 闭包与高阶函数

```ts
// 箭头函数闭包
let factor = 2;
let multiply = (x: number) => x * factor;

// 高阶函数：map / filter / reduce
let nums = [1, 2, 3, 4, 5];
let doubled = nums.map(x => x * 2);         // [2, 4, 6, 8, 10]
let evens = nums.filter(x => x % 2 == 0);    // [2, 4]
let sum = nums.reduce((a, b) => a + b, 0);   // 15

// 闭包默认只读借用 —— data 不消耗，闭包可多次调用
let data = [1, 2, 3];
let read = () => {
    console.log(data.length);  // 只读借用 data
};
read();
read();                      // ✅ 借用闭包可多次调用
console.log(data);            // ✅ data 仍可用

// move 闭包 —— 所有权转移进闭包，变为 FnOnce
let data2 = [4, 5, 6];
let consume = move () => {
    process(data2);            // data2 被 move 进闭包
};
consume();
// consume();                 // ❌ move 闭包只能调用一次
// console.log(data2);        // ❌ data2 已被 move
```

### 11.11 模块系统

```ts
// === 导出 ===
// 文件: math.trust
export function add(a: number, b: number): number {
    return a + b;
}

export const PI = 3.14159;

export interface Calculator {
    compute(): number;
}

// 默认导出
export default function greet(name: string) {
    return `Hello, ${name}`;
}

// === 导入 ===
// 文件: main.trust
import { add, PI } from "./math";
import greet from "./math";              // 默认导入
import * as math from "./math";          // 命名空间导入
```

### 11.12 异步编程

```ts
// Trust 的 async function 返回惰性 Future（调用时不执行，仅 await/spawn 时 poll）
async function fetchData(): Result<string, NetError> {
    let response = await http.get("/api/data")?;
    return Ok(response.body);
}

// 错误：先创建后 await 是串行执行（惰性 Future 特性）
let f1 = fetchUser(42);     // 未执行
let f2 = fetchConfig();     // 未执行
let user = await f1;        // poll f1，期间 f2 从未被 poll
let config = await f2;      // poll f2 — 两个操作串行

// 正确：用 join() 实现真正并发
async function loadAll(): Result<[User, Config], Error> {
    let (user, config) = await join(fetchUser(42), fetchConfig())?;
    return Ok([user, config]);
}

// spawn async —— 异步任务
async function main() {
    let handle = spawn(move async () => {
        let data = await fetchData();
        process(data);
    });
    await handle.join();
}
```

### 11.13 并发：spawn、Channel、shared

```ts
import { spawn, Channel, shared } from "std::sync";

// === spawn（OS 线程） ===
spawn(move () => {
    heavyComputation();    // CPU 密集型，move 确保 'static
});

// === Channel —— 消息传递 ===
let (tx, rx) = Channel<number>(64);    // 返回 (Sender, Receiver)

// 发送方：持有 tx（Sender 可 Clone 给多个发送方）
spawn(move async () => {
    for (let i = 0; i < 10; i++) {
        await tx.send(i);
    }
    tx.close();
});

// 接收方：持有 rx（Receiver 唯一，不可 Clone）
spawn(move async () => {
    loop {
        match (await rx.receive()) {
            case Ok(val) => console.log(`received: ${val}`);
            case Err(ChannelClosed) => break;
        }
    }
});

// 带超时的接收
match (await rx.receiveTimeout(5000)) {
    case Ok(val) => process(val);
    case Err(Timeout) => console.log("timeout");
    case Err(ChannelClosed) => console.log("closed");
}

// select —— 多通道就绪选择（分支内不写 await）
let (tx1, rx1) = Channel<string>(64);
let (tx2, rx2) = Channel<number>(64);
// 发送方可 spawn 到其他任务（§11.13 上文示例），此处省略
select {
    case msg = rx1.receive() => {
        console.log(`rx1: ${msg}`);
    }
    case val = rx2.receive() => {
        console.log(`rx2: ${val}`);
    }
}

// === shared —— 共享可变状态 ===
shared counter = 0;

spawn(move () => {
    counter.withLock(c => {
        c += 1;
    });
});

// 读取 shared 变量
let current = counter.withLock(c => c);

// 原子整数优化：shared number 自动变为 AtomicI32
shared total = 0;
spawn(move () => {
    total.withLock(t => { t += 10; });  // fetch_add，无锁
});
```

### 11.14 引用计数：Rc 与 Arc

```ts
import { Rc, Arc, Weak } from "std::rc";

// Rc<T> —— 单线程共享所有权
let a = Rc::new([1, 2, 3]);
let b = a.clone();           // 引用计数 +1（不复制数据）
let c = a.clone();           // 引用计数 +1

// Arc<T> —— 多线程共享所有权
let shared_data = Arc::new(MyStruct { value: 42 });

spawn(move () => {
    let local = shared_data.clone();   // 原子引用计数 +1
    console.log(local.value);          // Send + Sync 自动推导
});

// Weak<T> —— 弱引用（打破循环引用）
let strong = Rc::new([1, 2, 3]);
let weak = Rc::downgrade(strong);

match (weak.upgrade()) {
    case Some(rc) => console.log(rc[0]);  // 数据仍存活
    case None => console.log("freed");    // 已被释放
}
```

### 11.15 动态类型

```ts
// Dynamic 枚举 —— 封闭类型集合
let val: Dynamic = 42;
match (val) {
    case Dynamic.Number(n) => console.log(n * 2);
    case Dynamic.String(s) => console.log(s.toUpperCase());
    case Dynamic.Boolean(b) => console.log(b ? "yes" : "no");
    case Dynamic.Array(arr) => console.log(arr.length);
    case Dynamic.Null => console.log("null");
    default => throw Error("unexpected");
}

// Box<dyn Trait> —— 开放类型集合，vtable 分发
interface Serializable {
    serialize(): string;
}

impl Serializable for Point {
    function serialize() {
        return `{x: ${this.x}, y: ${this.y}}`;
    }
}

function log(value: Box<dyn Serializable>) {
    console.log(value.serialize());  // vtable 分发，O(1)
}

let pt = Box::new(Point { x: 1, y: 2 });
log(pt);  // 自动转型为 Box<dyn Serializable>
```

### 11.16 错误处理完整模式

```ts
// === 可恢复错误：Result + ? ===
function loadAndParse(path: string): Result<Data, AppError> {
    let content = fs.readToString(path)?;     // IoError → AppError
    let data = parse(content)?;               // ParseError → AppError
    return Ok(data);
}

// === 不可恢复错误：throw Error → panic! ===
function mustSucceed(path: string): Data {
    let content = fs.readToString(path).expect("config file required");
    return parse(content).expect("invalid config format");
}

// 等价于：
function mustSucceed2(path: string): Data {
    let content = match (fs.readToString(path)) {
        case Ok(s) => s,
        case Err(e) => throw Error(`config file required: ${e}`),
    };
    let data = match (parse(content)) {
        case Ok(d) => d,
        case Err(e) => throw Error(`invalid config: ${e}`),
    };
    return data;
}
```

### 11.17 泛型完整示例

```ts
// 泛型约束 —— 结构化 extends（编译为隐式 trait）
function firstElement<T extends { length: number }>(arr: T): number {
    return arr.length;
}
firstElement([1, 2, 3]);     // ✅ Vec 有 len()
firstElement("hello");        // ✅ String 有 len()

// 泛型约束 —— 名义类型 trait
interface Comparable {
    compare(other: this): number;
}

function max<T extends Comparable>(a: T, b: T): T {
    if (a.compare(b) > 0) { return a; }
    return b;
}

// 多 trait 约束
function process<T extends Clone + Serializable>(item: T): T {
    let copy = item.clone();
    console.log(copy.serialize());
    return item;
}
```

### 11.18 测试

```ts
// === 单元测试 ===
test function add_works() {
    assert(1 + 1 == 2);
    assert(add(2, 3) == 5);
}

// 异步测试
test async function fetch_timeout() {
    let result = await api.fetch("/health").timeout(5000);
    assert(result.isOk());
}

// 预期 panic 的测试
#[should_panic]
test function invalid_unwrap() {
    let val: Option<number> = None;
    let x = val!;      // 触发 panic
}

// === 文档测试 ===
/// 计算平方值
///
/// ```trust
/// assert(square(3) == 9);
/// assert(square(-4) == 16);
/// ```
function square(x: number): number {
    return x * x;
}
```

### 11.19 外部绑定与生命周期

```ts
// extern 块 —— 调用 Rust 库
extern "rust" {
    fn sqlx_query<T>(query: string, ...args): Result<Vec<T>, SqlxError>;
    fn sha256(data: number[]): [number; 32];
}

// 生命周期标注（仅在返回引用或复杂场景需要）
function getFirst<'a>(arr: &'a number[]): &'a number {
    return &arr[0];          // 返回值生命周期与 arr 绑定
}

// 大多数场景生命周期自动推导（无需标注）
function getLen(arr: number[]): number {
    return arr.length;        // 返回非引用 → 自动推导
}
```

### 11.20 完整程序示例：HTTP 服务

```ts
import { spawn, Channel, shared } from "std::sync";
import { HttpServer, Request, Response } from "std::net";
import { readToString } from "std::fs";

// 配置
interface Config {
    port: number;
    static_dir: string;
}

// 请求计数器（原子操作，无锁）
shared request_count = 0;

// 路由处理
async function handleRequest(req: Request): Result<Response, HttpError> {
    // 原子递增计数器
    request_count.withLock(c => { c += 1; });

    // 路由匹配
    let path = req.url.path;
    if (path == "/health") {
        let count = request_count.withLock(c => c);
        let resp = Response::json({ status: "ok", requests: count })?;
        return Ok(resp);
    }

    if (path.startsWith("/static/")) {
        let file_path = `./public/${path.slice(8)}`;
        let content = readToString(file_path)?;
        return Ok(Response::html(content));
    }

    return Ok(Response::text("Not Found", 404));
}

// 入口
async function main(): Result<void, Error> {
    let config = loadConfig() ?? Config { port: 3000, static_dir: "./public" };
    let server = HttpServer::bind(`127.0.0.1:${config.port}`)?;

    console.log(`listening on port ${config.port}`);

    // 优雅关闭通道
    let (tx_shutdown, rx_shutdown) = Channel<void>();

    // 每个连接 spawn 一个异步任务
    loop {
        select {
            case conn = server.accept() => {
                spawn(move async () => {
                    match (await handleRequest(conn.req)) {
                        case Ok(res) => await conn.respond(res);
                        case Err(e) => await conn.respond(Response::text(e.toString(), 500));
                    }
                });
            }
            case _ = rx_shutdown.receive() => {
                break;
            }
        }
    }

    let total = request_count.withLock(c => c);
    console.log(`shutdown. handled ${total} requests.`);
    return Ok(());
}
```

## 12. 未来展望与限制

### 12.1 未来可能扩展

- **SIMD 与向量化支持：** 通过标准库提供平台无关的并行运算。
- **Wasm 编译目标：** 生成 Rust 后再编译为 WebAssembly，保留所有安全特性。
- **与 JS 互操作层：** 通过 `wasm-bindgen` 类似的机制，允许在浏览器中调用 Trust 编译的模块。
- **更高级的生命周期省略：** 进一步减少手动标注。
- **受限声明式宏（计划 v0.3+）：** 类似 Rust 的 `macro_rules!`，但限制为 hygienic（卫生宏）、仅限局部语法转换、不允许引入非结构化的 token 流。宏展开后在 TIR 层重新做完整分析，错误信息必须指向宏调用点而非展开后代码。过程宏（proc-macro）因破坏静态可分析性被永久拒绝（见 §15.4）。

### 12.2 已知限制

- **学习曲线：** 开发者仍须理解所有权、移动语义。Trust 的语法亲和减少了符号层面的认知负荷，但**无法隐藏所有权心智模型**——例如：① 闭包捕获变量后为什么原变量失效（move closure）；② 为什么 `Rc` 不能跨线程使用（`Send` 约束缺失）；③ async 函数中跨越 `.await` 的引用为什么有时需要 `'static` 生命周期；④ `match` 语句中为什么部分移动后原值不可用。这些是语义层面的概念，而非语法问题。Trust 通过友好的错误信息降低了修复门槛，但开发者仍需建立所有权直觉。
- **生态重建：** 无法使用巨大的 npm 生态，需要逐步积累原生库。`trust bindgen` 工具是缓解这一问题的关键基础设施，但其覆盖率和生成质量需要时间成熟。
- **编译时间：** 双重编译（Trust → Rust → 机器码）可能导致较长的构建时间。Trust → Rust 的代码生成预计较快（毫秒级），瓶颈在 rustc。增量编译（Trust 编译器缓存 + Cargo 增量编译）可缓解，但冷启动编译时间仍以分钟计。对于习惯了 Web 开发中毫秒级 HMR 的开发者，需要适应这种节奏。
- **无法 100% 模拟 JS 动态行为：** 部分设计模式需彻底改变。典型例子包括：Redux/Vuex 风格的 mutable state、依赖动态属性的对象扩展、基于原型链的方法覆盖、Proxy 拦截模式。

---

## 13. AI 友好性

Trust 从语言设计的第一天起就将 AI 编码工具（LLM 代码生成、Copilot 风格补全、自动重构）作为一等公民考量。

### 13.1 语法设计对 LLM 友好

Trust 的语法是 TypeScript 的子集+扩展——这是对 LLM 最友好的设计决策之一。LLM 在 TS/JS 代码上的训练数据远多于 Rust，因此生成 Trust 代码的"语法正确率"天然更高：

- `function` 替代 `fn`、`switch/case` 替代 Rust 的 `match`、箭头函数替代 `|...|` 闭包——这些符号与 LLM 的训练分布高度吻合
- `inout`、`shared`、`withLock` 等 Trust 特有关键字是**无歧义的标记**——LLM 可以明确学习"这里是所有权边界"而非猜测 `&mut` 或 `Arc<Mutex<>>` 的意图
- Trust 的 `Result<T,E>` + `?` 模式比 Rust 的 `match` + `map_err` 链更容易被 LLM 正确生成（减少 boilerplate 即减少出错空间）

> **已知风险：** LLM 会"自信地"生成 TS 风格的错误代码——如 `let b = a; a.push(1)`。Trust 的应对不是改变语法，而是通过结构化错误输出（§9.1.1）让 AI 工具快速发现并修复这类语义错误。

### 13.2 结构化错误输出

详见 §9.1.1。Trust 的 `--error-format=json` 为 AI 工具提供了精确的修复锚点（文件、行号、错误代码、建议修复），使 AI 编码助手可以实现"编译失败 → 解析 JSON 错误 → 生成修复 → 重新编译"的自动闭环。这是 Trust 相对于 Rust（错误信息偏向人类阅读）的 AI 友好优势。

### 13.3 确定性编译模型：编译通过 = 无内存错误

Trust 的核心安全承诺（§1.1、§5.5）对 AI 生成代码具有独特价值：

- **审查降维：** AI 生成的 C/TS 代码需要人类审查内存安全（use-after-free、double-free、data race、buffer overflow）。AI 生成的 Trust 代码只需审查**业务逻辑正确性**。编译通过即证明无内存安全 bug。
- **Fuzzing 安全：** AI 或 fuzzer 生成的随机输入可以安全地注入 Trust 程序——即使触发极端路径，编译器保证不会出现段错误或数据竞争（详见 §15.2）。
- **逻辑竞态是人类审查的最后边界：** 编译通过不等于无 bug——逻辑竞态（race condition）仍需审查。Trust 通过并发压力测试框架（§15.3）辅助暴露此类问题。

### 13.4 AI 专用所有权分析 API（计划 v0.2+）

Trust 编译器的 TIR 层（§9.1）在分析阶段拥有完整的变量所有权图和生命周期约束。计划在 v0.2+ 提供 `--analyze-ownership` 模式，以结构化格式输出指定位置的完整借用状态：

```json
{
  "variable": "data",
  "location": {"file": "src/main.trust", "line": 42},
  "borrow_state": "mutable_borrowed",
  "borrowed_by": "pushOne",
  "valid_until_line": 45,
  "available_after_line": 46
}
```

AI 工具可以基于此信息进行精确修复——例如"在 `.await` 前 clone 变量以避免跨 await 引用问题"。

> **隐私与稳定性说明：** 所有权分析 API 的 JSON schema 在 TIR 稳定化之前（预计 v0.3）为实验性质。所有分析在本地编译器执行，不涉及云端传输。

### 13.5 不与核心哲学冲突的边界

以下特性被评估后明确拒绝，因为它们会引入不可验证的谎言或破坏 Trust 的"编译器保证安全"承诺：

| 拒绝的特性 | 理由 |
|-----------|------|
| `// @trust: pure` 等意图注释（纯注释无验证） | 如果注释不被编译器强制检查，开发者可以标注 `pure` 却在函数内执行 I/O——这是"谎言注释"，给 AI 和人类传递虚假的安全感 |
| 完整 REPL | REPL 的会话状态持续性与 move 语义一次性消耗在物理上矛盾。替代方案：`trust eval`（§9.4） |

> **如果引入意图标注**，必须以编译器强制验证为前提——例如 `pure` 等价于 `const fn` 约束（禁止 `shared` 写入、禁止 `fs`/`net` 调用、禁止 `throw`）。未经检查的注释在 Trust 中不存在。

---

## 14. 测试体系

Trust 内置完整的测试支持，将测试视为一等语言特性。

### 14.1 内置测试框架

Trust 提供 `test` 关键字声明测试函数，编译器自动识别并生成对应的 Rust `#[test]` 函数。`test function` 和 `#[test]` 两种语法等价，`#[test]` 属性语法兼容 Rust 宏风格，在需要附加多个属性时更简洁（如 `#[test] #[should_panic]`）。

```ts
// test function 关键字语法
test function add_works() {
    assert(1 + 1 == 2);
}

// #[test] 属性语法（等价写法）
#[test]
function subtract_works() {
    assert(5 - 3 == 2);
}

// 异步测试
test async function fetch_timeout() {
    let result = await api.fetch("/health").timeout(5000);
    assert(result.isOk());
}

// 预期 panic 的测试
#[should_panic]
test function invalid_assertion() {
    throw Error("expected failure");
}
```

运行测试：
```bash
trust test                    # 运行所有测试
trust test --filter "fetch"   # 按名称过滤
trust test --threads 4        # 并行线程数
```

> 底层复用 Cargo test 基础设施，因此与 Rust 生态的 CI 工具（如 `cargo-tarpaulin`、`cargo-nextest`）天然兼容。

### 14.2 文档测试（Doctest）

在文档注释中嵌入可执行的 Trust 代码块，编译时自动提取并作为测试运行：

```ts
/// 计算两个数的和
///
/// ```trust
/// let result = add(2, 3);
/// assert(result == 5);
/// ```
function add(a: number, b: number): number {
    return a + b;
}
```

> 文档测试保证示例代码**永远可编译、永远正确**。这是 Rust 生态中最强大的文档质量倍增器，Trust 直接继承此设计。

### 14.3 属性测试与模糊测试（计划 v0.2+）

Trust 的"无数据竞争 + 无段错误"特性使 fuzzing 极其安全——即使在模糊测试中触发极端输入，编译器保证不会出现内存损坏。标准库计划提供 `#[property]` 标注支持：

```ts
#[property]
test function sort_is_idempotent(arr: number[]) {
    let sorted1 = arr.sorted();
    let sorted2 = sorted1.sorted();
    assert(sorted1 == sorted2);  // 排序两次结果不变
}
```

编译器在 `#[property]` 模式下自动生成随机输入并验证属性。

### 14.4 并发压力测试（计划 v0.3+）

针对 Trust 无法消除的逻辑竞态（§5.5），提供随机化调度测试框架：

```ts
#[concurrent]
test function counter_concurrency() {
    shared counter = 0;
    let (tx_done, rx_done) = Channel<void>();

    for (let i = 0; i < 10; i++) {
        let tx = tx_done.clone();        // Sender 可 Clone
        spawn(move async () => {
            counter.withLock(c => { c += 1; });
            await tx.send();
        });
    }

    for (let i = 0; i < 10; i++) {
        await rx_done.receive();
    }

    let final = counter.withLock(c => c);
    assert(final == 10);  // 逻辑正确性断言
}
```

`#[concurrent]` 标注触发测试运行器的随机化调度模式——通过随机化线程/异步任务执行顺序，在统计上提高暴露逻辑竞态的概率。底层可集成 Rust 的 `loom` 或自定义调度器。

### 14.5 Mock 与依赖注入

Trust 通过已有的 `Box<dyn Trait>`（§3.4.2）支持基于 trait 对象的依赖注入：

```ts
interface Database {
    async query(sql: string): Result<Vec<Row>, DbError>;
}

// 生产代码使用泛型 + 静态分发
async function getUsers<T: Database>(db: T): Result<Vec<User>, DbError> {
    return db.query("SELECT * FROM users").await;
}

// 测试代码使用 Box<dyn Database> + 动态分发
#[test]
async function test_getUsers() {
    let mockDb: Box<dyn Database> = MockDatabase::new();
    let users = await getUsers(mockDb);
    assert(users.unwrap().length == 3);
}
```

> **孤儿规则限制：** Trust 继承 Rust 的孤儿规则（orphan rule）——只有当 trait 或类型在你的 crate 中定义时，才能为类型实现该 trait。这意味着跨 crate 的 mock 需要类型自身在测试 crate 中定义，或在 trait 定义 crate 中提供测试工具。完全的语言级 mock 支持（如 `#[test_impl]` 打破孤儿规则的局部例外）在 v0.2+ 考虑。

### 14.6 快照测试（计划 v0.2+）

工具链提供 `trust snapshot` 子命令，用于管理快照文件：

```ts
test function cli_output() {
    let result = runCommand("list --format json");
    snapshot(result);  // 首次运行保存为 .snap，后续运行比对
}
```

### 14.7 借用模式覆盖率（计划 v0.3+）

Trust 在传统行/分支覆盖率之上，创新性地提供**借用模式覆盖率**——编译器静态分析函数的所有可能借用模式（只读、可变、move、shared），报告哪些模式在测试中未被触发：

```bash
$ trust test --coverage borrow
src/main.trust:45 process_data(Data) 
  已覆盖: &Data（只读借用）
  未覆盖: inout Data（可变借用）← 缺少对修改路径的测试
```

> 这利用了 Trust 编译器 TIR 层已有的所有权信息。对于系统编程中的并发安全测试，确保所有借用模式被覆盖对发现潜在 bug 至关重要。

---

## 15. 被明确拒绝的特性及理由

以下特性经过了严格的设计评审后被**永久拒绝**。将它们记录在此是为了防止未来的设计讨论反复回到已被论证不可行的方案上。

### 15.1 `try/catch` 异常捕获

**拒绝理由：**
1. Trust 已选择 `Result<T,E>` + `?` 的显式错误传播模型（§6.1）。`try/catch` 引入第二套错误系统，导致认知分裂。
2. `try` 块内的所有权语义不可救——move 后的变量在 `catch` 块中不可用，但控制流隐式跳转使得这一点在代码中不可见。
3. 与 §2.2 的牺牲列表冲突——该列表已将"可抛出任意值的异常"列为必须移除的特性。`try/catch` 重新引入被牺牲的特性，破坏文档的诚实性和语言的一致性。

### 15.2 `defer` 延迟执行

**拒绝理由：**
1. 与所有权线性语义冲突——如果 `defer` 闭包捕获 `f`（move），`f` 后续不可用；如果捕获 `&mut f`，`f` 在函数剩余部分被冻结。两者都严重限制实用性。
2. `withLock` 块级作用域（§5.3）和 RAII 自动 `drop` 已覆盖 90% 的资源清理场景。
3. Trust 不需要 Go 的 `defer`——Rust 的所有权系统天然管理资源生命周期。

### 15.3 `|>` 管道操作符

**拒绝理由：**
Trust 的语法亲和目标是 TypeScript（§1.2）。TS 没有管道操作符，引入 `|>` 使语言从"TS 风格"滑向"Elm/Elixir 风格"，破坏心智一致性。Trust 的方法链（`.filter().map().reduce()`）已覆盖相同场景。

### 15.4 过程宏（Procedural Macros）

**拒绝理由：**
过程宏允许任意代码在编译期操作 AST TokenStream，意味着"看到的代码不是被编译的代码"。这与 §2.2 的"完全静态可分析的内存图"物理上不可调和。替代方案：`trust generate` 子命令在编译前展开代码生成模板，生成的代码进入版本控制（类似 Go 的 `go generate`）。

### 15.5 不可验证的 `@trust` 意图注释

**拒绝理由：**
如果 `// @trust: pure` 不被编译器强制验证，开发者可以标注 `pure` 却在函数内执行 I/O——这是"谎言注释"。Trust 的 brand 是"编译器保证安全"，所有宣称的不变量必须被编译器强制检查。如果未来引入注释标注，必须以编译器验证为前提（如 `pure` 等价于 `const fn` 约束的子集），验证失败的标注是**编译错误**而非警告。

### 15.6 完整 REPL（交互式解释器）

**拒绝理由：**
REPL 的"会话状态持续性"与 move 语义的"一次性消耗"在物理上矛盾。在 REPL 中执行 `let b = a` 后 `a` 被 move，下一行输入 `a.push(1)` 必须报错，这与 REPL 的"探索性、可回溯"直觉冲突。替代方案：`trust eval`（§9.4）提供无状态的单次求值，notebook 风格通过显式导出/导入传递跨 cell 状态。

### 15.7 `!` 断言操作符用于 `Result`

**拒绝理由：**
`!` 已引入并限制为仅用于 `Option<T>`（§6.2.1）。不允许用于 `Result<T,E>`——`Option::None` 代表"值缺失"（开发者自己知道是否有值），`Result::Err` 代表"操作失败"（开发者无法在编译时预知）。允许 `Result!` 将训练开发者习惯性忽略错误，与渐近式安全哲学冲突。

### 15.8 默认静默的编译器自动修复

**拒绝理由：**
如果编译器默认自动插入 `.clone()` / `inout` / `mut`，开发者永远不会理解所有权转移，离开 Trust 后无法独立写 Rust。Trust 的 `--fix` 模式（§9.5）提供手动确认的辅助修复，但不替代所有权教育。这是 Trust 作为"Rust onboarding 路径"（§16）的根本承诺——你学会的不是 Trust 的语法糖，而是 Rust 的心智模型。

---

## 16. 为什么不用 Rust 直接写？

> 一个合理的质疑：Trust 的核心学习曲线是所有权心智模型而非语法——`fn` 换成 `function`、`&mut` 换成 `inout` 的语法糖真的值得一个全新的编译器基础设施吗？

**Trust 降低的不是"概念难度"，而是"表达难度"：**

| 维度 | Rust | Trust | 收益 |
|------|------|-------|------|
| **函数声明** | `fn foo<'a, T: AsRef<str>>(x: &'a T) -> &'a str` | `function foo<T extends AsRef<str>>(x: T): string` | 消除生命周期标注噪音，泛型语法更直观 |
| **错误处理** | `let file = File::open("a.txt").map_err(|e| MyError::Io(e))?;` | `let file = fs.open("a.txt")?;` | 链式错误转换被编译器自动处理 |
| **迭代/闭包** | `items.iter().filter(|&x| x > 0).map(|x| x * 2).collect::<Vec<_>>()` | `items.filter(x => x > 0).map(x => x * 2)` | 箭头函数、省略类型标注 |
| **async** | `async fn fetch() -> Result<Data> { ... }` + 手动 Pin/Box | `async function fetch(): Result<Data> { ... }` | `Pin<Box<dyn Future>>` 被编译器自动包裹 |
| **模式匹配** | `match msg { Msg::Quit => ..., Msg::Data { payload } => ... }` | `switch (msg.kind) { case "quit": ...; case "data": process(msg.payload) }` | TS 开发者熟悉的 switch/case |
| **共享状态** | `let counter = Arc::new(AtomicI32::new(0)); counter.fetch_add(1, Ordering::Relaxed);` | `shared counter = 0; counter.withLock(c => { c += 1; });` | shared 关键字消除 Arc/Atomic 的样板代码 |

**Trust 的目标用户画像：**

1. **TS/JS 全栈开发者** 需要写性能敏感或系统级模块（CLI 工具、图像处理、嵌入式 Wasm），但被 Rust 的 `fn` / `impl` / `<>`/ 生命周期语法劝退。
2. **团队中混合 TS 和 Rust 开发者**——Trust 提供统一的语法层，让 TS 开发者可以阅读和理解系统级代码（即使不亲手写），降低团队的认知分裂。
3. **教学场景**——用 Trust 教授所有权和并发概念，学生不需要同时学习 Rust 的陌生符号体系。

**Trust 不试图替代 Rust**，而是作为 Rust 生态的"TS 语法入口"。熟悉 Trust 的开发者可以平滑过渡到直接写 Rust（因为底层概念一致），这使 Trust 既是独立语言，也是 Rust 的 onboarding 路径。

---

## 17. 结语

Trust 是一项雄心勃勃的尝试，它打破了"系统语言必复杂，高级语言必不安全"的固有界限。通过果断放弃与静态安全冲突的特性，并引入精心设计的并发抽象，Trust 让开发者能够用熟悉的 TypeScript 风格，编写出无数据竞争、无内存错误、性能媲美 C++ 的软件。它不仅是语言，更是一套关于如何在易用性与安全性之间取得最优平衡的方法论。
