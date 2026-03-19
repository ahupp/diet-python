use super::block_py::{BlockPyCallableDef, CfgBlock, CfgModule};
use std::env;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CfgTraceConfig {
    pub(crate) qualname_filter: Option<String>,
    pub(crate) include_params: bool,
}

pub(crate) trait TraceBlockMeta {
    fn trace_params(&self) -> &[String];
}

pub(crate) fn parse_cfg_trace_env() -> Option<CfgTraceConfig> {
    let raw = env::var("DIET_PYTHON_BB_TRACE").ok()?;
    parse_cfg_trace_config(raw.as_str())
}

pub(crate) fn parse_cfg_trace_config(raw: &str) -> Option<CfgTraceConfig> {
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
    Some(CfgTraceConfig {
        qualname_filter,
        include_params,
    })
}

pub(crate) fn instrument_cfg_module_for_trace<D, S, T, M>(
    module: &mut CfgModule<BlockPyCallableDef<D, CfgBlock<S, T, M>>>,
    config: &CfgTraceConfig,
    make_trace_stmt: impl Fn(&str, &str, &[String]) -> S,
) where
    M: TraceBlockMeta,
{
    for function in &mut module.callable_defs {
        if let Some(filter) = config.qualname_filter.as_ref() {
            if function.names.qualname != *filter {
                continue;
            }
        }
        let qualname = function.names.qualname.clone();
        for block in &mut function.blocks {
            let trace_stmt = make_trace_stmt(
                qualname.as_str(),
                block.label.as_str(),
                if config.include_params {
                    block.meta.trace_params()
                } else {
                    &[]
                },
            );
            block.body.insert(0, trace_stmt);
        }
    }
}
