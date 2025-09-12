use std::cell::Cell;

use ruff_python_ast::visitor::transformer::{walk_stmt, Transformer};
use ruff_python_ast::{self as ast, Expr, Pattern, Stmt};

pub struct MatchCaseRewriter {
    count: Cell<usize>,
}

enum PatternTest {
    Test { expr: Expr, assigns: Vec<Stmt> },
    Wildcard { assigns: Vec<Stmt> },
    Unsupported,
}

impl MatchCaseRewriter {
    pub fn new() -> Self {
        Self {
            count: Cell::new(0),
        }
    }

    fn test_for_pattern(&self, pattern: &Pattern, subject: Expr) -> PatternTest {
        use ast::{
            PatternMatchAs, PatternMatchOr, PatternMatchSingleton, PatternMatchValue, Singleton,
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
                    match self.test_for_pattern(p, subject.clone()) {
                        Test { expr, assigns } if assigns.is_empty() => tests.push(expr),
                        _ => return Unsupported,
                    }
                }
                let mut iter = tests.into_iter();
                if let Some(mut test) = iter.next() {
                    for expr in iter {
                        test = crate::py_expr!(
                            "{left:expr} or {right:expr}",
                            left = test,
                            right = expr
                        );
                    }
                    Test {
                        expr: test,
                        assigns: vec![],
                    }
                } else {
                    Unsupported
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
                    Some(p) => match self.test_for_pattern(p, subject) {
                        Test { expr, mut assigns } => {
                            assigns.push(assign);
                            Test { expr, assigns }
                        }
                        Wildcard { mut assigns } => {
                            assigns.push(assign);
                            Wildcard { assigns }
                        }
                        Unsupported => Unsupported,
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
            }) => self.test_for_pattern(p, subject),
            _ => Unsupported,
        }
    }
}

impl Transformer for MatchCaseRewriter {
    fn visit_stmt(&self, stmt: &mut Stmt) {
        walk_stmt(self, stmt);

        if let Stmt::Match(ast::StmtMatch { subject, cases, .. }) = stmt {
            if cases.is_empty() {
                return;
            }

            let id = self.count.get() + 1;
            self.count.set(id);
            let subject_name = format!("_dp_match_{}", id);
            let tmp_expr = crate::py_expr!("{name:id}", name = subject_name.as_str());
            // Pre-check for unsupported patterns
            for case in cases.iter() {
                if matches!(
                    self.test_for_pattern(&case.pattern, tmp_expr.clone()),
                    PatternTest::Unsupported
                ) {
                    return;
                }
            }

            let assign = crate::py_stmt!(
                "{name:id} = {value:expr}",
                name = subject_name.as_str(),
                value = *subject.clone(),
            );

            let mut chain = crate::py_stmt!("pass");
            for case in std::mem::take(cases).into_iter().rev() {
                let ast::MatchCase {
                    pattern,
                    guard,
                    mut body,
                    ..
                } = case;
                use PatternTest::*;
                match self.test_for_pattern(&pattern, tmp_expr.clone()) {
                    Unsupported => unreachable!(),
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
    {next:stmt}
",
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
    {next:stmt}
",
                            test = test_expr,
                            body = block,
                            next = chain,
                        );
                    }
                }
            }

            let wrapper = crate::py_stmt!(
                "
{assign:stmt}
{chain:stmt}
",
                assign = assign,
                chain = chain,
            );

            *stmt = wrapper;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assert_flatten_eq;
    use ruff_python_ast::visitor::transformer::walk_body;
    use ruff_python_parser::parse_module;

    fn rewrite(source: &str) -> Vec<Stmt> {
        let parsed = parse_module(source).expect("parse error");
        let mut module = parsed.into_syntax();
        let rewriter = MatchCaseRewriter::new();
        walk_body(&rewriter, &mut module.body);
        module.body
    }

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
if _dp_match_1 == 1:
    a()
elif _dp_match_1 == 2:
    b()
else:
    c()
"#;
        let output = rewrite(input);
        assert_flatten_eq!(output, expected);
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
if _dp_match_1 == 1 and cond:
    a()
else:
    b()
"#;
        let output = rewrite(input);
        assert_flatten_eq!(output, expected);
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
if _dp_match_1 == 1 or _dp_match_1 == 2:
    a()
else:
    b()
"#;
        let output = rewrite(input);
        assert_flatten_eq!(output, expected);
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
if _dp_match_1 is None:
    a()
else:
    b()
"#;
        let output = rewrite(input);
        assert_flatten_eq!(output, expected);
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
if _dp_match_1 == 1:
    y = _dp_match_1
    a()
else:
    b()
"#;
        let output = rewrite(input);
        assert_flatten_eq!(output, expected);
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
if _dp_match_1 == 1:
    a()
else:
    y = _dp_match_1
    b()
"#;
        let output = rewrite(input);
        assert_flatten_eq!(output, expected);
    }
}
