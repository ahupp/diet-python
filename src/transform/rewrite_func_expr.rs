use super::context::Context;
use crate::{py_expr, py_stmt};
use ruff_python_ast::name::Name;
use ruff_python_ast::{self as ast, Expr, Stmt};
use ruff_text_size::TextRange;

pub(crate) fn rewrite_lambda(lambda: ast::ExprLambda, ctx: &Context, buf: &mut Vec<Stmt>) -> Expr {
    let func_name = ctx.fresh("lambda");

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

    let mut func_def = py_stmt!(
        r#"
def {func_name:id}():
    return {body:expr}
"#,
        func_name = func_name.as_str(),
        body = *body
    );

    if let Stmt::FunctionDef(ast::StmtFunctionDef {
        ref mut parameters, ..
    }) = &mut func_def[0]
    {
        *parameters = Box::new(updated_parameters);
    }

    buf.extend(func_def);

    py_expr!("{func:id}", func = func_name.as_str())
}

pub(crate) fn rewrite_generator(
    generator: ast::ExprGenerator,
    ctx: &Context,
    buf: &mut Vec<Stmt>,
) -> Expr {
    let ast::ExprGenerator {
        elt, generators, ..
    } = generator;

    let first_iter_expr = generators
        .first()
        .expect("generator expects at least one comprehension")
        .iter
        .clone();

    let func_name = ctx.fresh("gen");

    let param_name = if let Expr::Name(ast::ExprName { id, .. }) = &first_iter_expr {
        id.clone()
    } else {
        Name::new(ctx.fresh("iter"))
    };

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
        *iter = Box::new(py_expr!("\n{name:id}", name = param_name.as_str()));
    }

    let func_def = py_stmt!(
        r#"
def {func:id}({param:id}):
    {body:stmt}
"#,
        func = func_name.as_str(),
        param = param_name.as_str(),
        body = body,
    );

    buf.extend(func_def);

    py_expr!(
        "{func:id}(__dp__.iter({iter:expr}))",
        iter = first_iter_expr,
        func = func_name.as_str(),
    )
}
