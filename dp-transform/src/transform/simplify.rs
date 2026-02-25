use crate::{
    transform::context::Context,
    transformer::{walk_body, walk_expr, Transformer},
};
use ruff_python_ast::{self as ast, Expr, Stmt, StmtBody};
use ruff_python_parser::parse_expression;

pub(crate) struct Flattener;

impl Flattener {
    fn visit_body(&mut self, body: &mut StmtBody) {
        let body = &mut body.body;
        let mut i = 0;
        while i < body.len() {
            self.visit_stmt(body[i].as_mut());
            if let Stmt::BodyStmt(ast::StmtBody { body: inner, .. }) = body[i].as_mut() {
                let replacement = std::mem::take(inner);
                body.splice(i..=i, replacement);
                continue;
            }
            if let Stmt::If(ast::StmtIf {
                test,
                body: inner,
                elif_else_clauses,
                ..
            }) = body[i].as_mut()
            {
                if elif_else_clauses.is_empty()
                    && matches!(
                        test.as_ref(),
                        Expr::BooleanLiteral(ast::ExprBooleanLiteral { value: true, .. })
                    )
                {
                    let replacement = std::mem::take(&mut inner.body);
                    body.splice(i..=i, replacement);
                    continue;
                }
            }
            i += 1;
        }
    }
}

fn remove_placeholder_pass(body: &mut StmtBody) {
    let body = &mut body.body;
    if body.len() == 1 {
        if let Stmt::Pass(ast::StmtPass { range, .. }) = body[0].as_ref() {
            if range.is_empty() {
                body.clear();
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
                self.visit_body(body);
                remove_placeholder_pass(body);
                for clause in elif_else_clauses.iter_mut() {
                    self.visit_body(&mut clause.body);
                    remove_placeholder_pass(&mut clause.body);
                }
            }
            Stmt::For(ast::StmtFor {
                body: inner,
                orelse,
                ..
            }) => {
                self.visit_body(inner);
                remove_placeholder_pass(inner);
                self.visit_body(orelse);
                remove_placeholder_pass(orelse);
            }
            Stmt::While(ast::StmtWhile {
                body: inner,
                orelse,
                ..
            }) => {
                self.visit_body(inner);
                remove_placeholder_pass(inner);
                self.visit_body(orelse);
                remove_placeholder_pass(orelse);
            }
            Stmt::Try(ast::StmtTry {
                body: inner,
                handlers,
                orelse,
                finalbody,
                ..
            }) => {
                self.visit_body(inner);
                remove_placeholder_pass(inner);
                for handler in handlers.iter_mut() {
                    let ast::ExceptHandler::ExceptHandler(ast::ExceptHandlerExceptHandler {
                        body,
                        ..
                    }) = handler;
                    self.visit_body(body);
                    remove_placeholder_pass(body);
                }
                self.visit_body(orelse);
                remove_placeholder_pass(orelse);
                self.visit_body(finalbody);
                remove_placeholder_pass(finalbody);
            }
            Stmt::FunctionDef(ast::StmtFunctionDef { body: inner, .. }) => {
                self.visit_body(inner);
                remove_placeholder_pass(inner);
            }
            _ => {}
        }
    }
}

fn is_dp_current_exception_call(expr: &Expr) -> bool {
    let Expr::Call(ast::ExprCall {
        func, arguments, ..
    }) = expr
    else {
        return false;
    };
    if !arguments.args.is_empty() || !arguments.keywords.is_empty() {
        return false;
    }
    is_dp_helper_name(func.as_ref(), "current_exception")
}

fn is_dp_helper_name(func: &Expr, helper: &str) -> bool {
    if matches!(
        func,
        Expr::Name(ast::ExprName { id, .. }) if id.as_str() == format!("__dp_{helper}")
    ) {
        return true;
    }
    matches!(
        func,
        Expr::Attribute(ast::ExprAttribute { value, attr, .. })
            if matches!(value.as_ref(), Expr::Name(ast::ExprName { id, .. }) if id.as_str() == "__dp__")
                && attr.as_str() == helper
    )
}

fn is_nameerror_expr(expr: &Expr) -> bool {
    matches!(
        expr,
        Expr::Name(ast::ExprName { id, .. }) if id.as_str() == "NameError"
    )
}

fn extract_quiet_delitem_args(try_stmt: &ast::StmtTry) -> Option<(Expr, Expr)> {
    if !try_stmt.orelse.body.is_empty() || !try_stmt.finalbody.body.is_empty() {
        return None;
    }
    if try_stmt.body.body.len() != 1 || try_stmt.handlers.len() != 1 {
        return None;
    }

    let Stmt::Expr(ast::StmtExpr { value, .. }) = try_stmt.body.body[0].as_ref() else {
        return None;
    };
    let Expr::Call(ast::ExprCall {
        func, arguments, ..
    }) = value.as_ref()
    else {
        return None;
    };
    if arguments.args.len() != 2 || !arguments.keywords.is_empty() {
        return None;
    }
    if !is_dp_helper_name(func.as_ref(), "delitem") {
        return None;
    }
    let del_obj = arguments.args[0].clone();
    let del_key = arguments.args[1].clone();

    let ast::ExceptHandler::ExceptHandler(ast::ExceptHandlerExceptHandler {
        type_,
        name,
        body,
        ..
    }) = &try_stmt.handlers[0];
    if type_.is_some() || name.is_some() || body.body.len() != 1 {
        return None;
    }
    let Stmt::If(ast::StmtIf {
        test,
        body: if_body,
        elif_else_clauses,
        ..
    }) = body.body[0].as_ref()
    else {
        return None;
    };
    let Expr::Call(ast::ExprCall {
        func,
        arguments: test_arguments,
        ..
    }) = test.as_ref()
    else {
        return None;
    };
    if test_arguments.args.len() != 2 || !test_arguments.keywords.is_empty() {
        return None;
    }
    if !is_dp_helper_name(func.as_ref(), "exception_matches") {
        return None;
    }
    if !is_dp_current_exception_call(&test_arguments.args[0])
        || !is_nameerror_expr(&test_arguments.args[1])
    {
        return None;
    }
    if if_body.body.len() != 1 || !matches!(if_body.body[0].as_ref(), Stmt::Pass(_)) {
        return None;
    }
    if elif_else_clauses.len() != 1 {
        return None;
    }
    let else_clause = &elif_else_clauses[0];
    if else_clause.test.is_some() {
        return None;
    }
    if else_clause.body.body.len() != 1 {
        return None;
    }
    if !matches!(
        else_clause.body.body[0].as_ref(),
        Stmt::Raise(ast::StmtRaise {
            exc: None,
            cause: None,
            ..
        })
    ) {
        return None;
    }

    Some((del_obj, del_key))
}

struct StripGeneratedPasses;

impl Transformer for &mut StripGeneratedPasses {
    fn visit_body(&mut self, body: &mut StmtBody) {
        walk_body(self, body);
        let mut updated = Vec::with_capacity(body.body.len());
        for stmt in body.body.drain(..) {
            let stmt = *stmt;
            match stmt {
                Stmt::Try(try_stmt) => {
                    if let Some((obj, key)) = extract_quiet_delitem_args(&try_stmt) {
                        updated.push(crate::py_stmt!(
                            "__dp_delitem_quietly({obj:expr}, {key:expr})",
                            obj = obj,
                            key = key,
                        ));
                    } else {
                        updated.push(Stmt::Try(try_stmt));
                    }
                    continue;
                }
                Stmt::If(mut if_stmt) => {
                    if if_stmt.body.body.is_empty() {
                        if_stmt.body.body.push(Box::new(Stmt::Pass(ast::StmtPass {
                            node_index: Default::default(),
                            range: Default::default(),
                        })));
                    }
                    for clause in if_stmt.elif_else_clauses.iter_mut() {
                        if clause.body.body.is_empty() {
                            clause.body.body.push(Box::new(Stmt::Pass(ast::StmtPass {
                                node_index: Default::default(),
                                range: Default::default(),
                            })));
                        }
                    }
                    if_stmt.elif_else_clauses.retain(|clause| {
                        clause.body.body.len().ne(&1)
                            || !matches!(clause.body.body[0].as_ref(), Stmt::Pass(_))
                    });

                    let is_empty_if = if_stmt.body.body.len().eq(&1)
                        && matches!(if_stmt.body.body[0].as_ref(), Stmt::Pass(_))
                        && if_stmt.elif_else_clauses.is_empty();
                    if is_empty_if {
                        updated.push(crate::py_stmt!(
                            "__dp_truth({expr:expr})",
                            expr = if_stmt.test
                        ));
                        continue;
                    }

                    updated.push(Stmt::If(if_stmt));
                    continue;
                }
                Stmt::Expr(ast::StmtExpr { ref value, .. })
                    if matches!(
                        value.as_ref(),
                        Expr::Name(ast::ExprName { id, .. })
                            if id.as_str().starts_with("_dp_")
                    ) =>
                {
                    continue;
                }
                other => {
                    updated.push(other);
                    continue;
                }
            }
        }

        if updated.len() > 1 {
            updated.retain(|stmt| !matches!(stmt, Stmt::Pass(_)));

            if updated.is_empty() {
                updated.push(crate::py_stmt!("pass"));
            }
        }

        body.body = updated.into_iter().map(Box::new).collect();
    }
}

pub fn flatten(stmts: &mut StmtBody) {
    let mut flattener = Flattener;
    (&mut flattener).visit_body(stmts);
}

pub fn strip_generated_passes(_context: &Context, stmts: &mut StmtBody) {
    flatten(stmts);

    let mut stripper = StripGeneratedPasses;
    (&mut stripper).visit_body(stmts);
}

fn string_to_str_bytes_expr(value: &str) -> Expr {
    let mut source = String::from("__dp_decode_literal_bytes(b\"");
    source.push_str(&escape_bytes_for_double_quoted_literal(value.as_bytes()));
    source.push_str("\")");
    let parsed = parse_expression(&source).unwrap_or_else(|err| {
        panic!("failed to build lowered string literal expression from {source:?}: {err}")
    });
    *parsed.into_syntax().body
}

fn escape_bytes_for_double_quoted_literal(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 4);
    for &byte in bytes {
        match byte {
            b'\\' => out.push_str("\\\\"),
            b'"' => out.push_str("\\\""),
            b'\n' => out.push_str("\\n"),
            b'\r' => out.push_str("\\r"),
            b'\t' => out.push_str("\\t"),
            0x20..=0x7e => out.push(byte as char),
            _ => out.push_str(&format!("\\x{:02x}", byte)),
        }
    }
    out
}

fn is_docstring_stmt(stmt: &Stmt) -> bool {
    matches!(
        stmt,
        Stmt::Expr(ast::StmtExpr { value, .. }) if matches!(value.as_ref(), Expr::StringLiteral(_))
    )
}

struct StringBytesLowerer;

impl Transformer for &mut StringBytesLowerer {
    fn visit_body(&mut self, body: &mut StmtBody) {
        for (index, stmt) in body.body.iter_mut().enumerate() {
            if index == 0 && is_docstring_stmt(stmt) {
                continue;
            }
            self.visit_stmt(stmt);
        }
    }

    fn visit_expr(&mut self, expr: &mut Expr) {
        if let Expr::Call(call) = expr {
            let is_dp_getattr = call.arguments.keywords.is_empty()
                && call.arguments.args.len() == 2
                && matches!(
                    call.func.as_ref(),
                    Expr::Name(name) if name.id.as_str() == "__dp_getattr"
                )
                && matches!(&call.arguments.args[1], Expr::StringLiteral(_));
            if is_dp_getattr {
                self.visit_expr(call.func.as_mut());
                self.visit_expr(&mut call.arguments.args[0]);
                return;
            }
        }
        walk_expr(self, expr);
        if let Expr::StringLiteral(ast::ExprStringLiteral { value, .. }) = expr {
            *expr = string_to_str_bytes_expr(value.to_string().as_str());
        }
    }
}

pub fn lower_string_literals_to_bytes(stmts: &mut StmtBody) {
    let mut lowerer = StringBytesLowerer;
    (&mut lowerer).visit_body(stmts);
}
