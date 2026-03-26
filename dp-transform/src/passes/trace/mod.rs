use crate::block_py::{
    core_positional_call_expr_with_meta, BlockPyModule, BlockPyPass, BlockPyStmt, CoreBlockPyExpr,
    CoreBlockPyLiteral, CoreStringLiteral,
};
use crate::passes::PreparedBbBlockPyPass;
use ruff_python_ast::{self as ast, ExprName};
use ruff_text_size::TextRange;
use std::env;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TraceConfig {
    pub(crate) qualname_filter: Option<String>,
    pub(crate) include_params: bool,
}

pub(crate) fn parse_trace_env() -> Option<TraceConfig> {
    let raw = env::var("DIET_PYTHON_BB_TRACE").ok()?;
    parse_trace_config(raw.as_str())
}

pub(crate) fn parse_trace_config(raw: &str) -> Option<TraceConfig> {
    let trimmed = raw.trim();
    if trimmed.is_empty() || trimmed == "0" {
        return None;
    }
    let (selector, include_params) = if let Some(stripped) = trimmed.strip_suffix(":params") {
        (stripped.trim(), true)
    } else {
        (trimmed, false)
    };
    let qualname_filter = match selector {
        "" | "1" | "*" | "all" => None,
        value => Some(value.to_string()),
    };
    Some(TraceConfig {
        qualname_filter,
        include_params,
    })
}

fn instrument_cfg_module_for_trace<P: BlockPyPass>(
    module: &mut BlockPyModule<P>,
    config: &TraceConfig,
    make_trace_stmt: impl Fn(&str, &str, &[String]) -> BlockPyStmt<P::Expr>,
) where
    BlockPyStmt<P::Expr>: Into<P::Stmt>,
{
    for function in &mut module.callable_defs {
        if let Some(filter) = config.qualname_filter.as_ref() {
            if function.names.qualname != *filter {
                continue;
            }
        }
        let qualname = function.names.qualname.clone();
        for block in &mut function.blocks {
            let block_params = block.param_name_vec();
            let trace_stmt = make_trace_stmt(
                qualname.as_str(),
                block.label.as_str(),
                if config.include_params {
                    block_params.as_slice()
                } else {
                    &[]
                },
            );
            block.body.insert(0, trace_stmt.into());
        }
    }
}

pub(crate) fn instrument_bb_module_for_trace(
    module: &mut BlockPyModule<PreparedBbBlockPyPass>,
    config: &TraceConfig,
) {
    instrument_cfg_module_for_trace(module, config, |qualname, label, params| {
        let trace_expr = if !params.is_empty() {
            helper_call_expr(
                "__dp_bb_trace_enter",
                vec![
                    string_literal_expr(qualname),
                    string_literal_expr(label),
                    param_pairs_expr(params),
                ],
            )
        } else {
            helper_call_expr(
                "__dp_bb_trace_enter",
                vec![string_literal_expr(qualname), string_literal_expr(label)],
            )
        };
        BlockPyStmt::Expr(trace_expr)
    });
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

fn string_literal_expr(value: &str) -> CoreBlockPyExpr {
    CoreBlockPyExpr::Literal(CoreBlockPyLiteral::StringLiteral(CoreStringLiteral {
        range: compat_range(),
        node_index: compat_node_index(),
        value: value.to_string(),
    }))
}

fn helper_call_expr(helper_name: &str, args: Vec<CoreBlockPyExpr>) -> CoreBlockPyExpr {
    core_positional_call_expr_with_meta(helper_name, compat_node_index(), compat_range(), args)
}

fn tuple_expr(values: Vec<CoreBlockPyExpr>) -> CoreBlockPyExpr {
    helper_call_expr("__dp_tuple", values)
}

fn param_pairs_expr(params: &[String]) -> CoreBlockPyExpr {
    tuple_expr(
        params
            .iter()
            .map(|param| {
                tuple_expr(vec![
                    string_literal_expr(param),
                    CoreBlockPyExpr::Name(load_name(param).into()),
                ])
            })
            .collect(),
    )
}

#[cfg(test)]
mod test;
