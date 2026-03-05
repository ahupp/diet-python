use super::symbol_analysis::load_names_in_stmt;
use crate::transformer::{walk_expr, walk_stmt, Transformer};
use crate::{py_expr, py_stmt};
use ruff_python_ast::{self as ast, Expr, Stmt};
use ruff_python_parser::parse_expression;

pub(super) fn make_param_specs_expr(parameters: &ast::Parameters) -> Expr {
    let mut specs = Vec::new();
    for param in &parameters.posonlyargs {
        push_param_specs(
            &mut specs,
            param.parameter.name.id.as_str(),
            "/",
            param.parameter.annotation.as_deref(),
            param.default.as_deref(),
        );
    }
    for param in &parameters.args {
        push_param_specs(
            &mut specs,
            param.parameter.name.id.as_str(),
            "",
            param.parameter.annotation.as_deref(),
            param.default.as_deref(),
        );
    }
    if let Some(param) = &parameters.vararg {
        push_param_specs(
            &mut specs,
            param.name.id.as_str(),
            "*",
            param.annotation.as_deref(),
            None,
        );
    }
    for param in &parameters.kwonlyargs {
        push_param_specs(
            &mut specs,
            param.parameter.name.id.as_str(),
            "kw:",
            param.parameter.annotation.as_deref(),
            param.default.as_deref(),
        );
    }
    if let Some(param) = &parameters.kwarg {
        push_param_specs(
            &mut specs,
            param.name.id.as_str(),
            "**",
            param.annotation.as_deref(),
            None,
        );
    }
    make_dp_tuple(specs)
}

pub(super) fn make_dp_tuple(items: Vec<Expr>) -> Expr {
    let Expr::Call(mut call) = py_expr!("__dp_tuple()") else {
        panic!("expected call expression for __dp_tuple");
    };
    call.arguments.args = items.into();
    Expr::Call(call)
}

pub(super) fn raise_stmt_from_name(name: &str) -> ast::StmtRaise {
    match py_stmt!("raise {exc:id}", exc = name) {
        Stmt::Raise(raise_stmt) => raise_stmt,
        _ => unreachable!("expected raise statement"),
    }
}

pub(super) fn rewrite_exception_accesses(mut body: Vec<Box<Stmt>>, exc_name: &str) -> Vec<Box<Stmt>> {
    let mut rewriter = ExceptExceptionRewriter {
        exception_name: exc_name.to_string(),
    };
    for stmt in body.iter_mut() {
        rewriter.visit_stmt(stmt.as_mut());
    }
    body
}

pub(super) fn body_uses_name(body: &[Box<Stmt>], name: &str) -> bool {
    body.iter()
        .any(|stmt| load_names_in_stmt(stmt.as_ref()).contains(name))
}

pub(super) fn name_expr(name: &str) -> Option<Expr> {
    parse_expression(name)
        .ok()
        .map(|expr| *expr.into_syntax().body)
}

struct ExceptExceptionRewriter {
    exception_name: String,
}

impl ExceptExceptionRewriter {
    fn exception_name_expr(&self) -> Expr {
        py_expr!("{name:id}", name = self.exception_name.as_str())
    }
}

impl Transformer for ExceptExceptionRewriter {
    fn visit_stmt(&mut self, stmt: &mut Stmt) {
        match stmt {
            Stmt::FunctionDef(_) | Stmt::ClassDef(_) => {}
            Stmt::Raise(raise_stmt) if raise_stmt.exc.is_none() && raise_stmt.cause.is_none() => {
                raise_stmt.exc = Some(Box::new(self.exception_name_expr()));
            }
            _ => walk_stmt(self, stmt),
        }
    }

    fn visit_expr(&mut self, expr: &mut Expr) {
        if let Expr::Call(call) = expr {
            if call.arguments.args.is_empty() && call.arguments.keywords.is_empty() {
                if is_dp_lookup_call(call.func.as_ref(), "current_exception") {
                    *expr = self.exception_name_expr();
                    return;
                }
                if is_dp_lookup_call(call.func.as_ref(), "exc_info") {
                    *expr = py_expr!(
                        "__dp_exc_info_from_exception({exc:id})",
                        exc = self.exception_name.as_str(),
                    );
                    return;
                }
            }
        }
        walk_expr(self, expr);
    }
}

pub(super) fn is_dp_lookup_call(func: &Expr, attr_name: &str) -> bool {
    if matches!(
        func,
        Expr::Name(name) if name.id.as_str() == format!("__dp_{attr_name}")
    ) {
        return true;
    }
    if let Expr::Attribute(attr) = func {
        if attr.attr.as_str() == attr_name {
            if let Expr::Name(module) = attr.value.as_ref() {
                return module.id.as_str() == "__dp__";
            }
        }
    }
    if let Expr::Call(call) = func {
        if !call.arguments.keywords.is_empty() || call.arguments.args.len() != 2 {
            return false;
        }
        if !matches!(
            call.func.as_ref(),
            Expr::Name(name) if name.id.as_str() == "__dp_getattr"
        ) {
            return false;
        }
        let base_matches = matches!(
            &call.arguments.args[0],
            Expr::Name(base) if base.id.as_str() == "__dp__"
        );
        if !base_matches {
            return false;
        }
        return expr_static_str(&call.arguments.args[1]).as_deref() == Some(attr_name);
    }
    false
}

fn expr_static_str(expr: &Expr) -> Option<String> {
    match expr {
        Expr::StringLiteral(value) => Some(value.value.to_str().to_string()),
        Expr::Call(call)
            if call.arguments.keywords.is_empty()
                && call.arguments.args.len() == 1
                && matches!(
                    call.func.as_ref(),
                    Expr::Name(name)
                        if matches!(
                            name.id.as_str(),
                            "__dp_decode_literal_bytes" | "__dp_decode_literal_source_bytes"
                        )
                ) =>
        {
            match &call.arguments.args[0] {
                Expr::BytesLiteral(bytes) => {
                    let value: std::borrow::Cow<[u8]> = (&bytes.value).into();
                    String::from_utf8(value.into_owned()).ok()
                }
                _ => None,
            }
        }
        _ => None,
    }
}

fn push_param_specs(
    specs: &mut Vec<Expr>,
    name: &str,
    prefix: &str,
    _annotation: Option<&Expr>,
    default: Option<&Expr>,
) {
    let label = format!("{prefix}{name}");
    let annotation_expr = py_expr!("None");
    let default_expr = default
        .cloned()
        .unwrap_or_else(|| py_expr!("__dp__.NO_DEFAULT"));
    specs.push(make_dp_tuple(vec![
        py_expr!("{value:literal}", value = label.as_str()),
        annotation_expr,
        default_expr,
    ]));
}
