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

fn integer_expr(value: usize) -> Expr {
    *parse_expression(&value.to_string())
        .expect("parse error")
        .into_syntax()
        .body
}

fn test_for_pattern(pattern: &Pattern, subject: Expr) -> PatternTest {
    use ast::{
        PatternMatchAs, PatternMatchClass, PatternMatchMapping, PatternMatchOr,
        PatternMatchSequence, PatternMatchSingleton, PatternMatchStar, PatternMatchValue,
        Singleton,
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
            let mut branches: Vec<(Expr, Vec<Stmt>)> = Vec::new();
            for p in patterns {
                match test_for_pattern(p, subject.clone()) {
                    Test { expr, assigns } => branches.push((expr, assigns)),
                    Wildcard { assigns } => branches.push((py_expr!("True"), assigns)),
                }
            }

            let tests: Vec<Expr> = branches.iter().map(|(expr, _)| expr.clone()).collect();
            let mut assigns = Vec::new();
            if branches.iter().any(|(_, assigns)| !assigns.is_empty()) {
                let mut chain = py_stmt!("pass");
                for (expr, branch_assigns) in branches.iter().rev() {
                    let block = branch_assigns.clone();
                    chain = py_stmt!(
                        "
if {test:expr}:
    {body:stmt}
else:
    {next:stmt}",
                        test = expr.clone(),
                        body = block,
                        next = chain,
                    );
                }
                assigns.push(chain);
            }

            let test = fold_exprs(tests, ast::BoolOp::Or);
            Test {
                expr: test,
                assigns,
            }
        }
        Pattern::MatchSequence(PatternMatchSequence { patterns, .. }) => {
            let mut tests = vec![
                py_expr!(
                    "hasattr({subject:expr}, '__len__')",
                    subject = subject.clone()
                ),
                py_expr!(
                    "hasattr({subject:expr}, '__getitem__')",
                    subject = subject.clone()
                ),
                py_expr!(
                    "not isinstance({subject:expr}, (str, bytes, bytearray))",
                    subject = subject.clone()
                ),
            ];
            let mut assigns = Vec::new();
            let len_expr = py_expr!("len({subject:expr})", subject = subject.clone());

            if patterns.is_empty() {
                tests.push(py_expr!(
                    "{len:expr} == {count:expr}",
                    len = len_expr.clone(),
                    count = integer_expr(0)
                ));
            } else if let Some(star_index) = patterns
                .iter()
                .position(|pattern| matches!(pattern, Pattern::MatchStar(_)))
            {
                let before = star_index;
                let after = patterns.len() - star_index - 1;
                tests.push(py_expr!(
                    "{len:expr} >= {count:expr}",
                    len = len_expr.clone(),
                    count = integer_expr(before + after)
                ));

                for (index, pattern) in patterns.iter().enumerate() {
                    if index < star_index {
                        let element = py_expr!(
                            "{subject:expr}[{idx:expr}]",
                            subject = subject.clone(),
                            idx = integer_expr(index)
                        );
                        match test_for_pattern(pattern, element) {
                            Test {
                                expr,
                                assigns: mut sub_assigns,
                            } => {
                                tests.push(expr);
                                assigns.append(&mut sub_assigns);
                            }
                            Wildcard {
                                assigns: mut sub_assigns,
                            } => {
                                assigns.append(&mut sub_assigns);
                            }
                        }
                    } else if index == star_index {
                        if let Pattern::MatchStar(PatternMatchStar {
                            name: Some(name), ..
                        }) = pattern
                        {
                            let start_expr = integer_expr(before);
                            let end_expr = py_expr!(
                                "{len:expr} - {offset:expr}",
                                len = len_expr.clone(),
                                offset = integer_expr(after)
                            );
                            let slice_expr = py_expr!(
                                "{subject:expr}[{start:expr}:{end:expr}]",
                                subject = subject.clone(),
                                start = start_expr,
                                end = end_expr
                            );
                            let list_expr = py_expr!("list({value:expr})", value = slice_expr);
                            assigns.push(py_stmt!(
                                "{name:id} = {value:expr}",
                                name = name.as_str(),
                                value = list_expr
                            ));
                        }
                    } else {
                        let distance = patterns.len() - index;
                        let offset_expr = integer_expr(distance);
                        let index_expr = py_expr!(
                            "{len:expr} - {offset:expr}",
                            len = len_expr.clone(),
                            offset = offset_expr
                        );
                        let element = py_expr!(
                            "{subject:expr}[{idx:expr}]",
                            subject = subject.clone(),
                            idx = index_expr
                        );
                        match test_for_pattern(pattern, element) {
                            Test {
                                expr,
                                assigns: mut sub_assigns,
                            } => {
                                tests.push(expr);
                                assigns.append(&mut sub_assigns);
                            }
                            Wildcard {
                                assigns: mut sub_assigns,
                            } => {
                                assigns.append(&mut sub_assigns);
                            }
                        }
                    }
                }
            } else {
                tests.push(py_expr!(
                    "{len:expr} == {count:expr}",
                    len = len_expr.clone(),
                    count = integer_expr(patterns.len())
                ));
                for (index, pattern) in patterns.iter().enumerate() {
                    let element = py_expr!(
                        "{subject:expr}[{idx:expr}]",
                        subject = subject.clone(),
                        idx = integer_expr(index)
                    );
                    match test_for_pattern(pattern, element) {
                        Test {
                            expr,
                            assigns: mut sub_assigns,
                        } => {
                            tests.push(expr);
                            assigns.append(&mut sub_assigns);
                        }
                        Wildcard {
                            assigns: mut sub_assigns,
                        } => {
                            assigns.append(&mut sub_assigns);
                        }
                    }
                }
            }

            let test = fold_exprs(tests, ast::BoolOp::And);
            Test {
                expr: test,
                assigns,
            }
        }
        Pattern::MatchMapping(PatternMatchMapping {
            keys,
            patterns,
            rest,
            ..
        }) => {
            let mut tests = vec![
                py_expr!("hasattr({subject:expr}, 'keys')", subject = subject.clone()),
                py_expr!(
                    "hasattr({subject:expr}, '__getitem__')",
                    subject = subject.clone()
                ),
            ];
            let mut assigns = Vec::new();

            for (key, pattern) in keys.iter().zip(patterns.iter()) {
                let contains = py_expr!(
                    "{key:expr} in {subject:expr}",
                    key = key.clone(),
                    subject = subject.clone()
                );
                tests.push(contains);
                let value = py_expr!(
                    "{subject:expr}[{key:expr}]",
                    subject = subject.clone(),
                    key = key.clone()
                );
                match test_for_pattern(pattern, value) {
                    Test {
                        expr,
                        assigns: mut sub_assigns,
                    } => {
                        tests.push(expr);
                        assigns.append(&mut sub_assigns);
                    }
                    Wildcard {
                        assigns: mut sub_assigns,
                    } => {
                        assigns.append(&mut sub_assigns);
                    }
                }
            }

            if let Some(name) = rest {
                assigns.push(py_stmt!(
                    "{name:id} = dict({subject:expr})",
                    name = name.as_str(),
                    subject = subject.clone()
                ));
                for key in keys.iter() {
                    assigns.push(py_stmt!(
                        "{name:id}.pop({key:expr}, None)",
                        name = name.as_str(),
                        key = key.clone()
                    ));
                }
            }

            let test = fold_exprs(tests, ast::BoolOp::And);
            Test {
                expr: test,
                assigns,
            }
        }
        Pattern::MatchStar(PatternMatchStar { name, .. }) => {
            let tests = vec![
                py_expr!(
                    "hasattr({subject:expr}, '__len__')",
                    subject = subject.clone()
                ),
                py_expr!(
                    "hasattr({subject:expr}, '__getitem__')",
                    subject = subject.clone()
                ),
                py_expr!(
                    "not isinstance({subject:expr}, (str, bytes, bytearray))",
                    subject = subject.clone()
                ),
            ];
            let expr = fold_exprs(tests, ast::BoolOp::And);
            let mut assigns = Vec::new();
            if let Some(name) = name {
                let list_expr = py_expr!("list({subject:expr})", subject = subject.clone());
                assigns.push(py_stmt!(
                    "{name:id} = {value:expr}",
                    name = name.as_str(),
                    value = list_expr
                ));
            }
            Test { expr, assigns }
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

    #[test]
    fn rewrites_match_sequence_pattern() {
        let input = r#"
match x:
    case [a, 2]:
        a()
    case _:
        b()
"#;
        let expected = r#"
_dp_match_1 = x
_dp_tmp_2 = hasattr(_dp_match_1, '__len__')
_dp_tmp_3 = hasattr(_dp_match_1, '__getitem__')
_dp_tmp_4 = str, bytes, bytearray
_dp_tmp_5 = isinstance(_dp_match_1, _dp_tmp_4)
_dp_tmp_6 = __dp__.not_(_dp_tmp_5)
_dp_tmp_7 = len(_dp_match_1)
_dp_tmp_8 = __dp__.eq(_dp_tmp_7, 2)
_dp_tmp_9 = __dp__.getitem(_dp_match_1, 1)
_dp_tmp_10 = __dp__.eq(_dp_tmp_9, 2)
_dp_tmp_11 = _dp_tmp_2
if _dp_tmp_11:
    _dp_tmp_11 = _dp_tmp_3
if _dp_tmp_11:
    _dp_tmp_11 = _dp_tmp_6
if _dp_tmp_11:
    _dp_tmp_11 = _dp_tmp_8
if _dp_tmp_11:
    _dp_tmp_11 = _dp_tmp_10
if _dp_tmp_11:
    _dp_tmp_12 = __dp__.getitem(_dp_match_1, 0)
    a = _dp_tmp_12
    _dp_tmp_13 = a()
    _dp_tmp_13
else:
    _dp_tmp_14 = b()
    _dp_tmp_14
"#;
        assert_transform_eq(input, expected);
    }

    #[test]
    fn rewrites_match_sequence_with_star() {
        let input = r#"
match x:
    case [first, *rest, last]:
        a()
    case _:
        b()
"#;
        let expected = r#"
_dp_match_1 = x
_dp_tmp_2 = hasattr(_dp_match_1, '__len__')
_dp_tmp_3 = hasattr(_dp_match_1, '__getitem__')
_dp_tmp_4 = str, bytes, bytearray
_dp_tmp_5 = isinstance(_dp_match_1, _dp_tmp_4)
_dp_tmp_6 = __dp__.not_(_dp_tmp_5)
_dp_tmp_7 = len(_dp_match_1)
_dp_tmp_8 = __dp__.ge(_dp_tmp_7, 2)
_dp_tmp_9 = _dp_tmp_2
if _dp_tmp_9:
    _dp_tmp_9 = _dp_tmp_3
if _dp_tmp_9:
    _dp_tmp_9 = _dp_tmp_6
if _dp_tmp_9:
    _dp_tmp_9 = _dp_tmp_8
if _dp_tmp_9:
    _dp_tmp_10 = __dp__.getitem(_dp_match_1, 0)
    first = _dp_tmp_10
    _dp_tmp_11 = len(_dp_match_1)
    _dp_tmp_12 = __dp__.sub(_dp_tmp_11, 1)
    _dp_tmp_13 = slice(1, _dp_tmp_12, None)
    _dp_tmp_14 = __dp__.getitem(_dp_match_1, _dp_tmp_13)
    _dp_tmp_15 = list(_dp_tmp_14)
    rest = _dp_tmp_15
    _dp_tmp_16 = len(_dp_match_1)
    _dp_tmp_17 = __dp__.sub(_dp_tmp_16, 1)
    _dp_tmp_18 = __dp__.getitem(_dp_match_1, _dp_tmp_17)
    last = _dp_tmp_18
    _dp_tmp_19 = a()
    _dp_tmp_19
else:
    _dp_tmp_20 = b()
    _dp_tmp_20
"#;
        assert_transform_eq(input, expected);
    }

    #[test]
    fn rewrites_match_mapping_pattern() {
        let input = r#"
match x:
    case {"a": a, "b": 2, **rest}:
        a()
    case _:
        b()
"#;
        let expected = r#"
_dp_match_1 = x
_dp_tmp_2 = hasattr(_dp_match_1, 'keys')
_dp_tmp_3 = hasattr(_dp_match_1, '__getitem__')
_dp_tmp_4 = __dp__.contains(_dp_match_1, "a")
_dp_tmp_5 = __dp__.contains(_dp_match_1, "b")
_dp_tmp_6 = __dp__.getitem(_dp_match_1, "b")
_dp_tmp_7 = __dp__.eq(_dp_tmp_6, 2)
_dp_tmp_8 = _dp_tmp_2
if _dp_tmp_8:
    _dp_tmp_8 = _dp_tmp_3
if _dp_tmp_8:
    _dp_tmp_8 = _dp_tmp_4
if _dp_tmp_8:
    _dp_tmp_8 = _dp_tmp_5
if _dp_tmp_8:
    _dp_tmp_8 = _dp_tmp_7
if _dp_tmp_8:
    _dp_tmp_9 = __dp__.getitem(_dp_match_1, "a")
    a = _dp_tmp_9
    _dp_tmp_10 = dict(_dp_match_1)
    rest = _dp_tmp_10
    _dp_tmp_11 = rest.pop
    _dp_tmp_12 = _dp_tmp_11("a", None)
    _dp_tmp_12
    _dp_tmp_13 = rest.pop
    _dp_tmp_14 = _dp_tmp_13("b", None)
    _dp_tmp_14
    _dp_tmp_15 = a()
    _dp_tmp_15
else:
    _dp_tmp_16 = b()
    _dp_tmp_16
"#;
        assert_transform_eq(input, expected);
    }

    #[test]
    fn rewrites_match_or_with_assignments() {
        let input = r#"
match x:
    case (a, b) | [a, b]:
        a()
    case _:
        b()
"#;
        let expected = r#"
_dp_match_1 = x
_dp_tmp_2 = hasattr(_dp_match_1, '__len__')
_dp_tmp_3 = hasattr(_dp_match_1, '__getitem__')
_dp_tmp_4 = str, bytes, bytearray
_dp_tmp_5 = isinstance(_dp_match_1, _dp_tmp_4)
_dp_tmp_6 = __dp__.not_(_dp_tmp_5)
_dp_tmp_7 = len(_dp_match_1)
_dp_tmp_8 = __dp__.eq(_dp_tmp_7, 2)
_dp_tmp_9 = _dp_tmp_2
if _dp_tmp_9:
    _dp_tmp_9 = _dp_tmp_3
if _dp_tmp_9:
    _dp_tmp_9 = _dp_tmp_6
if _dp_tmp_9:
    _dp_tmp_9 = _dp_tmp_8
_dp_tmp_10 = hasattr(_dp_match_1, '__len__')
_dp_tmp_11 = hasattr(_dp_match_1, '__getitem__')
_dp_tmp_12 = str, bytes, bytearray
_dp_tmp_13 = isinstance(_dp_match_1, _dp_tmp_12)
_dp_tmp_14 = __dp__.not_(_dp_tmp_13)
_dp_tmp_15 = len(_dp_match_1)
_dp_tmp_16 = __dp__.eq(_dp_tmp_15, 2)
_dp_tmp_17 = _dp_tmp_10
if _dp_tmp_17:
    _dp_tmp_17 = _dp_tmp_11
if _dp_tmp_17:
    _dp_tmp_17 = _dp_tmp_14
if _dp_tmp_17:
    _dp_tmp_17 = _dp_tmp_16
_dp_tmp_18 = _dp_tmp_9
if __dp__.not_(_dp_tmp_18):
    _dp_tmp_18 = _dp_tmp_17
if _dp_tmp_18:
    _dp_tmp_19 = hasattr(_dp_match_1, '__len__')
    _dp_tmp_20 = hasattr(_dp_match_1, '__getitem__')
    _dp_tmp_21 = str, bytes, bytearray
    _dp_tmp_22 = isinstance(_dp_match_1, _dp_tmp_21)
    _dp_tmp_23 = __dp__.not_(_dp_tmp_22)
    _dp_tmp_24 = len(_dp_match_1)
    _dp_tmp_25 = __dp__.eq(_dp_tmp_24, 2)
    _dp_tmp_26 = _dp_tmp_19
    if _dp_tmp_26:
        _dp_tmp_26 = _dp_tmp_20
    if _dp_tmp_26:
        _dp_tmp_26 = _dp_tmp_23
    if _dp_tmp_26:
        _dp_tmp_26 = _dp_tmp_25
    if _dp_tmp_26:
        _dp_tmp_27 = __dp__.getitem(_dp_match_1, 0)
        a = _dp_tmp_27
        _dp_tmp_28 = __dp__.getitem(_dp_match_1, 1)
        b = _dp_tmp_28
    else:
        _dp_tmp_29 = hasattr(_dp_match_1, '__len__')
        _dp_tmp_30 = hasattr(_dp_match_1, '__getitem__')
        _dp_tmp_31 = str, bytes, bytearray
        _dp_tmp_32 = isinstance(_dp_match_1, _dp_tmp_31)
        _dp_tmp_33 = __dp__.not_(_dp_tmp_32)
        _dp_tmp_34 = len(_dp_match_1)
        _dp_tmp_35 = __dp__.eq(_dp_tmp_34, 2)
        _dp_tmp_36 = _dp_tmp_29
        if _dp_tmp_36:
            _dp_tmp_36 = _dp_tmp_30
        if _dp_tmp_36:
            _dp_tmp_36 = _dp_tmp_33
        if _dp_tmp_36:
            _dp_tmp_36 = _dp_tmp_35
        if _dp_tmp_36:
            _dp_tmp_37 = __dp__.getitem(_dp_match_1, 0)
            a = _dp_tmp_37
            _dp_tmp_38 = __dp__.getitem(_dp_match_1, 1)
            b = _dp_tmp_38
        else:
            pass
    _dp_tmp_39 = a()
    _dp_tmp_39
else:
    _dp_tmp_40 = b()
    _dp_tmp_40
"#;
        assert_transform_eq(input, expected);
    }
}
