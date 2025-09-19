use super::{
    context::Context, rewrite_assert, rewrite_class_def, rewrite_complex_expr, rewrite_decorator,
    rewrite_expr_to_stmt, rewrite_for_loop, rewrite_import, rewrite_match_case, rewrite_string,
    rewrite_try_except, rewrite_with, Options,
};
use crate::body_transform::{walk_expr, walk_stmt, Transformer};
use crate::template::{make_binop, make_generator, make_tuple, make_unaryop, single_stmt};
use crate::{py_expr, py_stmt};
use ruff_python_ast::{self as ast, Expr, Operator, Stmt, UnaryOp};
use ruff_text_size::TextRange;

pub struct ExprRewriter<'a> {
    ctx: &'a Context,
    options: Options,
}

impl<'a> ExprRewriter<'a> {
    pub fn new(ctx: &'a Context) -> Self {
        Self {
            options: ctx.options,
            ctx,
        }
    }

    pub fn rewrite_body(&mut self, body: &mut Vec<Stmt>) {
        for stmt in body.iter_mut() {
            self.visit_stmt(stmt);
        }
    }

    fn wrap_truthy_expr(&mut self, expr: &mut Expr) {
        if !self.options.truthy || is_truth_call(expr) {
            return;
        }

        let original = expr.clone();
        *expr = py_expr!(
            "
__dp__.truth({expr:expr})
",
            expr = original,
        );
    }

    fn lower_lambdas_generators(&mut self, stmt: &mut Stmt) -> bool {
        let mut lowerer = rewrite_expr_to_stmt::LambdaGeneratorLowerer::new(self.ctx);
        lowerer.rewrite(stmt);
        let mut lowered_functions = lowerer.into_statements();
        if lowered_functions.is_empty() {
            false
        } else {
            lowered_functions.push(stmt.clone());
            *stmt = single_stmt(lowered_functions);
            true
        }
    }

    fn rewrite_target(&mut self, target: Expr, value: Expr, out: &mut Vec<Stmt>) {
        match target {
            Expr::Tuple(tuple) => {
                self.rewrite_unpack_target(tuple.elts, value, out, UnpackTargetKind::Tuple);
            }
            Expr::List(list) => {
                self.rewrite_unpack_target(list.elts, value, out, UnpackTargetKind::List);
            }
            Expr::Attribute(attr) => {
                let obj = (*attr.value).clone();
                let mut stmt = py_stmt!(
                    "
__dp__.setattr({obj:expr}, {name:literal}, {value:expr})
",
                    obj = obj,
                    name = attr.attr.as_str(),
                    value = value,
                );
                walk_stmt(self, &mut stmt);
                out.push(stmt);
            }
            Expr::Subscript(sub) => {
                let obj = (*sub.value).clone();
                let key = (*sub.slice).clone();
                let mut stmt = py_stmt!(
                    "
__dp__.setitem({obj:expr}, {key:expr}, {value:expr})
",
                    obj = obj,
                    key = key,
                    value = value,
                );
                walk_stmt(self, &mut stmt);
                out.push(stmt);
            }
            Expr::Name(_) => {
                let mut stmt = py_stmt!(
                    "
{target:expr} = {value:expr}
",
                    target = target,
                    value = value,
                );
                walk_stmt(self, &mut stmt);
                out.push(stmt);
            }
            _ => {
                panic!("unsupported assignment target");
            }
        }
    }

    fn rewrite_unpack_target(
        &mut self,
        elts: Vec<Expr>,
        value: Expr,
        out: &mut Vec<Stmt>,
        kind: UnpackTargetKind,
    ) {
        let (tmp_expr, tmp_stmt) = match value {
            Expr::Name(_) => (value, None),
            other => {
                let tmp_name = self.ctx.fresh("tmp");
                let mut stmt = py_stmt!(
                    "
{name:id} = {value:expr}",
                    name = tmp_name.as_str(),
                    value = other,
                );
                walk_stmt(self, &mut stmt);
                (
                    py_expr!(
                        "
{name:id}",
                        name = tmp_name.as_str(),
                    ),
                    Some(stmt),
                )
            }
        };

        if let Some(stmt) = tmp_stmt {
            out.push(stmt);
        }

        let elts_len = elts.len();
        let mut starred_index: Option<usize> = None;
        for (i, elt) in elts.iter().enumerate() {
            if matches!(elt, Expr::Starred(_)) {
                if starred_index.is_some() {
                    panic!("unsupported starred assignment target");
                }
                starred_index = Some(i);
            }
        }

        let prefix_len = starred_index.unwrap_or(elts_len);
        let suffix_len = starred_index.map_or(0, |idx| elts_len - idx - 1);

        for (i, elt) in elts.into_iter().enumerate() {
            match elt {
                Expr::Starred(ast::ExprStarred { value, .. }) => {
                    let slice_expr = if suffix_len == 0 {
                        py_expr!(
                            "
__dp__.getitem({tmp:expr}, slice({start:literal}, None, None))",
                            tmp = tmp_expr.clone(),
                            start = prefix_len,
                        )
                    } else {
                        let stop = -(suffix_len as isize);
                        py_expr!(
                            "
__dp__.getitem({tmp:expr}, slice({start:literal}, {stop:literal}, None))",
                            tmp = tmp_expr.clone(),
                            start = prefix_len,
                            stop = stop,
                        )
                    };
                    let collection_expr = match kind {
                        UnpackTargetKind::Tuple => py_expr!(
                            "
tuple({slice:expr})",
                            slice = slice_expr,
                        ),
                        UnpackTargetKind::List => py_expr!(
                            "
list({slice:expr})",
                            slice = slice_expr,
                        ),
                    };
                    self.rewrite_target(*value, collection_expr, out);
                }
                _ => {
                    let value = match starred_index {
                        Some(star_idx) if i > star_idx => {
                            let idx = (i as isize) - (elts_len as isize);
                            py_expr!(
                                "
__dp__.getitem({tmp:expr}, {idx:literal})",
                                tmp = tmp_expr.clone(),
                                idx = idx,
                            )
                        }
                        _ => py_expr!(
                            "
__dp__.getitem({tmp:expr}, {idx:literal})",
                            tmp = tmp_expr.clone(),
                            idx = i,
                        ),
                    };
                    self.rewrite_target(elt, value, out);
                }
            }
        }
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

enum UnpackTargetKind {
    Tuple,
    List,
}

impl<'a> Transformer for ExprRewriter<'a> {
    fn visit_expr(&mut self, expr: &mut Expr) {
        let original = expr.clone();
        *expr = match original {
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
            Expr::EllipsisLiteral(_) => {
                py_expr!("Ellipsis")
            }
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
            Expr::NoneLiteral(_) => {
                py_expr!("None")
            }
            Expr::Tuple(tuple) if matches!(tuple.ctx, ast::ExprContext::Load) => {
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
            _ => original,
        };
        walk_expr(self, expr);
    }

    fn visit_stmt(&mut self, stmt: &mut Stmt) {
        rewrite_complex_expr::rewrite(stmt, self.ctx);

        if self.lower_lambdas_generators(stmt) {
            self.visit_stmt(stmt);
            return;
        }

        if matches!(stmt, Stmt::FunctionDef(_)) {
            if let Stmt::FunctionDef(ast::StmtFunctionDef {
                decorator_list,
                name,
                ..
            }) = stmt
            {
                if !decorator_list.is_empty() {
                    let decorators = std::mem::take(decorator_list);
                    let func_name = name.id.clone();
                    let func_def = stmt.clone();
                    *stmt = rewrite_decorator::rewrite(
                        decorators,
                        func_name.as_str(),
                        func_def,
                        None,
                        self.ctx,
                    );
                    self.visit_stmt(stmt);
                    return;
                }
            }
            walk_stmt(self, stmt);
            return;
        }

        let current = stmt.clone();
        *stmt = match current {
            Stmt::With(with) => rewrite_with::rewrite(with, self.ctx, self),
            Stmt::For(for_stmt) => rewrite_for_loop::rewrite(for_stmt, self.ctx, self),
            Stmt::Assert(assert) => rewrite_assert::rewrite(assert.clone()),
            Stmt::ClassDef(class_def) => {
                let decorated = !class_def.decorator_list.is_empty();
                let mut base_stmt = rewrite_class_def::rewrite(class_def.clone(), decorated);
                self.visit_stmt(&mut base_stmt);
                let class_name = class_def.name.id.clone();
                if decorated {
                    let decorators = class_def.decorator_list.clone();
                    let base_name = format!("_dp_class_{}", class_name);
                    rewrite_decorator::rewrite(
                        decorators,
                        class_name.as_str(),
                        base_stmt,
                        Some(base_name.as_str()),
                        self.ctx,
                    )
                } else {
                    base_stmt
                }
            }
            Stmt::Try(try_stmt) => rewrite_try_except::rewrite(try_stmt.clone(), self.ctx),
            Stmt::Match(match_stmt) => rewrite_match_case::rewrite(match_stmt.clone(), self.ctx),
            Stmt::Import(import) => rewrite_import::rewrite(import.clone()),
            Stmt::ImportFrom(import_from) => {
                match rewrite_import::rewrite_from(import_from.clone(), &self.options) {
                    Some(stmt) => stmt,
                    None => Stmt::ImportFrom(import_from.clone()),
                }
            }
            Stmt::AnnAssign(ann_assign) => {
                if let Some(value) = ann_assign.value.clone().map(|v| *v) {
                    let mut stmts = Vec::new();
                    self.rewrite_target((*ann_assign.target).clone(), value, &mut stmts);
                    single_stmt(stmts)
                } else {
                    py_stmt!("{body:stmt}", body = Vec::new())
                }
            }
            Stmt::Assign(assign) => {
                let value = (*assign.value).clone();
                let mut stmts = Vec::new();
                if assign.targets.len() > 1 {
                    let tmp_name = self.ctx.fresh("tmp");
                    let tmp_expr = py_expr!(
                        "
{name:id}
",
                        name = tmp_name.as_str(),
                    );
                    let tmp_stmt = py_stmt!(
                        "
{name:id} = {value:expr}
",
                        name = tmp_name.as_str(),
                        value = value,
                    );

                    stmts.push(tmp_stmt);
                    for target in &assign.targets {
                        self.rewrite_target(target.clone(), tmp_expr.clone(), &mut stmts);
                    }
                } else {
                    self.rewrite_target(assign.targets[0].clone(), value, &mut stmts);
                }

                single_stmt(stmts)
            }
            Stmt::AugAssign(aug) => {
                let target = (*aug.target).clone();
                let value = (*aug.value).clone();

                let func_name = match aug.op {
                    Operator::Add => "iadd",
                    Operator::Sub => "isub",
                    Operator::Mult => "imul",
                    Operator::MatMult => "imatmul",
                    Operator::Div => "itruediv",
                    Operator::Mod => "imod",
                    Operator::Pow => "ipow",
                    Operator::LShift => "ilshift",
                    Operator::RShift => "irshift",
                    Operator::BitOr => "ior",
                    Operator::BitXor => "ixor",
                    Operator::BitAnd => "iand",
                    Operator::FloorDiv => "ifloordiv",
                };

                let mut target_expr = target.clone();
                match &mut target_expr {
                    Expr::Name(name) => name.ctx = ast::ExprContext::Load,
                    Expr::Attribute(attr) => attr.ctx = ast::ExprContext::Load,
                    Expr::Subscript(sub) => sub.ctx = ast::ExprContext::Load,
                    _ => {}
                }
                let call = make_binop(func_name, target_expr, value);
                let mut stmts = Vec::new();
                self.rewrite_target(target, call, &mut stmts);
                single_stmt(stmts)
            }
            Stmt::Delete(del) => {
                let mut stmts = Vec::with_capacity(del.targets.len());
                for target in &del.targets {
                    let new_stmt = if let Expr::Subscript(sub) = target {
                        py_stmt!(
                            "__dp__.delitem({obj:expr}, {key:expr})",
                            obj = (*sub.value).clone(),
                            key = (*sub.slice).clone(),
                        )
                    } else if let Expr::Attribute(attr) = target {
                        py_stmt!(
                            "__dp__.delattr({obj:expr}, {name:literal})",
                            obj = (*attr.value).clone(),
                            name = attr.attr.as_str(),
                        )
                    } else {
                        py_stmt!("del {target:expr}", target = target.clone())
                    };

                    stmts.push(new_stmt);
                }
                single_stmt(stmts)
            }
            Stmt::Raise(ast::StmtRaise {
                exc: Some(exc),
                cause: Some(cause),
                ..
            }) => {
                py_stmt!(
                    "raise __dp__.raise_from({exc:expr}, {cause:expr})",
                    exc = *exc.clone(),
                    cause = *cause.clone(),
                )
            }
            _ => stmt.clone(),
        };

        walk_stmt(self, stmt);

        if self.options.truthy {
            match stmt {
                Stmt::If(ast::StmtIf {
                    test,
                    elif_else_clauses,
                    ..
                }) => {
                    self.wrap_truthy_expr(test);
                    for clause in elif_else_clauses {
                        if let Some(test) = &mut clause.test {
                            self.wrap_truthy_expr(test);
                        }
                    }
                }
                Stmt::While(ast::StmtWhile { test, .. }) => {
                    self.wrap_truthy_expr(test);
                }
                _ => {}
            }
        }

        if self.lower_lambdas_generators(stmt) {
            self.visit_stmt(stmt);
            return;
        }
    }
}

fn is_truth_call(expr: &Expr) -> bool {
    match expr {
        Expr::Call(ast::ExprCall {
            func, arguments, ..
        }) if arguments.args.len() == 1 && arguments.keywords.is_empty() => match func.as_ref() {
            Expr::Attribute(ast::ExprAttribute { value, attr, .. }) if attr.as_str() == "truth" => {
                matches!(
                    value.as_ref(),
                    Expr::Name(ast::ExprName { id, .. })
                        if id.as_str() == "__dp__"
                )
            }
            _ => false,
        },
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use crate::test_util::assert_transform_eq_truthy;

    crate::transform_fixture_test!("tests_expr.txt");

    #[test]
    fn rewrites_truthy_if_condition() {
        let input = r#"
if a:
    pass
else:
    pass
"#;
        let expected = r#"
if __dp__.truth(a):
    pass
else:
    pass
"#;
        assert_transform_eq_truthy(input, expected);
    }

    #[test]
    fn rewrites_truthy_elif_and_else() {
        let input = r#"
if a:
    pass
elif b:
    pass
else:
    pass
"#;
        let expected = r#"
if __dp__.truth(a):
    pass
elif __dp__.truth(b):
    pass
else:
    pass
"#;
        assert_transform_eq_truthy(input, expected);
    }

    #[test]
    fn rewrites_truthy_while_condition() {
        let input = r#"
while a:
    pass
"#;
        let expected = r#"
while __dp__.truth(a):
    pass
"#;
        assert_transform_eq_truthy(input, expected);
    }
}
