//! HIR 节点定义 — §SEM-REQ-002
//!
//! 本模块定义所有 HIR 节点类型。
//! AST→HIR 降级在 `name_res` 模块中完成，
//! 类型检查在 `typeck` 模块中完成。

use std::collections::HashMap;
use trust_parser::ast::{BinOp as AstBinOp, ImportKind, Span, UnaryOp as AstUnaryOp};

// ============================================================================
// 顶层结构 — §3.1.2 类型系统架构
// ============================================================================

/// §3.1.2: HIR 模块聚合 AST 降级后的所有声明（设计文档 §3.1 类型系统架构）
#[derive(Debug, Clone, PartialEq)]
pub struct HirProgram {
    /// 文件路径
    pub file: String,
    /// 导入声明（已解析，含符号绑定）
    pub imports: Vec<HirImport>,
    /// 导出声明
    pub exports: Vec<HirExport>,
    /// 顶层语句（函数声明、const、shared 等）
    pub items: Vec<HirItem>,
    /// 全局作用域符号表
    pub scope: Scope,
}

/// HIR 级别的顶层项——表达式语句已降级为具体声明
#[derive(Debug, Clone, PartialEq)]
pub enum HirItem {
    Function(HirFunction),
    Const(HirConst),
    Shared(HirShared),
    /// 模块级 let（Phase 1 不支持顶层裸表达式，此变体为占位）
    Stub(HirStub),
}

// ============================================================================
// 函数 — §3.1.3
// ============================================================================

/// §3.1.3: 函数签名 + 局部作用域
#[derive(Debug, Clone, PartialEq)]
pub struct HirFunction {
    pub name: String,
    pub params: Vec<HirParam>,
    pub return_type: HirType,
    pub body: HirBlock,
    /// 函数局部作用域（参数 + 局部变量）
    pub scope: Scope,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct HirParam {
    pub name: String,
    pub mode: ParamMode,
    pub ty: HirType,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParamMode {
    /// 只读借用 → &T
    Default,
    /// 可变借用 → &mut T
    InOut,
    /// 所有权转移 → T
    Move,
}

// ============================================================================
// 语句 — §3.1.3
// ============================================================================

#[derive(Debug, Clone, PartialEq)]
pub enum HirStmt {
    Let(HirLet),
    Const(HirConst),
    Shared(HirShared),
    If(HirIf),
    For(HirFor),
    ForOf(HirForOf),
    While(HirWhile),
    Loop(HirLoop),
    Return(HirReturn),
    Break(HirBreak),
    Continue(HirContinue),
    Expr(HirExpr),
    /// 哨兵：类型错误导致无法降级时填充
    Error,
}

#[derive(Debug, Clone, PartialEq)]
pub struct HirLet {
    pub name: String,
    pub mutable: bool,
    pub ty: HirType,
    pub init: Box<HirExpr>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct HirConst {
    pub name: String,
    pub ty: HirType,
    pub init: Box<HirExpr>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct HirShared {
    pub name: String,
    pub ty: HirType,
    pub init: Box<HirExpr>,
    pub span: Span,
}

// ========== 控制流结构体（HirStmt 变体参数） ==========

#[derive(Debug, Clone, PartialEq)]
pub struct HirBlock {
    pub statements: Vec<HirStmt>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct HirIf {
    pub condition: Box<HirExpr>,
    pub then_branch: HirBlock,
    pub else_branch: Option<HirBlock>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct HirFor {
    pub init: Box<HirStmt>,
    pub condition: Box<HirExpr>,
    pub update: Box<HirExpr>,
    pub body: HirBlock,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct HirForOf {
    pub item: String,
    pub iterator: Box<HirExpr>,
    pub body: HirBlock,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct HirWhile {
    pub condition: Box<HirExpr>,
    pub body: HirBlock,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct HirLoop {
    pub body: HirBlock,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct HirReturn {
    pub value: Option<Box<HirExpr>>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct HirBreak {
    pub value: Option<Box<HirExpr>>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct HirContinue {
    pub span: Span,
}

/// Phase 1 占位——parser 解析但 HIR 尚未降级的项
#[derive(Debug, Clone, PartialEq)]
pub struct HirStub {
    pub reason: String,
    pub span: Span,
}

// ============================================================================
// 表达式 — §3.1.4
// ============================================================================

/// Phase 1 降级：所有 AST Expr 映射到 HIR 对应变体
/// 不含泛型/闭包推断/method resolve（Phase 2+）
#[derive(Debug, Clone, PartialEq)]
pub enum HirExpr {
    /// 整数字面量 i32
    IntLiteral(i32, Span),
    /// 浮点字面量 f64
    FloatLiteral(f64, Span),
    /// BigInt 字面量 i64
    BigIntLiteral(i64, Span),
    /// 字符串字面量 String
    StringLiteral(String, Span),
    /// 布尔字面量 bool
    BoolLiteral(bool, Span),
    /// 模板字面量（已拼接为 String 或保留插值）
    TemplateLiteral(Vec<HirTemplatePart>, Span),
    /// 标识符引用（名称解析后填充 HirBinding；降级阶段用 Unresolved 占位）
    Ident(String, HirBinding, Span),
    /// 二元运算（左右操作数类型已检查）
    Binary(Box<HirExpr>, BinOp, Box<HirExpr>, HirType, Span),
    /// 一元运算
    Unary(UnaryOp, Box<HirExpr>, HirType, Span),
    /// 函数调用（实参类型已验证）
    Call(Box<HirExpr>, Vec<HirCallArg>, HirType, Span),
    /// 箭头函数（闭包）— Phase 1 参数需显式类型标注；bool = is_move
    ArrowFn(Vec<HirParam>, HirType, HirBlock, bool, Span),
    /// `as` 显式类型转换
    AsCast(Box<HirExpr>, HirType, Span),
    /// `&` 显式引用
    Reference(Box<HirExpr>, Span),
    /// `!` 断言 unwrap — Phase 1 仅 AST 解析，HIR 透传（押后 Phase 3）
    AssertUnwrap(Box<HirExpr>, Span),
    /// `?` Try 传播 — Phase 1 仅 AST 解析，HIR 透传（押后 Phase 3）
    TryPropagate(Box<HirExpr>, Span),
    /// `if` 表达式 — `let x = if (c) { 1 } else { 0 }` 降级产物（AC-SEM-002）
    If(HirIf, Span),
    /// `loop` 表达式 — `let x = loop { break 1; }` 降级产物
    Loop(HirLoop, Span),
    /// 块表达式 — `let x = { let y = 1; y }` 降级产物
    Block(HirBlock, Span),
    /// 哨兵：类型错误
    Error(Span),
}

#[derive(Debug, Clone, PartialEq)]
pub struct HirTemplatePart {
    pub kind: HirTemplatePartKind,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum HirTemplatePartKind {
    String(String),
    Expr(Box<HirExpr>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct HirCallArg {
    pub mode: ParamMode,
    pub expr: Box<HirExpr>,
    pub span: Span,
}

// ============================================================================
// 类型 — §3.1.5
// ============================================================================

/// Phase 1 类型：仅基本类型 + 错误哨兵
/// Phase 2+: Generic / TraitObject / Option / Result / ADT
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HirType {
    /// number 字面量 42 → i32
    I32,
    /// number 字面量 3.14 → f64
    F64,
    /// bigint 字面量 → i64
    I64,
    /// string 类型
    String,
    /// boolean 类型
    Bool,
    /// void（函数无返回值）
    Void,
    /// bigint 类型标注（显式写 bigint，区别于 i64 字面量推断）
    BigInt,
    /// 数组类型 `number[]` → Array(I32)
    Array(Box<HirType>),
    /// 命名类型引用（import 解析后的符号）
    Named(String),
    /// 函数类型（用于变量持有函数引用）
    Function(Vec<HirType>, Box<HirType>),
    /// 引用类型（`&T` 类型标注；与 HirExpr::Reference 区分——这是类型层面的 Ref）
    Ref(Box<HirType>),
    /// 哨兵：类型推断/检查失败
    Error,
}

// ============================================================================
// 作用域与符号表 — §3.1.6
// ============================================================================

/// 作用域——AST 降级后填充
#[derive(Debug, Clone, PartialEq)]
pub struct Scope {
    /// 父作用域（None = 模块/全局作用域）
    pub parent: Option<Box<Scope>>,
    /// 当前作用域绑定的符号
    pub bindings: HashMap<String, HirBinding>,
}

impl Scope {
    pub fn new() -> Self {
        Scope { parent: None, bindings: HashMap::new() }
    }

    pub fn new_child(parent: Box<Scope>) -> Self {
        Scope { parent: Some(parent), bindings: HashMap::new() }
    }

    /// 在当前作用域插入绑定
    pub fn insert(&mut self, name: &str, binding: HirBinding) {
        self.bindings.insert(name.to_string(), binding);
    }

    /// 从最内层向外查找符号；无匹配时返回 None
    pub fn lookup(&self, name: &str) -> Option<&HirBinding> {
        if let Some(b) = self.bindings.get(name) {
            return Some(b);
        }
        self.parent.as_ref().and_then(|p| p.lookup(name))
    }
}

/// 符号绑定——名称解析的结果
#[derive(Debug, Clone, PartialEq)]
pub enum HirBinding {
    /// 降级阶段占位，名称解析后替换为实际绑定
    Unresolved { name: String, span: Span },
    /// 局部变量（let）
    LocalVar { ty: HirType, mutable: bool, span: Span },
    /// 模块级常量（const）
    ModuleConst { ty: HirType, span: Span },
    /// 模块级共享变量（shared）
    ModuleShared { ty: HirType, span: Span },
    /// 函数
    Function { param_types: Vec<HirType>, return_type: HirType, span: Span },
    /// 导入的函数/变量（跨文件绑定）
    Import { source: String, export_name: String, ty: HirType, span: Span },
}

/// HIR 导入声明（已解析）
#[derive(Debug, Clone, PartialEq)]
pub struct HirImport {
    pub kind: ImportKind,
    /// 已解析为实际文件路径
    pub source_path: String,
    pub bindings: Vec<(String, HirBinding)>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct HirExport {
    pub name: String,
    pub binding: HirBinding,
    pub is_default: bool,
    pub span: Span,
}

impl Default for Scope {
    fn default() -> Self {
        Self::new()
    }
}
// ============================================================================

/// 二元运算符——与 trust_parser::ast::BinOp 名称对齐（复用以简化降级映射）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Eq,
    Ne,
    Lt,
    Gt,
    Le,
    Ge,
    And,
    Or,
    /// 空值合并（Phase 1 降级时排除，HIR 中保留以对齐 AST 变体）
    QuestionQuestion,
}

/// 一元运算符——与 AST UnaryOp 对齐
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOp {
    /// -x（算术取反；与 AST UnaryOp::Neg 对齐）
    Neg,
    /// !x（逻辑非）
    Not,
}

// ============================================================================
// 类型转换工具
// ============================================================================

impl HirType {
    /// §3.3.2: 从 AST Type 降级到 HirType
    pub fn from_ast_type(ast_ty: &trust_parser::ast::Type) -> Self {
        match ast_ty {
            trust_parser::ast::Type::NumberType => HirType::I32, // 默认 I32
            trust_parser::ast::Type::StringType => HirType::String,
            trust_parser::ast::Type::BooleanType => HirType::Bool,
            trust_parser::ast::Type::BigIntType => HirType::BigInt,
            trust_parser::ast::Type::VoidType => HirType::Void,
            trust_parser::ast::Type::Named(s) => HirType::Named(s.clone()),
            trust_parser::ast::Type::Array(t) => HirType::Array(Box::new(Self::from_ast_type(t))),
            trust_parser::ast::Type::Tuple(_) => HirType::Error, // Phase 1 无 Tuple
            trust_parser::ast::Type::Ref(t) => HirType::Ref(Box::new(Self::from_ast_type(t))),
        }
    }

    /// 从 AST BinOp 直接转换（名称一致，零开销）
    pub fn binop_from_ast(op: AstBinOp) -> BinOp {
        match op {
            AstBinOp::Add => BinOp::Add,
            AstBinOp::Sub => BinOp::Sub,
            AstBinOp::Mul => BinOp::Mul,
            AstBinOp::Div => BinOp::Div,
            AstBinOp::Mod => BinOp::Mod,
            AstBinOp::Eq => BinOp::Eq,
            AstBinOp::Ne => BinOp::Ne,
            AstBinOp::Lt => BinOp::Lt,
            AstBinOp::Gt => BinOp::Gt,
            AstBinOp::Le => BinOp::Le,
            AstBinOp::Ge => BinOp::Ge,
            AstBinOp::And => BinOp::And,
            AstBinOp::Or => BinOp::Or,
            AstBinOp::QuestionQuestion => BinOp::QuestionQuestion,
        }
    }

    pub fn unaryop_from_ast(op: AstUnaryOp) -> UnaryOp {
        match op {
            AstUnaryOp::Neg => UnaryOp::Neg,
            AstUnaryOp::Not => UnaryOp::Not,
        }
    }

    pub fn param_mode_from_ast(mode: &trust_parser::ast::ParamMode) -> ParamMode {
        match mode {
            trust_parser::ast::ParamMode::Default => ParamMode::Default,
            trust_parser::ast::ParamMode::InOut => ParamMode::InOut,
            trust_parser::ast::ParamMode::Move => ParamMode::Move,
        }
    }

    pub fn as_rust_type(&self) -> &'static str {
        match self {
            HirType::I32 | HirType::F64 | HirType::I64 | HirType::BigInt => "number",
            HirType::String => "string",
            HirType::Bool => "boolean",
            HirType::Void => "void",
            HirType::Array(_) => "array",
            HirType::Named(_) => "named",
            HirType::Function(..) => "function",
            HirType::Ref(_) => "ref",
            HirType::Error => "error",
        }
    }
}

impl std::fmt::Display for HirType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HirType::I32 => write!(f, "i32"),
            HirType::F64 => write!(f, "f64"),
            HirType::I64 => write!(f, "i64"),
            HirType::String => write!(f, "string"),
            HirType::Bool => write!(f, "boolean"),
            HirType::Void => write!(f, "void"),
            HirType::BigInt => write!(f, "bigint"),
            HirType::Array(t) => write!(f, "{}[]", t),
            HirType::Named(s) => write!(f, "{s}"),
            HirType::Function(params, ret) => {
                let p: Vec<String> = params.iter().map(|t| t.to_string()).collect();
                write!(f, "({}) -> {}", p.join(", "), ret)
            }
            HirType::Ref(t) => write!(f, "&{t}"),
            HirType::Error => write!(f, "<error>"),
        }
    }
}

// ============================================================================
// 单元测试 — §4.1
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // === HirType Display ===

    #[test]
    fn hir_type_display_i32() {
        assert_eq!(format!("{}", HirType::I32), "i32");
    }

    #[test]
    fn hir_type_display_f64() {
        assert_eq!(format!("{}", HirType::F64), "f64");
    }

    #[test]
    fn hir_type_display_string() {
        assert_eq!(format!("{}", HirType::String), "string");
    }

    #[test]
    fn hir_type_display_void() {
        assert_eq!(format!("{}", HirType::Void), "void");
    }

    #[test]
    fn hir_type_display_array() {
        assert_eq!(format!("{}", HirType::Array(Box::new(HirType::I32))), "i32[]");
    }

    #[test]
    fn hir_type_display_named() {
        assert_eq!(format!("{}", HirType::Named("Foo".into())), "Foo");
    }

    // === Scope ===

    #[test]
    fn scope_insert_and_lookup_current() {
        let mut scope = Scope::new();
        scope.insert(
            "x",
            HirBinding::LocalVar { ty: HirType::I32, mutable: false, span: Span::dummy() },
        );
        let b = scope.lookup("x").expect("x should be found");
        assert!(matches!(b, HirBinding::LocalVar { ty: HirType::I32, .. }));
    }

    #[test]
    fn scope_lookup_parent_when_not_in_child() {
        let mut parent = Scope::new();
        parent.insert("a", HirBinding::ModuleConst { ty: HirType::String, span: Span::dummy() });
        let child = Scope::new_child(Box::new(parent));
        assert!(child.lookup("a").is_some());
    }

    #[test]
    fn scope_lookup_none_for_unknown() {
        let scope = Scope::new();
        assert!(scope.lookup("nope").is_none());
    }

    // === HirType::from_ast_type ===

    #[test]
    fn from_ast_type_number_defaults_to_i32() {
        let h = HirType::from_ast_type(&trust_parser::ast::Type::NumberType);
        assert_eq!(h, HirType::I32);
    }

    #[test]
    fn from_ast_type_string() {
        let h = HirType::from_ast_type(&trust_parser::ast::Type::StringType);
        assert_eq!(h, HirType::String);
    }

    // === BinOp/UnaryOp conversion ===

    #[test]
    fn binop_from_ast_all_variants_no_panic() {
        // 验证所有 AST BinOp 都可以转换
        let ops = [
            AstBinOp::Add,
            AstBinOp::Sub,
            AstBinOp::Mul,
            AstBinOp::Div,
            AstBinOp::Mod,
            AstBinOp::Eq,
            AstBinOp::Ne,
            AstBinOp::Lt,
            AstBinOp::Gt,
            AstBinOp::Le,
            AstBinOp::Ge,
            AstBinOp::And,
            AstBinOp::Or,
            AstBinOp::QuestionQuestion,
        ];
        for op in &ops {
            let _ = HirType::binop_from_ast(op.clone());
        }
    }
}
