use diet_python::min_ast::{
    Arg, ExceptHandler, ExprNode, FunctionDef, Module, Parameter, StmtNode,
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
