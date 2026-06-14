# Trust 语言重构 Decision Log

> 创建：2026-06-14 | 分支：lang-redesign  
> 版本：Final — 全部 17 项设计决策已确认，所有重大分歧已清零

## 决策总表

| # | 决策 |
|---|------|
| D1 | 类型标注可选，编译器全推断 |
| D2 | 保留所有权（`inout`/`move`/`shared`） |
| D3 | 取消 `interface` |
| D4 | 纯结构类型（同形状即兼容） |
| D5 | 方法：Go 风格 receiver（`function Type.method() {}`） |
| D6 | 隐式泛型（有标注=固定，无标注=泛型，禁混用） |
| D7 | 无返回必须 `:void` |
| D8 | 禁止动态分发（无 `Box<dyn Trait>`、`Dynamic`） |
| D9 | `unknown` 类型（必须被标注变量接住才能用） |
| D10 | `match` 仅用于 `unknown`（类型匹配，全失败→panic） |
| D11 | 去掉 ADT |
| D12 | `number` = f64（合并 i32/f64） |
| D13 | 数组像 JS（动态数组） |
| D18 | 元组保留（`[T, U]`） |
| D19 | `const` 编译时常量保留 |
| D20 | 去掉 `loop`（`for`/`while` 足够） |
| D21 | `extern "rust"` FFI 作为高阶功能保留 |
| D22 | 测试强化（保留 `test`/`#[should_panic]`/文档测试，增加更多能力） |
| D23 | 去掉 `select` 和 `bigint` |
| D24 | 保留 `?.`/`??`（底层编译器用 `Option<T>` 内部翻译即可） |
| D25 | `unknown`= `dynamic` 但编译期必须确认类型（match 分支各走各的泛型单态化） |
| D26 | D6 混用规则：部分标注不算混用——已标注=固定，未标注=泛型（同 TS） |
| D14 | 错误：`throw`/`try-catch`（编译期穷举保证）+ `panic!` |
| D15 | 空值：只有 `null`，无 `undefined`（编译器强制 null 检查） |
| D16 | 编译器保证全部安全 |
| D17 | 用户不接触 Rust 底层类型（Box/Rc/Arc/Weak 由编译器内部分配） |

## 语言核心

- **语法：** JS 风格（`function`/`let`/`const`/`switch`/`try-catch`/`?.`/`??`）
- **语义：** Rust 所有权（`inout`/`move`/`shared`）+ 编译器全推断
- **目标：** Trust 源码 → Rust 源码 → 原生二进制
- **唯一复杂度：** 所有权。其余全部编译器自动处理

## 编译器工作检查清单

### 移除项
- [ ] `interface` 关键字
- [ ] `Result<T,E>` / `Option<T>` → 换成 `throw`/`null`
- [ ] `?` 操作符（Result 传播） → 换成 `try/catch`
- [ ] `impl` 块 → 换成 Go 风格 receiver
- [ ] `Box<dyn Trait>` / `Dynamic` — 禁止
- [ ] ADT（`type X = | ...`） — 换成 `unknown` + `match`
- [ ] 数值类型分离（i32/f64 → `number`=f64）
- [ ] 名义类型
- [ ] Rust 底层类型暴露（用户不写 Box/Rc/Arc/Weak）

### 新增项
- [ ] 编译器全类型推断
- [ ] `unknown` + `match` 类型匹配
- [ ] Go 风格 receiver 方法
- [ ] 隐式泛型（无标注=泛型）
- [ ] `try/catch` 穷举检查
- [ ] `null` 安全（编译器收窄 + `?.` + `??`）
- [ ] `:void` 强制返回标注
- [ ] JS 风格数组 + JS 风格字符串 API
- [ ] 精简并发（spawn/Channel/shared）
- [ ] `async`/`await` + `join()`
