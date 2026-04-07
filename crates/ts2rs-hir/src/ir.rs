//! ts2rs 高层 IR（与 swc AST 解耦，便于语义检查与代码生成）。
//!
//! **Span**：`IRExpr` / `IRStmt` / `IRFunction` 上的 [`Span`] 来自 swc；诊断与代码生成错误应使用
//! 所属函数的 [`IRFunction::cm`]、[`IRFunction::source_path`] 与**具体节点**的 `span`（见 [`crate::error::diag`]）。
//!
//! **函数级 `ir_id`**：单次编译内单调递增，用于调试或后续 MIR/SSA 粗粒度锚点；不保证跨编译稳定。

use std::cmp::Ordering;
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
        TsType::Union(_) => 11,
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
        span: Span,
    },
    /// `console.log(...)` 内建
    BuiltinLog {
        args: Vec<IRExpr>,
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
}

/// 模板字面量片段
#[derive(Debug, Clone)]
pub enum TplPart {
    Static(String),
    Interp(Box<IRExpr>),
}

#[derive(Debug, Clone)]
pub enum IRStmt {
    /// `;` 空语句
    Empty {
        span: Span,
    },
    Let {
        name: String,
        ty: TsType,
        init: IRExpr,
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

#[derive(Clone)]
pub struct IRFunction {
    /// 单次编译内唯一序号（含嵌套 `function`）。
    pub ir_id: u32,
    pub name: String,
    pub params: Vec<(String, TsType)>,
    pub ret: TsType,
    pub body: Vec<IRStmt>,
    pub span: Span,
    /// 该函数所在文件的源映射（多文件时用于诊断）。
    pub cm: Lrc<SourceMap>,
    /// 该函数所在文件路径（与 `entry_path` 比较以定位 `main`）。
    pub source_path: String,
}

impl fmt::Debug for IRFunction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("IRFunction")
            .field("ir_id", &self.ir_id)
            .field("name", &self.name)
            .field("params", &self.params)
            .field("ret", &self.ret)
            .field("body", &self.body)
            .field("span", &self.span)
            .field("source_path", &self.source_path)
            .finish()
    }
}

#[derive(Debug, Clone)]
pub struct IRModule {
    pub fns: Vec<IRFunction>,
    /// 编译入口文件路径（用于要求 `main` 定义在入口模块）。
    pub entry_path: String,
}
