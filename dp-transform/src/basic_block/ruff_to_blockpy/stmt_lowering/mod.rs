use super::*;
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
    stmt: &T,
    out: &mut BlockPyStmtFragmentBuilder<E>,
    loop_ctx: Option<&LoopContext>,
    next_label_id: &mut usize,
) -> Result<(), String>
where
    T: StmtLowerer + Clone,
    E: From<Expr> + std::fmt::Debug,
{
    let simplified = stmt.clone().simplify_ast();
    lower_stmt_into_with_expr(&simplified, out, loop_ctx, next_label_id)
}

pub(super) trait StmtLowerer {
    fn simplify_ast(self) -> Stmt
    where
        Self: Sized;

    fn to_blockpy<E>(
        &self,
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

macro_rules! impl_identity_stmt_lowerer {
    ($ty:path, $variant:path) => {
        impl StmtLowerer for $ty {
            fn simplify_ast(self) -> Stmt {
                $variant(self)
            }
        }
    };
}

macro_rules! impl_unreduced_stmt_lowerer {
    ($ty:path, $variant:path, $message:literal) => {
        impl StmtLowerer for $ty {
            fn simplify_ast(self) -> Stmt {
                $variant(self)
            }

            fn to_blockpy<E>(
                &self,
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
mod delete_stmt;
mod direct;
mod if_stmt;
mod import_stmt;
mod try_stmt;
mod unreduced;
mod with_stmt;

pub(crate) use assign_stmt::build_for_target_assign_body;
pub(crate) use try_stmt::{lower_star_try_stmt_sequence, lower_try_stmt_sequence};
pub(crate) use with_stmt::lower_with_stmt_sequence;

pub(super) fn simplify_stmt_ast_for_blockpy(stmt: Stmt) -> Stmt {
    match stmt {
        Stmt::BodyStmt(body) => body.simplify_ast(),
        Stmt::Global(stmt) => stmt.simplify_ast(),
        Stmt::Nonlocal(stmt) => stmt.simplify_ast(),
        Stmt::Pass(stmt) => stmt.simplify_ast(),
        Stmt::Expr(stmt) => stmt.simplify_ast(),
        Stmt::Assign(stmt) => stmt.simplify_ast(),
        Stmt::Delete(stmt) => stmt.simplify_ast(),
        Stmt::FunctionDef(stmt) => stmt.simplify_ast(),
        Stmt::ClassDef(stmt) => stmt.simplify_ast(),
        Stmt::TypeAlias(stmt) => stmt.simplify_ast(),
        Stmt::AugAssign(stmt) => stmt.simplify_ast(),
        Stmt::AnnAssign(stmt) => stmt.simplify_ast(),
        Stmt::If(stmt) => stmt.simplify_ast(),
        Stmt::While(stmt) => stmt.simplify_ast(),
        Stmt::For(stmt) => stmt.simplify_ast(),
        Stmt::With(stmt) => stmt.simplify_ast(),
        Stmt::Match(stmt) => stmt.simplify_ast(),
        Stmt::Assert(stmt) => stmt.simplify_ast(),
        Stmt::Import(stmt) => stmt.simplify_ast(),
        Stmt::ImportFrom(stmt) => stmt.simplify_ast(),
        Stmt::Break(stmt) => stmt.simplify_ast(),
        Stmt::Continue(stmt) => stmt.simplify_ast(),
        Stmt::Return(stmt) => stmt.simplify_ast(),
        Stmt::Raise(stmt) => stmt.simplify_ast(),
        Stmt::Try(stmt) => stmt.simplify_ast(),
        Stmt::IpyEscapeCommand(stmt) => stmt.simplify_ast(),
    }
}

pub(crate) fn lower_stmt_into(
    stmt: &Stmt,
    out: &mut crate::basic_block::block_py::BlockPyCfgFragmentBuilder<BlockPyStmt, BlockPyTerm>,
    loop_ctx: Option<&LoopContext>,
    next_label_id: &mut usize,
) -> Result<(), String> {
    lower_stmt_into_with_expr(stmt, out, loop_ctx, next_label_id)
}

pub(crate) fn lower_stmt_into_with_expr<E>(
    stmt: &Stmt,
    out: &mut BlockPyStmtFragmentBuilder<E>,
    loop_ctx: Option<&LoopContext>,
    next_label_id: &mut usize,
) -> Result<(), String>
where
    E: From<Expr> + std::fmt::Debug,
{
    match stmt {
        Stmt::BodyStmt(body) => body.to_blockpy(out, loop_ctx, next_label_id),
        Stmt::Global(stmt) => stmt.to_blockpy(out, loop_ctx, next_label_id),
        Stmt::Nonlocal(stmt) => stmt.to_blockpy(out, loop_ctx, next_label_id),
        Stmt::Pass(stmt) => stmt.to_blockpy(out, loop_ctx, next_label_id),
        Stmt::Expr(stmt) => stmt.to_blockpy(out, loop_ctx, next_label_id),
        Stmt::Assign(stmt) => stmt.to_blockpy(out, loop_ctx, next_label_id),
        Stmt::Delete(stmt) => stmt.to_blockpy(out, loop_ctx, next_label_id),
        Stmt::FunctionDef(stmt) => stmt.to_blockpy(out, loop_ctx, next_label_id),
        Stmt::ClassDef(stmt) => stmt.to_blockpy(out, loop_ctx, next_label_id),
        Stmt::TypeAlias(stmt) => stmt.to_blockpy(out, loop_ctx, next_label_id),
        Stmt::AugAssign(stmt) => stmt.to_blockpy(out, loop_ctx, next_label_id),
        Stmt::AnnAssign(stmt) => stmt.to_blockpy(out, loop_ctx, next_label_id),
        Stmt::If(stmt) => stmt.to_blockpy(out, loop_ctx, next_label_id),
        Stmt::While(stmt) => stmt.to_blockpy(out, loop_ctx, next_label_id),
        Stmt::For(stmt) => stmt.to_blockpy(out, loop_ctx, next_label_id),
        Stmt::With(stmt) => stmt.to_blockpy(out, loop_ctx, next_label_id),
        Stmt::Match(stmt) => stmt.to_blockpy(out, loop_ctx, next_label_id),
        Stmt::Assert(stmt) => stmt.to_blockpy(out, loop_ctx, next_label_id),
        Stmt::Import(stmt) => stmt.to_blockpy(out, loop_ctx, next_label_id),
        Stmt::ImportFrom(stmt) => stmt.to_blockpy(out, loop_ctx, next_label_id),
        Stmt::Break(stmt) => stmt.to_blockpy(out, loop_ctx, next_label_id),
        Stmt::Continue(stmt) => stmt.to_blockpy(out, loop_ctx, next_label_id),
        Stmt::Return(stmt) => stmt.to_blockpy(out, loop_ctx, next_label_id),
        Stmt::Raise(stmt) => stmt.to_blockpy(out, loop_ctx, next_label_id),
        Stmt::Try(stmt) => stmt.to_blockpy(out, loop_ctx, next_label_id),
        Stmt::IpyEscapeCommand(stmt) => stmt.to_blockpy(out, loop_ctx, next_label_id),
        other => {
            return Err(format!(
                "unsupported statement reached Ruff AST -> BlockPy conversion: {}\nstmt:\n{}",
                stmt_kind_name(other),
                ruff_ast_to_string(other).trim_end()
            ));
        }
    }?;
    Ok(())
}
