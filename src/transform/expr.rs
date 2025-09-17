use super::{
    context::Context, rewrite_assert, rewrite_class_def, rewrite_complex_expr, rewrite_decorator,
    rewrite_expr_to_stmt, rewrite_for_loop, rewrite_import, rewrite_match_case, rewrite_string,
    rewrite_try_except, rewrite_with, Options,
};
use crate::template::{make_binop, make_generator, make_tuple, make_unaryop, single_stmt};
use crate::{py_expr, py_stmt};
use ruff_python_ast::visitor::transformer::{walk_expr, walk_stmt, Transformer};
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

    pub fn rewrite_body(&self, body: &mut Vec<Stmt>) {
        for stmt in body.iter_mut() {
            self.visit_stmt(stmt);
        }
    }

    fn wrap_truthy_expr(&self, expr: &mut Expr) {
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

    fn lower_lambdas_generators(&self, stmt: &mut Stmt) -> bool {
        let lowerer = rewrite_expr_to_stmt::LambdaGeneratorLowerer::new(self.ctx);
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

    fn rewrite_target(&self, target: Expr, value: Expr, out: &mut Vec<Stmt>) {
        match target {
            Expr::Tuple(tuple) => {
                self.rewrite_unpack_target(
                    tuple.elts,
                    value,
                    out,
                    UnpackTargetKind::Tuple,
                );
            }
            Expr::List(list) => {
                self.rewrite_unpack_target(
                    list.elts,
                    value,
                    out,
                    UnpackTargetKind::List,
                );
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
        &self,
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
        for (i, elt) in elts.into_iter().enumerate() {
            match elt {
                Expr::Starred(ast::ExprStarred { value, .. }) if i == elts_len - 1 => {
                    let slice_expr = py_expr!(
                        "
__dp__.getitem({tmp:expr}, slice({start:literal}, None, None))",
                        tmp = tmp_expr.clone(),
                        start = i,
                    );
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
                    let mut elt_stmt = py_stmt!(
                        "
{target:expr} = {value:expr}",
                        target = *value,
                        value = collection_expr,
                    );
                    walk_stmt(self, &mut elt_stmt);
                    out.push(elt_stmt);
                }
                Expr::Starred(_) => {
                    panic!("unsupported starred assignment target");
                }
                _ => {
                    let mut elt_stmt = py_stmt!(
                        "
{target:expr} = __dp__.getitem({tmp:expr}, {idx:literal})",
                        target = elt,
                        tmp = tmp_expr.clone(),
                        idx = i,
                    );
                    walk_stmt(self, &mut elt_stmt);
                    out.push(elt_stmt);
                }
            }
        }
    }
}

enum UnpackTargetKind {
    Tuple,
    List,
}

impl<'a> Transformer for ExprRewriter<'a> {
    fn visit_expr(&self, expr: &mut Expr) {
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
            Expr::List(ast::ExprList { elts, ctx, .. })
                if matches!(ctx, ast::ExprContext::Load) =>
            {
                let tuple = make_tuple(elts);
                py_expr!("list({tuple:expr})", tuple = tuple,)
            }
            Expr::Set(ast::ExprSet { elts, .. }) => {
                let tuple = make_tuple(elts);
                py_expr!("set({tuple:expr})", tuple = tuple,)
            }
            Expr::Dict(ast::ExprDict { items, .. })
                if items.iter().all(|item| item.key.is_some()) =>
            {
                let pairs: Vec<Expr> = items
                    .into_iter()
                    .map(|item| {
                        let key = item.key.unwrap();
                        let value = item.value;
                        py_expr!("({key:expr}, {value:expr})", key = key, value = value,)
                    })
                    .collect();
                let tuple = make_tuple(pairs);
                py_expr!("dict({tuple:expr})", tuple = tuple,)
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

    fn visit_stmt(&self, stmt: &mut Stmt) {
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
            Stmt::With(with) => rewrite_with::rewrite(with.clone(), self.ctx),
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
            Stmt::For(for_stmt) => rewrite_for_loop::rewrite(for_stmt.clone(), self.ctx),
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
                let target = (*ann_assign.target).clone();
                let value = ann_assign
                    .value
                    .clone()
                    .map(|v| *v)
                    .unwrap_or_else(|| py_expr!("None"));
                let mut stmts = Vec::new();
                self.rewrite_target(target, value, &mut stmts);
                single_stmt(stmts)
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
    use crate::test_util::{assert_transform_eq, assert_transform_eq_truthy};

    #[test]
    fn rewrites_binary_ops() {
        let cases = [("a + b", "__dp__.add(a, b)"), ("a - b", "__dp__.sub(a, b)")];

        for (input, expected) in cases {
            assert_transform_eq(input, expected);
        }
    }

    #[test]
    fn rewrites_aug_assign() {
        let input = r#"
x = 1
x += 2
"#;
        let expected = r#"
x = 1
x = __dp__.iadd(x, 2)
"#;
        assert_transform_eq(input, expected);
    }

    #[test]
    fn rewrites_attribute_aug_assign() {
        let input = "a.b += c";
        let expected = "__dp__.setattr(a, \"b\", __dp__.iadd(a.b, c))";
        assert_transform_eq(input, expected);
    }

    #[test]
    fn rewrites_ann_assign() {
        let input = "x: int = 1";
        let expected = "x = 1";
        assert_transform_eq(input, expected);
    }

    #[test]
    fn rewrites_unary_ops() {
        let cases = [
            ("-a", "__dp__.neg(a)"),
            ("~b", "__dp__.invert(b)"),
            ("not c", "__dp__.not_(c)"),
            ("+a", "__dp__.pos(a)"),
        ];

        for (input, expected) in cases {
            assert_transform_eq(input, expected);
        }
    }

    #[test]
    fn rewrites_bool_ops() {
        let cases = [
            (
                "a or b",
                r#"
_dp_tmp_1 = a
if __dp__.not_(_dp_tmp_1):
    _dp_tmp_1 = b
_dp_tmp_1
"#,
            ),
            (
                "a and b",
                r#"
_dp_tmp_1 = a
if _dp_tmp_1:
    _dp_tmp_1 = b
_dp_tmp_1
"#,
            ),
            (
                "f() or a",
                r#"
_dp_tmp_1 = f()
_dp_tmp_2 = _dp_tmp_1
if __dp__.not_(_dp_tmp_2):
    _dp_tmp_2 = a
_dp_tmp_2
"#,
            ),
            (
                "f() and a",
                r#"
_dp_tmp_1 = f()
_dp_tmp_2 = _dp_tmp_1
if _dp_tmp_2:
    _dp_tmp_2 = a
_dp_tmp_2
"#,
            ),
        ];

        for (input, expected) in cases {
            assert_transform_eq(input, expected);
        }
    }

    #[test]
    fn rewrites_multi_bool_ops() {
        let input = "a or b or c";
        let expected = r#"
_dp_tmp_1 = a
if __dp__.not_(_dp_tmp_1):
    _dp_tmp_1 = b
if __dp__.not_(_dp_tmp_1):
    _dp_tmp_1 = c
_dp_tmp_1
"#;
        assert_transform_eq(input, expected);

        let input = "a and b and c";
        let expected = r#"
_dp_tmp_1 = a
if _dp_tmp_1:
    _dp_tmp_1 = b
if _dp_tmp_1:
    _dp_tmp_1 = c
_dp_tmp_1
"#;
        assert_transform_eq(input, expected);
    }

    #[test]
    fn rewrites_bool_assignments() {
        let input = "x = a and b";
        let expected = r#"
_dp_tmp_1 = a
if _dp_tmp_1:
    _dp_tmp_1 = b
x = _dp_tmp_1
"#;
        assert_transform_eq(input, expected);

        let input = "x = a or b";
        let expected = r#"
_dp_tmp_1 = a
if __dp__.not_(_dp_tmp_1):
    _dp_tmp_1 = b
x = _dp_tmp_1
"#;
        assert_transform_eq(input, expected);
    }

    #[test]
    fn rewrites_comparisons() {
        let cases = [
            (
                "a == b",
                r#"
_dp_tmp_1 = __dp__.eq(a, b)
_dp_tmp_1
"#,
            ),
            (
                "a != b",
                r#"
_dp_tmp_1 = __dp__.ne(a, b)
_dp_tmp_1
"#,
            ),
            (
                "a < b",
                r#"
_dp_tmp_1 = __dp__.lt(a, b)
_dp_tmp_1
"#,
            ),
            (
                "a <= b",
                r#"
_dp_tmp_1 = __dp__.le(a, b)
_dp_tmp_1
"#,
            ),
            (
                "a > b",
                r#"
_dp_tmp_1 = __dp__.gt(a, b)
_dp_tmp_1
"#,
            ),
            (
                "a >= b",
                r#"
_dp_tmp_1 = __dp__.ge(a, b)
_dp_tmp_1
"#,
            ),
            (
                "a is b",
                r#"
_dp_tmp_1 = __dp__.is_(a, b)
_dp_tmp_1
"#,
            ),
            (
                "a is not b",
                r#"
_dp_tmp_1 = __dp__.is_not(a, b)
_dp_tmp_1
"#,
            ),
            (
                "a in b",
                r#"
_dp_tmp_1 = __dp__.contains(b, a)
_dp_tmp_1
"#,
            ),
            (
                "a not in b",
                r#"
_dp_tmp_1 = __dp__.not_(__dp__.contains(b, a))
_dp_tmp_1
"#,
            ),
        ];

        for (input, expected) in cases {
            assert_transform_eq(input, expected);
        }
    }

    #[test]
    fn rewrites_if_expr() {
        let cases = [
            (
                r#"
a if b else c
"#,
                r#"
if b:
    _dp_tmp_1 = a
else:
    _dp_tmp_1 = c
_dp_tmp_1
"#,
            ),
            (
                r#"
(a + 1) if f() else (b + 2)
"#,
                r#"
_dp_tmp_1 = f()
_dp_tmp_2 = __dp__.add(a, 1)
_dp_tmp_3 = __dp__.add(b, 2)
if _dp_tmp_1:
    _dp_tmp_4 = _dp_tmp_2
else:
    _dp_tmp_4 = _dp_tmp_3
_dp_tmp_4
"#,
            ),
        ];
        for (input, expected) in cases {
            assert_transform_eq(input, expected);
        }
    }

    #[test]
    fn rewrites_getitem() {
        assert_transform_eq("a[b]", "__dp__.getitem(a, b)");
    }

    #[test]
    fn rewrites_delitem() {
        assert_transform_eq("del a[b]", "__dp__.delitem(a, b)");
    }

    #[test]
    fn rewrites_delattr() {
        assert_transform_eq("del a.b", "__dp__.delattr(a, \"b\")");
    }

    #[test]
    fn rewrites_nested_delitem() {
        assert_transform_eq("del a.b[1]", "__dp__.delitem(a.b, 1)");
    }

    #[test]
    fn rewrites_delattr_after_getitem() {
        assert_transform_eq(
            "del a.b[1].c",
            "__dp__.delattr(__dp__.getitem(a.b, 1), \"c\")",
        );
    }

    #[test]
    fn rewrites_multi_delitem_targets() {
        let input = "del a[0], b[0]";
        let expected = r#"
__dp__.delitem(a, 0)
__dp__.delitem(b, 0)
"#;
        assert_transform_eq(input, expected);
    }

    #[test]
    fn rewrites_chain_assignment() {
        let input = "a = b = c";
        let expected = r#"
_dp_tmp_1 = c
a = _dp_tmp_1
b = _dp_tmp_1
"#;
        assert_transform_eq(input, expected);
    }

    #[test]
    fn rewrites_raise_from() {
        assert_transform_eq(
            "raise ValueError from exc",
            "raise __dp__.raise_from(ValueError, exc)",
        );
    }

    #[test]
    fn does_not_rewrite_plain_raise() {
        assert_transform_eq("raise ValueError", "raise ValueError");
    }

    #[test]
    fn rewrites_list_literal() {
        let input = "a = [1, 2, 3]";
        let expected = r#"
a = list((1, 2, 3))
"#;
        assert_transform_eq(input, expected);
    }

    #[test]
    fn rewrites_set_literal() {
        let input = "a = {1, 2, 3}";
        let expected = r#"
a = set((1, 2, 3))
"#;
        assert_transform_eq(input, expected);
    }

    #[test]
    fn rewrites_dict_literal() {
        let input = "a = {'a': 1, 'b': 2}";
        let expected = r#"
a = dict((('a', 1), ('b', 2)))
"#;
        assert_transform_eq(input, expected);
    }

    #[test]
    fn rewrites_slices() {
        let cases = [
            ("a[1:2:3]", "__dp__.getitem(a, slice(1, 2, 3))"),
            ("a[1:2]", "__dp__.getitem(a, slice(1, 2, None))"),
            ("a[:2]", "__dp__.getitem(a, slice(None, 2, None))"),
            ("a[::2]", "__dp__.getitem(a, slice(None, None, 2))"),
            ("a[:]", "__dp__.getitem(a, slice(None, None, None))"),
        ];

        for (input, expected) in cases {
            assert_transform_eq(input, expected);
        }
    }

    #[test]
    fn rewrites_complex_literals() {
        let cases = [
            ("a = 1j", "a = complex(0.0, 1.0)"),
            ("a = 1 + 2j", "a = __dp__.add(1, complex(0.0, 2.0))"),
        ];

        for (input, expected) in cases {
            assert_transform_eq(input, expected);
        }
    }

    #[test]
    fn rewrites_ellipsis() {
        let cases = [("a = ...", "a = Ellipsis"), ("...", "Ellipsis")];

        for (input, expected) in cases {
            assert_transform_eq(input, expected);
        }
    }

    #[test]
    fn rewrites_attribute_access() {
        let cases = [("obj.attr", "obj.attr"), ("foo.bar.baz", "foo.bar.baz")];

        for (input, expected) in cases {
            assert_transform_eq(input, expected);
        }
    }

    #[test]
    fn desugars_tuple_unpacking() {
        let input = "a, b = c";
        let expected = r#"
a = __dp__.getitem(c, 0)
b = __dp__.getitem(c, 1)
"#;
        assert_transform_eq(input, expected);
    }

    #[test]
    fn desugars_tuple_unpacking_with_star() {
        let input = "a, *b = c";
        let expected = r#"
a = __dp__.getitem(c, 0)
b = tuple(__dp__.getitem(c, slice(1, None, None)))
"#;
        assert_transform_eq(input, expected);
    }

    #[test]
    fn desugars_list_unpacking() {
        let input = "[a, b] = c";
        let expected = r#"
a = __dp__.getitem(c, 0)
b = __dp__.getitem(c, 1)
"#;
        assert_transform_eq(input, expected);
    }

    #[test]
    fn desugars_list_unpacking_with_star() {
        let input = "[a, *b] = c";
        let expected = r#"
a = __dp__.getitem(c, 0)
b = list(__dp__.getitem(c, slice(1, None, None)))
"#;
        assert_transform_eq(input, expected);
    }

    #[test]
    fn rewrites_attribute_assignment() {
        let input = "a.b = c";
        let expected = r#"
__dp__.setattr(a, "b", c)
"#;
        assert_transform_eq(input, expected);
    }

    #[test]
    fn rewrites_subscript_assignment() {
        let input = "a[b] = c";
        let expected = r#"
__dp__.setitem(a, b, c)
"#;
        assert_transform_eq(input, expected);
    }

    #[test]
    fn rewrites_chain_assignment_with_subscript() {
        let input = "a[0] = b = 1";
        let expected = r#"
_dp_tmp_1 = 1
__dp__.setitem(a, 0, _dp_tmp_1)
b = _dp_tmp_1
"#;
        assert_transform_eq(input, expected);
    }

    #[test]
    fn rewrites_list_comp() {
        let input = "r = [a + 1 for a in items if a % 2 == 0]";
        let expected = r#"
_dp_tmp_1 = __dp__.mod(a, 2)
_dp_tmp_2 = __dp__.eq(_dp_tmp_1, 0)
_dp_tmp_3 = __dp__.add(a, 1)
def _dp_gen_5(items):
    _dp_iter_6 = __dp__.iter(items)
    while True:
        try:
            a = __dp__.next(_dp_iter_6)
        except:
            __dp__.check_stopiteration()
            break
        else:
            if _dp_tmp_2:
                yield _dp_tmp_3
_dp_tmp_4 = list(_dp_gen_5(__dp__.iter(items)))
r = _dp_tmp_4
"#;
        assert_transform_eq(input, expected);
    }

    #[test]
    fn rewrites_set_comp() {
        let input = "r = {a for a in items}";
        let expected = r#"
def _dp_gen_1(items):
    _dp_iter_2 = __dp__.iter(items)
    while True:
        try:
            a = __dp__.next(_dp_iter_2)
        except:
            __dp__.check_stopiteration()
            break
        else:
            yield a
r = set(_dp_gen_1(__dp__.iter(items)))
"#;
        assert_transform_eq(input, expected);
    }

    #[test]
    fn rewrites_dict_comp() {
        let input = "r = {k: v + 1 for k, v in items if k % 2 == 0}";
        let expected = r#"
_dp_tmp_1 = k, v
_dp_tmp_2 = __dp__.mod(k, 2)
_dp_tmp_3 = __dp__.eq(_dp_tmp_2, 0)
_dp_tmp_4 = __dp__.add(v, 1)
def _dp_gen_6(items):
    _dp_iter_7 = __dp__.iter(items)
    while True:
        try:
            _dp_tmp_1 = __dp__.next(_dp_iter_7)
        except:
            __dp__.check_stopiteration()
            break
        else:
            if _dp_tmp_3:
                yield k, _dp_tmp_4
_dp_tmp_5 = dict(_dp_gen_6(__dp__.iter(items)))
r = _dp_tmp_5
"#;
        assert_transform_eq(input, expected);
    }

    #[test]
    fn rewrites_multi_generator_list_comp() {
        let input = "r = [a * b for a in items for b in items2]";
        let expected = r#"
def _dp_gen_1(items):
    _dp_iter_2 = __dp__.iter(items)
    while True:
        try:
            a = __dp__.next(_dp_iter_2)
        except:
            __dp__.check_stopiteration()
            break
        else:
            _dp_iter_3 = __dp__.iter(items2)
            while True:
                try:
                    b = __dp__.next(_dp_iter_3)
                except:
                    __dp__.check_stopiteration()
                    break
                else:
                    yield __dp__.mul(a, b)
r = list(_dp_gen_1(__dp__.iter(items)))
"#;
        assert_transform_eq(input, expected);
    }

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
