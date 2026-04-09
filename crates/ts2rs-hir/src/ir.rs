//! ts2rs 高层 IR（与 swc AST 解耦，便于语义检查与代码生成）。
//!
//! **Span**：`IRExpr` / `IRStmt` / `IRFunction` 上的 [`Span`] 来自 swc；诊断与代码生成错误应使用
//! 所属函数的 [`IRFunction::cm`]、[`IRFunction::source_path`] 与**具体节点**的 `span`（见 [`crate::error::diag`]）。
//!
//! **函数级 `ir_id`**：单次编译内单调递增，用于调试或后续 MIR/SSA 粗粒度锚点；不保证跨编译稳定。

use std::cmp::Ordering;
use std::collections::HashMap;
use std::fmt;

use swc_common::sync::Lrc;
use swc_common::SourceMap;
use swc_common::Span;

/// Wraps [`Lrc`]`<`[`SourceMap`]`>` so [`IRFunction`] / [`IRClass`] implement `std::marker::Send` for [`rayon`].
/// `swc_common::sync::Lrc` does not implement `Send`, but a shared source map is only used for span lookup (like `Arc`).
#[derive(Clone)]
pub struct SendSourceMap(pub Lrc<SourceMap>);

unsafe impl Send for SendSourceMap {}
unsafe impl Sync for SendSourceMap {}

impl std::ops::Deref for SendSourceMap {
    type Target = Lrc<SourceMap>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl AsRef<SourceMap> for SendSourceMap {
    fn as_ref(&self) -> &SourceMap {
        self.0.as_ref()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TsType {
    Number,
    Boolean,
    String,
    /// 字面量类型，如 `42`、`'a'`、`true`（类型位置）。
    NumberLit(i32),
    BoolLit(bool),
    StringLit(String),
    /// `console.log` 等无值表达式
    Void,
    /// 字面量 `null`
    Null,
    /// `undefined` / `void 0`
    Undefined,
    /// 受限数组：`number[]`
    ArrayNumber,
    /// 受限数组：`string[]`
    ArrayString,
    /// `Promise.all` 等元素为 `Response` 时的同质数组（`Vec<reqwest::Response>`）
    ArrayHttpResponse,
    /// `fetch()` 返回的 Response（受限子集：`status` / `ok` / `text()` / `json()` / `body.getReader()` 流式读）
    HttpResponse,
    /// `response.body` 链式占位（不可单独作为 `let` 右值，仅用于 `.getReader()`）
    ReadableStream,
    /// `response.body.getReader()` 的 reader（仅用于 `await reader.read()`）
    ReadableStreamDefaultReader,
    /// `await reader.read()` 的结果：`done` / `value`
    StreamReadResult,
    /// 流式字节块（`Vec<u8>`）
    Uint8Array,
    /// 受限对象：`{ k: number, ... }`（字段名排序、唯一）
    ObjectNum(Vec<String>),
    /// 联合类型 `A | B`（成员已规范化：扁平化、去重、按 [`cmp_ts_type`] 排序）。
    Union(Vec<TsType>),
    /// 类型参数（泛型）
    TypeParam(String),
    /// 函数类型（高阶函数子集）
    Fn {
        params: Vec<TsType>,
        ret: Box<TsType>,
    },
    /// 类实例（OO 子集）
    ClassInstance(String),
    /// `Promise<T>`（async / `fetch` / `fetchText` 等）
    Promise(Box<TsType>),
}

/// 稳定全序，用于联合类型规范化与 `B | A` 与 `A | B` 相等。
pub fn cmp_ts_type(a: &TsType, b: &TsType) -> Ordering {
    use Ordering::*;
    use TsType::*;
    let ra = variant_rank(a);
    let rb = variant_rank(b);
    if ra != rb {
        return ra.cmp(&rb);
    }
    match (a, b) {
        (BoolLit(x), BoolLit(y)) => x.cmp(y),
        (NumberLit(x), NumberLit(y)) => x.cmp(y),
        (StringLit(x), StringLit(y)) => x.cmp(y),
        (ObjectNum(x), ObjectNum(y)) => x.cmp(y),
        (TypeParam(x), TypeParam(y)) => x.cmp(y),
        (ClassInstance(x), ClassInstance(y)) => x.cmp(y),
        (ArrayString, ArrayString) => Equal,
        (ArrayHttpResponse, ArrayHttpResponse) => Equal,
        (HttpResponse, HttpResponse) => Equal,
        (ReadableStream, ReadableStream) => Equal,
        (ReadableStreamDefaultReader, ReadableStreamDefaultReader) => Equal,
        (StreamReadResult, StreamReadResult) => Equal,
        (Uint8Array, Uint8Array) => Equal,
        (Promise(a), Promise(b)) => cmp_ts_type(a, b),
        (
            Fn {
                params: ap,
                ret: ar,
            },
            Fn {
                params: bp,
                ret: br,
            },
        ) => {
            let c = ap.len().cmp(&bp.len());
            if c != Equal {
                return c;
            }
            for (x, y) in ap.iter().zip(bp.iter()) {
                let cc = cmp_ts_type(x, y);
                if cc != Equal {
                    return cc;
                }
            }
            cmp_ts_type(ar, br)
        }
        (Union(x), Union(y)) => {
            let mut ita = x.iter();
            let mut itb = y.iter();
            loop {
                match (ita.next(), itb.next()) {
                    (None, None) => return Equal,
                    (None, Some(_)) => return Less,
                    (Some(_), None) => return Greater,
                    (Some(a), Some(b)) => {
                        let c = cmp_ts_type(a, b);
                        if c != Equal {
                            return c;
                        }
                    }
                }
            }
        }
        _ => Equal,
    }
}

fn variant_rank(t: &TsType) -> u8 {
    match t {
        TsType::Void => 0,
        TsType::Null => 1,
        TsType::Undefined => 2,
        TsType::Boolean => 3,
        TsType::BoolLit(_) => 4,
        TsType::Number => 5,
        TsType::NumberLit(_) => 6,
        TsType::String => 7,
        TsType::StringLit(_) => 8,
        TsType::ArrayNumber => 9,
        TsType::ArrayString => 10,
        TsType::ArrayHttpResponse => 11,
        TsType::HttpResponse => 12,
        TsType::ReadableStream => 13,
        TsType::ReadableStreamDefaultReader => 14,
        TsType::StreamReadResult => 15,
        TsType::Uint8Array => 16,
        TsType::ObjectNum(_) => 17,
        TsType::TypeParam(_) => 18,
        TsType::Fn { .. } => 19,
        TsType::ClassInstance(_) => 20,
        TsType::Promise(_) => 21,
        TsType::Union(_) => 22,
    }
}

fn collect_union_flat(t: TsType, out: &mut Vec<TsType>) {
    match t {
        TsType::Union(inner) => {
            for x in inner {
                collect_union_flat(x, out);
            }
        }
        other => out.push(other),
    }
}

/// 扁平化嵌套 `|`、去重、排序；单成员折叠为标量；空并集返回 [`TsType::Void`]（调用方应拒绝空输入）。
pub fn normalize_union(members: Vec<TsType>) -> TsType {
    let mut flat = Vec::new();
    for m in members {
        collect_union_flat(m, &mut flat);
    }
    flat.sort_by(cmp_ts_type);
    flat.dedup();
    match flat.len() {
        0 => TsType::Void,
        1 => flat.pop().expect("len checked"),
        _ => TsType::Union(flat),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IRBinOp {
    Add,
    Sub,
    Mul,
    Div,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    /// `&&`（boolean；`number` 按 `!= 0` 真值，见 [`BinaryKind::Logical`]）
    LogicalAnd,
    /// `||`（同上）
    LogicalOr,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IRUnaryOp {
    Not,
    Neg,
}

/// `obj.length` 的代码生成策略（由语义阶段在 `prop == "length"` 时写入）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemberLengthDispatch {
    /// JS `String.prototype.length`（UTF-16 码元数）
    JsStringUtf16,
    /// `number[]` → `Vec<i32>::len`
    VecLen,
    /// `Uint8Array` → `Vec<u8>::len`
    Uint8ArrayLen,
}

/// `StreamReadResult` 的 `.done` / `.value`（由 sem 填入）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamReadResultMember {
    Done,
    Value,
}

/// `Math.abs` / `Math.min` 等受限内建（整数 `number` 子集）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MathBuiltinKind {
    Abs,
    Min,
    Max,
    Floor,
    Ceil,
    /// `Math.sign(x)` → `-1 | 0 | 1`
    Sign,
    /// 向零截断（`x` 为 `number` / `i32`）
    Trunc,
    /// 四舍五入到最近整数（`0.5` 远离零）
    Round,
    /// `Math.pow(base, exp)`：`exp` 须为非负且结果在 `i32` 可表示范围内（见 sem）
    Pow,
}

/// `Number.parseInt` / `Number.parseFloat`（`parseFloat` 在 Rust 中向零截断为 `i32`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NumberBuiltinKind {
    ParseInt,
    ParseFloat,
}

/// `JSON.stringify` / `JSON.parse` 子集。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JsonBuiltinKind {
    /// 参数：`number` | `boolean` | `string`
    Stringify,
    /// 参数：`string`；动态调用须为合法 JSON **number** 文档（`trim()` 后）；字符串字面量可走编译期折叠
    Parse,
}

/// 全局 `encodeURIComponent` / `decodeURIComponent`（trust：`string` → `string`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UriBuiltinKind {
    EncodeComponent,
    DecodeComponent,
}

/// `String.prototype` 内建子集（UTF-16 码元语义与 `length` / `charCodeAt` 一致）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StringMethodKind {
    CharAt,
    CharCodeAt,
    Slice,
    Substring,
    IndexOf,
    Includes,
}

/// `Response.prototype.text` / `json`（由 sem 校验 receiver 为 [`TsType::HttpResponse`]）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HttpResponseMethodKind {
    Text,
    /// 与动态 `JSON.parse` 一致：读 body 后 `serde_json::from_str::<f64>(trim())`
    Json,
}

/// `response.status` / `response.ok` / `response.body`（由 sem 填入）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HttpResponseMember {
    Status,
    Ok,
    Body,
}

/// `fetch(url, init)` 的 `init`（第二参对象字面量）。
#[derive(Debug, Clone)]
pub struct FetchInit {
    pub method: Option<String>,
    pub headers: Vec<(String, String)>,
    pub body: Option<Box<IRExpr>>,
}

/// `arr[i]` / `s[i]` 的下标语义（由 sem 填入）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IndexKind {
    /// `number[]` → `Vec<i32>`
    ArrayNumber,
    /// `string[]` → `Vec<String>` 元素下标
    ArrayStringElem,
    /// JS 字符串：下标为 UTF-16 码元索引
    StringUtf16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryKind {
    Int,
    StrConcat,
    /// `&&` / `||`：左右是否在 Rust 中按 number 真值（`!= 0`）；`false` 表示该侧为 boolean 表达式
    Logical {
        lhs_number_truthy: bool,
        rhs_number_truthy: bool,
    },
}

#[derive(Debug, Clone)]
pub enum IRExpr {
    /// 数值字面量/运行时 `number`（Rust 侧为 `f64`）。
    Number(f64, Span),
    Bool(bool, Span),
    Str(String, Span),
    Ident(String, Span),
    Binary {
        op: IRBinOp,
        left: Box<IRExpr>,
        right: Box<IRExpr>,
        span: Span,
        /// `None` 在语义检查后填入
        kind: Option<BinaryKind>,
    },
    Unary {
        op: IRUnaryOp,
        arg: Box<IRExpr>,
        span: Span,
    },
    Call {
        callee: String,
        args: Vec<IRExpr>,
        /// 显式类型实参 `f<T, U>(...)`
        type_args: Vec<TsType>,
        span: Span,
    },
    /// `obj.m(args)`：脱糖为全局函数 `m(receiver, ...args)`（`receiver` 为 `obj` 的值）。
    MethodCall {
        receiver: Box<IRExpr>,
        method: String,
        args: Vec<IRExpr>,
        type_args: Vec<TsType>,
        span: Span,
    },
    /// `f?.(args)`：可选调用（callee 为顶层标识符）；类型检查与 [`IRExpr::Call`] 一致，代码生成同 [`IRExpr::Call`]（非空 callee 下与 `f(args)` 等价）。
    OptionalCall {
        callee: String,
        args: Vec<IRExpr>,
        type_args: Vec<TsType>,
        span: Span,
    },
    /// `obj?.m(args)`：可选方法调用；类型检查与 [`IRExpr::MethodCall`] 一致，代码生成同 [`IRExpr::MethodCall`]。
    OptionalMethodCall {
        receiver: Box<IRExpr>,
        method: String,
        args: Vec<IRExpr>,
        type_args: Vec<TsType>,
        span: Span,
    },
    /// `console.log` / `console.error` / `console.debug`（`stderr: true` 时生成 `eprintln!`）
    BuiltinLog {
        args: Vec<IRExpr>,
        /// `true`：`error` / `debug`；`false`：`log`
        stderr: bool,
        span: Span,
    },
    /// `cond ? a : b`（`cond_ty` 由语义阶段填入）
    Conditional {
        test: Box<IRExpr>,
        cons: Box<IRExpr>,
        alt: Box<IRExpr>,
        span: Span,
        cond_ty: Option<TsType>,
    },
    /// 逗号表达式 `(a, b, c)`，结果为最后一项的类型与值
    Seq {
        exprs: Vec<IRExpr>,
        span: Span,
    },
    /// 模板字符串片段（静态与插值交替）
    Tpl {
        parts: Vec<TplPart>,
        span: Span,
    },
    /// 属性访问 `obj.prop`（受限子集，见 `sem`）
    Member {
        obj: Box<IRExpr>,
        prop: String,
        span: Span,
        /// `prop == "length"` 时由 `sem` 填入；`None` 表示对象数字字段等走 `HashMap::get`
        length_dispatch: Option<MemberLengthDispatch>,
        /// `Response.status` / `Response.ok` / `Response.body` 时由 `sem` 填入
        http_response_member: Option<HttpResponseMember>,
        /// `StreamReadResult.done` / `value` 时由 `sem` 填入
        stream_read_member: Option<StreamReadResultMember>,
    },
    /// 字面量 `null`
    Null(Span),
    /// 字面量 `undefined`（含 `void 0`）
    Undefined(Span),
    /// 空值合并 `a ?? b`
    NullishCoalesce {
        left: Box<IRExpr>,
        right: Box<IRExpr>,
        span: Span,
    },
    /// 可选成员访问 `obj?.prop`
    OptionalMember {
        obj: Box<IRExpr>,
        prop: String,
        span: Span,
        length_dispatch: Option<MemberLengthDispatch>,
        http_response_member: Option<HttpResponseMember>,
        stream_read_member: Option<StreamReadResultMember>,
    },
    /// `Math.abs` / `Math.min` / …（受限子集）
    MathBuiltin {
        kind: MathBuiltinKind,
        args: Vec<IRExpr>,
        span: Span,
    },
    /// `Number.parseInt` / `Number.parseFloat`
    NumberBuiltin {
        kind: NumberBuiltinKind,
        args: Vec<IRExpr>,
        span: Span,
    },
    /// `JSON.stringify` / `JSON.parse`
    JsonBuiltin {
        kind: JsonBuiltinKind,
        args: Vec<IRExpr>,
        span: Span,
        /// `JSON.stringify`：由 sem 填入第一个参数的静态类型，供 codegen 选择分支
        stringify_inferred_ty: Option<TsType>,
    },
    /// `encodeURIComponent` / `decodeURIComponent`
    UriBuiltin {
        kind: UriBuiltinKind,
        args: Vec<IRExpr>,
        span: Span,
    },
    /// `String.prototype.*` 内建（不经过全局 `method(receiver, …)`）
    StringMethodBuiltin {
        kind: StringMethodKind,
        receiver: Box<IRExpr>,
        args: Vec<IRExpr>,
        span: Span,
    },
    /// 从标准输入读取一行（无换行符），`string`
    ReadStdinLine {
        span: Span,
    },
    /// 数组字面量 `[a, b, ...]`（无空洞、无 spread）
    ArrayLit {
        elems: Vec<IRExpr>,
        span: Span,
    },
    /// 对象字面量 `{ k: v }`（仅 key-value）
    ObjectLit {
        fields: Vec<(String, IRExpr)>,
        span: Span,
    },
    /// 下标访问 `arr[i]` / `s[i]`（`index_kind` 由 sem 填入）
    Index {
        obj: Box<IRExpr>,
        index: Box<IRExpr>,
        span: Span,
        index_kind: Option<IndexKind>,
    },
    /// 箭头函数表达式（函数值）
    ArrowFn {
        params: Vec<(String, TsType)>,
        ret: TsType,
        body: Vec<IRStmt>,
        span: Span,
    },
    /// `this`
    This(Span),
    /// `super`
    Super(Span),
    /// `await expr`（`arg` 须为 `Promise<T>`，整体类型为 `T`）
    Await {
        arg: Box<IRExpr>,
        span: Span,
    },
    /// 内建 `fetchText(url)` → `Promise<string>`（须由 `await` 包裹）
    FetchText {
        url: Box<IRExpr>,
        span: Span,
    },
    /// 浏览器式 `fetch(url, init?)` → `Promise<HttpResponse>`（须由 `await` 包裹）
    Fetch {
        url: Box<IRExpr>,
        init: Option<FetchInit>,
        span: Span,
    },
    /// `response.text()` / `response.json()`（返回 `Promise`，须 `await`）
    HttpResponseMethodBuiltin {
        kind: HttpResponseMethodKind,
        receiver: Box<IRExpr>,
        span: Span,
    },
    /// `expr.body.getReader()`（`expr` 须为 `HttpResponse`；`stream_slot` 由 sem 填入，供 codegen）
    HttpResponseBodyGetReader {
        response: Box<IRExpr>,
        span: Span,
        stream_slot: Option<u32>,
    },
    /// `reader.read()`（返回 `Promise<StreamReadResult>`，须 `await`；`reader_slot` 由 sem 填入）
    ReaderRead {
        reader_name: String,
        span: Span,
        reader_slot: Option<u32>,
    },
    /// `Promise.all([...])`，元素须为同质 `Promise<T>`
    PromiseAll {
        elems: Vec<IRExpr>,
        span: Span,
    },
}

/// 模板字面量片段
#[derive(Debug, Clone)]
pub enum TplPart {
    Static(String),
    Interp(Box<IRExpr>),
}

/// 语句 IR。源语言 `switch` 在 [`crate::build`] 中降为嵌套 [`IRStmt::If`]（与字面量 `===` 比较），无单独的 `Switch` 变体。
#[derive(Debug, Clone)]
pub enum IRStmt {
    /// `;` 空语句
    Empty {
        span: Span,
    },
    Let {
        name: String,
        ty: TsType,
        /// `None` = `let x: T;`（须由语义明确赋值后再读）
        init: Option<IRExpr>,
        /// `true` = `let`（可赋值），`false` = `const`
        mutable: bool,
        span: Span,
    },
    /// `name = rhs`（rhs 已由 build 产出）
    Assign {
        name: String,
        rhs: IRExpr,
        span: Span,
    },
    /// `obj.prop = rhs`（OO/对象子集）
    MemberAssign {
        obj: String,
        prop: String,
        rhs: IRExpr,
        span: Span,
    },
    Expr {
        expr: IRExpr,
        span: Span,
    },
    Return {
        arg: Option<IRExpr>,
        span: Span,
    },
    Block {
        stmts: Vec<IRStmt>,
        span: Span,
    },
    If {
        cond: IRExpr,
        /// 由语义阶段填入，供代码生成区分 `!= 0` 与布尔条件
        cond_ty: TsType,
        then_b: Vec<IRStmt>,
        else_b: Option<Vec<IRStmt>>,
        span: Span,
    },
    While {
        cond: IRExpr,
        cond_ty: TsType,
        body: Vec<IRStmt>,
        span: Span,
    },
    /// `for (key in target) { ... }`
    ForIn {
        key: String,
        key_ty: TsType,
        target: IRExpr,
        kind: Option<ForInKind>,
        body: Vec<IRStmt>,
        span: Span,
    },
    /// `do { body } while (cond);`
    DoWhile {
        body: Vec<IRStmt>,
        cond: IRExpr,
        cond_ty: TsType,
        span: Span,
    },
    Break {
        span: Span,
    },
    Continue {
        span: Span,
    },
    /// 块内 `function foo() { ... }`（Rust 中生成嵌套 `fn`）
    FnDecl {
        func: Box<IRFunction>,
        span: Span,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ForInKind {
    ObjectKeys,
    ArrayIndices,
}

#[derive(Debug, Clone)]
pub struct IRClassMethod {
    pub name: String,
    pub params: Vec<(String, TsType)>,
    pub ret: TsType,
    pub body: Vec<IRStmt>,
    pub is_override: bool,
    pub owner: String,
    pub span: Span,
}

#[derive(Clone)]
pub struct IRClass {
    pub name: String,
    pub extends: Option<String>,
    pub fields: Vec<(String, TsType)>,
    pub ctor: Option<IRClassMethod>,
    pub methods: Vec<IRClassMethod>,
    pub span: Span,
    pub cm: SendSourceMap,
    pub source_path: String,
}

impl fmt::Debug for IRClass {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("IRClass")
            .field("name", &self.name)
            .field("extends", &self.extends)
            .field("fields", &self.fields)
            .field("ctor", &self.ctor)
            .field("methods", &self.methods)
            .field("span", &self.span)
            .field("source_path", &self.source_path)
            .finish()
    }
}

#[derive(Clone)]
pub struct IRFunction {
    /// 单次编译内唯一序号（含嵌套 `function`）。
    pub ir_id: u32,
    pub name: String,
    /// 泛型类型参数名（单态化后应为空）。
    pub type_params: Vec<String>,
    pub params: Vec<(String, TsType)>,
    pub ret: TsType,
    pub body: Vec<IRStmt>,
    pub span: Span,
    /// 该函数所在文件的源映射（多文件时用于诊断）。
    pub cm: SendSourceMap,
    /// 该函数所在文件路径（与 `entry_path` 比较以定位 `main`）。
    pub source_path: String,
    /// 单态化实例来源（如 `foo<number>`），用于诊断可读性。
    pub mono_origin: Option<String>,
    /// `async function`；`ret` 存 **兑现类型** `T`（`Promise<T>` 已剥开）。
    pub is_async: bool,
}

impl fmt::Debug for IRFunction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("IRFunction")
            .field("ir_id", &self.ir_id)
            .field("name", &self.name)
            .field("type_params", &self.type_params)
            .field("params", &self.params)
            .field("ret", &self.ret)
            .field("body", &self.body)
            .field("span", &self.span)
            .field("source_path", &self.source_path)
            .field("mono_origin", &self.mono_origin)
            .field("is_async", &self.is_async)
            .finish()
    }
}

#[derive(Debug, Clone)]
pub struct IRGenericTypeDecl {
    pub type_params: Vec<String>,
    pub body: TsType,
}

/// 单文件内按 swc `BytePos.0` 索引的 leading 注释行（已规范化，codegen 可加 `// ` 前缀）。
pub type TsLeadingComments = HashMap<u32, Vec<String>>;

#[derive(Debug, Clone)]
pub struct IRModule {
    pub fns: Vec<IRFunction>,
    pub classes: Vec<IRClass>,
    pub generic_types: HashMap<String, IRGenericTypeDecl>,
    /// 编译入口文件路径（用于要求 `main` 定义在入口模块）。
    pub entry_path: String,
    /// 各源路径对应的 TS leading 注释快照（由解析器收集；无注释时为空）。
    pub ts_comments_by_path: HashMap<String, TsLeadingComments>,
}
