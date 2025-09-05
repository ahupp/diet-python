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

        #[allow(unused_mut)]
        let mut values: HashMap<&str, PlaceholderValue> = HashMap::new();
        $(values.insert(stringify!($name), PlaceholderValue::from($value));)*

        let re = Regex::new(r"\{([a-zA-Z_][a-zA-Z0-9_]*)\:([a-zA-Z_][a-zA-Z0-9_]*)\}").unwrap();
        let src = {
            let values = &mut values;
            re.replace_all($template, |caps: &regex::Captures| {
                let name = &caps[1];
                match &caps[2] {
                    "id" => match values.remove(name) {
                        Some(PlaceholderValue::Id(value)) => value,
                        _ => panic!("expected id for placeholder `{name}`"),
                    },
                    "expr" => var_for_placeholder((name, &PlaceholderKind::Expr)),
                    "stmt" => var_for_placeholder((name, &PlaceholderKind::Stmt)),
                    other => panic!("unknown placeholder type `{other}` for `{name}`"),
                }
            })
        };
        let src = src.to_string();

        let module = parse_module(&src)
            .expect("template parse error")
            .into_syntax();

        SyntaxTemplate::new($template, module.body, values).into_stmts()
    }};
}

use regex::Regex;
use ruff_python_ast::visitor::transformer::{walk_expr, walk_stmt, Transformer};
use ruff_python_ast::{self as ast, Expr, Stmt};
use ruff_text_size::TextRange;
use std::{cell::RefCell, collections::HashMap};

pub(crate) enum PlaceholderKind {
    Expr,
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
                r"^__dp_placeholder(?:_(?P<kind>stmt))?_(?P<name>[a-zA-Z_][a-zA-Z0-9_]*)__$",
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
        match stmt {
            Stmt::Expr(ast::StmtExpr { value, .. }) => {
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
            Stmt::FunctionDef(_) => {
                walk_stmt(self, stmt);
                if let Stmt::FunctionDef(ast::StmtFunctionDef { body, .. }) = stmt {
                    flatten_stmt_placeholders(body);
                }
                return;
            }
            _ => {}
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
    use ruff_python_ast::{
        self as ast,
        comparable::{ComparableExpr, ComparableStmt},
    };
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

    #[test]
    fn inserts_function_parts() {
        let body = parse_module("a = 1").unwrap().into_syntax().body;
        let stmts = py_stmt!(
            "def {func:id}({param:id}):\n    {body:stmt}",
            func = "foo",
            param = "arg",
            body = body.clone(),
        );
        assert_eq!(stmts.len(), 1);
        match &stmts[0] {
            ruff_python_ast::Stmt::FunctionDef(ast::StmtFunctionDef {
                name,
                parameters,
                body: fn_body,
                ..
            }) => {
                assert_eq!(name.id.as_str(), "foo");
                assert_eq!(parameters.args[0].parameter.name.id.as_str(), "arg");
                assert_eq!(
                    ComparableStmt::from(&fn_body[0]),
                    ComparableStmt::from(&body[0])
                );
            }
            _ => panic!("expected function def"),
        }
    }
}
