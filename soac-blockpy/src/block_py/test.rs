use super::*;
use crate::passes::CoreBlockPyPass;
use crate::py_expr;

#[derive(Debug, Clone)]
struct StructuredExprPass;

impl BlockPyPass for StructuredExprPass {
    type Expr = Expr;
}

#[test]
fn cfg_block_new_sets_explicit_term() {
    let block = Block::new(
        BlockLabel::from_index(0),
        vec![StructuredInstr::Expr(py_expr!("x"))],
        BlockTerm::<Expr>::Jump(crate::block_py::BlockEdge::new(BlockLabel::from_index(1))),
        Vec::new(),
        None,
    );

    assert_eq!(block.body.len(), 1);
    assert!(matches!(block.body[0], StructuredInstr::Expr(_)));
    assert!(matches!(block.term, BlockTerm::Jump(_)));
}

#[test]
fn cfg_block_from_fragment_without_term_uses_implicit_none_return_value() {
    let block = Block::from_builder(
        BlockLabel::from_index(0),
        BlockBuilder::from_stmts(vec![StructuredInstr::Expr(py_expr!("x"))]),
        Vec::new(),
        None,
        None,
    );

    assert_eq!(block.body.len(), 1);
    assert!(matches!(
        &block.term,
        BlockTerm::Return(Expr::NoneLiteral(_))
    ));
}

#[test]
fn stmt_fragment_can_carry_optional_term() {
    let fragment: BlockBuilder<StructuredInstr<Expr>, BlockTerm<Expr>> = BlockBuilder::with_term(
        vec![StructuredInstr::Expr(py_expr!("x"))],
        Some(BlockTerm::Return(py_expr!("None"))),
    );

    assert_eq!(fragment.body.len(), 1);
    assert!(matches!(fragment.body[0], StructuredInstr::Expr(_)));
    assert!(matches!(fragment.term, Some(BlockTerm::Return(_))));
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

fn test_name_gen() -> FunctionNameGen {
    let module_name_gen = ModuleNameGen::new(0);
    module_name_gen.next_function_name_gen()
}

#[test]
fn module_visitor_walks_blockpy_in_evaluation_order() {
    #[derive(Default)]
    struct TraceVisitor {
        trace: Vec<String>,
    }

    impl BlockPyModuleVisitor<StructuredExprPass> for TraceVisitor {
        fn visit_module(
            &mut self,
            module: &BlockPyModule<StructuredExprPass, StructuredInstr<Expr>>,
        ) {
            self.trace.push("module".to_string());
            walk_module(self, module);
        }

        fn visit_fn(&mut self, func: &BlockPyFunction<StructuredExprPass, StructuredInstr<Expr>>) {
            self.trace.push(format!("fn:{}", func.names.bind_name));
            walk_fn(self, func);
        }

        fn visit_block(&mut self, block: &Block<StructuredInstr<Expr>, Expr>) {
            self.trace.push(format!("block:{}", block.label));
            walk_block(self, block);
        }

        fn visit_fragment(
            &mut self,
            fragment: &BlockBuilder<StructuredInstr<Expr>, BlockTerm<Expr>>,
        ) {
            self.trace.push("fragment".to_string());
            walk_fragment(self, fragment);
        }

        fn visit_stmt(&mut self, stmt: &StructuredInstr<Expr>) {
            let kind = match stmt {
                StructuredInstr::Expr(_) => "expr",
                StructuredInstr::If(_) => "if",
            };
            self.trace.push(format!("stmt:{kind}"));
            walk_stmt(self, stmt);
        }

        fn visit_term(&mut self, term: &BlockTerm<Expr>) {
            let kind = match term {
                BlockTerm::Jump(_) => "jump",
                BlockTerm::IfTerm(_) => "if",
                BlockTerm::BranchTable(_) => "branch_table",
                BlockTerm::Raise(_) => "raise",
                BlockTerm::Return(_) => "return",
            };
            self.trace.push(format!("term:{kind}"));
            walk_term(self, term);
        }

        fn visit_label(&mut self, label: &BlockLabel) {
            self.trace.push(format!("label:{label}"));
        }

        fn visit_expr(&mut self, expr: &Expr) {
            let Expr::Name(name) = expr else {
                panic!("expected name expr in visitor trace test");
            };
            self.trace.push(format!("expr:{}", name.id));
        }
    }

    let module = BlockPyModule::<StructuredExprPass, StructuredInstr<Expr>> {
        module_name_gen: ModuleNameGen::new(0),
        callable_defs: vec![BlockPyFunction {
            function_id: FunctionId(0),
            name_gen: test_name_gen(),
            names: FunctionName::new("f", "f", "f", "f"),
            kind: FunctionKind::Function,
            params: ParamSpec::default(),
            blocks: vec![
                Block {
                    label: BlockLabel::from_index(0),
                    body: vec![
                        StructuredInstr::Expr(py_expr!("assign_one")),
                        StructuredInstr::If(StructuredIf {
                            test: py_expr!("if_test"),
                            body: BlockBuilder::with_term(
                                vec![StructuredInstr::Expr(py_expr!("then_expr"))],
                                Some(BlockTerm::Return(py_expr!("then_return"))),
                            ),
                            orelse: BlockBuilder::with_term(
                                vec![StructuredInstr::Expr(py_expr!("else_expr"))],
                                Some(BlockTerm::Raise(TermRaise {
                                    exc: Some(py_expr!("else_raise")),
                                })),
                            ),
                        }),
                        StructuredInstr::Expr(py_expr!("after_if")),
                    ],
                    term: BlockTerm::IfTerm(TermIf {
                        test: py_expr!("block_term_test"),
                        then_label: BlockLabel::from_index(1),
                        else_label: BlockLabel::from_index(2),
                    }),
                    params: Vec::new(),
                    exc_edge: None,
                },
                Block {
                    label: BlockLabel::from_index(3),
                    body: vec![StructuredInstr::Expr(py_expr!("trash"))],
                    term: BlockTerm::Return(py_expr!("final_return")),
                    params: Vec::new(),
                    exc_edge: None,
                },
            ],
            doc: None,
            storage_layout: None,
            scope: CallableScopeInfo::default(),
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
        kind: FunctionKind::Function,
        params: ParamSpec::default(),
        blocks: vec![Block {
            label: BlockLabel::from_index(0),
            body: vec![CellRefForName::new("captured".to_string()).into()],
            term: BlockTerm::Return(<CoreBlockPyExpr as ImplicitNoneExpr>::implicit_none_expr()),
            params: Vec::new(),
            exc_edge: None,
        }],
        doc: None,
        storage_layout: None,
        scope: CallableScopeInfo::default(),
    };

    let layout =
        compute_storage_layout_from_scope(&function).expect("structured cell ref should capture");

    assert_eq!(
        layout.freevars,
        vec![ClosureSlot {
            logical_name: "captured".to_string(),
            storage_name: "_dp_cell_captured".to_string(),
            init: ClosureInit::InheritedCapture,
        }]
    );
}
