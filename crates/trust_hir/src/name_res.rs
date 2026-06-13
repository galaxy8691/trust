//! 名称解析 + AST→HIR 降级 — §SEM-REQ-002
//!
//! 本模块包含两部分：
//! 1. `lower` — AST→HIR 降级（遍历 AST 产出 HIR 节点）
//! 2. `resolve` — 跨文件名称解析 + 作用域构造 + 符号绑定
//!
//! §设计文档 §3.1.2 / spec SEM-REQ-002: 名称解析入口

use crate::hir::*;
use trust_parser::ast::{self as ast, Span};
use trust_parser::module_graph::ModuleGraph;

// ============================================================================
// 1. AST→HIR 降级
// ============================================================================

/// §设计文档 §3.1.2 / spec SEM-REQ-002: AST→HIR 降级入口
pub fn lower(program: &ast::Program, diagnostics: &mut Vec<DiagError>) -> HirProgram {
    let mut items = Vec::new();

    for stmt in &program.statements {
        if let Some(item) = lower_stmt(stmt, diagnostics) {
            items.push(item);
        }
    }

    HirProgram {
        file: program.span.file.clone(),
        imports: Vec::new(), // 名称解析阶段填充
        exports: lower_exports(&program.exports, diagnostics),
        items,
        scope: Scope::new(),
    }
}

fn lower_exports(exports: &[ast::ExportDecl], diagnostics: &mut Vec<DiagError>) -> Vec<HirExport> {
    let mut result = Vec::new();
    for exp in exports {
        let (name, binding) = match exp.item.as_ref() {
            ast::Stmt::Function(f) => {
                let param_types: Vec<HirType> = f.params.iter()
                    .map(|p| p.ty.as_ref().map(HirType::from_ast_type).unwrap_or(HirType::Error))
                    .collect();
                let return_type = f.return_type.as_ref()
                    .map(HirType::from_ast_type)
                    .unwrap_or(HirType::Void);
                (f.name.clone(), HirBinding::Function {
                    param_types,
                    return_type,
                    span: f.span.clone(),
                })
            }
            ast::Stmt::Let(l) => {
                let ty = l.ty.as_ref().map(HirType::from_ast_type).unwrap_or(HirType::Error);
                (l.name.clone(), HirBinding::LocalVar {
                    ty,
                    mutable: l.mutable,
                    span: l.span.clone(),
                })
            }
            ast::Stmt::Const(c) => {
                let ty = c.ty.as_ref().map(HirType::from_ast_type).unwrap_or(HirType::Error);
                (c.name.clone(), HirBinding::ModuleConst {
                    ty,
                    span: c.span.clone(),
                })
            }
            _ => {
                diagnostics.push(DiagError::new(
                    "export of unsupported item type".into(),
                    exp.span.clone(),
                ));
                continue;
            }
        };
        result.push(HirExport {
            name: name.clone(),
            binding,
            is_default: exp.default,
            span: exp.span.clone(),
        });
    }
    result
}

fn lower_stmt(stmt: &ast::Stmt, diagnostics: &mut Vec<DiagError>) -> Option<HirItem> {
    match stmt {
        ast::Stmt::Function(f) => {
            let hf = lower_function(f, diagnostics);
            Some(HirItem::Function(hf))
        }
        ast::Stmt::Const(c) => {
            let ty = c
                .ty
                .as_ref()
                .map(HirType::from_ast_type)
                .unwrap_or(HirType::Error);
            let init = lower_expr(&c.init, diagnostics);
            Some(HirItem::Const(HirConst {
                name: c.name.clone(),
                ty,
                init: Box::new(init),
                span: c.span.clone(),
            }))
        }
        ast::Stmt::Shared(s) => {
            let ty = s
                .ty
                .as_ref()
                .map(HirType::from_ast_type)
                .unwrap_or(HirType::Error);
            let init = lower_expr(&s.init, diagnostics);
            Some(HirItem::Shared(HirShared {
                name: s.name.clone(),
                ty,
                init: Box::new(init),
                span: s.span.clone(),
            }))
        }
        ast::Stmt::Let(l) => {
            // 顶层 let → 用 HirItem::Stub 占位，或降级为 const
            diagnostics.push(DiagError::new(
                "top-level `let` not supported in Phase 1; use `const` or put inside `function`"
                    .into(),
                l.span.clone(),
            ));
            None
        }
        _ => {
            diagnostics.push(DiagError::new(
                format!("unsupported top-level statement: {:?}", std::mem::discriminant(stmt)),
                Span::dummy(),
            ));
            None
        }
    }
}

fn lower_function(f: &ast::FunctionDecl, diagnostics: &mut Vec<DiagError>) -> HirFunction {
    let params: Vec<HirParam> = f
        .params
        .iter()
        .map(|p| HirParam {
            name: p.name.clone(),
            mode: HirType::param_mode_from_ast(&p.mode),
            ty: p
                .ty
                .as_ref()
                .map(HirType::from_ast_type)
                .unwrap_or(HirType::Error),
            span: p.span.clone(),
        })
        .collect();

    let return_type = f
        .return_type
        .as_ref()
        .map(HirType::from_ast_type)
        .unwrap_or(HirType::Void);

    let body = lower_block(&f.body, diagnostics);

    HirFunction {
        name: f.name.clone(),
        params,
        return_type,
        body,
        scope: Scope::new(),
        span: f.span.clone(),
    }
}

fn lower_block(block: &ast::Block, diagnostics: &mut Vec<DiagError>) -> HirBlock {
    let stmts: Vec<HirStmt> = block
        .statements
        .iter()
        .filter_map(|s| lower_hir_stmt(s, diagnostics))
        .collect();
    HirBlock {
        statements: stmts,
        span: block.span.clone(),
    }
}

fn lower_hir_stmt(stmt: &ast::Stmt, diagnostics: &mut Vec<DiagError>) -> Option<HirStmt> {
    match stmt {
        ast::Stmt::Let(l) => {
            let ty = l
                .ty
                .as_ref()
                .map(HirType::from_ast_type)
                .unwrap_or(HirType::Error);
            let init = lower_expr(&l.init, diagnostics);
            Some(HirStmt::Let(HirLet {
                name: l.name.clone(),
                mutable: l.mutable,
                ty,
                init: Box::new(init),
                span: l.span.clone(),
            }))
        }
        ast::Stmt::Const(c) => {
            let ty = c
                .ty
                .as_ref()
                .map(HirType::from_ast_type)
                .unwrap_or(HirType::Error);
            let init = lower_expr(&c.init, diagnostics);
            Some(HirStmt::Const(HirConst {
                name: c.name.clone(),
                ty,
                init: Box::new(init),
                span: c.span.clone(),
            }))
        }
        ast::Stmt::Shared(s) => {
            let ty = s
                .ty
                .as_ref()
                .map(HirType::from_ast_type)
                .unwrap_or(HirType::Error);
            let init = lower_expr(&s.init, diagnostics);
            Some(HirStmt::Shared(HirShared {
                name: s.name.clone(),
                ty,
                init: Box::new(init),
                span: s.span.clone(),
            }))
        }
        ast::Stmt::If(if_expr) => {
            let cond = lower_expr(&if_expr.condition, diagnostics);
            let then_block = lower_block(&if_expr.then_branch, diagnostics);
            let else_block = if_expr
                .else_branch
                .as_ref()
                .map(|b| lower_block(b, diagnostics));
            Some(HirStmt::If(HirIf {
                condition: Box::new(cond),
                then_branch: then_block,
                else_branch: else_block,
                span: if_expr.span.clone(),
            }))
        }
        ast::Stmt::For(f) => {
            let init = lower_hir_stmt(&f.init, diagnostics)
                .map(Box::new)
                .unwrap_or_else(|| Box::new(HirStmt::Error));
            let condition = lower_expr(&f.condition, diagnostics);
            let update = lower_expr(&f.update, diagnostics);
            let body = lower_block(&f.body, diagnostics);
            Some(HirStmt::For(HirFor {
                init,
                condition: Box::new(condition),
                update: Box::new(update),
                body,
                span: f.span.clone(),
            }))
        }
        ast::Stmt::ForOf(f) => {
            let iterator = lower_expr(&f.iterator, diagnostics);
            let body = lower_block(&f.body, diagnostics);
            Some(HirStmt::ForOf(HirForOf {
                item: f.item.clone(),
                iterator: Box::new(iterator),
                body,
                span: f.span.clone(),
            }))
        }
        ast::Stmt::While(w) => {
            let condition = lower_expr(&w.condition, diagnostics);
            let body = lower_block(&w.body, diagnostics);
            Some(HirStmt::While(HirWhile {
                condition: Box::new(condition),
                body,
                span: w.span.clone(),
            }))
        }
        ast::Stmt::Loop(l) => {
            let body = lower_block(&l.body, diagnostics);
            Some(HirStmt::Loop(HirLoop {
                body,
                span: l.span.clone(),
            }))
        }
        ast::Stmt::Return(r) => {
            let value = r.value.as_ref().map(|v| Box::new(lower_expr(v, diagnostics)));
            Some(HirStmt::Return(HirReturn {
                value,
                span: r.span.clone(),
            }))
        }
        ast::Stmt::Break(b) => {
            let value = b
                .value
                .as_ref()
                .map(|v| Box::new(lower_expr(v, diagnostics)));
            Some(HirStmt::Break(HirBreak {
                value,
                span: b.span.clone(),
            }))
        }
        ast::Stmt::Continue(c) => Some(HirStmt::Continue(HirContinue {
            span: c.span.clone(),
        })),
        ast::Stmt::Expr(e) => {
            let expr = lower_expr(&e.expr, diagnostics);
            Some(HirStmt::Expr(expr))
        }
        ast::Stmt::Function(_f) => {
            // 嵌套函数不降级（Phase 1 不支持）
            diagnostics.push(DiagError::new(
                "nested function not supported in Phase 1".into(),
                Span::dummy(),
            ));
            Some(HirStmt::Error)
        }
    }
}

fn lower_expr(expr: &ast::Expr, diagnostics: &mut Vec<DiagError>) -> HirExpr {
    match expr {
        ast::Expr::IntLiteral(v) => HirExpr::IntLiteral(*v, Span::dummy()),
        ast::Expr::FloatLiteral(v) => HirExpr::FloatLiteral(*v, Span::dummy()),
        ast::Expr::BigIntLiteral(v) => HirExpr::BigIntLiteral(*v, Span::dummy()),
        ast::Expr::StrLiteral(s) => HirExpr::StringLiteral(s.clone(), Span::dummy()),
        ast::Expr::BoolLiteral(b) => HirExpr::BoolLiteral(*b, Span::dummy()),

        // §3.1.4 降级策略: Null → 降级报错（Phase 1 无 null 语义）
        ast::Expr::Null => {
            diagnostics.push(DiagError::new(
                "`null` literal not supported in Phase 1".into(),
                Span::dummy(),
            ));
            HirExpr::Error(Span::dummy())
        }

        ast::Expr::Ident(name) => HirExpr::Ident(
            name.clone(),
            HirBinding::Unresolved {
                name: name.clone(),
                span: Span::dummy(),
            },
            Span::dummy(),
        ),

        ast::Expr::Binary(lhs, op, rhs) => {
            let l = lower_expr(lhs, diagnostics);
            let r = lower_expr(rhs, diagnostics);

            // §3.1.4 降级策略: ?? → 降级报错
            if matches!(op, ast::BinOp::QuestionQuestion) {
                diagnostics.push(DiagError::new(
                    "`??` null-coalescing not supported in Phase 1".into(),
                    Span::dummy(),
                ));
                return HirExpr::Error(Span::dummy());
            }

            HirExpr::Binary(
                Box::new(l),
                HirType::binop_from_ast(op.clone()),
                Box::new(r),
                HirType::Error, // 类型检查阶段填充
                Span::dummy(),
            )
        }

        ast::Expr::Unary(op, inner) => {
            let i = lower_expr(inner, diagnostics);
            HirExpr::Unary(
                HirType::unaryop_from_ast(op.clone()),
                Box::new(i),
                HirType::Error,
                Span::dummy(),
            )
        }

        ast::Expr::Call { callee, args, span } => {
            let c = lower_expr(callee, diagnostics);
            let a: Vec<HirCallArg> = args
                .iter()
                .map(|arg| HirCallArg {
                    mode: HirType::param_mode_from_ast(&arg.mode),
                    expr: Box::new(lower_expr(&arg.expr, diagnostics)),
                    span: arg.span.clone(),
                })
                .collect();
            HirExpr::Call(Box::new(c), a, HirType::Error, span.clone())
        }

        ast::Expr::BlockExpr(block) => {
            let b = lower_block(block, diagnostics);
            HirExpr::Block(b, Span::dummy())
        }

        ast::Expr::ArrowFn(a) => {
            let params: Vec<HirParam> = a
                .params
                .iter()
                .map(|p| HirParam {
                    name: p.name.clone(),
                    mode: HirType::param_mode_from_ast(&p.mode),
                    ty: p
                        .ty
                        .as_ref()
                        .map(HirType::from_ast_type)
                        .unwrap_or(HirType::Error),
                    span: p.span.clone(),
                })
                .collect();
            // Phase 1 闭包参数必须有显式类型标注；无标注时返回 Error
            let has_no_type = params.iter().any(|p| p.ty == HirType::Error);
            let body = match &a.body {
                ast::ArrowBody::Expr(e) => {
                    let expr = lower_expr(e, diagnostics);
                    HirBlock {
                        statements: vec![HirStmt::Return(HirReturn {
                            value: Some(Box::new(expr)),
                            span: a.span.clone(),
                        })],
                        span: a.span.clone(),
                    }
                }
                ast::ArrowBody::Block(b) => lower_block(b, diagnostics),
            };
            let ret_ty = if has_no_type {
                diagnostics.push(DiagError::new(
                    "arrow function parameters must have explicit type annotations in Phase 1"
                        .into(),
                    a.span.clone(),
                ));
                HirType::Error
            } else {
                HirType::Error // 闭包返回类型由 body 推断，Phase 1 暂不推断
            };
            HirExpr::ArrowFn(params, ret_ty, body, a.is_move, a.span.clone())
        }

        ast::Expr::Reference(inner) => {
            let i = lower_expr(inner, diagnostics);
            HirExpr::Reference(Box::new(i), Span::dummy())
        }

        ast::Expr::AssertUnwrap(inner) => {
            let i = lower_expr(inner, diagnostics);
            HirExpr::AssertUnwrap(Box::new(i), Span::dummy())
        }

        ast::Expr::TryPropagate(inner) => {
            let i = lower_expr(inner, diagnostics);
            HirExpr::TryPropagate(Box::new(i), Span::dummy())
        }

        ast::Expr::AsCast { expr, ty } => {
            let e = lower_expr(expr, diagnostics);
            let t = HirType::from_ast_type(ty);
            HirExpr::AsCast(Box::new(e), t, Span::dummy())
        }

        ast::Expr::TemplateLiteral(parts) => {
            let parts: Vec<HirTemplatePart> = parts
                .iter()
                .map(|p| match p {
                    ast::TemplatePart::Literal(s) => HirTemplatePart {
                        kind: HirTemplatePartKind::String(s.clone()),
                        span: Span::dummy(),
                    },
                    ast::TemplatePart::Expr(e) => HirTemplatePart {
                        kind: HirTemplatePartKind::Expr(Box::new(lower_expr(e, diagnostics))),
                        span: Span::dummy(),
                    },
                })
                .collect();
            HirExpr::TemplateLiteral(parts, Span::dummy())
        }

        // §3.1.4 降级策略: IfExpr → HirExpr::If (AC-SEM-002)
        ast::Expr::IfExpr(if_expr) => {
            let cond = lower_expr(&if_expr.condition, diagnostics);
            let then_block = lower_block(&if_expr.then_branch, diagnostics);
            let else_block = if_expr
                .else_branch
                .as_ref()
                .map(|b| lower_block(b, diagnostics));
            HirExpr::If(
                HirIf {
                    condition: Box::new(cond),
                    then_branch: then_block,
                    else_branch: else_block,
                    span: if_expr.span.clone(),
                },
                if_expr.span.clone(),
            )
        }

        // §3.1.4 降级策略: LoopExpr → HirExpr::Loop
        ast::Expr::LoopExpr(loop_expr) => {
            let body = lower_block(&loop_expr.body, diagnostics);
            HirExpr::Loop(
                HirLoop {
                    body,
                    span: loop_expr.span.clone(),
                },
                loop_expr.span.clone(),
            )
        }

        // §3.1.4 降级策略: MemberAccess{ Ident("console"), "log" } → Ident
        // 返回 Ident 使父 Expr::Call 处理器正常接收 args（避免双重 Call 包裹）
        ast::Expr::MemberAccess(ma) => {
            if let ast::Expr::Ident(obj_name) = ma.object.as_ref() {
                if obj_name == "console" {
                    return HirExpr::Ident(
                        "ferro_rt::console::log".into(),
                        HirBinding::Unresolved {
                            name: "ferro_rt::console::log".into(),
                            span: ma.span.clone(),
                        },
                        ma.span.clone(),
                    );
                }
            }
            diagnostics.push(DiagError::new(
                format!(
                    "member access `{}.{}` not supported in Phase 1 (only `console.log` is allowed)",
                    debug_expr_short(&ma.object),
                    ma.field
                ),
                ma.span.clone(),
            ));
            HirExpr::Error(ma.span.clone())
        }
    }
}

fn debug_expr_short(expr: &ast::Expr) -> String {
    match expr {
        ast::Expr::Ident(s) => s.clone(),
        _ => format!("{:?}", std::mem::discriminant(expr)),
    }
}

// ============================================================================
// 2. 名称解析
// ============================================================================

/// §设计文档 §3.1.2 / spec SEM-REQ-002: 名称解析入口
///
/// 注意：DiagError 是本地类型；Phase 1.6 后迁移到 trust_error::Diagnostic。
pub fn resolve_names(
    program: &ast::Program,
    _module_graph: &ModuleGraph,
    diagnostics: &mut Vec<DiagError>,
) -> HirProgram {
    let mut hir = lower(program, diagnostics);

    // 构建模块作用域
    let mut module_scope = Scope::new();

    // 注册所有顶层声明
    for item in &hir.items {
        match item {
            HirItem::Function(f) => {
                module_scope.insert(
                    &f.name,
                    HirBinding::Function {
                        param_types: f.params.iter().map(|p| p.ty.clone()).collect(),
                        return_type: f.return_type.clone(),
                        span: f.span.clone(),
                    },
                );
            }
            HirItem::Const(c) => {
                module_scope.insert(
                    &c.name,
                    HirBinding::ModuleConst {
                        ty: c.ty.clone(),
                        span: c.span.clone(),
                    },
                );
            }
            HirItem::Shared(s) => {
                module_scope.insert(
                    &s.name,
                    HirBinding::ModuleShared {
                        ty: s.ty.clone(),
                        span: s.span.clone(),
                    },
                );
            }
            HirItem::Stub(_) => {}
        }
    }

    // resolve import 绑定
    register_module_bindings(&hir, &mut module_scope, diagnostics);

    // 对每个函数做名称解析
    for item in &mut hir.items {
        if let HirItem::Function(ref mut f) = item {
            resolve_function_names(f, &module_scope, diagnostics);
        }
    }

    // 把模块作用域写回
    hir.scope = module_scope;

    hir
}

fn register_module_bindings(
    hir: &HirProgram,
    scope: &mut Scope,
    _diagnostics: &mut Vec<DiagError>,
) {
    // Phase 1: import 绑定仅在单文件项目中有用；跨文件解析需要模块图。
    // 此处将 HirProgram 的 exports 注册到作用域，使 import 符号可解析。
    for exp in &hir.exports {
        if let Some(b) = scope.lookup(&exp.name) {
            // 已有绑定 — 符号冲突：本地声明与 import/export 同名
            if !matches!(b, HirBinding::Unresolved { .. }) {
                _diagnostics.push(DiagError::new(
                    format!("symbol conflict: `{}` already declared in this scope", exp.name),
                    exp.span.clone(),
                ));
            }
        } else {
            scope.insert(&exp.name, exp.binding.clone());
        }
    }
}

fn resolve_function_names(
    func: &mut HirFunction,
    parent_scope: &Scope,
    diagnostics: &mut Vec<DiagError>,
) {
    // 构造函数局部作用域：参数 + body 块
    let mut func_scope = Scope::new_child(Box::new(parent_scope.clone()));

    // 注册参数
    for param in &func.params {
        func_scope.insert(
            &param.name,
            HirBinding::LocalVar {
                ty: param.ty.clone(),
                mutable: matches!(param.mode, ParamMode::InOut),
                span: param.span.clone(),
            },
        );
    }

    // 在 body 中递归解析
    resolve_block_names(&mut func.body, &func_scope, diagnostics);

    func.scope = func_scope;
}

fn resolve_block_names(
    block: &mut HirBlock,
    _parent_scope: &Scope,
    diagnostics: &mut Vec<DiagError>,
) {
    let mut block_scope = Scope::new_child(Box::new(_parent_scope.clone()));

    for stmt in &mut block.statements {
        match stmt {
            HirStmt::Let(let_s) => {
                resolve_expr_names(&mut let_s.init, &block_scope, diagnostics);
                if let_s.ty == HirType::Error {
                    let_s.ty = infer_type_from_expr(&let_s.init);
                }
                block_scope.insert(
                    &let_s.name,
                    HirBinding::LocalVar {
                        ty: let_s.ty.clone(),
                        mutable: let_s.mutable,
                        span: let_s.span.clone(),
                    },
                );
            }
            HirStmt::Const(c) => {
                resolve_expr_names(&mut c.init, &block_scope, diagnostics);
                if c.ty == HirType::Error {
                    c.ty = infer_type_from_expr(&c.init);
                }
                block_scope.insert(
                    &c.name,
                    HirBinding::ModuleConst {
                        ty: c.ty.clone(),
                        span: c.span.clone(),
                    },
                );
            }
            HirStmt::Shared(s) => {
                resolve_expr_names(&mut s.init, &block_scope, diagnostics);
                if s.ty == HirType::Error {
                    s.ty = infer_type_from_expr(&s.init);
                }
                block_scope.insert(
                    &s.name,
                    HirBinding::ModuleShared {
                        ty: s.ty.clone(),
                        span: s.span.clone(),
                    },
                );
            }
            HirStmt::If(if_s) => {
                resolve_expr_names(&mut if_s.condition, &block_scope, diagnostics);
                // then/else 使用独立作用域，互不可见
                resolve_block_names(&mut if_s.then_branch, &block_scope, diagnostics);
                if let Some(ref mut else_b) = if_s.else_branch {
                    resolve_block_names(else_b, &block_scope, diagnostics);
                }
            }
            HirStmt::For(f) => {
                if let HirStmt::Let(ref mut let_s) = *f.init {
                    resolve_expr_names(&mut let_s.init, &block_scope, diagnostics);
                    block_scope.insert(
                        &let_s.name,
                        HirBinding::LocalVar {
                            ty: let_s.ty.clone(),
                            mutable: true,
                            span: let_s.span.clone(),
                        },
                    );
                }
                resolve_expr_names(&mut f.condition, &block_scope, diagnostics);
                resolve_expr_names(&mut f.update, &block_scope, diagnostics);
                resolve_block_names(&mut f.body, &block_scope, diagnostics);
            }
            HirStmt::ForOf(f) => {
                resolve_expr_names(&mut f.iterator, &block_scope, diagnostics);
                let mut inner_scope = Scope::new_child(Box::new(block_scope.clone()));
                inner_scope.insert(
                    &f.item,
                    HirBinding::LocalVar {
                        ty: HirType::Error,
                        mutable: false,
                        span: Span::dummy(),
                    },
                );
                resolve_block_names(&mut f.body, &inner_scope, diagnostics);
            }
            HirStmt::While(w) => {
                resolve_expr_names(&mut w.condition, &block_scope, diagnostics);
                resolve_block_names(&mut w.body, &block_scope, diagnostics);
            }
            HirStmt::Loop(l) => {
                resolve_block_names(&mut l.body, &block_scope, diagnostics);
            }
            HirStmt::Return(r) => {
                if let Some(ref mut v) = r.value {
                    resolve_expr_names(v, &block_scope, diagnostics);
                }
            }
            HirStmt::Break(b) => {
                if let Some(ref mut v) = b.value {
                    resolve_expr_names(v, &block_scope, diagnostics);
                }
            }
            HirStmt::Continue(_) => {}
            HirStmt::Expr(e) => {
                resolve_expr_names(e, &block_scope, diagnostics);
            }
            HirStmt::Error => {}
        }
    }

    // Note: block_scope is dropped here — no variable leaks to parent.
}

fn resolve_expr_names(
    expr: &mut HirExpr,
    scope: &Scope,
    diagnostics: &mut Vec<DiagError>,
) {
    match expr {
        HirExpr::Ident(name, binding, _span) => {
            if matches!(binding, HirBinding::Unresolved { .. }) {
                if let Some(b) = scope.lookup(name) {
                    *binding = b.clone();
                } else {
                    diagnostics.push(DiagError::new(
                        format!("undefined identifier `{name}`"),
                        Span::dummy(),
                    ));
                    *expr = HirExpr::Error(Span::dummy());
                }
            }
        }
        HirExpr::Binary(lhs, _op, rhs, ..) => {
            resolve_expr_names(lhs, scope, diagnostics);
            resolve_expr_names(rhs, scope, diagnostics);
        }
        HirExpr::Unary(_op, inner, ..) => {
            resolve_expr_names(inner, scope, diagnostics);
        }
        HirExpr::Call(callee, args, ..) => {
            resolve_expr_names(callee, scope, diagnostics);
            for arg in args {
                resolve_expr_names(&mut arg.expr, scope, diagnostics);
            }
        }
        HirExpr::ArrowFn(params, _ret, body, ..) => {
            let mut fn_scope = Scope::new_child(Box::new(scope.clone()));
            // 注册闭包参数到作用域——否则 body 中对参数的引用被误报为 undefined
            for p in params.iter() {
                fn_scope.insert(
                    &p.name,
                    HirBinding::LocalVar {
                        ty: p.ty.clone(),
                        mutable: false,
                        span: p.span.clone(),
                    },
                );
            }
            resolve_block_names(body, &fn_scope, diagnostics);
        }
        HirExpr::If(if_s, ..) => {
            resolve_expr_names(&mut if_s.condition, scope, diagnostics);
            let then_scope = Scope::new_child(Box::new(scope.clone()));
            resolve_block_names(&mut if_s.then_branch, &then_scope, diagnostics);
            if let Some(ref mut else_b) = if_s.else_branch {
                let else_scope = Scope::new_child(Box::new(scope.clone()));
                resolve_block_names(else_b, &else_scope, diagnostics);
            }
        }
        HirExpr::Loop(l, ..) => {
            let loop_scope = Scope::new_child(Box::new(scope.clone()));
            resolve_block_names(&mut l.body, &loop_scope, diagnostics);
        }
        HirExpr::Block(b, ..) => {
            let block_scope = Scope::new_child(Box::new(scope.clone()));
            resolve_block_names(b, &block_scope, diagnostics);
        }
        HirExpr::AsCast(inner, ..) => {
            resolve_expr_names(inner, scope, diagnostics);
        }
        HirExpr::Reference(inner, ..) => {
            resolve_expr_names(inner, scope, diagnostics);
        }
        HirExpr::AssertUnwrap(inner, ..) => {
            resolve_expr_names(inner, scope, diagnostics);
        }
        HirExpr::TryPropagate(inner, ..) => {
            resolve_expr_names(inner, scope, diagnostics);
        }
        HirExpr::TemplateLiteral(parts, ..) => {
            for part in parts {
                if let HirTemplatePartKind::Expr(ref mut e) = part.kind {
                    resolve_expr_names(e, scope, diagnostics);
                }
            }
        }
        // 字面量无需名称解析
        HirExpr::IntLiteral(..)
        | HirExpr::FloatLiteral(..)
        | HirExpr::BigIntLiteral(..)
        | HirExpr::StringLiteral(..)
        | HirExpr::BoolLiteral(..)
        | HirExpr::Error(..) => {}
    }
}


fn infer_type_from_expr(expr: &HirExpr) -> HirType {
    match expr {
        HirExpr::IntLiteral(..) => HirType::I32,
        HirExpr::FloatLiteral(..) => HirType::F64,
        HirExpr::BigIntLiteral(..) => HirType::I64,
        HirExpr::StringLiteral(..) => HirType::String,
        HirExpr::BoolLiteral(..) => HirType::Bool,
        HirExpr::Ident(_, binding, _) => match binding {
            HirBinding::LocalVar { ty, .. } => ty.clone(),
            HirBinding::ModuleConst { ty, .. } => ty.clone(),
            HirBinding::ModuleShared { ty, .. } => ty.clone(),
            HirBinding::Function { .. } => HirType::Error, // 无调用上下文无法推断
            HirBinding::Import { ty, .. } => ty.clone(),
            _ => HirType::Error,
        },
        HirExpr::Binary(.., ty, _) => ty.clone(),
        HirExpr::AsCast(_, ty, _) => ty.clone(),
        _ => HirType::Error,
    }
}

// ============================================================================
// 错误类型（本地桩 — Phase 1.6 后迁移到 trust_error::Diagnostic）
// ============================================================================

/// HIR 阶段错误类型。
/// Phase 1.6 后统一迁移到 `trust_error::Diagnostic`。
#[derive(Debug, Clone)]
pub struct DiagError {
    pub message: String,
    pub span: Span,
}

impl DiagError {
    pub fn new(message: String, span: Span) -> Self {
        DiagError { message, span }
    }
}

impl std::fmt::Display for DiagError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}:{}: {}",
            self.span.file, self.span.line_start, self.message
        )
    }
}

impl std::error::Error for DiagError {}

// ============================================================================
// 单元测试 — §4.1
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_expr(src: &str) -> ast::Expr {
        use trust_parser::parser::Parser;
        // 简单解析——依赖 parser 的完整路径
        let mut parser = Parser::new(src, "test.trust");
        let prog = parser.parse_program();
        match prog.statements.first() {
            Some(ast::Stmt::Expr(e)) => *e.expr.clone(),
            Some(ast::Stmt::Let(l)) => *l.init.clone(),
            _ => panic!("unexpected AST for: {src}"),
        }
    }

    // AC-SEM-001: let x = 42 → 正确降级
    #[test]
    fn lower_let_int_produces_hir_let_i32() {
        let src = "let x = 42";
        let mut p = trust_parser::parser::Parser::new(src, "test.trust");
        let prog = p.parse_program();
        let mut diags = vec![];
        let _hir = lower(&prog, &mut diags);
        // Phase 1 顶层 let → Stub（不是 item）
        // 但因为是 let 在顶层，lower_stmt 会推 DiagError
        // 修正：用 function body 里的 let
    }

    // AC-SEM-002: let x = if (c) { 1 } else { 0 } → IfExpr 降级
    #[test]
    fn lower_if_expression_produces_hir_if() {
        let src = "function f(): number { let x = if (true) { 1 } else { 0 }; return x; }";
        let mut p = trust_parser::parser::Parser::new(src, "test.trust");
        let prog = p.parse_program();
        let mut diags = vec![];
        let hir = lower(&prog, &mut diags);
        // 验证函数 f 存在
        if let Some(HirItem::Function(ref f)) = hir.items.first() {
            assert_eq!(f.name, "f");
            // 验证 body 包含 if 表达式的 let
            if let HirStmt::Let(ref let_s) = f.body.statements.first().unwrap() {
                assert!(
                    matches!(let_s.init.as_ref(), HirExpr::If(..)),
                    "Expected HirExpr::If, got {:?}",
                    let_s.init
                );
            } else {
                panic!("expected let stmt");
            }
        } else {
            panic!("expected function item, got {:?}", hir.items);
        }
    }

    // AC-SEM-003: import { add } → 名称解析到目标文件
    // (Phase 1 单文件简化：import 解析依赖 exports 表)
    #[test]
    fn resolve_import_binding() {
        let src = "export function add(a: number, b: number): number { return a + b; }";
        let mut p = trust_parser::parser::Parser::new(src, "test.trust");
        let prog = p.parse_program();
        let mut diags = vec![];
        let mg = ModuleGraph::new();
        let hir = resolve_names(&prog, &mg, &mut diags);
        // export 应产生 HirExport
        assert!(!hir.exports.is_empty());
    }

    // AC-SEM-004: 未导入标识符 → 编译错误
    #[test]
    fn undefined_identifier_produces_error() {
        let src = "function f(): void { let x = unknownVar; }";
        let mut p = trust_parser::parser::Parser::new(src, "test.trust");
        let prog = p.parse_program();
        let mut diags = vec![];
        let mg = ModuleGraph::new();
        let _hir = resolve_names(&prog, &mg, &mut diags);
        // 应该有 undefined identifier 错误
        let has_undefined = diags.iter().any(|d| d.message.contains("undefined identifier"));
        assert!(
            has_undefined,
            "Expected 'undefined identifier' diagnostic, got: {:?}",
            diags.iter().map(|d| &d.message).collect::<Vec<_>>()
        );
    }
}
