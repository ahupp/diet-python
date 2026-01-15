use crate::body_transform::{walk_stmt, Transformer};
use crate::{py_expr, py_stmt};
use crate::transform::driver::{ExprRewriter, Rewrite};
use ruff_python_ast::{self as ast, Stmt};
use ruff_text_size::TextRange;

struct ForElseBreakRewriter {
    flag: String,
    depth: usize,
}

impl ForElseBreakRewriter {
    fn new(flag: String) -> Self {
        Self { flag, depth: 0 }
    }
}

impl Transformer for ForElseBreakRewriter {
    fn visit_stmt(&mut self, stmt: &mut Stmt) {
        match stmt {
            Stmt::Break(_) if self.depth == 0 => {
                let assign_stmt = py_stmt!("{flag:id} = True", flag = self.flag.as_str());
                let assign_stmt = assign_stmt
                    .into_iter()
                    .next()
                    .expect("expected assignment statement");
                let break_stmt = Stmt::Break(ast::StmtBreak {
                    node_index: ast::AtomicNodeIndex::default(),
                    range: TextRange::default(),
                });
                *stmt = Stmt::If(ast::StmtIf {
                    node_index: ast::AtomicNodeIndex::default(),
                    range: TextRange::default(),
                    test: Box::new(py_expr!("True")),
                    body: vec![assign_stmt, break_stmt],
                    elif_else_clauses: Vec::new(),
                });
            }
            Stmt::For(ast::StmtFor { body, orelse, .. })
            | Stmt::While(ast::StmtWhile { body, orelse, .. }) => {
                self.depth += 1;
                self.visit_body(body);
                self.visit_body(orelse);
                self.depth -= 1;
            }
            Stmt::FunctionDef(_) | Stmt::ClassDef(_) => {}
            _ => walk_stmt(self, stmt),
        }
    }
}

pub fn rewrite_for(
    ast::StmtFor {
        target,
        iter,
        body,
        orelse,
        is_async,
        ..
    }: ast::StmtFor,
    rewriter: &mut ExprRewriter,
) -> Rewrite {
    let iter_name = rewriter.context().fresh("iter");

    let has_orelse = !orelse.is_empty();

    let mut body = body;
    let orelse = orelse;

    if has_orelse {
        let broke_name = rewriter.context().fresh("loop_broken");
        let mut break_rewriter = ForElseBreakRewriter::new(broke_name.as_str().to_string());
        break_rewriter.visit_body(&mut body);

        let mut rewritten = if is_async {
            let next_name = rewriter.context().fresh("iter_next");
            let iter_obj = rewriter.context().fresh("iter_obj");
            let iter_type = rewriter.context().fresh("iter_type");
            py_stmt!(
                r#"
{iter_obj:id} = {iter:expr}
if not hasattr({iter_obj:id}, "__aiter__"):
    {iter_type:id} = type({iter_obj:id}).__name__
    {iter_obj:id} = None
    raise TypeError("'async for' requires an object with __aiter__ method, got " + {iter_type:id})
{iter_name:id} = {iter_obj:id}.__aiter__()
if not hasattr({iter_name:id}, "__anext__"):
    {iter_type:id} = type({iter_name:id}).__name__
    {iter_name:id} = None
    raise TypeError("'async for' received an object from __aiter__ that does not implement __anext__: " + {iter_type:id})
{broke_name:id} = False
while True:
    try:
        {next_name:id} = await __dp__.anext({iter_name:id})
    except:
        __dp__.acheck_stopiteration()
        break
    else:
        {target:expr} = {next_name:id}
        {body:stmt}
if not {broke_name:id}:
    {orelse:stmt}
    "#,
                iter_obj = iter_obj.as_str(),
                iter_type = iter_type.as_str(),
                iter_name = iter_name.as_str(),
                iter = iter,
                next_name = next_name.as_str(),
                target = target,
                body = body,
                orelse = orelse,
                broke_name = broke_name.as_str(),
            )
        } else {
            py_stmt!(
                r#"
{iter_name:id} = __dp__.iter({iter:expr})
{broke_name:id} = False
while True:
    try:
        {target:expr} = __dp__.next({iter_name:id})
    except:
        __dp__.check_stopiteration()
        break
    else:
        {body:stmt}
if not {broke_name:id}:
    {orelse:stmt}
    "#,
                iter_name = iter_name.as_str(),
                iter = iter,
                target = target,
                body = body,
                orelse = orelse,
                broke_name = broke_name.as_str(),
            )
        };

        rewriter.visit_body(&mut rewritten);
        return Rewrite::Visit(rewritten);
    }

    let mut rewritten = if is_async {
        let next_name = rewriter.context().fresh("iter_next");
        let iter_obj = rewriter.context().fresh("iter_obj");
        let iter_type = rewriter.context().fresh("iter_type");
        py_stmt!(
            r#"
{iter_obj:id} = {iter:expr}
if not hasattr({iter_obj:id}, "__aiter__"):
    {iter_type:id} = type({iter_obj:id}).__name__
    {iter_obj:id} = None
    raise TypeError("'async for' requires an object with __aiter__ method, got " + {iter_type:id})
{iter_name:id} = {iter_obj:id}.__aiter__()
if not hasattr({iter_name:id}, "__anext__"):
    {iter_type:id} = type({iter_name:id}).__name__
    {iter_name:id} = None
    raise TypeError("'async for' received an object from __aiter__ that does not implement __anext__: " + {iter_type:id})
while True:
    try:
        {next_name:id} = await __dp__.anext({iter_name:id})
    except:
        __dp__.acheck_stopiteration()
        break
    else:
        {target:expr} = {next_name:id}
        {body:stmt}
    "#,
            iter_obj = iter_obj.as_str(),
            iter_type = iter_type.as_str(),
            iter_name = iter_name.as_str(),
            iter = iter,
            next_name = next_name.as_str(),
            target = target,
            body = body,
        )
    } else {
        py_stmt!(
            r#"
{iter_name:id} = __dp__.iter({iter:expr})
while True:
    try:
        {target:expr} = __dp__.next({iter_name:id})
    except:
        __dp__.check_stopiteration()
        break
    else:
        {body:stmt}
    "#,
            iter_name = iter_name.as_str(),
            iter = iter,
            target = target,
            body = body,
        )
    };

    rewriter.visit_body(&mut rewritten);

    Rewrite::Visit(rewritten)
}

pub fn rewrite_while(mut while_stmt: ast::StmtWhile, rewriter: &mut ExprRewriter) -> Rewrite {
    let guard = rewriter.expand_here(while_stmt.test.as_mut());

    if guard.is_empty() {
        // Unclear if / when this ever happens
        return Rewrite::Walk(vec![Stmt::While(while_stmt)]);
    }

    let ast::StmtWhile {
        test, body, orelse, ..
    } = while_stmt;

    Rewrite::Visit(py_stmt!(
        r#"
while True:
    {guard:stmt}
    if not {condition:expr}:
        {orelse:stmt}
        break
    {body:stmt}
"#,
        guard = guard,
        condition = *test,
        body = body,
        orelse = orelse,
    ))
}

#[cfg(test)]
mod tests {
    crate::transform_fixture_test!("tests_rewrite_loop.txt");
}
