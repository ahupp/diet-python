use diet_python::min_ast::{ExprNode, FunctionDef, Number, StmtNode};
use diet_python::transform_min_ast;

#[test]
fn builds_minimal_ast() {
    let src = r#"
async def f(x, *a, y=True, **k):
    await g(x, *a, y=y, **k)
    return (True, False)
"#;
    let module = transform_min_ast(src, None).unwrap();
    let expected = r#"
Module { body: [FunctionDef(FunctionDef { info: (), name: "f", params: [Positional { name: "x", default: None }, VarArg { name: "a" }, KwOnly { name: "y", default: Some(Name { info: (), id: "True" }) }, KwArg { name: "k" }], body: [Expr { info: (), value: Await { info: (), value: Call { info: (), func: Name { info: (), id: "g" }, args: [Positional(Name { info: (), id: "x" }), Starred(Name { info: (), id: "a" }), Keyword { name: "y", value: Name { info: (), id: "y" } }, KwStarred(Name { info: (), id: "k" })] } } }, Return { info: (), value: Some(Tuple { info: (), elts: [Name { info: (), id: "True" }, Name { info: (), id: "False" }] }) }], is_async: true, scope_vars: OuterScopeVars { globals: [], nonlocals: [] } })] }
"#;
    assert_eq!(format!("{module:?}"), expected.trim());
}

#[test]
fn nonlocal_statements() {
    let src = r#"
def outer():
    x = 0
    y = 0
    def inner():
        nonlocal x, y
"#;
    let module = transform_min_ast(src, None).unwrap();
    if let StmtNode::FunctionDef(FunctionDef { body, .. }) = &module.body[0] {
        if let StmtNode::FunctionDef(FunctionDef { scope_vars, .. }) = &body[2] {
            assert_eq!(scope_vars.nonlocals, vec!["x".to_string(), "y".to_string()]);
        } else {
            panic!("expected inner function definition");
        }
    } else {
        panic!("expected outer function definition");
    }
}

#[test]
fn number_literals() {
    let src = r#"
x = 1
y = 2.5
"#;
    let module = transform_min_ast(src, None).unwrap();
    assert_eq!(
        module.body,
        vec![
            StmtNode::Assign {
                info: (),
                target: "x".to_string(),
                value: ExprNode::Number {
                    info: (),
                    value: Number::Int("1".to_string())
                },
            },
            StmtNode::Assign {
                info: (),
                target: "y".to_string(),
                value: ExprNode::Number {
                    info: (),
                    value: Number::Float("2.5".to_string())
                },
            },
        ]
    );
}

#[test]
fn none_becomes_name() {
    let src = r#"
x = None
"#;
    let module = transform_min_ast(src, None).unwrap();
    assert_eq!(
        module.body,
        vec![StmtNode::Assign {
            info: (),
            target: "x".to_string(),
            value: ExprNode::Name {
                info: (),
                id: "None".to_string()
            },
        }]
    );
}

#[test]
#[should_panic]
fn top_level_nonlocal_panics() {
    let src = r#"
nonlocal x
"#;
    transform_min_ast(src, None).unwrap();
}

#[test]
fn top_level_global_ignored() {
    let src = r#"
global x
"#;
    let module = transform_min_ast(src, None).unwrap();
    assert!(module.body.is_empty());
}

#[test]
fn string_literals() {
    let src = r#"
x = 'hi'
"#;
    let module = transform_min_ast(src, None).unwrap();
    assert_eq!(
        module.body,
        vec![StmtNode::Assign {
            info: (),
            target: "x".to_string(),
            value: ExprNode::String {
                info: (),
                value: "hi".to_string()
            },
        }]
    );
}
