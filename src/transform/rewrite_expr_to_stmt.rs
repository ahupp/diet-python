use std::cell::RefCell;

use super::context::Context;
use crate::template::single_stmt;
use crate::{py_expr, py_stmt};
use ruff_python_ast::name::Name;
use ruff_python_ast::visitor::transformer::{walk_expr, walk_stmt, Transformer};
use ruff_python_ast::{self as ast, Expr, Stmt};
use ruff_text_size::TextRange;

#[derive(Debug)]
pub(crate) enum Modified {
    Yes(Stmt),
    No(Stmt),
}

pub(crate) fn expr_to_stmt(ctx: &Context, stmt: Stmt) -> Modified {
    match stmt {
        Stmt::Assign(assign) => rewrite_assign(ctx, assign),
        Stmt::Expr(expr) => rewrite_expr_stmt(ctx, expr),
        other => rewrite_with_functions(ctx, other),
    }
}

struct LambdaGeneratorLowerer<'ctx> {
    ctx: &'ctx Context,
    functions: RefCell<Vec<Stmt>>,
}

impl<'ctx> LambdaGeneratorLowerer<'ctx> {
    fn new(ctx: &'ctx Context) -> Self {
        Self {
            ctx,
            functions: RefCell::new(Vec::new()),
        }
    }

    fn into_statements(self) -> Vec<Stmt> {
        self.functions.into_inner()
    }

    fn lower_lambda(&self, expr: &mut Expr, lambda: ast::ExprLambda) {
        let func_name = self.ctx.fresh("lambda");

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

        let mut func_def = py_stmt!(
            "\ndef {func:id}():\n    return {body:expr}",
            func = func_name.as_str(),
            body = *body,
        );

        if let Stmt::FunctionDef(ast::StmtFunctionDef {
            parameters: params, ..
        }) = &mut func_def
        {
            *params = Box::new(parameters);
        }

        self.visit_stmt(&mut func_def);
        self.functions.borrow_mut().push(func_def);

        *expr = py_expr!("\n{func:id}", func = func_name.as_str());
    }

    fn lower_generator(&self, expr: &mut Expr, generator: ast::ExprGenerator) {
        let ast::ExprGenerator {
            elt, generators, ..
        } = generator;

        let first_iter_expr = generators
            .first()
            .expect("generator expects at least one comprehension")
            .iter
            .clone();

        let func_name = self.ctx.fresh("gen");

        let param_name = if let Expr::Name(ast::ExprName { id, .. }) = &first_iter_expr {
            id.clone()
        } else {
            Name::new(self.ctx.fresh("iter"))
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

        let mut func_def = py_stmt!(
            "\ndef {func:id}({param:id}):\n    {body:stmt}",
            func = func_name.as_str(),
            param = param_name.as_str(),
            body = body,
        );

        self.visit_stmt(&mut func_def);
        self.functions.borrow_mut().push(func_def);

        *expr = py_expr!(
            "\n{func:id}(__dp__.iter({iter:expr}))",
            iter = first_iter_expr,
            func = func_name.as_str(),
        );
    }
}

impl<'ctx> Transformer for LambdaGeneratorLowerer<'ctx> {
    fn visit_expr(&self, expr: &mut Expr) {
        match expr.clone() {
            Expr::Lambda(lambda) => {
                self.lower_lambda(expr, lambda);
                return;
            }
            Expr::Generator(generator) => {
                self.lower_generator(expr, generator);
                return;
            }
            _ => {}
        }

        walk_expr(self, expr);
    }

    fn visit_stmt(&self, stmt: &mut Stmt) {
        walk_stmt(self, stmt);
    }
}

fn expr_boolop_to_stmts(target: &str, bool_op: ast::ExprBoolOp) -> Vec<Stmt> {
    let ast::ExprBoolOp { op, values, .. } = bool_op;

    let mut values = values.into_iter();
    let first = values.next().expect("bool op expects at least one value");
    let mut stmts = match first {
        Expr::BoolOp(bool_op) => expr_boolop_to_stmts(target, bool_op),
        other => vec![assign_to_target(target, other)],
    };

    for value in values {
        let body_stmt = match value {
            Expr::BoolOp(bool_op) => single_stmt(expr_boolop_to_stmts(target, bool_op)),
            other => assign_to_target(target, other),
        };
        let test_expr = match op {
            ast::BoolOp::And => target_expr(target),
            ast::BoolOp::Or => py_expr!("\nnot {target:expr}", target = target_expr(target),),
        };
        let stmt = py_stmt!(
            "\nif {test:expr}:\n    {body:stmt}",
            test = test_expr,
            body = body_stmt,
        );
        stmts.push(stmt);
    }

    stmts
}

fn assign_to_target(target: &str, value: Expr) -> Stmt {
    py_stmt!(
        "\n{target:id} = {value:expr}",
        target = target,
        value = value,
    )
}

fn target_expr(target: &str) -> Expr {
    py_expr!("\n{target:id}", target = target,)
}

fn rewrite_assign(ctx: &Context, mut assign: ast::StmtAssign) -> Modified {
    let value_expr = *assign.value;

    if assign.targets.len() == 1 {
        if let Some(Expr::Name(ast::ExprName { id, .. })) = assign.targets.first() {
            if let Expr::BoolOp(bool_op) = &value_expr {
                let target_name = id.to_string();
                let bool_op = bool_op.clone();
                let new_stmt = single_stmt(expr_boolop_to_stmts(&target_name, bool_op));
                return Modified::Yes(new_stmt);
            }
        }
    }

    let mut value_expr = value_expr;
    let mut lowered_functions = match value_expr.clone() {
        Expr::Lambda(lambda) => {
            let lowerer = LambdaGeneratorLowerer::new(ctx);
            lowerer.lower_lambda(&mut value_expr, lambda);
            lowerer.into_statements()
        }
        Expr::Generator(generator) => {
            let lowerer = LambdaGeneratorLowerer::new(ctx);
            lowerer.lower_generator(&mut value_expr, generator);
            lowerer.into_statements()
        }
        _ => Vec::new(),
    };

    assign.value = Box::new(value_expr);

    if !lowered_functions.is_empty() {
        lowered_functions.push(Stmt::Assign(assign));
        Modified::Yes(single_stmt(lowered_functions))
    } else {
        rewrite_with_functions(ctx, Stmt::Assign(assign))
    }
}

fn rewrite_expr_stmt(ctx: &Context, mut expr_stmt: ast::StmtExpr) -> Modified {
    let value_expr = *expr_stmt.value;

    if let Expr::BoolOp(bool_op) = &value_expr {
        let new_stmt = single_stmt(expr_boolop_to_stmts("_", bool_op.clone()));
        return Modified::Yes(new_stmt);
    }

    let mut value_expr = value_expr;
    let mut lowered_functions = match value_expr.clone() {
        Expr::Lambda(lambda) => {
            let lowerer = LambdaGeneratorLowerer::new(ctx);
            lowerer.lower_lambda(&mut value_expr, lambda);
            lowerer.into_statements()
        }
        Expr::Generator(generator) => {
            let lowerer = LambdaGeneratorLowerer::new(ctx);
            lowerer.lower_generator(&mut value_expr, generator);
            lowerer.into_statements()
        }
        _ => Vec::new(),
    };

    expr_stmt.value = Box::new(value_expr);

    if !lowered_functions.is_empty() {
        lowered_functions.push(Stmt::Expr(expr_stmt));
        Modified::Yes(single_stmt(lowered_functions))
    } else {
        rewrite_with_functions(ctx, Stmt::Expr(expr_stmt))
    }
}

fn rewrite_with_functions(ctx: &Context, mut stmt: Stmt) -> Modified {
    let functions = lower_lambdas_generators(ctx, &mut stmt);
    if functions.is_empty() {
        Modified::No(stmt)
    } else {
        let mut stmts = functions;
        stmts.push(stmt);
        Modified::Yes(single_stmt(stmts))
    }
}

fn lower_lambdas_generators(ctx: &Context, stmt: &mut Stmt) -> Vec<Stmt> {
    let lowerer = LambdaGeneratorLowerer::new(ctx);
    lowerer.visit_stmt(stmt);
    lowerer.into_statements()
}

#[cfg(test)]
mod tests {
    use crate::test_util::assert_transform_eq;

    #[test]
    fn rewrites_bool_and_assignment() {
        let input = "x = a and b";
        let expected = r#"
x = a
if x:
    x = b
"#;

        assert_transform_eq(input, expected);
    }

    #[test]
    fn skips_non_bool_assignment() {
        let input = "x = value";
        let expected = "x = value";

        assert_transform_eq(input, expected);
    }

    #[test]
    fn rewrites_bool_expr_statement() {
        let input = "a and b";
        let expected = r#"
_ = a
if _:
    _ = b
"#;

        assert_transform_eq(input, expected);
    }

    #[test]
    fn rewrites_lambda_assignment() {
        let input = "x = lambda: 1";
        let expected = r#"
def _dp_lambda_1():
    return 1
x = _dp_lambda_1
"#;

        assert_transform_eq(input, expected);
    }

    #[test]
    fn rewrites_lambda_in_return_stmt() {
        let input = "return lambda: 1";
        let expected = r#"
def _dp_lambda_1():
    return 1
return _dp_lambda_1
"#;

        assert_transform_eq(input, expected);
    }

    #[test]
    fn rewrites_generator_assignment() {
        let input = "x = (i for i in items)";
        let expected = r#"
def _dp_gen_1(items):
    _dp_iter_2 = __dp__.iter(items)
    while True:
        try:
            i = __dp__.next(_dp_iter_2)
        except:
            _dp_exc_3 = __dp__.current_exception()
            if __dp__.isinstance(_dp_exc_3, StopIteration):
                break
            else:
                raise
        yield i
x = _dp_gen_1(__dp__.iter(items))
"#;

        assert_transform_eq(input, expected);
    }
}
