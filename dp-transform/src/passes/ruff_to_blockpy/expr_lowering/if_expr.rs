use super::BlockPySetupExprLowerer;
use crate::block_py::{BlockPyAssign, BlockPyIf, BlockPyStmt, BlockPyStmtFragmentBuilder};
use crate::passes::ruff_to_blockpy::expr_lowering::fresh_setup_name;
use crate::passes::ruff_to_blockpy::LoopContext;
use crate::py_expr;
use ruff_python_ast::{self as ast, Expr};

fn store_name(name: &str) -> ast::ExprName {
    ast::ExprName {
        id: name.into(),
        ctx: ast::ExprContext::Store,
        range: Default::default(),
        node_index: ast::AtomicNodeIndex::default(),
    }
}

fn load_name(name: &str) -> Expr {
    py_expr!("{name:id}", name = name)
}

fn assign_name<E>(target: &str, value: Expr) -> BlockPyStmt<E>
where
    E: From<Expr>,
{
    BlockPyStmt::Assign(BlockPyAssign {
        target: store_name(target),
        value: value.into(),
    })
}

pub(super) fn lower_if_expr_into<L, E>(
    lowerer: &L,
    if_expr: ast::ExprIf,
    out: &mut BlockPyStmtFragmentBuilder<E>,
    loop_ctx: Option<&LoopContext>,
    next_label_id: &mut usize,
) -> Result<Expr, String>
where
    L: BlockPySetupExprLowerer + ?Sized,
    E: From<Expr> + std::fmt::Debug,
{
    let ast::ExprIf {
        test, body, orelse, ..
    } = if_expr;
    let target = fresh_setup_name("tmp");
    let test = lowerer.lower_expr_ast_into(*test, out, loop_ctx, next_label_id)?;

    let mut body_out = BlockPyStmtFragmentBuilder::<E>::new();
    let body_value = lowerer.lower_expr_ast_into(*body, &mut body_out, loop_ctx, next_label_id)?;
    body_out.push_stmt(assign_name(&target, body_value));

    let mut orelse_out = BlockPyStmtFragmentBuilder::<E>::new();
    let orelse_value =
        lowerer.lower_expr_ast_into(*orelse, &mut orelse_out, loop_ctx, next_label_id)?;
    orelse_out.push_stmt(assign_name(&target, orelse_value));

    out.push_stmt(BlockPyStmt::If(BlockPyIf {
        test: test.into(),
        body: body_out.finish(),
        orelse: orelse_out.finish(),
    }));
    Ok(load_name(&target))
}

#[cfg(test)]
mod tests {
    use crate::block_py::{BlockPyStmt, BlockPyStmtFragmentBuilder};
    use crate::passes::ruff_to_blockpy::expr_lowering::lower_expr_into_with_setup;
    use crate::py_expr;
    use ruff_python_ast::Expr;

    #[test]
    fn if_expr_lowering_emits_blockpy_setup_directly() {
        let mut out = BlockPyStmtFragmentBuilder::<Expr>::new();
        let mut next_label_id = 0usize;

        let lowered = lower_expr_into_with_setup(
            py_expr!("a if cond else b"),
            &mut out,
            None,
            &mut next_label_id,
        )
        .expect("expr lowering should succeed");

        let fragment = out.finish();
        assert!(matches!(lowered, Expr::Name(_)), "{lowered:?}");
        let [BlockPyStmt::If(if_stmt)] = &fragment.body[..] else {
            panic!("expected one structured if stmt, got {fragment:?}");
        };
        assert!(
            if_stmt
                .body
                .body
                .iter()
                .any(|stmt| matches!(stmt, BlockPyStmt::Assign(_))),
            "{if_stmt:?}"
        );
        assert!(
            if_stmt
                .orelse
                .body
                .iter()
                .any(|stmt| matches!(stmt, BlockPyStmt::Assign(_))),
            "{if_stmt:?}"
        );
    }
}
