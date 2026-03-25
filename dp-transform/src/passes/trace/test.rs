use super::{instrument_bb_module_for_trace, parse_trace_config, TraceConfig};
use crate::passes::{lower_try_jump_exception_flow, normalize_bb_module_for_codegen};
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
    let prepared = lower_try_jump_exception_flow(&bb_module).expect("bb lowering should succeed");
    let mut normalized = normalize_bb_module_for_codegen(&prepared);
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
