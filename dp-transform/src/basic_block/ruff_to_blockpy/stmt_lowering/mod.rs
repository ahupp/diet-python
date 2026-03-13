use super::*;
use crate::basic_block::ast_to_ast::context::Context;
use crate::basic_block::block_py::{
    BlockPyAssign, BlockPyDelete, BlockPyExpr, BlockPyIf, BlockPyRaise, BlockPyStmt, BlockPyTerm,
    SemanticBlockPyBlock as BlockPyBlock,
};

pub(super) type BlockPyStmtFragmentBuilder<E> =
    crate::basic_block::block_py::BlockPyCfgFragmentBuilder<BlockPyStmt<E>, BlockPyTerm<E>>;

pub(super) fn stmt_from_rewrite(
    rewrite: crate::basic_block::ast_to_ast::ast_rewrite::Rewrite,
) -> Stmt {
    match rewrite {
        crate::basic_block::ast_to_ast::ast_rewrite::Rewrite::Unmodified(stmt)
        | crate::basic_block::ast_to_ast::ast_rewrite::Rewrite::Walk(stmt) => stmt,
    }
}

pub(super) fn lower_stmt_via_simplify<T, E>(
    context: &Context,
    stmt: &T,
    out: &mut BlockPyStmtFragmentBuilder<E>,
    loop_ctx: Option<&LoopContext>,
    next_label_id: &mut usize,
) -> Result<(), String>
where
    T: StmtLowerer + Clone,
    E: From<Expr> + std::fmt::Debug,
{
    let simplified = stmt.clone().simplify_ast(context);
    lower_stmt_into_with_expr(context, &simplified, out, loop_ctx, next_label_id)
}

pub(super) trait StmtLowerer {
    fn simplify_ast(self, _context: &Context) -> Stmt
    where
        Self: Sized;

    fn to_blockpy<E>(
        &self,
        _context: &Context,
        _out: &mut BlockPyStmtFragmentBuilder<E>,
        _loop_ctx: Option<&LoopContext>,
        _next_label_id: &mut usize,
    ) -> Result<(), String>
    where
        E: From<Expr> + std::fmt::Debug,
    {
        panic!(
            "{} should have already been reduced before BlockPy lowering",
            std::any::type_name::<Self>()
        );
    }
}

macro_rules! impl_unreduced_stmt_lowerer {
    ($ty:path, $variant:path, $message:literal) => {
        impl StmtLowerer for $ty {
            fn simplify_ast(self, _context: &Context) -> Stmt {
                $variant(self)
            }

            fn to_blockpy<E>(
                &self,
                _context: &Context,
                _out: &mut BlockPyStmtFragmentBuilder<E>,
                _loop_ctx: Option<&LoopContext>,
                _next_label_id: &mut usize,
            ) -> Result<(), String>
            where
                E: From<Expr> + std::fmt::Debug,
            {
                panic!($message);
            }
        }
    };
}

mod assert_stmt;
mod assign_stmt;
mod augassign_stmt;
mod delete_stmt;
mod direct;
mod if_stmt;
mod import_from_stmt;
mod import_stmt;
mod match_stmt;
mod try_stmt;
mod type_alias_stmt;
mod unreduced;
mod with_stmt;

pub(crate) use assert_stmt::rewrite_assert_stmt;
pub(crate) use assign_stmt::{
    build_for_target_assign_body, rewrite_assign_stmt, rewrite_augassign_stmt,
    should_rewrite_assignment_targets,
};
pub(crate) use delete_stmt::rewrite_delete_stmt;
pub(crate) use direct::rewrite_raise_stmt;
pub(crate) use if_stmt::expand_if_chain;
pub(crate) use match_stmt::rewrite_match_stmt;
pub(crate) use try_stmt::{lower_star_try_stmt_sequence, lower_try_stmt_sequence};
pub(crate) use type_alias_stmt::rewrite_type_alias_stmt;
pub(crate) use with_stmt::lower_with_stmt_sequence;

pub(super) fn simplify_stmt_ast_for_blockpy(context: &Context, stmt: Stmt) -> Stmt {
    match stmt {
        Stmt::BodyStmt(body) => body.simplify_ast(context),
        Stmt::Global(stmt) => stmt.simplify_ast(context),
        Stmt::Nonlocal(stmt) => stmt.simplify_ast(context),
        Stmt::Pass(stmt) => stmt.simplify_ast(context),
        Stmt::Expr(stmt) => stmt.simplify_ast(context),
        Stmt::Assign(stmt) => stmt.simplify_ast(context),
        Stmt::Delete(stmt) => stmt.simplify_ast(context),
        Stmt::FunctionDef(stmt) => stmt.simplify_ast(context),
        Stmt::ClassDef(stmt) => stmt.simplify_ast(context),
        Stmt::TypeAlias(stmt) => stmt.simplify_ast(context),
        Stmt::AugAssign(stmt) => stmt.simplify_ast(context),
        Stmt::AnnAssign(stmt) => stmt.simplify_ast(context),
        Stmt::If(stmt) => stmt.simplify_ast(context),
        Stmt::While(stmt) => stmt.simplify_ast(context),
        Stmt::For(stmt) => stmt.simplify_ast(context),
        Stmt::With(stmt) => stmt.simplify_ast(context),
        Stmt::Match(stmt) => stmt.simplify_ast(context),
        Stmt::Assert(stmt) => stmt.simplify_ast(context),
        Stmt::Import(stmt) => stmt.simplify_ast(context),
        Stmt::ImportFrom(stmt) => stmt.simplify_ast(context),
        Stmt::Break(stmt) => stmt.simplify_ast(context),
        Stmt::Continue(stmt) => stmt.simplify_ast(context),
        Stmt::Return(stmt) => stmt.simplify_ast(context),
        Stmt::Raise(stmt) => stmt.simplify_ast(context),
        Stmt::Try(stmt) => stmt.simplify_ast(context),
        Stmt::IpyEscapeCommand(stmt) => stmt.simplify_ast(context),
    }
}

pub(crate) fn lower_stmt_into(
    context: &Context,
    stmt: &Stmt,
    out: &mut crate::basic_block::block_py::BlockPyCfgFragmentBuilder<BlockPyStmt, BlockPyTerm>,
    loop_ctx: Option<&LoopContext>,
    next_label_id: &mut usize,
) -> Result<(), String> {
    lower_stmt_into_with_expr(context, stmt, out, loop_ctx, next_label_id)
}

pub(crate) fn lower_stmt_into_with_expr<E>(
    context: &Context,
    stmt: &Stmt,
    out: &mut BlockPyStmtFragmentBuilder<E>,
    loop_ctx: Option<&LoopContext>,
    next_label_id: &mut usize,
) -> Result<(), String>
where
    E: From<Expr> + std::fmt::Debug,
{
    match stmt {
        Stmt::BodyStmt(body) => body.to_blockpy(context, out, loop_ctx, next_label_id),
        Stmt::Global(stmt) => stmt.to_blockpy(context, out, loop_ctx, next_label_id),
        Stmt::Nonlocal(stmt) => stmt.to_blockpy(context, out, loop_ctx, next_label_id),
        Stmt::Pass(stmt) => stmt.to_blockpy(context, out, loop_ctx, next_label_id),
        Stmt::Expr(stmt) => stmt.to_blockpy(context, out, loop_ctx, next_label_id),
        Stmt::Assign(stmt) => stmt.to_blockpy(context, out, loop_ctx, next_label_id),
        Stmt::Delete(stmt) => stmt.to_blockpy(context, out, loop_ctx, next_label_id),
        Stmt::FunctionDef(stmt) => stmt.to_blockpy(context, out, loop_ctx, next_label_id),
        Stmt::ClassDef(stmt) => stmt.to_blockpy(context, out, loop_ctx, next_label_id),
        Stmt::TypeAlias(stmt) => stmt.to_blockpy(context, out, loop_ctx, next_label_id),
        Stmt::AugAssign(stmt) => stmt.to_blockpy(context, out, loop_ctx, next_label_id),
        Stmt::AnnAssign(stmt) => stmt.to_blockpy(context, out, loop_ctx, next_label_id),
        Stmt::If(stmt) => stmt.to_blockpy(context, out, loop_ctx, next_label_id),
        Stmt::While(stmt) => stmt.to_blockpy(context, out, loop_ctx, next_label_id),
        Stmt::For(stmt) => stmt.to_blockpy(context, out, loop_ctx, next_label_id),
        Stmt::With(stmt) => stmt.to_blockpy(context, out, loop_ctx, next_label_id),
        Stmt::Match(stmt) => stmt.to_blockpy(context, out, loop_ctx, next_label_id),
        Stmt::Assert(stmt) => stmt.to_blockpy(context, out, loop_ctx, next_label_id),
        Stmt::Import(stmt) => stmt.to_blockpy(context, out, loop_ctx, next_label_id),
        Stmt::ImportFrom(stmt) => stmt.to_blockpy(context, out, loop_ctx, next_label_id),
        Stmt::Break(stmt) => stmt.to_blockpy(context, out, loop_ctx, next_label_id),
        Stmt::Continue(stmt) => stmt.to_blockpy(context, out, loop_ctx, next_label_id),
        Stmt::Return(stmt) => stmt.to_blockpy(context, out, loop_ctx, next_label_id),
        Stmt::Raise(stmt) => stmt.to_blockpy(context, out, loop_ctx, next_label_id),
        Stmt::Try(stmt) => stmt.to_blockpy(context, out, loop_ctx, next_label_id),
        Stmt::IpyEscapeCommand(stmt) => stmt.to_blockpy(context, out, loop_ctx, next_label_id),
    }?;
    Ok(())
}
