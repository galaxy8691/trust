# Trust 类型系统设计 —— JS 语法 + 编译器推导 + unknown 边界

## 核心哲学

> **写起来像 JavaScript，安全由编译器保证。**

Trust 不是 TypeScript 的替代，也不是 Rust 的语法糖。它是一个从 JavaScript 开发者视角出发、但在编译期消除所有运行时类型错误的系统语言。

### 和现有语言的差异

| 语言 | 类型标注 | 运行时安全 | 目标用户 |
|------|----------|------------|----------|
| JavaScript | 无 | ❌ 无 | 快速开发 |
| TypeScript | 大量 | ❌ 编译到 JS，运行时仍可能崩 | 需要提示的 JS 开发者 |
| Rust | 中等 | ✅ 编译期保证 | 系统编程 |
| **Trust** | **极少（仅 unknown 边界）** | ✅ **编译期保证** | **JS 开发者想获得安全** |

---

## 类型推导系统

### 默认推导，不标类型

```rust
let x = 5;                    // 推导为 i32
let name = "hello";           // 推导为 String
let nums = [1, 2, 3];         // 推导为 Vec<i32>
let flag = true;              // 推导为 bool

function add(a, b) {          // 推导为 add(a: i32, b: i32) -> i32
    return a + b;
}

function greet(name) {        // 推导为 greet(name: String) -> String
    return `Hello, ${name}`;
}
```

**推导规则：**
- 字面量：整数默认 `i32`，浮点默认 `f64`，字符串 `String`
- 数组：元素类型统一推导，`[]` 空数组需要上下文或显式标注
- 函数：参数类型从调用处反向推导（Hindley-Milner 风格）
- 返回值：从函数体推导

### 泛型自动推导

```rust
function identity(value) {    // 推导为 identity<T>(value: T) -> T
    return value;
}

let n = identity(42);         // T = i32
let s = identity("hi");       // T = String

function first(arr) {         // 推导为 first<T>(arr: Vec<T>) -> T
    return arr[0];
}
```

### 何时需要显式标注

```rust
// 1. 没有上下文的空容器
let nums = [];                // ❌ 推导失败：不知道元素类型
let nums: Vec<i32> = [];      // ✅ 显式标注

// 2. 跨模块的公开 API（推荐标注，作为文档）
export function calculate(data: Vec<f64>): f64 { ... }

// 3. 复杂泛型约束
function process<T: Serializable>(items: Vec<T>) { ... }

// 4. unknown 解锁（见下节）
let raw = fetch(url);         // raw: unknown
let user: User = raw;         // 显式声明期望类型
```

---

## unknown：安全边界

### 设计意图

> **unknown 是 Trust 的安全阀门。**
>
> 当编译器无法静态知道类型时，返回 unknown。unknown 是"活着的"值（有所有权），但"死的"操作对象（什么都不能做）。
>
> 开发者必须显式声明"我相信这是什么类型"，才能解锁使用。

### 规则

```rust
// 1. unknown 可以来自任何地方
let raw: unknown = fetch("https://api.example.com/user");
let input: unknown = readFile("config.json");
let value: unknown = parseUserData(str);

// 2. unknown 上禁止一切操作
raw.name;              // ❌ error: unknown has no field 'name'
raw + 1;               // ❌ error: cannot perform arithmetic on unknown
raw.toString();        // ❌ error: unknown has no methods
console.log(raw);      // ❌ error: cannot format unknown

// 3. unknown 不能传给需要具体类型的参数
process(raw);          // ❌ error: expected User, found unknown

// 4. unknown 只能被"真实类型"接住（解锁）
let user: User = raw;  // ✅ 编译器插入运行时检查，或要求 User 实现 Validate

// 解锁后，user 拥有完整类型能力
console.log(user.name);
user.update();
```

### 解锁方式

#### 方式一：显式类型标注（静态降级）

```rust
let raw = fetch(url);           // unknown
let user: User = raw;           // 编译器检查 User 结构，插入验证
```

**编译器行为：**
- 如果 `User` 实现 `Validate` trait：插入运行时验证，失败时返回 `Result`
- 如果 `User` 是简单结构体：编译器生成字段存在检查和类型匹配
- 如果 `User` 是标记类型（nominal）：直接 cast（开发者自负责任）

#### 方式二：模式匹配（运行时收窄）

```rust
match (raw) {
    case { id: number, name: string } => {
        // 在这个分支，raw 被收窄为 { id: number, name: string }
        console.log(name);
    }
    case [number] => {
        // raw 是数字数组
    }
    default => {
        throw Error("unexpected format");
    }
}
```

#### 方式三：Result 包装（安全路径）

```rust
function fetchUser(id: number): Result<User, ParseError> {
    let raw = fetch(`/api/users/${id}`);  // unknown
    return parseAs<<User>(raw);           // 验证失败返回 Err
}

let user = fetchUser(42)?;  // ? 传播错误
```

### unknown 的所有权语义

```rust
let raw = fetch(url);      // raw: unknown，拥有返回值所有权

// 解锁时所有权转移
let user: User = raw;      // raw 被 move，user 获得所有权
// raw 现在不可用（符合 Trust move 语义）

// 如果不想 move，先 clone？
let raw2 = raw.clone();    // ❌ unknown 没有 clone — 不知道大小
// 必须先解锁才能 clone
let user: User = raw;
let user2 = user.clone();  // ✅
```

---

## 类型层级

```
unknown                    ← 顶层：什么都不知道
  │
  ├─ 显式标注 ──► 具体类型 T
  │
  ├─ 模式匹配 ──► 收窄类型
  │
  ├─ Dynamic                ← 标签联合（封闭集合）
  │   ├─ Dynamic.Number
  │   ├─ Dynamic.String
  │   └─ ...
  │
  └─ Box<dyn Trait>         ← trait 对象（开放集合）

推导类型                    ← 编译器自动推断，无需标注
  ├─ i32 / f64 / bool / String
  ├─ Vec<T>
  ├─ { x: number, y: number }  // 匿名结构体
  └─ function(...) -> ...
```

---

## 和 JavaScript 的对比

### 变量声明

```javascript
// JavaScript
let x = 5;
let y = "hello";
let obj = { a: 1, b: 2 };
```

```rust
// Trust
let x = 5;              // i32
let y = "hello";        // String
let obj = { a: 1, b: 2 };  // 匿名结构体 { a: i32, b: i32 }

obj.a = 3;              // ✅
obj.c = 4;              // ❌ 编译错误：结构体没有字段 'c'
```

### 函数

```javascript
// JavaScript
function add(a, b) {
    return a + b;
}
add(1, "2");  // "12" — 运行时灾难
```

```rust
// Trust
function add(a, b) {    // 推导为 add(a: i32, b: i32) -> i32
    return a + b;
}
add(1, "2");            // ❌ 编译错误：预期 i32，找到 String
```

### 外部数据

```javascript
// JavaScript
let data = await fetch("/api/user");
console.log(data.name);  // 可能 undefined，运行时崩
```

```rust
// Trust
let raw = fetch("/api/user");     // unknown
console.log(raw.name);            // ❌ 编译错误

let user: User = raw;             // 显式解锁
console.log(user.name);           // ✅
```

### 数组操作

```javascript
// JavaScript
let nums = [1, 2, 3];
let doubled = nums.map(x => x * 2);
let mixed = [1, "two", 3];  // 允许
```

```rust
// Trust
let nums = [1, 2, 3];             // Vec<i32>
let doubled = nums.map(x => x * 2);  // Vec<i32>，自动推导

let mixed = [1, "two", 3];        // ❌ 编译错误：数组元素类型必须统一
// 如果要混合，用 Dynamic 或联合类型
let mixed: Vec<Dynamic> = [Dynamic.Number(1), Dynamic.String("two")];
```

---

## 编译器推导算法（概述）

### 局部推导

```rust
function process(items) {        // items: ?T
    let first = items[0];        // first: ?T::Element
    return first * 2;            // 推断 ?T::Element = i32
}                                // 所以 items: Vec<i32>, 返回 i32

// 调用时验证
process([1, 2, 3]);             // ✅
process(["a", "b"]);            // ✅ 推导为 Vec<String>，但 String * 2 非法 → 错误在函数体
```

### 跨函数推导

```rust
// 从调用处反向推导
function transform(data) {       // data: ?T
    return data.map(x => x.toUpperCase());  // 需要 data 有 map，x 有 toUpperCase
}

transform(["a", "b"]);           // 推断 data: Vec<String>，返回 Vec<String>
```

### 推导失败

```rust
let x = [];                     // ❌ 推导失败：无元素，无上下文

function mystery(a, b) {        // a: ?T, b: ?U
    return [a, b];              // ❌ 推导失败：不知道数组元素类型
}

// 解决：显式标注
function mystery(a: number, b: string) {
    return [Dynamic.Number(a), Dynamic.String(b)];
}
```

---

## unknown 的实现策略

### 编译期

```rust
// 源码
let raw = fetch(url);
let user: User = raw;

// 编译后（概念）
let raw: Unknown = fetch(url);
let user: Result<User, CastError> = Unknown::cast<<User>(raw);
// 如果 User 没有实现 Validate，编译器警告："unchecked cast"
```

### 运行时

```rust
// 如果 User 实现 Validate
trait Validate {
    fn validate(value: Unknown) -> Result<Self, ValidationError>;
}

impl Validate for User {
    fn validate(value: Unknown) -> Result<User, ValidationError> {
        // 检查 required 字段存在
        // 检查字段类型匹配
        // 嵌套结构递归验证
    }
}
```

### 优化

- 简单结构体（无嵌套）：编译器内联验证代码
- 已知 API 结构：编译期生成解析代码（类似 serde）
- JIT 验证缓存：同一结构多次验证时复用

---

## 迁移路径

### 从 JavaScript

```javascript
// JS 代码
async function loadUser(id) {
    const res = await fetch(`/api/users/${id}`);
    const data = await res.json();
    return {
        name: data.name,
        age: data.age
    };
}
```

```rust
// Trust 等价代码
async function loadUser(id) {
    let raw = fetch(`/api/users/${id}`);  // unknown
    let data: { name: string, age: number } = raw;  // 显式解锁
    return { name: data.name, age: data.age };
}
```

**差异：**
- 没有 `await res.json()` — fetch 直接返回 unknown
- 多一行类型解锁 — 但保证了 safety
- 没有 `const/let` 区别 — 默认不可变，需要 `mut` 显式声明

---

## 总结

| 特性 | Trust 设计 |
|------|-----------|
| 语法风格 | 接近 JavaScript |
| 类型系统 | 静态强类型，大量自动推导 |
| 类型标注 | 极少需要（仅 unknown 边界和泛型约束） |
| 外部数据 | unknown 类型，必须显式解锁 |
| 运行时安全 | 编译期保证，无类型错误崩溃 |
| 性能 | 编译到 Rust，零成本抽象 |
| GC | 无（所有权 + 引用计数可选） |

**Trust 的目标：**
> 让 JavaScript 开发者不用学习类型体操，就能获得 Rust 级别的安全和性能。
>
> 唯一的代价：处理外部数据时，多一行类型声明。
