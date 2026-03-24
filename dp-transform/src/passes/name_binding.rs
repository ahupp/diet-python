use crate::block_py::intrinsics::{LOAD_GLOBAL_INTRINSIC, STORE_GLOBAL_INTRINSIC};
use crate::block_py::{
    core_positional_call_expr_with_meta, core_positional_intrinsic_expr_with_meta, BindingTarget,
    BlockPyAssign, BlockPyBindingKind, BlockPyCallableSemanticInfo, BlockPyFunction, BlockPyIf,
    BlockPyModule, BlockPyModuleMap, BlockPyStmt, CoreBlockPyCall, CoreBlockPyCallArg,
    CoreBlockPyExpr, CoreBlockPyKeywordArg, CoreBlockPyLiteral, CoreStringLiteral, IntrinsicCall,
};
use crate::passes::CoreBlockPyPass;
use ruff_python_ast::{self as ast, ExprName};

fn is_internal_symbol(name: &str) -> bool {
    name.starts_with("_dp_") || name == "__dp__"
}

fn core_string_expr(
    value: String,
    node_index: ast::AtomicNodeIndex,
    range: ruff_text_size::TextRange,
) -> CoreBlockPyExpr {
    CoreBlockPyExpr::Literal(CoreBlockPyLiteral::StringLiteral(CoreStringLiteral {
        node_index,
        range,
        value,
    }))
}

fn globals_expr(
    node_index: ast::AtomicNodeIndex,
    range: ruff_text_size::TextRange,
) -> CoreBlockPyExpr {
    core_positional_call_expr_with_meta("__dp_globals", node_index, range, Vec::new())
}

fn rewrite_global_name_load(name: ExprName) -> CoreBlockPyExpr {
    let node_index = name.node_index.clone();
    let range = name.range;
    let bind_name = name.id.to_string();
    core_positional_intrinsic_expr_with_meta(
        &LOAD_GLOBAL_INTRINSIC,
        node_index.clone(),
        range,
        vec![
            globals_expr(node_index.clone(), range),
            core_string_expr(bind_name, node_index, range),
        ],
    )
}

fn rewrite_global_binding_assign(
    assign: BlockPyAssign<CoreBlockPyExpr>,
) -> BlockPyStmt<CoreBlockPyExpr> {
    let node_index = assign.target.node_index.clone();
    let range = assign.target.range;
    let bind_name = assign.target.id.to_string();
    BlockPyStmt::Expr(core_positional_intrinsic_expr_with_meta(
        &STORE_GLOBAL_INTRINSIC,
        node_index.clone(),
        range,
        vec![
            globals_expr(node_index.clone(), range),
            core_string_expr(bind_name, node_index, range),
            assign.value,
        ],
    ))
}

struct NameBindingMapper<'a> {
    semantic: &'a BlockPyCallableSemanticInfo,
}

impl NameBindingMapper<'_> {
    fn rewrite_args(
        &self,
        args: Vec<CoreBlockPyCallArg<CoreBlockPyExpr>>,
    ) -> Vec<CoreBlockPyCallArg<CoreBlockPyExpr>> {
        args.into_iter()
            .map(|arg| match arg {
                CoreBlockPyCallArg::Positional(value) => {
                    CoreBlockPyCallArg::Positional(self.map_expr(value))
                }
                CoreBlockPyCallArg::Starred(value) => {
                    CoreBlockPyCallArg::Starred(self.map_expr(value))
                }
            })
            .collect()
    }

    fn rewrite_keywords(
        &self,
        keywords: Vec<CoreBlockPyKeywordArg<CoreBlockPyExpr>>,
    ) -> Vec<CoreBlockPyKeywordArg<CoreBlockPyExpr>> {
        keywords
            .into_iter()
            .map(|keyword| match keyword {
                CoreBlockPyKeywordArg::Named { arg, value } => CoreBlockPyKeywordArg::Named {
                    arg,
                    value: self.map_expr(value),
                },
                CoreBlockPyKeywordArg::Starred(value) => {
                    CoreBlockPyKeywordArg::Starred(self.map_expr(value))
                }
            })
            .collect()
    }
}

impl BlockPyModuleMap<CoreBlockPyPass, CoreBlockPyPass> for NameBindingMapper<'_> {
    fn map_assign(&self, assign: BlockPyAssign<CoreBlockPyExpr>) -> BlockPyStmt<CoreBlockPyExpr> {
        if self
            .semantic
            .binding_target_for_name(assign.target.id.as_str())
            == BindingTarget::ModuleGlobal
        {
            rewrite_global_binding_assign(BlockPyAssign {
                target: assign.target,
                value: self.map_expr(assign.value),
            })
        } else {
            BlockPyStmt::Assign(BlockPyAssign {
                target: assign.target,
                value: self.map_expr(assign.value),
            })
        }
    }

    fn map_expr(&self, expr: CoreBlockPyExpr) -> CoreBlockPyExpr {
        match expr {
            CoreBlockPyExpr::Name(name)
                if !is_internal_symbol(name.id.as_str())
                    && self.semantic.binding_kind(name.id.as_str())
                        == Some(BlockPyBindingKind::Global) =>
            {
                rewrite_global_name_load(name)
            }
            CoreBlockPyExpr::Name(name) => CoreBlockPyExpr::Name(name),
            CoreBlockPyExpr::Literal(literal) => CoreBlockPyExpr::Literal(literal),
            CoreBlockPyExpr::Call(CoreBlockPyCall {
                node_index,
                range,
                func,
                args,
                keywords,
            }) => {
                if args.is_empty()
                    && keywords.is_empty()
                    && matches!(
                        func.as_ref(),
                        CoreBlockPyExpr::Name(name)
                            if name.id.as_str() == "globals"
                                && self.semantic.binding_kind("globals")
                                    == Some(BlockPyBindingKind::Global)
                    )
                {
                    return globals_expr(node_index, range);
                }
                CoreBlockPyExpr::Call(CoreBlockPyCall {
                    node_index,
                    range,
                    func: Box::new(self.map_expr(*func)),
                    args: self.rewrite_args(args),
                    keywords: self.rewrite_keywords(keywords),
                })
            }
            CoreBlockPyExpr::Intrinsic(call) => CoreBlockPyExpr::Intrinsic(IntrinsicCall {
                intrinsic: call.intrinsic,
                node_index: call.node_index,
                range: call.range,
                args: self.rewrite_args(call.args),
                keywords: self.rewrite_keywords(call.keywords),
            }),
        }
    }
}

fn lower_name_binding_callable(
    callable: BlockPyFunction<CoreBlockPyPass>,
) -> BlockPyFunction<CoreBlockPyPass> {
    let semantic = callable.semantic.clone();
    NameBindingMapper {
        semantic: &semantic,
    }
    .map_fn(callable)
}

pub(crate) fn lower_name_binding_in_core_blockpy_module(
    module: BlockPyModule<CoreBlockPyPass>,
) -> BlockPyModule<CoreBlockPyPass> {
    module.map_callable_defs(lower_name_binding_callable)
}
