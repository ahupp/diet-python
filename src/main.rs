use std::{cell::Cell, env, fs, process};

use ruff_python_ast::name::Name;
use ruff_python_ast::visitor::transformer::{walk_body, walk_expr, walk_stmt, Transformer};
use ruff_python_ast::{self as ast, CmpOp, Expr, ExprContext, Identifier, Operator, Stmt, UnaryOp};
use ruff_python_codegen::{Generator, Stylist};
use ruff_python_parser::parse_module;
use ruff_text_size::TextRange;

struct BinOpRewriter {
    replaced: Cell<bool>,
}

impl BinOpRewriter {
    fn new() -> Self {
        Self {
            replaced: Cell::new(false),
        }
    }

    fn transformed(&self) -> bool {
        self.replaced.get()
    }
}

impl Transformer for BinOpRewriter {
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

            let func = Expr::Attribute(ast::ExprAttribute {
                node_index: ast::AtomicNodeIndex::default(),
                range: TextRange::default(),
                value: Box::new(Expr::Name(ast::ExprName {
                    node_index: ast::AtomicNodeIndex::default(),
                    range: TextRange::default(),
                    id: Name::new_static("operator"),
                    ctx: ExprContext::Load,
                })),
                attr: Identifier::new(Name::new_static(func_name), TextRange::default()),
                ctx: ExprContext::Load,
            });

            let call = Expr::Call(ast::ExprCall {
                node_index: ast::AtomicNodeIndex::default(),
                range: TextRange::default(),
                func: Box::new(func),
                arguments: ast::Arguments {
                    range: TextRange::default(),
                    node_index: ast::AtomicNodeIndex::default(),
                    args: vec![left, right].into_boxed_slice(),
                    keywords: Vec::new().into_boxed_slice(),
                },
            });

            *expr = call;
            self.replaced.set(true);
        } else if let Expr::UnaryOp(unary) = expr {
            let operand = *unary.operand.clone();

            let func_name = match unary.op {
                UnaryOp::Not => "not_",
                UnaryOp::Invert => "invert",
                UnaryOp::USub => "neg",
                _ => return,
            };

            let func = Expr::Attribute(ast::ExprAttribute {
                node_index: ast::AtomicNodeIndex::default(),
                range: TextRange::default(),
                value: Box::new(Expr::Name(ast::ExprName {
                    node_index: ast::AtomicNodeIndex::default(),
                    range: TextRange::default(),
                    id: Name::new_static("operator"),
                    ctx: ExprContext::Load,
                })),
                attr: Identifier::new(Name::new_static(func_name), TextRange::default()),
                ctx: ExprContext::Load,
            });

            let call = Expr::Call(ast::ExprCall {
                node_index: ast::AtomicNodeIndex::default(),
                range: TextRange::default(),
                func: Box::new(func),
                arguments: ast::Arguments {
                    range: TextRange::default(),
                    node_index: ast::AtomicNodeIndex::default(),
                    args: vec![operand].into_boxed_slice(),
                    keywords: Vec::new().into_boxed_slice(),
                },
            });

            *expr = call;
            self.replaced.set(true);
        } else if let Expr::Compare(compare) = expr {
            if compare.ops.len() == 1 && compare.comparators.len() == 1 {
                let left = (*compare.left).clone();
                let right = compare.comparators[0].clone();

                let make_call = |func_name: &'static str, args: Vec<Expr>| {
                    let func = Expr::Attribute(ast::ExprAttribute {
                        node_index: ast::AtomicNodeIndex::default(),
                        range: TextRange::default(),
                        value: Box::new(Expr::Name(ast::ExprName {
                            node_index: ast::AtomicNodeIndex::default(),
                            range: TextRange::default(),
                            id: Name::new_static("operator"),
                            ctx: ExprContext::Load,
                        })),
                        attr: Identifier::new(Name::new_static(func_name), TextRange::default()),
                        ctx: ExprContext::Load,
                    });

                    Expr::Call(ast::ExprCall {
                        node_index: ast::AtomicNodeIndex::default(),
                        range: TextRange::default(),
                        func: Box::new(func),
                        arguments: ast::Arguments {
                            range: TextRange::default(),
                            node_index: ast::AtomicNodeIndex::default(),
                            args: args.into_boxed_slice(),
                            keywords: Vec::new().into_boxed_slice(),
                        },
                    })
                };

                let call = match compare.ops[0] {
                    CmpOp::Eq => make_call("eq", vec![left, right]),
                    CmpOp::NotEq => make_call("ne", vec![left, right]),
                    CmpOp::Lt => make_call("lt", vec![left, right]),
                    CmpOp::Gt => make_call("gt", vec![left, right]),
                    CmpOp::IsNot => make_call("is_not", vec![left, right]),
                    CmpOp::In => make_call("contains", vec![right, left]),
                    CmpOp::NotIn => {
                        let contains = make_call("contains", vec![right, left]);
                        make_call("not_", vec![contains])
                    }
                    _ => return,
                };

                *expr = call;
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

            let func = Expr::Attribute(ast::ExprAttribute {
                node_index: ast::AtomicNodeIndex::default(),
                range: TextRange::default(),
                value: Box::new(Expr::Name(ast::ExprName {
                    node_index: ast::AtomicNodeIndex::default(),
                    range: TextRange::default(),
                    id: Name::new_static("operator"),
                    ctx: ExprContext::Load,
                })),
                attr: Identifier::new(Name::new_static(func_name), TextRange::default()),
                ctx: ExprContext::Load,
            });

            let call = Expr::Call(ast::ExprCall {
                node_index: ast::AtomicNodeIndex::default(),
                range: TextRange::default(),
                func: Box::new(func),
                arguments: ast::Arguments {
                    range: TextRange::default(),
                    node_index: ast::AtomicNodeIndex::default(),
                    args: vec![target.clone(), value].into_boxed_slice(),
                    keywords: Vec::new().into_boxed_slice(),
                },
            });

            *stmt = Stmt::Assign(ast::StmtAssign {
                node_index: ast::AtomicNodeIndex::default(),
                range: TextRange::default(),
                targets: vec![target],
                value: Box::new(call),
            });

            self.replaced.set(true);
        }
    }
}

fn ensure_operator_import(module: &mut ast::ModModule) {
    let has_import = module.body.iter().any(|stmt| {
        if let Stmt::Import(ast::StmtImport { names, .. }) = stmt {
            names
                .iter()
                .any(|alias| alias.name.id.as_str() == "operator")
        } else {
            false
        }
    });

    if !has_import {
        let alias = ast::Alias {
            range: TextRange::default(),
            node_index: ast::AtomicNodeIndex::default(),
            name: Identifier::new(Name::new_static("operator"), TextRange::default()),
            asname: None,
        };

        module.body.insert(
            0,
            Stmt::Import(ast::StmtImport {
                node_index: ast::AtomicNodeIndex::default(),
                range: TextRange::default(),
                names: vec![alias],
            }),
        );
    }
}

fn rewrite_source_inner(source: &str, ensure_import: bool) -> String {
    let parsed = parse_module(source).expect("parse error");
    let tokens = parsed.tokens().clone();
    let mut module = parsed.into_syntax();

    let transformer = BinOpRewriter::new();
    walk_body(&transformer, &mut module.body);

    if ensure_import && transformer.transformed() {
        ensure_operator_import(&mut module);
    }

    let stylist = Stylist::from_tokens(&tokens, source);
    let mut output = String::new();
    for stmt in &module.body {
        let snippet = Generator::from(&stylist).stmt(stmt);
        output.push_str(&snippet);
        output.push_str(stylist.line_ending().as_str());
    }
    output
}

fn transform_source(source: &str) -> String {
    rewrite_source_inner(source, true)
}

#[cfg(test)]
fn rewrite_source(source: &str) -> String {
    rewrite_source_inner(source, false)
}

fn main() {
    let path = env::args().nth(1).unwrap_or_else(|| {
        eprintln!("usage: diet-python <python-file>");
        process::exit(1);
    });

    let source = match fs::read_to_string(&path) {
        Ok(src) => src,
        Err(err) => {
            eprintln!("failed to read {}: {}", path, err);
            process::exit(1);
        }
    };

    let output = transform_source(&source);
    print!("{}", output);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rewrites_binary_ops() {
        let cases = [
            ("x = 1 + 2\n", "x = operator.add(1, 2)\n"),
            ("y = a - b\n", "y = operator.sub(a, b)\n"),
        ];

        for (input, expected) in cases {
            assert_eq!(rewrite_source(input), expected);
        }
    }

    #[test]
    fn rewrites_aug_assign() {
        let input = "x = 1\nx += 2\n";
        let expected = "x = 1\nx = operator.iadd(x, 2)\n";
        assert_eq!(rewrite_source(input), expected);
    }

    #[test]
    fn rewrites_unary_ops() {
        let cases = [
            ("x = -a\n", "x = operator.neg(a)\n"),
            ("y = ~b\n", "y = operator.invert(b)\n"),
            ("z = not c\n", "z = operator.not_(c)\n"),
        ];

        for (input, expected) in cases {
            assert_eq!(rewrite_source(input), expected);
        }
    }

    #[test]
    fn rewrites_comparisons() {
        let cases = [
            ("x = a == b\n", "x = operator.eq(a, b)\n"),
            ("x = a != b\n", "x = operator.ne(a, b)\n"),
            ("x = a < b\n", "x = operator.lt(a, b)\n"),
            ("x = a > b\n", "x = operator.gt(a, b)\n"),
            (
                "x = a is not b\n",
                "x = operator.is_not(a, b)\n",
            ),
            (
                "x = a in b\n",
                "x = operator.contains(b, a)\n",
            ),
            (
                "x = a not in b\n",
                "x = operator.not_(operator.contains(b, a))\n",
            ),
        ];

        for (input, expected) in cases {
            assert_eq!(rewrite_source(input), expected);
        }
    }
}
