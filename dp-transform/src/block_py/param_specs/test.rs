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
