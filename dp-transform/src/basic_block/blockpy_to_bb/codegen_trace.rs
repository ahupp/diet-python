use crate::basic_block::bb_ir::{BbBlockMeta, BbStmt};
use crate::basic_block::block_py::{
    BbBlockPyPass, BlockPyModule, CoreBlockPyCall, CoreBlockPyCallArg,
    CoreBlockPyExprWithoutAwaitOrYield, CoreBlockPyKeywordArg, CoreBlockPyLiteral,
};
use crate::basic_block::cfg_trace::{
    instrument_cfg_module_for_trace, CfgTraceConfig, TraceBlockMeta,
};
use ruff_python_ast::str::Quote;
use ruff_python_ast::{
    self as ast, ExprName, StringLiteral, StringLiteralFlags, StringLiteralValue,
};
use ruff_text_size::TextRange;

#[cfg(test)]
use crate::basic_block::cfg_trace::parse_cfg_trace_config;

impl TraceBlockMeta for BbBlockMeta {
    fn trace_params(&self) -> &[String] {
        &self.params
    }
}

pub(crate) fn instrument_bb_module_for_trace(
    module: &mut BlockPyModule<BbBlockPyPass>,
    config: &CfgTraceConfig,
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
        BbStmt::Expr(trace_expr)
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
        ast::ExprStringLiteral {
            range: compat_range(),
            node_index: compat_node_index(),
            value: StringLiteralValue::single(StringLiteral {
                range: compat_range(),
                node_index: compat_node_index(),
                value: value.into(),
                flags: StringLiteralFlags::empty().with_quote_style(Quote::Double),
            }),
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
    use super::{instrument_bb_module_for_trace, parse_cfg_trace_config};
    use crate::{
        basic_block::{cfg_trace::CfgTraceConfig, normalize_bb_module_for_codegen},
        transform_str_to_bb_ir_with_options, Options,
    };

    #[test]
    fn parses_all_and_params_variants() {
        assert_eq!(
            parse_cfg_trace_config("all:params"),
            Some(CfgTraceConfig {
                qualname_filter: None,
                include_params: true,
            })
        );
        assert_eq!(
            parse_cfg_trace_config("run"),
            Some(CfgTraceConfig {
                qualname_filter: Some("run".to_string()),
                include_params: false,
            })
        );
        assert_eq!(
            parse_cfg_trace_config("run:params"),
            Some(CfgTraceConfig {
                qualname_filter: Some("run".to_string()),
                include_params: true,
            })
        );
        assert_eq!(parse_cfg_trace_config("0"), None);
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
            &CfgTraceConfig {
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
            .map(crate::basic_block::block_py::pretty::bb_stmt_text)
            .find(|stmt| stmt.contains("__dp_bb_trace_enter"))
            .expect("missing trace op in f");
        assert!(f_trace.contains("__dp_bb_trace_enter"));
        assert!(f_trace.contains("x"));
        let g_has_trace = g
            .blocks
            .iter()
            .flat_map(|block| block.body.iter())
            .map(crate::basic_block::block_py::pretty::bb_stmt_text)
            .any(|stmt| stmt.contains("__dp_bb_trace_enter"));
        assert!(!g_has_trace);
    }
}
