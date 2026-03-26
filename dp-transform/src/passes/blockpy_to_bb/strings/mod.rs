use crate::block_py::{
    BbStmt, BlockPyModule, BlockPyRaise, BlockPyTerm, CoreBlockPyCall, CoreBlockPyCallArg,
    CoreBlockPyKeywordArg, CoreBlockPyLiteral, CoreBytesLiteral, IntrinsicCall,
    LocatedCoreBlockPyExpr, LocatedName, NameLocation,
};
use crate::passes::PreparedBbBlockPyPass;
use ruff_python_ast::{self as ast};
use ruff_text_size::TextRange;

pub fn normalize_bb_module_strings(
    module: &BlockPyModule<PreparedBbBlockPyPass>,
) -> BlockPyModule<PreparedBbBlockPyPass> {
    let mut normalized = module.clone();
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
    term: &mut BlockPyTerm<LocatedCoreBlockPyExpr>,
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

fn rewrite_bb_expr(rewriter: &mut CodegenExprNormalizer, expr: &mut LocatedCoreBlockPyExpr) {
    rewriter.rewrite_expr(expr);
}

struct CodegenExprNormalizer;

impl CodegenExprNormalizer {
    fn rewrite_expr(&mut self, expr: &mut LocatedCoreBlockPyExpr) {
        match expr {
            LocatedCoreBlockPyExpr::Call(call) => {
                self.rewrite_expr(call.func.as_mut());
                rewrite_call_parts(self, &mut call.args, &mut call.keywords);
            }
            LocatedCoreBlockPyExpr::Intrinsic(IntrinsicCall { args, .. }) => {
                for arg in args {
                    self.rewrite_expr(arg);
                }
            }
            LocatedCoreBlockPyExpr::Name(_) | LocatedCoreBlockPyExpr::Literal(_) => {}
        }

        match expr {
            LocatedCoreBlockPyExpr::Literal(CoreBlockPyLiteral::StringLiteral(node)) => {
                *expr = str_bytes_call_expr(node.value.as_bytes());
            }
            _ => {}
        }
    }
}

fn rewrite_call_parts(
    rewriter: &mut CodegenExprNormalizer,
    args: &mut [CoreBlockPyCallArg<LocatedCoreBlockPyExpr>],
    keywords: &mut [CoreBlockPyKeywordArg<LocatedCoreBlockPyExpr>],
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

fn load_name(id: &str) -> LocatedName {
    LocatedName {
        id: id.into(),
        ctx: ast::ExprContext::Load,
        range: compat_range(),
        node_index: compat_node_index(),
        location: NameLocation::Global,
    }
}

fn bytes_literal_expr(bytes: &[u8]) -> LocatedCoreBlockPyExpr {
    LocatedCoreBlockPyExpr::Literal(CoreBlockPyLiteral::BytesLiteral(CoreBytesLiteral {
        range: compat_range(),
        node_index: compat_node_index(),
        value: bytes.to_vec(),
    }))
}

fn helper_call_expr_with_meta(
    helper_name: &str,
    args: Vec<LocatedCoreBlockPyExpr>,
    (node_index, range): (ast::AtomicNodeIndex, TextRange),
) -> LocatedCoreBlockPyExpr {
    LocatedCoreBlockPyExpr::Call(CoreBlockPyCall {
        node_index,
        range,
        func: Box::new(LocatedCoreBlockPyExpr::Name(load_name(helper_name))),
        args: args
            .into_iter()
            .map(CoreBlockPyCallArg::Positional)
            .collect(),
        keywords: Vec::<CoreBlockPyKeywordArg<LocatedCoreBlockPyExpr>>::new(),
    })
}

fn helper_call_expr(
    helper_name: &str,
    args: Vec<LocatedCoreBlockPyExpr>,
) -> LocatedCoreBlockPyExpr {
    helper_call_expr_with_meta(helper_name, args, (compat_node_index(), compat_range()))
}

fn str_bytes_call_expr(bytes: &[u8]) -> LocatedCoreBlockPyExpr {
    helper_call_expr("str", vec![bytes_literal_expr(bytes)])
}

#[cfg(test)]
mod test;
