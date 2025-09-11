use diet_python::min_ast::{
    Arg, ExceptHandler, ExprNode, FunctionDef, Module, Number, Parameter, StmtNode,
};
use diet_python::transform_min_ast;

#[test]
fn builds_minimal_ast() {
    let src = "async def f(x, *a, y=True, **k):\n    await g(x, *a, y=y, **k)\n    return (True, False)\n";
    let module = transform_min_ast(src, None).unwrap();
    let expected = Module {
        body: vec![StmtNode::FunctionDef(FunctionDef {
            name: "f".to_string(),
            params: vec![
                Parameter::Positional {
                    name: "x".to_string(),
                    default: None,
                },
                Parameter::VarArg {
                    name: "a".to_string(),
                },
                Parameter::KwOnly {
                    name: "y".to_string(),
                    default: Some(ExprNode::Name("True".to_string())),
                },
                Parameter::KwArg {
                    name: "k".to_string(),
                },
            ],
            body: vec![
                StmtNode::Expr(ExprNode::Await(Box::new(ExprNode::Call {
                    func: Box::new(ExprNode::Name("g".to_string())),
                    args: vec![
                        Arg::Positional(ExprNode::Name("x".to_string())),
                        Arg::Starred(ExprNode::Name("a".to_string())),
                        Arg::Keyword {
                            name: "y".to_string(),
                            value: ExprNode::Name("y".to_string()),
                        },
                        Arg::KwStarred(ExprNode::Name("k".to_string())),
                    ],
                }))),
                StmtNode::Return {
                    value: Some(ExprNode::Tuple(vec![
                        ExprNode::Name("True".to_string()),
                        ExprNode::Name("False".to_string()),
                    ])),
                },
            ],
            is_async: true,
        })],
    };
    assert_eq!(module, expected);
}

#[test]
fn try_except_else() {
    let src = "try:\n    f()\nexcept E as e:\n    handle(e)\nelse:\n    g()\n";
    let module = transform_min_ast(src, None).unwrap();
    let expected = Module {
        body: vec![StmtNode::Try {
            body: vec![StmtNode::Expr(ExprNode::Call {
                func: Box::new(ExprNode::Name("f".to_string())),
                args: vec![],
            })],
            handlers: vec![ExceptHandler {
                type_: Some(ExprNode::Name("E".to_string())),
                name: Some("e".to_string()),
                body: vec![StmtNode::Expr(ExprNode::Call {
                    func: Box::new(ExprNode::Name("handle".to_string())),
                    args: vec![Arg::Positional(ExprNode::Name("e".to_string()))],
                })],
            }],
            orelse: vec![StmtNode::Expr(ExprNode::Call {
                func: Box::new(ExprNode::Name("g".to_string())),
                args: vec![],
            })],
            finalbody: vec![],
        }],
    };
    assert_eq!(module, expected);
}

#[test]
fn global_statements_split() {
    let src = "global a, b\n";
    let module = transform_min_ast(src, None).unwrap();
    assert_eq!(
        module.body,
        vec![StmtNode::Global("a".into()), StmtNode::Global("b".into()),]
    );
}

#[test]
fn nonlocal_statements_split() {
    let src = "def outer():\n    x = 0\n    y = 0\n    def inner():\n        nonlocal x, y\n";
    let module = transform_min_ast(src, None).unwrap();
    if let StmtNode::FunctionDef(FunctionDef { body, .. }) = &module.body[0] {
        if let StmtNode::FunctionDef(FunctionDef {
            body: inner_body, ..
        }) = &body[2]
        {
            assert_eq!(
                &inner_body[..2],
                [
                    StmtNode::Nonlocal("x".into()),
                    StmtNode::Nonlocal("y".into()),
                ]
            );
        } else {
            panic!("expected inner function definition");
        }
    } else {
        panic!("expected outer function definition");
    }
}

#[test]
fn number_literals() {
    let src = "x = 1\ny = 2.5\n";
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
