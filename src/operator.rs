use std::cell::Cell;

use ruff_python_ast::visitor::transformer::{walk_expr, walk_stmt, Transformer};
use ruff_python_ast::{self as ast, CmpOp, Expr, Operator, Stmt, UnaryOp};

pub struct OperatorRewriter {
    replaced: Cell<bool>,
}

impl OperatorRewriter {
    pub fn new() -> Self {
        Self {
            replaced: Cell::new(false),
        }
    }

    pub fn transformed(&self) -> bool {
        self.replaced.get()
    }

    fn make_call(func_name: &'static str, args: Vec<Expr>) -> Expr {
        match args.len() {
            1 => {
                let mut iter = args.into_iter();
                let arg = iter.next().unwrap();
                crate::py_expr!(
                    "operator.{func:id}({arg:expr})",
                    arg = arg,
                    func = func_name
                )
            }
            2 => {
                let mut iter = args.into_iter();
                let left = iter.next().unwrap();
                let right = iter.next().unwrap();
                crate::py_expr!(
                    "operator.{func:id}({left:expr}, {right:expr})",
                    left = left,
                    right = right,
                    func = func_name
                )
            }
            _ => unreachable!(),
        }
    }
}

impl Transformer for OperatorRewriter {
    fn visit_expr(&self, expr: &mut Expr) {
        walk_expr(self, expr);
        if let Expr::BinOp(bin) = expr {
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
            *expr = Self::make_call(func_name, vec![left, right]);
            self.replaced.set(true);
        } else if let Expr::UnaryOp(unary) = expr {
            let operand = *unary.operand.clone();

            let func_name = match unary.op {
                UnaryOp::Not => "not_",
                UnaryOp::Invert => "invert",
                UnaryOp::USub => "neg",
                _ => return,
            };
            *expr = Self::make_call(func_name, vec![operand]);
            self.replaced.set(true);
        } else if let Expr::Compare(compare) = expr {
            if compare.ops.len() == 1 && compare.comparators.len() == 1 {
                let left = (*compare.left).clone();
                let right = compare.comparators[0].clone();

                let call = match compare.ops[0] {
                    CmpOp::Eq => Self::make_call("eq", vec![left, right]),
                    CmpOp::NotEq => Self::make_call("ne", vec![left, right]),
                    CmpOp::Lt => Self::make_call("lt", vec![left, right]),
                    CmpOp::Gt => Self::make_call("gt", vec![left, right]),
                    CmpOp::IsNot => Self::make_call("is_not", vec![left, right]),
                    CmpOp::In => Self::make_call("contains", vec![right, left]),
                    CmpOp::NotIn => {
                        let contains = Self::make_call("contains", vec![right, left]);
                        Self::make_call("not_", vec![contains])
                    }
                    _ => return,
                };

                *expr = call;
                self.replaced.set(true);
            }
        } else if let Expr::Subscript(sub) = expr {
            if matches!(sub.ctx, ast::ExprContext::Load) {
                let obj = (*sub.value).clone();
                let key = (*sub.slice).clone();
                *expr = Self::make_call("getitem", vec![obj, key]);
                self.replaced.set(true);
            }
        }
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

            let call = Self::make_call(func_name, vec![target.clone(), value]);

            *stmt = crate::py_stmt!(
                "{target:expr} = {value:expr}",
                target = target,
                value = call,
            );

            self.replaced.set(true);
        } else if let Stmt::Assign(assign) = stmt {
            assert_eq!(
                assign.targets.len(),
                1,
                "expected single assignment target; MultiTargetRewriter must run first"
            );
            if let Expr::Subscript(sub) = &assign.targets[0] {
                let obj = (*sub.value).clone();
                let key = (*sub.slice).clone();
                let value = (*assign.value).clone();
                *stmt = crate::py_stmt!(
                    "operator.setitem({obj:expr}, {key:expr}, {value:expr})",
                    obj = obj,
                    key = key,
                    value = value,
                );
                self.replaced.set(true);
            }
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
                    "operator.delitem({obj:expr}, {key:expr})",
                    obj = obj,
                    key = key,
                );
                self.replaced.set(true);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gen::GeneratorRewriter;
    use crate::multi_target::MultiTargetRewriter;
    use ruff_python_ast::visitor::transformer::walk_body;
    use ruff_python_codegen::{Generator, Stylist};
    use ruff_python_parser::parse_module;

    fn rewrite_source(source: &str) -> String {
        let parsed = parse_module(source).expect("parse error");
        let tokens = parsed.tokens().clone();
        let mut module = parsed.into_syntax();

        let gen_transformer = GeneratorRewriter::new();
        gen_transformer.rewrite_body(&mut module.body);

        let for_transformer = crate::for_loop::ForLoopRewriter::new();
        walk_body(&for_transformer, &mut module.body);

        let multi_transformer = MultiTargetRewriter::new();
        walk_body(&multi_transformer, &mut module.body);

        let op_transformer = OperatorRewriter::new();
        walk_body(&op_transformer, &mut module.body);

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
        let cases = [
            ("a + b", "operator.add(a, b)"),
            ("a - b", "operator.sub(a, b)"),
        ];

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
x = operator.iadd(x, 2)
";
        let output = rewrite_source(input);
        assert_eq!(output.trim(), expected.trim());
    }

    #[test]
    fn rewrites_unary_ops() {
        let cases = [
            ("-a", "operator.neg(a)"),
            ("~b", "operator.invert(b)"),
            ("not c", "operator.not_(c)"),
        ];

        for (input, expected) in cases {
            let output = rewrite_source(input);
            assert_eq!(output.trim_end(), expected);
        }
    }

    #[test]
    fn rewrites_comparisons() {
        let cases = [
            ("a == b", "operator.eq(a, b)"),
            ("a != b", "operator.ne(a, b)"),
            ("a < b", "operator.lt(a, b)"),
            ("a > b", "operator.gt(a, b)"),
            ("a is not b", "operator.is_not(a, b)"),
            ("a in b", "operator.contains(b, a)"),
            ("a not in b", "operator.not_(operator.contains(b, a))"),
        ];

        for (input, expected) in cases {
            let output = rewrite_source(input);
            assert_eq!(output.trim_end(), expected);
        }
    }

    #[test]
    fn rewrites_getitem() {
        let output = rewrite_source("a[b]");
        assert_eq!(output.trim_end(), "operator.getitem(a, b)");
    }

    #[test]
    fn rewrites_setitem() {
        let output = rewrite_source("a[b] = c");
        assert_eq!(output.trim_end(), "operator.setitem(a, b, c)");
    }

    #[test]
    fn rewrites_delitem() {
        let output = rewrite_source("del a[b]");
        assert_eq!(output.trim_end(), "operator.delitem(a, b)");
    }

    #[test]
    fn rewrites_chain_assignment_with_subscript() {
        let output = rewrite_source("a[0] = b = 1");
        let expected = "_dp_tmp_1 = 1\noperator.setitem(a, 0, _dp_tmp_1)\nb = _dp_tmp_1";
        assert_eq!(output.trim(), expected.trim());
    }

    #[test]
    fn rewrites_multi_delitem_targets() {
        let output = rewrite_source("del a[0], b[0]");
        let expected = "operator.delitem(a, 0)\noperator.delitem(b, 0)";
        assert_eq!(output.trim(), expected.trim());
    }
}
