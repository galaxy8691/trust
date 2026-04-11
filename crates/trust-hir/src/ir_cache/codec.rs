//! HIR 片段 ↔ 磁盘快照（相对 Span）。

use std::path::Path;

use swc_common::{sync::Lrc, BytePos, FileName, SourceMap, Span};

use crate::build::ModuleIrFragment;
use crate::ir::*;
use crate::ir_cache::disk::*;

pub const SCHEMA_VERSION: u32 = 6;

#[derive(Debug)]
pub enum IrCacheError {
    Bincode(String),
    Schema(u32),
}

impl std::fmt::Display for IrCacheError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            IrCacheError::Bincode(s) => write!(f, "ir cache bincode: {s}"),
            IrCacheError::Schema(v) => write!(f, "ir cache schema mismatch: {v}"),
        }
    }
}

impl std::error::Error for IrCacheError {}

pub fn source_map_for_path(path: &Path, src: &str) -> (Lrc<SourceMap>, u32) {
    let cm: Lrc<SourceMap> = Default::default();
    let fm = cm.new_source_file(FileName::Real(path.to_path_buf()).into(), src.to_string());
    let base = fm.start_pos.0;
    (cm, base)
}

fn fragment_primary_cm(frag: &ModuleIrFragment) -> Lrc<SourceMap> {
    frag.fns
        .first()
        .map(|f| f.cm.0.clone())
        .or_else(|| frag.classes.first().map(|c| c.cm.0.clone()))
        .unwrap_or_else(|| Lrc::new(SourceMap::default()))
}

fn to_dspan(cm: &SourceMap, sp: Span) -> DSpan {
    let b = cm.lookup_source_file(sp.lo).start_pos.0;
    (sp.lo.0.saturating_sub(b), sp.hi.0.saturating_sub(b))
}

fn from_dspan(base: u32, d: DSpan) -> Span {
    Span::new(BytePos(base + d.0), BytePos(base + d.1))
}

fn encode_fetch_init(cm: &SourceMap, x: &FetchInit) -> DiskFetchInit {
    DiskFetchInit {
        method: x.method.clone(),
        headers: x.headers.clone(),
        body: x.body.as_ref().map(|b| Box::new(encode_expr(cm, b))),
    }
}

fn decode_fetch_init(cm: &Lrc<SourceMap>, base: u32, x: DiskFetchInit) -> FetchInit {
    FetchInit {
        method: x.method,
        headers: x.headers,
        body: x.body.map(|b| Box::new(decode_expr(cm, base, *b))),
    }
}

fn encode_tpl_part(cm: &SourceMap, p: &TplPart) -> DiskTplPart {
    match p {
        TplPart::Static(s) => DiskTplPart::Static(s.clone()),
        TplPart::Interp(e) => DiskTplPart::Interp(Box::new(encode_expr(cm, e))),
    }
}

fn decode_tpl_part(cm: &Lrc<SourceMap>, base: u32, p: DiskTplPart) -> TplPart {
    match p {
        DiskTplPart::Static(s) => TplPart::Static(s),
        DiskTplPart::Interp(e) => TplPart::Interp(Box::new(decode_expr(cm, base, *e))),
    }
}

fn encode_expr(cm: &SourceMap, e: &IRExpr) -> DiskIRExpr {
    match e {
        IRExpr::Number(n, sp) => DiskIRExpr::Number(*n, to_dspan(cm, *sp)),
        IRExpr::Bool(b, sp) => DiskIRExpr::Bool(*b, to_dspan(cm, *sp)),
        IRExpr::Str(s, sp) => DiskIRExpr::Str(s.clone(), to_dspan(cm, *sp)),
        IRExpr::Ident(s, sp) => DiskIRExpr::Ident(s.clone(), to_dspan(cm, *sp)),
        IRExpr::Binary {
            op,
            left,
            right,
            span,
            kind,
        } => DiskIRExpr::Binary {
            op: *op,
            left: Box::new(encode_expr(cm, left)),
            right: Box::new(encode_expr(cm, right)),
            span: to_dspan(cm, *span),
            kind: *kind,
        },
        IRExpr::Unary { op, arg, span } => DiskIRExpr::Unary {
            op: *op,
            arg: Box::new(encode_expr(cm, arg)),
            span: to_dspan(cm, *span),
        },
        IRExpr::Call {
            callee,
            args,
            type_args,
            span,
        } => DiskIRExpr::Call {
            callee: callee.clone(),
            args: args.iter().map(|a| encode_expr(cm, a)).collect(),
            type_args: type_args.clone(),
            span: to_dspan(cm, *span),
        },
        IRExpr::MethodCall {
            receiver,
            method,
            args,
            type_args,
            span,
            inherent_rust,
            inherent_rust_str_ref,
            inherent_rust_result_to_string,
        } => DiskIRExpr::MethodCall {
            receiver: Box::new(encode_expr(cm, receiver)),
            method: method.clone(),
            args: args.iter().map(|a| encode_expr(cm, a)).collect(),
            type_args: type_args.clone(),
            span: to_dspan(cm, *span),
            inherent_rust: inherent_rust.clone(),
            inherent_rust_str_ref: inherent_rust_str_ref.clone(),
            inherent_rust_result_to_string: *inherent_rust_result_to_string,
        },
        IRExpr::OptionalCall {
            callee,
            args,
            type_args,
            span,
        } => DiskIRExpr::OptionalCall {
            callee: callee.clone(),
            args: args.iter().map(|a| encode_expr(cm, a)).collect(),
            type_args: type_args.clone(),
            span: to_dspan(cm, *span),
        },
        IRExpr::OptionalMethodCall {
            receiver,
            method,
            args,
            type_args,
            span,
            inherent_rust,
            inherent_rust_str_ref,
            inherent_rust_result_to_string,
        } => DiskIRExpr::OptionalMethodCall {
            receiver: Box::new(encode_expr(cm, receiver)),
            method: method.clone(),
            args: args.iter().map(|a| encode_expr(cm, a)).collect(),
            type_args: type_args.clone(),
            span: to_dspan(cm, *span),
            inherent_rust: inherent_rust.clone(),
            inherent_rust_str_ref: inherent_rust_str_ref.clone(),
            inherent_rust_result_to_string: *inherent_rust_result_to_string,
        },
        IRExpr::RustNew {
            result_ty,
            rust_fn_path,
            unwrap_result,
            args,
            span,
        } => DiskIRExpr::RustNew {
            result_ty: result_ty.clone(),
            rust_fn_path: rust_fn_path.clone(),
            unwrap_result: *unwrap_result,
            args: args.iter().map(|a| encode_expr(cm, a)).collect(),
            span: to_dspan(cm, *span),
        },
        IRExpr::BuiltinLog { args, stderr, span } => DiskIRExpr::BuiltinLog {
            args: args.iter().map(|a| encode_expr(cm, a)).collect(),
            stderr: *stderr,
            span: to_dspan(cm, *span),
        },
        IRExpr::Conditional {
            test,
            cons,
            alt,
            span,
            cond_ty,
        } => DiskIRExpr::Conditional {
            test: Box::new(encode_expr(cm, test)),
            cons: Box::new(encode_expr(cm, cons)),
            alt: Box::new(encode_expr(cm, alt)),
            span: to_dspan(cm, *span),
            cond_ty: cond_ty.clone(),
        },
        IRExpr::Seq { exprs, span } => DiskIRExpr::Seq {
            exprs: exprs.iter().map(|a| encode_expr(cm, a)).collect(),
            span: to_dspan(cm, *span),
        },
        IRExpr::Tpl { parts, span } => DiskIRExpr::Tpl {
            parts: parts.iter().map(|p| encode_tpl_part(cm, p)).collect(),
            span: to_dspan(cm, *span),
        },
        IRExpr::Member {
            obj,
            prop,
            span,
            length_dispatch,
            http_response_member,
            stream_read_member,
            object_member_access,
        } => DiskIRExpr::Member {
            obj: Box::new(encode_expr(cm, obj)),
            prop: prop.clone(),
            span: to_dspan(cm, *span),
            length_dispatch: *length_dispatch,
            http_response_member: *http_response_member,
            stream_read_member: *stream_read_member,
            object_member_access: *object_member_access,
        },
        IRExpr::Null(sp) => DiskIRExpr::Null(to_dspan(cm, *sp)),
        IRExpr::Undefined(sp) => DiskIRExpr::Undefined(to_dspan(cm, *sp)),
        IRExpr::NullishCoalesce { left, right, span } => DiskIRExpr::NullishCoalesce {
            left: Box::new(encode_expr(cm, left)),
            right: Box::new(encode_expr(cm, right)),
            span: to_dspan(cm, *span),
        },
        IRExpr::OptionalMember {
            obj,
            prop,
            span,
            length_dispatch,
            http_response_member,
            stream_read_member,
            object_member_access,
        } => DiskIRExpr::OptionalMember {
            obj: Box::new(encode_expr(cm, obj)),
            prop: prop.clone(),
            span: to_dspan(cm, *span),
            length_dispatch: *length_dispatch,
            http_response_member: *http_response_member,
            stream_read_member: *stream_read_member,
            object_member_access: *object_member_access,
        },
        IRExpr::MathBuiltin { kind, args, span } => DiskIRExpr::MathBuiltin {
            kind: *kind,
            args: args.iter().map(|a| encode_expr(cm, a)).collect(),
            span: to_dspan(cm, *span),
        },
        IRExpr::NumberBuiltin { kind, args, span } => DiskIRExpr::NumberBuiltin {
            kind: *kind,
            args: args.iter().map(|a| encode_expr(cm, a)).collect(),
            span: to_dspan(cm, *span),
        },
        IRExpr::JsonBuiltin {
            kind,
            args,
            span,
            stringify_inferred_ty,
        } => DiskIRExpr::JsonBuiltin {
            kind: *kind,
            args: args.iter().map(|a| encode_expr(cm, a)).collect(),
            span: to_dspan(cm, *span),
            stringify_inferred_ty: stringify_inferred_ty.clone(),
        },
        IRExpr::UriBuiltin { kind, args, span } => DiskIRExpr::UriBuiltin {
            kind: *kind,
            args: args.iter().map(|a| encode_expr(cm, a)).collect(),
            span: to_dspan(cm, *span),
        },
        IRExpr::StringMethodBuiltin {
            kind,
            receiver,
            args,
            span,
        } => DiskIRExpr::StringMethodBuiltin {
            kind: *kind,
            receiver: Box::new(encode_expr(cm, receiver)),
            args: args.iter().map(|a| encode_expr(cm, a)).collect(),
            span: to_dspan(cm, *span),
        },
        IRExpr::ReadStdinLine { span } => DiskIRExpr::ReadStdinLine {
            span: to_dspan(cm, *span),
        },
        IRExpr::ReadFileText { path, span } => DiskIRExpr::ReadFileText {
            path: Box::new(encode_expr(cm, path)),
            span: to_dspan(cm, *span),
        },
        IRExpr::ReadFileTextAsync { path, span } => DiskIRExpr::ReadFileTextAsync {
            path: Box::new(encode_expr(cm, path)),
            span: to_dspan(cm, *span),
        },
        IRExpr::ArrayLit { elems, span } => DiskIRExpr::ArrayLit {
            elems: elems.iter().map(|a| encode_expr(cm, a)).collect(),
            span: to_dspan(cm, *span),
        },
        IRExpr::ObjectLit { fields, span } => DiskIRExpr::ObjectLit {
            fields: fields
                .iter()
                .map(|(k, v)| (k.clone(), encode_expr(cm, v)))
                .collect(),
            span: to_dspan(cm, *span),
        },
        IRExpr::Index {
            obj,
            index,
            span,
            index_kind,
        } => DiskIRExpr::Index {
            obj: Box::new(encode_expr(cm, obj)),
            index: Box::new(encode_expr(cm, index)),
            span: to_dspan(cm, *span),
            index_kind: *index_kind,
        },
        IRExpr::ArrowFn {
            params,
            ret,
            body,
            span,
        } => DiskIRExpr::ArrowFn {
            params: params.clone(),
            ret: ret.clone(),
            body: body.iter().map(|s| encode_stmt(cm, s)).collect(),
            span: to_dspan(cm, *span),
        },
        IRExpr::This(sp) => DiskIRExpr::This(to_dspan(cm, *sp)),
        IRExpr::Super(sp) => DiskIRExpr::Super(to_dspan(cm, *sp)),
        IRExpr::Await { arg, span } => DiskIRExpr::Await {
            arg: Box::new(encode_expr(cm, arg)),
            span: to_dspan(cm, *span),
        },
        IRExpr::FetchText { url, span } => DiskIRExpr::FetchText {
            url: Box::new(encode_expr(cm, url)),
            span: to_dspan(cm, *span),
        },
        IRExpr::Fetch { url, init, span } => DiskIRExpr::Fetch {
            url: Box::new(encode_expr(cm, url)),
            init: init.as_ref().map(|i| encode_fetch_init(cm, i)),
            span: to_dspan(cm, *span),
        },
        IRExpr::HttpResponseMethodBuiltin {
            kind,
            receiver,
            span,
        } => DiskIRExpr::HttpResponseMethodBuiltin {
            kind: *kind,
            receiver: Box::new(encode_expr(cm, receiver)),
            span: to_dspan(cm, *span),
        },
        IRExpr::HttpResponseBodyGetReader {
            response,
            span,
            stream_slot,
        } => DiskIRExpr::HttpResponseBodyGetReader {
            response: Box::new(encode_expr(cm, response)),
            span: to_dspan(cm, *span),
            stream_slot: *stream_slot,
        },
        IRExpr::ReaderRead {
            reader_name,
            span,
            reader_slot,
        } => DiskIRExpr::ReaderRead {
            reader_name: reader_name.clone(),
            span: to_dspan(cm, *span),
            reader_slot: *reader_slot,
        },
        IRExpr::PromiseAll { elems, span } => DiskIRExpr::PromiseAll {
            elems: elems.iter().map(|a| encode_expr(cm, a)).collect(),
            span: to_dspan(cm, *span),
        },
    }
}

fn decode_expr(cm: &Lrc<SourceMap>, base: u32, e: DiskIRExpr) -> IRExpr {
    match e {
        DiskIRExpr::Number(n, sp) => IRExpr::Number(n, from_dspan(base, sp)),
        DiskIRExpr::Bool(b, sp) => IRExpr::Bool(b, from_dspan(base, sp)),
        DiskIRExpr::Str(s, sp) => IRExpr::Str(s, from_dspan(base, sp)),
        DiskIRExpr::Ident(s, sp) => IRExpr::Ident(s, from_dspan(base, sp)),
        DiskIRExpr::Binary {
            op,
            left,
            right,
            span,
            kind,
        } => IRExpr::Binary {
            op,
            left: Box::new(decode_expr(cm, base, *left)),
            right: Box::new(decode_expr(cm, base, *right)),
            span: from_dspan(base, span),
            kind,
        },
        DiskIRExpr::Unary { op, arg, span } => IRExpr::Unary {
            op,
            arg: Box::new(decode_expr(cm, base, *arg)),
            span: from_dspan(base, span),
        },
        DiskIRExpr::Call {
            callee,
            args,
            type_args,
            span,
        } => IRExpr::Call {
            callee,
            args: args.into_iter().map(|a| decode_expr(cm, base, a)).collect(),
            type_args,
            span: from_dspan(base, span),
        },
        DiskIRExpr::MethodCall {
            receiver,
            method,
            args,
            type_args,
            span,
            inherent_rust,
            inherent_rust_str_ref,
            inherent_rust_result_to_string,
        } => IRExpr::MethodCall {
            receiver: Box::new(decode_expr(cm, base, *receiver)),
            method,
            args: args.into_iter().map(|a| decode_expr(cm, base, a)).collect(),
            type_args,
            span: from_dspan(base, span),
            inherent_rust,
            inherent_rust_str_ref,
            inherent_rust_result_to_string,
        },
        DiskIRExpr::OptionalCall {
            callee,
            args,
            type_args,
            span,
        } => IRExpr::OptionalCall {
            callee,
            args: args.into_iter().map(|a| decode_expr(cm, base, a)).collect(),
            type_args,
            span: from_dspan(base, span),
        },
        DiskIRExpr::OptionalMethodCall {
            receiver,
            method,
            args,
            type_args,
            span,
            inherent_rust,
            inherent_rust_str_ref,
            inherent_rust_result_to_string,
        } => IRExpr::OptionalMethodCall {
            receiver: Box::new(decode_expr(cm, base, *receiver)),
            method,
            args: args.into_iter().map(|a| decode_expr(cm, base, a)).collect(),
            type_args,
            span: from_dspan(base, span),
            inherent_rust,
            inherent_rust_str_ref,
            inherent_rust_result_to_string,
        },
        DiskIRExpr::RustNew {
            result_ty,
            rust_fn_path,
            unwrap_result,
            args,
            span,
        } => IRExpr::RustNew {
            result_ty,
            rust_fn_path,
            unwrap_result,
            args: args.into_iter().map(|a| decode_expr(cm, base, a)).collect(),
            span: from_dspan(base, span),
        },
        DiskIRExpr::BuiltinLog { args, stderr, span } => IRExpr::BuiltinLog {
            args: args.into_iter().map(|a| decode_expr(cm, base, a)).collect(),
            stderr,
            span: from_dspan(base, span),
        },
        DiskIRExpr::Conditional {
            test,
            cons,
            alt,
            span,
            cond_ty,
        } => IRExpr::Conditional {
            test: Box::new(decode_expr(cm, base, *test)),
            cons: Box::new(decode_expr(cm, base, *cons)),
            alt: Box::new(decode_expr(cm, base, *alt)),
            span: from_dspan(base, span),
            cond_ty,
        },
        DiskIRExpr::Seq { exprs, span } => IRExpr::Seq {
            exprs: exprs
                .into_iter()
                .map(|a| decode_expr(cm, base, a))
                .collect(),
            span: from_dspan(base, span),
        },
        DiskIRExpr::Tpl { parts, span } => IRExpr::Tpl {
            parts: parts
                .into_iter()
                .map(|p| decode_tpl_part(cm, base, p))
                .collect(),
            span: from_dspan(base, span),
        },
        DiskIRExpr::Member {
            obj,
            prop,
            span,
            length_dispatch,
            http_response_member,
            stream_read_member,
            object_member_access,
        } => IRExpr::Member {
            obj: Box::new(decode_expr(cm, base, *obj)),
            prop,
            span: from_dspan(base, span),
            length_dispatch,
            http_response_member,
            stream_read_member,
            object_member_access,
        },
        DiskIRExpr::Null(sp) => IRExpr::Null(from_dspan(base, sp)),
        DiskIRExpr::Undefined(sp) => IRExpr::Undefined(from_dspan(base, sp)),
        DiskIRExpr::NullishCoalesce { left, right, span } => IRExpr::NullishCoalesce {
            left: Box::new(decode_expr(cm, base, *left)),
            right: Box::new(decode_expr(cm, base, *right)),
            span: from_dspan(base, span),
        },
        DiskIRExpr::OptionalMember {
            obj,
            prop,
            span,
            length_dispatch,
            http_response_member,
            stream_read_member,
            object_member_access,
        } => IRExpr::OptionalMember {
            obj: Box::new(decode_expr(cm, base, *obj)),
            prop,
            span: from_dspan(base, span),
            length_dispatch,
            http_response_member,
            stream_read_member,
            object_member_access,
        },
        DiskIRExpr::MathBuiltin { kind, args, span } => IRExpr::MathBuiltin {
            kind,
            args: args.into_iter().map(|a| decode_expr(cm, base, a)).collect(),
            span: from_dspan(base, span),
        },
        DiskIRExpr::NumberBuiltin { kind, args, span } => IRExpr::NumberBuiltin {
            kind,
            args: args.into_iter().map(|a| decode_expr(cm, base, a)).collect(),
            span: from_dspan(base, span),
        },
        DiskIRExpr::JsonBuiltin {
            kind,
            args,
            span,
            stringify_inferred_ty,
        } => IRExpr::JsonBuiltin {
            kind,
            args: args.into_iter().map(|a| decode_expr(cm, base, a)).collect(),
            span: from_dspan(base, span),
            stringify_inferred_ty,
        },
        DiskIRExpr::UriBuiltin { kind, args, span } => IRExpr::UriBuiltin {
            kind,
            args: args.into_iter().map(|a| decode_expr(cm, base, a)).collect(),
            span: from_dspan(base, span),
        },
        DiskIRExpr::StringMethodBuiltin {
            kind,
            receiver,
            args,
            span,
        } => IRExpr::StringMethodBuiltin {
            kind,
            receiver: Box::new(decode_expr(cm, base, *receiver)),
            args: args.into_iter().map(|a| decode_expr(cm, base, a)).collect(),
            span: from_dspan(base, span),
        },
        DiskIRExpr::ReadStdinLine { span } => IRExpr::ReadStdinLine {
            span: from_dspan(base, span),
        },
        DiskIRExpr::ReadFileText { path, span } => IRExpr::ReadFileText {
            path: Box::new(decode_expr(cm, base, *path)),
            span: from_dspan(base, span),
        },
        DiskIRExpr::ReadFileTextAsync { path, span } => IRExpr::ReadFileTextAsync {
            path: Box::new(decode_expr(cm, base, *path)),
            span: from_dspan(base, span),
        },
        DiskIRExpr::ArrayLit { elems, span } => IRExpr::ArrayLit {
            elems: elems
                .into_iter()
                .map(|a| decode_expr(cm, base, a))
                .collect(),
            span: from_dspan(base, span),
        },
        DiskIRExpr::ObjectLit { fields, span } => IRExpr::ObjectLit {
            fields: fields
                .into_iter()
                .map(|(k, v)| (k, decode_expr(cm, base, v)))
                .collect(),
            span: from_dspan(base, span),
        },
        DiskIRExpr::Index {
            obj,
            index,
            span,
            index_kind,
        } => IRExpr::Index {
            obj: Box::new(decode_expr(cm, base, *obj)),
            index: Box::new(decode_expr(cm, base, *index)),
            span: from_dspan(base, span),
            index_kind,
        },
        DiskIRExpr::ArrowFn {
            params,
            ret,
            body,
            span,
        } => IRExpr::ArrowFn {
            params,
            ret,
            body: body.into_iter().map(|s| decode_stmt(cm, base, s)).collect(),
            span: from_dspan(base, span),
        },
        DiskIRExpr::This(sp) => IRExpr::This(from_dspan(base, sp)),
        DiskIRExpr::Super(sp) => IRExpr::Super(from_dspan(base, sp)),
        DiskIRExpr::Await { arg, span } => IRExpr::Await {
            arg: Box::new(decode_expr(cm, base, *arg)),
            span: from_dspan(base, span),
        },
        DiskIRExpr::FetchText { url, span } => IRExpr::FetchText {
            url: Box::new(decode_expr(cm, base, *url)),
            span: from_dspan(base, span),
        },
        DiskIRExpr::Fetch { url, init, span } => IRExpr::Fetch {
            url: Box::new(decode_expr(cm, base, *url)),
            init: init.map(|i| decode_fetch_init(cm, base, i)),
            span: from_dspan(base, span),
        },
        DiskIRExpr::HttpResponseMethodBuiltin {
            kind,
            receiver,
            span,
        } => IRExpr::HttpResponseMethodBuiltin {
            kind,
            receiver: Box::new(decode_expr(cm, base, *receiver)),
            span: from_dspan(base, span),
        },
        DiskIRExpr::HttpResponseBodyGetReader {
            response,
            span,
            stream_slot,
        } => IRExpr::HttpResponseBodyGetReader {
            response: Box::new(decode_expr(cm, base, *response)),
            span: from_dspan(base, span),
            stream_slot,
        },
        DiskIRExpr::ReaderRead {
            reader_name,
            span,
            reader_slot,
        } => IRExpr::ReaderRead {
            reader_name,
            span: from_dspan(base, span),
            reader_slot,
        },
        DiskIRExpr::PromiseAll { elems, span } => IRExpr::PromiseAll {
            elems: elems
                .into_iter()
                .map(|a| decode_expr(cm, base, a))
                .collect(),
            span: from_dspan(base, span),
        },
    }
}

fn encode_stmt(cm: &SourceMap, s: &IRStmt) -> DiskIRStmt {
    match s {
        IRStmt::Empty { span } => DiskIRStmt::Empty {
            span: to_dspan(cm, *span),
        },
        IRStmt::Let {
            name,
            ty,
            init,
            mutable,
            span,
        } => DiskIRStmt::Let {
            name: name.clone(),
            ty: ty.clone(),
            init: init.as_ref().map(|e| encode_expr(cm, e)),
            mutable: *mutable,
            span: to_dspan(cm, *span),
        },
        IRStmt::Assign { name, rhs, span } => DiskIRStmt::Assign {
            name: name.clone(),
            rhs: encode_expr(cm, rhs),
            span: to_dspan(cm, *span),
        },
        IRStmt::MemberAssign {
            obj,
            prop,
            rhs,
            span,
        } => DiskIRStmt::MemberAssign {
            obj: obj.clone(),
            prop: prop.clone(),
            rhs: encode_expr(cm, rhs),
            span: to_dspan(cm, *span),
        },
        IRStmt::Expr { expr, span } => DiskIRStmt::Expr {
            expr: encode_expr(cm, expr),
            span: to_dspan(cm, *span),
        },
        IRStmt::Return { arg, span } => DiskIRStmt::Return {
            arg: arg.as_ref().map(|e| encode_expr(cm, e)),
            span: to_dspan(cm, *span),
        },
        IRStmt::Block { stmts, span } => DiskIRStmt::Block {
            stmts: stmts.iter().map(|x| encode_stmt(cm, x)).collect(),
            span: to_dspan(cm, *span),
        },
        IRStmt::If {
            cond,
            cond_ty,
            then_b,
            else_b,
            span,
        } => DiskIRStmt::If {
            cond: encode_expr(cm, cond),
            cond_ty: cond_ty.clone(),
            then_b: then_b.iter().map(|x| encode_stmt(cm, x)).collect(),
            else_b: else_b
                .as_ref()
                .map(|b| b.iter().map(|x| encode_stmt(cm, x)).collect()),
            span: to_dspan(cm, *span),
        },
        IRStmt::While {
            cond,
            cond_ty,
            body,
            span,
        } => DiskIRStmt::While {
            cond: encode_expr(cm, cond),
            cond_ty: cond_ty.clone(),
            body: body.iter().map(|x| encode_stmt(cm, x)).collect(),
            span: to_dspan(cm, *span),
        },
        IRStmt::ForIn {
            key,
            key_ty,
            target,
            kind,
            body,
            span,
        } => DiskIRStmt::ForIn {
            key: key.clone(),
            key_ty: key_ty.clone(),
            target: encode_expr(cm, target),
            kind: *kind,
            body: body.iter().map(|x| encode_stmt(cm, x)).collect(),
            span: to_dspan(cm, *span),
        },
        IRStmt::ForOf {
            elem,
            elem_ty,
            target,
            body,
            span,
        } => DiskIRStmt::ForOf {
            elem: elem.clone(),
            elem_ty: elem_ty.clone(),
            target: encode_expr(cm, target),
            body: body.iter().map(|x| encode_stmt(cm, x)).collect(),
            span: to_dspan(cm, *span),
        },
        IRStmt::DoWhile {
            body,
            cond,
            cond_ty,
            span,
        } => DiskIRStmt::DoWhile {
            body: body.iter().map(|x| encode_stmt(cm, x)).collect(),
            cond: encode_expr(cm, cond),
            cond_ty: cond_ty.clone(),
            span: to_dspan(cm, *span),
        },
        IRStmt::Break { span } => DiskIRStmt::Break {
            span: to_dspan(cm, *span),
        },
        IRStmt::Continue { span } => DiskIRStmt::Continue {
            span: to_dspan(cm, *span),
        },
        IRStmt::FnDecl { func, span } => DiskIRStmt::FnDecl {
            func: Box::new(encode_function(cm, func)),
            span: to_dspan(cm, *span),
        },
    }
}

fn decode_stmt(cm: &Lrc<SourceMap>, base: u32, s: DiskIRStmt) -> IRStmt {
    match s {
        DiskIRStmt::Empty { span } => IRStmt::Empty {
            span: from_dspan(base, span),
        },
        DiskIRStmt::Let {
            name,
            ty,
            init,
            mutable,
            span,
        } => IRStmt::Let {
            name,
            ty,
            init: init.map(|e| decode_expr(cm, base, e)),
            mutable,
            span: from_dspan(base, span),
        },
        DiskIRStmt::Assign { name, rhs, span } => IRStmt::Assign {
            name,
            rhs: decode_expr(cm, base, rhs),
            span: from_dspan(base, span),
        },
        DiskIRStmt::MemberAssign {
            obj,
            prop,
            rhs,
            span,
        } => IRStmt::MemberAssign {
            obj,
            prop,
            rhs: decode_expr(cm, base, rhs),
            span: from_dspan(base, span),
        },
        DiskIRStmt::Expr { expr, span } => IRStmt::Expr {
            expr: decode_expr(cm, base, expr),
            span: from_dspan(base, span),
        },
        DiskIRStmt::Return { arg, span } => IRStmt::Return {
            arg: arg.map(|e| decode_expr(cm, base, e)),
            span: from_dspan(base, span),
        },
        DiskIRStmt::Block { stmts, span } => IRStmt::Block {
            stmts: stmts
                .into_iter()
                .map(|x| decode_stmt(cm, base, x))
                .collect(),
            span: from_dspan(base, span),
        },
        DiskIRStmt::If {
            cond,
            cond_ty,
            then_b,
            else_b,
            span,
        } => IRStmt::If {
            cond: decode_expr(cm, base, cond),
            cond_ty,
            then_b: then_b
                .into_iter()
                .map(|x| decode_stmt(cm, base, x))
                .collect(),
            else_b: else_b.map(|b| b.into_iter().map(|x| decode_stmt(cm, base, x)).collect()),
            span: from_dspan(base, span),
        },
        DiskIRStmt::While {
            cond,
            cond_ty,
            body,
            span,
        } => IRStmt::While {
            cond: decode_expr(cm, base, cond),
            cond_ty,
            body: body.into_iter().map(|x| decode_stmt(cm, base, x)).collect(),
            span: from_dspan(base, span),
        },
        DiskIRStmt::ForIn {
            key,
            key_ty,
            target,
            kind,
            body,
            span,
        } => IRStmt::ForIn {
            key,
            key_ty,
            target: decode_expr(cm, base, target),
            kind,
            body: body.into_iter().map(|x| decode_stmt(cm, base, x)).collect(),
            span: from_dspan(base, span),
        },
        DiskIRStmt::ForOf {
            elem,
            elem_ty,
            target,
            body,
            span,
        } => IRStmt::ForOf {
            elem,
            elem_ty,
            target: decode_expr(cm, base, target),
            body: body.into_iter().map(|x| decode_stmt(cm, base, x)).collect(),
            span: from_dspan(base, span),
        },
        DiskIRStmt::DoWhile {
            body,
            cond,
            cond_ty,
            span,
        } => IRStmt::DoWhile {
            body: body.into_iter().map(|x| decode_stmt(cm, base, x)).collect(),
            cond: decode_expr(cm, base, cond),
            cond_ty,
            span: from_dspan(base, span),
        },
        DiskIRStmt::Break { span } => IRStmt::Break {
            span: from_dspan(base, span),
        },
        DiskIRStmt::Continue { span } => IRStmt::Continue {
            span: from_dspan(base, span),
        },
        DiskIRStmt::FnDecl { func, span } => IRStmt::FnDecl {
            func: Box::new(decode_function(cm, base, *func)),
            span: from_dspan(base, span),
        },
    }
}

fn encode_function(cm: &SourceMap, f: &IRFunction) -> DiskIRFunction {
    DiskIRFunction {
        ir_id: f.ir_id,
        name: f.name.clone(),
        type_params: f.type_params.clone(),
        params: f.params.clone(),
        ret: f.ret.clone(),
        body: f.body.iter().map(|s| encode_stmt(cm, s)).collect(),
        span: to_dspan(cm, f.span),
        source_path: f.source_path.clone(),
        mono_origin: f.mono_origin.clone(),
        is_async: f.is_async,
    }
}

fn decode_function(cm: &Lrc<SourceMap>, base: u32, f: DiskIRFunction) -> IRFunction {
    let sm = SendSourceMap(cm.clone());
    IRFunction {
        ir_id: f.ir_id,
        name: f.name,
        type_params: f.type_params,
        params: f.params,
        ret: f.ret,
        body: f
            .body
            .into_iter()
            .map(|s| decode_stmt(cm, base, s))
            .collect(),
        span: from_dspan(base, f.span),
        cm: sm,
        source_path: f.source_path,
        mono_origin: f.mono_origin,
        is_async: f.is_async,
    }
}

fn encode_class_method(cm: &SourceMap, m: &IRClassMethod) -> DiskIRClassMethod {
    DiskIRClassMethod {
        name: m.name.clone(),
        params: m.params.clone(),
        ret: m.ret.clone(),
        body: m.body.iter().map(|s| encode_stmt(cm, s)).collect(),
        is_override: m.is_override,
        owner: m.owner.clone(),
        span: to_dspan(cm, m.span),
    }
}

fn decode_class_method(cm: &Lrc<SourceMap>, base: u32, m: DiskIRClassMethod) -> IRClassMethod {
    IRClassMethod {
        name: m.name,
        params: m.params,
        ret: m.ret,
        body: m
            .body
            .into_iter()
            .map(|s| decode_stmt(cm, base, s))
            .collect(),
        is_override: m.is_override,
        owner: m.owner,
        span: from_dspan(base, m.span),
    }
}

fn encode_class(cm: &SourceMap, c: &IRClass) -> DiskIRClass {
    DiskIRClass {
        name: c.name.clone(),
        extends: c.extends.clone(),
        fields: c.fields.clone(),
        ctor: c.ctor.as_ref().map(|x| encode_class_method(cm, x)),
        methods: c
            .methods
            .iter()
            .map(|m| encode_class_method(cm, m))
            .collect(),
        span: to_dspan(cm, c.span),
        source_path: c.source_path.clone(),
    }
}

fn decode_class(cm: &Lrc<SourceMap>, base: u32, c: DiskIRClass) -> IRClass {
    let sm = SendSourceMap(cm.clone());
    IRClass {
        name: c.name,
        extends: c.extends,
        fields: c.fields,
        ctor: c.ctor.map(|x| decode_class_method(cm, base, x)),
        methods: c
            .methods
            .into_iter()
            .map(|m| decode_class_method(cm, base, m))
            .collect(),
        span: from_dspan(base, c.span),
        cm: sm,
        source_path: c.source_path,
    }
}

/// 将片段编码为 bincode 字节（含 schema 版本）。
pub fn encode_fragment_to_bytes(frag: &ModuleIrFragment) -> Result<Vec<u8>, IrCacheError> {
    let cm = fragment_primary_cm(frag);
    let cmr = cm.as_ref();
    let disk = DiskModuleFragment {
        schema: SCHEMA_VERSION,
        fns: frag.fns.iter().map(|f| encode_function(cmr, f)).collect(),
        classes: frag.classes.iter().map(|c| encode_class(cmr, c)).collect(),
        ts_comments: frag.ts_comments.clone(),
        exported_types: frag.exported_types.clone(),
    };
    bincode::serialize(&disk).map_err(|e| IrCacheError::Bincode(e.to_string()))
}

/// 用当前磁盘上的 `source` 重建 [`SourceMap`]，再还原片段（须与编码时内容一致）。
pub fn decode_fragment_from_bytes(
    path: &Path,
    source: &str,
    bytes: &[u8],
) -> Result<ModuleIrFragment, IrCacheError> {
    let disk: DiskModuleFragment =
        bincode::deserialize(bytes).map_err(|e| IrCacheError::Bincode(e.to_string()))?;
    if disk.schema != SCHEMA_VERSION {
        return Err(IrCacheError::Schema(disk.schema));
    }
    let (cm, base) = source_map_for_path(path, source);
    Ok(ModuleIrFragment {
        fns: disk
            .fns
            .into_iter()
            .map(|f| decode_function(&cm, base, f))
            .collect(),
        classes: disk
            .classes
            .into_iter()
            .map(|c| decode_class(&cm, base, c))
            .collect(),
        ts_comments: disk.ts_comments,
        exported_types: disk.exported_types,
    })
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use swc_ecma_ast::Program;

    use crate::build::build_module_ir_fragment;
    use trust_parser::parse_typescript_file;

    use super::*;

    #[test]
    fn module_fragment_round_trip_bincode() {
        let src = "export function main(): number { return 1; }\n";
        let parsed = parse_typescript_file("t.ts", src).unwrap();
        let Program::Module(_) = &parsed.program else {
            panic!("expected module");
        };
        let mut next = 0u32;
        let frag = build_module_ir_fragment(
            "t.ts",
            &parsed.program,
            &parsed.source_map,
            &parsed.comments,
            false,
            &mut next,
            None,
        )
        .unwrap();
        let bytes = encode_fragment_to_bytes(&frag).unwrap();
        let back = decode_fragment_from_bytes(Path::new("t.ts"), src, &bytes).unwrap();
        assert_eq!(back.fns.len(), frag.fns.len());
        assert_eq!(back.fns[0].name, "main");
    }
}
