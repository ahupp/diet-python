use crate::block_py::{
    BbStmt, BlockPyModule, BlockPyRaise, BlockPyTerm, CoreBlockPyCall, CoreBlockPyCallArg,
    CoreBlockPyExpr, CoreBlockPyKeywordArg, CoreBlockPyLiteral, CoreBytesLiteral, IntrinsicCall,
};
use crate::passes::trace::{instrument_bb_module_for_trace, parse_trace_env};
use crate::passes::PreparedBbBlockPyPass;
use ruff_python_ast::{self as ast, ExprName};
use ruff_text_size::TextRange;

pub fn normalize_bb_module_for_codegen(
    module: &BlockPyModule<PreparedBbBlockPyPass>,
) -> BlockPyModule<PreparedBbBlockPyPass> {
    let mut normalized = module.clone();
    if let Some(config) = parse_trace_env() {
        instrument_bb_module_for_trace(&mut normalized, &config);
    }
    let mut rewriter = CodegenExprNormalizer;
    for function in &mut normalized.callable_defs {
        for block in &mut function.blocks {
            for op in &mut block.body {
                match op {
                    BbStmt::Assign(assign) => rewrite_bb_expr(&mut rewriter, &mut assign.value),
                    BbStmt::Expr(expr) => rewrite_bb_expr(&mut rewriter, expr),
                    BbStmt::Delete(_) => {}
                }
            }
            rewrite_term_exprs(&mut rewriter, &mut block.term);
        }
    }
    normalized
}

fn rewrite_term_exprs(
    rewriter: &mut CodegenExprNormalizer,
    term: &mut BlockPyTerm<CoreBlockPyExpr>,
) {
    match term {
        BlockPyTerm::Jump(_) => {}
        BlockPyTerm::IfTerm(if_term) => rewrite_bb_expr(rewriter, &mut if_term.test),
        BlockPyTerm::BranchTable(branch) => rewrite_bb_expr(rewriter, &mut branch.index),
        BlockPyTerm::Raise(BlockPyRaise { exc }) => {
            if let Some(exc) = exc.as_mut() {
                rewrite_bb_expr(rewriter, exc);
            }
        }
        BlockPyTerm::Return(value) => rewrite_bb_expr(rewriter, value),
    }
}

fn rewrite_bb_expr(rewriter: &mut CodegenExprNormalizer, expr: &mut CoreBlockPyExpr) {
    rewriter.rewrite_expr(expr);
}

struct CodegenExprNormalizer;

impl CodegenExprNormalizer {
    fn rewrite_expr(&mut self, expr: &mut CoreBlockPyExpr) {
        match expr {
            CoreBlockPyExpr::Call(call) => {
                self.rewrite_expr(call.func.as_mut());
                rewrite_call_parts(self, &mut call.args, &mut call.keywords);
            }
            CoreBlockPyExpr::Intrinsic(IntrinsicCall { args, keywords, .. }) => {
                rewrite_call_parts(self, args, keywords);
            }
            CoreBlockPyExpr::Name(_) | CoreBlockPyExpr::Literal(_) => {}
        }

        match expr {
            CoreBlockPyExpr::Literal(CoreBlockPyLiteral::StringLiteral(node)) => {
                *expr = str_bytes_call_expr(node.value.as_bytes());
            }
            _ => {}
        }
    }
}

fn rewrite_call_parts(
    rewriter: &mut CodegenExprNormalizer,
    args: &mut [CoreBlockPyCallArg<CoreBlockPyExpr>],
    keywords: &mut [CoreBlockPyKeywordArg<CoreBlockPyExpr>],
) {
    for arg in args {
        rewriter.rewrite_expr(arg.expr_mut());
    }
    for keyword in keywords {
        rewriter.rewrite_expr(keyword.expr_mut());
    }
}

fn compat_node_index() -> ast::AtomicNodeIndex {
    ast::AtomicNodeIndex::default()
}

fn compat_range() -> TextRange {
    TextRange::default()
}

fn load_name(id: &str) -> ExprName {
    ExprName {
        id: id.into(),
        ctx: ast::ExprContext::Load,
        range: compat_range(),
        node_index: compat_node_index(),
    }
}

fn bytes_literal_expr(bytes: &[u8]) -> CoreBlockPyExpr {
    CoreBlockPyExpr::Literal(CoreBlockPyLiteral::BytesLiteral(CoreBytesLiteral {
        range: compat_range(),
        node_index: compat_node_index(),
        value: bytes.to_vec(),
    }))
}

fn helper_call_expr_with_meta(
    helper_name: &str,
    args: Vec<CoreBlockPyExpr>,
    (node_index, range): (ast::AtomicNodeIndex, TextRange),
) -> CoreBlockPyExpr {
    CoreBlockPyExpr::Call(CoreBlockPyCall {
        node_index,
        range,
        func: Box::new(CoreBlockPyExpr::Name(load_name(helper_name))),
        args: args
            .into_iter()
            .map(CoreBlockPyCallArg::Positional)
            .collect(),
        keywords: Vec::<CoreBlockPyKeywordArg<CoreBlockPyExpr>>::new(),
    })
}

fn helper_call_expr(helper_name: &str, args: Vec<CoreBlockPyExpr>) -> CoreBlockPyExpr {
    helper_call_expr_with_meta(helper_name, args, (compat_node_index(), compat_range()))
}

fn str_bytes_call_expr(bytes: &[u8]) -> CoreBlockPyExpr {
    helper_call_expr("str", vec![bytes_literal_expr(bytes)])
}

#[cfg(test)]
mod test;
