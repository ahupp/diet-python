use super::{
    context::Context,
    rewrite_assert, rewrite_assign_del, rewrite_class_def, rewrite_decorator,
    rewrite_expr_to_stmt::{expr_boolop_to_stmts, expr_compare_to_stmts, expr_yield_from_to_stmt},
    rewrite_for_loop, rewrite_import, rewrite_match_case, rewrite_string, rewrite_try_except,
    rewrite_with, ImportStarHandling, Options,
};
use crate::body_transform::{walk_expr, walk_stmt, Transformer};
use crate::template::{make_binop, make_generator, make_tuple, make_unaryop};
use crate::{py_expr, py_stmt};
use ruff_python_ast::name::Name;
use ruff_python_ast::{self as ast, Expr, Operator, Stmt, UnaryOp};
use ruff_text_size::TextRange;
use std::mem::take;

pub struct ExprRewriter<'a> {
    ctx: &'a Context,
    options: Options,
    buf: Vec<Stmt>,
}

enum Rewrite {
    Walk(Vec<Stmt>),
    Visit(Vec<Stmt>),
}

impl<'a> ExprRewriter<'a> {
    pub fn new(ctx: &'a Context) -> Self {
        Self {
            options: ctx.options,
            ctx,
            buf: Vec::new(),
        }
    }

    fn should_rewrite_import_from(import_from: &ast::StmtImportFrom, options: &Options) -> bool {
        if import_from
            .names
            .iter()
            .any(|alias| alias.name.id.as_str() == "*")
        {
            !matches!(options.import_star_handling, ImportStarHandling::Allowed)
        } else {
            true
        }
    }

    fn process_statements(&mut self, initial: Vec<Stmt>) -> Vec<Stmt> {
        enum WorkItem {
            Process(Stmt),
            Walk(Stmt),
            Emit(Stmt),
        }

        let mut worklist: Vec<WorkItem> =
            initial.into_iter().rev().map(WorkItem::Process).collect();

        let mut buf_stack = take(&mut self.buf);
        let mut output = Vec::new();

        while let Some(item) = worklist.pop() {
            match item {
                WorkItem::Process(stmt) => match self.rewrite_stmt(stmt) {
                    Rewrite::Visit(stmts) => {
                        for stmt in stmts.into_iter().rev() {
                            worklist.push(WorkItem::Process(stmt));
                        }
                    }
                    Rewrite::Walk(stmts) => {
                        for stmt in stmts.into_iter().rev() {
                            worklist.push(WorkItem::Walk(stmt));
                        }
                    }
                },
                WorkItem::Walk(mut stmt) => {
                    walk_stmt(self, &mut stmt);
                    let mut buffered = take(&mut self.buf);
                    worklist.push(WorkItem::Emit(stmt));
                    while let Some(buffered_stmt) = buffered.pop() {
                        worklist.push(WorkItem::Process(buffered_stmt));
                    }
                }
                WorkItem::Emit(stmt) => output.push(stmt),
            }
        }

        self.buf = take(&mut buf_stack);

        output
    }

    pub(super) fn maybe_placeholder(&mut self, mut expr: Expr) -> Expr {
        if matches!(expr, Expr::Name(_)) {
            return expr;
        }

        self.visit_expr(&mut expr);

        if matches!(expr, Expr::Name(_)) {
            return expr;
        }

        let tmp = self.ctx.fresh("tmp");
        let placeholder_expr = py_expr!("{tmp:id}", tmp = tmp.as_str());
        let assign = py_stmt!(
            "\n{tmp:id} = {value:expr}",
            tmp = tmp.as_str(),
            value = expr,
        );
        self.buf.push(assign);
        placeholder_expr
    }

    fn rewrite_stmt(&mut self, stmt: Stmt) -> Rewrite {
        match stmt {
            Stmt::FunctionDef(mut func_def) if !func_def.decorator_list.is_empty() => {
                let decorators = take(&mut func_def.decorator_list);
                let func_name = func_def.name.id.clone();
                Rewrite::Visit(vec![rewrite_decorator::rewrite(
                    decorators,
                    func_name.as_str(),
                    Stmt::FunctionDef(func_def),
                    None,
                    self.ctx,
                )])
            }
            Stmt::With(with) => Rewrite::Visit(vec![rewrite_with::rewrite(with, self.ctx, self)]),
            Stmt::For(for_stmt) => {
                Rewrite::Visit(vec![rewrite_for_loop::rewrite(for_stmt, self.ctx, self)])
            }
            Stmt::Assert(assert) => Rewrite::Visit(vec![rewrite_assert::rewrite(assert)]),
            Stmt::ClassDef(class_def) => {
                let decorated = !class_def.decorator_list.is_empty();
                let base_stmt = rewrite_class_def::rewrite(class_def.clone(), decorated);
                if decorated {
                    let decorators = class_def.decorator_list.clone();
                    let class_name = class_def.name.id.clone();
                    let base_name = format!("_dp_class_{}", class_name);
                    Rewrite::Visit(vec![rewrite_decorator::rewrite(
                        decorators,
                        class_name.as_str(),
                        base_stmt,
                        Some(base_name.as_str()),
                        self.ctx,
                    )])
                } else {
                    Rewrite::Visit(vec![base_stmt])
                }
            }
            Stmt::Try(try_stmt) if rewrite_try_except::has_non_default_handler(&try_stmt) => {
                Rewrite::Visit(vec![rewrite_try_except::rewrite(try_stmt, self.ctx)])
            }
            Stmt::Match(match_stmt) => {
                Rewrite::Visit(vec![rewrite_match_case::rewrite(match_stmt, self.ctx)])
            }
            Stmt::Import(import) => Rewrite::Visit(vec![rewrite_import::rewrite(import)]),
            Stmt::ImportFrom(import_from)
                if Self::should_rewrite_import_from(&import_from, &self.options) =>
            {
                let stmt = rewrite_import::rewrite_from(import_from.clone(), &self.options);
                Rewrite::Visit(vec![stmt])
            }
            Stmt::ImportFrom(import_from) => Rewrite::Walk(vec![Stmt::ImportFrom(import_from)]),
            Stmt::AnnAssign(ann_assign) => {
                match rewrite_assign_del::rewrite_ann_assign(self, &ann_assign) {
                    Some(stmts) => Rewrite::Visit(stmts),
                    None => Rewrite::Walk(Vec::new()),
                }
            }
            Stmt::Assign(assign) => match rewrite_assign_del::rewrite_assign(self, &assign) {
                Some(stmts) => Rewrite::Visit(stmts),
                None => Rewrite::Walk(vec![Stmt::Assign(assign)]),
            },
            Stmt::AugAssign(aug) => {
                Rewrite::Visit(rewrite_assign_del::rewrite_aug_assign(self, &aug))
            }
            Stmt::Delete(del) => match rewrite_assign_del::rewrite_delete(self, &del) {
                Some(stmts) => Rewrite::Visit(stmts),
                None => Rewrite::Walk(vec![Stmt::Delete(del)]),
            },
            Stmt::Raise(mut raise) if raise.cause.is_some() => {
                match (raise.exc.take(), raise.cause.take()) {
                    (Some(exc), Some(cause)) => Rewrite::Visit(vec![py_stmt!(
                        "raise __dp__.raise_from({exc:expr}, {cause:expr})",
                        exc = *exc,
                        cause = *cause,
                    )]),
                    _ => panic!("raise with a cause but without an exception should be impossible"),
                }
            }
            other => Rewrite::Walk(vec![other]),
        }
    }

    fn lower_lambda(&mut self, lambda: ast::ExprLambda) -> Expr {
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

        let func_def = match func_def {
            Stmt::FunctionDef(mut function_def) => {
                function_def.parameters = Box::new(parameters);
                Stmt::FunctionDef(function_def)
            }
            other => other,
        };

        self.buf.push(func_def);

        py_expr!("\n{func:id}", func = func_name.as_str())
    }

    fn lower_generator(&mut self, generator: ast::ExprGenerator) -> Expr {
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

        self.buf.push(func_def);

        py_expr!(
            "\n{func:id}(__dp__.iter({iter:expr}))",
            iter = first_iter_expr,
            func = func_name.as_str(),
        )
    }
}

fn make_tuple_splat(tuple: ast::ExprTuple) -> Expr {
    if !tuple.elts.iter().any(|elt| matches!(elt, Expr::Starred(_))) {
        return Expr::Tuple(tuple);
    }

    let ast::ExprTuple { elts, .. } = tuple;

    let mut segments: Vec<Expr> = Vec::new();
    let mut values: Vec<Expr> = Vec::new();

    for elt in elts {
        match elt {
            Expr::Starred(ast::ExprStarred { value, .. }) => {
                if !values.is_empty() {
                    segments.push(make_tuple(std::mem::take(&mut values)));
                }
                segments.push(py_expr!("tuple({value:expr})", value = *value));
            }
            other => values.push(other),
        }
    }

    if !values.is_empty() {
        segments.push(make_tuple(values));
    }

    let mut parts = segments.into_iter();
    let mut expr = match parts.next() {
        Some(expr) => expr,
        None => return make_tuple(Vec::new()),
    };

    for part in parts {
        expr = py_expr!("{left:expr} + {right:expr}", left = expr, right = part);
    }

    expr
}

impl<'a> Transformer for ExprRewriter<'a> {
    fn visit_body(&mut self, body: &mut Vec<Stmt>) {
        let stmts = take(body);
        let output = self.process_statements(stmts);
        *body = output;
    }

    fn visit_expr(&mut self, expr: &mut Expr) {
        let rewritten = match expr.clone() {
            Expr::Named(named_expr) => {
                let ast::ExprNamed { target, value, .. } = named_expr;
                let target = *target;
                let value = *value;
                let value_expr = self.maybe_placeholder(value);
                let assign_target = py_stmt!(
                    "\n{target:expr} = {value:expr}\n",
                    target = target,
                    value = value_expr.clone(),
                );
                self.buf.push(assign_target);
                value_expr
            }
            Expr::If(if_expr) => {
                let tmp = self.ctx.fresh("tmp");
                let ast::ExprIf {
                    test, body, orelse, ..
                } = if_expr;
                let assign = py_stmt!(
                    "\nif {cond:expr}:\n    {tmp:id} = {body:expr}\nelse:\n    {tmp:id} = {orelse:expr}",
                    cond = *test,
                    tmp = tmp.as_str(),
                    body = *body,
                    orelse = *orelse,
                );
                self.buf.push(assign);
                py_expr!("{tmp:id}", tmp = tmp.as_str())
            }
            Expr::BoolOp(bool_op) => {
                let tmp = self.ctx.fresh("tmp");
                let stmts = expr_boolop_to_stmts(tmp.as_str(), bool_op);
                self.buf.extend(stmts);
                py_expr!("{tmp:id}", tmp = tmp.as_str())
            }
            Expr::Compare(compare) => {
                let tmp = self.ctx.fresh("tmp");
                let stmts = expr_compare_to_stmts(tmp.as_str(), compare);
                self.buf.extend(stmts);
                py_expr!("{tmp:id}", tmp = tmp.as_str())
            }
            Expr::YieldFrom(yield_from) => {
                let tmp = self.ctx.fresh("tmp");
                let stmts = expr_yield_from_to_stmt(self.ctx, tmp.as_str(), yield_from);
                self.buf.extend(stmts);
                py_expr!("{tmp:id}", tmp = tmp.as_str())
            }
            Expr::Lambda(lambda) => self.lower_lambda(lambda),
            Expr::Generator(generator) => self.lower_generator(generator),
            Expr::FString(f_string) => rewrite_string::rewrite_fstring(f_string),
            Expr::TString(t_string) => rewrite_string::rewrite_tstring(t_string),
            Expr::Slice(ast::ExprSlice {
                lower, upper, step, ..
            }) => {
                fn none_name() -> Expr {
                    py_expr!("None")
                }
                let lower_expr = lower.map(|expr| *expr).unwrap_or_else(none_name);
                let upper_expr = upper.map(|expr| *expr).unwrap_or_else(none_name);
                let step_expr = step.map(|expr| *expr).unwrap_or_else(none_name);
                py_expr!(
                    "slice({lower:expr}, {upper:expr}, {step:expr})",
                    lower = lower_expr,
                    upper = upper_expr,
                    step = step_expr,
                )
            }
            Expr::EllipsisLiteral(_) => py_expr!("Ellipsis"),
            Expr::NumberLiteral(ast::ExprNumberLiteral {
                value: ast::Number::Complex { real, imag },
                ..
            }) => {
                let real_expr = Expr::NumberLiteral(ast::ExprNumberLiteral {
                    node_index: ast::AtomicNodeIndex::default(),
                    range: TextRange::default(),
                    value: ast::Number::Float(real),
                });
                let imag_expr = Expr::NumberLiteral(ast::ExprNumberLiteral {
                    node_index: ast::AtomicNodeIndex::default(),
                    range: TextRange::default(),
                    value: ast::Number::Float(imag),
                });
                py_expr!(
                    "complex({real:expr}, {imag:expr})",
                    real = real_expr,
                    imag = imag_expr,
                )
            }
            Expr::Attribute(ast::ExprAttribute {
                value, attr, ctx, ..
            }) if matches!(ctx, ast::ExprContext::Load) && self.options.lower_attributes => {
                let value_expr = *value;
                py_expr!(
                    "getattr({value:expr}, {attr:literal})",
                    value = value_expr,
                    attr = attr.id.as_str(),
                )
            }
            Expr::Tuple(tuple)
                if matches!(tuple.ctx, ast::ExprContext::Load)
                    && tuple.elts.iter().any(|elt| matches!(elt, Expr::Starred(_))) =>
            {
                make_tuple_splat(tuple)
            }
            Expr::ListComp(ast::ExprListComp {
                elt, generators, ..
            }) => py_expr!("list({expr:expr})", expr = make_generator(*elt, generators)),
            Expr::SetComp(ast::ExprSetComp {
                elt, generators, ..
            }) => py_expr!("set({expr:expr})", expr = make_generator(*elt, generators)),
            Expr::DictComp(ast::ExprDictComp {
                key,
                value,
                generators,
                ..
            }) => {
                let tuple = py_expr!("({key:expr}, {value:expr})", key = *key, value = *value,);
                py_expr!(
                    "dict({expr:expr})",
                    expr = make_generator(tuple, generators)
                )
            }
            Expr::List(list) if matches!(list.ctx, ast::ExprContext::Load) => {
                let tuple = make_tuple_splat(ast::ExprTuple {
                    node_index: ast::AtomicNodeIndex::default(),
                    range: TextRange::default(),
                    elts: list.elts,
                    ctx: ast::ExprContext::Load,
                    parenthesized: false,
                });
                py_expr!("list({tuple:expr})", tuple = tuple,)
            }
            Expr::Set(ast::ExprSet { elts, .. }) => {
                let tuple = make_tuple(elts);
                py_expr!("set({tuple:expr})", tuple = tuple,)
            }
            Expr::Dict(ast::ExprDict { items, .. }) => {
                let mut iter = items.into_iter().peekable();
                let mut segments: Vec<Expr> = Vec::new();

                loop {
                    let mut keyed_pairs = Vec::new();
                    while matches!(iter.peek(), Some(ast::DictItem { key: Some(_), .. })) {
                        let item = iter.next().expect("peeked item should exist");
                        let key = item.key.expect("peek guaranteed key");
                        let value = item.value;
                        keyed_pairs.push(py_expr!(
                            "({key:expr}, {value:expr})",
                            key = key,
                            value = value,
                        ));
                    }

                    if !keyed_pairs.is_empty() {
                        let tuple = make_tuple(keyed_pairs);
                        segments.push(py_expr!("dict({tuple:expr})", tuple = tuple));
                    }

                    let Some(item) = iter.next() else {
                        break;
                    };

                    if let Some(key) = item.key {
                        let pair =
                            py_expr!("({key:expr}, {value:expr})", key = key, value = item.value,);
                        let tuple = make_tuple(vec![pair]);
                        segments.push(py_expr!("dict({tuple:expr})", tuple = tuple));
                    } else {
                        segments.push(py_expr!("dict({mapping:expr})", mapping = item.value));
                    }
                }

                match segments.len() {
                    0 => {
                        let tuple = make_tuple(Vec::new());
                        py_expr!("dict({tuple:expr})", tuple = tuple)
                    }
                    1 => segments.into_iter().next().unwrap(),
                    _ => {
                        let mut parts = segments.into_iter();
                        let mut expr = parts.next().expect("segments is non-empty");
                        for part in parts {
                            expr =
                                py_expr!("{left:expr} | {right:expr}", left = expr, right = part,);
                        }
                        expr
                    }
                }
            }
            Expr::BinOp(ast::ExprBinOp {
                left, right, op, ..
            }) => {
                let func_name = match op {
                    Operator::Add => "add",
                    Operator::Sub => "sub",
                    Operator::Mult => "mul",
                    Operator::MatMult => "matmul",
                    Operator::Div => "truediv",
                    Operator::Mod => "mod",
                    Operator::Pow => "pow",
                    Operator::LShift => "lshift",
                    Operator::RShift => "rshift",
                    Operator::BitOr => "or_",
                    Operator::BitXor => "xor",
                    Operator::BitAnd => "and_",
                    Operator::FloorDiv => "floordiv",
                };
                make_binop(func_name, *left, *right)
            }
            Expr::UnaryOp(ast::ExprUnaryOp { operand, op, .. }) => {
                let func_name = match op {
                    UnaryOp::Not => "not_",
                    UnaryOp::Invert => "invert",
                    UnaryOp::USub => "neg",
                    UnaryOp::UAdd => "pos",
                };
                make_unaryop(func_name, *operand)
            }
            Expr::Subscript(ast::ExprSubscript {
                value, slice, ctx, ..
            }) if matches!(ctx, ast::ExprContext::Load) => {
                let obj = *value;
                let key = *slice;
                make_binop("getitem", obj, key)
            }
            _ => {
                walk_expr(self, expr);
                return;
            }
        };
        *expr = rewritten;
        self.visit_expr(expr);
    }

    fn visit_stmt(&mut self, stmt: &mut Stmt) {
        let rewritten = self.process_statements(vec![stmt.clone()]);
        *stmt = match rewritten.len() {
            0 => py_stmt!("{body:stmt}", body = Vec::new()),
            1 => rewritten.into_iter().next().unwrap(),
            _ => py_stmt!("{body:stmt}", body = rewritten),
        };
    }
}

#[cfg(test)]
mod tests {
    crate::transform_fixture_test!("tests_expr.txt");
}
