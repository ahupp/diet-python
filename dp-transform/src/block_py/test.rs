use super::*;
use crate::py_expr;

#[test]
fn block_builder_sets_explicit_term() {
    let mut block: BlockPyBlockBuilder<Expr> =
        BlockPyBlockBuilder::new(BlockPyLabel::from("start"));
    block.push_stmt(BlockPyStmt::Expr(py_expr!("x")));
    block.set_term(BlockPyTerm::Jump(BlockPyLabel::from("after").into()));
    let block = block.finish(None);

    assert_eq!(block.body.len(), 1);
    assert!(matches!(block.body[0], BlockPyStmt::Expr(_)));
    assert!(matches!(block.term, BlockPyTerm::Jump(_)));
}

#[test]
fn block_builder_without_term_uses_implicit_none_return_value() {
    let mut block: BlockPyBlockBuilder<Expr> =
        BlockPyBlockBuilder::new(BlockPyLabel::from("start"));
    block.push_stmt(BlockPyStmt::Expr(py_expr!("x")));
    let block = block.finish(None);

    assert_eq!(block.body.len(), 1);
    assert!(matches!(
        &block.term,
        BlockPyTerm::Return(Expr::Name(name)) if name.id.as_str() == "__dp_NONE"
    ));
}

#[test]
fn stmt_fragment_can_carry_optional_term() {
    let fragment: BlockPyStmtFragment<Expr> = BlockPyStmtFragment::with_term(
        vec![BlockPyStmt::Expr(py_expr!("x"))],
        Some(BlockPyTerm::Return(py_expr!("__dp_NONE"))),
    );

    assert_eq!(fragment.body.len(), 1);
    assert!(matches!(fragment.body[0], BlockPyStmt::Expr(_)));
    assert!(matches!(fragment.term, Some(BlockPyTerm::Return(_))));
}

#[test]
fn core_blockpy_expr_wraps_and_rewrites_expr() {
    let mut expr = CoreBlockPyExprWithAwaitAndYield::from(py_expr!("x"));
    expr.rewrite_mut(|expr| *expr = py_expr!("y"));

    let Expr::Name(name) = expr.to_expr() else {
        panic!("expected name expr after rewrite");
    };
    assert_eq!(name.id.as_str(), "y");
}

fn name_expr(name: &str) -> ast::ExprName {
    let Expr::Name(name) = py_expr!("{name:id}", name = name) else {
        unreachable!();
    };
    name
}

fn test_name_gen() -> FunctionNameGen {
    let mut module_name_gen = ModuleNameGen::new(0);
    module_name_gen.next_function_name_gen()
}

#[test]
fn module_visitor_walks_blockpy_in_evaluation_order() {
    #[derive(Default)]
    struct TraceVisitor {
        trace: Vec<String>,
    }

    impl BlockPyModuleVisitor<RuffBlockPyPass> for TraceVisitor {
        fn visit_module(&mut self, module: &BlockPyModule<RuffBlockPyPass>) {
            self.trace.push("module".to_string());
            walk_module(self, module);
        }

        fn visit_fn(&mut self, func: &BlockPyFunction<RuffBlockPyPass>) {
            self.trace.push(format!("fn:{}", func.names.bind_name));
            walk_fn(self, func);
        }

        fn visit_block(&mut self, block: &PassBlock<RuffBlockPyPass>) {
            self.trace.push(format!("block:{}", block.label));
            walk_block(self, block);
        }

        fn visit_fragment(
            &mut self,
            fragment: &BlockPyCfgFragment<
                <RuffBlockPyPass as BlockPyPass>::Stmt,
                BlockPyTerm<PassExpr<RuffBlockPyPass>>,
            >,
        ) {
            self.trace.push("fragment".to_string());
            walk_fragment(self, fragment);
        }

        fn visit_stmt(&mut self, stmt: &BlockPyStmt<PassExpr<RuffBlockPyPass>>) {
            let kind = match stmt {
                BlockPyStmt::Assign(_) => "assign",
                BlockPyStmt::Expr(_) => "expr",
                BlockPyStmt::Delete(_) => "delete",
                BlockPyStmt::If(_) => "if",
            };
            self.trace.push(format!("stmt:{kind}"));
            walk_stmt(self, stmt);
        }

        fn visit_term(&mut self, term: &BlockPyTerm<PassExpr<RuffBlockPyPass>>) {
            let kind = match term {
                BlockPyTerm::Jump(_) => "jump",
                BlockPyTerm::IfTerm(_) => "if",
                BlockPyTerm::BranchTable(_) => "branch_table",
                BlockPyTerm::Raise(_) => "raise",
                BlockPyTerm::Return(_) => "return",
            };
            self.trace.push(format!("term:{kind}"));
            walk_term(self, term);
        }

        fn visit_label(&mut self, label: &BlockPyLabel) {
            self.trace.push(format!("label:{}", label.as_str()));
        }

        fn visit_expr(&mut self, expr: &PassExpr<RuffBlockPyPass>) {
            let Expr::Name(name) = expr else {
                panic!("expected name expr in visitor trace test");
            };
            self.trace.push(format!("expr:{}", name.id));
        }
    }

    let module = BlockPyModule::<RuffBlockPyPass> {
        callable_defs: vec![BlockPyFunction {
            function_id: FunctionId(0),
            name_gen: test_name_gen(),
            names: FunctionName::new("f", "f", "f", "f"),
            kind: BlockPyFunctionKind::Function,
            params: ParamSpec::default(),
            blocks: vec![
                CfgBlock {
                    label: BlockPyLabel::from("start"),
                    body: vec![
                        BlockPyStmt::Assign(BlockPyAssign {
                            target: name_expr("target"),
                            value: py_expr!("assign_one"),
                        }),
                        BlockPyStmt::If(BlockPyIf {
                            test: py_expr!("if_test"),
                            body: BlockPyCfgFragment::with_term(
                                vec![BlockPyStmt::Expr(py_expr!("then_expr"))],
                                Some(BlockPyTerm::Return(py_expr!("then_return"))),
                            ),
                            orelse: BlockPyCfgFragment::with_term(
                                vec![BlockPyStmt::Expr(py_expr!("else_expr"))],
                                Some(BlockPyTerm::Raise(BlockPyRaise {
                                    exc: Some(py_expr!("else_raise")),
                                })),
                            ),
                        }),
                        BlockPyStmt::Expr(py_expr!("after_if")),
                    ],
                    term: BlockPyTerm::IfTerm(BlockPyIfTerm {
                        test: py_expr!("block_term_test"),
                        then_label: BlockPyLabel::from("then"),
                        else_label: BlockPyLabel::from("else"),
                    }),
                    params: Vec::new(),
                    exc_edge: None,
                },
                CfgBlock {
                    label: BlockPyLabel::from("done"),
                    body: vec![BlockPyStmt::Delete(BlockPyDelete {
                        target: name_expr("trash"),
                    })],
                    term: BlockPyTerm::Return(py_expr!("final_return")),
                    params: Vec::new(),
                    exc_edge: None,
                },
            ],
            doc: None,
            closure_layout: None,
            facts: BlockPyCallableFacts::default(),
            semantic: BlockPyCallableSemanticInfo::default(),
        }],
    };

    let mut visitor = TraceVisitor::default();
    module.visit_module(&mut visitor);

    assert_eq!(
        visitor.trace,
        vec![
            "module",
            "fn:f",
            "block:start",
            "stmt:assign",
            "expr:assign_one",
            "stmt:if",
            "expr:if_test",
            "fragment",
            "stmt:expr",
            "expr:then_expr",
            "term:return",
            "expr:then_return",
            "fragment",
            "stmt:expr",
            "expr:else_expr",
            "term:raise",
            "expr:else_raise",
            "stmt:expr",
            "expr:after_if",
            "term:if",
            "expr:block_term_test",
            "label:then",
            "label:else",
            "block:done",
            "stmt:delete",
            "term:return",
            "expr:final_return",
        ]
        .into_iter()
        .map(str::to_string)
        .collect::<Vec<_>>()
    );
}

#[test]
fn stmt_conversion_to_no_await_rejects_await() {
    let stmt = BlockPyStmt::Expr(CoreBlockPyExprWithAwaitAndYield::Await(CoreBlockPyAwait {
        node_index: ast::AtomicNodeIndex::default(),
        range: ruff_text_size::TextRange::default(),
        value: Box::new(CoreBlockPyExprWithAwaitAndYield::Name(name_expr("x"))),
    }));

    assert!(BlockPyStmt::<CoreBlockPyExprWithYield>::try_from(stmt).is_err());
}

#[test]
fn term_conversion_to_no_yield_rejects_nested_yield() {
    let term = BlockPyTerm::Return(CoreBlockPyExprWithYield::Call(CoreBlockPyCall {
        node_index: ast::AtomicNodeIndex::default(),
        range: ruff_text_size::TextRange::default(),
        func: Box::new(CoreBlockPyExprWithYield::Name(name_expr("f"))),
        args: vec![CoreBlockPyCallArg::Positional(
            CoreBlockPyExprWithYield::Yield(CoreBlockPyYield {
                node_index: ast::AtomicNodeIndex::default(),
                range: ruff_text_size::TextRange::default(),
                value: Some(Box::new(CoreBlockPyExprWithYield::Name(name_expr("x")))),
            }),
        )],
        keywords: Vec::new(),
    }));

    assert!(BlockPyTerm::<CoreBlockPyExpr>::try_from(term).is_err());
}
