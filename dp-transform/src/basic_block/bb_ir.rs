use super::block_py::{
    CoreBlockPyExprWithoutAwaitOrYield, CoreBlockPyLiteral, CoreBlockPyStmtWithoutAwaitOrYield,
};
use super::cfg_ir::{CfgBlock, CfgModule};
use super::lowered_ir::{LoweredCfgFunction, LoweredFunctionKind};
use ruff_python_ast as ast;

pub type BbModule = CfgModule<BbFunction>;

#[derive(Debug, Clone, Default)]
pub struct BbBlockMeta {
    pub params: Vec<String>,
    pub exc_target_label: Option<String>,
    pub exc_name: Option<String>,
}

pub type BbStmt = CoreBlockPyStmtWithoutAwaitOrYield;
pub type BbBlock = CfgBlock<String, BbStmt, BbTerm, BbBlockMeta>;
pub type BbFunction = LoweredCfgFunction<BbBlock>;

#[derive(Debug, Clone)]
pub enum BbTerm {
    Jump(String),
    BrIf {
        test: CoreBlockPyExprWithoutAwaitOrYield,
        then_label: String,
        else_label: String,
    },
    BrTable {
        index: CoreBlockPyExprWithoutAwaitOrYield,
        targets: Vec<String>,
        default_label: String,
    },
    Raise {
        exc: Option<CoreBlockPyExprWithoutAwaitOrYield>,
        cause: Option<CoreBlockPyExprWithoutAwaitOrYield>,
    },
    Ret(Option<CoreBlockPyExprWithoutAwaitOrYield>),
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

pub fn bb_stmts_text(stmts: &[BbStmt]) -> String {
    let mut out = String::new();
    for stmt in stmts {
        out.push_str(&bb_stmt_text(stmt));
        out.push('\n');
    }
    out
}
