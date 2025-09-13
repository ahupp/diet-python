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
Module { body: [FunctionDef(FunctionDef { name: "f", params: [Positional { name: "x", default: None }, VarArg { name: "a" }, KwOnly { name: "y", default: Some(Name("True")) }, KwArg { name: "k" }], body: [Expr(Await(Call { func: Name("g"), args: [Positional(Name("x")), Starred(Name("a")), Keyword { name: "y", value: Name("y") }, KwStarred(Name("k"))] })), Return { value: Some(Tuple([Name("True"), Name("False")])) }], is_async: true, scope_vars: OuterScopeVars { globals: [], nonlocals: [] } })] }
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
                target: "x".to_string(),
                value: ExprNode::Number(Number::Int("1".to_string())),
            },
            StmtNode::Assign {
                target: "y".to_string(),
                value: ExprNode::Number(Number::Float("2.5".to_string())),
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
            target: "x".to_string(),
            value: ExprNode::Name("None".to_string()),
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
            target: "x".to_string(),
            value: ExprNode::String("hi".to_string()),
        }]
    );
}
