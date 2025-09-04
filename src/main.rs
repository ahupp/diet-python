use std::{cell::Cell, env, fs, process};

use ruff_python_ast::{self as ast, Expr, ExprContext, Identifier, Operator, Stmt};
use ruff_python_ast::name::Name;
use ruff_python_ast::visitor::transformer::{walk_body, walk_expr, walk_stmt, Transformer};
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
            names.iter().any(|alias| alias.name.id.as_str() == "operator")
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

fn transform_source(source: &str) -> String {
    let parsed = parse_module(source).expect("parse error");
    let tokens = parsed.tokens().clone();
    let mut module = parsed.into_syntax();

    let transformer = BinOpRewriter::new();
    walk_body(&transformer, &mut module.body);

    if transformer.transformed() {
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
            (
                "x = 1 + 2\n",
                "import operator\nx = operator.add(1, 2)\n",
            ),
            (
                "y = a - b\n",
                "import operator\ny = operator.sub(a, b)\n",
            ),
        ];

        for (input, expected) in cases {
            assert_eq!(transform_source(input), expected);
        }
    }

    #[test]
    fn rewrites_aug_assign() {
        let input = "x = 1\nx += 2\n";
        let expected = "import operator\nx = 1\nx = operator.iadd(x, 2)\n";
        assert_eq!(transform_source(input), expected);
    }
}
