use std::collections::{HashMap, HashSet};

use swc_common::{sync::Lrc, SourceMap, Span, Spanned};
use swc_ecma_ast::{
    BindingIdent, ClassDecl, ClassMember, Decl, ExportDecl, Expr, Lit, ModuleDecl, ModuleItem, Pat,
    Program, PropName, Stmt, TsEntityName, TsFnOrConstructorType, TsFnParam, TsInterfaceDecl,
    TsIntersectionType, TsKeywordTypeKind, TsLit, TsType as AstTsType, TsTypeAliasDecl, TsTypeAnn,
    TsTypeElement, TsUnionOrIntersectionType, TsUnionType,
};

use crate::error::{diag, CompileError};
use crate::ir::{normalize_union, ObjectProp, TsType};

/// `interface` / 对象字面量类型中允许的字段类型：`number` 或嵌套 `ObjectNum`。
fn validate_object_field_ty(
    ty: &TsType,
    cm: &Lrc<SourceMap>,
    path: &str,
    span: Span,
) -> Result<(), CompileError> {
    match ty {
        TsType::Number => Ok(()),
        TsType::ObjectNum(inner) => {
            for p in inner {
                validate_object_field_ty(&p.ty, cm, path, span)?;
            }
            Ok(())
        }
        _ => Err(diag(
            cm,
            path,
            span,
            "object/interface field type must be `number` or a nested object of numbers",
        )),
    }
}

pub(super) fn ts_type_from_pat_ann(
    pat: &Pat,
    cm: &Lrc<SourceMap>,
    path: &str,
    iface: &HashMap<String, TsType>,
) -> Result<TsType, CompileError> {
    match pat {
        Pat::Ident(BindingIdent { type_ann, .. }) => {
            ts_type_from_ann(type_ann, cm, path, pat.span(), iface, None)
        }
        _ => Err(diag(
            cm,
            path,
            pat.span(),
            "type annotation required for binding",
        )),
    }
}

pub(super) fn ts_type_from_ann(
    ann: &Option<Box<TsTypeAnn>>,
    cm: &Lrc<SourceMap>,
    path: &str,
    fallback_span: Span,
    iface: &HashMap<String, TsType>,
    type_params: Option<&HashSet<String>>,
) -> Result<TsType, CompileError> {
    let Some(ann) = ann else {
        return Err(diag(cm, path, fallback_span, "type annotation is required"));
    };
    ts_type_from_ast(&ann.type_ann, cm, path, iface, type_params)
}

pub(super) fn ts_type_from_ast(
    ty: &AstTsType,
    cm: &Lrc<SourceMap>,
    path: &str,
    iface: &HashMap<String, TsType>,
    type_params: Option<&HashSet<String>>,
) -> Result<TsType, CompileError> {
    match ty {
        AstTsType::TsKeywordType(k) => match k.kind {
            TsKeywordTypeKind::TsNumberKeyword => Ok(TsType::Number),
            TsKeywordTypeKind::TsBooleanKeyword => Ok(TsType::Boolean),
            TsKeywordTypeKind::TsStringKeyword => Ok(TsType::String),
            TsKeywordTypeKind::TsVoidKeyword => Ok(TsType::Void),
            TsKeywordTypeKind::TsNullKeyword => Ok(TsType::Null),
            TsKeywordTypeKind::TsUndefinedKeyword => Ok(TsType::Undefined),
            _ => Err(diag(
                cm,
                path,
                k.span,
                "only `number`, `boolean`, `string`, `void`, `null`, `undefined` keyword types are supported",
            )),
        },
        AstTsType::TsArrayType(a) => {
            let elem = ts_type_from_ast(&a.elem_type, cm, path, iface, type_params)?;
            if elem == TsType::Number {
                Ok(TsType::ArrayNumber)
            } else if elem == TsType::String {
                Ok(TsType::ArrayString)
            } else {
                Err(diag(
                    cm,
                    path,
                    a.span,
                    "only `number[]` and `string[]` are supported for array type annotations",
                ))
            }
        }
        AstTsType::TsLitType(lt) => match &lt.lit {
            TsLit::Number(n) => {
                let v = n.value;
                if v.fract() != 0.0 || !v.is_finite() {
                    return Err(diag(
                        cm,
                        path,
                        lt.span,
                        "only integer literal types are supported for `number` literals in type position",
                    ));
                }
                if v < i32::MIN as f64 || v > i32::MAX as f64 {
                    return Err(diag(
                        cm,
                        path,
                        lt.span,
                        "numeric literal type is out of range for `i32`",
                    ));
                }
                Ok(TsType::NumberLit(v as i32))
            }
            TsLit::Str(s) => Ok(TsType::StringLit(s.value.to_string_lossy().into_owned())),
            TsLit::Bool(b) => Ok(TsType::BoolLit(b.value)),
            TsLit::BigInt(_) => Err(diag(
                cm,
                path,
                lt.span,
                "`bigint` literal types are not supported",
            )),
            TsLit::Tpl(_) => Err(diag(
                cm,
                path,
                lt.span,
                "template literal types are not supported",
            )),
        },
        AstTsType::TsTypeLit(tl) => {
            object_num_from_type_elements(&tl.members, cm, path, tl.span, iface, type_params)
        }
        AstTsType::TsTypeRef(r) => {
            if let Some(tp) = type_params {
                if let TsEntityName::Ident(id) = &r.type_name {
                    let name = id.sym.to_string();
                    if tp.contains(&name) {
                        if r.type_params.is_some() {
                            return Err(diag(
                                cm,
                                path,
                                r.span,
                                "type arguments on type parameters are not supported",
                            ));
                        }
                        return Ok(TsType::TypeParam(name));
                    }
                }
            }
            match &r.type_name {
                TsEntityName::Ident(id) => {
                    let name = id.sym.to_string();
                    if name == "HttpResponse" {
                        if r.type_params.is_some() {
                            return Err(diag(
                                cm,
                                path,
                                r.span,
                                "`HttpResponse` does not take type parameters",
                            ));
                        }
                        return Ok(TsType::HttpResponse);
                    }
                    if name == "ReadableStream" {
                        if r.type_params.is_some() {
                            return Err(diag(
                                cm,
                                path,
                                r.span,
                                "`ReadableStream` does not take type parameters",
                            ));
                        }
                        return Ok(TsType::ReadableStream);
                    }
                    if name == "ReadableStreamDefaultReader" {
                        if r.type_params.is_some() {
                            return Err(diag(
                                cm,
                                path,
                                r.span,
                                "`ReadableStreamDefaultReader` does not take type parameters",
                            ));
                        }
                        return Ok(TsType::ReadableStreamDefaultReader);
                    }
                    if name == "StreamReadResult" {
                        if r.type_params.is_some() {
                            return Err(diag(
                                cm,
                                path,
                                r.span,
                                "`StreamReadResult` does not take type parameters",
                            ));
                        }
                        return Ok(TsType::StreamReadResult);
                    }
                    if name == "Uint8Array" {
                        if r.type_params.is_some() {
                            return Err(diag(
                                cm,
                                path,
                                r.span,
                                "`Uint8Array` does not take type parameters",
                            ));
                        }
                        return Ok(TsType::Uint8Array);
                    }
                    if name == "Promise" {
                        return Err(diag(
                            cm,
                            path,
                            r.span,
                            "`Promise` is not a trust type: use `async function …(): T` with the awaited type `T` directly (not `Promise<T>`), and `async_all([...])` instead of `Promise.all`",
                        ));
                    }
                    let base = iface.get(&name).cloned().ok_or_else(|| {
                        diag(
                            cm,
                            path,
                            r.span,
                            format!("unknown type name `{}`", id.sym),
                        )
                    })?;
                    if let Some(args) = &r.type_params {
                        if args.params.is_empty() {
                            return Ok(base);
                        }
                        // 当前阶段：先解析并接受显式 type args，实例化在 sem 阶段完成。
                        for a in &args.params {
                            let _ = ts_type_from_ast(a, cm, path, iface, type_params)?;
                        }
                        Ok(base)
                    } else {
                        Ok(base)
                    }
                }
                TsEntityName::TsQualifiedName(q) => Err(diag(
                    cm,
                    path,
                    q.span,
                    "qualified type names are not supported",
                )),
            }
        }
        AstTsType::TsFnOrConstructorType(TsFnOrConstructorType::TsFnType(ft)) => {
            let mut params = Vec::with_capacity(ft.params.len());
            for p in &ft.params {
                let pt = match p {
                    TsFnParam::Ident(i) => ts_type_from_ann(&i.type_ann, cm, path, i.span, iface, type_params)?,
                    _ => {
                        return Err(diag(
                            cm,
                            path,
                            p.span(),
                            "only identifier parameters are supported in function type",
                        ))
                    }
                };
                params.push(pt);
            }
            let ret = ts_type_from_ast(&ft.type_ann.type_ann, cm, path, iface, type_params)?;
            Ok(TsType::Fn {
                params,
                ret: Box::new(ret),
            })
        }
        AstTsType::TsUnionOrIntersectionType(u) => match u {
            TsUnionOrIntersectionType::TsUnionType(TsUnionType { types, span }) => {
                if types.is_empty() {
                    return Err(diag(
                        cm,
                        path,
                        *span,
                        "union type must have at least one member",
                    ));
                }
                let mut members = Vec::with_capacity(types.len());
                for t in types {
                    members.push(ts_type_from_ast(t.as_ref(), cm, path, iface, type_params)?);
                }
                Ok(normalize_union(members))
            }
            TsUnionOrIntersectionType::TsIntersectionType(TsIntersectionType { span, .. }) => Err(
                diag(
                    cm,
                    path,
                    *span,
                    "intersection types are not supported",
                ),
            ),
        },
        _ => Err(diag(
            cm,
            path,
            ty.span(),
            "unsupported type annotation",
        )),
    }
}

/// 收集具名类型；失败项记入 `errs` 并跳过，仍返回当前已收集的 `map`。
pub(super) fn collect_named_types_with_errors(
    program: &Program,
    cm: &Lrc<SourceMap>,
    path: &str,
    errs: &mut Vec<CompileError>,
) -> HashMap<String, TsType> {
    let mut map = HashMap::new();
    match program {
        Program::Module(m) => {
            for item in &m.body {
                match item {
                    ModuleItem::Stmt(Stmt::Decl(Decl::TsInterface(i))) => {
                        if let Err(e) = collect_one_interface(i.as_ref(), &mut map, cm, path) {
                            errs.push(e);
                        }
                    }
                    ModuleItem::Stmt(Stmt::Decl(Decl::TsTypeAlias(a))) => {
                        if let Err(e) = collect_one_type_alias(a.as_ref(), &mut map, cm, path) {
                            errs.push(e);
                        }
                    }
                    ModuleItem::Stmt(Stmt::Decl(Decl::Class(c))) => {
                        if let Err(e) = collect_one_class(c, &mut map, cm, path) {
                            errs.push(e);
                        }
                    }
                    ModuleItem::ModuleDecl(ModuleDecl::ExportDecl(ExportDecl { decl, .. })) => {
                        match decl {
                            Decl::TsInterface(i) => {
                                if let Err(e) =
                                    collect_one_interface(i.as_ref(), &mut map, cm, path)
                                {
                                    errs.push(e);
                                }
                            }
                            Decl::TsTypeAlias(a) => {
                                if let Err(e) =
                                    collect_one_type_alias(a.as_ref(), &mut map, cm, path)
                                {
                                    errs.push(e);
                                }
                            }
                            Decl::Class(c) => {
                                if let Err(e) = collect_one_class(c, &mut map, cm, path) {
                                    errs.push(e);
                                }
                            }
                            _ => {}
                        }
                    }
                    _ => {}
                }
            }
        }
        Program::Script(s) => {
            for stmt in &s.body {
                match stmt {
                    Stmt::Decl(Decl::TsInterface(i)) => {
                        if let Err(e) = collect_one_interface(i.as_ref(), &mut map, cm, path) {
                            errs.push(e);
                        }
                    }
                    Stmt::Decl(Decl::TsTypeAlias(a)) => {
                        if let Err(e) = collect_one_type_alias(a.as_ref(), &mut map, cm, path) {
                            errs.push(e);
                        }
                    }
                    Stmt::Decl(Decl::Class(c)) => {
                        if let Err(e) = collect_one_class(c, &mut map, cm, path) {
                            errs.push(e);
                        }
                    }
                    _ => {}
                }
            }
        }
    }
    map
}

fn collect_one_class(
    c: &ClassDecl,
    map: &mut HashMap<String, TsType>,
    cm: &Lrc<SourceMap>,
    path: &str,
) -> Result<(), CompileError> {
    let name = c.ident.sym.to_string();
    if map.contains_key(&name) {
        return Err(diag(
            cm,
            path,
            c.class.span,
            format!("duplicate type name `{name}`"),
        ));
    }
    let mut props: Vec<ObjectProp> = Vec::new();
    for m in &c.class.body {
        if let ClassMember::ClassProp(p) = m {
            let PropName::Ident(id) = &p.key else {
                return Err(diag(
                    cm,
                    path,
                    p.span,
                    "only identifier class fields are supported",
                ));
            };
            let ty = ts_type_from_ann(&p.type_ann, cm, path, p.span, map, None)?;
            validate_object_field_ty(&ty, cm, path, p.span)?;
            props.push(ObjectProp {
                name: id.sym.to_string(),
                optional: false,
                ty: Box::new(ty),
            });
        }
    }
    if let Some(sup) = &c.class.super_class {
        if let Expr::Ident(id) = &**sup {
            if let Some(TsType::ObjectNum(parent_props)) = map.get(id.sym.as_ref()) {
                for q in parent_props {
                    if !props.iter().any(|x| x.name == q.name) {
                        props.push(q.clone());
                    }
                }
            }
        }
    }
    props.sort_by(|a, b| a.name.cmp(&b.name));
    for w in props.windows(2) {
        if w[0].name == w[1].name {
            return Err(diag(
                cm,
                path,
                c.class.span,
                format!("duplicate class field `{}`", w[0].name),
            ));
        }
    }
    map.insert(name, TsType::ObjectNum(props));
    Ok(())
}

fn collect_one_interface(
    d: &TsInterfaceDecl,
    map: &mut HashMap<String, TsType>,
    cm: &Lrc<SourceMap>,
    path: &str,
) -> Result<(), CompileError> {
    let name = d.id.sym.to_string();
    if map.contains_key(&name) {
        return Err(diag(
            cm,
            path,
            d.id.span,
            format!("duplicate type name `{}`", d.id.sym),
        ));
    }
    if !d.extends.is_empty() {
        return Err(diag(
            cm,
            path,
            d.extends[0].span,
            "interface extends clauses are not supported",
        ));
    }
    let ty = object_num_from_type_elements(&d.body.body, cm, path, d.body.span, map, None)?;
    map.insert(name, ty);
    Ok(())
}

fn collect_one_type_alias(
    d: &TsTypeAliasDecl,
    map: &mut HashMap<String, TsType>,
    cm: &Lrc<SourceMap>,
    path: &str,
) -> Result<(), CompileError> {
    let name = d.id.sym.to_string();
    if map.contains_key(&name) {
        return Err(diag(
            cm,
            path,
            d.id.span,
            format!("duplicate type name `{}`", d.id.sym),
        ));
    }
    let mut tp = HashSet::new();
    if let Some(params) = &d.type_params {
        for p in &params.params {
            tp.insert(p.name.sym.to_string());
        }
    }
    let ty = ts_type_from_ast(
        d.type_ann.as_ref(),
        cm,
        path,
        map,
        if tp.is_empty() { None } else { Some(&tp) },
    )?;
    map.insert(name, ty);
    Ok(())
}

fn object_num_from_type_elements(
    members: &[TsTypeElement],
    cm: &Lrc<SourceMap>,
    path: &str,
    dup_span: Span,
    iface: &HashMap<String, TsType>,
    type_params: Option<&HashSet<String>>,
) -> Result<TsType, CompileError> {
    let mut props: Vec<ObjectProp> = Vec::new();
    for m in members {
        let TsTypeElement::TsPropertySignature(p) = m else {
            return Err(diag(
                cm,
                path,
                m.span(),
                "only property signatures are supported in object type literal",
            ));
        };
        let key = match &*p.key {
            Expr::Ident(i) => i.sym.to_string(),
            Expr::Lit(Lit::Str(s)) => s.value.to_string_lossy().into_owned(),
            _ => {
                return Err(diag(
                    cm,
                    path,
                    p.span,
                    "object type field key must be identifier or string literal",
                ));
            }
        };
        let Some(type_ann) = &p.type_ann else {
            return Err(diag(
                cm,
                path,
                p.span,
                "object type field annotation is required",
            ));
        };
        let ft = ts_type_from_ast(&type_ann.type_ann, cm, path, iface, type_params)?;
        validate_object_field_ty(&ft, cm, path, p.span)?;
        props.push(ObjectProp {
            name: key,
            optional: p.optional,
            ty: Box::new(ft),
        });
    }
    props.sort_by(|a, b| a.name.cmp(&b.name));
    for w in props.windows(2) {
        if w[0].name == w[1].name {
            return Err(diag(
                cm,
                path,
                dup_span,
                format!("duplicate object type field `{}`", w[0].name),
            ));
        }
    }
    Ok(TsType::ObjectNum(props))
}
