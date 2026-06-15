# 延期与承接总表（Deferred & Handoffs Registry）

> **版本：** v1.0 · **创建：** 2026-06-15  
> **用途：** 汇总各里程碑 spec / TODO / cross-check 中写明「延期」「归 Phase X」「承接」的工作项，避免在后续 Phase 遗漏。  
> **权威关系：** 各子 spec 仍是条目细节来源；本表是**索引与追踪层**，不替代 spec。

---

## 维护规则

1. **新增延期：** 在任何 spec / TODO 中写下「延期 / 归 Phase X / 承接 #N」时，**同步在本表增加一行**（或更新已有行）。
2. **完成闭环：** 目标 Phase 交付时，将状态改为 `✅`，填写 `闭环` 列（PR / commit / spec 验收项 ID），并在源 spec 勾选对应项。
3. **取消 / 废止：** 若设计变更使条目不再适用（如 `bigint` 移除），标 `🚫 废止` 并注明取代方案。
4. **ID 不变：** 一行对应一个可验收能力；勿因措辞调整而改 ID，便于跨文档引用（如 `H-2.2-03`）。
5. **季度抽查：** 每个大 Phase 启动前，对照本表与 `docs/ROADMAP.md` 做一次漏项扫描。

**相关文档：**

| 文档 | 角色 |
|------|------|
| `docs/ROADMAP.md` | Phase 级路线图与交付标准 |
| `docs/phases/*/TODO.md` | 当前 Phase 可执行任务 |
| `docs/phases/*/*-spec.md` | 里程碑验收条件（细节来源） |
| `docs/phases/2/2.1/known-failures.md` | 2.1 禁用测试 → 2.5 恢复清单 |
| `docs/phases/*/cross-check.md` | 规范交叉核对中的 🔜 项 |

---

## 状态图例

| 状态 | 含义 |
|------|------|
| 🔜 待做 | 已登记，目标 Phase 尚未启动或未完成 |
| 🔄 进行中 | 目标 Phase 已启动，条目在当前 sprint |
| ✅ 完成 | 已验收，见 `闭环` 列 |
| 🚫 废止 | 设计变更，不再实现 |
| ⏸ 占位 | 临时实现（如 `DiagError` 占位），正式能力仍待后续 |

---

## 按目标 Phase 索引

### Phase 2（修正 v2.0 — 进行中）

| ID | 能力 / 工作项 | 来源 | 阻塞 / 依赖 | 验收提示 | 状态 |
|----|---------------|------|-------------|----------|------|
| H-P2-01 | **#7 可变引用 `&mut x`** 端到端（parser `RefMut` + TIR borrowck） | 1.7 §8 #7 · `2/TODO.md §2.4.1` · `ROADMAP §2.4` | 2.1 完成 | e2e：`&mut` 合法场景通过、冲突报错 | 🔜 |
| H-P2-02 | **#8 闭包调用 `r()`** 端到端（name_res + `TirFunction.captures`） | 1.7 §8 #8 · `2/TODO.md §2.4.2` · `ROADMAP §2.4` | 2.1 完成 | `let f = (x)=>x+1; f(5)` e2e | 🔜 |
| H-P2-03 | **#10 JSON→serde 迁移评估** + `serde-evaluation.md` | 1.7 §8 #10 · `2/TODO.md §2.4.3` | 无 | 1 页决策文档 | 🔜 |
| H-P2-04 | **块体函数强制返回标注**（无 `:ReturnType` → 编译错误） | `2/TODO.md §2.3.1` · `2.1-spec` 前瞻 | 2.1 | typeck 测试 | ✅ |
| H-P2-05 | **表达式体函数** `function f(...) = expr` | `2/TODO.md §2.3.2` | 2.1 | parser + typeck 测试 | ✅ |
| H-P2-06 | **箭头函数返回类型推断** | `2/TODO.md §2.3.3` | 2.1 | typeck 测试 | ✅ |
| H-P2-07 | **2.5 测试迁移**：56 集成测试 v2.0 语义全绿 | `2/TODO.md §2.5` · `known-failures.md` | 2.1–2.4 | `cargo test --workspace` 零失败；含 2.3 表达式体边界 trustc e2e（§2.5.3） | 🔜 |
| H-P2-08 | **禁用测试恢复/改写**（loop/bigint/bang/try 等） | `2.1/known-failures.md` | 2.5 | 见 known-failures 表逐项 | 🔜 |
| H-P2-09 | **Trust.toml** 解析与 `Cargo.toml` 桥接 | `2/TODO.md §2.6.1` · `ROADMAP` Phase 1 下沉 | 2.1 | `trustc compile --project` | 🔜 |
| H-P2-10 | **CI 性能回归** + `benches/BASELINE.md` | `2/TODO.md §2.6.2` | 2.1 | criterion ±10% | 🔜 |
| H-P2-11 | **Fuzz 语料库** 从夹具初始化 | `2/TODO.md §2.6.3` | 2.1 | fuzz target 可跑 | 🔜 |
| H-P2-12 | **Phase 2 覆盖率基线**记录（非门控） | `2/TODO.md §2.6` | 可选 | `benches/BASELINE.md` 或等价 | 🔜 |
| H-P2-13 | **2^53 正式 `Warning`+`Help` 诊断**（替换 `DiagError` 占位） | `2.2-spec` MS-2.2-5 · `2.2/cross-check.md` · `typeck.rs` 注释 | `trust_error::Diagnostic` 扩展 | 字面量超范围 → `Severity::Warning` + Help 子诊断 | ⏸ |
| H-P2-14 | **spec 残留条目清理**（SYN/TYP 域） | `2.1/known-failures.md` | 2.2 number 重写后 | `trust-spec` grep SYN/TYP 审计项归零 | 🔜 |
| H-P2-15 | **stdlib `Result`/`Option` API 过渡注记**补全 | `2.1-spec` MS-2.1-9 · `known-failures.md` | 2.1 骨架后 | 每个残留 API 有 `Phase 4` 注记 | 🔜 |

### Phase 3（类型系统与方法）

| ID | 能力 / 工作项 | 来源 | 阻塞 / 依赖 | 验收提示 | 状态 |
|----|---------------|------|-------------|----------|------|
| H-P3-01 | **`unknown` 类型与表达式**（关键字已预留） | `2/TODO.md §2.1.1` · `2.1-spec` | Phase 2 完成 | 设计 §2.6 | 🔜 |
| H-P3-02 | **`unknown` + `match` 收窄** | `2/TODO.md` 规范表 · `2.1-spec` | H-P3-01 | typeck + e2e | 🔜 |
| H-P3-03 | **具名类型别名**（名义类型） | `ROADMAP §3` · 冻结矩阵 | Phase 2 | spec 冻结 + 实现 | 🔜 |
| H-P3-04 | **纯结构类型** | `ROADMAP §3` | Phase 2 | 同上 | 🔜 |
| H-P3-05 | **Go 风格 receiver 方法** | `ROADMAP §3` | Phase 2 | 同上 | 🔜 |
| H-P3-06 | **隐式泛型** | `ROADMAP §3` | Phase 2 | 同上 | 🔜 |
| H-P3-07a | **箭头参数从上下文推断** `(name) => expr` | 设计 §4.1 · `2.3-spec` MS-2.3-4 | H-P3-06 | `let greet = (name) => \`Hi ${name}\`` 无参标注 | 🔜 |
| H-P3-07b | **`test function` 块体返回标注规则** | `2.3-spec` §0 · 设计 §11 | parser `test function` | `test function f(): void { ... }` — 经同一 `lower_function` 路径继承 2.3 规则 | 🔜 |
| H-P3-07c | **嵌套函数声明** | `2.3-spec` §0 · `name_res.rs:289-296` | 类型系统扩展 | 当前 `lower_hir_stmt` 拒绝嵌套函数，Phase 3 启用后继承块体标注规则 | 🔜 |
| H-P3-07d | **`HirType::Error` → `Infer`/`Error` 拆分** | `2.3-spec` Step 3 技术债务 · `typeck.rs:76` | — | 消除"需推断"与"类型失败"双语义重载 | 🔜 |
| H-P3-07e | **spec SEM-REQ-003 interface 旧示例清理** | `2.3-spec` MS-2.3-7 · `cross-check.md` | — | `trust-spec` 全文中 interface 名义类型等 pre-v2.0 示例残留清理 | 🔜 |
| H-P3-07 | **`i++` / `+=` 等更新表达式语法** | `2/TODO.md §2.2.4` · `2.2-spec` Step 4 | Phase 2 number=f64 | parser + typeck；循环可用 `i++` | 🔜 |
| H-P3-08 | **#9 跨函数 `inout` 对称检查**（`inout this` 方法） | 1.7 §8 #9 · `ROADMAP §3` | receiver 方法 (H-P3-05) | borrowck 跨函数场景 | 🔜 |
| H-P3-09 | **#11 修复建议扩展**（3→≥8 规则） | 1.7 §8 #11 · `ROADMAP §4`（1.7 原定 Phase 3.2，ROADMAP 移至 Phase 4） | Phase 4 错误/`null` 落地后 | 规则数 + e2e 诊断 JSON | 🔜 |
| H-P3-10 | **match 装载形状不符** 占位 `panic!` → 正式 `throw` | `ROADMAP §3.4` · `2.2-spec Step 6` | Phase 4 `throw` | 见 H-P4-01 | 🔜 |

**规范写入（实现随 Phase 3 子任务）：** 具名类型、receiver、隐式泛型、`unknown`+`match` — `2.1-spec`「不在 2.1 写入」清单。

### Phase 4（错误处理与空安全）

| ID | 能力 / 工作项 | 来源 | 阻塞 / 依赖 | 验收提示 | 状态 |
|----|---------------|------|-------------|----------|------|
| H-P4-01 | **`try` / `catch` 语句**（关键字已预留） | `2/TODO.md §2.1.1` · `2.1-spec` | Phase 3 类型基础 | 设计 §5.1 | 🔜 |
| H-P4-02 | **`panic` 表达式** `panic!("msg")` | `2/TODO.md §2.1.1` | Phase 3 | 设计 §5.2 | 🔜 |
| H-P4-03 | **`throw` / 穷举错误推断** | `ROADMAP §4` · 冻结矩阵 | Phase 3 | spec ERR 章节冻结 | 🔜 |
| H-P4-04 | **完整 `null` 安全**（收窄、`?.`/`??` 完整语义） | `2.1-spec` · `ROADMAP §4` | Phase 3 | 设计 §2.7 | 🔜 |
| H-P4-05 | **stdlib API：`Result<T,E>` → `throws` 或 `T\|null`** | `stdlib.md` header · `2.1-spec` MS-2.1-9 | H-P4-03 | `fs.readToString` 等签名改写 | 🔜 |
| H-P4-06 | **stdlib 完整 API 重写**（相对 2.1 骨架） | `2.1-spec` MS-2.1-9 | H-P4-03/05 | stdlib 无过渡注记残留 | 🔜 |
| H-P4-07 | **`spec/trust-spec` 错误/`null` 章节冻结** + ERR/SEM 残留清理 | 冻结矩阵 · `2.1/known-failures.md` | H-P4-01–04 | cross-check 记录 + grep 审计项归零 | 🔜 |

### Phase 5（并发与异步）

| ID | 能力 / 工作项 | 来源 | 阻塞 / 依赖 | 验收提示 | 状态 |
|----|---------------|------|-------------|----------|------|
| H-P5-00 | **`async function` 块体返回标注规则** | `2.3-spec` §0 · 设计 §8 | parser `async function` | `async function f(): void { ... }` — 经同一 `lower_function` 路径继承 2.3 规则 | 🔜 |
| H-P5-01 | **`async` / `await` 运行时语义** | `ROADMAP §5` · 冻结矩阵 | Phase 4 | ferro_rt + e2e | 🔜 |
| H-P5-02 | **`spawn` / `Channel` / `shared` / `join` 等** | `ROADMAP §5` | Phase 4 | stdlib `sync` | 🔜 |
| H-P5-03 | **并发相关 stdlib 模块冻结** | 冻结矩阵 · `ROADMAP` | H-P5-01 | stdlib cross-check | 🔜 |

### Phase 6（标准库 / 集合类型）

| ID | 能力 / 工作项 | 来源 | 阻塞 / 依赖 | 验收提示 | 状态 |
|----|---------------|------|-------------|----------|------|
| H-P6-01 | **数组/切片索引语法 `arr[n]`** + codegen `as usize` | `2.2-spec` MS-2.2-5 · `2/TODO.md §2.2.4` | 集合类型 AST/parser | e2e 生成 Rust 含 `as usize` | 🔜 |
| H-P6-02 | **`.length` / 容量** → `number`(f64) | `2.2-spec` · `2.2/cross-check.md` | H-P6-01 · MemberAccess 扩展 | codegen `usize`→`f64` | 🔜 |
| H-P6-03 | **索引非整数 `Warning`** | `2.2-spec` Step 4 | H-P6-01 · Warning API (H-P2-13) | 非整数索引 warning | 🔜 |
| H-P6-04 | **整数语义 e2e**（索引 + length） | `2.2-spec` Step 6 | H-P6-01–03 | 2.2.4 验证项闭环 | 🔜 |
| H-P6-05 | **stdlib 索引/容量 number API 实现** | `2.2/cross-check.md` · `stdlib.md` | `std::collections` | API 签名与 f64 一致 | 🔜 |
| H-P6-06 | **`std::collections` 等模块落地** | `ROADMAP §6` | Phase 5 部分 | 模块级 cross-check | 🔜 |

### Phase 7（工具与 FFI）

| ID | 能力 / 工作项 | 来源 | 阻塞 / 依赖 | 验收提示 | 状态 |
|----|---------------|------|-------------|----------|------|
| H-P7-01 | **FFI `number` ↔ Rust `i64`/`u64` 跨边界转换** | `2.2-spec` MS-2.2-5 · `2/TODO.md §2.2.4` | extern 绑定 (7.3) | FFI e2e | 🔜 |
| H-P7-02 | **LSP / fmt / doc / bindgen** 等工具链 | `ROADMAP §7` | Phase 5–6 | 各 7.x 子 spec | 🔜 |
| H-P7-03 | **并发/FFI 规范章节冻结** | 冻结矩阵 | H-P7-01 | cross-check | 🔜 |

### Phase 8（生态）

| ID | 能力 / 工作项 | 来源 | 阻塞 / 依赖 | 验收提示 | 状态 |
|----|---------------|------|-------------|----------|------|
| H-P8-01 | **`[trust-dependencies]`**（Trust.toml 远期节） | `2/TODO.md §2.6.1` | 包生态 | 仅占位→实现 | 🔜 |
| H-P8-02 | **编译器前端自举** | `ROADMAP §8` | Phase 5 稳定 | 评估文档 | 🔜 |

### 持续 / 跨 Phase

| ID | 能力 / 工作项 | 来源 | 阻塞 / 依赖 | 验收提示 | 状态 |
|----|---------------|------|-------------|----------|------|
| H-X-01 | **#12 各 crate 错误类型迁移至 `trust_error::Diagnostic`** | 1.7 §8 #12 · `ROADMAP` | 每子阶段一批 | parser/HIR/TIR/codegen 无 `DiagError` 孤岛 | 🔄 |
| H-X-02 | **`std::...` 模块路径解析**（`1.2-spec` 路径规则） | `1.2-spec` | Phase 2+ 逐步 | import 路径 e2e | 🔜 |
| H-X-03 | **覆盖率 tarpaulin ≥70%**（非阻塞差距记录） | `2/TODO.md` Phase 2 交付 | 持续 | CI 或差距文档 | 🔜 |

### 已废止 / 已由其他项取代

| ID | 原工作项 | 原因 | 取代 |
|----|----------|------|------|
| H-VOID-01 | **bigint 字面量**端到端 (#1) | v2.0 移除 bigint；2.1 已删类型 | `number`(f64) + 2.5 改写测试 |
| H-VOID-02 | **loop + break 值** (#4–5) | v2.0 移除 `loop` | `while` + `break`（无值） |
| H-VOID-03 | **Phase 1 下沉 F2–F5 控制流测试** | 1.8 已闭环；F2–F3（for/while）→ 2.5 改写夹具；F4–F5（loop）→ 废止 | 2.5 夹具迁移 |

---

## 按来源 spec 反查

| 来源文档 | 本表 ID（摘录） |
|----------|----------------|
| `1.7/1.7-spec.md §8` | H-P2-01–03, H-P3-08–09, H-X-01, H-VOID-01–03 |
| `2/2.1/2.1-spec.md` | H-P3-01–06, H-P4-04–07, H-P2-14–15 |
| `2/2.2/2.2-spec.md` | H-P6-01–04, H-P2-13, H-P3-07, H-P3-10, H-P7-01 |
| `2/2.2/cross-check.md` | H-P6-05, H-P2-13 |
| `2/2.3/cross-check.md` | H-P3-07e（interface 清理） |
| `2/2.1/known-failures.md` | H-P2-07–08, H-P2-14–15 |
| `2/2.3/2.3-spec.md` | H-P2-04–06, H-P3-07a–07e, H-P5-00 |
| `2/TODO.md` | H-P2-01–15, H-P3-07, H-P6-01–02 |
| `spec/stdlib.md` | H-P4-05–06, H-P6-05 |
| `docs/ROADMAP.md` | 各 Phase 章节 ↔ 上表同 Phase 段 |

---

## 里程碑闭环检查清单（模板）

目标 Phase 启动会前，复制下列项并勾选：

```markdown
- [ ] 扫描本表「目标 Phase」段：全部 🔜 已纳入当期 TODO/spec
- [ ] 扫描上一 Phase 段：无遗漏的 🔜 未解释延期
- [ ] 更新 `cross-check.md` 中对应 🔜 → ✅
- [ ] 在 ROADMAP 交付标准中勾选对应项
- [ ] 废止项已标 🚫，无僵尸 🔜
```

---

> **变更记录**
> - v1.0 (2026-06-15)：初版，汇总 Phase 1.7 承接项 + Phase 2.1/2.2 spec 延期项 + ROADMAP 冻结矩阵。
