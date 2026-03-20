use super::*;
use crate::passes::ast_to_ast::ast_rewrite::Rewrite;
use crate::passes::ast_to_ast::expr_utils::make_tuple;
use ruff_python_ast::{TypeParam, TypeParamParamSpec, TypeParamTypeVar, TypeParamTypeVarTuple};

struct TypeParamInfo {
    bindings: Vec<Stmt>,
    param_names: Vec<String>,
    type_params_tuple: Option<Expr>,
}

fn make_type_param_info(type_params: ast::TypeParams) -> TypeParamInfo {
    let mut bindings = Vec::new();
    let mut param_names = Vec::new();
    let mut type_param_exprs = Vec::new();

    for type_param in type_params.type_params {
        match type_param {
            TypeParam::TypeVar(TypeParamTypeVar {
                name,
                bound,
                default,
                ..
            }) => {
                let param_name = name.as_str().to_string();
                let (constraints, bound_expr) = match bound {
                    Some(expr) => match *expr {
                        Expr::Tuple(ast::ExprTuple { elts, .. }) => (Some(make_tuple(elts)), None),
                        other => (None, Some(other)),
                    },
                    None => (None, None),
                };
                let default_expr = default.map(|expr| *expr);

                let bound_expr = bound_expr.unwrap_or_else(|| py_expr!("None"));
                let default_expr = default_expr.unwrap_or_else(|| py_expr!("None"));
                let constraints_expr = constraints.unwrap_or_else(|| py_expr!("None"));

                bindings.push(py_stmt!(
                    "{name:id} = __dp_typing_TypeVar({name_literal:literal}, {bound:expr}, {default:expr}, {constraints:expr})",
                    name = param_name.as_str(),
                    name_literal = param_name.as_str(),
                    bound = bound_expr,
                    default = default_expr,
                    constraints = constraints_expr,
                ));
                type_param_exprs.push(py_expr!("{name:id}", name = param_name.as_str()));
                param_names.push(param_name);
            }
            TypeParam::TypeVarTuple(TypeParamTypeVarTuple { name, default, .. }) => {
                let param_name = name.as_str().to_string();
                let binding = match default.map(|expr| *expr) {
                    Some(default_expr) => py_stmt!(
                        "{name:id} = __dp_typing_TypeVarTuple({name_literal:literal}, default={default:expr})",
                        name = param_name.as_str(),
                        name_literal = param_name.as_str(),
                        default = default_expr,
                    ),
                    None => py_stmt!(
                        "{name:id} = __dp_typing_TypeVarTuple({name_literal:literal})",
                        name = param_name.as_str(),
                        name_literal = param_name.as_str(),
                    ),
                };

                bindings.push(binding);
                type_param_exprs.push(py_expr!("{name:id}", name = param_name.as_str()));
                param_names.push(param_name);
            }
            TypeParam::ParamSpec(TypeParamParamSpec { name, default, .. }) => {
                let param_name = name.as_str().to_string();
                let binding = match default.map(|expr| *expr) {
                    Some(default_expr) => py_stmt!(
                        "{name:id} = __dp_typing_ParamSpec({name_literal:literal}, default={default:expr})",
                        name = param_name.as_str(),
                        name_literal = param_name.as_str(),
                        default = default_expr,
                    ),
                    None => py_stmt!(
                        "{name:id} = __dp_typing_ParamSpec({name_literal:literal})",
                        name = param_name.as_str(),
                        name_literal = param_name.as_str(),
                    ),
                };

                bindings.push(binding);
                type_param_exprs.push(py_expr!("{name:id}", name = param_name.as_str()));
                param_names.push(param_name);
            }
        }
    }

    let type_params_tuple = if type_param_exprs.is_empty() {
        None
    } else {
        Some(make_tuple(type_param_exprs))
    };

    TypeParamInfo {
        bindings,
        param_names,
        type_params_tuple,
    }
}

pub(crate) fn rewrite_type_alias_stmt(
    _context: &Context,
    type_alias: ast::StmtTypeAlias,
) -> Rewrite {
    let ast::StmtTypeAlias {
        name,
        type_params,
        value,
        range,
        node_index,
    } = type_alias;

    let Expr::Name(ast::ExprName { id, .. }) = name.as_ref() else {
        return Rewrite::Unmodified(Stmt::TypeAlias(ast::StmtTypeAlias {
            name,
            type_params,
            value,
            range,
            node_index,
        }));
    };

    let alias_name = id.as_str();

    let mut stmts = Vec::new();
    if let Some(type_params) = type_params {
        let type_param_info = make_type_param_info(*type_params);
        stmts.extend(type_param_info.bindings);
        if let Some(type_params_tuple) = type_param_info.type_params_tuple {
            let alias_expr = py_expr!(
                "__dp_typing_TypeAliasType({name:literal}, {value:expr}, type_params={params:expr})",
                name = alias_name,
                value = value,
                params = type_params_tuple,
            );
            stmts.push(py_stmt!(
                "{target:expr} = {alias:expr}",
                target = name,
                alias = alias_expr
            ));
        } else {
            let alias_expr = py_expr!(
                "__dp_typing_TypeAliasType({name:literal}, {value:expr})",
                name = alias_name,
                value = value,
            );
            stmts.push(py_stmt!(
                "{target:expr} = {alias:expr}",
                target = name,
                alias = alias_expr
            ));
        }
        for name in type_param_info.param_names {
            stmts.push(py_stmt!("del {name:id}", name = name.as_str()));
        }
        return Rewrite::Walk(stmts);
    }

    let alias_expr = py_expr!(
        "__dp_typing_TypeAliasType({name:literal}, {value:expr})",
        name = alias_name,
        value = value,
    );
    stmts.push(py_stmt!(
        "{target:expr} = {alias:expr}",
        target = name,
        alias = alias_expr
    ));

    Rewrite::Walk(stmts)
}

impl StmtLowerer for ast::StmtTypeAlias {
    fn simplify_ast(self, context: &Context) -> Vec<Stmt> {
        stmts_from_rewrite(rewrite_type_alias_stmt(context, self))
    }

    fn to_blockpy<E>(
        &self,
        context: &Context,
        out: &mut BlockPyStmtFragmentBuilder<E>,
        loop_ctx: Option<&LoopContext>,
        next_label_id: &mut usize,
    ) -> Result<(), String>
    where
        E: From<Expr> + std::fmt::Debug,
    {
        lower_stmt_via_simplify(context, self, out, loop_ctx, next_label_id)
    }
}

#[cfg(test)]
mod tests {
    use super::super::{simplify_stmt_ast_once_for_blockpy, BlockPyStmtFragmentBuilder};
    use super::*;
    use crate::passes::ast_to_ast::{context::Context, Options};

    #[test]
    fn stmt_type_alias_simplify_ast_desugars_before_blockpy_lowering() {
        let stmt = py_stmt!("type X = int");
        let Stmt::TypeAlias(type_alias) = stmt else {
            panic!("expected type alias stmt");
        };

        let context = Context::new(Options::for_test(), "");
        let simplified = simplify_stmt_ast_once_for_blockpy(&context, Stmt::TypeAlias(type_alias));

        assert!(!matches!(simplified.as_slice(), [Stmt::TypeAlias(_)]));
    }

    #[test]
    fn stmt_type_alias_to_blockpy_uses_trait_owned_simplification_path() {
        let stmt = py_stmt!("type X = int");
        let Stmt::TypeAlias(type_alias) = stmt else {
            panic!("expected type alias stmt");
        };
        let context = Context::new(Options::for_test(), "");
        let mut out = BlockPyStmtFragmentBuilder::<Expr>::new();
        let mut next_label_id = 0usize;

        type_alias
            .to_blockpy(&context, &mut out, None, &mut next_label_id)
            .expect("type alias lowering should succeed");

        let fragment = out.finish();
        assert!(!fragment.body.is_empty());
    }

    #[test]
    fn stmt_type_alias_rewrite_type_alias_stmt_handles_type_params() {
        let stmt = py_stmt!("type Alias[T] = list[T]");
        let Stmt::TypeAlias(type_alias) = stmt else {
            panic!("expected type alias stmt");
        };

        let context = Context::new(Options::for_test(), "");
        let rewritten = rewrite_type_alias_stmt(&context, type_alias);
        let simplified = stmts_from_rewrite(rewritten);

        assert!(!matches!(simplified.as_slice(), [Stmt::TypeAlias(_)]));
    }
}
