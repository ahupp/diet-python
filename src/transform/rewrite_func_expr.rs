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

    let parameters = parameters
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

    let func_def = py_stmt!(
        "\ndef {func:id}():\n    return {body:expr}",
        func = func_name.as_str(),
        body = *body,
    );

    let func_def = match func_def {
        Stmt::FunctionDef(mut function_def) => {
            function_def.parameters = Box::new(parameters);
            Stmt::FunctionDef(function_def)
        }
        other => other,
    };

    buf.push(func_def);

    py_expr!("\n{func:id}", func = func_name.as_str())
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

    let mut body = vec![py_stmt!("\nyield {value:expr}", value = *elt)];

    for comp in generators.iter().rev() {
        let mut inner = body;
        for if_expr in comp.ifs.iter().rev() {
            inner = vec![py_stmt!(
                "\nif {test:expr}:\n    {body:stmt}",
                test = if_expr.clone(),
                body = inner,
            )];
        }
        body = vec![if comp.is_async {
            py_stmt!(
                "\nasync for {target:expr} in {iter:expr}:\n    {body:stmt}",
                target = comp.target.clone(),
                iter = comp.iter.clone(),
                body = inner,
            )
        } else {
            py_stmt!(
                "\nfor {target:expr} in {iter:expr}:\n    {body:stmt}",
                target = comp.target.clone(),
                iter = comp.iter.clone(),
                body = inner,
            )
        }];
    }

    if let Stmt::For(ast::StmtFor { iter, .. }) = body.first_mut().unwrap() {
        *iter = Box::new(py_expr!("\n{name:id}", name = param_name.as_str()));
    }

    let func_def = py_stmt!(
        "\ndef {func:id}({param:id}):\n    {body:stmt}",
        func = func_name.as_str(),
        param = param_name.as_str(),
        body = body,
    );

    buf.push(func_def);

    py_expr!(
        "\n{func:id}(__dp__.iter({iter:expr}))",
        iter = first_iter_expr,
        func = func_name.as_str(),
    )
}
