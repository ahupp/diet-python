use std::cell::RefCell;

use super::context::Context;
use crate::body_transform::{walk_expr, walk_stmt, Transformer};
use crate::template::{make_binop, make_unaryop, single_stmt};
use crate::{py_expr, py_stmt};
use ruff_python_ast::name::Name;
use ruff_python_ast::{self as ast, CmpOp, Expr, Stmt};
use ruff_text_size::TextRange;

pub(crate) struct LambdaGeneratorLowerer<'ctx> {
    ctx: &'ctx Context,
    functions: RefCell<Vec<Stmt>>,
}

impl<'ctx> LambdaGeneratorLowerer<'ctx> {
    pub(crate) fn new(ctx: &'ctx Context) -> Self {
        Self {
            ctx,
            functions: RefCell::new(Vec::new()),
        }
    }

    pub(crate) fn into_statements(self) -> Vec<Stmt> {
        self.functions.into_inner()
    }

    pub(crate) fn rewrite(&self, stmt: &mut Stmt) {
        walk_stmt(self, stmt);
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

        let func_def = py_stmt!(
            "\ndef {func:id}():\n    return {body:expr}",
            func = func_name.as_str(),
            body = *body,
        );

        self.functions.borrow_mut().push(match func_def {
            Stmt::FunctionDef(mut function_def) => {
                function_def.parameters = Box::new(parameters);
                Stmt::FunctionDef(function_def)
            }
            other => other,
        });

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

        let func_def = py_stmt!(
            "\ndef {func:id}({param:id}):\n    {body:stmt}",
            func = func_name.as_str(),
            param = param_name.as_str(),
            body = body,
        );

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

    fn visit_stmt(&self, _: &mut Stmt) {
        // Only visit expressions referenced directly by this statement; callers
        // are responsible for rewriting any nested statements themselves.
    }
}

pub(crate) fn expr_boolop_to_stmts(target: &str, bool_op: ast::ExprBoolOp) -> Vec<Stmt> {
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

pub(crate) fn expr_compare_to_stmts(target: &str, compare: ast::ExprCompare) -> Vec<Stmt> {
    let ast::ExprCompare {
        left,
        ops,
        comparators,
        ..
    } = compare;

    let mut ops = ops.into_vec().into_iter();
    let mut comparators = comparators.into_vec().into_iter();

    let first_op = ops
        .next()
        .expect("compare expects at least one comparison operator");
    let first_comparator = comparators
        .next()
        .expect("compare expects at least one comparator");

    let mut stmts = vec![assign_to_target(
        target,
        compare_expr(first_op, *left, first_comparator.clone()),
    )];

    let mut current_left = first_comparator;

    for (op, comparator) in ops.zip(comparators) {
        let body_stmt = assign_to_target(
            target,
            compare_expr(op, current_left.clone(), comparator.clone()),
        );
        let stmt = py_stmt!(
            "\nif {test:expr}:\n    {body:stmt}",
            test = target_expr(target),
            body = body_stmt,
        );
        stmts.push(stmt);
        current_left = comparator;
    }

    stmts
}

fn compare_expr(op: CmpOp, left: Expr, right: Expr) -> Expr {
    match op {
        CmpOp::Eq => make_binop("eq", left, right),
        CmpOp::NotEq => make_binop("ne", left, right),
        CmpOp::Lt => make_binop("lt", left, right),
        CmpOp::LtE => make_binop("le", left, right),
        CmpOp::Gt => make_binop("gt", left, right),
        CmpOp::GtE => make_binop("ge", left, right),
        CmpOp::Is => make_binop("is_", left, right),
        CmpOp::IsNot => make_binop("is_not", left, right),
        CmpOp::In => make_binop("contains", right, left),
        CmpOp::NotIn => make_unaryop("not_", make_binop("contains", right, left)),
    }
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

#[cfg(test)]
mod tests {
    crate::transform_fixture_test!("tests_rewrite_expr_to_stmt.txt");
}
