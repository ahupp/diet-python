use crate::basic_block::bb_ir::{BbModule, BbOp};
use crate::py_stmt;
use ruff_python_parser::parse_expression;
use std::env;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct BbTraceConfig {
    pub(crate) qualname_filter: Option<String>,
    pub(crate) include_params: bool,
}

pub(crate) fn parse_bb_trace_env() -> Option<BbTraceConfig> {
    let raw = env::var("DIET_PYTHON_BB_TRACE").ok()?;
    parse_bb_trace_config(raw.as_str())
}

fn parse_bb_trace_config(raw: &str) -> Option<BbTraceConfig> {
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
    Some(BbTraceConfig {
        qualname_filter,
        include_params,
    })
}

pub(crate) fn instrument_bb_module_for_trace(module: &mut BbModule, config: &BbTraceConfig) {
    for function in module.functions_mut() {
        if let Some(filter) = config.qualname_filter.as_ref() {
            if function.qualname != *filter {
                continue;
            }
        }
        let qualname = function.qualname.clone();
        for block in &mut function.blocks {
            let trace_stmt = if config.include_params && !block.meta.params.is_empty() {
                let params_expr = parse_expression(
                    format!(
                        "__dp_bb_trace_enter({}, {}, {})",
                        quote_python_string(qualname.as_str()),
                        quote_python_string(block.label.as_str()),
                        param_pairs_expr_source(&block.meta.params),
                    )
                    .as_str(),
                )
                .unwrap_or_else(|err| {
                    panic!(
                        "failed to build BB trace expression for {}::{}: {err}",
                        qualname, block.label
                    )
                });
                py_stmt!("{value:expr}", value = *params_expr.into_syntax().body)
            } else {
                py_stmt!(
                    "__dp_bb_trace_enter({qualname:literal}, {label:literal})",
                    qualname = qualname.as_str(),
                    label = block.label.as_str(),
                )
            };
            block.body.insert(
                0,
                BbOp::from_stmt(trace_stmt).expect("failed to lower BB trace statement into BbOp"),
            );
        }
    }
}

fn quote_python_string(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 2);
    out.push('"');
    for ch in value.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            _ => out.push(ch),
        }
    }
    out.push('"');
    out
}

fn param_pairs_expr_source(params: &[String]) -> String {
    let mut source = String::from("(");
    for (index, param) in params.iter().enumerate() {
        if index > 0 {
            source.push_str(", ");
        }
        source.push('(');
        source.push_str(&quote_python_string(param.as_str()));
        source.push_str(", ");
        source.push_str(param.as_str());
        source.push(')');
    }
    if params.len() == 1 {
        source.push(',');
    }
    source.push(')');
    source
}

#[cfg(test)]
mod tests {
    use super::{instrument_bb_module_for_trace, parse_bb_trace_config, BbTraceConfig};
    use crate::{
        basic_block::normalize_bb_module_for_codegen, transform_str_to_bb_ir_with_options, Options,
    };

    #[test]
    fn parses_all_and_params_variants() {
        assert_eq!(
            parse_bb_trace_config("all:params"),
            Some(BbTraceConfig {
                qualname_filter: None,
                include_params: true,
            })
        );
        assert_eq!(
            parse_bb_trace_config("run"),
            Some(BbTraceConfig {
                qualname_filter: Some("run".to_string()),
                include_params: false,
            })
        );
        assert_eq!(
            parse_bb_trace_config("run:params"),
            Some(BbTraceConfig {
                qualname_filter: Some("run".to_string()),
                include_params: true,
            })
        );
        assert_eq!(parse_bb_trace_config("0"), None);
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
            &BbTraceConfig {
                qualname_filter: Some("f".to_string()),
                include_params: true,
            },
        );
        let f = normalized
            .functions()
            .iter()
            .find(|function| function.qualname == "f")
            .expect("missing f");
        let g = normalized
            .functions()
            .iter()
            .find(|function| function.qualname == "g")
            .expect("missing g");
        let f_trace = f
            .blocks
            .iter()
            .flat_map(|block| block.body.iter())
            .map(|op| crate::ruff_ast_to_string(&op.to_stmt()))
            .find(|stmt| stmt.contains("__dp_bb_trace_enter"))
            .expect("missing trace op in f");
        assert!(f_trace.contains("__dp_bb_trace_enter"));
        assert!(f_trace.contains("x"));
        let g_has_trace = g
            .blocks
            .iter()
            .flat_map(|block| block.body.iter())
            .map(|op| crate::ruff_ast_to_string(&op.to_stmt()))
            .any(|stmt| stmt.contains("__dp_bb_trace_enter"));
        assert!(!g_has_trace);
    }
}
