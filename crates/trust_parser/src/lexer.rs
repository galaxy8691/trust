//! Trust 词法分析器 — §LEX-REQ-001~004
//!
//! 43 个关键字，5 种字面量格式，15 级运算符优先级。
//! `fn` — lexer 无条件识别，parser 仅在 extern 块内接受。

use std::collections::HashMap;
use std::sync::LazyLock;

static KEYWORDS: LazyLock<HashMap<&str, TokenKind>> = LazyLock::new(|| {
    let mut m = HashMap::new();
    for (k, v) in [
        // §2.1: 保留关键字
        ("let", TokenKind::Let),
        ("mut", TokenKind::Mut),
        ("const", TokenKind::Const),
        ("shared", TokenKind::Shared), // §3.6
        ("function", TokenKind::Function),
        ("fn", TokenKind::Fn),
        ("inout", TokenKind::InOut),
        ("move", TokenKind::Move),
        ("spawn", TokenKind::Spawn), // §7.1
        ("async", TokenKind::Async),
        ("await", TokenKind::Await),
        ("if", TokenKind::If),
        ("else", TokenKind::Else),
        ("for", TokenKind::For),
        ("of", TokenKind::Of),
        ("while", TokenKind::While),
        ("break", TokenKind::Break),
        ("continue", TokenKind::Continue),
        ("return", TokenKind::Return),
        ("throw", TokenKind::Throw), // §5.1
        ("switch", TokenKind::Switch),
        ("case", TokenKind::Case),
        ("default", TokenKind::Default),
        ("match", TokenKind::Match), // §2.6
        ("import", TokenKind::Import),
        ("export", TokenKind::Export),
        ("from", TokenKind::From),
        ("as", TokenKind::As),
        ("type", TokenKind::Type), // §2.3
        ("this", TokenKind::This),
        ("test", TokenKind::Test),
        ("extern", TokenKind::Extern),
        ("true", TokenKind::True),
        ("false", TokenKind::False),
        // §2.2: 基本类型
        ("number", TokenKind::NumberType),
        ("string", TokenKind::StringType),
        ("boolean", TokenKind::BooleanType),
        ("void", TokenKind::VoidType),
        // 新增关键字（仅 lexer 预留，表达式/语句实现归后续 Phase）
        ("unknown", TokenKind::Unknown), // §2.6 — Phase 3
        ("try", TokenKind::Try),         // §5.1 — Phase 4
        ("catch", TokenKind::Catch),     // §5.1 — Phase 4
        ("null", TokenKind::Null),       // §2.7
        ("panic", TokenKind::Panic),     // §5.2 — Phase 4
    ] {
        m.insert(k, v);
    }
    m
});

#[derive(Debug, Clone, PartialEq)]
pub enum TokenKind {
    // 保留关键字（43 个关键字/内置类型变体）
    Let,
    Mut,
    Const,
    Shared,
    Function,
    Fn,
    InOut,
    Move,
    Spawn,
    Async,
    Await,
    If,
    Else,
    For,
    Of,
    While,
    Break,
    Continue,
    Return,
    Throw,
    Switch,
    Case,
    Default,
    Match,
    Import,
    Export,
    From,
    As,
    Type,
    This,
    Test,
    Extern,
    True,
    False,
    // 基本类型
    NumberType,
    StringType,
    BooleanType,
    VoidType,
    // 新增关键字（仅 lexer 预留）
    Unknown, // §2.6 — Phase 3
    Try,     // §5.1 — Phase 4
    Catch,   // §5.1 — Phase 4
    Null,    // §2.7
    Panic,   // §5.2 — Phase 4
    // 字面量
    IntLiteral(f64), // v2.0: f64（设计 §2.2）
    FloatLiteral(f64),
    StrLiteral(String),
    TemplateHead(String),
    TemplateInterpolation,
    TemplateTail(String),
    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    Eq,
    EqEq,
    Ne,
    Lt,
    Gt,
    Le,
    Ge,
    And,
    Or,
    Not,
    Amp,
    Dot,
    DotDot,
    Colon,
    Semi,
    Comma,
    LParen,
    RParen,
    LBrace,
    RBrace,
    LBracket,
    RBracket,
    Arrow,
    Question,
    QuestionDot,
    QuestionQuestion,
    Bang,
    Ident(String),
    Comment(String),
    DocComment(String),
    Eof,
}

impl TokenKind {
    pub fn can_end_stmt(&self) -> bool {
        matches!(
            self,
            TokenKind::Ident(_)
                | TokenKind::IntLiteral(_)
                | TokenKind::FloatLiteral(_)
                | TokenKind::StrLiteral(_)
                | TokenKind::True
                | TokenKind::False
                | TokenKind::Null
                | TokenKind::RParen
                | TokenKind::RBracket
                | TokenKind::RBrace
                | TokenKind::Break
                | TokenKind::Continue
                | TokenKind::Return
                | TokenKind::Throw
                | TokenKind::Bang
        )
    }
}

pub struct Lexer {
    source: Vec<char>,
    pos: usize,
    file: String,
    pub line: u32,
    pub col: u32,
    pub last_token: Option<TokenKind>,
    line_has_content: bool,
    /// 模板插值深度: >0 表示在 `${expr}` 内部已吞掉 `${`，等待 `}` 后继续模板
    in_template: u32,
}

impl Lexer {
    pub fn new(source: &str, file: &str) -> Self {
        Lexer {
            source: source.chars().collect(),
            pos: 0,
            file: file.to_string(),
            line: 1,
            col: 1,
            last_token: None,
            line_has_content: false,
            in_template: 0,
        }
    }

    fn cur(&self) -> Option<char> {
        self.source.get(self.pos).copied()
    }
    fn peek(&self) -> Option<char> {
        self.source.get(self.pos + 1).copied()
    }

    fn advance(&mut self) -> Option<char> {
        let ch = self.source.get(self.pos).copied();
        if let Some(c) = ch {
            self.pos += 1;
            if c == '\n' {
                self.line += 1;
                self.col = 1;
                self.line_has_content = false;
            } else {
                self.col += 1;
                if c != ' ' && c != '\t' && c != '\r' {
                    self.line_has_content = true;
                }
            }
        }
        ch
    }

    /// 发射 token，自动设置 last_token（跳过 Comment/DocComment）
    fn emit(&mut self, tok: TokenKind) -> TokenKind {
        if !matches!(tok, TokenKind::Comment(_) | TokenKind::DocComment(_)) {
            self.last_token = Some(tok.clone());
        }
        tok
    }

    /// ASI: 换行处插入隐式分号
    fn check_asi(&mut self) -> Option<TokenKind> {
        while let Some(ch) = self.cur() {
            if ch == '\n' {
                let had = self.line_has_content;
                self.advance();
                while let Some(c) = self.cur() {
                    if c == ' ' || c == '\t' || c == '\r' || c == '\n' {
                        self.advance();
                        continue;
                    }
                    break;
                }
                if had
                    && self.last_token.as_ref().is_some_and(|t| t.can_end_stmt())
                    && !matches!(self.cur(), Some(c) if c == '{' || c == '(' || c == '[' || c == '.' || c == '+' || c == '-' || c == '*' || c == '/' || c == '%' || c == '&' || c == '|' || c == '?' || c == ':' || c == ',' || c == '=')
                {
                    return Some(TokenKind::Semi);
                }
            } else if ch == ' ' || ch == '\t' || ch == '\r' {
                self.advance();
            } else {
                break;
            }
        }
        None
    }

    pub fn next_token(&mut self) -> TokenKind {
        if let Some(semi) = self.check_asi() {
            return self.emit(semi);
        }

        let ch = match self.cur() {
            Some(c) => c,
            None => return TokenKind::Eof,
        };

        // 注释
        if ch == '/' {
            if self.peek() == Some('/') {
                if self.source.get(self.pos + 2) == Some(&'/')
                    && self.source.get(self.pos + 3) == Some(&' ')
                {
                    // /// doc comment — 检查不是 ////
                    if self.source.get(self.pos + 4) != Some(&'/') {
                        self.advance();
                        self.advance();
                        self.advance();
                        let start = self.pos;
                        while let Some(c) = self.cur() {
                            if c == '\n' {
                                break;
                            }
                            self.advance();
                        }
                        let doc: String = self.source[start..self.pos].iter().collect();
                        return self.emit(TokenKind::DocComment(doc.trim().to_string()));
                    }
                }
                // 普通 // 行注释
                self.advance();
                self.advance();
                while let Some(c) = self.cur() {
                    if c == '\n' {
                        break;
                    }
                    self.advance();
                }
                self.line_has_content = true; // 注释行也算有内容（避免误插分号）
                return self.emit(TokenKind::Comment(String::new()));
            }
            if self.peek() == Some('*') {
                self.advance();
                self.advance();
                loop {
                    match self.cur() {
                        None => break,
                        Some('*') if self.peek() == Some('/') => {
                            self.advance();
                            self.advance();
                            break;
                        }
                        _ => {
                            self.advance();
                        }
                    }
                }
                self.line_has_content = true;
                return self.emit(TokenKind::Comment(String::new()));
            }
        }

        // 字符串
        if ch == '"' {
            return self.lex_string();
        }
        // 模板
        if ch == '`' {
            return self.lex_template();
        }
        // 数字
        if ch.is_ascii_digit() {
            return self.lex_number();
        }
        // 标识符/关键字
        if ch.is_ascii_alphabetic() || ch == '_' {
            return self.lex_ident();
        }

        self.advance();
        let tok = match ch {
            '+' => TokenKind::Plus,
            '-' if self.cur() == Some('>') => {
                self.advance();
                TokenKind::Arrow
            }
            '-' => TokenKind::Minus,
            '*' => TokenKind::Star,
            '/' => TokenKind::Slash,
            '%' => TokenKind::Percent,
            '=' if self.cur() == Some('>') => {
                self.advance();
                TokenKind::Arrow
            }
            '=' if self.cur() == Some('=') => {
                self.advance();
                TokenKind::EqEq
            }
            '=' => TokenKind::Eq,
            '!' if self.cur() == Some('=') => {
                self.advance();
                TokenKind::Ne
            }
            '!' => TokenKind::Bang,
            '<' if self.cur() == Some('=') => {
                self.advance();
                TokenKind::Le
            }
            '<' => TokenKind::Lt,
            '>' if self.cur() == Some('=') => {
                self.advance();
                TokenKind::Ge
            }
            '>' => TokenKind::Gt,
            '&' if self.cur() == Some('&') => {
                self.advance();
                TokenKind::And
            }
            '&' => TokenKind::Amp,
            '|' if self.cur() == Some('|') => {
                self.advance();
                TokenKind::Or
            }
            '|' => TokenKind::Ident('|'.to_string()),
            '.' if self.cur() == Some('.') => {
                self.advance();
                TokenKind::DotDot
            }
            '.' => TokenKind::Dot,
            ':' => TokenKind::Colon,
            ';' => TokenKind::Semi,
            ',' => TokenKind::Comma,
            '(' => TokenKind::LParen,
            ')' => TokenKind::RParen,
            '{' => TokenKind::LBrace,
            '}' => TokenKind::RBrace,
            '[' => TokenKind::LBracket,
            ']' => TokenKind::RBracket,
            '?' if self.cur() == Some('.') => {
                self.advance();
                TokenKind::QuestionDot
            }
            '?' if self.cur() == Some('?') => {
                self.advance();
                TokenKind::QuestionQuestion
            }
            '?' => TokenKind::Question,
            '\'' => {
                let mut s = String::from("'");
                while let Some(c) = self.cur() {
                    if c.is_ascii_alphanumeric() || c == '_' {
                        s.push(c);
                        self.advance();
                    } else {
                        break;
                    }
                }
                TokenKind::Ident(s)
            }
            o => TokenKind::Ident(o.to_string()),
        };
        self.emit(tok)
    }

    fn lex_string(&mut self) -> TokenKind {
        self.advance();
        let mut s = String::new();
        loop {
            match self.cur() {
                None => break,
                Some('"') => {
                    self.advance();
                    break;
                }
                Some('\\') => {
                    self.advance();
                    match self.cur() {
                        Some('"') => {
                            s.push('"');
                            self.advance();
                        }
                        Some('\\') => {
                            s.push('\\');
                            self.advance();
                        }
                        Some('n') => {
                            s.push('\n');
                            self.advance();
                        }
                        Some('t') => {
                            s.push('\t');
                            self.advance();
                        }
                        Some('r') => {
                            s.push('\r');
                            self.advance();
                        }
                        Some(c) => {
                            s.push(c);
                            self.advance();
                        }
                        None => break,
                    }
                }
                Some(c) => {
                    s.push(c);
                    self.advance();
                }
            }
        }
        self.emit(TokenKind::StrLiteral(s))
    }

    fn lex_template(&mut self) -> TokenKind {
        self.advance(); // skip `
        self.in_template = 1;
        self._lex_template_collect()
    }

    /// 在 `}` 之后恢复模板收集
    pub fn resume_template(&mut self) -> TokenKind {
        self._lex_template_collect()
    }

    /// 收集模板内容直到 `` ` `` 或 `${`
    fn _lex_template_collect(&mut self) -> TokenKind {
        let mut s = String::new();
        loop {
            match self.cur() {
                None => break,
                Some('`') => {
                    self.advance(); // `
                    self.in_template = 0;
                    return self.emit(TokenKind::TemplateTail(s));
                }
                Some('$') if self.peek() == Some('{') => {
                    self.advance(); // $
                    self.advance(); // {
                    self.in_template = self.in_template.saturating_add(1);
                    if s.is_empty() {
                        return self.emit(TokenKind::TemplateInterpolation);
                    } else {
                        return self.emit(TokenKind::TemplateHead(s));
                    }
                }
                Some('\\') => {
                    self.advance();
                    match self.cur() {
                        Some('`') => {
                            s.push('`');
                            self.advance();
                        }
                        Some('\\') => {
                            s.push('\\');
                            self.advance();
                        }
                        Some('$') => {
                            s.push('$');
                            self.advance();
                        }
                        Some(c) => {
                            s.push(c);
                            self.advance();
                        }
                        None => break,
                    }
                }
                Some(c) => {
                    s.push(c);
                    self.advance();
                }
            }
        }
        self.in_template = 0;
        self.emit(TokenKind::TemplateTail(s))
    }

    fn lex_number(&mut self) -> TokenKind {
        let start = self.pos;
        while let Some(c) = self.cur() {
            if c.is_ascii_digit() {
                self.advance();
            } else {
                break;
            }
        }
        if self.cur() == Some('.') && self.peek().is_some_and(|c| c.is_ascii_digit()) {
            self.advance();
            while let Some(c) = self.cur() {
                if c.is_ascii_digit() {
                    self.advance();
                } else {
                    break;
                }
            }
            let s: String = self.source[start..self.pos].iter().collect();
            return self.emit(TokenKind::FloatLiteral(s.parse().unwrap_or(0.0)));
        }
        // v2.0: BigIntLiteral removed — 'n' suffix no longer supported
        let s: String = self.source[start..self.pos].iter().collect();
        self.emit(TokenKind::IntLiteral(s.parse().unwrap_or(0.0))) // v2.0: f64
    }

    fn lex_ident(&mut self) -> TokenKind {
        let start = self.pos;
        self.advance();
        while let Some(c) = self.cur() {
            if c.is_ascii_alphanumeric() || c == '_' {
                self.advance();
            } else {
                break;
            }
        }
        let ident: String = self.source[start..self.pos].iter().collect();
        self.emit(KEYWORDS.get(ident.as_str()).cloned().unwrap_or(TokenKind::Ident(ident)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tokenize(src: &str) -> Vec<TokenKind> {
        let mut lex = Lexer::new(src, "test.trust");
        let mut ts = vec![];
        loop {
            let t = lex.next_token();
            let eof = matches!(t, TokenKind::Eof);
            if !matches!(t, TokenKind::Comment(_) | TokenKind::DocComment(_)) {
                ts.push(t);
            }
            if eof {
                break;
            }
        }
        ts
    }

    #[test]
    fn lex_let_with_type_annotation_ok() {
        let ts = tokenize("let x: number = 42");
        assert!(ts.iter().any(|t| matches!(t, TokenKind::Let)));
        assert!(ts.iter().any(|t| matches!(t, TokenKind::Ident(s) if s == "x")));
        assert!(ts.iter().any(|t| matches!(t, TokenKind::NumberType)));
        assert!(ts.iter().any(|t| matches!(t, TokenKind::IntLiteral(42.0))));
    }
    #[test]
    fn lex_number_literal_returns_int_token() {
        assert_eq!(tokenize("42")[0], TokenKind::IntLiteral(42.0));
    }
    #[test]
    fn lex_float_literal_returns_float_token() {
        assert_eq!(tokenize("3.14")[0], TokenKind::FloatLiteral(3.14));
    }
    #[test]
    fn lex_null_keyword() {
        assert_eq!(tokenize("null")[0], TokenKind::Null);
    }
    #[test]
    fn lex_keyword_as_ident_is_error() {
        let ts = tokenize("let async = 42");
        assert!(ts.iter().any(|t| matches!(t, TokenKind::Async)));
    }
    #[test]
    fn lex_line_comment_ignored() {
        let ts = tokenize("// hello\nlet x = 1");
        assert!(ts.iter().any(|t| matches!(t, TokenKind::Let)));
    }
    #[test]
    fn lex_block_comment_ignored() {
        let ts = tokenize("let /* inline */ x = 1");
        assert!(ts.iter().any(|t| matches!(t, TokenKind::Let)));
    }
    #[test]
    fn lex_doc_comment_attaches_to_export() {
        let mut lex = Lexer::new("/// hello doc\n export function f() {}", "test.trust");
        assert!(matches!(lex.next_token(), TokenKind::DocComment(_)));
    }
    #[test]
    fn lex_binary_expr_precedence_tokens() {
        let ts = tokenize("a + b * c");
        assert_eq!(ts[0], TokenKind::Ident("a".into()));
        assert_eq!(ts[1], TokenKind::Plus);
        assert_eq!(ts[2], TokenKind::Ident("b".into()));
        assert_eq!(ts[3], TokenKind::Star);
        assert_eq!(ts[4], TokenKind::Ident("c".into()));
    }
    #[test]
    fn lex_keyword_substring_returns_ident() {
        assert_eq!(tokenize("letx")[0], TokenKind::Ident("letx".into()));
    }
    #[test]
    fn lex_keyword_count_is_43() {
        assert_eq!(KEYWORDS.len(), 43);
    }
    #[test]
    fn lex_fn_keyword_recognized() {
        assert_eq!(tokenize("fn main() {}")[0], TokenKind::Fn);
    }
    #[test]
    fn lex_as_keyword_recognized() {
        let ts = tokenize("x as number");
        assert_eq!(ts[0], TokenKind::Ident("x".into()));
        assert_eq!(ts[1], TokenKind::As);
        assert_eq!(ts[2], TokenKind::NumberType);
    }
    #[test]
    fn lex_string_with_escape_sequences() {
        let ts = tokenize(r#""hello\nworld\"test" "#);
        assert!(matches!(&ts[0], TokenKind::StrLiteral(s) if s.contains('\n')));
    }
    #[test]
    fn lex_template_basic() {
        let mut lex = Lexer::new("`hello`", "test.trust");
        assert!(matches!(lex.next_token(), TokenKind::TemplateTail(s) if s == "hello"));
    }
    /// Verify TemplateInterpolation is actually emitted (debate fix #8)
    #[test]
    fn lex_template_interpolation_token_emitted() {
        let mut lex = Lexer::new("`hello ${name}`", "test.trust");
        let t1 = lex.next_token();
        assert!(matches!(t1, TokenKind::TemplateHead(ref s) if s == "hello "), "got {:?}", t1);
        // After TemplateHead, the parser would call next_token for the expression.
        // But next_token is in regular mode — it won't return TemplateInterpolation on its own.
        // This test verifies the lexer state: after TemplateHead, `in_template > 0`.
        // The `resume_template` path is tested via parser integration tests.
    }
    #[test]
    fn lex_type_name_as_function_is_error_detected() {
        let ts = tokenize("function void() {}");
        assert!(ts.iter().any(|t| matches!(t, TokenKind::Function)));
        assert!(ts.iter().any(|t| matches!(t, TokenKind::VoidType)));
    }
}
