#[macro_export]
macro_rules! py_expr {
    ($template:literal $(, $name:ident = $value:expr)* $(,)?) => {{
        use ruff_python_ast::{self as ast, Stmt};
        let mut stmts = $crate::py_stmt!($template $(, $name = $value)*);
        if stmts.len() != 1 {
            panic!("expected a single expression");
        }
        match stmts.pop().unwrap() {
            Stmt::Expr(ast::StmtExpr { value, .. }) => *value,
            _ => panic!("expected expression statement"),
        }
    }};
}

#[macro_export]
macro_rules! py_stmt {
    ($template:literal $(, $name:ident = $value:expr)* $(,)?) => {{
        use ruff_python_parser::parse_module;
        use std::collections::HashMap;
        use regex::Regex;
        use crate::template::{var_for_placeholder, PlaceholderKind, PlaceholderValue, SyntaxTemplate};

        let re = Regex::new(r"\{([a-zA-Z_][a-zA-Z0-9_]*)\:([a-zA-Z_][a-zA-Z0-9_]*)\}").unwrap();
        let src = re
            .replace_all($template, |caps: &regex::Captures| {
                let name = &caps[1];
                let kind = match &caps[2] {
                    "expr" => PlaceholderKind::Expr,
                    "id" => PlaceholderKind::Id,
                    "stmt" => PlaceholderKind::Stmt,
                    other => panic!("unknown placeholder type `{other}` for `{name}`"),
                };
                var_for_placeholder((name, &kind))
            })
            .to_string();

        let module = parse_module(&src)
            .expect("template parse error")
            .into_syntax();

        #[allow(unused_mut)]
        let mut values: HashMap<&str, PlaceholderValue> = HashMap::new();
        $(values.insert(stringify!($name), PlaceholderValue::from($value));)*

        SyntaxTemplate::new($template, module.body, values).into_stmts()
    }};
}

use regex::Regex;
use ruff_python_ast::visitor::transformer::{walk_expr, walk_stmt, Transformer};
use ruff_python_ast::{self as ast, name::Name, Expr, Identifier, Stmt};
use ruff_text_size::TextRange;
use std::{cell::RefCell, collections::HashMap};

pub(crate) enum PlaceholderKind {
    Expr,
    Id,
    Stmt,
}

pub(crate) enum PlaceholderValue {
    Expr(Box<Expr>),
    Id(String),
    Stmt(Vec<Stmt>),
}

impl From<Expr> for PlaceholderValue {
    fn from(value: Expr) -> Self {
        Self::Expr(Box::new(value))
    }
}

impl From<Box<Expr>> for PlaceholderValue {
    fn from(value: Box<Expr>) -> Self {
        Self::Expr(value)
    }
}

impl From<&str> for PlaceholderValue {
    fn from(value: &str) -> Self {
        Self::Id(value.to_string())
    }
}

impl From<String> for PlaceholderValue {
    fn from(value: String) -> Self {
        Self::Id(value)
    }
}

impl From<Stmt> for PlaceholderValue {
    fn from(value: Stmt) -> Self {
        Self::Stmt(vec![value])
    }
}

impl From<Vec<Stmt>> for PlaceholderValue {
    fn from(value: Vec<Stmt>) -> Self {
        Self::Stmt(value)
    }
}

pub(crate) fn var_for_placeholder((name, kind): (&str, &PlaceholderKind)) -> String {
    match kind {
        PlaceholderKind::Expr => format!("__dp_placeholder_{}__", name),
        PlaceholderKind::Id => format!("__dp_placeholder_ident_{}__", name),
        PlaceholderKind::Stmt => format!("__dp_placeholder_stmt_{}__", name),
    }
}

pub(crate) struct SyntaxTemplate {
    body: Vec<Stmt>,
}

impl SyntaxTemplate {
    pub(crate) fn new(
        template: &str,
        mut body: Vec<Stmt>,
        values: HashMap<&str, PlaceholderValue>,
    ) -> Self {
        let _ = template;

        let rewriter = PlaceholderRewriter {
            regex: Regex::new(
                r"^__dp_placeholder(?:_(?P<kind>ident|stmt))?_(?P<name>[a-zA-Z_][a-zA-Z0-9_]*)__$",
            )
            .unwrap(),
            values: RefCell::new(
                values
                    .into_iter()
                    .map(|(key, value)| (key.to_string(), value))
                    .collect(),
            ),
        };

        rewriter.visit_body(&mut body);
        flatten_stmt_placeholders(&mut body);

        Self { body }
    }

    pub(crate) fn into_stmts(self) -> Vec<Stmt> {
        self.body
    }
}

pub(crate) struct PlaceholderRewriter {
    regex: Regex,
    values: RefCell<HashMap<String, PlaceholderValue>>,
}

impl PlaceholderRewriter {
    fn parse_placeholder<'a>(&self, symbol: &'a str) -> Option<(PlaceholderKind, &'a str)> {
        self.regex.captures(symbol).map(|caps| {
            let kind = match caps.name("kind").map(|m| m.as_str()) {
                Some("ident") => PlaceholderKind::Id,
                Some("stmt") => PlaceholderKind::Stmt,
                _ => PlaceholderKind::Expr,
            };
            let name = caps.name("name").unwrap().as_str();
            (kind, name)
        })
    }
}

impl Transformer for PlaceholderRewriter {
    fn visit_expr(&self, expr: &mut Expr) {
        match expr {
            Expr::Attribute(ast::ExprAttribute { attr, .. }) => {
                if let Some((kind, name)) = self.parse_placeholder(attr.id.as_str()) {
                    match (kind, self.values.borrow_mut().remove(name)) {
                        (PlaceholderKind::Id, Some(PlaceholderValue::Id(value))) => {
                            *attr = Identifier::new(
                                Name::new(value),
                                TextRange::default(),
                            );
                        }
                        (PlaceholderKind::Id, _) => {
                            panic!("expected id for placeholder {name}");
                        }
                        (PlaceholderKind::Expr, Some(PlaceholderValue::Expr(value))) => {
                            *expr = *value;
                            return;
                        }
                        (PlaceholderKind::Expr, _) => {
                            panic!("expected expr for placeholder {name}");
                        }
                        (PlaceholderKind::Stmt, _) => {
                            panic!("expected stmt for placeholder {name}");
                        }
                    }
                }
            }
            Expr::Name(ast::ExprName { id, .. }) => {
                if let Some((kind, name)) = self.parse_placeholder(id.as_str()) {
                    match (kind, self.values.borrow_mut().remove(name)) {
                        (PlaceholderKind::Expr, Some(PlaceholderValue::Expr(value))) => {
                            *expr = *value;
                            return;
                        }
                        (PlaceholderKind::Expr, _) => {
                            panic!("expected expr for placeholder {name}");
                        }
                        (PlaceholderKind::Id, Some(PlaceholderValue::Id(value))) => {
                            *id = Name::new(value);
                        }
                        (PlaceholderKind::Id, _) => {
                            panic!("expected id for placeholder {name}");
                        }
                        (PlaceholderKind::Stmt, _) => {
                            panic!("expected stmt for placeholder {name}");
                        }
                    }
                }
            }
            _ => {}
        }
        walk_expr(self, expr);
    }

    fn visit_stmt(&self, stmt: &mut Stmt) {
        if let Stmt::Expr(ast::StmtExpr { value, .. }) = stmt {
            if let Expr::Name(ast::ExprName { id, .. }) = value.as_ref() {
                if let Some((kind, name)) = self.parse_placeholder(id.as_str()) {
                    if matches!(kind, PlaceholderKind::Stmt) {
                        match self.values.borrow_mut().remove(name) {
                            Some(PlaceholderValue::Stmt(value)) => {
                                *stmt = Stmt::If(ast::StmtIf {
                                    node_index: ast::AtomicNodeIndex::default(),
                                    range: TextRange::default(),
                                    test: Box::new(crate::py_expr!("True")),
                                    body: value,
                                    elif_else_clauses: Vec::new(),
                                });
                            }
                            _ => panic!("expected stmt for placeholder {name}"),
                        }
                    }
                }
            }
        }
        walk_stmt(self, stmt);
    }
}

fn flatten_stmt_placeholders(body: &mut Vec<Stmt>) {
    let mut i = 0;
    while i < body.len() {
        if let Stmt::If(ast::StmtIf {
            test,
            body: inner,
            elif_else_clauses,
            ..
        }) = &mut body[i]
        {
            if elif_else_clauses.is_empty()
                && matches!(
                    test.as_ref(),
                    Expr::BooleanLiteral(ast::ExprBooleanLiteral { value: true, .. })
                )
            {
                let replacement = std::mem::take(inner);
                body.splice(i..=i, replacement);
                continue;
            }
        }
        i += 1;
    }
}

#[cfg(test)]
mod tests {
    use ruff_python_ast::comparable::{ComparableExpr, ComparableStmt};
    use ruff_python_parser::{parse_expression, parse_module};

    #[test]
    fn inserts_placeholder() {
        let fragment = *parse_expression("2").unwrap().into_syntax().body;
        let expr = py_expr!("1 + {two:expr}", two = fragment);
        let expected = *parse_expression("1 + 2").unwrap().into_syntax().body;
        assert_eq!(ComparableExpr::from(&expr), ComparableExpr::from(&expected));
    }

    #[test]
    fn inserts_identifier() {
        let expr = py_expr!("operator.{func:id}(1)", func = "add");
        let expected = *parse_expression("operator.add(1)")
            .unwrap()
            .into_syntax()
            .body;
        assert_eq!(ComparableExpr::from(&expr), ComparableExpr::from(&expected));
    }

    #[test]
    fn inserts_stmt() {
        let body = parse_module("a = 1\nb = 2").unwrap().into_syntax().body;
        let stmts = py_stmt!("{body:stmt}", body = body.clone());
        assert_eq!(stmts.len(), 2);
        assert_eq!(
            ComparableStmt::from(&stmts[0]),
            ComparableStmt::from(&body[0])
        );
        assert_eq!(
            ComparableStmt::from(&stmts[1]),
            ComparableStmt::from(&body[1])
        );
    }
}
