use diet_python::min_ast::{ExprNode, Number, StmtNode};
use py_stmt_match_macro::py_stmt_match;

#[test]
fn matches_assign_placeholders_in_pattern() {
    let stmt = StmtNode::Assign {
        info: (),
        target: "result".to_string(),
        value: ExprNode::Number {
            info: (),
            value: Number::Int("1".to_string()),
        },
    };

    let matched = match stmt {
        py_stmt_match!("{target} = {ref value}") => {
            let _: String = target;
            let _: &ExprNode = value;

            assert_eq!(target, "result");
            assert!(matches!(
                value,
                ExprNode::Number {
                    value: Number::Int(digits),
                    ..
                } if digits == "1"
            ));

            true
        }
        _ => false,
    };

    assert!(matched, "expected the placeholder pattern to match");
}

#[test]
fn matches_mut_placeholder_and_allows_mutation() {
    let stmt = StmtNode::Return {
        info: (),
        value: Some(ExprNode::Number {
            info: (),
            value: Number::Int("1".to_string()),
        }),
    };

    let updated_value = match stmt {
        py_stmt_match!("return {mut value}") => {
            *value = ExprNode::Number {
                info: (),
                value: Number::Int("2".to_string()),
            };

            value.clone()
        }
        other => panic!("expected match, found: {other:?}"),
    };

    assert_eq!(
        updated_value,
        ExprNode::Number {
            info: (),
            value: Number::Int("2".to_string()),
        }
    );
}
