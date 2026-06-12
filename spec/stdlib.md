# Trust 标准库 API 规范 v0.0

> 版本：v0.0-draft · 对齐 `spec/trust-spec.md` · 对齐 `docs/Trust-设计文档.md §10`  
> 本文档定义 Trust 标准库每个模块的完整 API 签名、语义描述和 Rust 实现映射。

---

## 模块依赖图

```
                    ┌─────────┐
                    │ result  │  (Option / Result — 所有其他模块的基础)
                    └────┬────┘
                         │
         ┌───────────────┼───────────────┐
         │               │               │
    ┌────▼────┐    ┌─────▼──────┐   ┌───▼────┐
    │ string  │    │ collections│   │  fs    │
    └─────────┘    └──┬─────┬───┘   └────────┘
                      │     │
                 ┌────▼──┐  │
                 │  rc   │◄─┘  (rc 可使用 collections)
                 └───┬───┘
                     │
                ┌────▼────┐
                │  sync   │  (依赖 rc 的 Arc + collections 的 VecDeque)
                └────┬────┘
                     │
                ┌────▼────┐
                │  async  │  (依赖 sync 的 spawn)
                └────┬────┘
                     │
    ┌────────────────┼────────────────┐
    │                │                │
┌───▼───┐    ┌──────▼──────┐   ┌─────▼─────┐
│  net  │    │   process   │   │   time    │
└───────┘    └─────────────┘   └───────────┘
    │
┌───▼────┐
│ crypto │
└────────┘

serde —— 独立模块
```

---

## std::result — 可空与可恢复错误

> **说明：** `Option<T>` 和 `Result<T,E>` 是语言核心类型（由 `trust-spec.md` ERR-REQ-001 定义）。本模块只定义它们的方法。

### API

| 函数 | 签名 | 语义 | Rust 映射 |
|------|------|------|----------|
| 构造 | | | |
| `Option::Some` | `Some<T>(value: T) -> Option<T>` | 创建有值变体 | `Some(value)` |
| `Option::None` | `None -> Option<T>` | 创建空变体 | `None` |
| `Result::Ok` | `Ok<T,E>(value: T) -> Result<T,E>` | 创建成功变体 | `Ok(value)` |
| `Result::Err` | `Err<T,E>(error: E) -> Result<T,E>` | 创建错误变体 | `Err(error)` |
| 检查 | | | |
| `isSome` | `opt.isSome(): boolean` | 是否为 Some | `opt.is_some()` |
| `isNone` | `opt.isNone(): boolean` | 是否为 None | `opt.is_none()` |
| `isOk` | `result.isOk(): boolean` | 是否为 Ok | `result.is_ok()` |
| `isErr` | `result.isErr(): boolean` | 是否为 Err | `result.is_err()` |
| 解包 | | | |
| `unwrap` | `opt.unwrap(): T` | 解包，None 时 panic | `opt.unwrap()` |
| `unwrap` | `result.unwrap(): T` | 解包，Err 时 panic | `result.unwrap()` |
| `unwrapOr` | `opt.unwrapOr(default: T): T` | 解包或默认值 | `opt.unwrap_or(default)` |
| `unwrapOr` | `result.unwrapOr(default: T): T` | 解包或默认值 | `result.unwrap_or(default)` |
| `expect` | `opt.expect(msg: string): T` | 解包，None 时 panic 携带 msg | `opt.expect(msg)` |
| `expect` | `result.expect(msg: string): T` | 解包，Err 时 panic 携带 msg | `result.expect(msg)` |
| 映射 | | | |
| `map` | `opt.map<U>(f: (T) => U): Option<U>` | Some 时应用 f | `opt.map(f)` |
| `map` | `result.map<U>(f: (T) => U): Result<U,E>` | Ok 时应用 f | `result.map(f)` |
| `andThen` | `opt.andThen<U>(f: (T) => Option<U>): Option<U>` | Some 时链式调用 | `opt.and_then(f)` |
| `andThen` | `result.andThen<U>(f: (T) => Result<U,E>): Result<U,E>` | Ok 时链式调用 | `result.and_then(f)` |

### 设计决策

**`!` 仅限 Option：** `!` 断言"我知道这里有值"，仅允许用于 `Option<T>`。`Result<T,E>` 的错误不可控（I/O 失败）——允许 `Result!` 将训练开发者忽略错误。Trust 选择显式处理：`Result` 用 `?` 传播或用 `.expect()` 断言。

**`??` 同时用于 Option 和 Result：** `??` 映射 `unwrapOr`，不涉及 panic。用于 `Result` 时静默丢弃错误信息——适合错误可忽略的场景。

### 验收标准

- [ ] `Some(42).isSome()` → `true`
- [ ] `None.isNone()` → `true`
- [ ] `Ok(42).unwrap()` → `42`
- [ ] `None.unwrap()` → panic
- [ ] `None.unwrapOr(0)` → `0`
- [ ] `Err("fail").unwrapOr(0)` → `0`
- [ ] `Some(42).map(x => x * 2)` → `Some(84)`
- [ ] `None.map(x => x * 2)` → `None`

---

## std::collections — 集合类型

### 类型

| 类型 | 说明 | Rust 映射 |
|------|------|----------|
| `Vec<T>` | 动态数组 | `Vec<T>` |
| `HashMap<K,V>` | 哈希表 | `HashMap<K,V>` |
| `HashSet<T>` | 哈希集合 | `HashSet<T>` |
| `VecDeque<T>` | 双端队列 | `VecDeque<T>` |

### API — Vec<T>

| 函数 | 签名 | 语义 | Rust 映射 |
|------|------|------|----------|
| `Vec::new` | `Vec::new<T>() -> Vec<T>` | 创建空 Vec | `Vec::new()` |
| `Vec::withCapacity` | `Vec::withCapacity<T>(cap: number) -> Vec<T>` | 预分配容量 | `Vec::with_capacity(cap)` |
| `len` | `vec.len(): number` | 元素个数 | `vec.len()` |
| `isEmpty` | `vec.isEmpty(): boolean` | 是否为空 | `vec.is_empty()` |
| `push` | `(inout vec).push(value: T)` | 末尾追加 | `vec.push(value)` |
| `pop` | `(inout vec).pop(): Option<T>` | 移除并返回末尾元素 | `vec.pop()` |
| `get` | `vec.get(index: number): Option<T>` | 按索引获取 | `vec.get(index).cloned()` |
| `first` | `vec.first(): Option<T>` | 首元素 | `vec.first().cloned()` |
| `last` | `vec.last(): Option<T>` | 末元素 | `vec.last().cloned()` |
| `insert` | `(inout vec).insert(index: number, value: T)` | 在 index 处插入 | `vec.insert(index, value)` |
| `remove` | `(inout vec).remove(index: number): T` | 移除 index 处元素 | `vec.remove(index)` |
| `clear` | `(inout vec).clear()` | 清空 | `vec.clear()` |
| `slice` | `vec.slice(start: number, end?: number): Vec<T>` | 切片 | `vec[start..end].to_vec()` |

### API — HashMap<K,V>

| 函数 | 签名 | 语义 | Rust 映射 |
|------|------|------|----------|
| `HashMap::new` | `HashMap::new<K,V>() -> HashMap<K,V>` | 创建空 HashMap | `HashMap::new()` |
| `insert` | `(inout map).insert(key: K, value: V): Option<V>` | 插入键值对，返回旧值 | `map.insert(k, v)` |
| `get` | `map.get(key: K): Option<V>` | 按键获取 | `map.get(&k).cloned()` |
| `remove` | `(inout map).remove(key: K): Option<V>` | 移除并返回值 | `map.remove(&k)` |
| `contains` | `map.contains(key: K): boolean` | 是否包含键 | `map.contains_key(&k)` |
| `len` | `map.len(): number` | 键值对数量 | `map.len()` |

### API — HashSet<T>

| 函数 | 签名 | 语义 | Rust 映射 |
|------|------|------|----------|
| `HashSet::new` | `HashSet::new<T>() -> HashSet<T>` | 创建空 HashSet | `HashSet::new()` |
| `add` | `(inout set).add(value: T): boolean` | 添加，返回是否新增 | `set.insert(value)` |
| `contains` | `set.contains(value: T): boolean` | 是否包含 | `set.contains(&value)` |
| `remove` | `(inout set).remove(value: T): boolean` | 移除，返回是否存在 | `set.remove(&value)` |
| `len` | `set.len(): number` | 元素个数 | `set.len()` |

### API — VecDeque<T>

| 函数 | 签名 | 语义 | Rust 映射 |
|------|------|------|----------|
| `VecDeque::new` | `VecDeque::new<T>() -> VecDeque<T>` | 创建空队列 | `VecDeque::new()` |
| `pushFront` | `(inout dq).pushFront(value: T)` | 队首插入 | `d.push_front(value)` |
| `pushBack` | `(inout dq).pushBack(value: T)` | 队尾插入 | `d.push_back(value)` |
| `popFront` | `(inout dq).popFront(): Option<T>` | 队首弹出 | `d.pop_front()` |
| `popBack` | `(inout dq).popBack(): Option<T>` | 队尾弹出 | `d.pop_back()` |

### 设计决策

**迭代器链（map/filter/reduce）不属于 Vec 方法：** Trust 采用迭代器模式——`vec.iter().map(x => x * 2).filter(x => x > 0).reduce((a,b) => a + b, 0)`。迭代器是独立 trait，不在 collections 模块中定义。

### 验收标准

- [ ] `let v = Vec::new<number>(); v.push(1); v.len()` → `1`
- [ ] `v.pop()` → `Some(1)`
- [ ] `v.pop()` → `None`
- [ ] `v.get(0)` → `None`（不 panic，返回 Option）
- [ ] `let m = HashMap::new<string, number>(); m.insert("a", 1); m.get("a")` → `Some(1)`
- [ ] `m.get("b")` → `None`
- [ ] `let s = HashSet::new<number>(); s.add(1); s.add(1); s.len()` → `1`
- [ ] `let dq = VecDeque::new<number>(); dq.pushBack(1); dq.pushFront(2); dq.popFront()` → `Some(2)`

---

## std::string — UTF-8 字符串

### API

| 函数 | 签名 | 语义 | Rust 映射 |
|------|------|------|----------|
| `length` | `s.length: number` | 字节长度 | `s.len()` |
| `split` | `s.split(separator: string): Vec<string>` | 分割 | `s.split(sep).collect()` |
| `slice` | `s.slice(start: number, end?: number): string` | 切片 | `s[start..end].to_string()` |
| `replace` | `s.replace(from: string, to: string): string` | 替换 | `s.replace(from, to)` |
| `trim` | `s.trim(): string` | 去首尾空白 | `s.trim().to_string()` |
| `toUpperCase` | `s.toUpperCase(): string` | 转大写 | `s.to_uppercase()` |
| `toLowerCase` | `s.toLowerCase(): string` | 转小写 | `s.to_lowercase()` |
| `startsWith` | `s.startsWith(prefix: string): boolean` | 以 prefix 开头 | `s.starts_with(prefix)` |
| `endsWith` | `s.endsWith(suffix: string): boolean` | 以 suffix 结尾 | `s.ends_with(suffix)` |
| `includes` | `s.includes(sub: string): boolean` | 包含子串 | `s.contains(sub)` |

### 验收标准

- [ ] `"hello".length` → `5`
- [ ] `"a,b,c".split(",")` → `["a","b","c"]`
- [ ] `"  hi  ".trim()` → `"hi"`
- [ ] `"hello".toUpperCase()` → `"HELLO"`
- [ ] `"hello".startsWith("he")` → `true`
- [ ] `"".length` → `0`

---

## std::fs — 文件系统

### API

| 函数 | 签名 | 语义 | Rust 映射 |
|------|------|------|----------|
| `readToString` | `fs.readToString(path: string): Result<string, FsError>` | 读取文件全部内容 | `std::fs::read_to_string(path)` |
| `writeString` | `fs.writeString(path: string, content: string): Result<void, FsError>` | 写入字符串（覆盖） | `std::fs::write(path, content)` |
| `exists` | `fs.exists(path: string): boolean` | 文件是否存在 | `path.exists()` |
| `remove` | `fs.remove(path: string): Result<void, FsError>` | 删除文件 | `std::fs::remove_file(path)` |
| `readDir` | `fs.readDir(path: string): Result<Vec<string>, FsError>` | 列出目录条目 | `std::fs::read_dir(path)` |
| `metadata` | `fs.metadata(path: string): Result<FsMetadata, FsError>` | 文件元数据 | `std::fs::metadata(path)` |

### 类型

```
type FsError =
    | { kind: "NotFound"; path: string }
    | { kind: "PermissionDenied"; path: string }
    | { kind: "AlreadyExists"; path: string }
    | { kind: "IoError"; message: string }
```

### 验收标准

- [ ] `fs.readToString("missing.txt")` → `Err(FsError.NotFound)`
- [ ] `fs.writeString("test.txt", "hello")` → `Ok(())`
- [ ] `fs.readToString("test.txt")` → `Ok("hello")`
- [ ] `fs.exists("test.txt")` → `true`
- [ ] `fs.remove("test.txt")` → `Ok(())`
- [ ] `fs.exists("test.txt")` → `false`

---

## std::rc — 智能指针

### API

| 函数 | 签名 | 语义 | Rust 映射 |
|------|------|------|----------|
| **Box<T>** | | | |
| `Box::new` | `Box::new<T>(value: T) -> Box<T>` | 堆分配 | `Box::new(value)` |
| `intoInner` | `(move box).intoInner(): T` | 取出值，释放 Box | `*box` |
| **Rc<T>** | | | |
| `Rc::new` | `Rc::new<T>(value: T) -> Rc<T>` | 创建引用计数智能指针 | `Rc::new(value)` |
| `clone` | `rc.clone(): Rc<T>` | 引用计数 +1 | `Rc::clone(&rc)` |
| `strongCount` | `rc.strongCount(): number` | 强引用计数 | `Rc::strong_count(&rc)` |
| `weakCount` | `rc.weakCount(): number` | 弱引用计数 | `Rc::weak_count(&rc)` |
| **Arc<T>** | | | |
| `Arc::new` | `Arc::new<T>(value: T) -> Arc<T>` | 创建原子引用计数智能指针 | `Arc::new(value)` |
| `clone` | `arc.clone(): Arc<T>` | 原子引用计数 +1 | `Arc::clone(&arc)` |
| `strongCount` | `arc.strongCount(): number` | 强引用计数 | `Arc::strong_count(&arc)` |
| **Weak<T>** | | | |
| `downgrade` | `rc.downgrade(): Weak<T>` | 创建弱引用 | `Rc::downgrade(&rc)` |
| `upgrade` | `weak.upgrade(): Option<Rc<T>>` | 升级为强引用 | `weak.upgrade()` |

### 设计决策

**`Rc` 不实现 `Send`：** `Rc<T>` 使用非原子引用计数，跨线程使用导致数据竞争。Trust 编译器在 `spawn` 中使用 `Rc` 时报错——提示改用 `Arc`。

### 验收标准

- [ ] `let b = Box::new(42); *b` → `42`
- [ ] `let b = Box::new(42); let v = b.intoInner();` → `v == 42`，`b` 已 move 不可再用
- [ ] `let a = Rc::new([1,2,3]); let b = a.clone(); Rc::strongCount(a)` → `2`
- [ ] `let a = Arc::new([1,2,3]); let b = a.clone(); Arc::strongCount(a)` → `2`
- [ ] `let w = Rc::downgrade(a); w.upgrade()` → `Some`
- [ ] `Rc::new(1)` 传入 `spawn` → 编译错误（非 Send）
- [ ] `Arc::new(1)` 传入 `spawn` → 编译通过

---

## std::sync — 并发原语

### API — Channel / Sender / Receiver

| 函数 | 签名 | 语义 | Rust 映射 |
|------|------|------|----------|
| `Channel` | `Channel<T>(capacity?: number) -> (Sender<T>, Receiver<T>)` | 创建有界通道，默认容量 64 | `tokio::sync::mpsc::channel(cap)` |
| `send` | `(inout tx).send(value: T): Result<void, ChannelError>` | 发送消息，所有权转移 | `tx.send(value).await` |
| `receive` | `rx.receive(): Result<T, ChannelError>` | 接收消息，所有权取出 | `rx.recv().await` |
| `receiveTimeout` | `rx.receiveTimeout(ms: number): Result<T, ChannelError>` | 带超时接收 | `tokio::time::timeout` + `rx.recv()` |
| `close` | `(inout tx).close()` | 关闭通道 | `drop(tx)` 或显式 close |
| `clone` | `tx.clone(): Sender<T>` | 克隆发送端 | `tx.clone()` |

### API — Mutex / RwLock

| 函数 | 签名 | 语义 | Rust 映射 |
|------|------|------|----------|
| `Mutex::new` | `Mutex::new<T>(value: T) -> Mutex<T>` | 创建互斥锁 | `Mutex::new(value)` |
| `lock` | `mutex.lock(f: (inout T) => void)` | 获取锁并执行闭包 | `mutex.lock().unwrap()` |
| `RwLock::new` | `RwLock::new<T>(value: T) -> RwLock<T>` | 创建读写锁 | `RwLock::new(value)` |
| `read` | `rwLock.read(f: (T) => void)` | 读锁定 | `rwlock.read().unwrap()` |
| `write` | `rwLock.write(f: (inout T) => void)` | 写锁定 | `rwlock.write().unwrap()` |

### API — Atomic / spawn / shared

| 函数 | 签名 | 语义 | Rust 映射 |
|------|------|------|----------|
| `AtomicI32::new` | `AtomicI32::new(value: number) -> AtomicI32` | 创建原子整数 | `AtomicI32::new(value)` |
| `load` | `atomic.load(): number` | 原子读取 | `atomic.load(Ordering::Relaxed)` |
| `store` | `(inout atomic).store(value: number)` | 原子写入 | `atomic.store(value, Ordering::Relaxed)` |
| `fetchAdd` | `(inout atomic).fetchAdd(delta: number): number` | 原子加，返回旧值 | `atomic.fetch_add(delta, Ordering::Relaxed)` |
| `fetchSub` | `(inout atomic).fetchSub(delta: number): number` | 原子减，返回旧值 | `atomic.fetch_sub(delta, Ordering::Relaxed)` |
| `spawn` | `spawn(f: move () => void)` | 语言关键字 `spawn` 的线程封装 | `std::thread::spawn(f)` |
| `shared` | `shared x = init` | 语言关键字，编译为 `Arc<AtomicI32>` 或 `Arc<Mutex<T>>` | 编译器内置 |

### 类型
```
type ChannelError =
    | { kind: "Closed" }
    | { kind: "Timeout" }
```

### 设计决策

**Channel 分离为 Sender/Receiver：** `Channel<T>()` 返回 `(tx, rx)` 而非单个 Channel 对象。`spawn(move || tx.send())` 后 tx 被 move，rx 仍可用于另一个 spawn。`Sender: Clone` 支持多个发送方。

**`select` 分支内不写 `await`：** `select { case msg = rx.receive() => ... }` 分支内隐式 poll。此规则在语法层面由编译器检查——写了 `await` 为编译错误。

### 验收标准

- [ ] `let (tx, rx) = Channel<number>(64)` → 返回元组
- [ ] `tx.send(42)` → `Ok(())`
- [ ] `rx.receive()` → `Ok(42)`
- [ ] `tx.close(); rx.receive()` → `Err(ChannelError.Closed)`
- [ ] `rx.receiveTimeout(100)` 超时 → `Err(ChannelError.Timeout)`
- [ ] `select { case m = rx.receive() => ... }` 分支内无 `await` → 合法
- [ ] `select { case m = await rx.receive() => ... }` → 编译错误
- [ ] `let m = Mutex::new(0); m.lock(c => { *c += 1; });` → 互斥访问
- [ ] `let a = AtomicI32::new(0); a.fetchAdd(1)` → `0`（返回旧值），`a.load()` → `1`
- [ ] `spawn(move () => { doWork(); })` → 编译通过

---

## std::async — 异步运行时

### API

| 函数 | 签名 | 语义 | Rust 映射 |
|------|------|------|----------|
| `join` | `join<T1,T2>(f1: Future<T1>, f2: Future<T2>): Result<(T1,T2), JoinError>` | 并发等待两个 Future | `ferro_rt::join(f1, f2)` → 内部 `tokio::join!` |
| `sleep` | `sleep(ms: number): Future<void>` | 异步休眠 | `tokio::time::sleep(Duration::from_millis(ms))` |
| `spawn` | `spawn(move async () => T): JoinHandle<T>` | 异步任务（语言关键字 `spawn async` 封装） | `tokio::spawn(async { ... })` |

### 设计决策

**惰性 Future 模型：** Trust 的 `async function` 返回惰性 Future——调用时不执行，仅 `.await` 或 `spawn` 时由 executor poll。这是 Rust 编译目标的物理约束（详见 `trust-spec.md` CON-REQ-001）。`join()` 是真正并发的语法糖——同时 poll 两个 Future。

### 验收标准

- [ ] `async function f() { return 42; }; let result = await f();` → `42`
- [ ] `let (a, b) = await join(fetchUser(), fetchConfig())?;` → 两个操作并发执行
- [ ] `await sleep(100)` → 约 100ms 后继续
- [ ] `let h = spawn(move async () => { await work(); return 1; }); await h` → `1`
- [ ] `let f1 = fetch(); let f2 = fetch(); await f1; await f2;` → 串行执行（惰性 Future 特性）
- [ ] 编译器 lint 检测串行 await 模式 → 提示使用 `join()`

---

## std::time — 时间与时序

### API

| 函数 | 签名 | 语义 | Rust 映射 |
|------|------|------|----------|
| `now` | `time.now(): number` | Unix 时间戳（毫秒） | `SystemTime::now().duration_since(UNIX_EPOCH)` |
| `sleep` | `time.sleep(ms: number): Future<void>` | 同步睡眠 | `std::thread::sleep` |
| `Duration` | `Duration { ms: number }` | 时间段结构体 | `Duration::from_millis(ms)` |
| `elapsed` | `start.elapsed(): Duration` | 从 start 开始的经过时间 | `start.elapsed()` |

### 验收标准

- [ ] `time.now()` → 非负整数
- [ ] `time.sleep(10)` 同步阻塞约 10ms
- [ ] `let d = Duration { ms: 5000 }; d.ms` → `5000`

---

## std::process — 子进程管理（v0.2）

### API

| 函数 | 签名 | 语义 | Rust 映射 |
|------|------|------|----------|
| `run` | `process.run(cmd: string, args?: string[]): Result<ProcessOutput, ProcessError>` | 运行命令并等待完成 | `Command::new(cmd).args(args).output()` |
| `spawn` | `process.spawn(cmd: string, args?: string[]): Result<ProcessHandle, ProcessError>` | 启动进程（不等待） | `Command::new(cmd).args(args).spawn()` |
| `env.get` | `process.env.get(key: string): Option<string>` | 读取环境变量 | `std::env::var(key)` |
| `env.set` | `process.env.set(key: string, value: string)` | 设置环境变量 | `std::env::set_var(key, value)` |

### 验收标准

- [ ] `process.run("echo", ["hello"])` → `Ok(ProcessOutput { stdout: "hello\n", ... })`
- [ ] `process.env.get("PATH")` → `Some(...)`

---

## std::net — 网络（v0.2，手写绑定）

> v0.2 采用手写 `extern` 绑定，v0.2.1 迁移至 `trust bindgen` 自动生成。

### API

| 函数 | 签名 | 语义 | Rust 映射 |
|------|------|------|----------|
| **HTTP 客户端** | | | |
| `http.get` | `http.get(url: string): Future<Result<HttpResponse, NetError>>` | HTTP GET 请求 | `reqwest::get(url).await` |
| `http.post` | `http.post(url: string, body: string): Future<Result<HttpResponse, NetError>>` | HTTP POST 请求 | `reqwest::Client::new().post(url).body(body).send().await` |
| **TCP** | | | |
| `TcpListener::bind` | `TcpListener::bind(addr: string): Result<TcpListener, NetError>` | 绑定 TCP 端口 | `TcpListener::bind(addr)` |
| `accept` | `listener.accept(): Future<Result<TcpStream, NetError>>` | 接受连接 | `listener.accept().await` |
| `TcpStream::connect` | `TcpStream::connect(addr: string): Future<Result<TcpStream, NetError>>` | 连接远程 | `TcpStream::connect(addr).await` |
| `read` | `stream.read(buf: inout Vec<number>): Future<Result<number, NetError>>` | 读取数据 | `stream.read(&mut buf).await` |
| `write` | `stream.write(data: number[]): Future<Result<number, NetError>>` | 写入数据 | `stream.write(&data).await` |
| **TLS** | | | |
| `TlsStream::connect` | `TlsStream::connect(addr: string): Future<Result<TlsStream, NetError>>` | TLS 连接 | `tokio_native_tls::TlsStream` |

### 验收标准

- [ ] `http.get("https://example.com")` → `Ok(HttpResponse { status: 200, ... })`
- [ ] `TcpListener::bind("127.0.0.1:8080")` → 成功绑定
- [ ] `TcpStream::connect("127.0.0.1:8080")` → 成功连接

---

## std::serde — 序列化（v0.2，手写绑定）

> v0.2 采用手写 `extern` 绑定，v0.2.1 迁移至 `trust bindgen` 自动生成。

### API

| 函数 | 签名 | 语义 | Rust 映射 |
|------|------|------|----------|
| `fromStr` | `serde.fromStr<T>(s: string): Result<T, SerdeError>` | JSON 反序列化 | `serde_json::from_str(s)` |
| `toPrettyString` | `serde.toPrettyString<T>(value: T): Result<string, SerdeError>` | JSON 格式化序列化 | `serde_json::to_string_pretty(value)` |

### 验收标准

- [ ] `serde.fromStr<Point>(`{"x":1,"y":2}`)` → `Ok(Point { x: 1, y: 2 })`
- [ ] `serde.toPrettyString(Point { x: 1, y: 2 })` → `Ok("{\n  \"x\": 1,\n  \"y\": 2\n}")`

---

## std::crypto — 加密原语（v0.3）

### API

| 函数 | 签名 | 语义 | Rust 映射 |
|------|------|------|----------|
| `sha256` | `crypto.sha256(data: number[]): number[32]` | SHA-256 哈希 | `sha2::Sha256::digest(data)` |
| `blake3` | `crypto.blake3(data: number[]): number[32]` | BLAKE3 哈希 | `blake3::hash(data)` |

### 验收标准

- [ ] `crypto.sha256([])` → 32 字节数组
- [ ] `crypto.blake3([1,2,3])` ≠ `crypto.blake3([1,2,4])`

---

## Trust API → Rust 映射表

| Trust API | Rust 实现 | 来源 |
|-----------|----------|------|
| `Option::unwrap` | `Option::unwrap` | `std::option` |
| `Result::unwrapOr` | `Result::unwrap_or` | `std::result` |
| `Vec::push` | `Vec::push` | `std::vec` |
| `HashMap::insert` | `HashMap::insert` | `std::collections` |
| `String::split` | `str::split` + `collect` | `std::string` |
| `fs::readToString` | `std::fs::read_to_string` | `std::fs` |
| `Rc::new` | `std::rc::Rc::new` | `std::rc` |
| `Arc::new` | `std::sync::Arc::new` | `std::sync` |
| `Box::new` | `Box::new` | `std::boxed` |
| `Channel` | `tokio::sync::mpsc::channel` | `ferro_rt` |
| `Sender::send` | `tokio::sync::mpsc::Sender::send` | `ferro_rt` |
| `Mutex::lock` | `std::sync::Mutex::lock` | `ferro_rt` |
| `AtomicI32::fetchAdd` | `std::sync::atomic::AtomicI32::fetch_add` | `ferro_rt` |
| `spawn` (线程) | `std::thread::spawn` | `ferro_rt` |
| `spawn async` | `tokio::spawn` | `ferro_rt` |
| `join` | `ferro_rt::join` → `tokio::join!` | `ferro_rt` |
| `sleep` (异步) | `tokio::time::sleep` | `ferro_rt` |
| `time.now` | `std::time::SystemTime::now` | `std::time` |
| `process.run` | `std::process::Command::output` | `std::process` |
| `http.get` | `reqwest::get` | `ferro_rt` / extern binding |
| `serde.fromStr` | `serde_json::from_str` | `ferro_rt` / extern binding |
| `crypto.sha256` | `sha2::Sha256::digest` | extern binding |

---

> **审计定位：** 本文档与 `docs/Trust-设计文档.md §10`、`spec/trust-spec.md` LEX/SYN/CON/ERR/OWN 域、`docs/design-constraints.md §9.2` 对齐。  
> **下一步：** Phase 0.3 — 三方交叉审计。
