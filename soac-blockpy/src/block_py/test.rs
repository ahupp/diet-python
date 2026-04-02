use super::structured::{
    walk_block, walk_fn, walk_fragment, walk_module, walk_stmt, walk_term, BlockPyModuleVisitor,
};
use super::*;
use crate::passes::{CoreBlockPyPass, CoreBlockPyPassWithAwaitAndYield, CoreBlockPyPassWithYield};
use crate::py_expr;

#[derive(Debug, Clone)]
struct StructuredExprPass;

impl BlockPyPass for StructuredExprPass {
    type Expr = Expr;
    type Stmt = PassStructuredInstr<Self>;
}

#[test]
fn block_builder_sets_explicit_term() {
    let mut block: BlockPyBlockBuilder<Expr> =
        BlockPyBlockBuilder::new(BlockPyLabel::from_index(0));
    block.push_stmt(StructuredInstr::Expr(py_expr!("x")));
    block.set_term(BlockPyTerm::Jump(crate::block_py::BlockPyEdge::new(
        BlockPyLabel::from_index(1),
    )));
    let block = block.finish(None);

    assert_eq!(block.body.len(), 1);
    assert!(matches!(block.body[0], StructuredInstr::Expr(_)));
    assert!(matches!(block.term, BlockPyTerm::Jump(_)));
}

#[test]
fn block_builder_without_term_uses_implicit_none_return_value() {
    let mut block: BlockPyBlockBuilder<Expr> =
        BlockPyBlockBuilder::new(BlockPyLabel::from_index(0));
    block.push_stmt(StructuredInstr::Expr(py_expr!("x")));
    let block = block.finish(None);

    assert_eq!(block.body.len(), 1);
    assert!(matches!(
        &block.term,
        BlockPyTerm::Return(Expr::NoneLiteral(_))
    ));
}

#[test]
fn stmt_fragment_can_carry_optional_term() {
    let fragment: BlockPyStmtFragment<Expr> = BlockPyStmtFragment::with_term(
        vec![StructuredInstr::Expr(py_expr!("x"))],
        Some(BlockPyTerm::Return(py_expr!("None"))),
    );

    assert_eq!(fragment.body.len(), 1);
    assert!(matches!(fragment.body[0], StructuredInstr::Expr(_)));
    assert!(matches!(fragment.term, Some(BlockPyTerm::Return(_))));
}

#[test]
fn core_blockpy_expr_wraps_name_expr() {
    let expr = CoreBlockPyExprWithAwaitAndYield::from(py_expr!("y"));

    assert!(matches!(
        expr,
        CoreBlockPyExprWithAwaitAndYield::Load(op)
            if op.name.id_str() == "y"
    ));
}

#[test]
fn call_and_keyword_arg_expr_helpers_preserve_shape() {
    let mut positional = CoreBlockPyCallArg::Positional(py_expr!("x"));
    *positional.expr_mut() = py_expr!("y");
    assert!(matches!(
        positional,
        CoreBlockPyCallArg::Positional(Expr::Name(name)) if name.id.as_str() == "y"
    ));

    let starred = CoreBlockPyCallArg::Starred(py_expr!("z")).map_expr(|expr| {
        let Expr::Name(name) = expr else {
            panic!("expected name expr");
        };
        Expr::Name(name)
    });
    assert!(matches!(starred, CoreBlockPyCallArg::Starred(_)));

    let keyword = CoreBlockPyKeywordArg::Named {
        arg: ast::Identifier::new("value", ruff_text_size::TextRange::default()),
        value: py_expr!("a"),
    }
    .try_map_expr(|expr| -> Result<Expr, &'static str> {
        let Expr::Name(name) = expr else {
            return Err("expected name expr");
        };
        Ok(Expr::Name(name))
    })
    .expect("keyword arg mapping should succeed");
    assert!(matches!(
        keyword,
        CoreBlockPyKeywordArg::Named { arg, value: Expr::Name(_), .. } if arg.as_str() == "value"
    ));
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

    impl BlockPyModuleVisitor<StructuredExprPass> for TraceVisitor {
        fn visit_module(&mut self, module: &BlockPyModule<StructuredExprPass>) {
            self.trace.push("module".to_string());
            walk_module(self, module);
        }

        fn visit_fn(&mut self, func: &BlockPyFunction<StructuredExprPass>) {
            self.trace.push(format!("fn:{}", func.names.bind_name));
            walk_fn(self, func);
        }

        fn visit_block(&mut self, block: &PassBlock<StructuredExprPass>) {
            self.trace.push(format!("block:{}", block.label));
            walk_block(self, block);
        }

        fn visit_fragment(&mut self, fragment: &PassStructuredFragment<StructuredExprPass>) {
            self.trace.push("fragment".to_string());
            walk_fragment(self, fragment);
        }

        fn visit_stmt(&mut self, stmt: &PassStructuredInstr<StructuredExprPass>) {
            let kind = match stmt {
                StructuredInstr::Expr(_) => "expr",
                StructuredInstr::If(_) => "if",
            };
            self.trace.push(format!("stmt:{kind}"));
            walk_stmt(self, stmt);
        }

        fn visit_term(&mut self, term: &PassTerm<StructuredExprPass>) {
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
            self.trace.push(format!("label:{label}"));
        }

        fn visit_expr(&mut self, expr: &PassExpr<StructuredExprPass>) {
            let Expr::Name(name) = expr else {
                panic!("expected name expr in visitor trace test");
            };
            self.trace.push(format!("expr:{}", name.id));
        }
    }

    let module = BlockPyModule::<StructuredExprPass> {
        callable_defs: vec![BlockPyFunction {
            function_id: FunctionId(0),
            name_gen: test_name_gen(),
            names: FunctionName::new("f", "f", "f", "f"),
            kind: BlockPyFunctionKind::Function,
            params: ParamSpec::default(),
            blocks: vec![
                CfgBlock {
                    label: BlockPyLabel::from_index(0),
                    body: vec![
                        StructuredInstr::Expr(py_expr!("assign_one")),
                        StructuredInstr::If(BlockPyIf {
                            test: py_expr!("if_test"),
                            body: BlockPyCfgFragment::with_term(
                                vec![StructuredInstr::Expr(py_expr!("then_expr"))],
                                Some(BlockPyTerm::Return(py_expr!("then_return"))),
                            ),
                            orelse: BlockPyCfgFragment::with_term(
                                vec![StructuredInstr::Expr(py_expr!("else_expr"))],
                                Some(BlockPyTerm::Raise(BlockPyRaise {
                                    exc: Some(py_expr!("else_raise")),
                                })),
                            ),
                        }),
                        StructuredInstr::Expr(py_expr!("after_if")),
                    ],
                    term: BlockPyTerm::IfTerm(BlockPyIfTerm {
                        test: py_expr!("block_term_test"),
                        then_label: BlockPyLabel::from_index(1),
                        else_label: BlockPyLabel::from_index(2),
                    }),
                    params: Vec::new(),
                    exc_edge: None,
                },
                CfgBlock {
                    label: BlockPyLabel::from_index(3),
                    body: vec![StructuredInstr::Expr(py_expr!("trash"))],
                    term: BlockPyTerm::Return(py_expr!("final_return")),
                    params: Vec::new(),
                    exc_edge: None,
                },
            ],
            doc: None,
            storage_layout: None,
            semantic: BlockPyCallableSemanticInfo::default(),
        }],
        module_constants: Vec::new(),
    };

    let mut visitor = TraceVisitor::default();
    visitor.visit_module(&module);

    assert_eq!(
        visitor.trace,
        vec![
            "module",
            "fn:f",
            "block:bb0",
            "stmt:expr",
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
            "label:bb1",
            "label:bb2",
            "block:bb3",
            "stmt:expr",
            "expr:trash",
            "term:return",
            "expr:final_return",
        ]
        .into_iter()
        .map(str::to_string)
        .collect::<Vec<_>>()
    );
}

#[test]
fn storage_layout_semantics_collects_structured_cell_ref_logical_names() {
    let function = BlockPyFunction::<CoreBlockPyPass> {
        function_id: FunctionId(0),
        name_gen: test_name_gen(),
        names: FunctionName::new("f", "f", "f", "f"),
        kind: BlockPyFunctionKind::Function,
        params: ParamSpec::default(),
        blocks: vec![CfgBlock {
            label: BlockPyLabel::from_index(0),
            body: vec![BlockPyStmt::Expr(core_operation_expr(
                CellRefForName::new("captured".to_string()).with_meta(Meta::synthetic()),
            ))],
            term: BlockPyTerm::Return(<CoreBlockPyExpr as ImplicitNoneExpr>::implicit_none_expr()),
            params: Vec::new(),
            exc_edge: None,
        }],
        doc: None,
        storage_layout: None,
        semantic: BlockPyCallableSemanticInfo::default(),
    };

    let layout = compute_storage_layout_from_semantics(&function)
        .expect("structured cell ref should capture");

    assert_eq!(
        layout.freevars,
        vec![ClosureSlot {
            logical_name: "captured".to_string(),
            storage_name: "_dp_cell_captured".to_string(),
            init: ClosureInit::InheritedCapture,
        }]
    );
}

#[test]
fn stmt_conversion_to_no_await_rejects_await() {
    let stmt = StructuredInstr::Expr(CoreBlockPyExprWithAwaitAndYield::Await(
        CoreBlockPyAwait::new(CoreBlockPyExprWithAwaitAndYield::Name(
            name_expr("x").into(),
        ))
        .with_meta(Meta::default()),
    ));

    assert!(ExprTryMap::<
        CoreBlockPyPassWithAwaitAndYield,
        CoreBlockPyPassWithYield,
        CoreBlockPyExprWithAwaitAndYield,
    >::without_await()
    .try_map_stmt(stmt.into())
    .is_err());
}

#[test]
fn try_module_map_propagates_nested_expr_conversion_errors() {
    struct RejectAwaitMapper;

    impl BlockPyModuleTryMap<CoreBlockPyPassWithAwaitAndYield, CoreBlockPyPassWithYield>
        for RejectAwaitMapper
    {
        type Error = CoreBlockPyExprWithAwaitAndYield;
    }

    let module = BlockPyModule::<CoreBlockPyPassWithAwaitAndYield> {
        callable_defs: vec![BlockPyFunction {
            function_id: FunctionId(0),
            name_gen: test_name_gen(),
            names: FunctionName::new("f", "f", "f", "f"),
            kind: BlockPyFunctionKind::Function,
            params: ParamSpec::default(),
            blocks: vec![CfgBlock {
                label: BlockPyLabel::from_index(0),
                body: vec![BlockPyStmt::Expr(CoreBlockPyExprWithAwaitAndYield::Await(
                    CoreBlockPyAwait::new(CoreBlockPyExprWithAwaitAndYield::Name(
                        name_expr("x").into(),
                    ))
                    .with_meta(Meta::default()),
                ))],
                term: BlockPyTerm::Return(CoreBlockPyExprWithAwaitAndYield::Name(
                    name_expr("__dp_NONE").into(),
                )),
                params: Vec::new(),
                exc_edge: None,
            }],
            doc: None,
            storage_layout: None,
            semantic: BlockPyCallableSemanticInfo::default(),
        }],
        module_constants: Vec::new(),
    };

    assert!(module.try_map_module(&RejectAwaitMapper).is_err());
}

#[test]
fn term_conversion_to_no_yield_rejects_nested_yield() {
    let term = BlockPyTerm::Return(core_call_expr_with_meta(
        CoreBlockPyExprWithYield::Name(name_expr("f").into()),
        ast::AtomicNodeIndex::default(),
        ruff_text_size::TextRange::default(),
        vec![CoreBlockPyCallArg::Positional(
            CoreBlockPyExprWithYield::Yield(
                CoreBlockPyYield::new(CoreBlockPyExprWithYield::Name(name_expr("x").into()))
                    .with_meta(Meta::default()),
            ),
        )],
        Vec::new(),
    ));

    assert!(
        ExprTryMap::<CoreBlockPyPassWithYield, CoreBlockPyPass, CoreBlockPyExprWithYield>::without_yield()
            .try_map_term(term)
            .is_err()
    );
}
