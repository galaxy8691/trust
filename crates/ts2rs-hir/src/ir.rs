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
    /// `Promise<T>`（async / `fetchText` 等）
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
        TsType::ObjectNum(_) => 10,
        TsType::TypeParam(_) => 11,
        TsType::Fn { .. } => 12,
        TsType::ClassInstance(_) => 13,
        TsType::Promise(_) => 14,
        TsType::Union(_) => 15,
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
}

/// `Math.abs` / `Math.min` 等受限内建（整数 `number` 子集）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MathBuiltinKind {
    Abs,
    Min,
    Max,
    Floor,
    Ceil,
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
    Number(i32, Span),
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
    },
    /// `Math.abs` / `Math.min` / …（受限子集）
    MathBuiltin {
        kind: MathBuiltinKind,
        args: Vec<IRExpr>,
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
    /// 下标访问 `arr[i]`
    Index {
        obj: Box<IRExpr>,
        index: Box<IRExpr>,
        span: Span,
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
    /// 内建 `fetchText(url: string)` → `Promise<string>`（须由 `await` 包裹）
    FetchText {
        url: Box<IRExpr>,
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
    pub cm: Lrc<SourceMap>,
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
    pub cm: Lrc<SourceMap>,
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

#[derive(Debug, Clone)]
pub struct IRModule {
    pub fns: Vec<IRFunction>,
    pub classes: Vec<IRClass>,
    pub generic_types: HashMap<String, IRGenericTypeDecl>,
    /// 编译入口文件路径（用于要求 `main` 定义在入口模块）。
    pub entry_path: String,
}
