use std::cell::{Cell, RefCell};

use ruff_python_ast::name::Name;
use ruff_python_ast::visitor::transformer::{walk_expr, walk_stmt, Transformer};
use ruff_python_ast::{self as ast, Expr, Stmt};
use ruff_text_size::TextRange;

use crate::comprehension::rewrite_comprehension;

pub struct GeneratorRewriter {
    gen_count: Cell<usize>,
    scopes: RefCell<Vec<Vec<Stmt>>>,
}

impl GeneratorRewriter {
    pub fn new() -> Self {
        Self {
            gen_count: Cell::new(0),
            scopes: RefCell::new(Vec::new()),
        }
    }

    fn push_scope(&self) {
        self.scopes.borrow_mut().push(Vec::new());
    }

    fn pop_scope(&self) -> Vec<Stmt> {
        self.scopes.borrow_mut().pop().unwrap()
    }

    fn add_function(&self, func: Stmt) {
        self.scopes.borrow_mut().last_mut().unwrap().push(func);
    }

    pub fn rewrite_body(&self, body: &mut Vec<Stmt>) {
        self.push_scope();
        for stmt in body.iter_mut() {
            self.visit_stmt(stmt);
        }
        let functions = self.pop_scope();
        if !functions.is_empty() {
            body.splice(0..0, functions);
        }
    }
}

impl Transformer for GeneratorRewriter {
    fn visit_stmt(&self, stmt: &mut Stmt) {
        if matches!(stmt, Stmt::FunctionDef(_)) {
            self.push_scope();
            walk_stmt(self, stmt);
            let functions = self.pop_scope();
            if !functions.is_empty() {
                if let Stmt::FunctionDef(ast::StmtFunctionDef { body, .. }) = stmt {
                    body.splice(0..0, functions);
                }
            }
        } else {
            walk_stmt(self, stmt);
        }
    }

    fn visit_expr(&self, expr: &mut Expr) {
        if rewrite_comprehension(self, expr) {
            return;
        }

        walk_expr(self, expr);
        if let Expr::Generator(gen) = expr {
            let first_iter_expr = gen.generators.first().unwrap().iter.clone();

            let id = self.gen_count.get() + 1;
            self.gen_count.set(id);
            let func_name = format!("__dp_gen_{}", id);

            let param_name = if let Expr::Name(ast::ExprName { id, .. }) = &first_iter_expr {
                id.clone()
            } else {
                Name::new(format!("__dp_iter_{}", id))
            };

            let mut body = vec![crate::py_stmt!(
                "yield {value:expr}",
                value = (*gen.elt).clone(),
            )];

            for comp in gen.generators.iter().rev() {
                let mut inner = body;
                for if_expr in comp.ifs.iter().rev() {
                    inner = vec![Stmt::If(ast::StmtIf {
                        node_index: ast::AtomicNodeIndex::default(),
                        range: TextRange::default(),
                        test: Box::new(if_expr.clone()),
                        body: inner,
                        elif_else_clauses: Vec::new(),
                    })];
                }
                let for_stmt = Stmt::For(ast::StmtFor {
                    node_index: ast::AtomicNodeIndex::default(),
                    range: TextRange::default(),
                    is_async: comp.is_async,
                    target: Box::new(comp.target.clone()),
                    iter: Box::new(comp.iter.clone()),
                    body: inner,
                    orelse: Vec::new(),
                });
                body = vec![for_stmt];
            }

            if let Stmt::For(ast::StmtFor { iter, .. }) = body.first_mut().unwrap() {
                *iter = Box::new(crate::py_expr!("{name:id}", name = param_name.as_str()));
            }

            let func_def = crate::py_stmt!(
                "
def {func:id}({param:id}):
    {body:stmt}
",
                func = func_name.as_str(),
                param = param_name.as_str(),
                body = body,
            );

            self.add_function(func_def);

            *expr = crate::py_expr!(
                "{func:id}({iter:expr})",
                iter = first_iter_expr,
                func = func_name.as_str()
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ruff_python_codegen::{Generator as Codegen, Stylist};
    use ruff_python_parser::parse_module;

    fn rewrite_gen(source: &str) -> String {
        let parsed = parse_module(source).expect("parse error");
        let tokens = parsed.tokens().clone();
        let mut module = parsed.into_syntax();

        let rewriter = GeneratorRewriter::new();
        rewriter.rewrite_body(&mut module.body);

        let stylist = Stylist::from_tokens(&tokens, source);
        let mut output = String::new();
        for stmt in &module.body {
            let snippet = Codegen::from(&stylist).stmt(stmt);
            output.push_str(&snippet);
            output.push_str(stylist.line_ending().as_str());
        }
        output
    }

    #[test]
    fn rewrites_generator_expressions() {
        let input = "r = (a + 1 for a in items if a % 2 == 0)";
        let expected = r#"
def __dp_gen_1(items):
    for a in items:
        if a % 2 == 0:
            yield a + 1
r = __dp_gen_1(items)
"#;
        let output = rewrite_gen(input);
        assert_eq!(output.trim(), expected.trim());
    }

    #[test]
    fn defines_function_in_local_scope() {
        let input = r#"
def outer(items, offset):
    r = (a + offset for a in items)
    return r
"#;
        let expected = r#"
def outer(items, offset):

    def __dp_gen_1(items):
        for a in items:
            yield a + offset
    r = __dp_gen_1(items)
    return r
"#;
        let output = rewrite_gen(input);
        assert_eq!(output.trim(), expected.trim());
    }

    #[test]
    fn passes_iter_expression_as_argument() {
        let input = "
b = 1
r = (a + b for a in some_function())
";
        let expected = r#"
def __dp_gen_1(__dp_iter_1):
    for a in __dp_iter_1:
        yield a + b
b = 1
r = __dp_gen_1(some_function())
"#;
        let output = rewrite_gen(input);
        assert_eq!(output.trim(), expected.trim());
    }
}
