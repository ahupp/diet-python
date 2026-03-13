use super::{
    BlockPyBlockMeta, BlockPyBranchTable, BlockPyCfgFragment, BlockPyDelete, BlockPyIf,
    BlockPyIfTerm, BlockPyRaise, BlockPyStructuredIf, BlockPyTerm, CoreBlockPyAssign,
    CoreBlockPyBlock, CoreBlockPyCallableDef, CoreBlockPyExpr, CoreBlockPyModule, CoreBlockPyStmt,
    CoreBlockPyStmtFragment, CoreBlockPyTerm, SemanticBlockPyAssign, SemanticBlockPyBlock,
    SemanticBlockPyCallableDef, SemanticBlockPyModule, SemanticBlockPyStmt,
};
use crate::basic_block::cfg_ir::CfgCallableDef;

fn lower_semantic_expr(expr: &super::BlockPyExpr) -> CoreBlockPyExpr {
    expr.clone().into()
}

fn lower_semantic_stmt_fragment(fragment: &CoreLikeStmtFragmentInput) -> CoreBlockPyStmtFragment {
    BlockPyCfgFragment {
        body: fragment.body.iter().map(lower_semantic_stmt).collect(),
        term: fragment.term.as_ref().map(lower_semantic_term),
    }
}

type CoreLikeStmtFragmentInput = super::SemanticBlockPyStmtFragment;

fn lower_semantic_stmt(stmt: &SemanticBlockPyStmt) -> CoreBlockPyStmt {
    match stmt {
        SemanticBlockPyStmt::Pass => CoreBlockPyStmt::Pass,
        SemanticBlockPyStmt::Assign(assign) => CoreBlockPyStmt::Assign(CoreBlockPyAssign {
            target: assign.target.clone(),
            value: lower_semantic_expr(&assign.value),
        }),
        SemanticBlockPyStmt::Expr(expr) => CoreBlockPyStmt::Expr(lower_semantic_expr(expr)),
        SemanticBlockPyStmt::Delete(BlockPyDelete { target }) => {
            CoreBlockPyStmt::Delete(BlockPyDelete {
                target: target.clone(),
            })
        }
        SemanticBlockPyStmt::If(if_stmt) => CoreBlockPyStmt::If(lower_semantic_if(if_stmt)),
    }
}

fn lower_semantic_if(
    if_stmt: &BlockPyStructuredIf<super::BlockPyExpr>,
) -> BlockPyIf<CoreBlockPyExpr, CoreBlockPyStmt, CoreBlockPyTerm> {
    BlockPyIf {
        test: lower_semantic_expr(&if_stmt.test),
        body: lower_semantic_stmt_fragment(&if_stmt.body),
        orelse: lower_semantic_stmt_fragment(&if_stmt.orelse),
    }
}

fn lower_semantic_term(term: &BlockPyTerm<super::BlockPyExpr>) -> CoreBlockPyTerm {
    match term {
        BlockPyTerm::Jump(label) => CoreBlockPyTerm::Jump(label.clone()),
        BlockPyTerm::IfTerm(BlockPyIfTerm {
            test,
            then_label,
            else_label,
        }) => CoreBlockPyTerm::IfTerm(BlockPyIfTerm {
            test: lower_semantic_expr(test),
            then_label: then_label.clone(),
            else_label: else_label.clone(),
        }),
        BlockPyTerm::BranchTable(BlockPyBranchTable {
            index,
            targets,
            default_label,
        }) => CoreBlockPyTerm::BranchTable(BlockPyBranchTable {
            index: lower_semantic_expr(index),
            targets: targets.clone(),
            default_label: default_label.clone(),
        }),
        BlockPyTerm::Raise(BlockPyRaise { exc }) => CoreBlockPyTerm::Raise(BlockPyRaise {
            exc: exc.as_ref().map(lower_semantic_expr),
        }),
        BlockPyTerm::TryJump(try_jump) => CoreBlockPyTerm::TryJump(try_jump.clone()),
        BlockPyTerm::Return(value) => {
            CoreBlockPyTerm::Return(value.as_ref().map(lower_semantic_expr))
        }
    }
}

fn lower_semantic_block(block: &SemanticBlockPyBlock) -> CoreBlockPyBlock {
    CoreBlockPyBlock {
        label: block.label.clone(),
        body: block.body.iter().map(lower_semantic_stmt).collect(),
        term: lower_semantic_term(&block.term),
        meta: BlockPyBlockMeta {
            exc_param: block.meta.exc_param.clone(),
        },
    }
}

fn lower_semantic_callable_def(
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
        doc: callable_def.doc.as_ref().map(lower_semantic_expr),
        closure_layout: callable_def.closure_layout.clone(),
        local_cell_slots: callable_def.local_cell_slots.clone(),
    }
}

pub(crate) fn lower_semantic_blockpy_module_to_core(
    module: &SemanticBlockPyModule,
) -> CoreBlockPyModule {
    CoreBlockPyModule {
        module_init: module.module_init.clone(),
        callable_defs: module
            .callable_defs
            .iter()
            .map(lower_semantic_callable_def)
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::lower_semantic_blockpy_module_to_core;
    use crate::basic_block::block_py::pretty::blockpy_module_to_string;
    use crate::{transform_str_to_ruff_with_options, Options};

    #[test]
    fn lowering_semantic_blockpy_to_core_preserves_rendering() {
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
        let core = lower_semantic_blockpy_module_to_core(&blockpy);

        assert_eq!(
            blockpy_module_to_string(&blockpy),
            blockpy_module_to_string(&core)
        );
    }
}
