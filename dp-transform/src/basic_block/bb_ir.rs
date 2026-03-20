use crate::basic_block::block_py::{is_internal_entry_livein, BlockPyPass, BlockPyStmt};

pub use super::block_py::BbBlockMeta;
use super::block_py::{
    BbBlockPyPass, BlockPyFunction, BlockPyRaise, CoreBlockPyExprWithoutAwaitOrYield,
    CoreBlockPyLiteral, PassBlock,
};
use ruff_python_ast as ast;

pub type BbStmt = BlockPyStmt<<BbBlockPyPass as BlockPyPass>::Expr>;
pub type BbBlock = PassBlock<BbBlockPyPass>;

impl BlockPyFunction<BbBlockPyPass> {
    pub fn entry_liveins(&self) -> Vec<String> {
        if self.blocks.is_empty() {
            return Vec::new();
        }
        self.entry_block()
            .meta
            .params
            .iter()
            .filter(|name| !is_internal_entry_livein(name))
            .cloned()
            .collect()
    }
}

pub fn bb_expr_text(expr: &CoreBlockPyExprWithoutAwaitOrYield) -> String {
    match expr {
        CoreBlockPyExprWithoutAwaitOrYield::Name(name) => name.id.to_string(),
        CoreBlockPyExprWithoutAwaitOrYield::Literal(literal) => match literal {
            CoreBlockPyLiteral::StringLiteral(literal) => format!("{:?}", literal.value.to_str()),
            CoreBlockPyLiteral::BytesLiteral(literal) => {
                let mut out = String::from("b\"");
                for byte in literal.value.bytes() {
                    for escaped in std::ascii::escape_default(byte) {
                        out.push(escaped as char);
                    }
                }
                out.push('"');
                out
            }
            CoreBlockPyLiteral::NumberLiteral(literal) => match &literal.value {
                ast::Number::Int(value) => value.to_string(),
                ast::Number::Float(value) => value.to_string(),
                ast::Number::Complex { real, imag } => format!("{real}+{imag}j"),
            },
            CoreBlockPyLiteral::BooleanLiteral(literal) => literal.value.to_string(),
            CoreBlockPyLiteral::NoneLiteral(_) => "None".to_string(),
            CoreBlockPyLiteral::EllipsisLiteral(_) => "...".to_string(),
        },
        CoreBlockPyExprWithoutAwaitOrYield::Call(call) => {
            let mut parts = Vec::new();
            for arg in &call.args {
                parts.push(match arg {
                    super::block_py::CoreBlockPyCallArg::Positional(value) => bb_expr_text(value),
                    super::block_py::CoreBlockPyCallArg::Starred(value) => {
                        format!("*{}", bb_expr_text(value))
                    }
                });
            }
            for keyword in &call.keywords {
                parts.push(match keyword {
                    super::block_py::CoreBlockPyKeywordArg::Named { arg, value } => {
                        format!("{}={}", arg.id, bb_expr_text(value))
                    }
                    super::block_py::CoreBlockPyKeywordArg::Starred(value) => {
                        format!("**{}", bb_expr_text(value))
                    }
                });
            }
            format!("{}({})", bb_expr_text(&call.func), parts.join(", "))
        }
    }
}

pub fn bb_stmt_text(stmt: &BbStmt) -> String {
    match stmt {
        super::block_py::BlockPyStmt::Assign(assign) => {
            format!("{} = {}", assign.target.id, bb_expr_text(&assign.value))
        }
        super::block_py::BlockPyStmt::Expr(expr) => bb_expr_text(expr),
        super::block_py::BlockPyStmt::Delete(delete) => format!("del {}", delete.target.id),
        super::block_py::BlockPyStmt::If(_) => {
            panic!("structured BlockPy If is not allowed in BbBlock.body")
        }
    }
}

pub fn bb_raise_text(raise_stmt: &BlockPyRaise<CoreBlockPyExprWithoutAwaitOrYield>) -> String {
    let exc = raise_stmt
        .exc
        .as_ref()
        .map(bb_expr_text)
        .unwrap_or_else(|| "None".to_string());
    format!("raise exc={exc}")
}

pub fn bb_stmts_text(stmts: &[BbStmt]) -> String {
    let mut out = String::new();
    for stmt in stmts {
        out.push_str(&bb_stmt_text(stmt));
        out.push('\n');
    }
    out
}
