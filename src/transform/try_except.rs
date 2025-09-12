use std::cell::Cell;

use ruff_python_ast::visitor::transformer::{walk_stmt, Transformer};
use ruff_python_ast::{self as ast, Expr, Stmt};

pub struct TryExceptRewriter {
    count: Cell<usize>,
}

impl TryExceptRewriter {
    pub fn new() -> Self {
        Self {
            count: Cell::new(0),
        }
    }
}

impl Transformer for TryExceptRewriter {
    fn visit_stmt(&self, stmt: &mut Stmt) {
        walk_stmt(self, stmt);

        if let Stmt::Try(ast::StmtTry {
            body,
            handlers,
            orelse,
            finalbody,
            is_star,
            ..
        }) = stmt
        {
            if handlers.is_empty() {
                return;
            }

            let id = self.count.get() + 1;
            self.count.set(id);
            let exc_name = format!("_dp_exc_{}", id);

            let body_stmts = std::mem::take(body);
            let orelse_stmts = std::mem::take(orelse);
            let final_stmts = std::mem::take(finalbody);
            let handlers_vec = std::mem::take(handlers);

            let exc_assign = crate::py_stmt!(
                "
{exc:id} = __dp__.current_exception()
",
                exc = exc_name.as_str(),
            );
            let exc_expr = crate::py_expr!("{exc:id}", exc = exc_name.as_str());

            let mut processed: Vec<(Option<Expr>, Vec<Stmt>)> = Vec::new();
            for handler in handlers_vec {
                let ast::ExceptHandler::ExceptHandler(ast::ExceptHandlerExceptHandler {
                    type_,
                    name,
                    body,
                    ..
                }) = handler;
                let mut body_stmts = body;
                if let Some(name) = name {
                    let assign = crate::py_stmt!(
                        "{name:id} = {exc:id}",
                        name = name.id.as_str(),
                        exc = exc_name.as_str(),
                    );
                    body_stmts.insert(0, assign);
                }
                processed.push((type_.map(|e| *e), body_stmts));
            }

            let mut new_body = vec![exc_assign];
            let mut chain: Vec<Stmt> = vec![crate::py_stmt!("raise")];
            for (type_, body) in processed.into_iter().rev() {
                chain = if let Some(t) = type_ {
                    vec![crate::py_stmt!(
                        "
if __dp__.isinstance({exc:expr}, {typ:expr}):
    {body:stmt}
else:
    {next:stmt}
",
                        exc = exc_expr.clone(),
                        typ = t,
                        body = body,
                        next = chain,
                    )]
                } else {
                    body
                };
            }
            new_body.extend(chain);

            let mut try_stmt = crate::py_stmt!(
                "
try:
    {body:stmt}
except:
    {handler:stmt}
",
                body = body_stmts,
                handler = new_body,
            );

            if let Stmt::Try(ast::StmtTry {
                orelse,
                finalbody,
                is_star: star,
                ..
            }) = &mut try_stmt
            {
                *orelse = orelse_stmts;
                *finalbody = final_stmts;
                *star = *is_star;
            }

            *stmt = try_stmt;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assert_flatten_eq;
    use ruff_python_ast::visitor::transformer::walk_body;
    use ruff_python_parser::parse_module;

    fn rewrite_try(source: &str) -> Vec<Stmt> {
        let parsed = parse_module(source).expect("parse error");
        let mut module = parsed.into_syntax();
        let rewriter = TryExceptRewriter::new();
        walk_body(&rewriter, &mut module.body);
        module.body
    }

    #[test]
    fn rewrites_typed_except() {
        let input = r#"
try:
    f()
except E as e:
    g(e)
"#;
        let expected = r#"
try:
    f()
except:
    _dp_exc_1 = __dp__.current_exception()
    if __dp__.isinstance(_dp_exc_1, E):
        e = _dp_exc_1
        g(e)
    else:
        raise
"#;
        let output = rewrite_try(input);
        assert_flatten_eq!(output, expected);
    }

    #[test]
    fn rewrites_with_bare_except() {
        let input = r#"
try:
    f()
except E:
    h()
except:
    g()
"#;
        let expected = r#"
try:
    f()
except:
    _dp_exc_1 = __dp__.current_exception()
    if __dp__.isinstance(_dp_exc_1, E):
        h()
    else:
        g()
"#;
        let output = rewrite_try(input);
        assert_flatten_eq!(output, expected);
    }
}
