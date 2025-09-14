use std::cell::Cell;

use super::{
    rewrite_assert, rewrite_class_def, rewrite_for_loop, rewrite_match_case, rewrite_try_except,
    rewrite_with,
};
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
    tmp_count: Cell<usize>,
    for_count: Cell<usize>,
    try_count: Cell<usize>,
    match_count: Cell<usize>,
    with_count: Cell<usize>,
}

impl ExprRewriter {
    pub fn new() -> Self {
        Self {
            tmp_count: Cell::new(0),
            for_count: Cell::new(0),
            try_count: Cell::new(0),
            match_count: Cell::new(0),
            with_count: Cell::new(0),
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

    fn make_comp_call(func: &str, elt: Expr, generators: Vec<ast::Comprehension>) -> Expr {
        let gen_expr = Expr::Generator(ast::ExprGenerator {
            node_index: ast::AtomicNodeIndex::default(),
            range: TextRange::default(),
            elt: Box::new(elt),
            generators,
            parenthesized: false,
        });

        crate::py_expr!("{func:id}({gen:expr})", func = func, gen = gen_expr)
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
            Expr::Slice(ast::ExprSlice {
                lower, upper, step, ..
            }) => {
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
            Expr::Attribute(ast::ExprAttribute {
                value, attr, ctx, ..
            }) if matches!(ctx, ast::ExprContext::Load) => {
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
            Expr::ListComp(ast::ExprListComp {
                elt, generators, ..
            }) => Self::make_comp_call("list", *elt, generators),
            Expr::SetComp(ast::ExprSetComp {
                elt, generators, ..
            }) => Self::make_comp_call("set", *elt, generators),
            Expr::DictComp(ast::ExprDictComp {
                key,
                value,
                generators,
                ..
            }) => {
                let tuple =
                    crate::py_expr!("({key:expr}, {value:expr})", key = *key, value = *value,);
                Self::make_comp_call("dict", tuple, generators)
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
            Expr::Dict(ast::ExprDict { items, .. })
                if items.iter().all(|item| item.key.is_some()) =>
            {
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
            Expr::If(ast::ExprIf {
                test, body, orelse, ..
            }) => {
                let test_expr = *test;
                let body_expr = *body;
                let orelse_expr = *orelse;
                crate::py_expr!(
                    "__dp__.if_expr({cond:expr}, lambda: {body:expr}, lambda: {orelse:expr})",
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
                            "__dp__.or_expr({left:expr}, lambda: {right:expr})",
                            left = value,
                            right = result,
                        ),
                        ast::BoolOp::And => crate::py_expr!(
                            "__dp__.and_expr({left:expr}, lambda: {right:expr})",
                            left = value,
                            right = result,
                        ),
                    };
                }
                result
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
            Expr::Compare(ast::ExprCompare {
                left,
                ops,
                comparators,
                ..
            }) if ops.len() == 1 && comparators.len() == 1 => {
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
        *stmt = match stmt {
            Stmt::With(with) => rewrite_with::rewrite(with.clone(), &self.with_count),
            Stmt::Assert(assert) => rewrite_assert::rewrite(assert.clone()),
            Stmt::ClassDef(class_def) => rewrite_class_def::rewrite(class_def.clone()),
            Stmt::For(for_stmt) => rewrite_for_loop::rewrite(for_stmt.clone(), &self.for_count),
            Stmt::Try(try_stmt) => rewrite_try_except::rewrite(try_stmt.clone(), &self.try_count),
            Stmt::Match(match_stmt) => {
                rewrite_match_case::rewrite(match_stmt.clone(), &self.match_count)
            }
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
                    stmts.pop().unwrap()
                } else {
                    crate::py_stmt!(
                        "
{body:stmt}
",
                        body = stmts,
                    )
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

                crate::py_stmt!(
                    "{target:expr} = {value:expr}",
                    target = target,
                    value = call,
                )
            }
            Stmt::Delete(del) => {
                let mut stmts = Vec::with_capacity(del.targets.len());
                for target in &del.targets {
                    if let Expr::Subscript(sub) = target {
                        let obj = (*sub.value).clone();
                        let key = (*sub.slice).clone();
                        let mut new_stmt = crate::py_stmt!(
                            "__dp__.delitem({obj:expr}, {key:expr})",
                            obj = obj,
                            key = key,
                        );
                        walk_stmt(self, &mut new_stmt);
                        stmts.push(new_stmt);
                    } else if let Expr::Attribute(attr) = target {
                        let obj = (*attr.value).clone();
                        let mut new_stmt = crate::py_stmt!(
                            "__dp__.delattr({obj:expr}, {name:literal})",
                            obj = obj,
                            name = attr.attr.as_str(),
                        );
                        walk_stmt(self, &mut new_stmt);
                        stmts.push(new_stmt);
                    } else {
                        let mut new_stmt =
                            crate::py_stmt!("del {target:expr}", target = target.clone(),);
                        walk_stmt(self, &mut new_stmt);
                        stmts.push(new_stmt);
                    }
                }
                if stmts.len() == 1 {
                    stmts.pop().unwrap()
                } else {
                    crate::py_stmt!("{body:stmt}", body = stmts)
                }
            }
            Stmt::Raise(ast::StmtRaise {
                exc: Some(exc),
                cause: Some(cause),
                ..
            }) => {
                let exc_expr = *exc.clone();
                let cause_expr = *cause.clone();
                crate::py_stmt!(
                    "raise __dp__.raise_from({exc:expr}, {cause:expr})",
                    exc = exc_expr,
                    cause = cause_expr,
                )
            }
            _ => stmt.clone(),
        };

        walk_stmt(self, stmt);
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

        let expr_transformer = ExprRewriter::new();
        walk_body(&expr_transformer, &mut module.body);
        let gen_transformer = GeneratorRewriter::new();
        gen_transformer.rewrite_body(&mut module.body);
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
        let cases = [
            (r#"a + b"#, r#"getattr(__dp__, "add")(a, b)"#),
            (r#"a - b"#, r#"getattr(__dp__, "sub")(a, b)"#),
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
        let expected = r#"
x = 1
x = getattr(__dp__, "iadd")(x, 2)
"#;
        let output = rewrite_source(input);
        assert_eq!(output.trim(), expected.trim());
    }

    #[test]
    fn rewrites_unary_ops() {
        let cases = [
            (
                r#"
-a
"#,
                r#"
getattr(__dp__, "neg")(a)
"#,
            ),
            (
                r#"
~b
"#,
                r#"
getattr(__dp__, "invert")(b)
"#,
            ),
            (
                r#"
not c
"#,
                r#"
getattr(__dp__, "not_")(c)
"#,
            ),
            (
                r#"
+a
"#,
                r#"
getattr(__dp__, "pos")(a)
"#,
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
getattr(__dp__, "or_expr")(a, lambda: b)
"#,
            ),
            (
                r#"
a and b
"#,
                r#"
getattr(__dp__, "and_expr")(a, lambda: b)
"#,
            ),
            (
                r#"
f() or a
"#,
                r#"
getattr(__dp__, "or_expr")(f(), lambda: a)
"#,
            ),
            (
                r#"
f() and a
"#,
                r#"
getattr(__dp__, "and_expr")(f(), lambda: a)
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
getattr(__dp__, "or_expr")(a, lambda: getattr(__dp__, "or_expr")(b, lambda: c))
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
getattr(__dp__, "and_expr")(a, lambda: getattr(__dp__, "and_expr")(b, lambda: c))
"#
            .trim(),
        );
    }

    #[test]
    fn rewrites_comparisons() {
        let cases = [
            (r#"a == b"#, r#"getattr(__dp__, "eq")(a, b)"#),
            (r#"a != b"#, r#"getattr(__dp__, "ne")(a, b)"#),
            (r#"a < b"#, r#"getattr(__dp__, "lt")(a, b)"#),
            (r#"a <= b"#, r#"getattr(__dp__, "le")(a, b)"#),
            (r#"a > b"#, r#"getattr(__dp__, "gt")(a, b)"#),
            (r#"a >= b"#, r#"getattr(__dp__, "ge")(a, b)"#),
            (r#"a is b"#, r#"getattr(__dp__, "is_")(a, b)"#),
            (r#"a is not b"#, r#"getattr(__dp__, "is_not")(a, b)"#),
            (r#"a in b"#, r#"getattr(__dp__, "contains")(b, a)"#),
            (
                r#"a not in b"#,
                r#"getattr(__dp__, "not_")(getattr(__dp__, "contains")(b, a))"#,
            ),
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
getattr(__dp__, "if_expr")(b, lambda: a, lambda: c)
"#,
            ),
            (
                r#"
(a + 1) if f() else (b + 2)
"#,
                r#"
getattr(__dp__, "if_expr")(f(), lambda: getattr(__dp__, "add")(a, 1), lambda: getattr(__dp__, "add")(b, 2))
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
        assert_eq!(output.trim_end(), r#"getattr(__dp__, "getitem")(a, b)"#);
    }

    #[test]
    fn rewrites_delitem() {
        let output = rewrite_source("del a[b]");
        assert_eq!(output.trim_end(), r#"getattr(__dp__, "delitem")(a, b)"#);
    }

    #[test]
    fn rewrites_delattr() {
        let output = rewrite_source("del a.b");
        assert_eq!(output.trim_end(), r#"getattr(__dp__, "delattr")(a, "b")"#);
    }

    #[test]
    fn rewrites_nested_delitem() {
        let output = rewrite_source("del a.b[1]");
        assert_eq!(
            output.trim_end(),
            r#"getattr(__dp__, "delitem")(getattr(a, "b"), 1)"#
        );
    }

    #[test]
    fn rewrites_delattr_after_getitem() {
        let output = rewrite_source("del a.b[1].c");
        assert_eq!(
            output.trim_end(),
            r#"getattr(__dp__, "delattr")(getattr(__dp__, "getitem")(getattr(a, "b"), 1), "c")"#
        );
    }

    #[test]
    fn rewrites_multi_delitem_targets() {
        let output = rewrite_source("del a[0], b[0]");
        let expected = r#"getattr(__dp__, "delitem")(a, 0)
getattr(__dp__, "delitem")(b, 0)"#;
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
            r#"raise getattr(__dp__, "raise_from")(ValueError, exc)"#,
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
            (
                r#"a[1:2:3]"#,
                r#"getattr(__dp__, "getitem")(a, slice(1, 2, 3))"#,
            ),
            (
                r#"a[1:2]"#,
                r#"getattr(__dp__, "getitem")(a, slice(1, 2, None))"#,
            ),
            (
                r#"a[:2]"#,
                r#"getattr(__dp__, "getitem")(a, slice(None, 2, None))"#,
            ),
            (
                r#"a[::2]"#,
                r#"getattr(__dp__, "getitem")(a, slice(None, None, 2))"#,
            ),
            (
                r#"a[:]"#,
                r#"getattr(__dp__, "getitem")(a, slice(None, None, None))"#,
            ),
        ];

        for (input, expected) in cases {
            let output = rewrite_source(input);
            assert_eq!(output.trim_end(), expected);
        }
    }

    #[test]
    fn rewrites_complex_literals() {
        let cases = [
            (r#"a = 1j"#, r#"a = complex(0.0, 1.0)"#),
            (
                r#"a = 1 + 2j"#,
                r#"a = getattr(__dp__, "add")(1, complex(0.0, 2.0))"#,
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
a = getattr(__dp__, "getitem")(_dp_tmp_1, 0)
b = getattr(__dp__, "getitem")(_dp_tmp_1, 1)
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
a = getattr(__dp__, "getitem")(_dp_tmp_1, 0)
b = getattr(__dp__, "getitem")(_dp_tmp_1, 1)
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

    #[test]
    fn rewrites_list_comp() {
        let input = "
r = [a + 1 for a in items if a % 2 == 0]
";
        let output = rewrite_source(input);
        assert!(output.contains("getattr(__dp__, \"iter\")(items)"));
        assert!(output.contains("yield getattr(__dp__, \"add\")(a, 1)"));
    }

    #[test]
    fn rewrites_set_comp() {
        let input = "
r = {a for a in items}
";
        let output = rewrite_source(input);
        assert!(output.contains("getattr(__dp__, \"iter\")(items)"));
        assert!(output.contains("yield a"));
    }

    #[test]
    fn rewrites_dict_comp() {
        let input = "
r = {k: v + 1 for k, v in items if k % 2 == 0}
";
        let output = rewrite_source(input);
        assert!(output.contains("getattr(__dp__, \"iter\")(items)"));
        assert!(output.contains("yield k, getattr(__dp__, \"add\")(v, 1)"));
    }

    #[test]
    fn rewrites_multi_generator_list_comp() {
        let input = "
r = [a * b for a in items for b in items2]
";
        let output = rewrite_source(input);
        assert!(output.contains("getattr(__dp__, \"iter\")(items)"));
        assert!(output.contains("getattr(__dp__, \"mul\")(a, b)"));
    }
}
