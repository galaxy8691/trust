//! 磁盘缓存用的 IR 快照（Span 为相对源文件起点的 byte 偏移）。

use serde::{Deserialize, Serialize};

use crate::ir::{
    BinaryKind, ForInKind, HttpResponseMember, HttpResponseMethodKind, IRBinOp, IRUnaryOp,
    IndexKind, JsonBuiltinKind, MathBuiltinKind, MemberLengthDispatch, NumberBuiltinKind,
    ObjectMemberAccessKind, StreamReadResultMember, StringMethodKind, TsType, UriBuiltinKind,
};

pub type DSpan = (u32, u32);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DiskTplPart {
    Static(String),
    Interp(Box<DiskIRExpr>),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiskFetchInit {
    pub method: Option<String>,
    pub headers: Vec<(String, String)>,
    pub body: Option<Box<DiskIRExpr>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DiskIRExpr {
    Number(f64, DSpan),
    Bool(bool, DSpan),
    Str(String, DSpan),
    Ident(String, DSpan),
    Binary {
        op: IRBinOp,
        left: Box<DiskIRExpr>,
        right: Box<DiskIRExpr>,
        span: DSpan,
        kind: Option<BinaryKind>,
    },
    Unary {
        op: IRUnaryOp,
        arg: Box<DiskIRExpr>,
        span: DSpan,
    },
    Call {
        callee: String,
        args: Vec<DiskIRExpr>,
        type_args: Vec<TsType>,
        span: DSpan,
    },
    MethodCall {
        receiver: Box<DiskIRExpr>,
        method: String,
        args: Vec<DiskIRExpr>,
        type_args: Vec<TsType>,
        span: DSpan,
        inherent_rust: Option<String>,
        inherent_rust_str_ref: Option<Vec<bool>>,
        #[serde(default)]
        inherent_rust_result_to_string: bool,
    },
    OptionalCall {
        callee: String,
        args: Vec<DiskIRExpr>,
        type_args: Vec<TsType>,
        span: DSpan,
    },
    OptionalMethodCall {
        receiver: Box<DiskIRExpr>,
        method: String,
        args: Vec<DiskIRExpr>,
        type_args: Vec<TsType>,
        span: DSpan,
        inherent_rust: Option<String>,
        inherent_rust_str_ref: Option<Vec<bool>>,
        #[serde(default)]
        inherent_rust_result_to_string: bool,
    },
    RustNew {
        result_ty: TsType,
        rust_fn_path: String,
        unwrap_result: bool,
        args: Vec<DiskIRExpr>,
        span: DSpan,
    },
    BuiltinLog {
        args: Vec<DiskIRExpr>,
        stderr: bool,
        span: DSpan,
    },
    Conditional {
        test: Box<DiskIRExpr>,
        cons: Box<DiskIRExpr>,
        alt: Box<DiskIRExpr>,
        span: DSpan,
        cond_ty: Option<TsType>,
    },
    Seq {
        exprs: Vec<DiskIRExpr>,
        span: DSpan,
    },
    Tpl {
        parts: Vec<DiskTplPart>,
        span: DSpan,
    },
    Member {
        obj: Box<DiskIRExpr>,
        prop: String,
        span: DSpan,
        length_dispatch: Option<MemberLengthDispatch>,
        http_response_member: Option<HttpResponseMember>,
        stream_read_member: Option<StreamReadResultMember>,
        object_member_access: Option<ObjectMemberAccessKind>,
    },
    Null(DSpan),
    Undefined(DSpan),
    NullishCoalesce {
        left: Box<DiskIRExpr>,
        right: Box<DiskIRExpr>,
        span: DSpan,
    },
    OptionalMember {
        obj: Box<DiskIRExpr>,
        prop: String,
        span: DSpan,
        length_dispatch: Option<MemberLengthDispatch>,
        http_response_member: Option<HttpResponseMember>,
        stream_read_member: Option<StreamReadResultMember>,
        object_member_access: Option<ObjectMemberAccessKind>,
    },
    MathBuiltin {
        kind: MathBuiltinKind,
        args: Vec<DiskIRExpr>,
        span: DSpan,
    },
    NumberBuiltin {
        kind: NumberBuiltinKind,
        args: Vec<DiskIRExpr>,
        span: DSpan,
    },
    JsonBuiltin {
        kind: JsonBuiltinKind,
        args: Vec<DiskIRExpr>,
        span: DSpan,
        stringify_inferred_ty: Option<TsType>,
    },
    UriBuiltin {
        kind: UriBuiltinKind,
        args: Vec<DiskIRExpr>,
        span: DSpan,
    },
    StringMethodBuiltin {
        kind: StringMethodKind,
        receiver: Box<DiskIRExpr>,
        args: Vec<DiskIRExpr>,
        span: DSpan,
    },
    ReadStdinLine {
        span: DSpan,
    },
    ReadFileText {
        path: Box<DiskIRExpr>,
        span: DSpan,
    },
    ReadFileTextAsync {
        path: Box<DiskIRExpr>,
        span: DSpan,
    },
    ArrayLit {
        elems: Vec<DiskIRExpr>,
        span: DSpan,
    },
    ObjectLit {
        fields: Vec<(String, DiskIRExpr)>,
        span: DSpan,
    },
    Index {
        obj: Box<DiskIRExpr>,
        index: Box<DiskIRExpr>,
        span: DSpan,
        index_kind: Option<IndexKind>,
    },
    ArrowFn {
        params: Vec<(String, TsType)>,
        ret: TsType,
        body: Vec<DiskIRStmt>,
        span: DSpan,
    },
    This(DSpan),
    Super(DSpan),
    Await {
        arg: Box<DiskIRExpr>,
        span: DSpan,
    },
    FetchText {
        url: Box<DiskIRExpr>,
        span: DSpan,
    },
    Fetch {
        url: Box<DiskIRExpr>,
        init: Option<DiskFetchInit>,
        span: DSpan,
    },
    HttpResponseMethodBuiltin {
        kind: HttpResponseMethodKind,
        receiver: Box<DiskIRExpr>,
        span: DSpan,
    },
    HttpResponseBodyGetReader {
        response: Box<DiskIRExpr>,
        span: DSpan,
        stream_slot: Option<u32>,
    },
    ReaderRead {
        reader_name: String,
        span: DSpan,
        reader_slot: Option<u32>,
    },
    PromiseAll {
        elems: Vec<DiskIRExpr>,
        span: DSpan,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DiskIRStmt {
    Empty {
        span: DSpan,
    },
    Let {
        name: String,
        ty: TsType,
        init: Option<DiskIRExpr>,
        mutable: bool,
        span: DSpan,
    },
    Assign {
        name: String,
        rhs: DiskIRExpr,
        span: DSpan,
    },
    MemberAssign {
        obj: String,
        prop: String,
        rhs: DiskIRExpr,
        span: DSpan,
    },
    Expr {
        expr: DiskIRExpr,
        span: DSpan,
    },
    Return {
        arg: Option<DiskIRExpr>,
        span: DSpan,
    },
    Block {
        stmts: Vec<DiskIRStmt>,
        span: DSpan,
    },
    If {
        cond: DiskIRExpr,
        cond_ty: TsType,
        then_b: Vec<DiskIRStmt>,
        else_b: Option<Vec<DiskIRStmt>>,
        span: DSpan,
    },
    While {
        cond: DiskIRExpr,
        cond_ty: TsType,
        body: Vec<DiskIRStmt>,
        span: DSpan,
    },
    ForIn {
        key: String,
        key_ty: TsType,
        target: DiskIRExpr,
        kind: Option<ForInKind>,
        body: Vec<DiskIRStmt>,
        span: DSpan,
    },
    DoWhile {
        body: Vec<DiskIRStmt>,
        cond: DiskIRExpr,
        cond_ty: TsType,
        span: DSpan,
    },
    Break {
        span: DSpan,
    },
    Continue {
        span: DSpan,
    },
    FnDecl {
        func: Box<DiskIRFunction>,
        span: DSpan,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiskIRFunction {
    pub ir_id: u32,
    pub name: String,
    pub type_params: Vec<String>,
    pub params: Vec<(String, TsType)>,
    pub ret: TsType,
    pub body: Vec<DiskIRStmt>,
    pub span: DSpan,
    pub source_path: String,
    pub mono_origin: Option<String>,
    pub is_async: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiskIRClassMethod {
    pub name: String,
    pub params: Vec<(String, TsType)>,
    pub ret: TsType,
    pub body: Vec<DiskIRStmt>,
    pub is_override: bool,
    pub owner: String,
    pub span: DSpan,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiskIRClass {
    pub name: String,
    pub extends: Option<String>,
    pub fields: Vec<(String, TsType)>,
    pub ctor: Option<DiskIRClassMethod>,
    pub methods: Vec<DiskIRClassMethod>,
    pub span: DSpan,
    pub source_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiskModuleFragment {
    pub schema: u32,
    pub fns: Vec<DiskIRFunction>,
    pub classes: Vec<DiskIRClass>,
    pub ts_comments: Option<crate::ir::TsLeadingComments>,
    pub exported_types: std::collections::HashMap<String, crate::ir::TsType>,
}
