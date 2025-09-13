use std::cell::Cell;

use ruff_python_ast::visitor::transformer::{walk_expr, walk_stmt, Transformer};
use ruff_python_ast::{self as ast, CmpOp, Expr, Operator, Stmt, UnaryOp};
use ruff_text_size::TextRange;

fn make_binop(func_name: &'static str, left: Expr, right: Expr) -> Expr {
    crate::py_expr!(
        "__dp__.{func:id}({left:expr}, {right:expr})",
        left = left,
        right = right,
        func = func_name
    )
}

fn make_unaryop(func_name: &'static str, operand: Expr) -> Expr {
    crate::py_expr!(
        "__dp__.{func:id}({operand:expr})",
        operand = operand,
        func = func_name
    )
}

pub struct ExprRewriter {
    replaced: Cell<bool>,
}

impl ExprRewriter {
    pub fn new() -> Self {
        Self {
            replaced: Cell::new(false),
        }
    }

    pub fn transformed(&self) -> bool {
        self.replaced.get()
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
}

impl Transformer for ExprRewriter {
        fn visit_expr(&self, expr: &mut Expr) {
        walk_expr(self, expr);
        *expr = match expr {
            Expr::Slice(ast::ExprSlice { lower, upper, step, .. }) => {
                fn none_name() -> Expr {
                    crate::py_expr!("\nNone")
                }
                let lower_expr = lower
                    .as_ref()
                    .map(|expr| *expr.clone())
                    .unwrap_or_else(none_name);
                let upper_expr = upper
                    .as_ref()
                    .map(|expr| *expr.clone())
                    .unwrap_or_else(none_name);
                let step_expr = step
                    .as_ref()
                    .map(|expr| *expr.clone())
                    .unwrap_or_else(none_name);
                self.replaced.set(true);
                crate::py_expr!(
                    "\nslice({lower:expr}, {upper:expr}, {step:expr})",
                    lower = lower_expr,
                    upper = upper_expr,
                    step = step_expr,
                )
            }
            Expr::EllipsisLiteral(_) => {
                self.replaced.set(true);
                crate::py_expr!("\nEllipsis")
            }
            Expr::NumberLiteral(ast::ExprNumberLiteral { value: ast::Number::Complex { real, imag }, .. }) => {
                let real_expr = Expr::NumberLiteral(ast::ExprNumberLiteral {
                    node_index: ast::AtomicNodeIndex::default(),
                    range: TextRange::default(),
                    value: ast::Number::Float(*real),
                });
                let imag_expr = Expr::NumberLiteral(ast::ExprNumberLiteral {
                    node_index: ast::AtomicNodeIndex::default(),
                    range: TextRange::default(),
                    value: ast::Number::Float(*imag),
                });
                self.replaced.set(true);
                crate::py_expr!(
                    "\ncomplex({real:expr}, {imag:expr})",
                    real = real_expr,
                    imag = imag_expr,
                )
            }
            Expr::Attribute(ast::ExprAttribute { value, attr, ctx, .. }) if matches!(ctx, ast::ExprContext::Load) => {
                let value_expr = *value.clone();
                self.replaced.set(true);
                crate::py_expr!(
                    "\ngetattr({value:expr}, {attr:literal})",
                    value = value_expr,
                    attr = attr.id.as_str(),
                )
            }
            Expr::NoneLiteral(_) => {
                self.replaced.set(true);
                crate::py_expr!("\nNone")
            }
            Expr::List(ast::ExprList { elts, .. }) => {
                let tuple = Self::tuple_from(elts.clone());
                self.replaced.set(true);
                crate::py_expr!("\nlist({tuple:expr})", tuple = tuple,)
            }
            Expr::Set(ast::ExprSet { elts, .. }) => {
                let tuple = Self::tuple_from(elts.clone());
                self.replaced.set(true);
                crate::py_expr!("\nset({tuple:expr})", tuple = tuple,)
            }
            Expr::Dict(ast::ExprDict { items, .. }) if items.iter().all(|item| item.key.is_some()) => {
                let pairs: Vec<Expr> = items
                    .iter()
                    .map(|item| {
                        let key = item.key.clone().unwrap();
                        let value = item.value.clone();
                        crate::py_expr!("\n({key:expr}, {value:expr})", key = key, value = value,)
                    })
                    .collect();
                let tuple = Self::tuple_from(pairs);
                self.replaced.set(true);
                crate::py_expr!("\ndict({tuple:expr})", tuple = tuple,)
            }
            Expr::If(ast::ExprIf { test, body, orelse, .. }) => {
                let test_expr = *test.clone();
                let body_expr = *body.clone();
                let orelse_expr = *orelse.clone();
                self.replaced.set(true);
                crate::py_expr!(
                    "\n__dp__.if_expr({cond:expr}, lambda: {body:expr}, lambda: {orelse:expr})",
                    cond = test_expr,
                    body = body_expr,
                    orelse = orelse_expr,
                )
            }
            Expr::BoolOp(ast::ExprBoolOp { op, values, .. }) => {
                let mut vals = std::mem::take(values);
                let mut result = vals.pop().expect("boolop with no values");
                while let Some(value) = vals.pop() {
                    result = match op {
                        ast::BoolOp::Or => crate::py_expr!(
                            "\n__dp__.or_expr({left:expr}, lambda: {right:expr})",
                            left = value,
                            right = result,
                        ),
                        ast::BoolOp::And => crate::py_expr!(
                            "\n__dp__.and_expr({left:expr}, lambda: {right:expr})",
                            left = value,
                            right = result,
                        ),
                    };
                }
                self.replaced.set(true);
                result
            }
            Expr::BinOp(bin) => {
                let left = *bin.left.clone();
                let right = *bin.right.clone();
                let func_name = match bin.op {
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
                self.replaced.set(true);
                make_binop(func_name, left, right)
            }
            Expr::UnaryOp(unary) => {
                let operand = *unary.operand.clone();
                let func_name = match unary.op {
                    UnaryOp::Not => "not_",
                    UnaryOp::Invert => "invert",
                    UnaryOp::USub => "neg",
                    UnaryOp::UAdd => "pos",
                };
                self.replaced.set(true);
                make_unaryop(func_name, operand)
            }
            Expr::Compare(compare) if compare.ops.len() == 1 && compare.comparators.len() == 1 => {
                let left = (*compare.left).clone();
                let right = compare.comparators[0].clone();
                let call = match compare.ops[0] {
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
                self.replaced.set(true);
                call
            }
            Expr::Subscript(sub) if matches!(sub.ctx, ast::ExprContext::Load) => {
                let obj = (*sub.value).clone();
                let key = (*sub.slice).clone();
                self.replaced.set(true);
                make_binop("getitem", obj, key)
            }
            _ => return,
        };
    }


    fn visit_stmt(&self, stmt: &mut Stmt) {
        walk_stmt(self, stmt);

        if let Stmt::AugAssign(aug) = stmt {
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

            self.replaced.set(true);
        } else if let Stmt::Delete(del) = stmt {
            assert_eq!(
                del.targets.len(),
                1,
                "expected single delete target; MultiTargetRewriter must run first"
            );
            if let Expr::Subscript(sub) = &del.targets[0] {
                let obj = (*sub.value).clone();
                let key = (*sub.slice).clone();
                *stmt = crate::py_stmt!(
                    "__dp__.delitem({obj:expr}, {key:expr})",
                    obj = obj,
                    key = key,
                );
                self.replaced.set(true);
            } else if let Expr::Attribute(attr) = &del.targets[0] {
                let obj = (*attr.value).clone();
                *stmt = crate::py_stmt!(
                    "__dp__.delattr({obj:expr}, {name:literal})",
                    obj = obj,
                    name = attr.attr.as_str(),
                );
                self.replaced.set(true);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transform::gen::GeneratorRewriter;
    use crate::transform::multi_target::MultiTargetRewriter;
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

        let multi_transformer = MultiTargetRewriter::new();
        walk_body(&multi_transformer, &mut module.body);

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
        let cases = [("a + b", "__dp__.add(a, b)"), ("a - b", "__dp__.sub(a, b)")];

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
x = __dp__.iadd(x, 2)
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
__dp__.neg(a)
",
            ),
            (
                "
~b
",
                "
__dp__.invert(b)
",
            ),
            (
                "
not c
",
                "
__dp__.not_(c)
",
            ),
            (
                "
+a
",
                "
__dp__.pos(a)
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
__dp__.or_expr(a, lambda: b)
"#,
            ),
            (
                r#"
a and b
"#,
                r#"
__dp__.and_expr(a, lambda: b)
"#,
            ),
            (
                r#"
f() or a
"#,
                r#"
__dp__.or_expr(f(), lambda: a)
"#,
            ),
            (
                r#"
f() and a
"#,
                r#"
__dp__.and_expr(f(), lambda: a)
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
__dp__.or_expr(a, lambda: __dp__.or_expr(b, lambda: c))
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
__dp__.and_expr(a, lambda: __dp__.and_expr(b, lambda: c))
"#
            .trim(),
        );
    }

    #[test]
    fn rewrites_comparisons() {
        let cases = [
            ("a == b", "__dp__.eq(a, b)"),
            ("a != b", "__dp__.ne(a, b)"),
            ("a < b", "__dp__.lt(a, b)"),
            ("a <= b", "__dp__.le(a, b)"),
            ("a > b", "__dp__.gt(a, b)"),
            ("a >= b", "__dp__.ge(a, b)"),
            ("a is b", "__dp__.is_(a, b)"),
            ("a is not b", "__dp__.is_not(a, b)"),
            ("a in b", "__dp__.contains(b, a)"),
            ("a not in b", "__dp__.not_(__dp__.contains(b, a))"),
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
__dp__.if_expr(b, lambda: a, lambda: c)
"#,
            ),
            (
                r#"
(a + 1) if f() else (b + 2)
"#,
                r#"
__dp__.if_expr(f(), lambda: __dp__.add(a, 1), lambda: __dp__.add(b, 2))
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
        assert_eq!(output.trim_end(), "__dp__.getitem(a, b)");
    }

    #[test]
    fn rewrites_delitem() {
        let output = rewrite_source("del a[b]");
        assert_eq!(output.trim_end(), "__dp__.delitem(a, b)");
    }

    #[test]
    fn rewrites_delattr() {
        let output = rewrite_source("del a.b");
        assert_eq!(output.trim_end(), "__dp__.delattr(a, \"b\")");
    }

    #[test]
    fn rewrites_nested_delitem() {
        let output = rewrite_source("del a.b[1]");
        assert_eq!(
            output.trim_end(),
            "__dp__.delitem(getattr(a, \"b\"), 1)"
        );
    }

    #[test]
    fn rewrites_delattr_after_getitem() {
        let output = rewrite_source("del a.b[1].c");
        assert_eq!(
            output.trim_end(),
            "__dp__.delattr(__dp__.getitem(getattr(a, \"b\"), 1), \"c\")"
        );
    }

    #[test]
    fn rewrites_multi_delitem_targets() {
        let output = rewrite_source("del a[0], b[0]");
        let expected = "__dp__.delitem(a, 0)\n__dp__.delitem(b, 0)";
        assert_eq!(output.trim(), expected.trim());
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
            ("a[1:2:3]", "__dp__.getitem(a, slice(1, 2, 3))"),
            ("a[1:2]", "__dp__.getitem(a, slice(1, 2, None))"),
            ("a[:2]", "__dp__.getitem(a, slice(None, 2, None))"),
            ("a[::2]", "__dp__.getitem(a, slice(None, None, 2))"),
            ("a[:]", "__dp__.getitem(a, slice(None, None, None))"),
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
            (
                "a = 1 + 2j",
                "a = __dp__.add(1, complex(0.0, 2.0))",
            ),
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
            (
                "foo.bar.baz",
                "getattr(getattr(foo, \"bar\"), \"baz\")",
            ),
        ];

        for (input, expected) in cases {
            let output = rewrite_source(input);
            assert_eq!(output.trim_end(), expected);
        }
    }
}
