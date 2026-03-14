use crate::basic_block::ast_to_ast::context::Context;
use crate::basic_block::block_py::{
    BlockPyStmtFragmentBuilder, SemanticBlockPyAssign, SemanticBlockPyBlock, SemanticBlockPyExpr,
    SemanticBlockPyIf, SemanticBlockPyRaise, SemanticBlockPyStmt, SemanticBlockPyStmtFragment,
    SemanticBlockPyTerm,
};
use crate::basic_block::ruff_to_blockpy::lower_stmts_to_blockpy_stmts_with_context;
use crate::basic_block::stmt_utils::flatten_stmt_boxes;
use crate::template::into_body;
use crate::transformer::{walk_expr, walk_stmt, Transformer};
use crate::{py_expr, py_stmt};
use ruff_python_ast::{Expr, Stmt};

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
    fn visit_stmt(&mut self, stmt: &mut Stmt) {
        if matches!(stmt, Stmt::FunctionDef(_) | Stmt::ClassDef(_)) {
            return;
        }
        let mut prefix = Vec::new();
        match stmt {
            Stmt::Expr(expr_stmt) => {
                if let Expr::Await(await_expr) = expr_stmt.value.as_ref() {
                    expr_stmt.value = Box::new(py_expr!(
                        "yield from __dp_await_iter({value:expr})",
                        value = *await_expr.value.clone(),
                    ));
                    self.rewritten_count += 1;
                } else {
                    self.hoist_awaits_in_expr(expr_stmt.value.as_mut(), &mut prefix);
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
                    self.hoist_awaits_in_expr(assign_stmt.value.as_mut(), &mut prefix);
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
                        self.hoist_awaits_in_expr(value.as_mut(), &mut prefix);
                    }
                }
            }
            _ => {}
        }
        if !prefix.is_empty() {
            prefix.push(stmt.clone());
            *stmt = into_body(prefix);
        }
        walk_stmt(self, stmt);
    }
}

pub(crate) fn lower_coroutine_awaits_to_yield_from(stmts: &mut [Box<Stmt>]) -> bool {
    let mut pass = AwaitToYieldFromPass::default();
    for stmt in stmts {
        pass.visit_stmt(stmt.as_mut());
    }
    pass.rewritten_count > 0
}

pub(crate) fn lower_coroutine_awaits_in_stmt(stmt: Stmt) -> Stmt {
    let mut stmts = vec![Box::new(stmt)];
    lower_coroutine_awaits_to_yield_from(&mut stmts);
    debug_assert_eq!(stmts.len(), 1, "await lowering should preserve stmt count");
    *stmts.pop().expect("one stmt should remain")
}

fn lower_coroutine_await_stmt_to_blockpy_fragment(
    context: &Context,
    stmt: Stmt,
) -> Result<SemanticBlockPyStmtFragment, String> {
    let lowered = lower_coroutine_awaits_in_stmt(stmt);
    let lowered_stmts = flatten_stmt_boxes(&[Box::new(lowered)])
        .into_iter()
        .map(|stmt| stmt.as_ref().clone())
        .collect::<Vec<_>>();
    lower_stmts_to_blockpy_stmts_with_context::<SemanticBlockPyExpr>(context, &lowered_stmts)
}

fn lower_coroutine_awaits_in_blockpy_fragment(
    context: &Context,
    fragment: SemanticBlockPyStmtFragment,
) -> Result<SemanticBlockPyStmtFragment, String> {
    let mut lowered = BlockPyStmtFragmentBuilder::<SemanticBlockPyExpr>::new();
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
    stmt: SemanticBlockPyStmt,
    out: &mut BlockPyStmtFragmentBuilder<SemanticBlockPyExpr>,
) -> Result<(), String> {
    match stmt {
        SemanticBlockPyStmt::Pass | SemanticBlockPyStmt::Delete(_) => {
            out.push_stmt(stmt);
            Ok(())
        }
        SemanticBlockPyStmt::Expr(expr) => {
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
        SemanticBlockPyStmt::Assign(SemanticBlockPyAssign { target, value }) => {
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
        SemanticBlockPyStmt::If(if_stmt) => {
            out.push_stmt(SemanticBlockPyStmt::If(SemanticBlockPyIf {
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
    term: SemanticBlockPyTerm,
    out: &mut BlockPyStmtFragmentBuilder<SemanticBlockPyExpr>,
) -> Result<(), String> {
    match term {
        SemanticBlockPyTerm::Return(Some(value)) => {
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
    blocks: Vec<SemanticBlockPyBlock>,
) -> Result<Vec<SemanticBlockPyBlock>, String> {
    blocks
        .into_iter()
        .map(|block| {
            let mut lowered = BlockPyStmtFragmentBuilder::<SemanticBlockPyExpr>::new();
            for stmt in block.body {
                lower_coroutine_awaits_in_blockpy_stmt_into(context, stmt, &mut lowered)?;
            }
            lower_coroutine_awaits_in_blockpy_term_into(context, block.term, &mut lowered)?;
            let lowered = lowered.finish();
            Ok(SemanticBlockPyBlock {
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

fn blockpy_expr_contains_await(expr: &SemanticBlockPyExpr) -> bool {
    let mut raw: Expr = expr.clone().into();
    let mut probe = BlockPyAwaitProbe::default();
    probe.visit_expr(&mut raw);
    probe.has_await
}

fn blockpy_fragment_contains_await(fragment: &SemanticBlockPyStmtFragment) -> bool {
    fragment.body.iter().any(blockpy_stmt_contains_await)
        || fragment
            .term
            .as_ref()
            .is_some_and(blockpy_term_contains_await)
}

fn blockpy_stmt_contains_await(stmt: &SemanticBlockPyStmt) -> bool {
    match stmt {
        SemanticBlockPyStmt::Pass | SemanticBlockPyStmt::Delete(_) => false,
        SemanticBlockPyStmt::Expr(expr) => blockpy_expr_contains_await(expr),
        SemanticBlockPyStmt::Assign(assign) => blockpy_expr_contains_await(&assign.value),
        SemanticBlockPyStmt::If(if_stmt) => {
            blockpy_expr_contains_await(&if_stmt.test)
                || blockpy_fragment_contains_await(&if_stmt.body)
                || blockpy_fragment_contains_await(&if_stmt.orelse)
        }
    }
}

fn blockpy_term_contains_await(term: &SemanticBlockPyTerm) -> bool {
    match term {
        SemanticBlockPyTerm::Jump(_) | SemanticBlockPyTerm::TryJump(_) => false,
        SemanticBlockPyTerm::IfTerm(if_term) => blockpy_expr_contains_await(&if_term.test),
        SemanticBlockPyTerm::BranchTable(branch) => blockpy_expr_contains_await(&branch.index),
        SemanticBlockPyTerm::Raise(SemanticBlockPyRaise { exc }) => {
            exc.as_ref().is_some_and(blockpy_expr_contains_await)
        }
        SemanticBlockPyTerm::Return(value) => {
            value.as_ref().is_some_and(blockpy_expr_contains_await)
        }
    }
}

pub(crate) fn blockpy_blocks_contain_await_exprs(blocks: &[SemanticBlockPyBlock]) -> bool {
    blocks.iter().any(|block| {
        block.body.iter().any(blockpy_stmt_contains_await)
            || blockpy_term_contains_await(&block.term)
    })
}

pub(crate) fn coroutine_generator_marker_stmt() -> Box<Stmt> {
    Box::new(py_stmt!("if False:\n    yield __dp_NONE"))
}
