# Trust 语言设计文档 v2.0

> 版本：v2.0 · 分支：lang-redesign  
> 上一版已废弃 → `docs/Trust-设计文档-DEPRECATED.md`  
> 本文档基于 26 项设计决策重写，是 Trust 语言的唯一权威规范

**代号:** Trust  
**理念:** JavaScript 的语法，Rust 的安全，编译到原生代码。  
**目标:** 语法贴近 JS，类型可选（编译器全推断），所有权在编译期保证内存/并发安全。编译器将 Trust 源码翻译为 Rust，再由 Rust 工具链编译为原生二进制。

**唯一需要开发者关心的复杂度：所有权。** 其余（Box/Rc/Arc/Weak、类型推断、内存分配）全部由编译器自动处理。

---

## 1. 引言

### 1.1 背景与动机

JavaScript 生态强大但无法用于系统编程——GC 停顿、单线程模型、动态类型带来性能天花板和安全隐患。Rust 提供了零成本抽象和所有权系统，但学习曲线陡峭，`fn`/`impl`/`<>`/生命周期标注等概念对 JS 开发者形成障碍。

Trust 融合二者之长：用 JS 语法写逻辑，用 Rust 的所有权模型保安全。Trust 源码直接翻译为 Rust 源码，再由 Rust 工具链编译为原生二进制。

### 1.2 设计哲学

- **语法亲和：** 尽可能保持 JS 风格（`function`/`let`/`const`/`switch`/`try-catch`/`?.`/`??`），降低迁移成本
- **类型可选：** 编译器全推断。推断失败 → 编译错误
- **编译期安全：** 内存安全、并发安全在编译期保证，不引入隐式 GC 或重量级运行时
- **错误信息友好：** 编译错误映射回原始 Trust 代码，并提供修复建议
- **唯一复杂度：** 所有权（`inout`/`move`/`shared`）。其余由编译器自动处理

### 1.3 保留的 JS 特性

- `let` / `const` 变量声明
- 箭头函数、模板字符串
- `async` / `await` 异步语法
- 隐式泛型（参数无标注=泛型，有标注=固定）
- 模块化 `import` / `export` 语法
- `?.` / `??` 空值处理
- `test` 测试框架 + `#[should_panic]` + 文档测试

### 1.4 必须牺牲的 JS 特性

| 特性 | 处理方式 |
|------|----------|
| 垃圾回收（GC） | 移除。采用所有权，无 GC 停顿 |
| 原型继承、`prototype` | 移除。用组合代替 |
| `eval`、`new Function` | 完全禁止 |
| `Proxy`、`Reflect` | 不支持 |
| 隐式类型转换（如 `"5" * 2`） | 禁止。`number` = f64，转换需显式 |
| 对象动态增删属性 | 编译为固定字段结构体 |
| `undefined` | 移除。只有 `null` |
| `any` | 移除。用 `unknown` 替代 |
| `interface` / `implements` | 移除。纯结构类型 |
| `class` / `extends` | 移除。用对象字面量 + Go 风格 receiver |
| 完整 REPL | 用 `trust eval` 替代（见 §12.4） |

---

## 2. 类型系统

### 2.1 类型标注可选，编译器全推断

```js
// 无标注 → 编译器推断。推断失败 → 编译错误
let x = 42;          // 推断为 number
let name = "Alice";  // 推断为 string
let arr = [1, 2, 3]; // 推断为 number[]

// 有标注 → 固定类型
let count: number = 0;
let user: { name: string, age: number } = { name: "Bob", age: 30 };
```

### 2.2 基本类型

| 类型 | 说明 | 示例 |
|------|------|------|
| `number` | 64 位浮点（f64） | `42`, `3.14`, `-1` |
| `string` | UTF-8 字符串，API 贴近 JS | `"hello"`, `` `template ${x}` `` |
| `boolean` | `true` / `false` | |
| `null` | 唯一空值，无 `undefined` | |
| `T[]` | 动态数组，API 贴近 JS | `[1, 2, 3]` |
| `[T, U]` | 元组 | `["age", 30]` |
| `{ x: number }` | 匿名结构体 | |
| `T \| null` | 可空类型（内部翻译为 `Option<T>`） | |
| `unknown` | 动态类型，必须被标注变量接住才能使用 | `let data: unknown = fetch(...)` |

> **`number` = f64：** 合并了旧版 i32/f64 分离。整数和浮点统一为 64 位浮点。`number` 之间可以自由运算，不需要 `as` 转换。位运算（`&`/`|`/`^`/`<<`/`>>`）仅允许 `number` 类型（不区分整数/浮点），编译器不保证位运算在浮点值上的行为——开发者需要自己确保操作数是整数。
> 
> **整数语义场景：** `for (let i = 0; i < 10; i++)` 中的迭代变量 `i` 和数组索引（`arr[n]` 中的 `n`）自动当做整数处理——编译器在 codegen 时生成 `n as usize`，不要求显式转换。循环计数器和数组索引是 Trust 中唯一保留隐式整数语义的场景。

### 2.3 具名类型别名

使用 `type` 关键字为结构类型命名。`type` 仅创建**透明别名**——`type Point = { x: number, y: number }` 等价于直接书写 `{ x: number, y: number }`。同形状即兼容（结构类型）——两个同名 `Point` 或两个形状相同的匿名结构体可以互相赋值。

```js
// 具名类型别名——方便复用和文档化
type Point = { x: number, y: number };
type IoError = { message: string, code: number };
type Config = { host: string, port: number };

// receiver 方法定义在具名类型上
function Point.distance(other: Point): number {
    return Math.sqrt((this.x - other.x) ** 2 + (this.y - other.y) ** 2);
}

let pt: Point = { x: 3, y: 4 };
pt.distance({ x: 0, y: 0 });  // 5
```

**`type` 语法约束：**
- `type` 右侧**仅允许对象字面量类型**（`{ ... }`）——不允许 ADT 联合语法（D11）
- `type` 创建的是**透明别名**——不引入新的类型身份。两个 `type` 如果右侧形状相同，编译为同一 Rust 类型
- `type` 是可选语法——可以完全不用，直接用对象字面量描述形状。`type` 的作用是复用和文档化

### 2.4 纯结构类型

同形状即兼容。`{ x: number, y: number }` 不管来源可互相赋值。没有 `interface` 关键字——直接用对象字面量描述形状。

```js
function distance(p: { x: number, y: number }): number {
    return Math.sqrt(p.x * p.x + p.y * p.y);
}

let pt = { x: 3, y: 4 };
distance(pt);  // ✅ 结构一致即兼容

// {x, y} 属性简写（仅限类型上下文明确时）
let x = 10;
let y = 20;
let pt2: { x: number; y: number } = { x, y };  // 等价 { x: 10, y: 20 }
```

> **约束：** 简写仅在**类型上下文明确**时有效——即赋值目标、函数参数、返回值类型三者之一已知。对于 `let obj = { x, y }` 且无法从上下文推断目标类型时，编译器报错要求显式标注。

### 2.4 隐式泛型

函数参数无类型标注 → 该参数为泛型。有标注 = 固定类型。可混用。

```js
// x, y 无标注 → 泛型
function first(arr, n) { return arr[n]; }

// a: number 固定，b 无标注 → b 泛型
function mix(a: number, b) { return a + b; }

// 全部标注 → 全部固定
function add(a: number, b: number): number { return a + b; }
```

> **隐式泛型 vs 旧版显式 `<T>`：** v2.0 不保留 `<T extends ...>` 语法。泛型参数通过"无标注"隐式声明。如果函数需要泛型约束，通过标注参数为具体结构类型来实现。这比 `<T>` 更简洁，也更贴近 JS 开发者的直觉——"我没写类型 = 我不知道也不关心它是什么类型，你帮我处理"。

### 2.5 `unknown` + `match`

`unknown` 类似 JS 的 `dynamic`，但不能直接使用——必须通过 `match` 确认类型，编译期保证安全。`match` 的每个 `case` 是一个类型模式，全不匹配时 `panic`。

```js
let data: unknown = fetchData();

match (data) {
    case { name: string, age: number } => console.log(data.name);
    case number[] => data.forEach(x => console.log(x));
    case string => console.log(data);
}
// 全部不匹配 → panic!
```

编译器实现：`unknown` 编译为隐式泛型参数 + 每个 `match` 分支生成单态化代码。`match` 的每个 `case` 对应一个可能的 Rust 类型，编译器在调用点根据实际类型选择分支。这避免了 `Box<dyn Any>` 的动态分发开销。

`switch` 用于普通值匹配：`switch (x) { case 1: ...; case "hello": ... }`。

### 2.6 `null` 安全

只有 `null`，没有 `undefined`。编译器强制 null 检查——不检查直接使用 → 编译错误。

```js
let name: string | null = "Alice";
// console.log(name.length);  // ❌ name 可能是 null

if (name !== null) {
    console.log(name.length);  // ✅ 编译器收窄
}

let display = name ?? "guest";     // null 时使用默认值
let street = user?.address?.street; // 链式安全访问，哪层为 null 就返回 null
```

**`?.` 所有权约束：**
- 如果 `obj` 是只读借用，`?.` 不消耗所有权，`obj` 后续仍可使用
- 如果 `obj` 是 owned，`?.` 会 **move** `obj`，之后 `obj` 失效
- 推荐在只读借用上下文中使用 `?.`，或在 move 后不再需要原变量时使用

编译器内部将 `T | null` 翻译为 Rust 的 `Option<T>`，`?.`/`??` 映射为 `.and_then()`/`.unwrap_or()`。

---

## 3. 所有权（唯一复杂度）

### 3.1 变量默认不可变

```js
let x = 5;           // 不可变
// x = 6;            // ❌ x 不可变

let mut y = 10;      // 可变
y += 1;              // ✅
```

> **C-style for 循环例外：** `for (let i = 0; i < N; i++)` 中的迭代变量 `i` 隐式可变。这是唯一允许 `let` 声明被修改的场景。`for-of` 和 `while` 不受此例外影响。

### 3.2 移动语义

`let b = a;` 后 `a` 所有权转移给 `b`，`a` 失效。

```js
let a = [1, 2, 3];
let b = a;           // a 所有权转移给 b，a 失效
// console.log(a);   // ❌ a 已被移动
```

> **与 JS 的关键差异：** JS 中 `let b = a` 后 `a` 和 `b` 共享同一份数据。Trust 中 `a` 立即失效。这是 Trust 获得内存安全所付出的"语法税"——强制开发者显式思考数据生命周期。`Copy` 类型（`number`、`boolean`）会自动复制，不受移动语义影响。

### 3.3 参数三模式

| 声明 | 语义 | 调用处 |
|------|------|--------|
| `function f(x: T)` | 只读借用，不消耗所有权 | `f(x)` |
| `function f(inout x: T)` | 可变借用，独占访问 | `f(inout x)` |
| `function f(move x: T)` | 所有权转移，x 调用后失效 | `f(move x)` |

```js
function pushOne(inout arr: number[]) { arr.push(1); }

let mut data = [1, 2, 3];
pushOne(inout data);  // data 被修改
```

借用规则：同一时刻只能有一个可变借用或多个只读借用。编译器在后台应用 Rust 的借用规则。

### 3.4 闭包捕获

默认只读借用。`move` 关键字转移所有权，闭包变为 FnOnce（只能调用一次）。

```js
let data = [1, 2, 3];
let read = () => console.log(data.length);  // 只读借用
read(); read();                               // ✅ 可多次调用
console.log(data);                            // ✅ data 仍可用

let consume = move () => process(data);       // move 闭包
consume();                                    // ✅ 一次
// consume();                                 // ❌ FnOnce 只能调用一次
// console.log(data);                         // ❌ data 已被 move
```

> **设计取舍——`move` 闭包统一为 FnOnce：** 与 Rust 不同，Trust 将 `move` 闭包统一视为 `FnOnce`。理由：`move` 在函数参数中意味着"所有权转移"，闭包的 `move` 是同一语义的延伸。`spawn` 场景本就只调用一次。需要多次调用的回调场景，使用默认借用闭包（不写 `move`）。

### 3.5 显式引用 `&`

绝大多数情况下隐式借用足够——函数参数默认只读借用，方法调用自动取引用。仅在需要显式声明引用变量时使用 `&`：

```js
let data = [1, 2, 3];
let r1 = &data;    // 显式只读引用
let r2 = &data;    // ✅ 多个只读引用 OK
```

### 3.6 `shared` 共享可变状态

```js
shared counter = 0;

counter.withLock(c => { c += 1; });
let current = counter.withLock(c => c);
```

- `withLock` 提供闭包内的独占访问，锁自动获取和释放
- 对于 `number` 等支持原子操作的类型，编译器优化为原子指令
- **死锁风险：** 编译器不声称能检测所有嵌套死锁。建议避免嵌套 `withLock`

### 3.7 用户不接触底层 Rust 类型

`Box`/`Rc`/`Arc`/`Weak` 由编译器自动管理。`shared` 自动包裹为 `Arc<Mutex<T>>`，递归类型自动 `Box`，引用计数自动增减。用户代码中不出现这些 Rust 概念。

### 3.8 生命周期自动推导

绝大多数情况无需标注。仅在返回引用时需要轻量标注，编译器提供自动修复提示。

---

## 4. 函数与方法

### 4.1 函数声明

```js
// 标准函数
function add(a: number, b: number): number {
    return a + b;
}

// 单表达式简写
function square(x: number) = x * x;

// 箭头函数
let double = (x: number): number => x * 2;
let greet = (name) => `Hello, ${name}`;  // 参数类型可选

// 无返回值必须显式 :void
function log(msg: string): void {
    console.log(msg);
}
// function log(msg: string) { ... }  // ❌ 编译错误：无 :void
```

### 4.2 Go 风格 Receiver 方法

直接在类型上定义方法：

```js
function Point.distance(other: { x: number, y: number }): number {
    return Math.sqrt((this.x - other.x) ** 2 + (this.y - other.y) ** 2);
}

let pt = { x: 3, y: 4 };
pt.distance({ x: 0, y: 0 });  // 5
```

`this` 在 receiver 方法体内自动可用，默认只读借用。需要修改时声明 `inout this`，需要消耗时声明 `move this`。

方法解析：同形状类型共享方法——如果 B 与 A 同形状且 A 有 `.foo()`，B 也可以调用 `.foo()`。如果在不同模块定义了同名方法，导入路径决定使用哪个。

---

## 5. 错误处理

### 5.1 `throw` / `try-catch`

```js
function loadConfig(path: string): { host: string, port: number } {
    if (!fs.exists(path)) {
        throw Error("config not found");
    }
    return parse(fs.read(path));
}

try {
    let config = loadConfig("app.conf");
    startServer(config);
} catch (e: IoError) {
    console.log("file error: " + e.message);
} catch (e: ParseError) {
    console.log("format error: " + e.message);
} catch (e) {
    throw e;  // 兜底：重新抛出未知错误
}
```

**编译期保证：**
- `throw` 的参数必须是 `Error` 类型或自定义错误类型（`type X = { message: string }` 自动满足 `Error` 的形状要求——即需包含 `message: string` 字段）
- `catch` 按**结构形状**匹配（不是按类型名）。`catch (e: { message: string, code: number })` 匹配任何拥有这两个字段的错误对象。`catch (e: IoError)` 等价于 `catch (e: { message: string, code: number })`——但优先推荐显式写形状
- `try/catch` 必须穷举所有可达的错误类型，或含兜底 `catch (e)`
- 缺漏 → 编译错误

```js
// 自定义错误类型——仅需包含 message: string 字段即可作为 Error 抛出
type IoError = { message: string, code: number };
type ParseError = { message: string, line: number };

function readFile(path: string): string {
    if (!fs.exists(path)) {
        throw { message: "not found: " + path, code: 404 };  // 匹配 IoError 形状
    }
    return fs.read(path);
}

// catch 按结构形状匹配
try {
    let content = readFile("data.txt");
    process(content);
} catch (e: { message: string, code: number }) {
    console.log("IO error: " + e.message + " (code: " + e.code + ")");
} catch (e: { message: string, line: number }) {
    console.log("parse error at line " + e.line);
}
```

**`try` 块的所有权状态：** `try` 块内 move 的变量在 `catch` 块中不可用（已被消费）。如果需要 `catch` 块访问变量，在 `try` 前 clone 或使用只读借用传入。

#### 5.1.1 内部翻译策略：`throw`/`try-catch` → `Result<T, E>`

Rust 无异常机制，`throw`/`try-catch` 在 codegen 阶段翻译为 `Result<T, E>` + `match`：

```js
// Trust 源码
function readFile(path: string): string {
    if (!fs.exists(path)) {
        throw { message: "not found", code: 404 };
    }
    return fs.read(path);
}
```

```rust
// 生成的 Rust 代码（简化示意）
fn readFile(path: &str) -> Result<String, ReadFileError> {
    if !fs::exists(path) {
        return Err(ReadFileError::NotFound { message: "not found".into(), code: 404 });
    }
    Ok(fs::read_to_string(path))
}
```

**错误类型推断：** 编译器从函数体中收集所有 `throw` 语句抛出的结构形状，自动合成 `Result<T, E>` 中的错误枚举 `E`。调用方通过 `catch (e: {...})` 按结构形状匹配——编译器生成对应的 `match` 分支。

```js
// 编译器推断 E = { message: string, code: number } | { message: string, line: number }
try {
    let content = readFile("data.txt");
    process(content);
} catch (e: { message: string, code: number }) {  // 匹配 IO 错误
    console.log("IO error: " + e.message);
} catch (e: { message: string, line: number }) {  // 匹配解析错误
    console.log("parse error at line " + e.line);
}
```

```rust
// 生成的 Rust 代码（简化示意）
match readFile("data.txt") {
    Ok(content) => process(content),
    Err(e @ ReadFileError::NotFound { .. }) => println!("IO error: {}", e.message),
    Err(e @ ReadFileError::Parse { .. }) => println!("parse error at line {}", e.line),
}
```

**推断算法边界：**
- **函数内：** 编译器收集本函数所有 `throw` 语句的错误形状 + 本函数调用的其他 Trust 函数的错误类型（通过其 `Result<T, E>` 签名中的 `E`），合并为当前函数的错误枚举
- **FFI 边界：** `extern "rust"` 函数的错误类型无法自动推断——需在 `extern` 声明中显式标注错误形状：`fn external_fn(x: number): number throws { message: string }`
- **递归/互调：** 编译器固定点迭代直到错误枚举收敛。最大迭代深度 32（与泛型深度限制一致），超限报错要求显式标注
- **性能：** 推断仅增加 O(n) 编译开销（n = 函数调用图大小），不引入运行时开销

> **为什么用 `Result` 内部翻译而非 `panic!`+`catch_unwind`：** `catch_unwind` 不保证捕获所有 panic（`Abort` 等不可捕获），且无法实现穷举检查。`Result` 是 Rust 原生的可恢复错误机制，与 Trust 的编译期安全承诺一致。`throw`/`try-catch` 是语法糖——用户看到的是 JS 风格的 throw/catch，编译器内部生成的是 `Result<T, E>` + `match`，包括错误枚举的自动合成。

### 5.2 `panic!` 不可恢复错误

```js
function fatal(path: string): Config {
    return loadConfig(path) ?? panic!("config required");  // 程序崩溃
}
```

---

## 6. 控制流

### 6.1 `if` / `else`

`if` 是表达式，返回值。

```js
let label = if (score >= 60) { "pass" } else { "fail" };
```

### 6.2 `for` / `while`

```js
for (let i = 0; i < 10; i++) { console.log(i); }
for (let item of items) { process(item); }

let mut n = 5;
while (n > 0) { n -= 1; }
```

> **`loop` 不保留。** `for`/`while` 足够覆盖所有场景。需要无限循环时用 `while (true) {...}`。

### 6.3 `switch` 值匹配 vs `match` 类型匹配

```js
// switch —— 值匹配
switch (status) {
    case 200: handleOk(); break;
    case 404: handleNotFound(); break;
    default: handleUnknown(); break;
}

// match —— 类型匹配（仅用于 unknown）
let data: unknown = fetch();
match (data) {
    case { name: string } => console.log(data.name);
    case number[] => data.forEach(x => process(x));
    case string => console.log(data);
}
```

---

## 7. 并发（精简版）

保留 `spawn`、`Channel`、`shared`。`select` 取消。

### 7.1 `spawn`

```js
// OS 线程（CPU 密集）
spawn(move () => { heavyComputation(); });

// 异步任务（I/O 密集）
spawn(move async () => {
    let data = await fetchData();
    process(data);
});
```

`spawn` 要求闭包为 `move` 且捕获变量满足 `Send`。类型是否可跨线程发送由编译器自动分析。

### 7.2 `Channel<T>`

`Channel<T>(capacity?: number)` 返回 `(Sender<T>, Receiver<T>)` 元组。`Sender` 可 Clone（多个发送方），`Receiver` 不 Clone（唯一接收方）。默认有界容量 64。

`ChannelClosed` 是标准库预定义错误类型（`std::sync` 模块导出）：

```js
type ChannelClosed = { message: string };
// 当发送端全部 drop 时，receive() throw 此错误
// 当接收端 drop 时，send() throw 此错误
```

```js
let (tx, rx) = Channel<string>(64);  // (Sender, Receiver)

spawn(move async () => {
    await tx.send("hello");
    tx.close();
});

spawn(move async () => {
    try {
        let msg = await rx.receive();
        console.log(msg);
    } catch (e: ChannelClosed) {
        console.log("closed");
    }
});
```

`Sender` 可 Clone（多个发送方），`Receiver` 不 Clone（唯一接收方）。默认有界容量 64。发送即所有权转移。

### 7.3 `shared`

```js
shared counter = 0;
counter.withLock(c => { c += 1; });
```

### 7.4 数据竞争彻底根除

由于以下规则的组合，**数据竞争**在编译期被物理消除：

1. 不可变数据可自由跨线程共享
2. 可变数据通过 `inout` 保证独占访问
3. 跨线程共享的可变数据必须包裹在 `shared` + `withLock` 或通过 `Channel` 转移所有权

> **数据竞争 vs 逻辑竞态：** Trust 消除的是**数据竞争**（非同步内存访问导致的未定义行为）。但**逻辑竞态**（线程执行顺序导致结果不确定）仍然可能发生——这需要应用层设计来解决。

---

## 8. `async` / `await` + `join()`

```js
async function fetchUser(id: number): { name: string, id: number } {
    let response = await http.get("/api/user/" + id);
    return response.json();
}

// 并发执行——join() 同时 poll 多个 Future
let (user, config) = await join(fetchUser(42), fetchConfig());
```

惰性 Future（与 Rust 一致——调用时创建状态机但不执行，仅 `.await` 或 `spawn` 时推进）。

> **与 JS Promise 的关键差异：** JS 中 `let p = fetch()` 立即启动请求。Trust 中 `let f = fetchUser(42)` 创建惰性 Future，不执行任何代码。只有在 `.await` 或 `spawn` 时才推进。如果需要真正并发，使用 `join()`。

### 8.1 异步运行时

默认使用 Tokio。可通过 `Trust.toml` 配置切换：

```toml
[runtime]
async = "tokio"  # 默认；可选 "async-std" | "smol"
```

---

## 9. 模块系统

```js
import { add, PI } from "./math";
import greet from "./math";        // 默认导入
import * as math from "./math";    // 命名空间
export function hello(): void { console.log("hi"); }
```

编译后映射为 Rust 的 `mod`/`use`。使用 Cargo 生态，`Trust.toml` 声明依赖。

### 9.1 包管理

使用 Rust 的 Cargo 生态。可直接引用 crates.io 上的 Rust 包。

**类型绑定策略：**
| 级别 | 方式 | 适用场景 |
|------|------|---------|
| **自动绑定（计划中）** | `trust bindgen` 工具，从 Rust crate 的 `rustdoc` JSON 自动生成 Trust 类型声明 | 大多数 crate |
| **手写绑定** | 通过 `extern` 块声明外部 Rust 函数 | 复杂 API |

---

## 10. 与 Rust 互操作 (FFI)

高阶功能——用于调用 Rust 生态库。

```js
extern "rust" {
    fn sqlx_query<T>(query: string, ...args): T;
}
```

`extern` 块内使用 `fn` 关键字（而非 `function`），提醒读者"此处映射的是 Rust 函数"。声明不经过 Trust 所有权检查——正确性是开发者的责任。

### 10.1 与外部生态交互

- **无法使用 npm 包：** Trust 不兼容 JavaScript 运行时，所有依赖必须来自 Rust 生态或重新实现
- **生成 Rust 绑定：** 工具链可将 Trust 编译为 Rust crate，供其他 Rust 项目使用

---

## 11. 测试体系（强化）

```js
// 单元测试
test function add_works() {
    assert(add(2, 3) == 5);
}

// 异步测试
test async function fetch_ok() {
    let result = await api.get("/health");
    assert(result.status == 200);
}

// 预期 panic 的测试
#[should_panic]
test function bad_unwrap() { panic!("expected"); }

// 文档测试
/// 计算平方值
///
/// ```trust
/// assert(square(3) == 9);
/// assert(square(-4) == 16);
/// ```
function square(x: number): number { return x * x; }

// 属性测试（计划 v0.2+）
#[property]
test function sort_is_idempotent(arr: number[]) {
    let sorted1 = arr.sorted();
    let sorted2 = sorted1.sorted();
    assert(sorted1 == sorted2);
}

// 并发压力测试（计划 v0.3+）
#[concurrent]
test function counter_concurrency() {
    shared counter = 0;
    for (let i = 0; i < 10; i++) {
        spawn(move () => counter.withLock(c => { c += 1; }));
    }
    assert(counter.withLock(c => c) == 10);
}
```

测试命令：
```bash
trust test                      # 运行全部
trust test --filter "fetch"     # 按名称过滤
trust test --threads 4          # 并行线程数
```

> 底层复用 Cargo test 基础设施，与 `cargo-tarpaulin`、`cargo-nextest` 等 Rust 工具兼容。

---

## 12. 编译策略

Trust 编译器采用多层中间表示（IR）架构：

```
Trust 源码
  → Lexer / Parser (AST)
  → HIR (类型推断 + null 检查 + 方法解析)
  → TIR (所有权分析：moveck + borrowck)
  → Codegen (TIR → Rust 源码)
  → rustc (原生二进制)
```

**TIR 层设计：** TIR 层的 borrow checker 不需要完全复刻 rustc 的 MIR borrow checker——Trust 的类型系统和所有权规则是 Rust 的受限于集，TIR 只需验证 Trust 语义层面的安全性。生成的 Rust 代码在语义上保证通过 rustc 的检查（soundness by construction）。复杂场景（如高阶生命周期多态）将回退为生成带有显式生命周期标注的 Rust 代码并依赖 rustc 进行最终验证。

**结构化错误输出（`--error-format=json`）：**
```json
{
  "message": "变量 `data` 在第 12 行被移动后在第 15 行被使用",
  "level": "error",
  "code": "E0382",
  "spans": [
    { "file": "src/main.trust", "line_start": 12, "line_end": 12, "label": "data 在此处被移动" },
    { "file": "src/main.trust", "line_start": 15, "line_end": 15, "label": "移动后使用" }
  ],
  "children": [{ "message": "考虑在此处使用 data.clone()", "level": "help" }]
}
```

> 结构化错误仅包含 Trust 源码层的变量名和位置，不暴露 TIR 中间名。AI 编码助手可直接消费此 JSON。

### 12.1 运行时库（`ferro_rt`）

为支持 `Channel`、`shared`、`spawn` 等特性，提供轻量运行时库。`shared` 编译为 `Arc<Mutex<T>>`，`Channel` 编译为 `mpsc` channel。

### 12.2 Source Map 与调试

生成 Rust 代码的同时输出 source map，将 Trust 源码映射到生成的 Rust 代码行。

| 层级 | 方案 | 说明 |
|------|------|------|
| **IDE 内联调试** | Trust LSP 在 IDE 中展示 Trust 源码，断点通过 source map + DWARF 映射 | VS Code 优先 |
| **gdb/lldb 命令行** | 生成 `#line` 指令和 DWARF 信息 | 需要 DWARF 路径重写 |
| **回退模式** | 生成带 Trust 注释的 Rust 代码 | v0.1 即可提供 |

### 12.3 `--fix` 编译器辅助修复

```bash
$ trust check --fix src/main.trust
src/main.trust:15 — 变量 `data` 在第 12 行被移动
  建议：在此处添加 .clone()
  应用此修复？(y/N)
```

> **设计原则：** `--fix` 默认关闭，每次修复需开发者手动确认——编译器不会偷偷隐藏所有权决策。

### 12.4 `trust eval` 表达式求值

```bash
$ trust eval "2 + 3"
5

$ trust eval "let x = 5; let y = x * 2; y"
10

$ echo 'let arr = [1,2,3]; arr.map(x => x * 2)' | trust eval -
[2, 4, 6]
```

每次求值是**独立的编译单元**，无状态共享，因此不存在 REPL 的所有权矛盾。

---

## 13. 标准库大纲

| 模块 | 内容 | 优先级 |
|------|------|--------|
| `std::console` | `console.log` | v0.1 |
| `std::collections` | 动态数组、Map、Set | v0.1 |
| `std::string` | JS 风格字符串 API（`split`/`slice`/`replace`/`trim`/`toUpperCase`） | v0.1 |
| `std::sync` | `Channel`、`shared`、`spawn` | v0.1 |
| `std::async` | `join`、`sleep`、异步 I/O | v0.1 |
| `std::fs` | 文件读写 | v0.1 |
| `std::time` | 时间戳、定时器 | v0.1 |
| `std::net` | HTTP 客户端、TCP/UDP | v0.2 |
| `std::serde` | 序列化（JSON 等） | v0.2 |
| `std::crypto` | 哈希、加密原语 | v0.3 |
| `std::process` | 子进程管理、环境变量 | v0.2 |

> 在标准库覆盖不足的过渡期，可通过 `extern` 绑定直接使用 Rust 生态的相应 crate。

---

## 14. 被明确拒绝的特性及理由

| 拒绝的特性 | 理由 |
|-----------|------|
| `interface` / `implements` | JS 没有；Trust 用纯结构类型替代 |
| `Result<T,E>` / `Option<T>` | 换成 `throw`/`try-catch` + `null` |
| `?` 操作符（`Result` 传播） | 换成 `try-catch` |
| ADT（`type X = \| ...`） | `unknown` + `match` 覆盖了相同场景 |
| `impl` 块 | Go 风格 receiver 更简洁 |
| `Box<dyn Trait>` / `Dynamic` | D8 禁止动态分发 |
| `select` 多通道竞速 | 精简并发设计；未来需要时再加 |
| `bigint` | `number`=f64 足够 |
| `loop` | `for`/`while` 足够 |
| `defer` 延迟执行 | 所有权系统天然管理资源生命周期；`withLock` 块级作用域覆盖剩余场景 |
| `\|>` 管道操作符 | JS 没有；方法链 `.filter().map()` 已覆盖 |
| 过程宏（proc-macro） | 破坏静态可分析性 |
| `// @trust: pure` 意图注释 | 无编译器验证 = 谎言注释 |
| 完整 REPL | move 语义与会话状态持续性物理矛盾；用 `trust eval` 替代 |
| 默认静默的编译器自动修复 | 开发者不会理解所有权；`--fix` 交互相反 |
| `undefined` | 只有 `null` |
| `any` | `unknown` 替代（但必须类型确认后才能用） |

---

## 15. 未来展望与限制

### 15.1 未来可能扩展

- **SIMD 与向量化支持**
- **Wasm 编译目标：** Trust → Rust → wasm，保留所有安全特性
- **与 JS 互操作层：** 通过 `wasm-bindgen` 类似机制
- **受限声明式宏（计划 v0.3+）：** 类似 Rust `macro_rules!`，但 hygienic、仅限局部语法转换
- **`trust bindgen`：** 从 Rust crate 的 rustdoc 自动生成 Trust 类型声明（v0.2+）

### 15.2 已知限制

- **学习曲线：** 开发者必须理解所有权、移动语义。Trust 的语法降低了符号层面的认知负荷，但无法隐藏所有权心智模型
- **生态重建：** 无法使用 npm 生态，需要逐步积累原生库
- **编译时间：** Trust → Rust（毫秒级）+ rustc（分钟级）。增量编译可缓解
- **无法 100% 模拟 JS 动态行为：** 部分设计模式需彻底改变

---

## 16. 为什么不用 Rust 直接写？

Trust 降低的不是"概念难度"，而是"表达难度"：

| 维度 | Rust | Trust |
|------|------|-------|
| **函数声明** | `fn foo<'a, T: AsRef<str>>(x: &'a T) -> &'a str` | `function foo(x: string): string` |
| **错误处理** | `File::open("a.txt").map_err(...)?;` | `try { let f = fs.open("a.txt"); ... } catch (e: IoError) {...}` |
| **迭代/闭包** | `items.iter().filter(...).map(...).collect()` | `items.filter(x => x > 0).map(x => x * 2)` |
| **async** | 手动 `Pin<Box<dyn Future>>` | 编译器自动包裹 |
| **模式匹配** | `match msg { Msg::Quit => ... }` | `switch (msg.kind) { case "quit": ... }` |
| **共享状态** | `Arc::new(AtomicI32::new(0))` | `shared counter = 0` |

**Trust 的目标用户：**

1. **JS 全栈开发者** 需要写性能敏感模块（CLI、图像处理、嵌入式），但被 Rust 语法劝退
2. **混合团队**——Trust 提供统一语法层，JS 开发者可阅读和理解系统代码
3. **教学场景**——教所有权和并发，学生不需要同时学习 Rust 的符号体系

---

## 17. 语法速查表

```js
// === 变量 ===
let x = 42;
let mut y = 10;    y += 1;
const MAX = 100;

// === 函数 ===
function add(a: number, b: number): number { return a + b; }
function square(x: number) = x * x;                    // 单表达式简写
let double = (x: number): number => x * 2;             // 箭头函数
function log(msg: string): void { console.log(msg); }

// === 类型别名 ===
type Point = { x: number, y: number };
type IoError = { message: string, code: number };

// === 方法 (Go 风格 receiver) ===
function Point.distance(other: Point): number { return this.x - other.x; }

// === 所有权 ===
let a = [1, 2, 3];
let b = a;                          // a 被 move, 失效
let c = b.clone();                  // 深拷贝
function f(x: number[]): void { ... }           // 只读借用
function f(inout x: number[]): void { ... }     // 可变借用
function f(move x: number[]): void { ... }      // 所有权转移

// === 错误 ===
throw Error("message");
try { ... } catch (e: IoError) { ... } catch (e) { throw e; }
panic!("unrecoverable");

// === 空值 ===
let name: string | null = null;
name ?? "guest";       // null 时用默认值
user?.address?.street;  // 链式安全访问

// === 控制流 ===
if (x > 0) { ... } else { ... }
let label = if (score >= 60) { "pass" } else { "fail" };  // 表达式
for (let i = 0; i < 10; i++) { ... }
for (let item of items) { ... }
while (cond) { ... }
switch (x) { case 1: ...; break; default: ...; break; }

// === unknown + match ===
let data: unknown = fetch();
match (data) {
    case { name: string } => ...;
    case number[] => ...;
    case string => ...;
}

// === 并发 ===
spawn(move () => { ... });
spawn(move async () => { ... });
let (tx, rx) = Channel<string>(64);
shared counter = 0;
counter.withLock(c => { c += 1; });

// === async ===
async function fetch(): Data { ... }
let (a, b) = await join(fetchA(), fetchB());

// === 模块 ===
import { foo } from "./bar";
export function baz(): void { ... }

// === 测试 ===
test function myTest() { assert(true); }
test async function myAsyncTest() { ... }
#[should_panic] test function bad() { panic!(); }

// === FFI ===
extern "rust" { fn some_crate_fn(x: number): number; }
```

---

> **下一步：** 编译器适配。现有 parser/HIR/TIR/codegen 需要按本文档修改——移除 interface/Result/Option/ADT/impl/select/loop/bigint 相关代码，新增 receiver 方法解析、隐式泛型、unknown+match、try-catch 穷举检查、null 安全收窄。

---

## 18. AI 友好性

Trust 从语言设计的第一天起就将 AI 编码工具（LLM 代码生成、Copilot 风格补全、自动重构）作为一等公民考量。

### 18.1 语法设计对 LLM 友好

Trust 的语法贴近 JS——这是对 LLM 最友好的设计决策之一。LLM 在 JS 代码上的训练数据远多于 Rust，因此生成 Trust 代码的"语法正确率"天然更高：

- `function`、`switch/case`、箭头函数——这些符号与 LLM 的训练分布高度吻合
- `inout`、`shared`、`withLock` 等 Trust 特有关键字是**无歧义的标记**——LLM 可以明确学习"这里是所有权边界"
- Trust 的 `try-catch` + `null` 模式比 Rust 的 `Result`/`Option` + `match` 链更接近 LLM 的训练数据形状

> **已知风险：** LLM 会"自信地"生成 JS 风格的错误代码——如 `let b = a; a.push(1)`。Trust 通过结构化错误输出（§12）让 AI 工具快速发现并修复这类语义错误。

### 18.2 结构化错误输出

Trust 的 `--error-format=json` 为 AI 工具提供了精确的修复锚点（文件、行号、错误代码、建议修复），使 AI 编码助手可以实现"编译失败 → 解析 JSON 错误 → 生成修复 → 重新编译"的自动闭环。

### 18.3 确定性编译模型：编译通过 = 无内存错误

AI 生成的 Trust 代码只需审查**业务逻辑正确性**——编译通过即证明无内存安全 bug、无数据竞争。这大幅降低了 AI 生成代码的审查成本。

### 18.4 AI 专用所有权分析 API（计划 v0.2+）

Trust 编译器的 TIR 层在分析阶段拥有完整的变量所有权图。计划提供 `--analyze-ownership` 模式，以结构化格式输出指定位置的完整借用状态，供 AI 工具进行精确修复。

---

## 19. 完整程序示例：HTTP 服务

```js
import { spawn, Channel, shared } from "std::sync";
import { HttpServer, Request, Response } from "std::net";
import { readToString } from "std::fs";

type Config = { port: number, static_dir: string };

// 请求计数器（原子操作，无锁）
shared request_count = 0;

// 路由处理
async function handleRequest(req: Request): { status: number, body: string } {
    // 原子递增计数器
    request_count.withLock(c => { c += 1; });

    // 路由匹配
    let path = req.url.path;
    if (path == "/health") {
        let count = request_count.withLock(c => c);
        return { status: 200, body: `{"status":"ok","requests":${count}}` };
    }

    if (path.startsWith("/static/")) {
        let file_path = `./public/${path.slice(8)}`;
        try {
            let content = readToString(file_path);
            return { status: 200, body: content };
        } catch (e: IoError) {
            return { status: 404, body: "not found" };
        }
    }

    return { status: 404, body: "not found" };
}

// 入口
function main(): void {
    let config = loadConfig() ?? { port: 3000, static_dir: "./public" };
    let server = HttpServer.bind(`127.0.0.1:${config.port}`);

    console.log(`listening on port ${config.port}`);

    // 每个连接 spawn 一个异步任务
    spawn(move async () => {
        while (true) {
            try {
                let conn = await server.accept();
                spawn(move async () => {
                    let res = await handleRequest(conn.req);
                    conn.respond(res);
                });
            } catch (e: ServerClosed) {
                break;
            }
        }
    });

    // 保持主线程存活
    // 生产环境可用 Channel 实现优雅关闭
}
```

---

## 20. 被明确拒绝的特性及理由（完整版）

以下特性经过了严格的设计评审后被**永久拒绝**。记录在此是为了防止未来的设计讨论反复回到已被论证不可行的方案上。

| 拒绝的特性 | 理由 |
|-----------|------|
| `interface` / `implements` | JS 没有。Trust 用纯结构类型替代 |
| `Result<T,E>` / `Option<T>` | 换成 `throw`/`try-catch` + `null` |
| `?` 操作符（`Result` 传播） | 换成 `try-catch` |
| ADT（`type X = \| ...`） | `unknown` + `match` 覆盖了相同场景（编译期穷举 → 运行时类型匹配） |
| `impl` 块 | Go 风格 receiver 更简洁，语义等价 |
| `Box<dyn Trait>` / `Dynamic` 枚举 | 禁止动态分发。`unknown` + `match`（单态化）替代 |
| `select` 多通道竞速 | 精简并发设计。未来需要时可通过标准库扩展 |
| `bigint` | `number`=f64 足够覆盖大多数场景 |
| `loop` | `for`/`while` 足够。`while (true)` 可替代无限循环 |
| `defer` 延迟执行 | 所有权系统天然管理资源生命周期。`withLock` 块级作用域覆盖剩余场景 |
| `\|>` 管道操作符 | JS 没有。方法链 `.filter().map().reduce()` 已覆盖相同场景 |
| 过程宏（proc-macro） | 破坏"看到的代码就是被编译的代码"的可分析性。替代方案：`trust generate` 子命令 |
| `// @trust: pure` 等无验证意图注释 | 编译器不强制检查 → 开发者可标注 `pure` 却在函数内执行 I/O → "谎言注释" |
| 完整 REPL | move 语义的一次性消耗与 REPL 的会话状态持续性在物理上矛盾。替代方案：`trust eval` |
| 默认静默的编译器自动修复 | 开发者永远不会理解所有权。`--fix` 提供手动确认的辅助修复 |
| `undefined` | 只有 `null`。减少空值类型数量，降低认知负荷 |
| `any` | `unknown` 替代——但 `unknown` 必须被 `match` 类型确认后才能使用，比 `any` 更安全 |
