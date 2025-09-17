use super::context::Context;
use ruff_python_ast::{self as ast, Expr, Pattern, Stmt};
use ruff_python_parser::parse_expression;
use ruff_text_size::TextRange;

use crate::{py_expr, py_stmt};

enum PatternTest {
    Test { expr: Expr, assigns: Vec<Stmt> },
    Wildcard { assigns: Vec<Stmt> },
}

fn fold_exprs(exprs: Vec<Expr>, op: ast::BoolOp) -> Expr {
    if exprs.is_empty() {
        panic!("Empty expression list");
    } else {
        Expr::BoolOp(ast::ExprBoolOp {
            range: TextRange::default(),
            node_index: ast::AtomicNodeIndex::default(),
            op,
            values: exprs,
        })
    }
}

fn test_for_pattern(pattern: &Pattern, subject: Expr) -> PatternTest {
    use ast::{
        PatternMatchAs, PatternMatchClass, PatternMatchOr, PatternMatchSingleton,
        PatternMatchValue, Singleton,
    };
    use PatternTest::*;
    match pattern {
        Pattern::MatchValue(PatternMatchValue { value, .. }) => Test {
            expr: py_expr!(
                "{subject:expr} == {value:expr}",
                subject = subject,
                value = *value.clone()
            ),
            assigns: vec![],
        },
        Pattern::MatchSingleton(PatternMatchSingleton { value, .. }) => {
            let singleton_expr = match value {
                Singleton::None => py_expr!("None"),
                Singleton::True => py_expr!("True"),
                Singleton::False => py_expr!("False"),
            };
            Test {
                expr: py_expr!(
                    "{subject:expr} is {value:expr}",
                    subject = subject,
                    value = singleton_expr
                ),
                assigns: vec![],
            }
        }
        Pattern::MatchOr(PatternMatchOr { patterns, .. }) => {
            let mut tests = Vec::new();
            for p in patterns {
                match test_for_pattern(p, subject.clone()) {
                    Test { expr, assigns } if assigns.is_empty() => tests.push(expr),
                    _ => panic!("Unsupported pattern: {:?}", pattern),
                }
            }
            let test = fold_exprs(tests, ast::BoolOp::Or);
            Test {
                expr: test,
                assigns: vec![],
            }
        }
        Pattern::MatchClass(PatternMatchClass { cls, arguments, .. }) => {
            let mut tests = vec![py_expr!(
                "isinstance({subject:expr}, {cls:expr})",
                subject = subject.clone(),
                cls = *cls.clone()
            )];
            let mut assigns = Vec::new();
            let mut handle_attr =
                |pattern: &Pattern, attr_exists: Expr, attr_value: Expr| match test_for_pattern(
                    pattern, attr_value,
                ) {
                    Test {
                        expr,
                        assigns: mut a,
                    } => {
                        tests.push(attr_exists);
                        tests.push(expr);
                        assigns.append(&mut a);
                    }
                    Wildcard { assigns: mut a } => {
                        tests.push(attr_exists);
                        assigns.append(&mut a);
                    }
                };

            for (i, p) in arguments.patterns.iter().enumerate() {
                let idx_expr = *parse_expression(&i.to_string())
                    .expect("parse error")
                    .into_syntax()
                    .body;
                let attr_name = py_expr!(
                    "{cls:expr}.__match_args__[{idx:expr}]",
                    cls = *cls.clone(),
                    idx = idx_expr
                );
                let attr_exists = py_expr!(
                    "hasattr({subject:expr}, {name:expr})",
                    subject = subject.clone(),
                    name = attr_name.clone()
                );
                let attr_value = py_expr!(
                    "getattr({subject:expr}, {name:expr})",
                    subject = subject.clone(),
                    name = attr_name
                );
                handle_attr(p, attr_exists, attr_value);
            }

            for kw in &arguments.keywords {
                let attr_exists = py_expr!(
                    "hasattr({subject:expr}, {name:literal})",
                    subject = subject.clone(),
                    name = kw.attr.as_str()
                );
                let attr_value = py_expr!(
                    "getattr({subject:expr}, {name:literal})",
                    subject = subject.clone(),
                    name = kw.attr.as_str()
                );
                handle_attr(&kw.pattern, attr_exists, attr_value);
            }

            let test = fold_exprs(tests, ast::BoolOp::And);
            Test {
                expr: test,
                assigns,
            }
        }
        Pattern::MatchAs(PatternMatchAs {
            pattern: None,
            name: None,
            ..
        }) => Wildcard { assigns: vec![] },
        Pattern::MatchAs(PatternMatchAs {
            pattern,
            name: Some(name),
            ..
        }) => {
            let assign = py_stmt!(
                "{name:id} = {subject:expr}",
                name = name.as_str(),
                subject = subject.clone(),
            );
            match pattern {
                Some(p) => match test_for_pattern(p, subject) {
                    Test { expr, mut assigns } => {
                        assigns.push(assign);
                        Test { expr, assigns }
                    }
                    Wildcard { mut assigns } => {
                        assigns.push(assign);
                        Wildcard { assigns }
                    }
                },
                None => Wildcard {
                    assigns: vec![assign],
                },
            }
        }
        Pattern::MatchAs(PatternMatchAs {
            pattern: Some(p),
            name: None,
            ..
        }) => test_for_pattern(p, subject),
        _ => panic!("Unsupported pattern: {:?}", pattern),
    }
}

pub fn rewrite(ast::StmtMatch { subject, cases, .. }: ast::StmtMatch, ctx: &Context) -> Stmt {
    if cases.is_empty() {
        return py_stmt!("pass");
    }

    let subject_name = ctx.fresh("match");
    let tmp_expr = py_expr!("{name:id}", name = subject_name.as_str());

    let assign = py_stmt!(
        "{name:id} = {value:expr}",
        name = subject_name.as_str(),
        value = *subject.clone(),
    );

    let mut chain = py_stmt!("pass");
    for case in cases.into_iter().rev() {
        let ast::MatchCase {
            pattern,
            guard,
            body,
            ..
        } = case;
        use PatternTest::*;
        match test_for_pattern(&pattern, tmp_expr.clone()) {
            Wildcard { assigns } => {
                let mut block = assigns;
                block.extend(body);
                if let Some(g) = guard {
                    let test = *g;
                    chain = py_stmt!(
                        "
if {test:expr}:
    {body:stmt}
else:
    {next:stmt}",
                        test = test,
                        body = block,
                        next = chain,
                    );
                } else {
                    chain = py_stmt!("{body:stmt}", body = block);
                }
            }
            Test {
                expr: mut test_expr,
                assigns,
            } => {
                let mut block = assigns;
                block.extend(body);
                if let Some(g) = guard {
                    test_expr =
                        py_expr!("{test:expr} and {guard:expr}", test = test_expr, guard = *g,);
                }
                chain = py_stmt!(
                    "
if {test:expr}:
    {body:stmt}
else:
    {next:stmt}",
                    test = test_expr,
                    body = block,
                    next = chain,
                );
            }
        }
    }

    py_stmt!(
        "
{assign:stmt}
{chain:stmt}",
        assign = assign,
        chain = chain,
    )
}

#[cfg(test)]
mod tests {
    use crate::test_util::assert_transform_eq;

    #[test]
    fn rewrites_simple_match() {
        let input = r#"
match x:
    case 1:
        a()
    case 2:
        b()
    case _:
        c()
"#;
        let expected = r#"
_dp_match_1 = x
_dp_tmp_2 = __dp__.eq(_dp_match_1, 1)
if _dp_tmp_2:
    _dp_tmp_3 = a()
    _dp_tmp_3
else:
    _dp_tmp_4 = __dp__.eq(_dp_match_1, 2)
    if _dp_tmp_4:
        _dp_tmp_5 = b()
        _dp_tmp_5
    else:
        _dp_tmp_6 = c()
        _dp_tmp_6
"#;
        assert_transform_eq(input, expected);
    }

    #[test]
    fn rewrites_match_with_guard() {
        let input = r#"
match x:
    case 1 if cond:
        a()
    case _:
        b()
"#;
        let expected = r#"
_dp_match_1 = x
_dp_tmp_2 = __dp__.eq(_dp_match_1, 1)
_dp_tmp_3 = _dp_tmp_2
if _dp_tmp_3:
    _dp_tmp_3 = cond
if _dp_tmp_3:
    _dp_tmp_4 = a()
    _dp_tmp_4
else:
    _dp_tmp_5 = b()
    _dp_tmp_5
"#;
        assert_transform_eq(input, expected);
    }

    #[test]
    fn rewrites_match_or_pattern() {
        let input = r#"
match x:
    case 1 | 2:
        a()
    case _:
        b()
"#;
        let expected = r#"
_dp_match_1 = x
_dp_tmp_2 = __dp__.eq(_dp_match_1, 1)
_dp_tmp_3 = __dp__.eq(_dp_match_1, 2)
_dp_tmp_4 = _dp_tmp_2
if __dp__.not_(_dp_tmp_4):
    _dp_tmp_4 = _dp_tmp_3
if _dp_tmp_4:
    _dp_tmp_5 = a()
    _dp_tmp_5
else:
    _dp_tmp_6 = b()
    _dp_tmp_6
"#;
        assert_transform_eq(input, expected);
    }

    #[test]
    fn rewrites_match_singleton() {
        let input = r#"
match x:
    case None:
        a()
    case _:
        b()
"#;
        let expected = r#"
_dp_match_1 = x
_dp_tmp_2 = __dp__.is_(_dp_match_1, None)
if _dp_tmp_2:
    _dp_tmp_3 = a()
    _dp_tmp_3
else:
    _dp_tmp_4 = b()
    _dp_tmp_4
"#;
        assert_transform_eq(input, expected);
    }

    #[test]
    fn rewrites_match_as_pattern() {
        let input = r#"
match x:
    case 1 as y:
        a()
    case _:
        b()
"#;
        let expected = r#"
_dp_match_1 = x
_dp_tmp_2 = __dp__.eq(_dp_match_1, 1)
if _dp_tmp_2:
    y = _dp_match_1
    _dp_tmp_3 = a()
    _dp_tmp_3
else:
    _dp_tmp_4 = b()
    _dp_tmp_4
"#;
        assert_transform_eq(input, expected);
    }

    #[test]
    fn rewrites_match_capture_pattern() {
        let input = r#"
match x:
    case 1:
        a()
    case y:
        b()
"#;
        let expected = r#"
_dp_match_1 = x
_dp_tmp_2 = __dp__.eq(_dp_match_1, 1)
if _dp_tmp_2:
    _dp_tmp_3 = a()
    _dp_tmp_3
else:
    y = _dp_match_1
    _dp_tmp_4 = b()
    _dp_tmp_4
"#;
        assert_transform_eq(input, expected);
    }

    #[test]
    fn rewrites_match_class_with_match_args() {
        let input = r#"
match x:
    case C(1, b):
        a()
    case _:
        c()
"#;
        let expected = r#"
_dp_match_1 = x
_dp_tmp_2 = isinstance(_dp_match_1, C)
_dp_tmp_3 = C.__match_args__
_dp_tmp_4 = __dp__.getitem(_dp_tmp_3, 0)
_dp_tmp_5 = hasattr(_dp_match_1, _dp_tmp_4)
_dp_tmp_6 = C.__match_args__
_dp_tmp_7 = __dp__.getitem(_dp_tmp_6, 0)
_dp_tmp_8 = getattr(_dp_match_1, _dp_tmp_7)
_dp_tmp_9 = __dp__.eq(_dp_tmp_8, 1)
_dp_tmp_10 = C.__match_args__
_dp_tmp_11 = __dp__.getitem(_dp_tmp_10, 1)
_dp_tmp_12 = hasattr(_dp_match_1, _dp_tmp_11)
_dp_tmp_13 = _dp_tmp_2
if _dp_tmp_13:
    _dp_tmp_13 = _dp_tmp_5
if _dp_tmp_13:
    _dp_tmp_13 = _dp_tmp_9
if _dp_tmp_13:
    _dp_tmp_13 = _dp_tmp_12
if _dp_tmp_13:
    _dp_tmp_14 = C.__match_args__
    _dp_tmp_15 = __dp__.getitem(_dp_tmp_14, 1)
    _dp_tmp_16 = getattr(_dp_match_1, _dp_tmp_15)
    b = _dp_tmp_16
    _dp_tmp_17 = a()
    _dp_tmp_17
else:
    _dp_tmp_18 = c()
    _dp_tmp_18
"#;
        assert_transform_eq(input, expected);
    }
}
