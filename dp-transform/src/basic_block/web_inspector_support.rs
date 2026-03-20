use super::bb_ir;
use super::block_py::{
    BbBlockPyPass, BlockPyModule, CoreBlockPyPass, CoreBlockPyPassWithoutAwait,
    CoreBlockPyPassWithoutAwaitOrYield, LoweredRuffBlockPyPass, RuffBlockPyPass,
};
use super::blockpy_module_to_string;

fn render_semantic_blockpy_bundle(bundle: &BlockPyModule<LoweredRuffBlockPyPass>) -> String {
    let blockpy = bundle.clone();
    blockpy_module_to_string(&blockpy)
}

fn render_core_blockpy_bundle(bundle: &BlockPyModule<CoreBlockPyPass>) -> String {
    let blockpy = bundle.clone();
    blockpy_module_to_string(&blockpy)
}

fn render_core_blockpy_bundle_without_await(
    bundle: &BlockPyModule<CoreBlockPyPassWithoutAwait>,
) -> String {
    let blockpy = bundle.clone();
    blockpy_module_to_string(&blockpy)
}

fn render_core_blockpy_bundle_without_await_or_yield(
    bundle: &BlockPyModule<CoreBlockPyPassWithoutAwaitOrYield>,
) -> String {
    let blockpy = bundle.clone();
    blockpy_module_to_string(&blockpy)
}

fn render_bb_module(bundle: &BlockPyModule<BbBlockPyPass>) -> String {
    let mut out = String::new();
    if bundle
        .callable_defs
        .iter()
        .any(|function| function.names.bind_name == "_dp_module_init")
    {
        out.push_str("module_init: _dp_module_init\n\n");
    }
    for function in &bundle.callable_defs {
        out.push_str(&format!(
            "function {} [{}] entry={}\n",
            function.names.qualname,
            function.names.display_name,
            function.entry_label(),
        ));
        out.push_str(&format!("kind: {:?}\n", function.lowered_kind()));
        let param_names = function.params.names();
        if !param_names.is_empty() {
            out.push_str(&format!("params: {}\n", param_names.join(", ")));
        }
        for block in &function.blocks {
            let params = if block.meta.params.is_empty() {
                String::new()
            } else {
                format!("({})", block.meta.params.join(", "))
            };
            out.push_str(&format!("\n{}{}:\n", block.label, params));
            for stmt in &block.body {
                out.push_str("    ");
                out.push_str(&bb_ir::bb_stmt_text(stmt));
                out.push('\n');
            }
            out.push_str("    ");
            out.push_str(&render_bb_term(&block.term));
            out.push('\n');
        }
        out.push('\n');
    }
    out.trim_end().to_string()
}

fn render_bb_term(
    term: &super::block_py::BlockPyTerm<super::block_py::CoreBlockPyExprWithoutAwaitOrYield>,
) -> String {
    match term {
        super::block_py::BlockPyTerm::Jump(label) => format!("jump {label}"),
        super::block_py::BlockPyTerm::IfTerm(if_term) => format!(
            "if {} then {} else {}",
            bb_ir::bb_expr_text(&if_term.test),
            if_term.then_label,
            if_term.else_label
        ),
        super::block_py::BlockPyTerm::BranchTable(branch) => format!(
            "br_table index={} targets=[{}] default={}",
            bb_ir::bb_expr_text(&branch.index),
            branch
                .targets
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(", "),
            branch.default_label
        ),
        super::block_py::BlockPyTerm::Raise(raise_stmt) => bb_ir::bb_raise_text(raise_stmt),
        super::block_py::BlockPyTerm::Return(value) => value
            .as_ref()
            .map(|value| format!("return {}", bb_ir::bb_expr_text(value)))
            .unwrap_or_else(|| "return".to_string()),
        super::block_py::BlockPyTerm::TryJump(_) => {
            panic!("TryJump is not allowed in BbTerm")
        }
    }
}

pub(crate) fn render_tracked_pass_text(
    result: &crate::LoweringResult,
    name: &str,
) -> Option<String> {
    if let Some(body) = result.get_pass::<ruff_python_ast::Suite>(name) {
        return Some(crate::ruff_ast_to_string(body));
    }
    if let Some(module) = result.get_pass::<BlockPyModule<RuffBlockPyPass>>(name) {
        return Some(blockpy_module_to_string(module));
    }
    if let Some(bundle) = result.get_pass::<BlockPyModule<LoweredRuffBlockPyPass>>(name) {
        return Some(render_semantic_blockpy_bundle(bundle));
    }
    if let Some(bundle) = result.get_pass::<BlockPyModule<CoreBlockPyPass>>(name) {
        return Some(render_core_blockpy_bundle(bundle));
    }
    if let Some(bundle) = result.get_pass::<BlockPyModule<CoreBlockPyPassWithoutAwait>>(name) {
        return Some(render_core_blockpy_bundle_without_await(bundle));
    }
    if let Some(bundle) = result.get_pass::<BlockPyModule<CoreBlockPyPassWithoutAwaitOrYield>>(name)
    {
        return Some(render_core_blockpy_bundle_without_await_or_yield(bundle));
    }
    if let Some(bundle) = result.get_pass::<BlockPyModule<BbBlockPyPass>>(name) {
        return Some(render_bb_module(bundle));
    }
    None
}
