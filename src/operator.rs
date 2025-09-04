use std::cell::Cell;

use ruff_python_ast::name::Name;
use ruff_python_ast::visitor::transformer::{walk_expr, walk_stmt, Transformer};
use ruff_python_ast::{self as ast, CmpOp, Expr, ExprContext, Identifier, Operator, Stmt, UnaryOp};
use ruff_text_size::TextRange;

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

pub fn ensure_operator_import(module: &mut ast::ModModule) {
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
