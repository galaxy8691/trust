use std::collections::{HashMap, HashSet};

use swc_common::{sync::Lrc, SourceMap, Span, Spanned};
use swc_ecma_ast::{
    BindingIdent, ClassDecl, ClassMember, Decl, ExportDecl, Expr, Lit, ModuleDecl, ModuleItem, Pat,
    Program, PropName, Stmt, TsEntityName, TsFnOrConstructorType, TsFnParam, TsInterfaceDecl,
    TsIntersectionType, TsKeywordTypeKind, TsLit, TsType as AstTsType, TsTypeAliasDecl, TsTypeAnn,
    TsTypeElement, TsUnionOrIntersectionType, TsUnionType,
};

use crate::error::{diag, CompileError};
use crate::ir::{normalize_intersection, normalize_union, ObjectMemberKind, ObjectProp, TsType};

/// `interface` / 对象字面量类型中允许的字段类型：`number`、字面量类型或嵌套 `ObjectNum`。
fn validate_object_field_ty(
    ty: &TsType,
    cm: &Lrc<SourceMap>,
    path: &str,
    span: Span,
) -> Result<(), CompileError> {
    match ty {
        TsType::Number => Ok(()),
        // D1: 支持字面量类型作为 discriminant
        TsType::NumberLit(_) | TsType::BoolLit(_) | TsType::StringLit(_) => Ok(()),
        TsType::ObjectNum(inner) => {
            for p in inner {
                // R1: 只验证字段类型，方法签名不参与字段验证
                if let ObjectMemberKind::Field(ty) = &p.kind {
                    validate_object_field_ty(ty, cm, path, span)?;
                }
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

/// R1: 从函数参数提取类型
pub(super) fn ts_type_from_fn_param(
    param: &TsFnParam,
    cm: &Lrc<SourceMap>,
    path: &str,
    iface: &HashMap<String, TsType>,
    type_params: Option<&HashSet<String>>,
) -> Result<TsType, CompileError> {
    match param {
        TsFnParam::Ident(i) => ts_type_from_ann(&i.type_ann, cm, path, i.span, iface, type_params),
        TsFnParam::Array(_) => Err(diag(
            cm,
            path,
            param.span(),
            "array destructuring parameters are not supported",
        )),
        TsFnParam::Object(_) => Err(diag(
            cm,
            path,
            param.span(),
            "object destructuring parameters are not supported",
        )),
        TsFnParam::Rest(_) => Err(diag(
            cm,
            path,
            param.span(),
            "rest parameters are not supported",
        )),
    }
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
            TsUnionOrIntersectionType::TsIntersectionType(TsIntersectionType { types, span }) => {
                if types.is_empty() {
                    return Err(diag(
                        cm,
                        path,
                        *span,
                        "intersection type must have at least one member",
                    ));
                }
                let mut members = Vec::with_capacity(types.len());
                for t in types {
                    members.push(ts_type_from_ast(t.as_ref(), cm, path, iface, type_params)?);
                }
                match normalize_intersection(members) {
                    Ok(t) => Ok(t),
                    Err(e) => Err(diag(cm, path, *span, e)),
                }
            }
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
/// B2+: 使用两阶段收集：
/// 1) 收集所有接口名（空壳）到 map 用于前向引用
/// 2) 解析接口属性、extends 和类型别名
pub(super) fn collect_named_types_with_errors(
    program: &Program,
    cm: &Lrc<SourceMap>,
    path: &str,
    errs: &mut Vec<CompileError>,
) -> HashMap<String, TsType> {
    let mut map = HashMap::new();
    let mut interface_decls: Vec<&TsInterfaceDecl> = Vec::new();
    let mut type_alias_decls: Vec<&TsTypeAliasDecl> = Vec::new();
    
    // 第一阶段：收集所有声明的引用，并将接口名（空壳）插入 map
    match program {
        Program::Module(m) => {
            for item in &m.body {
                match item {
                    ModuleItem::Stmt(Stmt::Decl(Decl::TsInterface(i))) => {
                        interface_decls.push(i.as_ref());
                    }
                    ModuleItem::Stmt(Stmt::Decl(Decl::TsTypeAlias(a))) => {
                        type_alias_decls.push(a.as_ref());
                    }
                    ModuleItem::Stmt(Stmt::Decl(Decl::Class(c))) => {
                        if let Err(e) = collect_one_class(c, &mut map, cm, path) {
                            errs.push(e);
                        }
                    }
                    ModuleItem::ModuleDecl(ModuleDecl::ExportDecl(ExportDecl { decl, .. })) => {
                        match decl {
                            Decl::TsInterface(i) => {
                                interface_decls.push(i.as_ref());
                            }
                            Decl::TsTypeAlias(a) => {
                                type_alias_decls.push(a.as_ref());
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
                        interface_decls.push(i.as_ref());
                    }
                    Stmt::Decl(Decl::TsTypeAlias(a)) => {
                        type_alias_decls.push(a.as_ref());
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
    
    // 检查接口名重复并将空壳插入 map（支持前向引用）
    for d in &interface_decls {
        let name = d.id.sym.to_string();
        if map.contains_key(&name) {
            errs.push(diag(
                cm,
                path,
                d.id.span,
                format!("duplicate type name `{}`", d.id.sym),
            ));
            continue;
        }
        map.insert(name.clone(), TsType::Interface {
            name,
            extends: None,
            props: vec![],
        });
    }
    
    // 第二阶段：检测循环继承
    if let Err(e) = check_circular_extends(&interface_decls, &map, cm, path) {
        errs.push(e);
        return map; // 有循环继承，提前返回
    }
    
    // 第三阶段：解析接口（属性类型和 extends 都可以引用其他接口）
    let mut resolver = ExtendsResolver {
        map: &mut map,
        resolving: HashSet::new(),
    };
    for d in &interface_decls {
        if let Err(e) = resolve_interface(d, &mut resolver, cm, path) {
            errs.push(e);
        }
    }
    
    // 第三阶段：解析类型别名（可以引用接口）
    for d in &type_alias_decls {
        if let Err(e) = resolve_type_alias(d, &mut map, cm, path) {
            errs.push(e);
        }
    }
    
    map
}

/// B2+: 解析单个接口（此时所有接口名已在 map 中）
/// B2+: 检测循环继承
fn check_circular_extends(
    decls: &[&TsInterfaceDecl],
    map: &HashMap<String, TsType>,
    cm: &Lrc<SourceMap>,
    path: &str,
) -> Result<(), CompileError> {
    // 构建 extends 图
    let mut extends_graph: HashMap<String, Option<String>> = HashMap::new();
    for d in decls {
        let name = d.id.sym.to_string();
        let extends = if d.extends.is_empty() {
            None
        } else if d.extends.len() > 1 {
            None // 多个 extends 会在解析时报错
        } else {
            match d.extends[0].expr.as_ref() {
                Expr::Ident(id) => {
                    let parent = id.sym.to_string();
                    if map.contains_key(&parent) {
                        Some(parent)
                    } else {
                        None
                    }
                }
                _ => None,
            }
        };
        extends_graph.insert(name, extends);
    }
    
    // 检测循环
    fn check_cycle(
        name: &str,
        graph: &HashMap<String, Option<String>>,
        visiting: &mut Vec<String>,
        visited: &mut HashSet<String>,
    ) -> Option<String> {
        if visiting.contains(&name.to_string()) {
            return Some(name.to_string());
        }
        if visited.contains(name) {
            return None;
        }
        visiting.push(name.to_string());
        if let Some(Some(parent)) = graph.get(name) {
            if let Some(cycle) = check_cycle(parent, graph, visiting, visited) {
                return Some(cycle);
            }
        }
        visiting.pop();
        visited.insert(name.to_string());
        None
    }
    
    let mut visited = HashSet::new();
    for name in extends_graph.keys() {
        let mut visiting = Vec::new();
        if let Some(cycle_name) = check_cycle(name, &extends_graph, &mut visiting, &mut visited) {
            // 找到循环，查找对应的接口定义获取 span
            for d in decls {
                if d.id.sym == cycle_name {
                    return Err(diag(
                        cm,
                        path,
                        d.id.span,
                        format!("circular interface inheritance detected involving `{}`", cycle_name),
                    ));
                }
            }
        }
    }
    
    Ok(())
}

/// 用于检测循环继承的状态
struct ExtendsResolver<'a> {
    map: &'a mut HashMap<String, TsType>,
    resolving: HashSet<String>,
}

fn resolve_interface(
    d: &&TsInterfaceDecl,
    resolver: &mut ExtendsResolver,
    cm: &Lrc<SourceMap>,
    path: &str,
) -> Result<(), CompileError> {
    let name = d.id.sym.to_string();
    
    // 检测循环继承
    if resolver.resolving.contains(&name) {
        return Err(diag(
            cm,
            path,
            d.id.span,
            format!("circular interface inheritance detected involving `{}`", name),
        ));
    }
    
    // 解析 extends
    let extends = if d.extends.is_empty() {
        None
    } else if d.extends.len() > 1 {
        return Err(diag(
            cm,
            path,
            d.extends[1].span,
            "multiple interface extends are not supported",
        ));
    } else {
        match d.extends[0].expr.as_ref() {
            Expr::Ident(id) => {
                let parent_name = id.sym.to_string();
                // 检查父接口是否存在
                if !resolver.map.contains_key(&parent_name) {
                    return Err(diag(
                        cm,
                        path,
                        d.extends[0].span,
                        format!("interface extends unknown type `{}`", parent_name),
                    ));
                }
                Some(parent_name)
            }
            _ => {
                return Err(diag(
                    cm,
                    path,
                    d.extends[0].span,
                    "complex interface extends expressions are not supported",
                ));
            }
        }
    };
    
    // 解析自己的属性（可以引用其他接口，因为所有接口名已在 map 中）
    let own_props = object_props_from_type_elements(&d.body.body, cm, path, d.body.span, resolver.map, None)?;
    
    // 如果有 extends，合并父接口属性
    let props = if let Some(ref parent_name) = extends {
        resolver.resolving.insert(name.clone());
        let parent_props = get_interface_props(parent_name, resolver, cm, path, d.span)?;
        resolver.resolving.remove(&name);
        merge_interface_props(&parent_props, &own_props)
    } else {
        own_props
    };
    
    // 更新 map 中的接口定义
    resolver.map.insert(name, TsType::Interface {
        name: d.id.sym.to_string(),
        extends,
        props,
    });
    
    Ok(())
}

/// B2+: 获取接口的属性（用于 extends 合并）
fn get_interface_props(
    name: &str,
    resolver: &mut ExtendsResolver,
    cm: &Lrc<SourceMap>,
    path: &str,
    span: Span,
) -> Result<Vec<ObjectProp>, CompileError> {
    match resolver.map.get(name) {
        Some(TsType::Interface { props, extends, .. }) => {
            // 如果属性为空但有 extends，需要递归解析
            if props.is_empty() && extends.is_some() {
                // 返回空，让上层处理递归
                Ok(vec![])
            } else {
                Ok(props.clone())
            }
        }
        Some(TsType::ObjectNum(props)) => Ok(props.clone()),
        _ => Err(diag(
            cm,
            path,
            span,
            format!("`{}` is not an interface type", name),
        )),
    }
}

/// B2+: 合并父接口和子接口的属性（子覆盖父）
fn merge_interface_props(parent: &[ObjectProp], child: &[ObjectProp]) -> Vec<ObjectProp> {
    let mut result = parent.to_vec();
    for c in child {
        if let Some(idx) = result.iter().position(|p| p.name == c.name) {
            result[idx] = c.clone(); // 子覆盖父
        } else {
            result.push(c.clone());
        }
    }
    // 排序以确保确定性
    result.sort_by(|a, b| a.name.cmp(&b.name));
    result
}

/// B2+: 解析类型别名
fn resolve_type_alias(
    d: &&TsTypeAliasDecl,
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
                kind: ObjectMemberKind::Field(Box::new(ty)),
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

/// B2+: 收集接口定义（第一阶段），提取 extends 信息但不展平

/// 从类型元素收集属性
/// B2+: 添加 iface 参数以支持接口类型引用
fn object_props_from_type_elements(
    members: &[TsTypeElement],
    cm: &Lrc<SourceMap>,
    path: &str,
    dup_span: Span,
    iface: &HashMap<String, TsType>,
    type_params: Option<&HashSet<String>>,
) -> Result<Vec<ObjectProp>, CompileError> {
    let mut props: Vec<ObjectProp> = Vec::new();
    for m in members {
        match m {
            TsTypeElement::TsPropertySignature(p) => {
                let key = match &*p.key {
                    Expr::Ident(i) => i.sym.to_string(),
                    Expr::Lit(Lit::Str(s)) => s.value.to_string_lossy().into_owned(),
                    _ => {
                        return Err(diag(
                            cm,
                            path,
                            p.key.span(),
                            "unsupported property key in interface",
                        ));
                    }
                };
                let ft = if let Some(type_ann) = &p.type_ann {
                    ts_type_from_ast(&type_ann.type_ann, cm, path, iface, type_params)?
                } else {
                    TsType::Void
                };
                props.push(ObjectProp {
                    name: key,
                    optional: p.optional,
                    kind: crate::ir::ObjectMemberKind::Field(Box::new(ft)),
                });
            }
            TsTypeElement::TsMethodSignature(m) => {
                // R1: 支持接口方法签名
                let key = match &*m.key {
                    Expr::Ident(i) => i.sym.to_string(),
                    _ => {
                        return Err(diag(
                            cm,
                            path,
                            m.span,
                            "method name must be an identifier",
                        ));
                    }
                };
                // R1 v0: 拒绝泛型方法
                if m.type_params.is_some() {
                    return Err(diag(
                        cm,
                        path,
                        m.span,
                        "generic methods are not supported in interface (R1 v0)",
                    ));
                }
                // 解析参数类型
                let mut params = Vec::new();
                for param in &m.params {
                    let ty = ts_type_from_fn_param(param, cm, path, iface, type_params)?;
                    params.push(ty);
                }
                // 解析返回类型
                let ret = if let Some(type_ann) = &m.type_ann {
                    ts_type_from_ast(&type_ann.type_ann, cm, path, iface, type_params)?
                } else {
                    TsType::Void
                };
                props.push(ObjectProp {
                    name: key,
                    optional: false,
                    kind: crate::ir::ObjectMemberKind::Method { params, ret: Box::new(ret) },
                });
            }
            _ => {}
        }
    }
    
    // 检查重复
    props.sort_by(|a, b| a.name.cmp(&b.name));
    for w in props.windows(2) {
        if w[0].name == w[1].name {
            return Err(diag(
                cm,
                path,
                dup_span,
                format!("duplicate property name `{}` in interface", w[0].name),
            ));
        }
    }
    
    Ok(props)
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
        match m {
            TsTypeElement::TsPropertySignature(p) => {
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
                    kind: crate::ir::ObjectMemberKind::Field(Box::new(ft)),
                });
            }
            TsTypeElement::TsMethodSignature(m) => {
                // R1: 支持 interface 方法签名
                let key = match &*m.key {
                    Expr::Ident(i) => i.sym.to_string(),
                    _ => {
                        return Err(diag(
                            cm,
                            path,
                            m.span,
                            "method name must be an identifier",
                        ));
                    }
                };
                // R1 v0: 拒绝泛型方法
                if m.type_params.is_some() {
                    return Err(diag(
                        cm,
                        path,
                        m.span,
                        "generic methods are not supported in interface (R1 v0)",
                    ));
                }
                // R1 v0: 拒绝方法重载（通过检查重复名）
                if props.iter().any(|p| p.name == key) {
                    return Err(diag(
                        cm,
                        path,
                        m.span,
                        format!("method `{}` conflicts with existing field or method", key),
                    ));
                }
                // 解析参数类型
                let mut params = Vec::new();
                for param in &m.params {
                    let ty = ts_type_from_fn_param(param, cm, path, iface, type_params)?;
                    params.push(ty);
                }
                // 解析返回类型
                let ret = if let Some(type_ann) = &m.type_ann {
                    ts_type_from_ast(&type_ann.type_ann, cm, path, iface, type_params)?
                } else {
                    TsType::Void
                };
                props.push(ObjectProp {
                    name: key,
                    optional: false, // 方法默认非可选
                    kind: crate::ir::ObjectMemberKind::Method {
                        params,
                        ret: Box::new(ret),
                    },
                });
            }
            _ => {
                return Err(diag(
                    cm,
                    path,
                    m.span(),
                    "only property and method signatures are supported in interface",
                ));
            }
        }
    }
    props.sort_by(|a, b| a.name.cmp(&b.name));
    for w in props.windows(2) {
        if w[0].name == w[1].name {
            return Err(diag(
                cm,
                path,
                dup_span,
                format!("duplicate object type member `{}`", w[0].name),
            ));
        }
    }
    Ok(TsType::ObjectNum(props))
}
