use ruff_python_ast::{self as ast};
use ruff_python_ast::{Expr, Stmt};

use crate::transform::driver::ExprRewriter;
use crate::{py_expr, py_stmt, template::make_generator, transform::{driver::LoweredExpr}};
use crate::transform::context::{Context, ScopeKind};
use crate::body_transform::{walk_expr, Transformer};
use ruff_python_ast::name::Name;
use ruff_text_size::TextRange;
use std::collections::HashSet;


pub(crate) fn rewrite_lambda(lambda: ast::ExprLambda, ctx: &Context) -> LoweredExpr {
    let func_name = ctx.fresh("lambda");
    let scope = ctx.current_qualname().map(|(mut scope, kind)| {
        if kind == ScopeKind::Function {
            scope.push_str(".<locals>");
        }
        scope
    });

    let ast::ExprLambda {
        parameters, body, ..
    } = lambda;

    let updated_parameters = parameters
        .map(|params| *params)
        .unwrap_or_else(|| ast::Parameters {
            range: TextRange::default(),
            node_index: ast::AtomicNodeIndex::default(),
            posonlyargs: vec![],
            args: vec![],
            vararg: None,
            kwonlyargs: vec![],
            kwarg: None,
        });

    let scope_expr = match scope {
        Some(scope) => py_expr!("{scope:literal}", scope = scope),
        None => py_expr!("None"),
    };
    let mut buf = Vec::new();

    let mut func_def = py_stmt!(
        r#"
def {func_name:id}():
    return {body:expr}
{func_name:id} = __dp__.update_fn({func_name:id}, {qualname:expr}, "<lambda>")
"#,
        func_name = func_name.as_str(),
        body = *body,
        qualname = scope_expr,
    );

    if let Stmt::FunctionDef(ast::StmtFunctionDef {
        ref mut parameters, ..
    }) = &mut func_def[0]
    {
        *parameters = Box::new(updated_parameters);
    }

    buf.extend(func_def);

    LoweredExpr::unmodified_modified(py_expr!("{func:id}", func = func_name.as_str()), buf)
}

pub(crate) fn rewrite_generator(
    generator: ast::ExprGenerator,
    ctx: &Context,
) -> LoweredExpr {

    let needs_async = comprehension_needs_async(&generator.elt, &generator.generators);
    let mut buf = Vec::new();
    let ast::ExprGenerator {
        elt, generators, ..
    } = generator;

    let first_iter_expr = generators
        .first()
        .expect("generator expects at least one comprehension")
        .iter
        .clone();

    let func_name = ctx.fresh("gen");
    let qualname = ctx.make_qualname("<genexpr>");
    let param_name = Name::new(ctx.fresh("iter"));

    let named_targets = collect_named_targets(elt.as_ref(), &generators);
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

    let (global_targets, nonlocal_targets, prelude) = named_target_bindings(ctx, &named_targets);
    if !prelude.is_empty() {
        buf.extend(prelude);
    }

    if !global_targets.is_empty() || !nonlocal_targets.is_empty() {
        let mut bindings = Vec::new();
        for name in global_targets {
            bindings.extend(py_stmt!("global {name:id}", name = name.as_str()));
        }
        for name in nonlocal_targets {
            bindings.extend(py_stmt!("nonlocal {name:id}", name = name.as_str()));
        }
        bindings.extend(body);
        body = bindings;
    }

    let func_def = if needs_async {
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

    buf.extend(func_def);
    let scope = scope_expr(&qualname, "<genexpr>");
    buf.extend(py_stmt!(
        r#"
{func:id} = __dp__.update_fn({func:id}, {scope:expr}, {name:literal})
"#,
        func = func_name.as_str(),
        name = "<genexpr>",
        scope = scope,
    ));

    let expr = if generators
        .first()
        .expect("generator expects at least one comprehension")
        .is_async
    {
        py_expr!(
            "{func:id}({iter:expr})",
            iter = first_iter_expr,
            func = func_name.as_str(),
        )
    } else {
        py_expr!(
            "{func:id}(__dp__.iter({iter:expr}))",
            iter = first_iter_expr,
            func = func_name.as_str(),
        )
    };
    LoweredExpr::unmodified_modified(expr, buf) 
}

fn collect_named_targets(elt: &Expr, generators: &[ast::Comprehension]) -> Vec<String> {
    struct Collector {
        names: HashSet<String>,
    }

    impl Transformer for Collector {
        fn visit_expr(&mut self, expr: &mut Expr) {
            match expr {
                Expr::Named(ast::ExprNamed { target, value, .. }) => {
                    if let Expr::Name(ast::ExprName { id, .. }) = &**target {
                        self.names.insert(id.to_string());
                    }
                    self.visit_expr(value);
                    return;
                }
                Expr::Lambda(_) => return,
                _ => {}
            }
            walk_expr(self, expr);
        }
    }

    let mut collector = Collector {
        names: HashSet::new(),
    };
    let mut expr = Expr::Generator(ast::ExprGenerator {
        node_index: ast::AtomicNodeIndex::default(),
        range: TextRange::default(),
        elt: Box::new(elt.clone()),
        generators: generators.to_vec(),
        parenthesized: false,
    });
    collector.visit_expr(&mut expr);
    let mut names: Vec<String> = collector.names.into_iter().collect();
    names.sort();
    names
}

fn named_target_bindings(
    ctx: &Context,
    names: &[String],
) -> (Vec<String>, Vec<String>, Vec<Stmt>) {
    if names.is_empty() {
        return (Vec::new(), Vec::new(), Vec::new());
    }

    match ctx.current_qualname() {
        Some((_qualname, ScopeKind::Function)) => {
            let mut globals = Vec::new();
            let mut nonlocals = Vec::new();
            let mut prelude = Vec::new();
            for name in names {
                if ctx.is_global_in_current_scope(name) {
                    globals.push(name.clone());
                    continue;
                }
                nonlocals.push(name.clone());
                if !ctx.is_nonlocal_in_current_scope(name) {
                    prelude.extend(py_stmt!(
                        r#"
if False:
    {name:id} = None
"#,
                        name = name.as_str(),
                    ));
                }
            }
            (globals, nonlocals, prelude)
        }
        _ => (names.to_vec(), Vec::new(), Vec::new()),
    }
}

fn scope_expr(qualname: &str, name: &str) -> Expr {
    if qualname == name {
        return py_expr!("None");
    }
    if let Some(scope) = qualname.strip_suffix(name) {
        if let Some(scope) = scope.strip_suffix('.') {
            return py_expr!("{scope:literal}", scope = scope);
        }
    }
    py_expr!("None")
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

fn wrap_comprehension_body(body: Vec<Stmt>, comp: &ast::Comprehension) -> Vec<Stmt> {
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

    if comp.is_async {
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
    }
}




pub fn rewrite(elt: Expr, generators: Vec<ast::Comprehension>, container_type: &str, _append_fn: &str, rewrite: &mut ExprRewriter) -> LoweredExpr {

    let generator = match make_generator(elt, generators) {
        Expr::Generator(generator) => generator,
        _ => unreachable!("expected generator expression"),
    };

    let expr = rewrite_generator(generator, rewrite.context());
    let container_expr = py_expr!("__dp__.{container_type:id}({expr:expr})", expr = expr.expr, container_type = container_type);
    LoweredExpr::unmodified_modified(container_expr, expr.stmts)
}


fn expr_contains_await(expr: &Expr) -> bool {
    // TODO
    false
    // struct AwaitFinder {
    //     found: bool,
    // }

    // impl Transformer for AwaitFinder {
    //     fn visit_expr(&mut self, expr: &mut Expr) {
    //         let has_async_generator = match expr {
    //             Expr::ListComp(ast::ExprListComp { generators, .. })
    //             | Expr::SetComp(ast::ExprSetComp { generators, .. })
    //             | Expr::DictComp(ast::ExprDictComp { generators, .. }) => {
    //                 generators.iter().any(|comp| comp.is_async)
    //             }
    //             _ => false,
    //         };

    //         if has_async_generator {
    //             self.found = true;
    //             return;
    //         }
    //         if matches!(expr, Expr::Await(_)) {
    //             self.found = true;
    //             return;
    //         }
    //         if self.found {
    //             return;
    //         }
    //         walk_expr(&mut self, expr);
    //     }
    // }

    // let mut finder = AwaitFinder { found: false };
    // let mut expr = expr.clone();
    // finder.visit_expr(&mut expr);
    // finder.found
}
