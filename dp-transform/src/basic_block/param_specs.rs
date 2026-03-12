use crate::basic_block::expr_utils::make_dp_tuple;
use crate::py_expr;
use ruff_python_ast::{self as ast, Expr};

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum FunctionParamKind {
    PosOnly,
    PosOrKeyword,
    VarArg,
    KwOnly,
    KwArg,
}

impl FunctionParamKind {
    fn label_prefix(self) -> &'static str {
        match self {
            Self::PosOnly => "/",
            Self::PosOrKeyword => "",
            Self::VarArg => "*",
            Self::KwOnly => "kw:",
            Self::KwArg => "**",
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct FunctionParamSpec {
    pub kind: FunctionParamKind,
    pub name: String,
    pub default: Option<Expr>,
}

fn push_function_param_spec(
    specs: &mut Vec<FunctionParamSpec>,
    kind: FunctionParamKind,
    name: &str,
    default: Option<&Expr>,
) {
    specs.push(FunctionParamSpec {
        kind,
        name: name.to_string(),
        default: default.cloned(),
    });
}

pub(crate) fn collect_function_param_specs(parameters: &ast::Parameters) -> Vec<FunctionParamSpec> {
    let mut specs = Vec::new();
    for param in &parameters.posonlyargs {
        push_function_param_spec(
            &mut specs,
            FunctionParamKind::PosOnly,
            param.parameter.name.id.as_str(),
            param.default.as_deref(),
        );
    }
    for param in &parameters.args {
        push_function_param_spec(
            &mut specs,
            FunctionParamKind::PosOrKeyword,
            param.parameter.name.id.as_str(),
            param.default.as_deref(),
        );
    }
    if let Some(param) = &parameters.vararg {
        push_function_param_spec(
            &mut specs,
            FunctionParamKind::VarArg,
            param.name.id.as_str(),
            None,
        );
    }
    for param in &parameters.kwonlyargs {
        push_function_param_spec(
            &mut specs,
            FunctionParamKind::KwOnly,
            param.parameter.name.id.as_str(),
            param.default.as_deref(),
        );
    }
    if let Some(param) = &parameters.kwarg {
        push_function_param_spec(
            &mut specs,
            FunctionParamKind::KwArg,
            param.name.id.as_str(),
            None,
        );
    }
    specs
}

pub(crate) fn function_param_specs_to_expr(specs: &[FunctionParamSpec]) -> Expr {
    make_dp_tuple(
        specs
            .iter()
            .map(|spec| {
                let label = format!("{}{}", spec.kind.label_prefix(), spec.name);
                let annotation_expr = py_expr!("None");
                let default_expr = spec
                    .default
                    .clone()
                    .unwrap_or_else(|| py_expr!("__dp__.NO_DEFAULT"));
                make_dp_tuple(vec![
                    py_expr!("{value:literal}", value = label.as_str()),
                    annotation_expr,
                    default_expr,
                ])
            })
            .collect(),
    )
}

pub(crate) fn function_param_specs_expr(parameters: &ast::Parameters) -> Expr {
    function_param_specs_to_expr(&collect_function_param_specs(parameters))
}

#[cfg(test)]
mod tests {
    use super::{collect_function_param_specs, FunctionParamKind};
    use crate::py_stmt;
    use ruff_python_ast::Stmt;

    #[test]
    fn collect_function_param_specs_preserves_parameter_kinds_and_defaults() {
        let stmt = py_stmt!("def f(a, /, b=1, *c, d=2, **e):\n    pass");
        let Stmt::FunctionDef(func) = stmt else {
            panic!("expected function definition");
        };

        let specs = collect_function_param_specs(func.parameters.as_ref());
        assert_eq!(specs.len(), 5);
        assert_eq!(specs[0].kind, FunctionParamKind::PosOnly);
        assert_eq!(specs[0].name, "a");
        assert!(specs[0].default.is_none());

        assert_eq!(specs[1].kind, FunctionParamKind::PosOrKeyword);
        assert_eq!(specs[1].name, "b");
        assert!(specs[1].default.is_some());

        assert_eq!(specs[2].kind, FunctionParamKind::VarArg);
        assert_eq!(specs[2].name, "c");
        assert!(specs[2].default.is_none());

        assert_eq!(specs[3].kind, FunctionParamKind::KwOnly);
        assert_eq!(specs[3].name, "d");
        assert!(specs[3].default.is_some());

        assert_eq!(specs[4].kind, FunctionParamKind::KwArg);
        assert_eq!(specs[4].name, "e");
        assert!(specs[4].default.is_none());
    }
}
