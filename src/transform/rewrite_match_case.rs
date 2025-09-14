use std::cell::Cell;

use ruff_python_ast::{self as ast, Expr, Pattern, Stmt};
use ruff_python_parser::parse_expression;
use ruff_text_size::TextRange;

use crate::py_stmt;

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
            expr: crate::py_expr!(
                "{subject:expr} == {value:expr}",
                subject = subject,
                value = *value.clone()
            ),
            assigns: vec![],
        },
        Pattern::MatchSingleton(PatternMatchSingleton { value, .. }) => {
            let singleton_expr = match value {
                Singleton::None => crate::py_expr!("None"),
                Singleton::True => crate::py_expr!("True"),
                Singleton::False => crate::py_expr!("False"),
            };
            Test {
                expr: crate::py_expr!(
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
            let mut tests = vec![crate::py_expr!(
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
                let attr_name = crate::py_expr!(
                    "{cls:expr}.__match_args__[{idx:expr}]",
                    cls = *cls.clone(),
                    idx = idx_expr
                );
                let attr_exists = crate::py_expr!(
                    "hasattr({subject:expr}, {name:expr})",
                    subject = subject.clone(),
                    name = attr_name.clone()
                );
                let attr_value = crate::py_expr!(
                    "getattr({subject:expr}, {name:expr})",
                    subject = subject.clone(),
                    name = attr_name
                );
                handle_attr(p, attr_exists, attr_value);
            }

            for kw in &arguments.keywords {
                let attr_exists = crate::py_expr!(
                    "hasattr({subject:expr}, {name:literal})",
                    subject = subject.clone(),
                    name = kw.attr.as_str()
                );
                let attr_value = crate::py_expr!(
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
            let assign = crate::py_stmt!(
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

pub fn rewrite(ast::StmtMatch { subject, cases, .. }: ast::StmtMatch, count: &Cell<usize>) -> Stmt {
    if cases.is_empty() {
        return py_stmt!("pass");
    }

    let id = count.get() + 1;
    count.set(id);
    let subject_name = format!("_dp_match_{}", id);
    let tmp_expr = crate::py_expr!("{name:id}", name = subject_name.as_str());

    let assign = crate::py_stmt!(
        "{name:id} = {value:expr}",
        name = subject_name.as_str(),
        value = *subject.clone(),
    );

    let mut chain = crate::py_stmt!("pass");
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
                    chain = crate::py_stmt!(
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
                    chain = crate::py_stmt!("{body:stmt}", body = block);
                }
            }
            Test {
                expr: mut test_expr,
                assigns,
            } => {
                let mut block = assigns;
                block.extend(body);
                if let Some(g) = guard {
                    test_expr = crate::py_expr!(
                        "{test:expr} and {guard:expr}",
                        test = test_expr,
                        guard = *g,
                    );
                }
                chain = crate::py_stmt!(
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

    crate::py_stmt!(
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
if getattr(__dp__, "eq")(_dp_match_1, 1):
    a()
else:
    if getattr(__dp__, "eq")(_dp_match_1, 2):
        b()
    else:
        c()
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
def _dp_lambda_1():
    return cond
_dp_match_1 = x
if getattr(__dp__, "and_expr")(getattr(__dp__, "eq")(_dp_match_1, 1), _dp_lambda_1):
    a()
else:
    b()
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
def _dp_lambda_1():
    return getattr(__dp__, "eq")(_dp_match_1, 2)
_dp_match_1 = x
if getattr(__dp__, "or_expr")(getattr(__dp__, "eq")(_dp_match_1, 1), _dp_lambda_1):
    a()
else:
    b()
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
if getattr(__dp__, "is_")(_dp_match_1, None):
    a()
else:
    b()
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
if getattr(__dp__, "eq")(_dp_match_1, 1):
    y = _dp_match_1
    a()
else:
    b()
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
if getattr(__dp__, "eq")(_dp_match_1, 1):
    a()
else:
    y = _dp_match_1
    b()
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
def _dp_lambda_3():
    return hasattr(_dp_match_1, getattr(__dp__, "getitem")(getattr(C, "__match_args__"), 1))
def _dp_lambda_2():
    return getattr(__dp__, "and_expr")(getattr(__dp__, "eq")(getattr(_dp_match_1, getattr(__dp__, "getitem")(getattr(C, "__match_args__"), 0)), 1), _dp_lambda_3)
def _dp_lambda_1():
    return getattr(__dp__, "and_expr")(hasattr(_dp_match_1, getattr(__dp__, "getitem")(getattr(C, "__match_args__"), 0)), _dp_lambda_2)
_dp_match_1 = x
if getattr(__dp__, "and_expr")(isinstance(_dp_match_1, C), _dp_lambda_1):
    b = getattr(_dp_match_1, getattr(__dp__, "getitem")(getattr(C, "__match_args__"), 1))
    a()
else:
    c()
"#;
        assert_transform_eq(input, expected);
    }
}
