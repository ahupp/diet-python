use crate::basic_block::ast_to_ast::body::{body_from_suite, take_suite, Suite};
use crate::basic_block::ast_to_ast::context::Context;
use crate::basic_block::block_py::{
    BlockPyAssign, BlockPyBlock, BlockPyIf, BlockPyRaise, BlockPyStmt, BlockPyStmtFragment,
    BlockPyStmtFragmentBuilder, BlockPyTerm,
};
use crate::basic_block::ruff_to_blockpy::lower_stmts_to_blockpy_stmts_with_context;
use crate::transformer::{walk_expr, walk_stmt, Transformer};
use crate::{py_expr, py_stmt};
use ruff_python_ast::{self as ast, Expr, Stmt};
use std::mem::take;

#[derive(Default)]
struct AwaitToYieldFromPass {
    rewritten_count: usize,
    temp_counter: usize,
}

impl AwaitToYieldFromPass {
    fn fresh_tmp(&mut self) -> String {
        let name = format!("_dp_await_tmp_{}", self.temp_counter);
        self.temp_counter += 1;
        name
    }

    fn hoist_awaits_in_expr(&mut self, expr: &mut Expr, prefix: &mut Vec<Stmt>) {
        match expr {
            Expr::Await(await_expr) => {
                self.hoist_awaits_in_expr(await_expr.value.as_mut(), prefix);
                let tmp = self.fresh_tmp();
                prefix.push(py_stmt!(
                    "{tmp:id} = yield from __dp_await_iter({value:expr})",
                    tmp = tmp.as_str(),
                    value = *await_expr.value.clone(),
                ));
                *expr = py_expr!("{tmp:id}", tmp = tmp.as_str());
                self.rewritten_count += 1;
            }
            Expr::Call(call_expr) => {
                self.hoist_awaits_in_expr(call_expr.func.as_mut(), prefix);
                for arg in &mut call_expr.arguments.args {
                    self.hoist_awaits_in_expr(arg, prefix);
                }
                for keyword in &mut call_expr.arguments.keywords {
                    self.hoist_awaits_in_expr(&mut keyword.value, prefix);
                }
            }
            Expr::Attribute(attribute_expr) => {
                self.hoist_awaits_in_expr(attribute_expr.value.as_mut(), prefix);
            }
            Expr::Subscript(subscript_expr) => {
                self.hoist_awaits_in_expr(subscript_expr.value.as_mut(), prefix);
                self.hoist_awaits_in_expr(subscript_expr.slice.as_mut(), prefix);
            }
            Expr::UnaryOp(unary_expr) => {
                self.hoist_awaits_in_expr(unary_expr.operand.as_mut(), prefix);
            }
            Expr::BinOp(binop_expr) => {
                self.hoist_awaits_in_expr(binop_expr.left.as_mut(), prefix);
                self.hoist_awaits_in_expr(binop_expr.right.as_mut(), prefix);
            }
            Expr::List(list_expr) => {
                for item in &mut list_expr.elts {
                    self.hoist_awaits_in_expr(item, prefix);
                }
            }
            Expr::Tuple(tuple_expr) => {
                for item in &mut tuple_expr.elts {
                    self.hoist_awaits_in_expr(item, prefix);
                }
            }
            Expr::Set(set_expr) => {
                for item in &mut set_expr.elts {
                    self.hoist_awaits_in_expr(item, prefix);
                }
            }
            Expr::Dict(dict_expr) => {
                for item in &mut dict_expr.items {
                    if let Some(key_expr) = &mut item.key {
                        self.hoist_awaits_in_expr(key_expr, prefix);
                    }
                    self.hoist_awaits_in_expr(&mut item.value, prefix);
                }
            }
            Expr::Starred(starred_expr) => {
                self.hoist_awaits_in_expr(starred_expr.value.as_mut(), prefix);
            }
            _ => {}
        }
    }
}

impl Transformer for AwaitToYieldFromPass {
    fn visit_body(&mut self, body: &mut Suite) {
        let original = take(body);
        let mut rewritten = Vec::with_capacity(original.len());
        for mut stmt in original.into_iter().map(|stmt| *stmt) {
            let mut prefix = Vec::new();
            self.rewrite_stmt_head(&mut stmt, &mut prefix);
            rewritten.extend(prefix.into_iter().map(Box::new));
            walk_stmt(self, &mut stmt);
            rewritten.push(Box::new(stmt));
        }
        *body = rewritten;
    }

    fn visit_stmt(&mut self, stmt: &mut Stmt) {
        if matches!(stmt, Stmt::FunctionDef(_) | Stmt::ClassDef(_)) {
            return;
        }
        let mut prefix = Vec::new();
        self.rewrite_stmt_head(stmt, &mut prefix);
        debug_assert!(
            prefix.is_empty(),
            "await lowering statement splicing must happen via visit_body"
        );
        walk_stmt(self, stmt);
    }
}

impl AwaitToYieldFromPass {
    fn rewrite_stmt_head(&mut self, stmt: &mut Stmt, prefix: &mut Vec<Stmt>) {
        match stmt {
            Stmt::Expr(expr_stmt) => {
                if let Expr::Await(await_expr) = expr_stmt.value.as_ref() {
                    expr_stmt.value = Box::new(py_expr!(
                        "yield from __dp_await_iter({value:expr})",
                        value = *await_expr.value.clone(),
                    ));
                    self.rewritten_count += 1;
                } else {
                    self.hoist_awaits_in_expr(expr_stmt.value.as_mut(), prefix);
                }
            }
            Stmt::Assign(assign_stmt) => {
                if let Expr::Await(await_expr) = assign_stmt.value.as_ref() {
                    assign_stmt.value = Box::new(py_expr!(
                        "yield from __dp_await_iter({value:expr})",
                        value = *await_expr.value.clone(),
                    ));
                    self.rewritten_count += 1;
                } else {
                    self.hoist_awaits_in_expr(assign_stmt.value.as_mut(), prefix);
                }
            }
            Stmt::Return(return_stmt) => {
                if let Some(value) = return_stmt.value.as_mut() {
                    if let Expr::Await(await_expr) = value.as_ref() {
                        *value = Box::new(py_expr!(
                            "yield from __dp_await_iter({value:expr})",
                            value = *await_expr.value.clone(),
                        ));
                        self.rewritten_count += 1;
                    } else {
                        self.hoist_awaits_in_expr(value.as_mut(), prefix);
                    }
                }
            }
            _ => {}
        }
    }
}

pub(crate) fn lower_coroutine_awaits_to_yield_from(stmts: &mut Vec<Box<Stmt>>) -> bool {
    let mut pass = AwaitToYieldFromPass::default();
    let mut body = take(stmts);
    pass.visit_body(&mut body);
    *stmts = body;
    pass.rewritten_count > 0
}

pub(crate) fn lower_coroutine_awaits_in_stmt(stmt: Stmt) -> Vec<Stmt> {
    let mut stmts = vec![Box::new(stmt)];
    lower_coroutine_awaits_to_yield_from(&mut stmts);
    stmts.into_iter().map(|stmt| *stmt).collect()
}

fn lower_coroutine_await_stmt_to_blockpy_fragment(
    context: &Context,
    stmt: Stmt,
) -> Result<BlockPyStmtFragment<Expr>, String> {
    let lowered_stmts = lower_coroutine_awaits_in_stmt(stmt);
    lower_stmts_to_blockpy_stmts_with_context::<Expr>(context, &lowered_stmts)
}

fn lower_coroutine_awaits_in_blockpy_fragment(
    context: &Context,
    fragment: BlockPyStmtFragment<Expr>,
) -> Result<BlockPyStmtFragment<Expr>, String> {
    let mut lowered = BlockPyStmtFragmentBuilder::<Expr>::new();
    for stmt in fragment.body {
        lower_coroutine_awaits_in_blockpy_stmt_into(context, stmt, &mut lowered)?;
    }
    if let Some(term) = fragment.term {
        lower_coroutine_awaits_in_blockpy_term_into(context, term, &mut lowered)?;
    }
    Ok(lowered.finish())
}

fn lower_coroutine_awaits_in_blockpy_stmt_into(
    context: &Context,
    stmt: BlockPyStmt<Expr>,
    out: &mut BlockPyStmtFragmentBuilder<Expr>,
) -> Result<(), String> {
    match stmt {
        BlockPyStmt::Delete(_) => {
            out.push_stmt(stmt);
            Ok(())
        }
        BlockPyStmt::Expr(expr) => {
            let lowered = lower_coroutine_await_stmt_to_blockpy_fragment(
                context,
                py_stmt!("{value:expr}", value = Expr::from(expr)),
            )?;
            debug_assert!(
                lowered.term.is_none(),
                "expr await lowering should not produce a terminator"
            );
            out.extend(lowered.body);
            Ok(())
        }
        BlockPyStmt::Assign(BlockPyAssign { target, value }) => {
            let lowered = lower_coroutine_await_stmt_to_blockpy_fragment(
                context,
                py_stmt!(
                    "{target:id} = {value:expr}",
                    target = target.id.as_str(),
                    value = Expr::from(value),
                ),
            )?;
            debug_assert!(
                lowered.term.is_none(),
                "assign await lowering should not produce a terminator"
            );
            out.extend(lowered.body);
            Ok(())
        }
        BlockPyStmt::If(if_stmt) => {
            out.push_stmt(BlockPyStmt::If(BlockPyIf {
                test: if_stmt.test,
                body: lower_coroutine_awaits_in_blockpy_fragment(context, if_stmt.body)?,
                orelse: lower_coroutine_awaits_in_blockpy_fragment(context, if_stmt.orelse)?,
            }));
            Ok(())
        }
    }
}

fn lower_coroutine_awaits_in_blockpy_term_into(
    context: &Context,
    term: BlockPyTerm<Expr>,
    out: &mut BlockPyStmtFragmentBuilder<Expr>,
) -> Result<(), String> {
    match term {
        BlockPyTerm::Return(Some(value)) => {
            let lowered = lower_coroutine_await_stmt_to_blockpy_fragment(
                context,
                py_stmt!("return {value:expr}", value = Expr::from(value)),
            )?;
            out.extend(lowered.body);
            if let Some(term) = lowered.term {
                out.set_term(term);
            }
            Ok(())
        }
        other => {
            out.set_term(other);
            Ok(())
        }
    }
}

pub(crate) fn lower_coroutine_awaits_in_blockpy_blocks(
    context: &Context,
    blocks: Vec<BlockPyBlock<Expr>>,
) -> Result<Vec<BlockPyBlock<Expr>>, String> {
    blocks
        .into_iter()
        .map(|block| {
            let mut lowered = BlockPyStmtFragmentBuilder::<Expr>::new();
            for stmt in block.body {
                lower_coroutine_awaits_in_blockpy_stmt_into(context, stmt, &mut lowered)?;
            }
            lower_coroutine_awaits_in_blockpy_term_into(context, block.term, &mut lowered)?;
            let lowered = lowered.finish();
            Ok(BlockPyBlock {
                label: block.label,
                body: lowered.body,
                term: lowered
                    .term
                    .expect("await lowering should preserve block terminators"),
                meta: block.meta,
            })
        })
        .collect()
}

#[derive(Default)]
struct BlockPyAwaitProbe {
    has_await: bool,
}

impl Transformer for BlockPyAwaitProbe {
    fn visit_stmt(&mut self, stmt: &mut Stmt) {
        if matches!(stmt, Stmt::FunctionDef(_) | Stmt::ClassDef(_)) {
            return;
        }
        walk_stmt(self, stmt);
    }

    fn visit_expr(&mut self, expr: &mut Expr) {
        if matches!(expr, Expr::Await(_)) {
            self.has_await = true;
            return;
        }
        walk_expr(self, expr);
    }
}

fn blockpy_expr_contains_await(expr: &Expr) -> bool {
    let mut raw: Expr = expr.clone().into();
    let mut probe = BlockPyAwaitProbe::default();
    probe.visit_expr(&mut raw);
    probe.has_await
}

fn blockpy_fragment_contains_await(fragment: &BlockPyStmtFragment<Expr>) -> bool {
    fragment.body.iter().any(blockpy_stmt_contains_await)
        || fragment
            .term
            .as_ref()
            .is_some_and(blockpy_term_contains_await)
}

fn blockpy_stmt_contains_await(stmt: &BlockPyStmt<Expr>) -> bool {
    match stmt {
        BlockPyStmt::Delete(_) => false,
        BlockPyStmt::Expr(expr) => blockpy_expr_contains_await(expr),
        BlockPyStmt::Assign(assign) => blockpy_expr_contains_await(&assign.value),
        BlockPyStmt::If(if_stmt) => {
            blockpy_expr_contains_await(&if_stmt.test)
                || blockpy_fragment_contains_await(&if_stmt.body)
                || blockpy_fragment_contains_await(&if_stmt.orelse)
        }
    }
}

fn blockpy_term_contains_await(term: &BlockPyTerm<Expr>) -> bool {
    match term {
        BlockPyTerm::Jump(_) | BlockPyTerm::TryJump(_) => false,
        BlockPyTerm::IfTerm(if_term) => blockpy_expr_contains_await(&if_term.test),
        BlockPyTerm::BranchTable(branch) => blockpy_expr_contains_await(&branch.index),
        BlockPyTerm::Raise(BlockPyRaise { exc }) => {
            exc.as_ref().is_some_and(blockpy_expr_contains_await)
        }
        BlockPyTerm::Return(value) => value.as_ref().is_some_and(blockpy_expr_contains_await),
    }
}

pub(crate) fn blockpy_blocks_contain_await_exprs(blocks: &[BlockPyBlock<Expr>]) -> bool {
    blocks.iter().any(|block| {
        block.body.iter().any(blockpy_stmt_contains_await)
            || blockpy_term_contains_await(&block.term)
    })
}
