use std::cell::Cell;

use ruff_python_ast::visitor::transformer::{walk_expr, walk_stmt, Transformer};
use ruff_python_ast::{self as ast, CmpOp, Expr, Operator, Stmt, UnaryOp};
use ruff_text_size::TextRange;

fn make_binop(func_name: &'static str, left: Expr, right: Expr) -> Expr {
    crate::py_expr!(
        "_dp_{func:id}({left:expr}, {right:expr})",
        left = left,
        right = right,
        func = func_name
    )
}

fn make_unaryop(func_name: &'static str, operand: Expr) -> Expr {
    crate::py_expr!(
        "_dp_{func:id}({operand:expr})",
        operand = operand,
        func = func_name
    )
}

pub struct ExprRewriter {
    tmp_count: Cell<usize>,
}

impl ExprRewriter {
    pub fn new() -> Self {
        Self {
            tmp_count: Cell::new(0),
        }
    }

    fn next_tmp(&self) -> String {
        let id = self.tmp_count.get() + 1;
        self.tmp_count.set(id);
        format!("_dp_tmp_{}", id)
    }

    fn tuple_from(elts: Vec<Expr>) -> Expr {
        Expr::Tuple(ast::ExprTuple {
            node_index: ast::AtomicNodeIndex::default(),
            range: TextRange::default(),
            elts,
            ctx: ast::ExprContext::Load,
            parenthesized: false,
        })
    }

    fn rewrite_target(&self, target: Expr, value: Expr, out: &mut Vec<Stmt>) {
        match target {
            Expr::Tuple(tuple) => {
                let tmp_name = self.next_tmp();
                let tmp_expr = crate::py_expr!(
                    "
{name:id}
",
                    name = tmp_name.as_str(),
                );
                let mut tmp_stmt = crate::py_stmt!(
                    "
{name:id} = {value:expr}
",
                    name = tmp_name.as_str(),
                    value = value,
                );
                walk_stmt(self, &mut tmp_stmt);
                out.push(tmp_stmt);
                for (i, elt) in tuple.elts.into_iter().enumerate() {
                    let mut elt_stmt = crate::py_stmt!(
                        "
{target:expr} = {tmp:expr}[{idx:literal}]
",
                        target = elt,
                        tmp = tmp_expr.clone(),
                        idx = i,
                    );
                    walk_stmt(self, &mut elt_stmt);
                    out.push(elt_stmt);
                }
            }
            Expr::List(list) => {
                let tmp_name = self.next_tmp();
                let tmp_expr = crate::py_expr!(
                    "
{name:id}
",
                    name = tmp_name.as_str(),
                );
                let mut tmp_stmt = crate::py_stmt!(
                    "
{name:id} = {value:expr}
",
                    name = tmp_name.as_str(),
                    value = value,
                );
                walk_stmt(self, &mut tmp_stmt);
                out.push(tmp_stmt);
                for (i, elt) in list.elts.into_iter().enumerate() {
                    let mut elt_stmt = crate::py_stmt!(
                        "
{target:expr} = {tmp:expr}[{idx:literal}]
",
                        target = elt,
                        tmp = tmp_expr.clone(),
                        idx = i,
                    );
                    walk_stmt(self, &mut elt_stmt);
                    out.push(elt_stmt);
                }
            }
            Expr::Attribute(attr) => {
                let obj = (*attr.value).clone();
                let mut stmt = crate::py_stmt!(
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
                let mut stmt = crate::py_stmt!(
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
                let mut stmt = crate::py_stmt!(
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
}

impl Transformer for ExprRewriter {
    fn visit_expr(&self, expr: &mut Expr) {
        let original = expr.clone();
        *expr = match original {
            Expr::Slice(ast::ExprSlice { lower, upper, step, .. }) => {
                fn none_name() -> Expr {
                    crate::py_expr!("None")
                }
                let lower_expr = lower.map(|expr| *expr).unwrap_or_else(none_name);
                let upper_expr = upper.map(|expr| *expr).unwrap_or_else(none_name);
                let step_expr = step.map(|expr| *expr).unwrap_or_else(none_name);
                crate::py_expr!(
                    "slice({lower:expr}, {upper:expr}, {step:expr})",
                    lower = lower_expr,
                    upper = upper_expr,
                    step = step_expr,
                )
            }
            Expr::EllipsisLiteral(_) => {
                crate::py_expr!("Ellipsis")
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
                crate::py_expr!(
                    "complex({real:expr}, {imag:expr})",
                    real = real_expr,
                    imag = imag_expr,
                )
            }
            Expr::Attribute(ast::ExprAttribute { value, attr, ctx, .. })
                if matches!(ctx, ast::ExprContext::Load)
                    && !matches!(
                        value.as_ref(),
                        Expr::Name(ast::ExprName { id, .. })
                            if id.as_str() == "__dp__"
                                && matches!(
                                    attr.id.as_str(),
                                    "add"
                                        | "sub"
                                        | "mul"
                                        | "matmul"
                                        | "truediv"
                                        | "mod"
                                        | "pow"
                                        | "lshift"
                                        | "rshift"
                                        | "or_"
                                        | "xor"
                                        | "and_"
                                        | "floordiv"
                                        | "eq"
                                        | "ne"
                                        | "lt"
                                        | "le"
                                        | "gt"
                                        | "ge"
                                        | "is_"
                                        | "is_not"
                                        | "contains"
                                        | "neg"
                                        | "invert"
                                        | "not_"
                                        | "pos"
                                        | "iadd"
                                        | "isub"
                                        | "imul"
                                        | "imatmul"
                                        | "itruediv"
                                        | "imod"
                                        | "ipow"
                                        | "ilshift"
                                        | "irshift"
                                        | "ior"
                                        | "ixor"
                                        | "iand"
                                        | "ifloordiv"
                                        | "getitem"
                                        | "delitem"
                                        | "delattr"
                                        | "or_expr"
                                        | "and_expr"
                                        | "if_expr"
                                )
                    ) =>
            {
                let value_expr = *value;
                crate::py_expr!(
                    "getattr({value:expr}, {attr:literal})",
                    value = value_expr,
                    attr = attr.id.as_str(),
                )
            }
            Expr::NoneLiteral(_) => {
                crate::py_expr!("None")
            }
            Expr::List(ast::ExprList { elts, ctx, .. })
                if matches!(ctx, ast::ExprContext::Load) =>
            {
                let tuple = Self::tuple_from(elts);
                crate::py_expr!("list({tuple:expr})", tuple = tuple,)
            }
            Expr::Set(ast::ExprSet { elts, .. }) => {
                let tuple = Self::tuple_from(elts);
                crate::py_expr!("set({tuple:expr})", tuple = tuple,)
            }
            Expr::Dict(ast::ExprDict { items, .. }) if items.iter().all(|item| item.key.is_some()) => {
                let pairs: Vec<Expr> = items
                    .into_iter()
                    .map(|item| {
                        let key = item.key.unwrap();
                        let value = item.value;
                        crate::py_expr!("({key:expr}, {value:expr})", key = key, value = value,)
                    })
                    .collect();
                let tuple = Self::tuple_from(pairs);
                crate::py_expr!("dict({tuple:expr})", tuple = tuple,)
            }
            Expr::If(ast::ExprIf { test, body, orelse, .. }) => {
                let test_expr = *test;
                let body_expr = *body;
                let orelse_expr = *orelse;
                crate::py_expr!(
                    "_dp_if_expr({cond:expr}, lambda: {body:expr}, lambda: {orelse:expr})",
                    cond = test_expr,
                    body = body_expr,
                    orelse = orelse_expr,
                )
            }
            Expr::BoolOp(ast::ExprBoolOp { op, mut values, .. }) => {
                let mut result = values.pop().expect("boolop with no values");
                while let Some(value) = values.pop() {
                    result = match op {
                        ast::BoolOp::Or => crate::py_expr!(
                            "_dp_or_expr({left:expr}, lambda: {right:expr})",
                            left = value,
                            right = result,
                        ),
                        ast::BoolOp::And => crate::py_expr!(
                            "_dp_and_expr({left:expr}, lambda: {right:expr})",
                            left = value,
                            right = result,
                        ),
                    };
                }
                result
            }
            Expr::BinOp(ast::ExprBinOp { left, right, op, .. }) => {
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
            Expr::Compare(ast::ExprCompare { left, ops, comparators, .. })
                if ops.len() == 1 && comparators.len() == 1 =>
            {
                let mut ops_vec = ops.into_vec();
                let mut comps_vec = comparators.into_vec();
                let left = *left;
                let right = comps_vec.pop().unwrap();
                let op = ops_vec.pop().unwrap();
                let call = match op {
                    CmpOp::Eq => make_binop("eq", left, right),
                    CmpOp::NotEq => make_binop("ne", left, right),
                    CmpOp::Lt => make_binop("lt", left, right),
                    CmpOp::LtE => make_binop("le", left, right),
                    CmpOp::Gt => make_binop("gt", left, right),
                    CmpOp::GtE => make_binop("ge", left, right),
                    CmpOp::Is => make_binop("is_", left, right),
                    CmpOp::IsNot => make_binop("is_not", left, right),
                    CmpOp::In => make_binop("contains", right, left),
                    CmpOp::NotIn => {
                        let contains = make_binop("contains", right, left);
                        make_unaryop("not_", contains)
                    }
                };
                call
            }
            Expr::Subscript(ast::ExprSubscript { value, slice, ctx, .. })
                if matches!(ctx, ast::ExprContext::Load) =>
            {
                let obj = *value;
                let key = *slice;
                make_binop("getitem", obj, key)
            }
            _ => original,
        };
        walk_expr(self, expr);
    }

    fn visit_stmt(&self, stmt: &mut Stmt) {
        walk_stmt(self, stmt);

        match stmt {
            Stmt::Assign(assign) => {
                let value = (*assign.value).clone();
                let mut stmts = Vec::new();
                if assign.targets.len() > 1 {
                    let tmp_name = self.next_tmp();
                    let tmp_expr = crate::py_expr!(
                        "
{name:id}
",
                        name = tmp_name.as_str(),
                    );
                    let mut tmp_stmt = crate::py_stmt!(
                        "
{name:id} = {value:expr}
",
                        name = tmp_name.as_str(),
                        value = value,
                    );
                    walk_stmt(self, &mut tmp_stmt);
                    stmts.push(tmp_stmt);
                    for target in &assign.targets {
                        self.rewrite_target(target.clone(), tmp_expr.clone(), &mut stmts);
                    }
                } else {
                    self.rewrite_target(assign.targets[0].clone(), value, &mut stmts);
                }
                if stmts.len() == 1 {
                    *stmt = stmts.pop().unwrap();
                } else {
                    *stmt = crate::py_stmt!(
                        "
{body:stmt}
",
                        body = stmts,
                    );
                }
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

                let call = make_binop(func_name, target.clone(), value);

                *stmt = crate::py_stmt!(
                    "{target:expr} = {value:expr}",
                    target = target,
                    value = call,
                );
            }
            Stmt::Delete(del) => {
                let mut stmts = Vec::with_capacity(del.targets.len());
                for target in &del.targets {
                    if let Expr::Subscript(sub) = target {
                        let obj = (*sub.value).clone();
                        let key = (*sub.slice).clone();
                        stmts.push(crate::py_stmt!(
                            "_dp_delitem({obj:expr}, {key:expr})",
                            obj = obj,
                            key = key,
                        ));
                    } else if let Expr::Attribute(attr) = target {
                        let obj = (*attr.value).clone();
                        stmts.push(crate::py_stmt!(
                            "_dp_delattr({obj:expr}, {name:literal})",
                            obj = obj,
                            name = attr.attr.as_str(),
                        ));
                    } else {
                        stmts.push(crate::py_stmt!(
                            "del {target:expr}",
                            target = target.clone(),
                        ));
                    }
                }
                if stmts.len() == 1 {
                    *stmt = stmts.pop().unwrap();
                } else {
                    *stmt = crate::py_stmt!("{body:stmt}", body = stmts);
                }
            }
            Stmt::Raise(ast::StmtRaise { exc: Some(exc), cause: Some(cause), .. }) => {
                let exc_expr = *exc.clone();
                let cause_expr = *cause.clone();
                *stmt = crate::py_stmt!(
                    "raise __dp__.raise_from({exc:expr}, {cause:expr})",
                    exc = exc_expr,
                    cause = cause_expr,
                );
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transform::gen::GeneratorRewriter;
    use ruff_python_ast::visitor::transformer::walk_body;
    use ruff_python_codegen::{Generator, Stylist};
    use ruff_python_parser::parse_module;

    fn rewrite_source(source: &str) -> String {
        let parsed = parse_module(source).expect("parse error");
        let tokens = parsed.tokens().clone();
        let mut module = parsed.into_syntax();

        let gen_transformer = GeneratorRewriter::new();
        gen_transformer.rewrite_body(&mut module.body);

        let for_transformer = crate::transform::for_loop::ForLoopRewriter::new();
        walk_body(&for_transformer, &mut module.body);

        let expr_transformer = ExprRewriter::new();
        walk_body(&expr_transformer, &mut module.body);

        crate::template::flatten(&mut module.body);

        let stylist = Stylist::from_tokens(&tokens, source);
        let mut output = String::new();
        for stmt in &module.body {
            let snippet = Generator::from(&stylist).stmt(stmt);
            output.push_str(&snippet);
            output.push_str(stylist.line_ending().as_str());
        }
        output
    }

    #[test]
    fn rewrites_binary_ops() {
        let cases = [("a + b", "_dp_add(a, b)"), ("a - b", "_dp_sub(a, b)")];

        for (input, expected) in cases {
            let output = rewrite_source(input);
            assert_eq!(output.trim_end(), expected);
        }
    }

    #[test]
    fn rewrites_aug_assign() {
        let input = "
x = 1
x += 2
";
        let expected = "
x = 1
x = _dp_iadd(x, 2)
";
        let output = rewrite_source(input);
        assert_eq!(output.trim(), expected.trim());
    }

    #[test]
    fn rewrites_unary_ops() {
        let cases = [
            (
                "
-a
",
                "
_dp_neg(a)
",
            ),
            (
                "
~b
",
                "
_dp_invert(b)
",
            ),
            (
                "
not c
",
                "
_dp_not_(c)
",
            ),
            (
                "
+a
",
                "
_dp_pos(a)
",
            ),
        ];

        for (input, expected) in cases {
            let output = rewrite_source(input);
            assert_eq!(output.trim(), expected.trim());
        }
    }

    #[test]
    fn rewrites_bool_ops() {
        let cases = [
            (
                r#"
a or b
"#,
                r#"
_dp_or_expr(a, lambda: b)
"#,
            ),
            (
                r#"
a and b
"#,
                r#"
_dp_and_expr(a, lambda: b)
"#,
            ),
            (
                r#"
f() or a
"#,
                r#"
_dp_or_expr(f(), lambda: a)
"#,
            ),
            (
                r#"
f() and a
"#,
                r#"
_dp_and_expr(f(), lambda: a)
"#,
            ),
        ];

        for (input, expected) in cases {
            let output = rewrite_source(input);
            assert_eq!(output.trim(), expected.trim());
        }
    }

    #[test]
    fn rewrites_multi_bool_ops() {
        let output = rewrite_source(
            r#"
a or b or c
"#,
        );
        assert_eq!(
            output.trim(),
            r#"
_dp_or_expr(a, lambda: _dp_or_expr(b, lambda: c))
"#
            .trim(),
        );

        let output = rewrite_source(
            r#"
a and b and c
"#,
        );
        assert_eq!(
            output.trim(),
            r#"
_dp_and_expr(a, lambda: _dp_and_expr(b, lambda: c))
"#
            .trim(),
        );
    }

    #[test]
    fn rewrites_comparisons() {
        let cases = [
            ("a == b", "_dp_eq(a, b)"),
            ("a != b", "_dp_ne(a, b)"),
            ("a < b", "_dp_lt(a, b)"),
            ("a <= b", "_dp_le(a, b)"),
            ("a > b", "_dp_gt(a, b)"),
            ("a >= b", "_dp_ge(a, b)"),
            ("a is b", "_dp_is_(a, b)"),
            ("a is not b", "_dp_is_not(a, b)"),
            ("a in b", "_dp_contains(b, a)"),
            ("a not in b", "_dp_not_(_dp_contains(b, a))"),
        ];

        for (input, expected) in cases {
            let output = rewrite_source(input);
            assert_eq!(output.trim_end(), expected);
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
_dp_if_expr(b, lambda: a, lambda: c)
"#,
            ),
            (
                r#"
(a + 1) if f() else (b + 2)
"#,
                r#"
_dp_if_expr(f(), lambda: _dp_add(a, 1), lambda: _dp_add(b, 2))
"#,
            ),
        ];
        for (input, expected) in cases {
            let output = rewrite_source(input);
            assert_eq!(output.trim(), expected.trim());
        }
    }

    #[test]
    fn rewrites_getitem() {
        let output = rewrite_source("a[b]");
        assert_eq!(output.trim_end(), "_dp_getitem(a, b)");
    }

    #[test]
    fn rewrites_delitem() {
        let output = rewrite_source("del a[b]");
        assert_eq!(output.trim_end(), "_dp_delitem(a, b)");
    }

    #[test]
    fn rewrites_delattr() {
        let output = rewrite_source("del a.b");
        assert_eq!(output.trim_end(), "_dp_delattr(a, \"b\")");
    }

    #[test]
    fn rewrites_nested_delitem() {
        let output = rewrite_source("del a.b[1]");
        assert_eq!(output.trim_end(), "_dp_delitem(getattr(a, \"b\"), 1)");
    }

    #[test]
    fn rewrites_delattr_after_getitem() {
        let output = rewrite_source("del a.b[1].c");
        assert_eq!(
            output.trim_end(),
            "_dp_delattr(_dp_getitem(getattr(a, \"b\"), 1), \"c\")"
        );
    }

    #[test]
    fn rewrites_multi_delitem_targets() {
        let output = rewrite_source("del a[0], b[0]");
        let expected = "_dp_delitem(a, 0)\n_dp_delitem(b, 0)";
        assert_eq!(output.trim(), expected.trim());
    }

    #[test]
    fn rewrites_chain_assignment() {
        let output = rewrite_source(
            r#"
a = b = c
"#,
        );
        let expected = r#"
_dp_tmp_1 = c
a = _dp_tmp_1
b = _dp_tmp_1
"#;
        assert_eq!(output.trim(), expected.trim());
    }

    #[test]
    fn rewrites_raise_from() {
        let output = rewrite_source("raise ValueError from exc");
        assert_eq!(
            output.trim_end(),
            "raise __dp__.raise_from(ValueError, exc)",
        );
    }

    #[test]
    fn does_not_rewrite_plain_raise() {
        let output = rewrite_source("raise ValueError");
        assert_eq!(output.trim_end(), "raise ValueError");
    }

    #[test]
    fn rewrites_list_literal() {
        let input = r#"
a = [1, 2, 3]
"#;
        let expected = r#"
a = list((1, 2, 3))
"#;
        let output = rewrite_source(input);
        assert_eq!(output.trim(), expected.trim());
    }

    #[test]
    fn rewrites_set_literal() {
        let input = r#"
a = {1, 2, 3}
"#;
        let expected = r#"
a = set((1, 2, 3))
"#;
        let output = rewrite_source(input);
        assert_eq!(output.trim(), expected.trim());
    }

    #[test]
    fn rewrites_dict_literal() {
        let input = r#"
a = {'a': 1, 'b': 2}
"#;
        let expected = r#"
a = dict((('a', 1), ('b', 2)))
"#;
        let output = rewrite_source(input);
        assert_eq!(output.trim(), expected.trim());
    }

    #[test]
    fn rewrites_slices() {
        let cases = [
            ("a[1:2:3]", "_dp_getitem(a, slice(1, 2, 3))"),
            ("a[1:2]", "_dp_getitem(a, slice(1, 2, None))"),
            ("a[:2]", "_dp_getitem(a, slice(None, 2, None))"),
            ("a[::2]", "_dp_getitem(a, slice(None, None, 2))"),
            ("a[:]", "_dp_getitem(a, slice(None, None, None))"),
        ];

        for (input, expected) in cases {
            let output = rewrite_source(input);
            assert_eq!(output.trim_end(), expected);
        }
    }

    #[test]
    fn rewrites_complex_literals() {
        let cases = [
            ("a = 1j", "a = complex(0.0, 1.0)"),
            ("a = 1 + 2j", "a = _dp_add(1, complex(0.0, 2.0))"),
        ];

        for (input, expected) in cases {
            let output = rewrite_source(input);
            assert_eq!(output.trim_end(), expected);
        }
    }

    #[test]
    fn rewrites_ellipsis() {
        let cases = [("a = ...", "a = Ellipsis"), ("...", "Ellipsis")];

        for (input, expected) in cases {
            let output = rewrite_source(input);
            assert_eq!(output.trim_end(), expected);
        }
    }

    #[test]
    fn rewrites_attribute_access() {
        let cases = [
            ("obj.attr", "getattr(obj, \"attr\")"),
            ("foo.bar.baz", "getattr(getattr(foo, \"bar\"), \"baz\")"),
        ];

        for (input, expected) in cases {
            let output = rewrite_source(input);
            assert_eq!(output.trim_end(), expected);
        }
    }

    #[test]
    fn desugars_tuple_unpacking() {
        let output = rewrite_source(
            r#"
a, b = c
"#,
        );
        let expected = r#"
_dp_tmp_1 = c
a = _dp_getitem(_dp_tmp_1, 0)
b = _dp_getitem(_dp_tmp_1, 1)
"#;
        assert_eq!(output.trim(), expected.trim());
    }

    #[test]
    fn desugars_list_unpacking() {
        let output = rewrite_source(
            r#"
[a, b] = c
"#,
        );
        let expected = r#"
_dp_tmp_1 = c
a = _dp_getitem(_dp_tmp_1, 0)
b = _dp_getitem(_dp_tmp_1, 1)
"#;
        assert_eq!(output.trim(), expected.trim());
    }

    #[test]
    fn rewrites_attribute_assignment() {
        let output = rewrite_source(
            r#"
a.b = c
"#,
        );
        let expected = r#"
getattr(__dp__, "setattr")(a, "b", c)
"#;
        assert_eq!(output.trim(), expected.trim());
    }

    #[test]
    fn rewrites_subscript_assignment() {
        let output = rewrite_source(
            r#"
a[b] = c
"#,
        );
        let expected = r#"
getattr(__dp__, "setitem")(a, b, c)
"#;
        assert_eq!(output.trim(), expected.trim());
    }

    #[test]
    fn rewrites_chain_assignment_with_subscript() {
        let output = rewrite_source(
            r#"
a[0] = b = 1
"#,
        );
        let expected = r#"
_dp_tmp_1 = 1
getattr(__dp__, "setitem")(a, 0, _dp_tmp_1)
b = _dp_tmp_1
"#;
        assert_eq!(output.trim(), expected.trim());
    }
}
