use crate::block_py::{
    BlockPyModule, BlockPyPass, BlockPyStmt, CoreBlockPyCall, CoreBlockPyCallArg,
    CoreBlockPyExprWithoutAwaitOrYield, CoreBlockPyKeywordArg, CoreBlockPyLiteral,
    CoreStringLiteral,
};
use crate::passes::BbBlockPyPass;
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
    module: &mut BlockPyModule<BbBlockPyPass>,
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

fn string_literal_expr(value: &str) -> CoreBlockPyExprWithoutAwaitOrYield {
    CoreBlockPyExprWithoutAwaitOrYield::Literal(CoreBlockPyLiteral::StringLiteral(
        CoreStringLiteral {
            range: compat_range(),
            node_index: compat_node_index(),
            value: value.to_string(),
        },
    ))
}

fn helper_call_expr(
    helper_name: &str,
    args: Vec<CoreBlockPyExprWithoutAwaitOrYield>,
) -> CoreBlockPyExprWithoutAwaitOrYield {
    CoreBlockPyExprWithoutAwaitOrYield::Call(CoreBlockPyCall {
        node_index: compat_node_index(),
        range: compat_range(),
        func: Box::new(CoreBlockPyExprWithoutAwaitOrYield::Name(load_name(
            helper_name,
        ))),
        args: args
            .into_iter()
            .map(CoreBlockPyCallArg::Positional)
            .collect(),
        keywords: Vec::<CoreBlockPyKeywordArg<CoreBlockPyExprWithoutAwaitOrYield>>::new(),
    })
}

fn tuple_expr(
    values: Vec<CoreBlockPyExprWithoutAwaitOrYield>,
) -> CoreBlockPyExprWithoutAwaitOrYield {
    helper_call_expr("__dp_tuple", values)
}

fn param_pairs_expr(params: &[String]) -> CoreBlockPyExprWithoutAwaitOrYield {
    tuple_expr(
        params
            .iter()
            .map(|param| {
                tuple_expr(vec![
                    string_literal_expr(param),
                    CoreBlockPyExprWithoutAwaitOrYield::Name(load_name(param)),
                ])
            })
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::{instrument_bb_module_for_trace, parse_trace_config, TraceConfig};
    use crate::passes::normalize_bb_module_for_codegen;
    use crate::{transform_str_to_bb_ir_with_options, Options};

    #[test]
    fn parses_all_and_params_variants() {
        assert_eq!(
            parse_trace_config("all:params"),
            Some(TraceConfig {
                qualname_filter: None,
                include_params: true,
            })
        );
        assert_eq!(
            parse_trace_config("run"),
            Some(TraceConfig {
                qualname_filter: Some("run".to_string()),
                include_params: false,
            })
        );
        assert_eq!(
            parse_trace_config("run:params"),
            Some(TraceConfig {
                qualname_filter: Some("run".to_string()),
                include_params: true,
            })
        );
        assert_eq!(parse_trace_config("0"), None);
    }

    #[test]
    fn instruments_matching_function_blocks() {
        let source = "def f(x):\n    return x + 1\n\ndef g(y):\n    return y + 2\n";
        let options = Options::for_test();
        let bb_module = transform_str_to_bb_ir_with_options(source, options)
            .expect("transform should succeed")
            .expect("bb module should be available");
        let mut normalized = normalize_bb_module_for_codegen(&bb_module);
        instrument_bb_module_for_trace(
            &mut normalized,
            &TraceConfig {
                qualname_filter: Some("f".to_string()),
                include_params: true,
            },
        );
        let f = normalized
            .callable_defs
            .iter()
            .find(|function| function.names.qualname == "f")
            .expect("missing f");
        let g = normalized
            .callable_defs
            .iter()
            .find(|function| function.names.qualname == "g")
            .expect("missing g");
        let f_trace = f
            .blocks
            .iter()
            .flat_map(|block| block.body.iter())
            .map(crate::block_py::pretty::bb_stmt_text)
            .find(|stmt| stmt.contains("__dp_bb_trace_enter"))
            .expect("missing trace op in f");
        assert!(f_trace.contains("__dp_bb_trace_enter"));
        assert!(f_trace.contains("x"));
        let g_has_trace = g
            .blocks
            .iter()
            .flat_map(|block| block.body.iter())
            .map(crate::block_py::pretty::bb_stmt_text)
            .any(|stmt| stmt.contains("__dp_bb_trace_enter"));
        assert!(!g_has_trace);
    }
}
