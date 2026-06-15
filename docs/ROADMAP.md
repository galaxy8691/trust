# Trust 语言实现路线图

> 本文档定义 Trust 编译器及生态从零到可交付的完整实施路径。
> 每一步标注优先级（P0=阻塞、P1=重要、P2=增强）、预估工作量和依赖关系。
>
> **基准设计：** `docs/Trust-设计文档.md`（v2.0，唯一权威规范）。
> **重要变更（2026-06-14）：** 设计重构为 v2.0——移除 `interface`/`impl`/ADT/`Option`/`Result`/`?`/`select`/`loop`/`bigint`，`number` 合并为 f64，新增具名类型别名（名义）/纯结构类型/Go 风格 receiver/隐式泛型/`unknown`+`match`/`throw`-`try-catch`/`null` 安全。Phase 1 已基于**旧设计**完成，**Phase 2 负责将其修正到 v2.0**，Phase 3+ 实现 v2.0 新特性。

---

## Phase 0：语言规范 (v0.0 — 规范冻结)

**目标：** 将 `Trust-设计文档.md` 转化为可执行的、无歧义的语言规范文档。

### 0.1 编写 Trust Language Specification

**工作量：** 2-3 周  
**优先级：** P0（阻塞所有后续工作）

内容：
- 词法规范（token 定义、关键字列表、字面量格式、注释格式）
- 语法规范（形式化 EBNF 或类似格式的完整语法定义）
- 语义规范（每个语法制导翻译的规则——AST → HIR → TIR 的降级过程）
- 类型系统规范（v2.0：具名类型别名（名义身份）、纯结构类型、隐式泛型、`unknown`+`match` 的完整图）
- 所有权规则规范（移动语义、借用规则、`inout`/`move`/`shared` 的形式化定义）
- 编译模型规范（TIR 节点定义、代码生成映射规则）

**产出物：** `spec/trust-spec.md`（或 `spec/` 目录下的多文件）

### 0.2 编写 Trust 标准库规范

**工作量：** 1 周  
**优先级：** P0  
**依赖：** 0.1

- `trust_std` 每个模块的 API 签名列表
- 模块间的依赖图
- 与 Rust 标准库的映射关系

> **v2.0 对齐：** Phase 0 基于旧设计（含 `std::result` / 用户面 `Option`/`Result`）。v2.0 对齐在 Phase 2 增量推进——2.1 移除用户面 `Option`/`Result` 并更新模块大纲（设计 §13），Phase 4 补 `std::error` 与 `throws` 签名，Phase 5+ 补并发相关模块。

### 0.3 设计文档与规范的一致性审计

**工作量：** 3 天  
**优先级：** P0  
**依赖：** 0.1, 0.2  
**状态：** ✅ 完成（基于**旧设计**；审计报告：`docs/phases/0/0.3/audit-report.md`）

> **规范对齐 v2.0：** Phase 0 已完成并提交，不再回退。`spec/trust-spec.md` 与 `spec/stdlib.md` 的 v2.0 重新对齐在 **Phase 2** 中随实现增量推进（见 Phase 2 §规范与标准库对齐策略），首步在 **2.1** 完成词法/废弃语法清理与 stdlib 骨架修正。

---

## Phase 1：编译器核心 (v0.1 — 最小可用编译器) ✅ 完成 (2026-06-14)

> **TODO 追踪：** `docs/phases/1/TODO.md`
>
> ⚠️ **基于旧设计完成。** 本 Phase 在 v2.0 重构前交付，包含 `loop`/`bigint`、`number`=i32（区分 i32/f64）、`as` 转换等已废弃特性。**Phase 2 负责将其修正到 v2.0。** 已交付的编译管线骨架（parser/HIR/TIR/codegen/error/trustc）整体保留并复用。

**已交付（子任务索引）：** 1.1 项目初始化、1.2 `trust_parser`、1.3 `trust_hir`、1.4 `trust_tir`、1.5 `trust_codegen` + `ferro_rt` stub、1.6 `trust_error`、1.7 `trustc`（compile/check/eval）、1.8 集成测试（56 个全部通过）。详见 `docs/phases/1/TODO.md`。

**已下沉至 Phase 2 的遗留项（Phase 1 标记完成但未交付）：** `Trust.toml` 解析与桥接 `Cargo.toml`（原 1.7.2）、`&mut x`（#7）、闭包调用 `r()`（#8）、JSON→serde 评估（#10）；以及 CI 性能回归监控、`cargo bench` CI job、fuzz 语料库初始化等工程项。

**Phase 1 交付标准：** ✅ 达成
```ts
function main() {
    console.log("Hello, Trust!");
}
```

---

## Phase 2：修正 Phase 1 对齐 v2.0 设计 (v0.1.1)

> **目标：** 在不重写编译管线骨架的前提下，把 Phase 1 实现修正到 v2.0 设计。这是后续所有 Phase 的前置——必须先让 v0.1 编译器的语义与权威设计一致。
>
> **分支：** `phase2-v2-align`
>
> **本 Phase 合计：** 约 5-6 周（2.1~2.5 之和）
>
> **规范工作：** 除编译器实现修正外，本 Phase 同步推进 `spec/trust-spec.md`（语言规范）与 `spec/stdlib.md`（标准库规范）的 v2.0 增量对齐——删旧、前瞻写入、随 Phase 3+ 补齐新特性条目（非一次性整篇重写）。详见 `docs/phases/2/TODO.md` §规范与标准库对齐策略。  
> **延期/承接总表：** `docs/phases/DEFERRED-AND-HANDOFFS.md`（各 spec 中「归 Phase X」条目须同步登记，避免遗漏）。

### 2.1 移除已废弃的语法与类型 + 规范对齐 v2.0

**工作量：** 1.5-2 周  
**优先级：** P0  
**依赖：** Phase 1

| 子任务 | 涉及 crate | 说明 |
|--------|-----------|------|
| 移除 `loop` | parser(lexer 关键字/AST `Loop`/`LoopExpr`/parser/codegen)、hir、tir | 用 `while (true)` 替代；删除 `Loop` Stmt 与 `LoopExpr` |
| 移除 `bigint` | parser(lexer i64 字面量/`BigIntType`)、hir typeck、codegen | 设计 §14 已删；i64 精确整数改由 FFI 提供 |
| 移除 `interface`/`impl` 关键字 | parser(lexer 关键字表 + parser 同步点) | 未实现语义，仅清理关键字与同步点残留 |
| 移除 `select` 预留 | parser(AST 转义槽)、文档 | 设计 §14 已删 |
| 移除其余旧设计残留关键字 | parser lexer + `TokenKind` | `undefined`(§2.2)、`None`/`Some`(Option 不暴露)、`Ok`/`Err`(Result 不暴露)、`Rc`/`Arc`/`Weak`/`Box`(§3.7 用户不接触)、`dyn`(§14 禁动态分发)、`extends`(§2.5 无 `<T extends>`) |
| 关键字表重核（54 → 43） | parser lexer | **移除 16 个**：`loop`/`bigint`/`interface`/`impl`/`select`/`undefined`/`None`/`Some`/`Ok`/`Err`/`Rc`/`Arc`/`Weak`/`Box`/`dyn`/`extends`；**新增 5 个**：`unknown`/`try`/`catch`/`null`/`panic`（`type`/`match`/`throw`/`shared`/`spawn` 等已存在，无需新增） |

**规范对齐 v2.0（原 0.4，下沉至此，不回退 Phase 0）：**

**`spec/trust-spec.md`（语言规范）：**
- 删除已废弃条目：`interface`/`impl`/ADT/旧后缀 `expr?`/`expr!`/用户面 `Option`/`Result`/`select`/`loop`/`bigint`
- 重写 LEX-REQ-001 关键字表（43 个）与字面量说明（5 种）
- 前瞻写入 2.2/2.3 条目：`number`=f64 整数语义、块体函数强制返回标注、表达式体函数、位运算 `number` 约束
- Phase 3+ 新特性（receiver、`unknown`+`match`、`throw`/`try-catch` 等）**随各自 Phase 实现时补齐**

**`spec/stdlib.md`（标准库规范）：**
- 删除/废止 `std::result` 与用户面 `Option`/`Result` API
- 更新模块依赖图，对齐设计 §13 模块大纲（新增 `std::error`、`std::console` 骨架占位）
- 仍含 `Result<T,E>` 的 API 标注过渡注记（完整 `throws` 签名归 Phase 4）

**共通：**
- 每完成一项修正即更新设计文档 / `trust-spec` / `stdlib` / `design-constraints.md` 四者一致性
- **废止旧审计**：在 `docs/phases/0/0.3/audit-report.md` 顶部标注"基于旧设计、已被 v2.0 取代"
- **章节冻结矩阵**：词法规范在 2.1 完成前冻结 → 类型系统核心在 2.2/2.3 前冻结 → 标准库模块大纲 2.1 骨架后逐 Phase 冻结 → 具名类型/泛型/`unknown` 随 Phase 3 → 错误/`null` 随 Phase 4 → 并发/FFI 可延至 Phase 5/7

### 2.2 `number` 统一为 f64

**工作量：** 1-1.5 周  
**优先级：** P0  
**依赖：** 2.1

| 子任务 | 涉及 crate | 说明 |
|--------|-----------|------|
| 类型统一 | hir typeck | 删除 i32/f64 区分与 `i32 + f64 → error` 规则；`number` 之间自由运算 |
| codegen 映射 | codegen | `number` → `f64`（替换现有 i32 映射），字面量生成 f64（`404` → `404.0`） |
| `as` 收敛 | parser/hir | `number` 之间不再需要 `as`（设计 §2.2）；`as` 仅保留用于必要的非 number 转换 |
| 整数语义 | hir/codegen | f64↔整数自动转换：数组索引 `arr[n]` → `n as usize`、循环计数、长度/容量、FFI 整数（设计 §2.2 表）；超 2^53 字面量/索引发 `help` 级警告 |
| 位运算约束 | hir typeck | 位运算 `&`/`\|`/`^`/`<<`/`>>` 仅允许 `number`（设计 §2.2）；编译器不保证浮点值上的位运算行为，开发者自行确保整数操作数 |

> 注：数组下标/`.length` 的完整整数语义依赖集合类型，复杂部分可与 Phase 6（`std::collections`）协同；Phase 2 先落地循环计数与字面量精度警告。

### 2.3 函数声明规则对齐 ✅

**工作量：** 1 周 · **状态：** 已完成（2026-07）  
**依赖：** 2.1 · **规格：** `docs/phases/2/2.3/2.3-spec.md` · **核对：** `2.3/cross-check.md`

- **块体函数强制返回标注**（name_res `lower_function`，含 `export function` + typeck body）
- **表达式体函数** `function f(...) = expr` + `is_expression_body` 推断
- **箭头函数** `(params): T? =>` parser + 返回推断/标注优先
- **延期：** `(name) =>` 参数推断 → Phase 3；trustc 边界 e2e → 2.5.3

### 2.4 承接 Phase 1 遗留项

**工作量：** 1 周  
**优先级：** P1  
**依赖：** 2.1

- **#7 可变引用 `&mut x`**：parser（`let mut` 已支持，补 `&mut` 表达式）+ TIR borrowck 可变借用路径
- **#8 闭包调用 `r()`**：name_res 保留 ArrowFn 绑定 + 闭包 `TirFunction` 实现（为 Phase 3 隐式泛型的闭包类型推断打基础）
- **#10 JSON→serde 迁移评估**：`trust_error` JSON 输出是否引入 serde 的零依赖策略决策

### 2.5 测试与夹具迁移

**工作量：** 0.5 周  
**优先级：** P0  
**依赖：** 2.1, 2.2, 2.3, 2.4（`&mut`/闭包 e2e 依赖 2.4）

- 移除/改写依赖 `loop`/`bigint`/`i32-f64 区分`/`as 数字转换` 的端到端夹具与快照
- 56 个集成测试在 v2.0 语义下重新全部通过
- `number`=f64 的 codegen 快照更新（`i32` → `f64`）

**Phase 2 交付标准：** Phase 1 的全部能力在 v2.0 语义下工作——`number`=f64、无 `loop`/`bigint`/`interface`/`impl`/`select`、块体函数强制返回标注、`&mut`/闭包调用可用，56+ 集成测试通过。

---

## Phase 3：类型系统与方法 (v0.1.2)

> **本 Phase 合计：** 约 11-12.5 周（3.1~3.4 之和）
>
> **持续承接（贯穿 Phase 3-4）：** #12 各 crate 错误类型渐进迁移至 `trust_error::Diagnostic`（每子阶段迁移一批，不设截止时间）。

### 3.1 具名类型别名与纯结构类型

**工作量：** 3 周  
**优先级：** P1  
**依赖：** 2.3

- `type X = { ... }` 解析（右侧仅对象字面量类型，不允许 ADT 联合）
- **双重语义**（设计 §2.3）：方法绑定层名义（每个 `type` → 独立 Rust struct），赋值兼容层结构（同形状即兼容，编译器生成 `From`/`Into` 互转）
- 纯结构类型兼容判定（匿名 `{x,y}` ↔ 具名类型）
- `{x, y}` 属性简写（类型上下文明确时）

### 3.2 Go 风格 Receiver 方法

**工作量：** 2 周  
**优先级：** P1  
**依赖：** 3.1, 2.3（`function Type.method(...)` 本质是函数声明，依赖 2.3 的函数声明规则）

- `function Type.method(...)` 解析 → 编译为 Rust `impl` 块
- `this` 隐式 receiver（默认只读借用 / `inout this` / `move this`）
- **方法解析（名义模型）**：方法绑定到具名类型；匿名结构体调用时自动 `From`/`Into` 转换到匹配的具名类型；多个同形状具名类型有同名方法 → 报错要求显式标注消歧
- 跨模块同名方法由导入路径决定
- > **承接：** #9 跨函数 `inout` 标注的对称检查（`inout this` 方法是典型触发场景）

### 3.3 隐式泛型

**工作量：** 4-5 周（±2；约束反推 + 单态化的实际复杂度可能超预期）  
**优先级：** P1  
**依赖：** 3.1

- **无标注参数 = 泛型，有标注 = 固定**（设计 §2.5），**不引入 `<T>` 语法**
- 泛型参数推断 + 约束反推（从函数体使用反推所需能力）
- 单态化代码生成（每个调用点按实际类型实例化）
- 与 #8 闭包类型推断共享推断机制

### 3.4 `unknown` + `match`

**工作量：** 2-2.5 周  
**优先级：** P1  
**依赖：** 3.1, 3.3

- `unknown` 类型：不能直接使用（取成员/调方法 → 编译错误）
- **类型化装载**：`let p: People = expr`（标注目标类型）→ 运行期形状校验，失败 `throw`
- **`match` 类型匹配**：`case` 类型模式 + 分支内自动收窄 + 可选 `case _` 兜底（无兜底全不匹配 → `panic`）
- **运行期表示**：编译器生成带类型标签的动态载荷（`Value`：tag + payload；对象/数组变体携带字段名→类型描述符），非 `Box<dyn Any>` 虚表分发
- `switch`（值匹配）与 `match`（类型匹配）并存，明确区分
- > **隐含依赖 4.1：** "装载形状不符 → `throw`"依赖 Phase 4 的 `throw`/`try-catch`。Phase 3 实现时可先用 `panic!` 占位，Phase 4 完成后替换为 `throw`

---

## Phase 4：错误处理与空安全 (v0.1.3)

> **本 Phase 合计：** 约 5-6 周（4.1~4.3 之和）

### 4.1 `throw` / `try-catch`

**工作量：** 3-4 周（±1；跨函数错误枚举固定点合并）  
**优先级：** P1  
**依赖：** 3.1

- `throw` 参数 = 含 `message: string` 的结构；`Error("msg")` 内置便捷构造器（返回 `{ message }`）
- `try` / `catch (e: {shape})` / 兜底 `catch (e)` 解析
- **catch 按结构形状匹配**：声明顺序优先、不可达 catch 警告、同形状错误用字段值二次判别（设计 §5.1）
- **内部翻译 `throw`/`try-catch` → `Result<T, E>`**（设计 §5.1.1）：每种 throw 形状 → 一个枚举变体，自动合成错误枚举 `E`；catch 形状 → 满足该形状的变体集合的 `match`
- **穷举推断**：固定点迭代合并调用图的错误枚举（最大深度 32，超限要求显式 `throws` 标注）；`throws` 语法 = 返回类型后跟对象字面量类型；泛型函数 E 不随类型参数化、单态化实例共享同一枚举；FFI `extern` 函数需显式 `throws` 标注
- `Result<T,E>`/`Option<T>` 仅作编译器内部实现类型，不暴露给用户

### 4.2 `null` 安全

**工作量：** 1.5 周  
**优先级：** P1  
**依赖：** 2.1, 3.1（2.1 清理旧 `Option`/`Result`/`?` 关键字；3.1 提供 `T` 的具名类型基础。与 `number`=f64 无关）

- `T | null` 类型（内部翻译为 `Option<T>`）
- 编译器强制 null 检查 + `if (x !== null)` 收窄
- `??` 空值合并（→ `unwrap_or`）、`?.` 链式安全访问（→ `and_then`）
- `?.` 所有权约束（owned 上 `?.` 会 move；只读借用上不消耗）

### 4.3 `panic!`

**工作量：** 3 天  
**优先级：** P1  
**依赖：** 1.4（间接依赖 Phase 1 已交付的 1.5 codegen / 1.6 `trust_error`）

- `panic!("msg")` → Rust `panic!`
- `?? panic!(...)` 组合
- > **承接：** #11 修复建议覆盖率扩展（从 3 种规则扩展到 ≥8 种；用户在此阶段频繁遇到所有权 + null/错误处理错误，修复建议价值最大）

---

## Phase 5：并发与异步 (v0.2)

> 设计 §6/§7/§8。**无 `select`**（已删）。
>
> **本 Phase 合计：** 约 8-9 周（5.1~5.4 之和）

### 5.1 `ferro_rt` 运行时库

**工作量：** 3-4 周  
**优先级：** P0  
**依赖：** 1.7, 4.1

| 子任务 | 说明 |
|--------|------|
| `Channel<T>` | `tokio::sync::mpsc` 封装，返回 `(Sender<T>, Receiver<T>)`；`Sender` 可 Clone、`Receiver` 唯一；`ChannelClosed` 错误类型 |
| `shared<T>` | `Arc<Mutex<T>>` 封装；`number` 等优化为原子指令 |
| `spawn` | `std::thread::spawn`（OS 线程）+ `tokio::spawn`（async） |
| `join()` | `tokio::join!` 包装，并发 poll 多个 Future |
| Tokio runtime 初始化 | `#[tokio::main]` 等价物，可经 `Trust.toml` 切换 runtime |
| Feature gate | `default = ["tokio"]` |
| Miri 测试 | `ferro_rt` 的 `unsafe` 块（constraints §3.2） |

### 5.2 `spawn` / `spawn async`

**工作量：** 1 周  
**优先级：** P0  
**依赖：** 5.1, 2.4（闭包调用 #8）。隐式泛型 3.3 非硬依赖——spawn 可先要求闭包参数显式标注

- `spawn` 闭包强制 `move` 检查
- `Send` 自动推导（跨线程能力分析）
- 非 async → `std::thread::spawn`；async → `tokio::spawn`

### 5.3 `shared` + `withLock`

**工作量：** 2 周  
**优先级：** P0  
**依赖：** 5.1

- `shared counter = 0` → `Arc<Mutex<T>>`
- `withLock(c => {...})`：闭包参数 `c` 为 `&mut T`（可变引用，非副本）/只读返回时 `&T`
- `number` shared 原子优化
- 嵌套 `withLock` 死锁警告

### 5.4 `async` / `await` + `join()`

**工作量：** 2 周  
**优先级：** P0  
**依赖：** 5.1

- `async function` / `await` 解析与降级
- 惰性 Future 语义（`.await` 或 `spawn` 才推进）
- `join(a, b)` 并发执行 + 解构返回

---

## Phase 6：标准库 (v0.2 — v0.3)

> **本 Phase 合计：** 约 11.5 周（6.1~6.7 之和）

### 6.1 `std::collections`

**工作量：** 2 周 · P1 · 依赖 3.3
- 动态数组 `T[]`、Map、Set（贴近 JS 的方法名）；与 2.2 整数索引语义协同

### 6.2 `std::string`

**工作量：** 1 周 · P1 · 依赖 3.1
- `split`/`slice`/`replace`/`trim`/`toUpperCase` 等 JS 风格 API

### 6.3 `std::fs`

**工作量：** 1 周 · P1 · 依赖 4.1
- 文件读写、目录遍历、元数据；错误经 `throw`

### 6.4 `std::async` / `std::time` / `std::process`

**工作量：** 1.5 周 · P1 · 依赖 5.4
- `join`/`sleep`/异步 I/O、时间戳/定时器、子进程/环境变量

### 6.5 `std::net`（v0.2）

**工作量：** 2 周 · P2 · 依赖 5.2 + 7.3 的 `extern "rust"` 解析能力（须先行，见下）
- HTTP 客户端、TCP/UDP
- **版本区分（解里程碑前向依赖）：** v0.2 以**手写 `extern` 绑定**提供（标注 `unstable`），仅需 7.3 的 `extern "rust"` 解析基础——该基础须前移到 v0.2；完整 bindgen 自动生成版（7.4）随 v0.3 转 stable

### 6.6 `std::serde`（v0.2）

**工作量：** 2 周 · P2 · 依赖 3.4 + 7.3 的 `extern "rust"` 解析能力（同 6.5）
- JSON 等序列化；与 `unknown` 类型化装载/`match` 协同；基于 Rust serde 封装
- **版本区分：** 同 6.5——v0.2 手写 `extern` 绑定（`unstable`），bindgen 版随 v0.3 转 stable

### 6.7 `std::crypto`（v0.3）

**工作量：** 2 周 · P2 · 依赖 6.6
- SHA-256、BLAKE3、对称/非对称加密原语

---

## Phase 7：开发者工具与 FFI (v0.2+)

> **本 Phase 合计：** 约 23-35 周（7.1~7.6 之和；LSP/工具链跨度大，多为 P2 可并行/延后）

### 7.1 `trust test` — 测试框架

**工作量：** 2 周 · P1 · 依赖 1.7
- `test function` → `#[test]`、`test async function`、`#[should_panic]`
- `#[property]` 属性测试（v0.2+）、`#[concurrent]` 并发压力测试（v0.3+）
- `trust test --filter` / `--threads`；复用 Cargo test 基础设施

### 7.2 Doctest

**工作量：** 1 周 · P1 · 依赖 7.1
- `/// ```trust` 代码块提取，编译并作为测试运行

### 7.3 FFI `extern "rust"`

**工作量：** 2-3 周 · P1 · 依赖 3.3, 4.1
- `extern "rust" { fn ... }` 解析（块内用 `fn` 关键字）
- **所有权规则**（设计 §10）：参数 move 进 Rust 侧、返回值 move 给调用者；`&T` 不直接支持（用 `shared`/`Channel`）；`string`↔`&str`/`String`；`number`↔Rust 整数自动转换；`...args` 可变形参映射
- 错误经 `throws {shape}` 标注映射到 `Result`
- `extern` 块内 `<T>` 表示 Rust 侧泛型（不含 trait bound）

### 7.4 `trust bindgen` — Rust 绑定生成器

**工作量：** 4-6 周 · P1 · 依赖 7.3
- 从 `rustdoc` JSON 生成 Trust 类型声明
- 自动推导参数模式（借用 / move / inout）
- 处理常见 trait、泛型、`Option`/`Result` ↔ `null`/`throw` 映射

### 7.5 Trust LSP + VS Code 扩展

**工作量：** 6-8 周（LSP）+ 2-3 周（扩展） · P2 · 依赖 1.3, 1.6
- 语法高亮、实时诊断、跳转/查找引用、补全、Hover 类型、`--fix` code action

### 7.6 `trust fmt` / `trust doc` / `trust generate`

**工作量：** 各 2-4 周 · P2
- `fmt` 格式化（依赖 1.2）
- `doc` 文档生成（`///` 提取，依赖 1.7）
- `generate` 编译前代码生成（替代 proc macro，进版本控制，依赖 3.2）

---

## Phase 8：生态与发布 (v0.3+)

> **本 Phase 合计：** 约 16+ 周（8.1 自举 12 周 + 8.2/8.3 各 1/3 周；8.4 远期未计）

### 8.1 自举

**依赖：** Phase 1-5

| 子任务 | 内容 | 工作量 | 优先级 |
|--------|------|--------|--------|
| **8.1a 自举子集定义** | 确定优先用 Trust 重写的模块，建立自举测试流水线（Trust 源码 → 阶段1编译器 → 阶段2编译器 → 对比） | 1 周 | P1 |
| **8.1b ferro_rt 自举** | 用 Trust 重写 ferro_rt | 3 周 | P1 |
| **8.1c 编译器前端自举** | 用 Trust 重写 trust_parser + trust_hir | 8 周（远期估计，Phase 5 完成后重新评估） | P2 |

### 8.2 crates.io 发布

**工作量：** 1 周 · P2 · 依赖 Phase 5（核心运行时稳定后再做稳定发布；alpha 发布流水线可在 Phase 2 后提前准备）
- `cargo publish` 各 workspace crate、SemVer 版本同步、CHANGELOG 自动生成

### 8.3 文档站

**工作量：** 3 周 · P2 · 依赖 7.6
- 《Trust 语言之旅》、标准库 API 文档、从 TS/JS 迁移指南、与 Rust 互操作指南

### 8.4 包注册中心（Trust Registry）

**工作量：** 远期 · P2
- Trust 原生 crate 索引、`[trust-dependencies]` 直接引用 Cargo 依赖

---

## 总结：关键里程碑

| 里程碑 | 交付标准 | 状态 |
|--------|---------|------|
| **v0.0 — 规范冻结** | `spec/trust-spec.md` + `spec/stdlib.md` 完成、设计/规范/constraints 一致（旧设计已审计；v2.0 对齐随 Phase 2+ 推进） | ✅ 完成（旧设计） |
| **v0.1 — 最小编译器** | 编译变量/函数/控制流 → Rust → 可执行二进制 | ✅ 完成（旧设计） |
| **v0.1.1 — 对齐 v2.0** | Phase 1 修正到 v2.0 + `trust-spec`/`stdlib` 增量对齐：`number`=f64、移除旧语法残留、块体强制返回标注、`&mut`/闭包调用 | Phase 2 |
| **v0.1.2 — 类型系统与方法** | 具名类型别名、纯结构类型、receiver 方法、隐式泛型、`unknown`+`match` | Phase 3 |
| **v0.1.3 — 错误处理与空安全** | `throw`/`try-catch`（→ `Result` + 穷举推断）、`null` 安全（`?.`/`??`） | Phase 4 |
| **v0.2 — 并发与异步** | `ferro_rt` + `spawn`/`Channel`/`shared`/`withLock`/`join` + `async`/`await`（无 `select`） | Phase 5 |
| **v0.2.x — 标准库** | collections/string/fs/async；net/serde 先以手写 `extern` 绑定提供（`unstable`，需 7.3 `extern` 解析先行），bindgen（stable）版随 v0.3 | Phase 6 |
| **v0.3 — 工具与生态** | test/doctest/FFI/bindgen/LSP/fmt/doc、文档站、自举 | Phase 7-8 |

---

> **下一步：** Phase 2.1 —— 移除旧语法残留、重核关键字表 43，并增量对齐 `spec/trust-spec.md` 与 `spec/stdlib.md`（`phase2-v2-align` 分支）。
