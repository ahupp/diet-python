use crate::block_py::{
    core_operation_expr, core_positional_call_expr_with_meta, BlockPyFunction, BlockPyModule,
    CodegenBlockPyLiteral, CoreBytesLiteral, LocatedCodegenBlockPyExpr, LocatedName, MakeString,
    NameLocation, StructuredBlockPyStmt,
};
use crate::passes::CodegenBlockPyPass;
use ruff_python_ast::{self as ast};
use ruff_text_size::TextRange;
use std::collections::HashMap;
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

pub(crate) fn instrument_bb_module_for_trace(
    module: &mut BlockPyModule<CodegenBlockPyPass>,
    config: &TraceConfig,
) {
    for function in &mut module.callable_defs {
        if let Some(filter) = config.qualname_filter.as_ref() {
            if function.names.qualname != *filter {
                continue;
            }
        }
        let qualname = function.names.qualname.clone();
        let locator = PreparedTraceNameLocator::new(function);
        for block in &mut function.blocks {
            let block_params = block.param_name_vec();
            let trace_expr = if config.include_params && !block_params.is_empty() {
                helper_call_expr(
                    "__dp_bb_trace_enter",
                    vec![
                        string_literal_expr(qualname.as_str()),
                        string_literal_expr(block.label.as_str()),
                        param_pairs_expr(&locator, block_params.as_slice()),
                    ],
                )
            } else {
                helper_call_expr(
                    "__dp_bb_trace_enter",
                    vec![
                        string_literal_expr(qualname.as_str()),
                        string_literal_expr(block.label.as_str()),
                    ],
                )
            };
            block
                .body
                .insert(0, StructuredBlockPyStmt::Expr(trace_expr).into());
        }
    }
}

fn compat_node_index() -> ast::AtomicNodeIndex {
    ast::AtomicNodeIndex::default()
}

fn compat_range() -> TextRange {
    TextRange::default()
}

struct PreparedTraceNameLocator {
    param_slots: HashMap<String, u32>,
    existing_locations: HashMap<String, NameLocation>,
    captured_cell_slots: HashMap<String, u32>,
    owned_cell_slots: HashMap<String, u32>,
}

impl PreparedTraceNameLocator {
    fn new(function: &BlockPyFunction<CodegenBlockPyPass>) -> Self {
        let param_slots = function
            .params
            .names()
            .into_iter()
            .enumerate()
            .map(|(slot, name)| (name, slot as u32))
            .collect::<HashMap<_, _>>();
        let mut existing_locations = HashMap::new();
        for block in &function.blocks {
            for stmt in &block.body {
                match stmt {
                    crate::block_py::BlockPyStmt::Assign(assign) => {
                        existing_locations
                            .entry(assign.target.id.to_string())
                            .or_insert(assign.target.location);
                    }
                    crate::block_py::BlockPyStmt::Expr(_)
                    | crate::block_py::BlockPyStmt::Delete(_) => {}
                }
            }
        }
        let captured_cell_slots = function
            .closure_layout
            .as_ref()
            .map(|layout| {
                let mut slots = HashMap::new();
                for (slot, closure_slot) in layout.freevars.iter().enumerate() {
                    slots.insert(closure_slot.storage_name.clone(), slot as u32);
                    slots.insert(closure_slot.logical_name.clone(), slot as u32);
                }
                slots
            })
            .unwrap_or_default();
        let owned_cell_slots = function
            .closure_layout
            .as_ref()
            .map(|layout| {
                let mut slots = HashMap::new();
                for (slot, closure_slot) in layout
                    .cellvars
                    .iter()
                    .chain(layout.runtime_cells.iter())
                    .enumerate()
                {
                    slots.insert(closure_slot.storage_name.clone(), slot as u32);
                    slots.insert(closure_slot.logical_name.clone(), slot as u32);
                }
                slots
            })
            .unwrap_or_default();
        Self {
            param_slots,
            existing_locations,
            captured_cell_slots,
            owned_cell_slots,
        }
    }

    fn load_name(&self, id: &str) -> LocatedName {
        let location = if let Some(slot) = self.param_slots.get(id).copied() {
            NameLocation::Local { slot }
        } else if let Some(location) = self.existing_locations.get(id).copied() {
            location
        } else if let Some(slot) = self.captured_cell_slots.get(id).copied() {
            NameLocation::ClosureCell { slot }
        } else if let Some(slot) = self.owned_cell_slots.get(id).copied() {
            NameLocation::OwnedCell { slot }
        } else {
            NameLocation::Global
        };
        LocatedName {
            id: id.into(),
            ctx: ast::ExprContext::Load,
            range: compat_range(),
            node_index: compat_node_index(),
            location,
        }
    }
}

fn bytes_literal_expr(value: &[u8]) -> LocatedCodegenBlockPyExpr {
    LocatedCodegenBlockPyExpr::Literal(CodegenBlockPyLiteral::BytesLiteral(CoreBytesLiteral {
        range: compat_range(),
        node_index: compat_node_index(),
        value: value.to_vec(),
    }))
}

fn helper_call_expr(
    helper_name: &str,
    args: Vec<LocatedCodegenBlockPyExpr>,
) -> LocatedCodegenBlockPyExpr {
    core_positional_call_expr_with_meta(helper_name, compat_node_index(), compat_range(), args)
}

fn string_literal_expr(value: &str) -> LocatedCodegenBlockPyExpr {
    core_operation_expr(crate::block_py::Operation::MakeString(MakeString {
        node_index: compat_node_index(),
        range: compat_range(),
        arg0: bytes_literal_expr(value.as_bytes()),
    }))
}

fn tuple_expr(values: Vec<LocatedCodegenBlockPyExpr>) -> LocatedCodegenBlockPyExpr {
    helper_call_expr("__dp_tuple", values)
}

fn param_pairs_expr(
    locator: &PreparedTraceNameLocator,
    params: &[String],
) -> LocatedCodegenBlockPyExpr {
    tuple_expr(
        params
            .iter()
            .map(|param| {
                tuple_expr(vec![
                    string_literal_expr(param),
                    LocatedCodegenBlockPyExpr::Name(locator.load_name(param)),
                ])
            })
            .collect(),
    )
}

#[cfg(test)]
mod test;
