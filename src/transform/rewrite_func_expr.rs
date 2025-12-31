use super::context::Context;
use crate::{py_expr, py_stmt};
use ruff_python_ast::name::Name;
use ruff_python_ast::{self as ast, Expr, Stmt};
use ruff_text_size::TextRange;

pub(crate) fn rewrite_lambda(lambda: ast::ExprLambda, ctx: &Context, buf: &mut Vec<Stmt>) -> Expr {
    let func_name = ctx.fresh("lambda");
    let qualname = ctx.make_qualname("<lambda>");

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
    buf.extend(py_stmt!(
        "{func:id}.__name__ = {name:literal}",
        func = func_name.as_str(),
        name = "<lambda>",
    ));
    buf.extend(py_stmt!(
        "{func:id}.__qualname__ = {qualname:literal}",
        func = func_name.as_str(),
        qualname = qualname,
    ));

    py_expr!("{func:id}", func = func_name.as_str())
}

pub(crate) fn rewrite_generator(
    generator: ast::ExprGenerator,
    ctx: &Context,
    needs_async: bool,
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
    let qualname = ctx.make_qualname("<genexpr>");
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
        *iter = Box::new(py_expr!("\n{name:id}", name = param_name.as_str()));
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
    buf.extend(py_stmt!(
        r#"
{func:id}.__name__ = {name:literal}
{func:id}.__qualname__ = {qualname:literal}
{func:id}.__code__ = {func:id}.__code__.replace(co_name={name:literal}, co_qualname={qualname:literal})
"#,
        func = func_name.as_str(),
        name = "<genexpr>",
        qualname = qualname,
    ));

    if generators
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
    }
}
