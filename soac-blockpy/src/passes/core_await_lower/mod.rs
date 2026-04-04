use crate::block_py::{
    core_runtime_positional_call_expr_with_meta, map_module, BlockPyModule,
    try_lower_core_expr_without_await_with_mapper, CoreBlockPyExprWithAwaitAndYield,
    CoreBlockPyExprWithYield, HasMeta, InstrExprNode, MapExpr, TryMapExpr, UnresolvedName,
    WithMeta, YieldFrom,
};
use crate::passes::{CoreBlockPyPassWithAwaitAndYield, CoreBlockPyPassWithYield};

pub(crate) struct ErrOnAwait;

impl
    TryMapExpr<
        CoreBlockPyExprWithAwaitAndYield,
        CoreBlockPyExprWithYield,
        CoreBlockPyExprWithAwaitAndYield,
    > for ErrOnAwait
{
    fn try_map_expr(
        &mut self,
        expr: CoreBlockPyExprWithAwaitAndYield,
    ) -> Result<CoreBlockPyExprWithYield, CoreBlockPyExprWithAwaitAndYield> {
        try_lower_core_expr_without_await_with_mapper(expr, self)
    }

    fn try_map_name(
        &mut self,
        name: UnresolvedName,
    ) -> Result<UnresolvedName, CoreBlockPyExprWithAwaitAndYield> {
        Ok(name)
    }
}

struct CoreAwaitLoweringMap;

macro_rules! lower_core_await_expr_arms {
    ([$map:ident] [$expr:ident] [$($variant:ident),*]) => {
        lower_core_await_expr_arms!(@collect [$map] [$expr] [] [$($variant),*])
    };
    (@collect [$map:ident] [$expr:ident] [$($arms:tt)*] []) => {
        match $expr {
            $($arms)*
        }
    };
    (@collect [$map:ident] [$expr:ident] [$($arms:tt)*] [Await $(, $rest:ident)*]) => {
        lower_core_await_expr_arms!(
            @collect
            [$map]
            [$expr]
            [$($arms)*
                CoreBlockPyExprWithAwaitAndYield::Await(node) => {
                    let meta = node.meta();
                    CoreBlockPyExprWithYield::YieldFrom(
                        YieldFrom::new(core_runtime_positional_call_expr_with_meta(
                            "await_iter",
                            meta.node_index.clone(),
                            meta.range,
                            vec![$map.map_expr(*node.value)],
                        ))
                        .with_meta(meta),
                    )
                },
            ]
            [$($rest),*]
        )
    };
    (@collect [$map:ident] [$expr:ident] [$($arms:tt)*] [Literal $(, $rest:ident)*]) => {
        lower_core_await_expr_arms!(
            @collect
            [$map]
            [$expr]
            [$($arms)*
                CoreBlockPyExprWithAwaitAndYield::Literal(node) => node.into(),
            ]
            [$($rest),*]
        )
    };
    (@collect [$map:ident] [$expr:ident] [$($arms:tt)*] [CellRefForName $(, $rest:ident)*]) => {
        lower_core_await_expr_arms!(
            @collect
            [$map]
            [$expr]
            [$($arms)*
                CoreBlockPyExprWithAwaitAndYield::CellRefForName(node) => node.into(),
            ]
            [$($rest),*]
        )
    };
    (@collect [$map:ident] [$expr:ident] [$($arms:tt)*] [CellRef $(, $rest:ident)*]) => {
        lower_core_await_expr_arms!(
            @collect
            [$map]
            [$expr]
            [$($arms)*
                CoreBlockPyExprWithAwaitAndYield::CellRef(node) => node.into(),
            ]
            [$($rest),*]
        )
    };
    (@collect [$map:ident] [$expr:ident] [$($arms:tt)*] [$variant:ident $(, $rest:ident)*]) => {
        lower_core_await_expr_arms!(
            @collect
            [$map]
            [$expr]
            [$($arms)*
                CoreBlockPyExprWithAwaitAndYield::$variant(node) => {
                    node.map_typed_children($map).into()
                },
            ]
            [$($rest),*]
        )
    };
}

impl MapExpr<CoreBlockPyExprWithAwaitAndYield, CoreBlockPyExprWithYield> for CoreAwaitLoweringMap {
    fn map_expr(&mut self, expr: CoreBlockPyExprWithAwaitAndYield) -> CoreBlockPyExprWithYield {
        crate::block_py::__soac_enum_variants_CoreBlockPyExprWithAwaitAndYield!(
            lower_core_await_expr_arms,
            [self],
            [expr]
        )
    }

    fn map_name(&mut self, name: UnresolvedName) -> UnresolvedName {
        name
    }
}

pub(crate) fn lower_awaits_in_core_blockpy_module(
    module: BlockPyModule<CoreBlockPyPassWithAwaitAndYield>,
) -> BlockPyModule<CoreBlockPyPassWithYield> {
    let mut mapper = CoreAwaitLoweringMap;
    map_module(&mut mapper, module)
}

#[cfg(test)]
mod test;
