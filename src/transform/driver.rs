use super::{
    context::Context,
    rewrite_assert, rewrite_assign_del, rewrite_class_def, rewrite_decorator, rewrite_exception,
    rewrite_expr_to_stmt::{expr_boolop_to_stmts, expr_compare_to_stmts, expr_yield_from_to_stmt},
    rewrite_func_expr, rewrite_import, rewrite_loop, rewrite_match_case, rewrite_string,
    rewrite_with, Options,
};
use crate::body_transform::{walk_expr, walk_stmt, Transformer};
use crate::template::{is_simple, make_binop, make_generator, make_tuple, make_unaryop};
use crate::{py_expr, py_stmt};
use ruff_python_ast::{self as ast, Expr, Operator, Stmt, UnaryOp};
use ruff_text_size::TextRange;
use std::mem::take;

pub struct ExprRewriter<'a> {
    ctx: &'a Context,
    options: Options,
    buf: Vec<Stmt>,
}

pub(crate) enum Rewrite {
    Walk(Vec<Stmt>),
    Visit(Vec<Stmt>),
}

impl Rewrite {
    pub(crate) fn into_statements(self) -> Vec<Stmt> {
        match self {
            Rewrite::Walk(stmts) | Rewrite::Visit(stmts) => stmts,
        }
    }
}

impl<'a> ExprRewriter<'a> {
    pub fn new(ctx: &'a Context) -> Self {
        Self {
            options: ctx.options,
            ctx,
            buf: Vec::new(),
        }
    }

    pub(super) fn context(&self) -> &Context {
        self.ctx
    }

    pub(crate) fn rewrite_block(&mut self, body: Vec<Stmt>) -> Vec<Stmt> {
        self.process_statements(body)
    }

    pub(crate) fn with_function_scope<F, R>(&mut self, qualname: String, f: F) -> R
    where
        F: FnOnce(&mut Self) -> R,
    {
        self.ctx.push_function(qualname);
        let result = f(self);
        self.ctx.pop_function();
        result
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

    /// Expand the buffered statements for an expression directly in-place within a block,
    /// instead of emitting them before the block executes.
    pub(super) fn expand_here(&mut self, expr: &mut Expr) -> Vec<Stmt> {
        let saved = take(&mut self.buf);
        self.visit_expr(expr);
        let produced = take(&mut self.buf);
        self.buf = saved;
        produced
    }

    pub(super) fn maybe_placeholder(&mut self, mut expr: Expr) -> Expr {
        fn is_temp_skippable(expr: &Expr) -> bool {
            is_simple(expr) && !matches!(expr, Expr::StringLiteral(_) | Expr::BytesLiteral(_))
        }

        if is_temp_skippable(&expr) {
            return expr;
        }

        self.visit_expr(&mut expr);

        if is_temp_skippable(&expr) {
            return expr;
        }

        let tmp = self.ctx.fresh("tmp");
        let placeholder_expr = py_expr!("{tmp:id}", tmp = tmp.as_str());
        let assign = py_stmt!("{tmp:id} = {value:expr}", tmp = tmp.as_str(), value = expr);
        self.buf.extend(assign);
        placeholder_expr
    }

    pub(super) fn maybe_placeholder_within(&mut self, expr: Expr) -> (Vec<Stmt>, Expr) {
        let saved = take(&mut self.buf);
        let expr = self.maybe_placeholder(expr);
        let stmts = take(&mut self.buf);
        self.buf = saved;
        (stmts, expr)
    }

    fn rewrite_stmt(&mut self, stmt: Stmt) -> Rewrite {
        match stmt {
            Stmt::FunctionDef(mut func_def) => {
                let func_name = func_def.name.id.as_str().to_string();
                let qualname = self
                    .context()
                    .current_function_qualname()
                    .map(|enclosing| format!("{enclosing}.<locals>.{func_name}"))
                    .unwrap_or(func_name.clone());
                self.with_function_scope(qualname, |rewriter| {
                    let body = take(&mut func_def.body);
                    func_def.body = rewriter.rewrite_block(body);
                });
                let decorators = take(&mut func_def.decorator_list);
                let func_name = func_def.name.id.clone();
                rewrite_decorator::rewrite(
                    decorators,
                    func_name.as_str(),
                    vec![Stmt::FunctionDef(func_def)],
                    self.ctx,
                )
            }
            Stmt::With(with) => rewrite_with::rewrite(with, self.ctx, self),
            Stmt::While(while_stmt) => rewrite_loop::rewrite_while(while_stmt, self),
            Stmt::For(for_stmt) => rewrite_loop::rewrite_for(for_stmt, self.ctx, self),
            Stmt::Assert(assert) => rewrite_assert::rewrite(assert),
            Stmt::ClassDef(mut class_def) => {
                let class_name = class_def.name.id.as_str().to_string();
                let qualname = self
                    .context()
                    .current_function_qualname()
                    .map(|enclosing| format!("{enclosing}.<locals>.{class_name}"));
                let decorators = take(&mut class_def.decorator_list);
                rewrite_class_def::rewrite(class_def.clone(), decorators, self, qualname)
            }
            Stmt::Try(try_stmt) => rewrite_exception::rewrite_try(try_stmt, self.ctx),
            Stmt::If(if_stmt)
                if if_stmt
                    .elif_else_clauses
                    .iter()
                    .any(|clause| clause.test.is_some()) =>
            {
                Rewrite::Visit(vec![expand_if_chain(if_stmt).into()])
            }
            Stmt::Match(match_stmt) => rewrite_match_case::rewrite(match_stmt, self.ctx),
            Stmt::Import(import) => rewrite_import::rewrite(import),
            Stmt::ImportFrom(import_from) => {
                rewrite_import::rewrite_from(import_from.clone(), &self.options)
            }

            Stmt::AnnAssign(ann_assign) => rewrite_assign_del::rewrite_ann_assign(self, ann_assign),
            Stmt::Assign(assign) => rewrite_assign_del::rewrite_assign(self, assign),
            Stmt::AugAssign(aug) => rewrite_assign_del::rewrite_aug_assign(self, aug),
            Stmt::Delete(del) => rewrite_assign_del::rewrite_delete(self, del),
            Stmt::Raise(raise) => rewrite_exception::rewrite_raise(raise),
            other => Rewrite::Walk(vec![other]),
        }
    }
}

fn make_tuple_splat(elts: Vec<Expr>) -> Expr {
    let mut segments: Vec<Expr> = Vec::new();
    let mut values: Vec<Expr> = Vec::new();

    for elt in elts {
        match elt {
            Expr::Starred(ast::ExprStarred { value, .. }) => {
                if !values.is_empty() {
                    segments.push(make_tuple(std::mem::take(&mut values)));
                }
                segments.push(py_expr!("__dp__.tuple({value:expr})", value = *value));
            }
            other => values.push(other),
        }
    }

    if !values.is_empty() {
        segments.push(make_tuple(values));
    }

    segments
        .into_iter()
        .reduce(|left, right| py_expr!("{left:expr} + {right:expr}", left = left, right = right))
        .unwrap_or_else(|| make_tuple(Vec::new()))
}

fn expand_if_chain(mut if_stmt: ast::StmtIf) -> ast::StmtIf {
    let mut else_body: Option<Vec<Stmt>> = None;

    for clause in if_stmt.elif_else_clauses.into_iter().rev() {
        match clause.test {
            Some(test) => {
                let mut nested_if = ast::StmtIf {
                    node_index: ast::AtomicNodeIndex::default(),
                    range: TextRange::default(),
                    test: Box::new(test),
                    body: clause.body,
                    elif_else_clauses: Vec::new(),
                };

                if let Some(body) = else_body.take() {
                    nested_if.elif_else_clauses.push(ast::ElifElseClause {
                        test: None,
                        body,
                        range: TextRange::default(),
                        node_index: ast::AtomicNodeIndex::default(),
                    });
                }

                else_body = Some(vec![Stmt::If(nested_if)]);
            }
            None => {
                else_body = Some(clause.body);
            }
        }
    }

    if let Some(body) = else_body {
        if_stmt.elif_else_clauses = vec![ast::ElifElseClause {
            range: TextRange::default(),
            node_index: ast::AtomicNodeIndex::default(),
            test: None,
            body,
        }];
    } else {
        if_stmt.elif_else_clauses = Vec::new();
    }

    if_stmt
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
                self.buf.extend(assign_target);
                value_expr
            }
            Expr::If(if_expr) => {
                let tmp = self.ctx.fresh("tmp");
                let ast::ExprIf {
                    test, body, orelse, ..
                } = if_expr;
                let assign = py_stmt!(
                    r#"
if {cond:expr}:
    {tmp:id} = {body:expr}
else:
    {tmp:id} = {orelse:expr}
"#,
                    cond = *test,
                    tmp = tmp.as_str(),
                    body = *body,
                    orelse = *orelse,
                );
                self.buf.extend(assign);
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
                let stmts = expr_compare_to_stmts(self.ctx, tmp.as_str(), compare);
                self.buf.extend(stmts);
                py_expr!("{tmp:id}", tmp = tmp.as_str())
            }
            Expr::YieldFrom(yield_from) => {
                let tmp = self.ctx.fresh("tmp");
                let stmts = expr_yield_from_to_stmt(self.ctx, tmp.as_str(), yield_from);
                self.buf.extend(stmts);
                py_expr!("{tmp:id}", tmp = tmp.as_str())
            }
            Expr::Lambda(lambda) => {
                rewrite_func_expr::rewrite_lambda(lambda, self.ctx, &mut self.buf)
            }
            Expr::Generator(generator) => {
                rewrite_func_expr::rewrite_generator(generator, self.ctx, &mut self.buf)
            }
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
                    "__dp__.slice({lower:expr}, {upper:expr}, {step:expr})",
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
            Expr::ListComp(ast::ExprListComp {
                elt, generators, ..
            }) => py_expr!(
                "__dp__.list({expr:expr})",
                expr = make_generator(*elt, generators)
            ),
            Expr::SetComp(ast::ExprSetComp {
                elt, generators, ..
            }) => py_expr!(
                "__dp__.set({expr:expr})",
                expr = make_generator(*elt, generators)
            ),
            Expr::DictComp(ast::ExprDictComp {
                key,
                value,
                generators,
                ..
            }) => {
                let tuple = py_expr!("({key:expr}, {value:expr})", key = *key, value = *value,);
                py_expr!(
                    "__dp__.dict({expr:expr})",
                    expr = make_generator(tuple, generators)
                )
            }

            // tuple/list/dict unpacking
            Expr::Tuple(tuple)
                if matches!(tuple.ctx, ast::ExprContext::Load)
                    && tuple.elts.iter().any(|elt| matches!(elt, Expr::Starred(_))) =>
            {
                make_tuple_splat(tuple.elts)
            }
            Expr::List(list) if matches!(list.ctx, ast::ExprContext::Load) => {
                let tuple = make_tuple_splat(list.elts);
                py_expr!("__dp__.list({tuple:expr})", tuple = tuple,)
            }
            Expr::Set(ast::ExprSet { elts, .. }) => {
                let tuple = make_tuple(elts);
                py_expr!("__dp__.set({tuple:expr})", tuple = tuple,)
            }
            Expr::Dict(ast::ExprDict { items, .. }) => {
                let mut segments: Vec<Expr> = Vec::new();

                let mut keyed_pairs = Vec::new();
                for item in items.into_iter() {
                    match item {
                        ast::DictItem {
                            key: Some(key),
                            value,
                        } => {
                            keyed_pairs.push(py_expr!(
                                "({key:expr}, {value:expr})",
                                key = key,
                                value = value,
                            ));
                        }
                        ast::DictItem { key: None, value } => {
                            if !keyed_pairs.is_empty() {
                                let tuple = make_tuple(take(&mut keyed_pairs));
                                segments.push(py_expr!("__dp__.dict({tuple:expr})", tuple = tuple));
                            }
                            segments.push(py_expr!("__dp__.dict({mapping:expr})", mapping = value));
                        }
                    }
                }

                if !keyed_pairs.is_empty() {
                    let tuple = make_tuple(take(&mut keyed_pairs));
                    segments.push(py_expr!("__dp__.dict({tuple:expr})", tuple = tuple));
                }

                match segments.len() {
                    0 => {
                        py_expr!("__dp__.dict()")
                    }
                    _ => segments
                        .into_iter()
                        .reduce(|left, right| {
                            py_expr!("{left:expr} | {right:expr}", left = left, right = right)
                        })
                        .expect("segments is non-empty"),
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
            }) if matches!(ctx, ast::ExprContext::Load) => make_binop("getitem", *value, *slice),
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
            0 => py_stmt!("pass")[0].clone(),
            _ => rewritten[0].clone(),
        };
    }
}

#[cfg(test)]
mod tests {
    crate::transform_fixture_test!("tests_expr.txt");
}
