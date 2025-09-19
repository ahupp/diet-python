#[macro_export]
macro_rules! py_expr {
    ($template:literal $(, $name:ident = $value:expr)* $(,)?) => {{
        use ruff_python_ast::{self as ast, Stmt};
        use crate::py_stmt;
        let stmt = py_stmt!($template $(, $name = $value)*);
        match stmt {
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
        use crate::template::{
            var_for_placeholder, PlaceholderKind, PlaceholderValue, SyntaxTemplate, single_stmt,
        };

        #[allow(unused_mut)]
        let mut values: HashMap<&str, PlaceholderValue> = HashMap::new();
        #[allow(unused_mut)]
        let mut ids: HashMap<&str, serde_json::Value> = HashMap::new();
        $(match $crate::template::IntoPlaceholder::into_placeholder($value) {
            Ok(value) => { values.insert(stringify!($name), value); }
            Err(id) => { ids.insert(stringify!($name), id); }
        });*

        let re = Regex::new(r"\{([a-zA-Z_][a-zA-Z0-9_]*)\:([a-zA-Z_][a-zA-Z0-9_]*)\}").unwrap();
        let src = {
            let ids = &ids;
            re.replace_all($template, |caps: &regex::Captures| {
                let name = &caps[1];
                match &caps[2] {
                    "id" => match ids.get(name).and_then(|v| v.as_str()) {
                        Some(value) => value.to_string(),
                        _ => panic!("expected id for placeholder `{name}`"),
                    },
                    "expr" => var_for_placeholder((name, &PlaceholderKind::Expr)),
                    "stmt" => var_for_placeholder((name, &PlaceholderKind::Stmt)),
                    "literal" => match ids.get(name) {
                        Some(value) => serde_json::to_string(value)
                            .expect("failed to serialize literal"),
                        _ => panic!("expected literal for placeholder `{name}`"),
                    },
                    other => panic!("unknown placeholder type `{other}` for `{name}`"),
                }
            })
        };
        let src = src.to_string();

        let module = match parse_module(&src) {
            Ok(module) => module.into_syntax(),
            Err(e) => {
                println!("template parse error: {}\n{}", e, src);
                panic!("template parse error");
            }
        };

        let mut stmts = module.body;
        let mut template = SyntaxTemplate::new($template, values);
        template.visit_stmts(&mut stmts);
        single_stmt(stmts)
    }};
}

use crate::body_transform::{walk_expr, walk_stmt, Transformer};
use regex::Regex;
use ruff_python_ast::{self as ast, Expr, Stmt};
use ruff_text_size::TextRange;
use serde_json::Value;
use std::{cell::RefCell, collections::HashMap};

pub(crate) fn is_simple(expr: &Expr) -> bool {
    matches!(
        expr,
        Expr::Name(_)
            | Expr::NumberLiteral(_)
            | Expr::StringLiteral(_)
            | Expr::BytesLiteral(_)
            | Expr::BooleanLiteral(_)
            | Expr::NoneLiteral(_)
            | Expr::EllipsisLiteral(_)
    )
}

pub(crate) fn make_tuple(elts: Vec<Expr>) -> Expr {
    Expr::Tuple(ast::ExprTuple {
        node_index: ast::AtomicNodeIndex::default(),
        range: TextRange::default(),
        elts,
        ctx: ast::ExprContext::Load,
        parenthesized: false,
    })
}

pub(crate) fn make_binop(func_name: &'static str, left: Expr, right: Expr) -> Expr {
    py_expr!(
        "__dp__.{func:id}({left:expr}, {right:expr})",
        left = left,
        right = right,
        func = func_name
    )
}

pub(crate) fn make_unaryop(func_name: &'static str, operand: Expr) -> Expr {
    py_expr!(
        "__dp__.{func:id}({operand:expr})",
        operand = operand,
        func = func_name
    )
}

pub(crate) fn make_generator(elt: Expr, generators: Vec<ast::Comprehension>) -> Expr {
    Expr::Generator(ast::ExprGenerator {
        node_index: ast::AtomicNodeIndex::default(),
        range: TextRange::default(),
        elt: Box::new(elt),
        generators,
        parenthesized: false,
    })
}

pub(crate) fn single_stmt(mut stmts: Vec<Stmt>) -> Stmt {
    if stmts.len() == 1 {
        stmts.pop().unwrap()
    } else {
        Stmt::If(ast::StmtIf {
            node_index: ast::AtomicNodeIndex::default(),
            range: TextRange::default(),
            test: Box::new(ast::Expr::BooleanLiteral(ast::ExprBooleanLiteral {
                node_index: ast::AtomicNodeIndex::default(),
                range: TextRange::default(),
                value: true,
            })),
            body: stmts,
            elif_else_clauses: Vec::new(),
        })
    }
}
pub(crate) enum PlaceholderKind {
    Expr,
    Stmt,
}

pub(crate) enum PlaceholderValue {
    Expr(Box<Expr>),
    Stmt(Vec<Stmt>),
}

pub(crate) trait IntoPlaceholder {
    fn into_placeholder(self) -> Result<PlaceholderValue, Value>;
}

impl IntoPlaceholder for Expr {
    fn into_placeholder(self) -> Result<PlaceholderValue, Value> {
        Ok(PlaceholderValue::Expr(Box::new(self)))
    }
}

impl IntoPlaceholder for Box<Expr> {
    fn into_placeholder(self) -> Result<PlaceholderValue, Value> {
        Ok(PlaceholderValue::Expr(self))
    }
}

impl IntoPlaceholder for &str {
    fn into_placeholder(self) -> Result<PlaceholderValue, Value> {
        Err(Value::String(self.to_string()))
    }
}

impl IntoPlaceholder for String {
    fn into_placeholder(self) -> Result<PlaceholderValue, Value> {
        Err(Value::String(self))
    }
}

impl IntoPlaceholder for Stmt {
    fn into_placeholder(self) -> Result<PlaceholderValue, Value> {
        Ok(PlaceholderValue::Stmt(vec![self]))
    }
}

impl IntoPlaceholder for Vec<Stmt> {
    fn into_placeholder(self) -> Result<PlaceholderValue, Value> {
        if self.is_empty() {
            return Ok(PlaceholderValue::Stmt(vec![Stmt::Pass(ast::StmtPass {
                node_index: Default::default(),
                range: Default::default(),
            })]));
        }
        Ok(PlaceholderValue::Stmt(self))
    }
}

macro_rules! impl_into_placeholder_for_signed {
    ($($ty:ty),*) => {
        $(impl IntoPlaceholder for $ty {
            fn into_placeholder(self) -> Result<PlaceholderValue, Value> {
                Err(Value::Number(serde_json::Number::from(self as i64)))
            }
        })*
    };
}

macro_rules! impl_into_placeholder_for_unsigned {
    ($($ty:ty),*) => {
        $(impl IntoPlaceholder for $ty {
            fn into_placeholder(self) -> Result<PlaceholderValue, Value> {
                Err(Value::Number(serde_json::Number::from(self as u64)))
            }
        })*
    };
}

impl_into_placeholder_for_signed!(i8, i16, i32, i64, isize);
impl_into_placeholder_for_unsigned!(u8, u16, u32, u64, usize);

pub(crate) fn var_for_placeholder((name, kind): (&str, &PlaceholderKind)) -> String {
    match kind {
        PlaceholderKind::Expr => format!("_dp_placeholder_{}__", name),
        PlaceholderKind::Stmt => format!("_dp_placeholder_stmt_{}__", name),
    }
}

pub(crate) struct SyntaxTemplate {
    regex: Regex,
    values: RefCell<HashMap<String, PlaceholderValue>>,
}

impl SyntaxTemplate {
    pub(crate) fn new(template: &str, values: HashMap<&str, PlaceholderValue>) -> Self {
        let _ = template;

        Self {
            regex: Regex::new(
                r"^_dp_placeholder(?:_(?P<kind>stmt))?_(?P<name>[a-zA-Z_][a-zA-Z0-9_]*)__$",
            )
            .unwrap(),
            values: RefCell::new(
                values
                    .into_iter()
                    .map(|(key, value)| (key.to_string(), value))
                    .collect(),
            ),
        }
    }

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

    pub(crate) fn visit_stmts(&mut self, body: &mut Vec<Stmt>) {
        self.visit_body(body);
    }
}

impl Transformer for SyntaxTemplate {
    fn visit_expr(&mut self, expr: &mut Expr) {
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

    fn visit_stmt(&mut self, stmt: &mut Stmt) {
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
                                        test: Box::new(py_expr!("True")),
                                        body: value,
                                        elif_else_clauses: Vec::new(),
                                    });
                                }
                                Some(PlaceholderValue::Expr(value)) => {
                                    *stmt = Stmt::If(ast::StmtIf {
                                        node_index: ast::AtomicNodeIndex::default(),
                                        range: TextRange::default(),
                                        test: Box::new(crate::py_expr!("True")),
                                        body: vec![Stmt::Expr(ast::StmtExpr {
                                            node_index: ast::AtomicNodeIndex::default(),
                                            range: TextRange::default(),
                                            value,
                                        })],
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
                return;
            }
            _ => {}
        }
        walk_stmt(self, stmt);
    }
}

pub(crate) struct Flattener;

impl Flattener {
    fn visit_stmts(&mut self, body: &mut Vec<Stmt>) {
        let mut i = 0;
        while i < body.len() {
            self.visit_stmt(&mut body[i]);
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
}

fn remove_placeholder_pass(stmts: &mut Vec<Stmt>) {
    if stmts.len() == 1 {
        if let Stmt::Pass(ast::StmtPass { range, .. }) = &stmts[0] {
            if range.is_empty() {
                stmts.clear();
            }
        }
    }
}

impl Transformer for Flattener {
    fn visit_stmt(&mut self, stmt: &mut Stmt) {
        match stmt {
            Stmt::If(ast::StmtIf {
                body,
                elif_else_clauses,
                ..
            }) => {
                self.visit_stmts(body);
                remove_placeholder_pass(body);
                for clause in elif_else_clauses.iter_mut() {
                    self.visit_stmts(&mut clause.body);
                    remove_placeholder_pass(&mut clause.body);
                }
            }
            Stmt::For(ast::StmtFor {
                body: inner,
                orelse,
                ..
            }) => {
                self.visit_stmts(inner);
                remove_placeholder_pass(inner);
                self.visit_stmts(orelse);
                remove_placeholder_pass(orelse);
            }
            Stmt::While(ast::StmtWhile {
                body: inner,
                orelse,
                ..
            }) => {
                self.visit_stmts(inner);
                remove_placeholder_pass(inner);
                self.visit_stmts(orelse);
                remove_placeholder_pass(orelse);
            }
            Stmt::Try(ast::StmtTry {
                body: inner,
                handlers,
                orelse,
                finalbody,
                ..
            }) => {
                self.visit_stmts(inner);
                remove_placeholder_pass(inner);
                for handler in handlers.iter_mut() {
                    let ast::ExceptHandler::ExceptHandler(ast::ExceptHandlerExceptHandler {
                        body,
                        ..
                    }) = handler;
                    self.visit_stmts(body);
                    remove_placeholder_pass(body);
                }
                self.visit_stmts(orelse);
                remove_placeholder_pass(orelse);
                self.visit_stmts(finalbody);
                remove_placeholder_pass(finalbody);
            }
            Stmt::FunctionDef(ast::StmtFunctionDef { body: inner, .. }) => {
                self.visit_stmts(inner);
                remove_placeholder_pass(inner);
            }
            _ => {}
        }
    }
}

pub(crate) fn flatten(body: &mut Vec<Stmt>) {
    let mut flattener = Flattener;
    flattener.visit_stmts(body);
}

#[cfg(test)]
mod tests {
    use crate::test_util::assert_ast_eq;
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
    fn reuses_identifier() {
        let expr = py_expr!("{name:id} + {name:id}", name = "x");
        let expected = *parse_expression("x + x").unwrap().into_syntax().body;
        assert_eq!(ComparableExpr::from(&expr), ComparableExpr::from(&expected));
    }

    #[test]
    fn inserts_literal() {
        let expr = py_expr!("{s:literal}", s = "abc");
        let expected = *parse_expression("\"abc\"").unwrap().into_syntax().body;
        assert_eq!(ComparableExpr::from(&expr), ComparableExpr::from(&expected));
    }

    #[test]
    fn inserts_int_literal() {
        let expr = py_expr!("{n:literal}", n = 5);
        let expected = *parse_expression("5").unwrap().into_syntax().body;
        assert_eq!(ComparableExpr::from(&expr), ComparableExpr::from(&expected));
    }

    #[test]
    fn inserts_stmt() {
        let body = parse_module(
            "
a = 1
b = 2
",
        )
        .unwrap()
        .into_syntax()
        .body;
        let stmt = py_stmt!("{body:stmt}", body = body.clone());
        assert_ast_eq(
            &[stmt],
            &[py_stmt!(
                "
a = 1
b = 2
",
            )],
        );
    }

    #[test]
    fn wraps_expr_in_stmt() {
        let expr = *parse_expression("a + 1").unwrap().into_syntax().body;
        let mut actual = vec![py_stmt!(
            "
{expr:stmt}
",
            expr = expr,
        )];
        crate::template::flatten(&mut actual);
        assert_ast_eq(
            &actual,
            &[py_stmt!(
                "
a + 1
",
            )],
        );
    }

    #[test]
    fn inserts_function_parts() {
        let body = parse_module("a = 1").unwrap().into_syntax().body;
        let stmt = py_stmt!(
            "
def {func:id}({param:id}):
    {body:stmt}
",
            func = "foo",
            param = "arg",
            body = body.clone(),
        );
        match stmt {
            ruff_python_ast::Stmt::FunctionDef(ast::StmtFunctionDef {
                name,
                parameters,
                body: mut fn_body,
                ..
            }) => {
                crate::template::flatten(&mut fn_body);
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

    #[test]
    fn preserves_else_if() {
        let inner = py_stmt!(
            "
if b:
    x
else:
    y
",
        );
        let mut actual = vec![py_stmt!(
            "
if a:
    z
else:
    {inner:stmt}
",
            inner = vec![inner],
        )];
        crate::template::flatten(&mut actual);
        let mut expected = vec![py_stmt!(
            "
if a:
    z
else:
    if b:
        x
    else:
        y
",
        )];
        crate::template::flatten(&mut expected);
        assert_ast_eq(&actual, &expected);
    }
}
