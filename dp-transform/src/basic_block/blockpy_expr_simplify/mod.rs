use super::{
    block_py::{
        BlockPyBlockMeta, BlockPyBranchTable, BlockPyCfgFragment, BlockPyDelete, BlockPyIf,
        BlockPyIfTerm, BlockPyRaise, BlockPyStmtFragmentBuilder, BlockPyTerm, CoreBlockPyAssign,
        CoreBlockPyBlock, CoreBlockPyCallableDef, CoreBlockPyExpr, CoreBlockPyModule,
        CoreBlockPyStmt, CoreBlockPyStmtFragment, CoreBlockPyTerm, SemanticBlockPyBlock,
        SemanticBlockPyCallableDef, SemanticBlockPyExpr, SemanticBlockPyModule,
        SemanticBlockPyStmt, SemanticBlockPyStmtFragment, SemanticBlockPyTerm,
    },
    cfg_ir::CfgCallableDef,
};

type CoreStmtBuilder = BlockPyStmtFragmentBuilder<CoreBlockPyExpr>;
type SemanticExpr = SemanticBlockPyExpr;

pub(crate) trait PureCoreExprReducer {
    fn reduce_expr(&self, expr: &SemanticExpr) -> CoreBlockPyExpr;
}

struct DefaultCoreExprReducer;

impl PureCoreExprReducer for DefaultCoreExprReducer {
    fn reduce_expr(&self, expr: &SemanticExpr) -> CoreBlockPyExpr {
        expr.clone().into()
    }
}

fn finish_expr_setup(builder: CoreStmtBuilder) -> Vec<CoreBlockPyStmt> {
    let fragment = builder.finish();
    assert!(
        fragment.term.is_none(),
        "semantic-to-core expression lowering produced an unexpected terminator",
    );
    fragment.body
}

fn lower_semantic_expr_into(builder: &mut CoreStmtBuilder, expr: &SemanticExpr) -> CoreBlockPyExpr {
    let _ = builder;
    DefaultCoreExprReducer.reduce_expr(expr)
}

fn lower_semantic_expr_without_setup(expr: &SemanticExpr) -> CoreBlockPyExpr {
    let mut setup = CoreStmtBuilder::new();
    let lowered = lower_semantic_expr_into(&mut setup, expr);
    assert!(
        finish_expr_setup(setup).is_empty(),
        "semantic-to-core metadata expression lowering unexpectedly emitted setup statements",
    );
    lowered
}

fn lower_semantic_stmt_fragment(fragment: &CoreLikeStmtFragmentInput) -> CoreBlockPyStmtFragment {
    let mut builder = CoreStmtBuilder::new();
    for stmt in &fragment.body {
        lower_semantic_stmt_into(&mut builder, stmt);
    }
    if let Some(term) = &fragment.term {
        lower_semantic_term_into(&mut builder, term);
    }
    builder.finish()
}

type CoreLikeStmtFragmentInput = SemanticBlockPyStmtFragment;

fn lower_semantic_stmt_into(builder: &mut CoreStmtBuilder, stmt: &SemanticBlockPyStmt) {
    match stmt {
        SemanticBlockPyStmt::Pass => builder.push_stmt(CoreBlockPyStmt::Pass),
        SemanticBlockPyStmt::Assign(assign) => {
            let mut setup = CoreStmtBuilder::new();
            let value = lower_semantic_expr_into(&mut setup, &assign.value);
            builder.extend(finish_expr_setup(setup));
            builder.push_stmt(CoreBlockPyStmt::Assign(CoreBlockPyAssign {
                target: assign.target.clone(),
                value,
            }));
        }
        SemanticBlockPyStmt::Expr(expr) => {
            let mut setup = CoreStmtBuilder::new();
            let expr = lower_semantic_expr_into(&mut setup, expr);
            builder.extend(finish_expr_setup(setup));
            builder.push_stmt(CoreBlockPyStmt::Expr(expr));
        }
        SemanticBlockPyStmt::Delete(BlockPyDelete { target }) => {
            builder.push_stmt(CoreBlockPyStmt::Delete(BlockPyDelete {
                target: target.clone(),
            }));
        }
        SemanticBlockPyStmt::If(if_stmt) => {
            let mut setup = CoreStmtBuilder::new();
            let test = lower_semantic_expr_into(&mut setup, &if_stmt.test);
            builder.extend(finish_expr_setup(setup));
            builder.push_stmt(CoreBlockPyStmt::If(BlockPyIf {
                test,
                body: lower_semantic_stmt_fragment(&if_stmt.body),
                orelse: lower_semantic_stmt_fragment(&if_stmt.orelse),
            }));
        }
    }
}

fn lower_semantic_term_into(builder: &mut CoreStmtBuilder, term: &SemanticBlockPyTerm) {
    match term {
        BlockPyTerm::Jump(label) => builder.set_term(CoreBlockPyTerm::Jump(label.clone())),
        BlockPyTerm::IfTerm(BlockPyIfTerm {
            test,
            then_label,
            else_label,
        }) => {
            let mut setup = CoreStmtBuilder::new();
            let test = lower_semantic_expr_into(&mut setup, test);
            builder.extend(finish_expr_setup(setup));
            builder.set_term(CoreBlockPyTerm::IfTerm(BlockPyIfTerm {
                test,
                then_label: then_label.clone(),
                else_label: else_label.clone(),
            }));
        }
        BlockPyTerm::BranchTable(BlockPyBranchTable {
            index,
            targets,
            default_label,
        }) => {
            let mut setup = CoreStmtBuilder::new();
            let index = lower_semantic_expr_into(&mut setup, index);
            builder.extend(finish_expr_setup(setup));
            builder.set_term(CoreBlockPyTerm::BranchTable(BlockPyBranchTable {
                index,
                targets: targets.clone(),
                default_label: default_label.clone(),
            }));
        }
        BlockPyTerm::Raise(BlockPyRaise { exc }) => {
            let exc = exc.as_ref().map(|exc| {
                let mut setup = CoreStmtBuilder::new();
                let exc = lower_semantic_expr_into(&mut setup, exc);
                builder.extend(finish_expr_setup(setup));
                exc
            });
            builder.set_term(CoreBlockPyTerm::Raise(BlockPyRaise { exc }));
        }
        BlockPyTerm::TryJump(try_jump) => {
            builder.set_term(CoreBlockPyTerm::TryJump(try_jump.clone()))
        }
        BlockPyTerm::Return(value) => {
            let value = value.as_ref().map(|value| {
                let mut setup = CoreStmtBuilder::new();
                let value = lower_semantic_expr_into(&mut setup, value);
                builder.extend(finish_expr_setup(setup));
                value
            });
            builder.set_term(CoreBlockPyTerm::Return(value));
        }
    }
}

fn lower_semantic_block(block: &SemanticBlockPyBlock) -> CoreBlockPyBlock {
    let fragment = lower_semantic_stmt_fragment(&BlockPyCfgFragment {
        body: block.body.clone(),
        term: Some(block.term.clone()),
    });
    CoreBlockPyBlock {
        label: block.label.clone(),
        body: fragment.body,
        term: fragment
            .term
            .expect("semantic BlockPy block must lower to a core terminator"),
        meta: BlockPyBlockMeta {
            exc_param: block.meta.exc_param.clone(),
        },
    }
}

pub(crate) fn simplify_blockpy_callable_def_exprs(
    callable_def: &SemanticBlockPyCallableDef,
) -> CoreBlockPyCallableDef {
    CoreBlockPyCallableDef {
        cfg: CfgCallableDef {
            function_id: callable_def.function_id,
            bind_name: callable_def.bind_name.clone(),
            display_name: callable_def.display_name.clone(),
            qualname: callable_def.qualname.clone(),
            kind: callable_def.kind,
            params: callable_def.params.clone(),
            entry_liveins: callable_def.entry_liveins.clone(),
            blocks: callable_def
                .blocks
                .iter()
                .map(lower_semantic_block)
                .collect(),
        },
        doc: callable_def
            .doc
            .as_ref()
            .map(lower_semantic_expr_without_setup),
        closure_layout: callable_def.closure_layout.clone(),
        local_cell_slots: callable_def.local_cell_slots.clone(),
    }
}

pub(crate) fn simplify_blockpy_module_exprs(module: &SemanticBlockPyModule) -> CoreBlockPyModule {
    CoreBlockPyModule {
        module_init: module.module_init.clone(),
        callable_defs: module
            .callable_defs
            .iter()
            .map(simplify_blockpy_callable_def_exprs)
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::simplify_blockpy_module_exprs;
    use crate::basic_block::block_py::pretty::blockpy_module_to_string;
    use crate::basic_block::block_py::{CoreBlockPyCallArg, CoreBlockPyExpr, SemanticBlockPyExpr};
    use crate::py_expr;
    use crate::{transform_str_to_ruff_with_options, Options};

    #[test]
    fn expr_simplify_preserves_control_flow_but_reduces_exprs() {
        let blockpy = transform_str_to_ruff_with_options(
            r#"
def f(x):
    if x:
        return 1
    return 2
"#,
            Options::for_test(),
        )
        .unwrap()
        .blockpy_module
        .expect("expected BlockPy module");
        let core = simplify_blockpy_module_exprs(&blockpy);
        let semantic_rendered = blockpy_module_to_string(&blockpy);
        let core_rendered = blockpy_module_to_string(&core);

        assert!(semantic_rendered.contains("__dp__.NO_DEFAULT"));
        assert!(core_rendered.contains("__dp_getattr(__dp__, \"NO_DEFAULT\")"));
        assert!(semantic_rendered.contains("function f(x)"));
        assert!(core_rendered.contains("function f(x)"));
        assert!(semantic_rendered.contains("return 1"));
        assert!(core_rendered.contains("return 1"));
    }

    #[test]
    fn expr_simplify_recurses_bottom_up_for_operator_family() {
        let expr = SemanticBlockPyExpr::from(py_expr!("-(x + 1)"));
        let lowered = super::lower_semantic_expr_without_setup(&expr);

        let CoreBlockPyExpr::Call(outer) = lowered else {
            panic!("expected call-shaped core expr");
        };
        assert!(matches!(
            &*outer.func,
            CoreBlockPyExpr::Name(name) if name.id.as_str() == "__dp_neg"
        ));
        let [CoreBlockPyCallArg::Positional(CoreBlockPyExpr::Call(inner))] = &outer.args[..] else {
            panic!("expected __dp_neg to receive one lowered call arg");
        };
        assert!(matches!(
            &*inner.func,
            CoreBlockPyExpr::Name(name) if name.id.as_str() == "__dp_add"
        ));
    }
}
