//! Trust AST 节点定义 — §SEM-REQ-001
//!
//! 本模块定义所有 AST 节点类型，parser 产出 AST，后续 HIR/TIR 消费。
//! Phase 1 实际解析的 Stmt 变体: Let/Const/Shared/Function/If/For/ForOf/While/Loop/
//!   Return/Break/Continue/Expr。
//! 其余变体为 Phase 2+ 占位。

// ============================================================================
// Source Span — §3.1.1
// ============================================================================

/// 每个 AST 节点必须携带 source span。
#[derive(Debug, Clone, PartialEq)]
pub struct Span {
    pub file: String,
    pub line_start: u32, // 1-based
    pub line_end: u32,
    pub col_start: u32, // 1-based
    pub col_end: u32,
}

impl Span {
    pub fn dummy() -> Self {
        Span { file: String::new(), line_start: 0, line_end: 0, col_start: 0, col_end: 0 }
    }
}

// ============================================================================
// 语句 — §3.1.2
// ============================================================================

#[derive(Debug, Clone, PartialEq)]
pub enum Stmt {
    Let(LetStmt),
    Const(ConstStmt),
    Shared(SharedStmt),
    Function(FunctionDecl),
    If(IfExpr),
    For(ForStmt),
    ForOf(ForOfStmt),
    While(WhileStmt),
    Loop(LoopExpr),
    Return(ReturnStmt),
    Break(BreakStmt),
    Continue(ContinueStmt),
    Expr(ExprStmt),
}

/// 文件顶层结构
#[derive(Debug, Clone, PartialEq)]
pub struct Program {
    pub imports: Vec<ImportDecl>,
    pub exports: Vec<ExportDecl>,
    pub statements: Vec<Stmt>,
    pub span: Span,
}

// ============================================================================
// 关键语句结构体 — §3.1.3
// ============================================================================

#[derive(Debug, Clone, PartialEq)]
pub struct LetStmt {
    pub name: String,
    pub ty: Option<Type>,
    pub init: Box<Expr>,
    pub mutable: bool,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ConstStmt {
    pub name: String,
    pub ty: Option<Type>,
    pub init: Box<Expr>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SharedStmt {
    pub name: String,
    pub ty: Option<Type>,
    pub init: Box<Expr>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FunctionDecl {
    pub name: String,
    pub params: Vec<Param>,
    pub return_type: Option<Type>,
    pub body: Block,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Param {
    pub name: String,
    pub mode: ParamMode,
    pub ty: Option<Type>,
    /// `a?: T` — Phase 2 启用，Phase 1 字段预留，parser 拒绝 `?` 语法
    pub optional: bool,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ParamMode {
    Default,
    InOut,
    Move,
}

#[derive(Debug, Clone, PartialEq)]
pub struct IfExpr {
    pub condition: Box<Expr>,
    pub then_branch: Block,
    pub else_branch: Option<Block>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LoopExpr {
    pub body: Block,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ForStmt {
    pub init: Box<Stmt>,
    pub condition: Box<Expr>,
    pub update: Box<Expr>,
    pub body: Block,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ForOfStmt {
    pub item: String,
    pub iterator: Box<Expr>,
    pub body: Block,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct WhileStmt {
    pub condition: Box<Expr>,
    pub body: Block,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ReturnStmt {
    pub value: Option<Box<Expr>>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BreakStmt {
    /// 仅在 loop 中合法带值
    pub value: Option<Box<Expr>>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ContinueStmt {
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ExprStmt {
    pub expr: Box<Expr>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Block {
    pub statements: Vec<Stmt>,
    pub span: Span,
}

// ============================================================================
// 导入 / 导出 / 调用参数 — §3.1.4
// ============================================================================

#[derive(Debug, Clone, PartialEq)]
pub struct ImportDecl {
    pub kind: ImportKind,
    pub path: String,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ImportKind {
    Named(Vec<String>),
    Default(String),
    Namespace(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct ExportDecl {
    pub item: Box<Stmt>,
    pub default: bool,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CallArg {
    /// 调用处所有权标注
    pub mode: ParamMode,
    pub expr: Box<Expr>,
    pub span: Span,
}

// ============================================================================
// 类型节点 — §3.1.5
// ============================================================================

/// 映射: "number"→NumberType, "string"→StringType, "boolean"→BooleanType,
///        "bigint"→BigIntType,  "void"→VoidType
#[derive(Debug, Clone, PartialEq)]
pub enum Type {
    NumberType,
    StringType,
    BooleanType,
    BigIntType,
    VoidType,
    /// 名义类型标识符（Phase 1 预留，Phase 2 启用）
    Named(String),
    /// `T[]`
    Array(Box<Type>),
    /// `[T1, T2, ...]`
    Tuple(Vec<Type>),
    /// `&T`
    Ref(Box<Type>),
}

// ============================================================================
// 表达式 — §3.1.6
// ============================================================================

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    IntLiteral(i32),
    FloatLiteral(f64),
    BigIntLiteral(i64),
    StrLiteral(String),
    BoolLiteral(bool),
    Null,
    Ident(String),
    /// 成员访问 / 可选链: `expr?.field` 或 `expr.field`
    MemberAccess(MemberAccess),
    Binary(Box<Expr>, BinOp, Box<Expr>),
    Unary(UnaryOp, Box<Expr>),
    Call {
        callee: Box<Expr>,
        args: Vec<CallArg>,
        span: Span,
    },
    /// `{ stmt* }`
    BlockExpr(Block),
    ArrowFn(ArrowFn),
    /// `&expr`
    Reference(Box<Expr>),
    /// `expr!`
    AssertUnwrap(Box<Expr>),
    /// `expr?`
    TryPropagate(Box<Expr>),
    /// `expr as Type`
    AsCast {
        expr: Box<Expr>,
        ty: Type,
    },
    /// `` `...${expr}...` `` — 由 parser 组装，非 lexer 输出
    TemplateLiteral(Vec<TemplatePart>),
    /// `if` 是表达式
    IfExpr(Box<IfExpr>),
    /// `loop` 是表达式
    LoopExpr(Box<LoopExpr>),
    /// `name = expr` — 不可变/可变变量的赋值表达式
    Assign {
        name: String,
        value: Box<Expr>,
    },
}

/// 注：不保留 Expr::Paren —— 括号由 parser 在 Pratt 解析中消耗，不产生 AST 节点。

#[derive(Debug, Clone, PartialEq)]
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
    QuestionQuestion,
}

#[derive(Debug, Clone, PartialEq)]
pub enum UnaryOp {
    Neg,
    Not,
}

/// 成员访问 / 可选链
#[derive(Debug, Clone, PartialEq)]
pub struct MemberAccess {
    pub object: Box<Expr>,
    pub field: String,
    /// true = `?.` (可选链), false = `.` (普通成员访问)
    pub optional: bool,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ArrowFn {
    pub params: Vec<Param>,
    pub body: ArrowBody,
    pub is_move: bool,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ArrowBody {
    Expr(Box<Expr>),
    Block(Block),
}

// ============================================================================
// 模板字面量 — §3.1.7
// ============================================================================

/// 由 parser 消耗 TemplateHead / TemplateInterpolation / TemplateTail token 后组装
#[derive(Debug, Clone, PartialEq)]
pub enum TemplatePart {
    Literal(String),
    Expr(Box<Expr>),
}

// ============================================================================
// 验收标准 — §3.1.8
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    /// AC-AST-001: `let x = 42` 产生正确 LetStmt
    #[test]
    fn ast_let_simple_produces_correct_node() {
        let node = Stmt::Let(LetStmt {
            name: "x".into(),
            ty: None,
            init: Box::new(Expr::IntLiteral(42)),
            mutable: false,
            span: Span::dummy(),
        });
        assert!(matches!(node, Stmt::Let(_)));
        if let Stmt::Let(l) = &node {
            assert_eq!(l.name, "x");
            assert!(!l.mutable);
            assert_eq!(l.init, Box::new(Expr::IntLiteral(42)));
        }
    }

    /// AC-AST-002: 每个 AST 节点携带 source span
    #[test]
    fn ast_all_nodes_have_span() {
        let stmt = Stmt::Let(LetStmt {
            name: "x".into(),
            ty: None,
            init: Box::new(Expr::IntLiteral(1)),
            mutable: false,
            span: Span {
                file: "test.trust".into(),
                line_start: 1,
                line_end: 1,
                col_start: 1,
                col_end: 9,
            },
        });
        match &stmt {
            Stmt::Let(l) => assert!(!l.span.file.is_empty()),
            _ => unreachable!(),
        }
    }

    /// AC-AST-003: Debug 输出可完整遍历（不含省略号 `..`）
    #[test]
    fn ast_debug_no_ellipsis() {
        let stmt = Stmt::Let(LetStmt {
            name: "x".into(),
            ty: Some(Type::NumberType),
            init: Box::new(Expr::IntLiteral(42)),
            mutable: false,
            span: Span::dummy(),
        });
        let debug = format!("{:?}", stmt);
        assert!(!debug.contains(".."), "Debug output contains ellipsis: {}", debug);
        assert!(debug.contains("Let"));
        assert!(debug.contains("NumberType"));
        assert!(debug.contains("IntLiteral(42)"));
    }

    /// Expr 枚举变体完整性测试
    #[test]
    fn expr_variants_cover_all_phase1_syntax() {
        // 验证 Expr 包含 Phase 1 需要的所有变体
        let literals = vec![
            Expr::IntLiteral(1),
            Expr::FloatLiteral(2.0),
            Expr::BigIntLiteral(3),
            Expr::StrLiteral("hello".into()),
            Expr::BoolLiteral(true),
            Expr::Null,
        ];
        assert_eq!(literals.len(), 6);

        // Binary / Unary
        let _bin =
            Expr::Binary(Box::new(Expr::IntLiteral(1)), BinOp::Add, Box::new(Expr::IntLiteral(2)));
        let _un = Expr::Unary(UnaryOp::Neg, Box::new(Expr::IntLiteral(1)));

        // Reference / AssertUnwrap / TryPropagate / AsCast
        let _ref = Expr::Reference(Box::new(Expr::Ident("x".into())));
        let _bang = Expr::AssertUnwrap(Box::new(Expr::Ident("x".into())));
        let _q = Expr::TryPropagate(Box::new(Expr::Ident("x".into())));
        let _as = Expr::AsCast { expr: Box::new(Expr::IntLiteral(1)), ty: Type::NumberType };

        // MemberAccess
        let _ma = Expr::MemberAccess(MemberAccess {
            object: Box::new(Expr::Ident("x".into())),
            field: "y".into(),
            optional: false,
            span: Span::dummy(),
        });
    }
}
