#[macro_export]
macro_rules! py_expr {
    ($template:literal $(, $name:ident = $value:expr)* $(,)?) => {{
        use ruff_python_ast::{self as ast, Stmt};
        use crate::py_stmt;
        let stmts = py_stmt!($template $(, $name = $value)*);
        if stmts.len() != 1 {
            panic!("expected single statement");
        }
        let stmt = stmts.into_iter().next().unwrap();
        match stmt {
            Stmt::Expr(ast::StmtExpr { value, .. }) => *value,
            _ => panic!("expected expression statement"),
        }
    }};
}

#[macro_export]
macro_rules! py_stmt {
    ($template:literal $(, $name:ident = $value:expr)* $(,)?) => {{
        use std::collections::HashMap;
        #[allow(unused_imports)]
        use crate::template::{PlaceholderValue, SyntaxTemplate};

        #[allow(unused_mut)]
        let mut values: HashMap<&str, PlaceholderValue> = HashMap::new();
        #[allow(unused_mut)]
        let mut ids: HashMap<&str, serde_json::Value> = HashMap::new();
        $(match $crate::template::IntoPlaceholder::into_placeholder($value) {
            Ok(value) => { values.insert(stringify!($name), value); }
            Err(id) => { ids.insert(stringify!($name), id); }
        });*

        static TEMPLATE: ::std::sync::LazyLock<$crate::template::SyntaxTemplate> =
            ::std::sync::LazyLock::new(|| {
                $crate::template::SyntaxTemplate::parse($template)
            });

        let template = (*TEMPLATE).clone();
        let values = values.into_iter().map(|(name, value)| (name.to_string(), value)).collect();
        let ids = ids.into_iter().map(|(name, value)| (name.to_string(), value)).collect();
        template.instantiate(values, ids)
    }};
}

use crate::body_transform::{walk_expr, walk_keyword, walk_parameter, walk_stmt, Transformer};
use regex::Regex;
use ruff_python_ast::{self as ast, Expr, Stmt};
use ruff_python_parser::parse_expression;
use ruff_text_size::TextRange;
use serde_json::Value;
use std::{collections::HashMap, sync::LazyLock};

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

impl IntoPlaceholder for Box<Stmt> {
    fn into_placeholder(self) -> Result<PlaceholderValue, Value> {
        Ok(PlaceholderValue::Stmt(vec![*self]))
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

pub(crate) fn var_for_placeholder(name: &str, ty: PlaceholderType) -> String {
    match ty {
        PlaceholderType::Expr => format!("_dp_placeholder_expr_{}__", name),
        PlaceholderType::Stmt => format!("_dp_placeholder_stmt_{}__", name),
        PlaceholderType::Identifier => format!("_dp_placeholder_id_{}__", name),
        PlaceholderType::Literal => format!("_dp_placeholder_literal_{}__", name),
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PlaceholderType {
    Expr,
    Stmt,
    Identifier,
    Literal,
}

#[derive(Clone, Debug)]
pub(crate) struct SyntaxTemplate {
    stmts: Vec<Stmt>,
}

impl SyntaxTemplate {
    pub(crate) fn parse(template: &str) -> Self {
        let regex = placeholder_template_regex();
        let src = regex
            .replace_all(template, |caps: &regex::Captures| {
                let name = caps.get(1).unwrap().as_str();
                let ty = match caps.get(2).unwrap().as_str() {
                    "expr" => PlaceholderType::Expr,
                    "stmt" => PlaceholderType::Stmt,
                    "id" => PlaceholderType::Identifier,
                    "literal" => PlaceholderType::Literal,
                    other => panic!("unknown placeholder type `{other}` for `{name}`"),
                };
                var_for_placeholder(name, ty)
            })
            .to_string();

        let module = match ruff_python_parser::parse_module(&src) {
            Ok(module) => module.into_syntax(),
            Err(e) => {
                println!("template parse error: {}\n{}", e, src);
                panic!("template parse error");
            }
        };

        Self { stmts: module.body }
    }

    pub(crate) fn instantiate(
        mut self,
        values: HashMap<String, PlaceholderValue>,
        ids: HashMap<String, Value>,
    ) -> Vec<Stmt> {
        let mut transformer = PlaceholderReplacer::new(values, ids);
        transformer.visit_body(&mut self.stmts);
        transformer.finish();
        self.stmts
    }
}

struct PlaceholderReplacer {
    values: HashMap<String, PlaceholderValue>,
    ids: HashMap<String, Value>,
}

impl PlaceholderReplacer {
    fn new(values: HashMap<String, PlaceholderValue>, ids: HashMap<String, Value>) -> Self {
        Self { values, ids }
    }

    fn parse_placeholder<'b>(&self, symbol: &'b str) -> Option<(PlaceholderType, &'b str)> {
        placeholder_regex().captures(symbol).map(|caps| {
            let ty = match caps.name("ty").unwrap().as_str() {
                "expr" => PlaceholderType::Expr,
                "stmt" => PlaceholderType::Stmt,
                "id" => PlaceholderType::Identifier,
                "literal" => PlaceholderType::Literal,
                other => panic!("unknown placeholder type `{other}`"),
            };
            let name = caps.name("name").unwrap().as_str();
            (ty, name)
        })
    }

    fn take_value(&mut self, name: &str) -> PlaceholderValue {
        self.values
            .remove(name)
            .unwrap_or_else(|| panic!("expected value for placeholder {name}"))
    }

    fn get_id(&self, name: &str) -> Value {
        self.ids
            .get(name)
            .cloned()
            .unwrap_or_else(|| panic!("expected id or literal for placeholder {name}"))
    }

    fn replace_identifier(&mut self, identifier: &mut ast::Identifier) {
        if let Some((ty, name)) = self.parse_placeholder(identifier.id.as_str()) {
            match ty {
                PlaceholderType::Identifier => {
                    let value = self.get_id(name);
                    identifier.id = identifier_string(name, value).into();
                    return;
                }
                PlaceholderType::Literal | PlaceholderType::Expr | PlaceholderType::Stmt => {
                    panic!("unexpected placeholder `{name}` in identifier context");
                }
            }
        }

        let original = identifier.id.as_str();
        let regex = placeholder_text_regex();
        if regex.is_match(original) {
            let mut result = String::with_capacity(original.len());
            let mut last_end = 0;
            for caps in regex.captures_iter(original) {
                let mat = caps.get(0).unwrap();
                result.push_str(&original[last_end..mat.start()]);
                let ty = caps.name("ty").unwrap().as_str();
                let name = caps.name("name").unwrap().as_str();
                match ty {
                    "id" => {
                        let value = self.get_id(name);
                        result.push_str(&identifier_string(name, value));
                    }
                    other => panic!("unsupported placeholder type `{other}` in identifier"),
                }
                last_end = mat.end();
            }
            result.push_str(&original[last_end..]);
            identifier.id = result.into();
        }
    }

    fn replace_optional_identifier(&mut self, identifier: &mut Option<ast::Identifier>) {
        if let Some(identifier) = identifier {
            self.replace_identifier(identifier);
        }
    }

    fn replace_name(&mut self, name: &mut ruff_python_ast::name::Name) {
        let original = name.as_str();
        let regex = placeholder_text_regex();
        if regex.is_match(original) {
            let mut result = String::with_capacity(original.len());
            let mut last_end = 0;
            for caps in regex.captures_iter(original) {
                let mat = caps.get(0).unwrap();
                result.push_str(&original[last_end..mat.start()]);
                let ty = caps.name("ty").unwrap().as_str();
                let placeholder = caps.name("name").unwrap().as_str();
                match ty {
                    "id" => {
                        let value = self.get_id(placeholder);
                        result.push_str(&identifier_string(placeholder, value));
                    }
                    other => panic!("unsupported placeholder type `{other}` in name"),
                }
                last_end = mat.end();
            }
            result.push_str(&original[last_end..]);
            *name = result.into();
        }
    }

    fn finish(self) {
        if !self.values.is_empty() {
            let keys: Vec<_> = self.values.keys().cloned().collect();
            panic!("unused placeholders: {}", keys.join(", "));
        }
    }
}

impl Transformer for PlaceholderReplacer {
    fn visit_expr(&mut self, expr: &mut Expr) {
        match expr {
            Expr::Name(ast::ExprName { id, .. }) => {
                if let Some((placeholder_type, name)) = self.parse_placeholder(id.as_str()) {
                    match placeholder_type {
                        PlaceholderType::Expr => match self.take_value(name) {
                            PlaceholderValue::Expr(value) => {
                                *expr = *value;
                                return;
                            }
                            PlaceholderValue::Stmt(_) => {
                                panic!("expected expr for placeholder {name}");
                            }
                        },
                        PlaceholderType::Identifier => {
                            let value = self.get_id(name);
                            *expr = identifier_expr(name, value);
                            return;
                        }
                        PlaceholderType::Literal => {
                            let value = self.get_id(name);
                            *expr = literal_expr(name, value);
                            return;
                        }
                        PlaceholderType::Stmt => {
                            panic!("expected stmt placeholder {name}");
                        }
                    }
                }
                self.replace_name(id);
            }
            Expr::Attribute(ast::ExprAttribute { attr, .. }) => {
                self.replace_identifier(attr);
            }
            _ => {}
        }
        walk_expr(self, expr);
    }

    fn visit_stmt(&mut self, stmt: &mut Stmt) {
        match stmt {
            Stmt::FunctionDef(func) => {
                self.replace_identifier(&mut func.name);
                walk_stmt(self, stmt);
                return;
            }
            Stmt::ClassDef(class_def) => {
                self.replace_identifier(&mut class_def.name);
            }
            _ => {}
        }

        match stmt {
            Stmt::Expr(ast::StmtExpr { value, .. }) => {
                if let Expr::Name(ast::ExprName { id, .. }) = value.as_ref() {
                    if let Some((placeholder_type, name)) = self.parse_placeholder(id.as_str()) {
                        if matches!(placeholder_type, PlaceholderType::Stmt) {
                            match self.take_value(name) {
                                PlaceholderValue::Stmt(value) => {
                                    *stmt = Stmt::If(ast::StmtIf {
                                        node_index: ast::AtomicNodeIndex::default(),
                                        range: TextRange::default(),
                                        test: Box::new(py_expr!("True")),
                                        body: value,
                                        elif_else_clauses: Vec::new(),
                                    });
                                }
                                PlaceholderValue::Expr(value) => {
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
                            }
                        }
                    }
                }
            }
            _ => {}
        }
        walk_stmt(self, stmt);
    }

    fn visit_parameter(&mut self, parameter: &mut ast::Parameter) {
        self.replace_identifier(&mut parameter.name);
        walk_parameter(self, parameter);
    }

    fn visit_keyword(&mut self, keyword: &mut ast::Keyword) {
        self.replace_optional_identifier(&mut keyword.arg);
        walk_keyword(self, keyword);
    }

    fn visit_alias(&mut self, alias: &mut ast::Alias) {
        self.replace_identifier(&mut alias.name);
        self.replace_optional_identifier(&mut alias.asname);
    }
}

fn placeholder_regex() -> &'static Regex {
    static REGEX: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(
            r"^_dp_placeholder_(?P<ty>expr|stmt|id|literal)_(?P<name>[a-zA-Z_][a-zA-Z0-9_]*)__$",
        )
        .unwrap()
    });
    &REGEX
}

fn placeholder_template_regex() -> &'static Regex {
    static REGEX: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"\{([a-zA-Z_][a-zA-Z0-9_]*)\:([a-zA-Z_][a-zA-Z0-9_]*)\}").unwrap()
    });
    &REGEX
}

fn placeholder_text_regex() -> &'static Regex {
    static REGEX: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(
            r"_dp_placeholder_(?P<ty>expr|stmt|id|literal)_(?P<name>[a-zA-Z_][a-zA-Z0-9_]*)__",
        )
        .unwrap()
    });
    &REGEX
}

fn identifier_string(name: &str, value: Value) -> String {
    match value {
        Value::String(value) => value,
        other => panic!("expected string identifier for placeholder `{name}`, got {other:?}"),
    }
}

fn identifier_expr(name: &str, value: Value) -> Expr {
    let identifier = identifier_string(name, value);
    parse_expression(&identifier)
        .map(|expr| *expr.into_syntax().body)
        .unwrap_or_else(|err| {
            panic!("failed to parse identifier `{identifier}` for placeholder `{name}`: {err}")
        })
}

fn literal_expr(name: &str, value: Value) -> Expr {
    match value {
        Value::Bool(true) => parse_constant_expr("True", name),
        Value::Bool(false) => parse_constant_expr("False", name),
        Value::Null => parse_constant_expr("None", name),
        Value::String(value) => {
            let src = serde_json::to_string(&value).expect("failed to serialize literal");
            parse_dynamic_expr(&src, name)
        }
        Value::Number(value) => parse_dynamic_expr(&value.to_string(), name),
        other => panic!("unsupported literal for placeholder `{name}`: {other:?}"),
    }
}

fn parse_constant_expr(src: &str, name: &str) -> Expr {
    parse_dynamic_expr(src, name)
}

fn parse_dynamic_expr(src: &str, name: &str) -> Expr {
    parse_expression(src)
        .map(|expr| *expr.into_syntax().body)
        .unwrap_or_else(|err| {
            panic!("failed to parse literal `{src}` for placeholder `{name}`: {err}")
        })
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

        assert_ast_eq(
            py_stmt!("{body:stmt}", body = body.clone()),
            py_stmt!(
                "
a = 1
b = 2
",
            ),
        );
    }

    #[test]
    fn inserts_boxed_stmt() {
        let mut body = parse_module("a = 1").unwrap().into_syntax().body;
        let stmt = body.pop().unwrap();
        let actual = py_stmt!("{body:stmt}", body = Box::new(stmt.clone()));
        let expected = py_stmt!("{body:stmt}", body = vec![stmt]);
        assert_ast_eq(actual, expected);
    }

    #[test]
    fn wraps_expr_in_stmt() {
        let expr = *parse_expression("a + 1").unwrap().into_syntax().body;
        let mut actual = py_stmt!(
            "
{expr:stmt}
",
            expr = expr,
        );
        crate::template::flatten(&mut actual);
        assert_ast_eq(
            actual,
            py_stmt!(
                "
a + 1
",
            ),
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
        match stmt.into_iter().next().unwrap() {
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
        let mut actual = py_stmt!(
            "
if a:
    z
else:
    {inner:stmt}
",
            inner = inner,
        );
        crate::template::flatten(&mut actual);
        let mut expected = py_stmt!(
            "
if a:
    z
else:
    if b:
        x
    else:
        y
",
        );
        crate::template::flatten(&mut expected);
        assert_ast_eq(actual, expected);
    }

    #[test]
    fn inserts_boxed_expr() {
        let expr = *parse_expression("a + 1").unwrap().into_syntax().body;
        let actual = py_stmt!("return {expr:expr}", expr = Box::new(expr.clone()));
        let expected = py_stmt!("return {expr:expr}", expr = expr);
        assert_ast_eq(actual, expected);
    }
}
