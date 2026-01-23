use std::collections::HashSet;

use ruff_python_ast::{self as ast};
use ruff_python_ast::{Expr, Stmt};

use crate::transform::ast_rewrite::LoweredExpr;
use crate::transform::scope::Scope;
use crate::{py_expr, py_stmt};
use crate::transform::context::{Context};
use crate::body_transform::{walk_expr, walk_stmt, Transformer};
use ruff_python_ast::name::Name;
use ruff_text_size::TextRange;


pub(crate) fn rewrite_lambda<'a>(lambda: ast::ExprLambda, ctx: &Context, _scope: &'a Scope<'a>) -> LoweredExpr {
    let func_name = ctx.fresh("lambda");

    let ast::ExprLambda {
        parameters, body, ..
    } = lambda;

    let updated_parameters = parameters
        .map(|params| *params)
        .unwrap_or_default();

    let mut buf = Vec::new();

    // TODO: qualname
    let mut func_def = py_stmt!(
        r#"
def {func_name:id}():
    return {body:expr}
"#,
        func_name = func_name.as_str(),
        body = *body,
    );

    if let Stmt::FunctionDef(ast::StmtFunctionDef {
        ref mut parameters, ..
    }) = &mut func_def[0]
    {
        *parameters = Box::new(updated_parameters);
    }

    buf.extend(func_def);

    LoweredExpr::modified(py_expr!("{func:id}", func = func_name.as_str()), buf)
}

pub(crate) fn rewrite_generator<'a>(
    generator: ast::ExprGenerator,
    ctx: &Context,
    scope: &'a Scope<'a>,
) -> LoweredExpr {

    let needs_async = comprehension_needs_async(&generator.elt, &generator.generators);
    let ast::ExprGenerator {
        elt, generators, ..
    } = generator;

    let named_expr_targets = collect_named_expr_targets(&elt, &generators, true);
    let (global_targets, nonlocal_targets) =
        classify_named_expr_targets(&named_expr_targets, scope);

    let first_iter_expr = generators
        .first()
        .expect("generator expects at least one comprehension")
        .iter
        .clone();

    let func_name = ctx.fresh("gen");

    let param_name = Name::new(ctx.fresh("iter"));

    let mut body = py_stmt!("yield {value:expr}", value = *elt);

    for comp in generators.iter().rev() {
        let mut inner = body;
        for if_expr in comp.ifs.iter().rev() {
            inner = py_stmt!(
                r#"
if {test:expr}:
    {body:stmt}
"#,
                test = if_expr.clone(),
                body = inner,
            )
        }
        body = if comp.is_async {
            py_stmt!(
                r#"
async for {target:expr} in {iter:expr}:
    {body:stmt}
"#,
                target = comp.target.clone(),
                iter = comp.iter.clone(),
                body = inner,
            )
        } else {
            py_stmt!(
                r#"
for {target:expr} in {iter:expr}:
    {body:stmt}
"#,
                target = comp.target.clone(),
                iter = comp.iter.clone(),
                body = inner,
            )
        };
    }

    if let Stmt::For(ast::StmtFor { iter, .. }) = body.first_mut().unwrap() {
        *iter = Box::new(py_expr!("{name:id}", name = param_name.as_str()));
    }

    // TODO: qualname
    let mut func_def = if needs_async {
        py_stmt!(
            r#"
async def {func:id}({param:id}):
    {body:stmt}
"#,
            func = func_name.as_str(),
            param = param_name.as_str(),
            body = body,
        )
    } else {
        py_stmt!(
            r#"
def {func:id}({param:id}):
    {body:stmt}
"#,
            func = func_name.as_str(),
            param = param_name.as_str(),
            body = body,
        )
    };

    if let Stmt::FunctionDef(func_def_stmt) = &mut func_def[0] {
        insert_scope_decls(func_def_stmt, global_targets, nonlocal_targets);
    }

    let expr = py_expr!(
        "{func:id}({iter:expr})",
        iter = first_iter_expr,
        func = func_name.as_str(),
    );
    LoweredExpr::modified(expr, func_def) 
}


fn generators_need_async(generators: &[ast::Comprehension]) -> bool {
    generators.iter().any(|comp| {
        comp.is_async
            || expr_contains_await(&comp.iter)
            || comp.ifs.iter().any(expr_contains_await)
    })
}

pub fn comprehension_needs_async(elt: &Expr, generators: &[ast::Comprehension]) -> bool {
    expr_contains_await(elt) || generators_need_async(generators)
}

pub fn rewrite<'a>(
    context: &Context,
    scope: &'a Scope<'a>,
    elt: Expr,
    generators: Vec<ast::Comprehension>,
    container_type: &str,
    append_fn: &str,
) -> LoweredExpr {
    let mut needs_async = comprehension_needs_async(&elt, &generators);
    let first_iter_expr = generators
        .first()
        .expect("comprehension expects at least one generator")
        .iter
        .clone();

    let func_name = context.fresh("comp");
    let param_name = Name::new(context.fresh("iter"));
    let result_name = context.fresh("result");


    let named_expr_targets = collect_named_expr_targets(&elt, &generators, true);
    let (global_targets, nonlocal_targets) =
        classify_named_expr_targets(&named_expr_targets, scope);

    let mut body = if container_type == "dict" {
        let (key, value) = match elt {
            Expr::Tuple(ast::ExprTuple { mut elts, .. }) if elts.len() == 2 => {
                (elts.remove(0), elts.remove(0))
            }
            _ => unreachable!("dict comprehension expects tuple key/value"),
        };
        py_stmt!(
            "__dp__.setitem({result:id}, {key:expr}, {value:expr})",
            result = result_name.as_str(),
            key = key,
            value = value,
        )
    } else {
        py_stmt!(
            "{result:id}.{append_fn:id}({value:expr})",
            result = result_name.as_str(),
            append_fn = append_fn,
            value = elt,
        )
    };

    for comp in generators.iter().rev() {
        let mut inner = body;
        for if_expr in comp.ifs.iter().rev() {
            inner = py_stmt!(
                r#"
if {test:expr}:
    {body:stmt}
"#,
                test = if_expr.clone(),
                body = inner,
            );
        }
        body = if comp.is_async {
            py_stmt!(
                r#"
async for {target:expr} in {iter:expr}:
    {body:stmt}
"#,
                target = comp.target.clone(),
                iter = comp.iter.clone(),
                body = inner,
            )
        } else {
            py_stmt!(
                r#"
for {target:expr} in {iter:expr}:
    {body:stmt}
"#,
                target = comp.target.clone(),
                iter = comp.iter.clone(),
                body = inner,
            )
        };
    }

    if let Stmt::For(ast::StmtFor { iter, .. }) = body.first_mut().unwrap() {
        *iter = Box::new(py_expr!("{name:id}", name = param_name.as_str()));
    }


    let mut func_def = py_stmt!(
        r#"
def {func:id}({param:id}):
    {result:id} = __dp__.{container_type:id}()
    {body:stmt}
    return {result:id}
"#,
        func = func_name.as_str(),
        param = param_name.as_str(),
        body = body,
        result = result_name.as_str(),
        container_type = container_type,
    );
    if let Stmt::FunctionDef(func_def_stmt) = &mut func_def[0] {
        insert_scope_decls(func_def_stmt, global_targets, nonlocal_targets);
        asyncify_internal_function(func_def_stmt);
        if needs_async {
            func_def_stmt.is_async = true;
        }
        needs_async = func_def_stmt.is_async;
    }

    let call_expr = py_expr!(
        "{func:id}({iter:expr})",
        iter = first_iter_expr,
        func = func_name.as_str(),
    );
    let expr = if needs_async {
        py_expr!("await {value:expr}", value = call_expr)
    } else {
        call_expr
    };

    LoweredExpr::modified(expr, func_def)
}

fn collect_named_expr_targets(
    elt: &Expr,
    generators: &[ast::Comprehension],
    skip_first_iter: bool,
) -> HashSet<String> {
    let mut collector = NamedExprCollector {
        names: HashSet::new(),
    };

    let mut elt_clone = elt.clone();
    collector.visit_expr(&mut elt_clone);

    for (index, comp) in generators.iter().enumerate() {
        for if_expr in &comp.ifs {
            let mut if_clone = if_expr.clone();
            collector.visit_expr(&mut if_clone);
        }
        if !skip_first_iter || index > 0 {
            let mut iter_clone = comp.iter.clone();
            collector.visit_expr(&mut iter_clone);
        }
    }

    collector.names
}

fn classify_named_expr_targets<'a>(
    names: &HashSet<String>,
    scope: &'a Scope<'a>,
) -> (Vec<String>, Vec<String>) {
    if names.is_empty() {
        return (Vec::new(), Vec::new());
    }

    let mut global_targets = Vec::new();
    let mut nonlocal_targets = Vec::new();

    for name in names {
        if scope.is_global(name) {
            global_targets.push(name.clone());
        } else {
            nonlocal_targets.push(name.clone());
        }
    }

    global_targets.sort();
    nonlocal_targets.sort();
    (global_targets, nonlocal_targets)
}

fn insert_scope_decls(
    func_def: &mut ast::StmtFunctionDef,
    global_targets: Vec<String>,
    nonlocal_targets: Vec<String>,
) {
    let mut decls = Vec::new();
    if let Some(stmt) = build_global_stmt(global_targets) {
        decls.push(stmt);
    }
    if let Some(stmt) = build_nonlocal_stmt(nonlocal_targets) {
        decls.push(stmt);
    }

    if decls.is_empty() {
        return;
    }

    let insert_at = match func_def.body.first() {
        Some(Stmt::Expr(ast::StmtExpr { value, .. }))
            if matches!(value.as_ref(), Expr::StringLiteral(_)) => 1,
        _ => 0,
    };
    func_def.body.splice(insert_at..insert_at, decls);
}

fn build_global_stmt(names: Vec<String>) -> Option<Stmt> {
    if names.is_empty() {
        return None;
    }
    let names = names
        .into_iter()
        .map(|name| ast::Identifier::new(name.as_str(), TextRange::default()))
        .collect();
    Some(Stmt::Global(ast::StmtGlobal {
        names,
        range: TextRange::default(),
        node_index: ast::AtomicNodeIndex::default(),
    }))
}

fn build_nonlocal_stmt(names: Vec<String>) -> Option<Stmt> {
    if names.is_empty() {
        return None;
    }
    let names = names
        .into_iter()
        .map(|name| ast::Identifier::new(name.as_str(), TextRange::default()))
        .collect();
    Some(Stmt::Nonlocal(ast::StmtNonlocal {
        names,
        range: TextRange::default(),
        node_index: ast::AtomicNodeIndex::default(),
    }))
}

struct NamedExprCollector {
    names: HashSet<String>,
}

impl Transformer for NamedExprCollector {
    fn visit_expr(&mut self, expr: &mut Expr) {
        match expr {
            Expr::Named(ast::ExprNamed { target, value, .. }) => {
                if let Expr::Name(ast::ExprName { id, .. }) = target.as_ref() {
                    self.names.insert(id.as_str().to_string());
                } else {
                    self.visit_expr(target);
                }
                self.visit_expr(value);
                return;
            }
            Expr::Lambda(_)
            | Expr::Generator(_)
            | Expr::ListComp(_)
            | Expr::SetComp(_)
            | Expr::DictComp(_) => {
                return;
            }
            _ => {}
        }

        walk_expr(self, expr);
    }
}

fn asyncify_internal_function(func_def: &mut ast::StmtFunctionDef) -> bool {
    if func_def.is_async || !func_def.name.id.as_str().starts_with("_dp_") {
        return func_def.is_async;
    }

    struct AwaitFinder {
        found: bool,
    }

    impl Transformer for AwaitFinder {
        fn visit_stmt(&mut self, stmt: &mut Stmt) {
            if self.found {
                return;
            }
            match stmt {
                Stmt::FunctionDef(_) | Stmt::ClassDef(_) => return,
                Stmt::For(ast::StmtFor { is_async, .. }) => {
                    if *is_async {
                        self.found = true;
                        return;
                    }
                }
                _ => {}
            }
            walk_stmt(self, stmt);
        }

        fn visit_expr(&mut self, expr: &mut Expr) {
            if self.found {
                return;
            }
            if matches!(expr, Expr::Await(_)) {
                self.found = true;
                return;
            }
            if matches!(expr, Expr::Lambda(_)) {
                return;
            }
            walk_expr(self, expr);
        }
    }

    let mut finder = AwaitFinder { found: false };
    for stmt in &mut func_def.body {
        finder.visit_stmt(stmt);
        if finder.found {
            break;
        }
    }

    if finder.found {
        func_def.is_async = true;
    }
    func_def.is_async
}

fn expr_contains_await(expr: &Expr) -> bool {
    struct AwaitFinder {
        found: bool,
    }

    impl Transformer for AwaitFinder {
        fn visit_expr(&mut self, expr: &mut Expr) {
            if self.found {
                return;
            }

            let has_async_generator = match expr {
                Expr::ListComp(ast::ExprListComp { generators, .. })
                | Expr::SetComp(ast::ExprSetComp { generators, .. })
                | Expr::DictComp(ast::ExprDictComp { generators, .. }) => {
                    generators.iter().any(|comp| comp.is_async)
                }
                _ => false,
            };

            if matches!(expr, Expr::Generator(_)) {
                return;
            }

            if has_async_generator || matches!(expr, Expr::Await(_)) {
                self.found = true;
                return;
            }

            walk_expr(self, expr);
        }
    }

    let mut finder = AwaitFinder { found: false };
    let mut expr = expr.clone();
    finder.visit_expr(&mut expr);
    finder.found
}
