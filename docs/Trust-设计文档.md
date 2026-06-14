# Trust 语言设计文档 v2.0

> 版本：v2.0-draft · 分支：lang-redesign  
> 上一版已废弃 → `docs/Trust-设计文档-DEPRECATED.md`  
> 本文档基于 26 项设计决策重写

**代号:** Trust  
**理念:** JavaScript 的语法，Rust 的安全，编译到原生代码。  
**目标:** 语法贴近 JS，类型可选（编译器全推断），所有权在编译期保证内存/并发安全。编译器将 Trust 源码翻译为 Rust，再由 Rust 工具链编译为原生二进制。

**唯一需要开发者关心的复杂度：所有权。** 其余（Box/Rc/Arc/Weak、类型推断、内存分配）全部由编译器自动处理。

---

## 1. 类型系统

### 1.1 类型标注可选，编译器全推断

```js
// 无标注 → 编译器推断。推断失败 → 编译错误
let x = 42;          // 推断为 number
let name = "Alice";  // 推断为 string
let arr = [1, 2, 3]; // 推断为 number[]

// 有标注 → 固定类型
let count: number = 0;
let user: { name: string, age: number } = { name: "Bob", age: 30 };
```

### 1.2 基本类型

| 类型 | 说明 | 示例 |
|------|------|------|
| `number` | 64 位浮点（f64） | `42`, `3.14`, `-1` |
| `string` | UTF-8 字符串，API 贴近 JS | `"hello"`, `\`template ${x}\`` |
| `boolean` | `true` / `false` | |
| `null` | 唯一空值，无 `undefined` | |
| `T[]` | 动态数组，API 贴近 JS | `[1, 2, 3]` |
| `[T, U]` | 元组 | `["age", 30]` |
| `{ x: number }` | 匿名结构体 | |
| `T \| null` | 可空类型（编译器内部翻译为 Option<T>） | |
| `unknown` | 动态类型，必须被标注变量接住才能使用 | `let data: unknown = fetch(...)` |

### 1.3 纯结构类型

同形状即兼容。`{ x: number, y: number }` 不管来源可互相赋值。代码中没有 `interface` 关键字——直接用对象字面量描述形状。

```js
function distance(p: { x: number, y: number }): number {
    return Math.sqrt(p.x * p.x + p.y * p.y);
}

let pt = { x: 3, y: 4 };
distance(pt);  // ✅ 结构一致即兼容
```

### 1.4 隐式泛型

函数参数无类型标注 → 该参数为泛型。有标注 = 固定类型。可混用。

```js
// x, y 无标注 → 泛型
function first(arr, n) { return arr[n]; }

// a: number 固定，b 无标注 → b 泛型
function mix(a: number, b) { return a + b; }

// 全部标注 → 全部固定
function add(a: number, b: number): number { return a + b; }
```

### 1.5 `unknown` + `match`

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

`switch` 仍然用于普通值匹配：`switch (x) { case 1: ...; case "hello": ... }`

### 1.6 `null` 安全

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

`?.` 和 `??` 保留。编译器内部将 `T | null` 翻译为 Rust 的 `Option<T>`，`?.`/`??` 映射为 `.and_then()`/`.unwrap_or()`。

---

## 2. 所有权（唯一复杂度）

继承且只保留当前设计。`inout`/`move`/`shared` 语义不变。

### 2.1 变量默认不可变

```js
let x = 5;           // 不可变
let mut y = 10;      // 可变
y += 1;
```

### 2.2 移动语义

```js
let a = [1, 2, 3];
let b = a;           // a 所有权转移给 b，a 失效
// console.log(a);   // ❌ a 已被移动
```

### 2.3 参数三模式

| 声明 | 语义 |
|------|------|
| `function f(x: T)` | 只读借用，不消耗所有权 |
| `function f(inout x: T)` | 可变借用，独占访问 |
| `function f(move x: T)` | 所有权转移，x 调用后失效 |

```js
function pushOne(inout arr: number[]) { arr.push(1); }

let mut data = [1, 2, 3];
pushOne(inout data);  // data 被修改
```

### 2.4 闭包捕获

默认只读借用。`move` 关键字转移所有权，闭包变为 FnOnce（只能调用一次）。

```js
let data = [1, 2, 3];
let read = () => console.log(data.length);  // 只读借用
read(); read();                               // ✅ 可多次调用

let consume = move () => process(data);       // move 闭包
consume();                                    // ✅ 一次
// consume();                                 // ❌ FnOnce 只能调用一次
```

### 2.5 `shared` 共享可变状态

```js
shared counter = 0;

counter.withLock(c => { c += 1; });
let current = counter.withLock(c => c);
```

### 2.6 用户不接触底层 Rust 类型

`Box`/`Rc`/`Arc`/`Weak` 由编译器自动管理。`shared` 自动包裹为 `Arc<Mutex<T>>`，递归类型自动 `Box`，引用计数自动增减。用户代码中不出现这些 Rust 概念。

---

## 3. 函数与方法

### 3.1 函数声明

```js
// 标准函数
function add(a: number, b: number): number {
    return a + b;
}

// 无返回值必须显式 :void
function log(msg: string): void {
    console.log(msg);
}
// function log(msg: string) { ... }  // ❌ 编译错误：无标注返回类型
```

### 3.2 Go 风格 Receiver 方法

直接在类型上定义方法：

```js
function Point.distance(other: { x: number, y: number }): number {
    return Math.sqrt((this.x - other.x) ** 2 + (this.y - other.y) ** 2);
}

let pt = { x: 3, y: 4 };
pt.distance({ x: 0, y: 0 });  // 5
```

`this` 在 receiver 方法体内自动可用，默认只读借用。需要修改时声明 `inout this`，需要消耗时声明 `move this`。

---

## 4. 错误处理

### 4.1 `throw` / `try-catch`

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
}
```

**编译期保证：** `throw` 的参数必须是 `Error` 类型。`try/catch` 必须穷举所有可达的错误类型，或含兜底 `catch (e)`。缺漏 → 编译错误。

### 4.2 `panic!` 不可恢复错误

```js
function fatal(path: string): Config {
    return loadConfig(path) ?? panic!("config required");  // 程序崩溃
}
```

---

## 5. 控制流

### 5.1 `if` / `else`

`if` 是表达式，返回值。

```js
let label = if (score >= 60) { "pass" } else { "fail" };
```

### 5.2 `for` / `while`

```js
for (let i = 0; i < 10; i++) { console.log(i); }
for (let item of items) { process(item); }

let mut n = 5;
while (n > 0) { n -= 1; }
```

`loop` 不保留——`for`/`while` 足够。

### 5.3 `switch` 值匹配 vs `match` 类型匹配

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

## 6. 并发（精简版）

保留 `spawn`、`Channel`、`shared`。`select` 取消。

### 6.1 `spawn`

```js
// OS 线程（CPU 密集）
spawn(move () => { heavyComputation(); });

// 异步任务（I/O 密集）
spawn(move async () => {
    let data = await fetchData();
    process(data);
});
```

### 6.2 `Channel<T>`

```js
let (tx, rx) = Channel<string>(64);

spawn(move async () => {
    await tx.send("hello");
    tx.close();
});

spawn(move async () => {
    while (true) {
        try {
            let msg = await rx.receive();
            console.log(msg);
        } catch (e: ChannelClosed) {
            break;
        }
    }
});
```

### 6.3 `shared`

```js
shared counter = 0;
counter.withLock(c => { c += 1; });
```

---

## 7. `async` / `await` + `join()`

```js
async function fetchUser(id: number): { name: string, id: number } {
    let response = await http.get("/api/user/" + id);
    return response.json();
}

// 并发执行
let (user, config) = await join(fetchUser(42), fetchConfig());
```

惰性 Future（与 Rust 一致——调用时创建状态机但不执行，仅 `.await` 或 `spawn` 时推进）。

---

## 8. 模块系统

```js
import { add, PI } from "./math";
import greet from "./math";        // 默认导入
import * as math from "./math";    // 命名空间
export function hello(): void { console.log("hi"); }
```

---

## 9. 与 Rust 互操作 (FFI)

高阶功能——用于调用 Rust 生态库。`extern "rust"` 块用于声明外部 Rust 函数：

```js
extern "rust" {
    fn sqlx_query<T>(query: string, ...args): T;
}
```

`extern` 块内使用 `fn` 关键字（而非 `function`），提醒读者"此处映射的是 Rust 函数"。声明不经过 Trust 所有权检查——正确性是开发者的责任。

---

## 10. 测试体系（强化）

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
/// ```trust
/// assert(add(2, 3) == 5);
/// ```
function add(a: number, b: number): number { return a + b; }
```

测试命令：
```bash
trust test                      # 运行全部
trust test --filter "fetch"     # 按名称过滤
```

---

## 11. 标准库大纲

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

---

## 12. 编译器管线（不变）

```text
Trust 源码
  → Lexer / Parser (AST)
  → HIR (类型推断 + null 检查 + 方法解析)
  → TIR (所有权分析：moveck + borrowck)
  → Codegen (TIR → Rust 源码)
  → rustc (原生二进制)
```

结构化错误输出（`--error-format=json`）、`--fix` 交互修复、source map 全部保留。

---

## 13. 语法速查表

```js
// === 变量 ===
let x = 42;
let mut y = 10;
const MAX = 100;

// === 函数 ===
function add(a: number, b: number): number { return a + b; }
function log(msg: string): void { console.log(msg); }

// === 方法 (Go 风格 receiver) ===
function Point.distance(other: { x: number }): number { return this.x - other.x; }

// === 所有权 ===
function read(arr: number[]) { ... }          // 只读借用
function write(inout arr: number[]) { ... }   // 可变借用
function consume(move arr: number[]) { ... }  // 所有权转移

// === 错误 ===
throw Error("message");
try { ... } catch (e: IoError) { ... } catch (e) { throw e; }
panic!("unrecoverable");

// === 空值 ===
let name: string | null = null;
name ?? "guest";
user?.address?.street;

// === 控制流 ===
let label = if (x > 0) { "yes" } else { "no" };
for (let i = 0; i < 10; i++) { ... }
for (let item of items) { ... }
while (cond) { ... }
switch (x) { case 1: ...; break; default: ...; break; }

// === unknown + match ===
let data: unknown = fetch();
match (data) {
    case { name: string } => ...;
    case number[] => ...;
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
