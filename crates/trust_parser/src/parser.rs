//! Trust 语法分析器 — Phase 1 子集 (25 AC-SYN)

use crate::ast::*;
use crate::lexer::{Lexer, TokenKind};

#[derive(Debug, Clone)]
pub struct Diagnostic {
    pub level: DiagLevel,
    pub message: String,
    pub span: Span,
}
#[derive(Debug, Clone, PartialEq)]
pub enum DiagLevel {
    Error,
    Warning,
}

pub struct Parser {
    lexer: Lexer,
    pub diagnostics: Vec<Diagnostic>,
    cur: TokenKind,
    file: String,
}

impl Parser {
    pub fn new(src: &str, file: &str) -> Self {
        let mut lex = Lexer::new(src, file);
        let cur = lex.next_token();
        Parser { lexer: lex, diagnostics: vec![], cur, file: file.to_string() }
    }

    fn advance(&mut self) {
        self.cur = self.lexer.next_token();
    }

    fn span(&self) -> Span {
        Span {
            file: self.file.clone(),
            line_start: self.lexer.line,
            line_end: self.lexer.line,
            col_start: self.lexer.col,
            col_end: self.lexer.col,
        }
    }

    fn error(&mut self, msg: &str) {
        self.diagnostics.push(Diagnostic {
            level: DiagLevel::Error,
            message: msg.to_string(),
            span: self.span(),
        });
    }

    fn expect_semi(&mut self) {
        if matches!(self.cur, TokenKind::Semi) {
            self.advance();
        }
    }

    fn is_sync(&self) -> bool {
        matches!(
            self.cur,
            TokenKind::Semi
                | TokenKind::RBrace
                | TokenKind::Function
                | TokenKind::Import
                | TokenKind::Export
                | TokenKind::Type
                | TokenKind::Test
                | TokenKind::Async
        )
    }

    fn panic_mode(&mut self) {
        while !matches!(self.cur, TokenKind::Eof) && !self.is_sync() {
            self.advance();
        }
    }

    // =================================================================
    // 顶层
    // =================================================================

    pub fn parse_program(&mut self) -> Program {
        let imports = self.parse_imports();
        let exports = self.parse_exports();
        let mut stmts = vec![];
        while !matches!(self.cur, TokenKind::Eof) {
            match self.parse_stmt() {
                Some(s) => stmts.push(s),
                None => self.panic_mode(),
            }
        }
        Program { imports, exports, statements: stmts, span: self.span() }
    }

    fn parse_imports(&mut self) -> Vec<ImportDecl> {
        let mut v = vec![];
        while matches!(self.cur, TokenKind::Import) {
            if let Some(i) = self.parse_import() {
                v.push(i);
            }
        }
        v
    }
    fn parse_exports(&mut self) -> Vec<ExportDecl> {
        let mut v = vec![];
        while matches!(self.cur, TokenKind::Export) {
            if let Some(e) = self.parse_export() {
                v.push(e);
            }
        }
        v
    }

    fn parse_stmt(&mut self) -> Option<Stmt> {
        match &self.cur {
            TokenKind::Let => self.parse_let(false),
            TokenKind::Const => self.parse_let(true),
            TokenKind::Shared => {
                self.advance(); // shared
                let name = self.expect_ident("shared name")?;
                let ty = if matches!(self.cur, TokenKind::Colon) {
                    self.advance();
                    self.parse_type()
                } else {
                    None
                };
                if !matches!(self.cur, TokenKind::Eq) {
                    self.error("expected =");
                    return None;
                }
                self.advance();
                let init = self.parse_expr()?;
                self.expect_semi();
                Some(Stmt::Shared(SharedStmt { name, ty, init: Box::new(init), span: self.span() }))
            }
            TokenKind::Function => self.parse_fn(),
            TokenKind::If => self.parse_if(),
            TokenKind::For => self.parse_for(),
            TokenKind::While => self.parse_while(),
            TokenKind::Return => self.parse_ret(),
            TokenKind::Break => self.parse_break(),
            TokenKind::Continue => {
                self.advance();
                self.expect_semi();
                Some(Stmt::Continue(ContinueStmt { span: self.span() }))
            }
            TokenKind::LBrace => self.parse_block().map(|b| {
                Stmt::Expr(ExprStmt { expr: Box::new(Expr::BlockExpr(b)), span: self.span() })
            }),
            TokenKind::Semi => {
                self.advance();
                None
            }
            _ => self.parse_expr_stmt(),
        }
    }

    fn parse_expr_stmt(&mut self) -> Option<Stmt> {
        if !self.can_expr_start() {
            self.error(&format!("unexpected {:?}", self.cur));
            self.advance();
            return None;
        }
        let e = self.parse_expr()?;
        self.expect_semi();
        Some(Stmt::Expr(ExprStmt { expr: Box::new(e), span: self.span() }))
    }

    // =================================================================
    // 变量声明
    // =================================================================

    fn parse_let(&mut self, is_const: bool) -> Option<Stmt> {
        self.advance(); // let/const
        let mutable = if !is_const && matches!(self.cur, TokenKind::Mut) {
            self.advance();
            true
        } else {
            false
        };
        let name = self.expect_ident("variable name")?;
        let ty = if matches!(self.cur, TokenKind::Colon) {
            self.advance();
            self.parse_type()
        } else {
            None
        };
        if !matches!(self.cur, TokenKind::Eq) {
            self.error("expected =");
            return None;
        }
        self.advance();
        let init = self.parse_expr()?;
        self.expect_semi();
        if is_const {
            Some(Stmt::Const(ConstStmt { name, ty, init: Box::new(init), span: self.span() }))
        } else {
            Some(Stmt::Let(LetStmt { name, ty, init: Box::new(init), mutable, span: self.span() }))
        }
    }

    // =================================================================
    // 函数
    // =================================================================

    fn parse_fn(&mut self) -> Option<Stmt> {
        self.advance(); // function
        let name = self.expect_ident("function name")?;
        let params = self.parse_params()?;
        let ret = if matches!(self.cur, TokenKind::Colon) {
            self.advance();
            self.parse_type()
        } else {
            None
        };
        let body = if matches!(self.cur, TokenKind::Eq) {
            self.advance();
            let e = self.parse_expr()?;
            self.expect_semi();
            Block {
                statements: vec![Stmt::Return(ReturnStmt {
                    value: Some(Box::new(e)),
                    span: self.span(),
                })],
                span: self.span(),
            }
        } else {
            self.parse_block()?
        };
        Some(Stmt::Function(FunctionDecl {
            name,
            params,
            return_type: ret,
            body,
            span: self.span(),
        }))
    }

    fn parse_params(&mut self) -> Option<Vec<Param>> {
        if !matches!(self.cur, TokenKind::LParen) {
            self.error("expected (");
            return None;
        }
        self.advance();
        let mut v = vec![];
        if !matches!(self.cur, TokenKind::RParen) {
            loop {
                let mode = match &self.cur {
                    TokenKind::InOut => {
                        self.advance();
                        ParamMode::InOut
                    }
                    TokenKind::Move => {
                        self.advance();
                        ParamMode::Move
                    }
                    _ => ParamMode::Default,
                };
                let name = self.expect_ident("param name")?;
                let ty = if matches!(self.cur, TokenKind::Colon) {
                    self.advance();
                    self.parse_type()
                } else {
                    None
                };
                v.push(Param { name, mode, ty, optional: false, span: self.span() });
                if matches!(self.cur, TokenKind::Comma) {
                    self.advance();
                } else {
                    break;
                }
            }
        }
        if !matches!(self.cur, TokenKind::RParen) {
            self.error("expected )");
        }
        self.advance();
        Some(v)
    }

    fn expect_ident(&mut self, ctx: &str) -> Option<String> {
        match &self.cur {
            TokenKind::Ident(s) => {
                let r = s.clone();
                self.advance();
                Some(r)
            }
            _ => {
                self.error(&format!("expected {} but got {:?}", ctx, self.cur));
                None
            }
        }
    }

    // =================================================================
    // 控制流
    // =================================================================

    fn parse_block(&mut self) -> Option<Block> {
        if !matches!(self.cur, TokenKind::LBrace) {
            self.error("expected {");
            return None;
        }
        self.advance();
        let mut stmts = vec![];
        while !matches!(self.cur, TokenKind::RBrace | TokenKind::Eof) {
            match self.parse_stmt() {
                Some(s) => stmts.push(s),
                None => self.panic_mode(),
            }
        }
        if matches!(self.cur, TokenKind::RBrace) {
            self.advance();
        }
        Some(Block { statements: stmts, span: self.span() })
    }

    fn parse_if(&mut self) -> Option<Stmt> {
        self.advance();
        let cond = self.parse_paren_expr()?;
        let then = self.parse_block()?;
        let els = if matches!(self.cur, TokenKind::Else) {
            self.advance();
            if matches!(self.cur, TokenKind::If) {
                Some(Block { statements: vec![self.parse_if()?], span: self.span() })
            } else {
                self.parse_block()
            }
        } else {
            None
        };
        Some(Stmt::If(IfExpr {
            condition: Box::new(cond),
            then_branch: then,
            else_branch: els,
            span: self.span(),
        }))
    }

    fn parse_for(&mut self) -> Option<Stmt> {
        self.advance();
        if !matches!(self.cur, TokenKind::LParen) {
            self.error("expected (");
            return None;
        }
        self.advance();
        if !matches!(self.cur, TokenKind::Let) {
            self.error("expected let in for");
            return None;
        }
        self.advance();
        let name = self.expect_ident("for var")?;
        if matches!(self.cur, TokenKind::Colon) {
            // for-of: for (let item of items)
            self.advance();
            let _ty = self.parse_type();
            if !matches!(self.cur, TokenKind::Eq) {
                self.error("expected =");
                return None;
            }
            self.advance();
            let _init2 = self.parse_expr()?;
            if !matches!(self.cur, TokenKind::Of) {
                self.error("expected of");
                return None;
            }
            self.advance();
            let iter = self.parse_expr()?;
            if !matches!(self.cur, TokenKind::RParen) {
                self.error("expected )");
            }
            self.advance();
            let body = self.parse_block()?;
            Some(Stmt::ForOf(ForOfStmt {
                item: name,
                iterator: Box::new(iter),
                body,
                span: self.span(),
            }))
        } else if matches!(self.cur, TokenKind::Of) {
            // for-of without type: for (let item of items)
            self.advance();
            let iter = self.parse_expr()?;
            if !matches!(self.cur, TokenKind::RParen) {
                self.error("expected )");
            }
            self.advance();
            let body = self.parse_block()?;
            Some(Stmt::ForOf(ForOfStmt {
                item: name,
                iterator: Box::new(iter),
                body,
                span: self.span(),
            }))
        } else {
            // C-style: for (let i = 0; i < 10; i++)
            if !matches!(self.cur, TokenKind::Eq) {
                self.error("expected =");
                return None;
            }
            self.advance();
            let init_val = self.parse_expr()?;
            if !matches!(self.cur, TokenKind::Semi) {
                self.error("expected ;");
                return None;
            }
            self.advance();
            let cond = self.parse_expr()?;
            if !matches!(self.cur, TokenKind::Semi) {
                self.error("expected ;");
                return None;
            }
            self.advance();
            let update = self.parse_expr()?;
            if !matches!(self.cur, TokenKind::RParen) {
                self.error("expected )");
            }
            self.advance();
            let body = self.parse_block()?;
            let init_stmt = Stmt::Let(LetStmt {
                name: name.clone(),
                ty: None,
                init: Box::new(init_val),
                mutable: true,
                span: self.span(),
            });
            Some(Stmt::For(ForStmt {
                init: Box::new(init_stmt),
                condition: Box::new(cond),
                update: Box::new(update),
                body,
                span: self.span(),
            }))
        }
    }

    fn parse_while(&mut self) -> Option<Stmt> {
        self.advance();
        let cond = self.parse_paren_expr()?;
        let body = self.parse_block()?;
        Some(Stmt::While(WhileStmt { condition: Box::new(cond), body, span: self.span() }))
    }

    fn parse_ret(&mut self) -> Option<Stmt> {
        self.advance();
        let v = if self.can_expr_start() { self.parse_expr() } else { None };
        self.expect_semi();
        Some(Stmt::Return(ReturnStmt { value: v.map(Box::new), span: self.span() }))
    }

    fn parse_break(&mut self) -> Option<Stmt> {
        self.advance();
        // v2.0: break with value is not supported (loop removed)
        // v2.0: break value removed (loop removed)
        if self.can_expr_start() {
            self.error("break with value is not supported");
            let _ = self.parse_expr(); // consume for error recovery
        }
        self.expect_semi();
        #[allow(deprecated)]
        { Some(Stmt::Break(BreakStmt { value: None, span: self.span() })) }
    }

    fn parse_paren_expr(&mut self) -> Option<Expr> {
        if !matches!(self.cur, TokenKind::LParen) {
            self.error("expected (");
            return None;
        }
        self.advance();
        let e = self.parse_expr()?;
        if !matches!(self.cur, TokenKind::RParen) {
            self.error("expected )");
        }
        self.advance();
        Some(e)
    }

    // =================================================================
    // 模块
    // =================================================================

    fn parse_import(&mut self) -> Option<ImportDecl> {
        self.advance();
        let kind = match &self.cur {
            TokenKind::Star => {
                self.advance(); // *
                if !matches!(self.cur, TokenKind::As) {
                    self.error("expected 'as' after *");
                    return None;
                }
                self.advance(); // as
                let n = self.expect_ident("ns name")?;
                ImportKind::Namespace(n)
            }
            TokenKind::LBrace => {
                self.advance();
                let mut ns = vec![];
                while let TokenKind::Ident(s) = &self.cur {
                    ns.push(s.clone());
                    self.advance();
                    if !matches!(self.cur, TokenKind::Comma) {
                        break;
                    }
                    self.advance();
                }
                if !matches!(self.cur, TokenKind::RBrace) {
                    self.error("expected }");
                }
                self.advance();
                ImportKind::Named(ns)
            }
            TokenKind::Ident(s) => {
                let n = s.clone();
                self.advance();
                ImportKind::Default(n)
            }
            _ => {
                self.error("expected import spec");
                return None;
            }
        };
        if !matches!(self.cur, TokenKind::From) {
            self.error("expected from");
            return None;
        }
        self.advance();
        let path = match &self.cur {
            TokenKind::StrLiteral(s) => s.clone(),
            _ => {
                self.error("expected path");
                return None;
            }
        };
        self.advance();
        self.expect_semi();
        Some(ImportDecl { kind, path, span: self.span() })
    }

    fn parse_export(&mut self) -> Option<ExportDecl> {
        self.advance();
        let def = matches!(self.cur, TokenKind::Default);
        if def {
            self.advance();
        }
        let item = self.parse_stmt()?;
        Some(ExportDecl { item: Box::new(item), default: def, span: self.span() })
    }

    // =================================================================
    // 类型
    // =================================================================

    fn parse_type(&mut self) -> Option<Type> {
        let base = match &self.cur {
            TokenKind::NumberType => {
                self.advance();
                Type::NumberType
            }
            TokenKind::StringType => {
                self.advance();
                Type::StringType
            }
            TokenKind::BooleanType => {
                self.advance();
                Type::BooleanType
            }
            TokenKind::VoidType => {
                self.advance();
                Type::VoidType
            }
            TokenKind::Amp => {
                self.advance();
                return self.parse_type().map(|t| Type::Ref(Box::new(t)));
            }
            TokenKind::Ident(_) => {
                let s = self.expect_ident("type name")?;
                Type::Named(s)
            }
            _ => {
                self.error(&format!("expected type, got {:?}", self.cur));
                return None;
            }
        };
        // Postfix: T[]
        if matches!(self.cur, TokenKind::LBracket) {
            self.advance(); // [
            if !matches!(self.cur, TokenKind::RBracket) {
                self.error("expected ]");
            }
            self.advance(); // ]
            Some(Type::Array(Box::new(base)))
        } else {
            Some(base)
        }
    }

    // =================================================================
    // 表达式
    // =================================================================

    fn can_expr_start(&self) -> bool {
        matches!(
            &self.cur,
            TokenKind::IntLiteral(_)
                | TokenKind::FloatLiteral(_)
                // v2.0: BigIntLiteral removed
                | TokenKind::StrLiteral(_)
                | TokenKind::True
                | TokenKind::False
                | TokenKind::Null
                | TokenKind::Ident(_)
                | TokenKind::LParen
                | TokenKind::LBrace
                | TokenKind::Amp
                | TokenKind::Bang
                | TokenKind::Minus
                | TokenKind::If
                // TokenKind::Loop removed in v2.0
                | TokenKind::Move
                | TokenKind::TemplateHead(_)
                | TokenKind::TemplateTail(_)
        )
    }

    pub(crate) fn parse_expr(&mut self) -> Option<Expr> {
        self.parse_binary(0)
    }

    fn parse_binary(&mut self, min: u8) -> Option<Expr> {
        let mut lhs = self.parse_unary()?;
        loop {
            let (lbp, rbp, op) = match &self.cur {
                TokenKind::QuestionQuestion => (8, 9, Some(BinOp::QuestionQuestion)),
                TokenKind::EqEq | TokenKind::Ne => (
                    7,
                    8,
                    Some(match &self.cur {
                        TokenKind::EqEq => BinOp::Eq,
                        _ => BinOp::Ne,
                    }),
                ),
                TokenKind::Lt => (7, 8, Some(BinOp::Lt)),
                TokenKind::Gt => (7, 8, Some(BinOp::Gt)),
                TokenKind::Le => (7, 8, Some(BinOp::Le)),
                TokenKind::Ge => (7, 8, Some(BinOp::Ge)),
                TokenKind::And => (6, 7, Some(BinOp::And)),
                TokenKind::Or => (5, 6, Some(BinOp::Or)),
                TokenKind::Plus => (10, 11, Some(BinOp::Add)),
                TokenKind::Minus => (10, 11, Some(BinOp::Sub)),
                TokenKind::Star => (11, 12, Some(BinOp::Mul)),
                TokenKind::Slash => (11, 12, Some(BinOp::Div)),
                TokenKind::Percent => (11, 12, Some(BinOp::Mod)),
                TokenKind::Eq => (4, 3, None), // assignment, right-assoc
                TokenKind::As => {
                    self.advance();
                    let ty = self.parse_type()?;
                    lhs = Expr::AsCast { expr: Box::new(lhs), ty };
                    continue;
                }
                // v2.0: AssertUnwrap(!) and TryPropagate(?) suffix operators removed
                TokenKind::QuestionDot => {
                    self.advance();
                    let f = self.expect_ident("field")?;
                    lhs = Expr::MemberAccess(MemberAccess {
                        object: Box::new(lhs),
                        field: f,
                        optional: true,
                        span: self.span(),
                    });
                    continue;
                }
                _ => break,
            };
            if lbp < min {
                break;
            }
            self.advance();
            if let Some(binop) = op {
                let rhs = self.parse_binary(rbp)?;
                lhs = Expr::Binary(Box::new(lhs), binop, Box::new(rhs));
            } else {
                // assignment (right-associative): `name = expr`
                let rhs = self.parse_binary(rbp)?;
                match &lhs {
                    Expr::Ident(name) => {
                        lhs = Expr::Assign { name: name.clone(), value: Box::new(rhs) };
                    }
                    _ => {
                        // 非 ident 的左值 → Phase 1 不支持成员赋值
                        self.error("only simple variable assignment (name = expr) is supported");
                        lhs = rhs; // 回退：丢弃赋值语义
                    }
                }
            }
        }
        Some(lhs)
    }

    fn parse_unary(&mut self) -> Option<Expr> {
        match &self.cur {
            TokenKind::Bang => {
                self.advance();
                self.parse_unary().map(|e| Expr::Unary(UnaryOp::Not, Box::new(e)))
            }
            TokenKind::Minus => {
                self.advance();
                self.parse_unary().map(|e| Expr::Unary(UnaryOp::Neg, Box::new(e)))
            }
            TokenKind::Amp => {
                self.advance();
                self.parse_unary().map(|e| Expr::Reference(Box::new(e)))
            }
            _ => self.parse_postfix(),
        }
    }

    fn parse_postfix(&mut self) -> Option<Expr> {
        let mut e = self.parse_primary()?;
        loop {
            match &self.cur {
                TokenKind::LParen => {
                    self.advance();
                    let mut args = vec![];
                    if !matches!(self.cur, TokenKind::RParen) {
                        loop {
                            let mode = match &self.cur {
                                TokenKind::InOut => {
                                    self.advance();
                                    ParamMode::InOut
                                }
                                TokenKind::Move => {
                                    self.advance();
                                    ParamMode::Move
                                }
                                _ => ParamMode::Default,
                            };
                            let a = self.parse_expr()?;
                            args.push(CallArg { mode, expr: Box::new(a), span: self.span() });
                            if matches!(self.cur, TokenKind::Comma) {
                                self.advance();
                            } else {
                                break;
                            }
                        }
                    }
                    if !matches!(self.cur, TokenKind::RParen) {
                        self.error("expected )");
                    }
                    self.advance();
                    e = Expr::Call { callee: Box::new(e), args, span: self.span() };
                }
                TokenKind::Dot => {
                    self.advance();
                    let f = self.expect_ident("field")?;
                    e = Expr::MemberAccess(MemberAccess {
                        object: Box::new(e),
                        field: f,
                        optional: false,
                        span: self.span(),
                    });
                }
                _ => break,
            }
        }
        Some(e)
    }

    fn parse_primary(&mut self) -> Option<Expr> {
        match &self.cur {
            TokenKind::IntLiteral(n) => {
                let v = *n;
                self.advance();
                Some(Expr::IntLiteral(v))
            }
            TokenKind::FloatLiteral(n) => {
                let v = *n;
                self.advance();
                Some(Expr::FloatLiteral(v))
            }
            TokenKind::StrLiteral(s) => {
                let v = s.clone();
                self.advance();
                Some(Expr::StrLiteral(v))
            }
            TokenKind::True => {
                self.advance();
                Some(Expr::BoolLiteral(true))
            }
            TokenKind::False => {
                self.advance();
                Some(Expr::BoolLiteral(false))
            }
            TokenKind::Null => {
                self.advance();
                Some(Expr::Null)
            }
            TokenKind::Ident(s) => {
                let v = s.clone();
                self.advance();
                Some(Expr::Ident(v))
            }
            TokenKind::LParen => {
                self.advance();
                let e = self.parse_expr()?;
                if !matches!(self.cur, TokenKind::RParen) {
                    self.error("expected )");
                }
                self.advance();
                if matches!(self.cur, TokenKind::Arrow) {
                    // (x) => expr
                    self.advance();
                    let body = if matches!(self.cur, TokenKind::LBrace) {
                        ArrowBody::Block(self.parse_block()?)
                    } else {
                        ArrowBody::Expr(Box::new(self.parse_expr()?))
                    };
                    let params = match e {
                        Expr::Ident(n) => vec![Param {
                            name: n,
                            mode: ParamMode::Default,
                            ty: None,
                            optional: false,
                            span: self.span(),
                        }],
                        _ => {
                            self.error("expected param");
                            return None;
                        }
                    };
                    return Some(Expr::ArrowFn(ArrowFn {
                        params,
                        body,
                        is_move: false,
                        span: self.span(),
                    }));
                }
                Some(e) // parenthesized expr
            }
            TokenKind::LBrace => self.parse_block().map(Expr::BlockExpr),
            TokenKind::If => {
                let s = self.parse_if()?;
                match s {
                    Stmt::If(i) => Some(Expr::IfExpr(Box::new(i))),
                    _ => None,
                }
            }
            // v2.0: Loop removed
            TokenKind::Move => {
                self.advance();
                if !matches!(self.cur, TokenKind::LParen) {
                    self.error("expected (");
                    return None;
                }
                self.advance();
                let mut params = vec![];
                if !matches!(self.cur, TokenKind::RParen) {
                    loop {
                        let n = self.expect_ident("param")?;
                        let ty = if matches!(self.cur, TokenKind::Colon) {
                            self.advance();
                            self.parse_type()
                        } else {
                            None
                        };
                        params.push(Param {
                            name: n,
                            mode: ParamMode::Default,
                            ty,
                            optional: false,
                            span: self.span(),
                        });
                        if matches!(self.cur, TokenKind::Comma) {
                            self.advance();
                        } else {
                            break;
                        }
                    }
                }
                if !matches!(self.cur, TokenKind::RParen) {
                    self.error("expected )");
                }
                self.advance();
                if !matches!(self.cur, TokenKind::Arrow) {
                    self.error("expected =>");
                    return None;
                }
                self.advance();
                let body = if matches!(self.cur, TokenKind::LBrace) {
                    ArrowBody::Block(self.parse_block()?)
                } else {
                    ArrowBody::Expr(Box::new(self.parse_expr()?))
                };
                Some(Expr::ArrowFn(ArrowFn { params, body, is_move: true, span: self.span() }))
            }
            TokenKind::TemplateHead(s) => {
                let h = s.clone();
                self.advance();
                Some(self.parse_template(h))
            }
            TokenKind::TemplateInterpolation => {
                self.advance();
                Some(self.parse_template(String::new()))
            }
            _ => {
                self.error(&format!("unexpected {:?}", self.cur));
                None
            }
        }
    }

    fn parse_template(&mut self, head: String) -> Expr {
        let mut parts = vec![];
        if !head.is_empty() {
            parts.push(TemplatePart::Literal(head));
        }
        loop {
            // parse interpolation expression (after TemplateHead or TemplateInterpolation)
            if matches!(self.cur, TokenKind::TemplateInterpolation) {
                self.advance();
            }
            if self.can_expr_start() {
                if let Some(e) = self.parse_expr() {
                    parts.push(TemplatePart::Expr(Box::new(e)));
                }
            }
            // consume the closing } of ${expr}
            if matches!(self.cur, TokenKind::RBrace) {
                self.advance();
            }
            // resume template mode: collect text until next `${` or closing `` ` ``
            self.cur = self.lexer.resume_template();
            match &self.cur {
                TokenKind::TemplateTail(s) => {
                    if !s.is_empty() {
                        parts.push(TemplatePart::Literal(s.clone()));
                    }
                    self.advance();
                    break;
                }
                TokenKind::TemplateHead(s) => {
                    if !s.is_empty() {
                        parts.push(TemplatePart::Literal(s.clone()));
                    }
                    self.advance();
                    // loop — TemplateInterpolation follows
                }
                TokenKind::TemplateInterpolation => {
                    self.advance();
                    // loop — expression follows
                }
                _ => break,
            }
        }
        Expr::TemplateLiteral(parts)
    }
}

pub fn parse(src: &str) -> Result<Program, Vec<Diagnostic>> {
    let mut p = Parser::new(src, "main.trust");
    let prog = p.parse_program();
    if p.diagnostics.iter().any(|d| d.level == DiagLevel::Error) {
        Err(p.diagnostics)
    } else {
        Ok(prog)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(src: &str) -> Program {
        parse(src).expect("parse error")
    }
    fn s1(src: &str) -> Stmt {
        p(src).statements.into_iter().next().unwrap()
    }

    // AC-SYN-001~004
    #[test]
    fn syn001_let_simple() {
        match s1("let x = 42") {
            Stmt::Let(l) => assert!(!l.mutable),
            _ => panic!(),
        }
    }
    #[test]
    fn syn002_let_mut() {
        match s1("let mut y = 10") {
            Stmt::Let(l) => assert!(l.mutable),
            _ => panic!(),
        }
    }
    #[test]
    fn syn003_const() {
        assert!(matches!(s1("const MAX = 100"), Stmt::Const(_)));
    }
    #[test]
    fn syn005_function() {
        match s1("function add(a:number,b:number):number{return a+b}") {
            Stmt::Function(f) => {
                assert_eq!(f.name, "add");
                assert_eq!(f.params.len(), 2);
            }
            _ => panic!(),
        }
    }
    #[test]
    fn syn006_expr_shorthand() {
        match s1("function sq(x:number)=x*x") {
            Stmt::Function(f) => {
                assert_eq!(f.name, "sq");
                assert!(f.body.statements.len() > 0);
            }
            _ => panic!(),
        }
    }
    #[test]
    fn syn008_inout() {
        match s1("function push(inout arr:number[]){}") {
            Stmt::Function(f) => assert_eq!(f.params[0].mode, ParamMode::InOut),
            _ => panic!(),
        }
    }
    #[test]
    fn syn009_if_expr() {
        assert!(matches!(s1("if(x>0){1}else{0}"), Stmt::If(_)));
    }
    // v2.0: syn010_loop_break removed (loop removed)
    #[test]
    fn syn011_for_c() {
        assert!(matches!(s1("for(let i=0;i<10;i=i+1){}"), Stmt::For(_)));
    }
    #[test]
    fn syn012_for_of() {
        assert!(matches!(s1("for(let item of items){}"), Stmt::ForOf(_)));
    }
    #[test]
    fn syn020_import_named() {
        let prog = p("import {foo,bar} from \"./util\"");
        assert_eq!(prog.imports.len(), 1);
    }
    #[test]
    fn syn021_import_default() {
        let prog = p("import g from \"./g\"");
        match &prog.imports[0].kind {
            ImportKind::Default(n) => assert_eq!(n, "g"),
            _ => panic!(),
        }
    }
    #[test]
    fn syn022_import_ns() {
        let prog = p("import * as m from \"./m\"");
        match &prog.imports[0].kind {
            ImportKind::Namespace(n) => assert_eq!(n, "m"),
            _ => panic!(),
        }
    }
    #[test]
    fn syn023_export() {
        assert_eq!(p("export function f(){}").exports.len(), 1);
    }
    #[test]
    fn syn030_arrow() {
        let st = s1("let f=(x)=>x*2");
        match st {
            Stmt::Let(l) => assert!(matches!(*l.init, Expr::ArrowFn(_))),
            _ => panic!(),
        }
    }
    #[test]
    fn syn031_move_closure() {
        let st = s1("let c=move()=>process()");
        match st {
            Stmt::Let(l) => match *l.init {
                Expr::ArrowFn(a) => assert!(a.is_move),
                _ => panic!(),
            },
            _ => panic!(),
        }
    }
    #[test]
    fn syn036_ref() {
        let st = s1("let r=&data");
        match st {
            Stmt::Let(l) => assert!(matches!(*l.init, Expr::Reference(_))),
            _ => panic!(),
        }
    }
    // v2.0: syn037_bang and syn038_try removed (AssertUnwrap/TryPropagate removed)
    #[test]
    fn syn039_opt_chain() {
        let st = s1("let s=user?.addr?.street");
        match st {
            Stmt::Let(l) => assert!(matches!(*l.init, Expr::MemberAccess(_))),
            _ => panic!(),
        }
    }
    #[test]
    fn syn040_nullish() {
        let st = s1("let n=maybeName??\"anon\"");
        match st {
            Stmt::Let(l) => match *l.init {
                Expr::Binary(_, BinOp::QuestionQuestion, _) => (),
                _ => panic!(),
            },
            _ => panic!(),
        }
    }
    #[test]
    fn sep001_newline() {
        assert_eq!(p("let x=42\nlet y=10").statements.len(), 2);
    }
    #[test]
    fn sep002_block_ret() {
        match s1("let x={let y=2;y}") {
            Stmt::Let(l) => assert!(matches!(*l.init, Expr::BlockExpr(_))),
            _ => panic!(),
        }
    }
    #[test]
    fn sep004_ret_newline() {
        assert!(p("function f(){return 42\nlet x=1}").statements.len() == 1);
    }
    #[test]
    fn type_annotation() {
        match s1("let x:number=42") {
            Stmt::Let(l) => assert_eq!(l.ty, Some(Type::NumberType)),
            _ => panic!(),
        }
    }
    #[test]
    fn as_cast() {
        match s1("let c=a as f64+b") {
            Stmt::Let(l) => assert!(matches!(*l.init, Expr::Binary(_, _, _))),
            _ => panic!(),
        }
    }
    #[test]
    fn while_loop() {
        assert!(matches!(s1("while(x>0){x=x-1}"), Stmt::While(_)));
    }
    #[test]
    fn return_stmt() {
        assert!(matches!(s1("return 42"), Stmt::Return(_)));
    }
    #[test]
    fn break_stmt() {
        assert!(matches!(s1("break"), Stmt::Break(_)));
    }
    #[test]
    fn continue_stmt() {
        assert!(matches!(s1("continue"), Stmt::Continue(_)));
    }
}
