use crate::basic_block::expr_utils::make_dp_tuple;
use crate::py_expr;
use ruff_python_ast::{self as ast, Expr};

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum ParamKind {
    Any,
    PosOnly,
    VarArg,
    KwOnly,
    KwArg,
}

impl ParamKind {
    fn runtime_label(self) -> &'static str {
        match self {
            Self::Any => "Any",
            Self::PosOnly => "PosOnly",
            Self::VarArg => "VarArg",
            Self::KwOnly => "KwOnly",
            Self::KwArg => "KwArg",
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Param {
    pub name: String,
    pub kind: ParamKind,
    pub has_default: bool,
}

#[derive(Debug, Clone, Default, Eq, PartialEq)]
pub struct ParamSpec {
    pub params: Vec<Param>,
}

impl ParamSpec {
    pub fn names(&self) -> Vec<String> {
        self.params.iter().map(|param| param.name.clone()).collect()
    }

    pub fn default_count(&self) -> usize {
        self.params.iter().filter(|param| param.has_default).count()
    }

    pub(crate) fn validate_default_count(&self, count: usize) {
        assert_eq!(
            self.default_count(),
            count,
            "ParamSpec default count does not match defaults payload",
        );
    }
}

fn push_param(
    spec: &mut ParamSpec,
    defaults: &mut Vec<Expr>,
    param: Param,
    default: Option<&Expr>,
) {
    if param.has_default {
        defaults.push(
            default
                .cloned()
                .expect("params marked with has_default should carry a default expression"),
        );
    }
    spec.params.push(param);
}

pub(crate) fn collect_param_spec_and_defaults(
    parameters: &ast::Parameters,
) -> (ParamSpec, Vec<Expr>) {
    let mut spec = ParamSpec::default();
    let mut defaults = Vec::new();

    for param in &parameters.posonlyargs {
        push_param(
            &mut spec,
            &mut defaults,
            Param {
                name: param.parameter.name.id.to_string(),
                kind: ParamKind::PosOnly,
                has_default: param.default.is_some(),
            },
            param.default.as_deref(),
        );
    }
    for param in &parameters.args {
        push_param(
            &mut spec,
            &mut defaults,
            Param {
                name: param.parameter.name.id.to_string(),
                kind: ParamKind::Any,
                has_default: param.default.is_some(),
            },
            param.default.as_deref(),
        );
    }
    if let Some(param) = &parameters.vararg {
        spec.params.push(Param {
            name: param.name.id.to_string(),
            kind: ParamKind::VarArg,
            has_default: false,
        });
    }
    for param in &parameters.kwonlyargs {
        push_param(
            &mut spec,
            &mut defaults,
            Param {
                name: param.parameter.name.id.to_string(),
                kind: ParamKind::KwOnly,
                has_default: param.default.is_some(),
            },
            param.default.as_deref(),
        );
    }
    if let Some(param) = &parameters.kwarg {
        spec.params.push(Param {
            name: param.name.id.to_string(),
            kind: ParamKind::KwArg,
            has_default: false,
        });
    }

    spec.validate_default_count(defaults.len());
    (spec, defaults)
}

pub(crate) fn param_spec_to_expr(spec: &ParamSpec) -> Expr {
    make_dp_tuple(
        spec.params
            .iter()
            .map(|param| {
                make_dp_tuple(vec![
                    py_expr!("{value:literal}", value = param.name.as_str()),
                    py_expr!("{value:literal}", value = param.kind.runtime_label()),
                    if param.has_default {
                        py_expr!("True")
                    } else {
                        py_expr!("False")
                    },
                ])
            })
            .collect(),
    )
}

pub(crate) fn param_defaults_to_expr(defaults: &[Expr]) -> Expr {
    make_dp_tuple(defaults.to_vec())
}

#[cfg(test)]
mod tests {
    use super::{collect_param_spec_and_defaults, ParamKind};
    use crate::py_stmt;
    use ruff_python_ast::Stmt;

    #[test]
    fn collect_param_spec_and_defaults_preserves_parameter_kinds_and_defaults() {
        let stmt = py_stmt!("def f(a, /, b=1, *c, d=2, **e):\n    pass");
        let Stmt::FunctionDef(func) = stmt else {
            panic!("expected function definition");
        };

        let (spec, defaults) = collect_param_spec_and_defaults(func.parameters.as_ref());
        assert_eq!(spec.params.len(), 5);
        assert_eq!(defaults.len(), 2);
        assert_eq!(spec.params[0].kind, ParamKind::PosOnly);
        assert_eq!(spec.params[0].name, "a");
        assert!(!spec.params[0].has_default);

        assert_eq!(spec.params[1].kind, ParamKind::Any);
        assert_eq!(spec.params[1].name, "b");
        assert!(spec.params[1].has_default);

        assert_eq!(spec.params[2].kind, ParamKind::VarArg);
        assert_eq!(spec.params[2].name, "c");
        assert!(!spec.params[2].has_default);

        assert_eq!(spec.params[3].kind, ParamKind::KwOnly);
        assert_eq!(spec.params[3].name, "d");
        assert!(spec.params[3].has_default);

        assert_eq!(spec.params[4].kind, ParamKind::KwArg);
        assert_eq!(spec.params[4].name, "e");
        assert!(!spec.params[4].has_default);
    }
}
