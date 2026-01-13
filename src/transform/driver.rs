use super::{
    context::{Context, ScopeInfo, ScopeKind},
    rewrite_assign_del, rewrite_decorator,
    rewrite_expr_to_stmt::{expr_boolop_to_stmts, expr_compare_to_stmts},
    rewrite_func_expr, rewrite_import, rewrite_match_case, Options,
};
use crate::template::{is_simple, make_binop, make_generator, make_tuple, make_unaryop};
use crate::transform::simple::{
    rewrite_assert, rewrite_exception, rewrite_loop, rewrite_string, rewrite_with,
};
use crate::{
    body_transform::{walk_expr, walk_stmt, Transformer},
    transform::class_def,
};
use crate::{py_expr, py_stmt};
use ruff_python_ast::{self as ast, Expr, Identifier, Operator, Stmt, UnaryOp};
use ruff_python_codegen::{Generator, Indentation};
use ruff_source_file::LineEnding;
use ruff_text_size::TextRange;
use std::mem::take;

// TODO: rename RewriteContext, fold Context into it
pub struct ExprRewriter {
    ctx: Context,
    options: Options,
    buf: Vec<Stmt>,
    qualname_stack: Vec<(ScopeKind, String)>,
}

pub(crate) enum Rewrite {
    Walk(Vec<Stmt>),
    Visit(Vec<Stmt>),
}


impl ExprRewriter {
    pub fn new(ctx: Context) -> Self {
        Self {
            options: ctx.options,
            ctx,
            buf: Vec::new(),
            qualname_stack: Vec::new(),
        }
    }

    pub(super) fn context(&self) -> &Context {
        &self.ctx
    }

    fn generators_need_async(&self, generators: &[ast::Comprehension]) -> bool {
        generators.iter().any(|comp| {
            comp.is_async
                || expr_contains_await(&comp.iter)
                || comp.ifs.iter().any(expr_contains_await)
        })
    }

    fn comprehension_needs_async(&self, elt: &Expr, generators: &[ast::Comprehension]) -> bool {
        expr_contains_await(elt) || self.generators_need_async(generators)
    }

    fn wrap_comprehension_body(&self, body: Vec<Stmt>, comp: &ast::Comprehension) -> Vec<Stmt> {
        let mut inner = body;
        for if_expr in comp.ifs.iter().rev() {
            inner = py_stmt!(
                r#"
if {test:expr}:
    {body:stmt}
"#,
                test = if_expr.clone(),
                body = inner,
            );
        }

        if comp.is_async {
            py_stmt!(
                r#"
async for {target:expr} in {iter:expr}:
    {body:stmt}
"#,
                target = comp.target.clone(),
                iter = comp.iter.clone(),
                body = inner,
            )
        } else {
            py_stmt!(
                r#"
for {target:expr} in {iter:expr}:
    {body:stmt}
"#,
                target = comp.target.clone(),
                iter = comp.iter.clone(),
                body = inner,
            )
        }
    }

    fn rewrite_async_list_comp(&mut self, elt: Expr, generators: Vec<ast::Comprehension>) -> Expr {
        let tmp = self.ctx.fresh("tmp");
        let mut body = py_stmt!("{tmp:id}.append({elt:expr})", tmp = tmp.as_str(), elt = elt,);

        for comp in generators.iter().rev() {
            body = self.wrap_comprehension_body(body, comp);
        }

        self.buf
            .extend(py_stmt!("{tmp:id} = __dp__.list(())", tmp = tmp.as_str()));
        self.buf.extend(body);
        py_expr!("{tmp:id}", tmp = tmp.as_str())
    }

    fn rewrite_async_set_comp(&mut self, elt: Expr, generators: Vec<ast::Comprehension>) -> Expr {
        let tmp = self.ctx.fresh("tmp");
        let mut body = py_stmt!("{tmp:id}.add({elt:expr})", tmp = tmp.as_str(), elt = elt,);

        for comp in generators.iter().rev() {
            body = self.wrap_comprehension_body(body, comp);
        }

        self.buf
            .extend(py_stmt!("{tmp:id} = __dp__.set(())", tmp = tmp.as_str()));
        self.buf.extend(body);
        py_expr!("{tmp:id}", tmp = tmp.as_str())
    }

    fn rewrite_async_dict_comp(
        &mut self,
        key: Expr,
        value: Expr,
        generators: Vec<ast::Comprehension>,
    ) -> Expr {
        let tmp = self.ctx.fresh("tmp");
        let mut body = py_stmt!(
            "__dp__.setitem({tmp:id}, {key:expr}, {value:expr})",
            tmp = tmp.as_str(),
            key = key,
            value = value,
        );

        for comp in generators.iter().rev() {
            body = self.wrap_comprehension_body(body, comp);
        }

        self.buf
            .extend(py_stmt!("{tmp:id} = __dp__.dict(())", tmp = tmp.as_str()));
        self.buf.extend(body);
        py_expr!("{tmp:id}", tmp = tmp.as_str())
    }

    pub(crate) fn rewrite_block(&mut self, body: Vec<Stmt>) -> Vec<Stmt> {
        self.process_statements(body)
    }

    pub(crate) fn with_function_scope<F, R>(&mut self, scope: ScopeInfo, f: F) -> R
    where
        F: FnOnce(&mut Self) -> R,
    {
        self.ctx.push_scope(scope);
        let result = f(self);
        self.ctx.pop_scope();
        result
    }


    fn scope_expr_for_child(&self) -> Expr {
        match self.context().current_qualname() {
            Some((mut scope, kind)) => {
                if kind == ScopeKind::Function {
                    scope.push_str(".<locals>");
                }
                py_expr!("{scope:literal}", scope = scope.as_str())
            }
            None => py_expr!("None"),
        }
    }

    fn decorator_placeholders(
        &mut self,
        decorators: Vec<ast::Decorator>,
    ) -> (Vec<Stmt>, Vec<Expr>) {
        let mut prelude = Vec::new();
        let mut exprs = Vec::with_capacity(decorators.len());
        for decorator in decorators {
            let expr = decorator.expression;
            if is_simple(&expr) {
                exprs.push(expr);
            } else {
                let tmp = self.ctx.fresh("tmp");
                prelude.extend(py_stmt!(
                    "{tmp:id} = {value:expr}",
                    tmp = tmp.as_str(),
                    value = expr
                ));
                exprs.push(py_expr!("{tmp:id}", tmp = tmp.as_str()));
            }
        }
        (prelude, exprs)
    }

    fn rewrite_function_def_in_class(&mut self, mut func_def: ast::StmtFunctionDef) -> Vec<Stmt> {
        let original_name = func_def.name.id.to_string();
        if original_name.starts_with("_dp_") {
            return vec![Stmt::FunctionDef(func_def)];
        }

        let renamed = format!("_dp_fn_{original_name}");
        func_def.name = Identifier::new(renamed.as_str(), TextRange::default());

        let decorators = take(&mut func_def.decorator_list);
        let (mut prelude, decorator_exprs) = self.decorator_placeholders(decorators);

        let scope_expr = self.scope_expr_for_child();
        let mut decorated = py_expr!(
            "__dp__.update_fn({name:id}, {scope:expr}, {orig:literal})",
            name = renamed.as_str(),
            scope = scope_expr,
            orig = original_name.as_str(),
        );
        for decorator in decorator_exprs.into_iter().rev() {
            decorated = py_expr!(
                "{decorator:expr}({decorated:expr})",
                decorator = decorator,
                decorated = decorated
            );
        }
        prelude.push(Stmt::FunctionDef(func_def));
        prelude.extend(py_stmt!(
            "{name:id} = {decorated:expr}",
            name = original_name.as_str(),
            decorated = decorated
        ));
        prelude
    }

    fn rewrite_class_body_function_defs(&mut self, body: &mut Vec<Stmt>) {
        struct Rewriter<'a> {
            driver: &'a mut ExprRewriter,
        }

        impl Transformer for Rewriter<'_> {
            fn visit_body(&mut self, body: &mut Vec<Stmt>) {
                let mut rewritten = Vec::with_capacity(body.len());
                for stmt in take(body) {
                    match stmt {
                        Stmt::FunctionDef(func_def) => {
                            rewritten.extend(self.driver.rewrite_function_def_in_class(func_def));
                        }
                        Stmt::ClassDef(mut class_def) => {
                            let name = class_def.name.id.to_string();
                            self.driver
                                .qualname_stack
                                .push((ScopeKind::Class, name));
                            self.visit_body(&mut class_def.body);
                            self.driver.qualname_stack.pop();
                            rewritten.push(Stmt::ClassDef(class_def));
                        }
                        mut other => {
                            self.visit_stmt(&mut other);
                            rewritten.push(other);
                        }
                    }
                }
                *body = rewritten;
            }

            fn visit_stmt(&mut self, stmt: &mut Stmt) {
                match stmt {
                    Stmt::FunctionDef(_) | Stmt::ClassDef(_) => {}
                    _ => walk_stmt(self, stmt),
                }
            }
        }

        let mut rewriter = Rewriter { driver: self };
        rewriter.visit_body(body);
    }

    fn process_statements(&mut self, initial: Vec<Stmt>) -> Vec<Stmt> {
        enum WorkItem {
            Process(Stmt),
            Walk(Stmt),
            Emit(Stmt),
        }

        let mut worklist: Vec<WorkItem> =
            initial.into_iter().rev().map(WorkItem::Process).collect();

        let mut buf_stack = take(&mut self.buf);
        let mut output = Vec::new();

        while let Some(item) = worklist.pop() {
            match item {
                WorkItem::Process(stmt) => match self.rewrite_stmt(stmt) {
                    Rewrite::Visit(stmts) => {
                        for stmt in stmts.into_iter().rev() {
                            worklist.push(WorkItem::Process(stmt));
                        }
                    }
                    Rewrite::Walk(stmts) => {
                        for stmt in stmts.into_iter().rev() {
                            worklist.push(WorkItem::Walk(stmt));
                        }
                    }
                },
                WorkItem::Walk(mut stmt) => {
                    walk_stmt(self, &mut stmt);
                    let mut buffered = take(&mut self.buf);
                    worklist.push(WorkItem::Emit(stmt));
                    while let Some(buffered_stmt) = buffered.pop() {
                        worklist.push(WorkItem::Process(buffered_stmt));
                    }
                }
                WorkItem::Emit(stmt) => output.push(stmt),
            }
        }

        self.buf = take(&mut buf_stack);

        output
    }

    /// Expand the buffered statements for an expression directly in-place within a block,
    /// instead of emitting them before the block executes.
    pub(super) fn expand_here(&mut self, expr: &mut Expr) -> Vec<Stmt> {
        let saved = take(&mut self.buf);
        self.visit_expr(expr);
        let produced = take(&mut self.buf);
        self.buf = saved;
        produced
    }

    pub(super) fn maybe_placeholder(&mut self, mut expr: Expr) -> Expr {
        fn is_temp_skippable(expr: &Expr) -> bool {
            is_simple(expr) && !matches!(expr, Expr::StringLiteral(_) | Expr::BytesLiteral(_))
        }

        if is_temp_skippable(&expr) {
            return expr;
        }

        self.visit_expr(&mut expr);

        if is_temp_skippable(&expr) {
            return expr;
        }

        let tmp = self.ctx.fresh("tmp");
        let placeholder_expr = py_expr!("{tmp:id}", tmp = tmp.as_str());
        let assign = py_stmt!("{tmp:id} = {value:expr}", tmp = tmp.as_str(), value = expr);
        self.buf.extend(assign);
        placeholder_expr
    }

    fn rewrite_function_def(&mut self, mut func_def: ast::StmtFunctionDef) -> Rewrite {
        let func_name = func_def.name.id.to_string();
        let mut scope = self.context().analyze_function_scope(&func_def);
        let should_rewrite = !func_name.starts_with("_dp_");
        let original_name = func_name.clone();
        let renamed = if should_rewrite {
            format!("_dp_fn_{func_name}")
        } else {
            func_name.clone()
        };

        if should_rewrite {
            func_def.name = Identifier::new(renamed.as_str(), TextRange::default());
        }

        if !should_rewrite {
            if let Some(stripped) = func_name.strip_prefix("_dp_fn_") {
                if let Some((prefix, _)) = scope.qualname.rsplit_once('.') {
                    scope.qualname = format!("{prefix}.{stripped}");
                } else {
                    scope.qualname = stripped.to_string();
                }
            }
        }

        let scope_expr = self.scope_expr_for_child();
        let qualname_entry = if let Some(stripped) = func_name.strip_prefix("_dp_fn_") {
            stripped.to_string()
        } else {
            original_name.clone()
        };
        self.qualname_stack
            .push((ScopeKind::Function, qualname_entry));
        func_def.body = self.with_function_scope(scope, |rewriter| {
            rewriter.rewrite_block(take(&mut func_def.body))
        });
        self.qualname_stack.pop();

        let decorators = take(&mut func_def.decorator_list);
        if !should_rewrite {
            return rewrite_decorator::rewrite(decorators, func_name.as_str(), vec![Stmt::FunctionDef(func_def)], self);
        }

        let (mut prelude, decorator_exprs) = self.decorator_placeholders(decorators);
        let mut decorated = py_expr!(
            "__dp__.update_fn({name:id}, {scope:expr}, {orig:literal})",
            name = renamed.as_str(),
            scope = scope_expr,
            orig = original_name.as_str(),
        );
        for decorator in decorator_exprs.into_iter().rev() {
            decorated = py_expr!(
                "{decorator:expr}({decorated:expr})",
                decorator = decorator,
                decorated = decorated
            );
        }
        prelude.push(Stmt::FunctionDef(func_def));
        prelude.extend(py_stmt!(
            "{name:id} = {decorated:expr}",
            name = original_name.as_str(),
            decorated = decorated
        ));
        prelude.extend(py_stmt!("del {name:id}", name = renamed.as_str()));
        Rewrite::Visit(prelude)
    }

    fn rewrite_stmt(&mut self, stmt: Stmt) -> Rewrite {
        match stmt {
            Stmt::FunctionDef(func_def) => self.rewrite_function_def(func_def),
            Stmt::With(with) => rewrite_with::rewrite(with, self),
            Stmt::While(while_stmt) => rewrite_loop::rewrite_while(while_stmt, self),
            Stmt::For(for_stmt) => rewrite_loop::rewrite_for(for_stmt, self),
            Stmt::Assert(assert) => rewrite_assert::rewrite(assert),
            Stmt::ClassDef(mut class_def) => {
                let name = class_def.name.id.to_string();
                self.qualname_stack.push((ScopeKind::Class, name));
                self.rewrite_class_body_function_defs(&mut class_def.body);
                self.qualname_stack.pop();
                class_def::rewrite(class_def, self)
            }
            Stmt::Try(try_stmt) => rewrite_exception::rewrite_try(try_stmt, &self.ctx),
            Stmt::If(if_stmt)
                if if_stmt
                    .elif_else_clauses
                    .iter()
                    .any(|clause| clause.test.is_some()) =>
            {
                Rewrite::Visit(vec![expand_if_chain(if_stmt).into()])
            }
            Stmt::Match(match_stmt) => rewrite_match_case::rewrite(match_stmt, &self.ctx),
            Stmt::Import(import) => rewrite_import::rewrite(import),
            Stmt::ImportFrom(import_from) => {
                rewrite_import::rewrite_from(import_from.clone(), &self.ctx, &self.options)
            }

            Stmt::AnnAssign(ann_assign) => rewrite_assign_del::rewrite_ann_assign(self, ann_assign),
            Stmt::Assign(assign) => rewrite_assign_del::rewrite_assign(self, assign),
            Stmt::AugAssign(aug) => rewrite_assign_del::rewrite_aug_assign(self, aug),
            Stmt::Delete(del) => rewrite_assign_del::rewrite_delete(self, del),
            Stmt::Raise(raise) => rewrite_exception::rewrite_raise(raise),
            other => Rewrite::Walk(vec![other]),
        }
    }
}

fn make_tuple_splat(elts: Vec<Expr>) -> Expr {
    let mut segments: Vec<Expr> = Vec::new();
    let mut values: Vec<Expr> = Vec::new();

    for elt in elts {
        match elt {
            Expr::Starred(ast::ExprStarred { value, .. }) => {
                if !values.is_empty() {
                    segments.push(make_tuple(std::mem::take(&mut values)));
                }
                segments.push(py_expr!("__dp__.tuple({value:expr})", value = *value));
            }
            other => values.push(other),
        }
    }

    if !values.is_empty() {
        segments.push(make_tuple(values));
    }

    segments
        .into_iter()
        .reduce(|left, right| py_expr!("{left:expr} + {right:expr}", left = left, right = right))
        .unwrap_or_else(|| make_tuple(Vec::new()))
}

fn expr_contains_await(expr: &Expr) -> bool {
    struct AwaitFinder {
        found: bool,
    }

    impl Transformer for AwaitFinder {
        fn visit_expr(&mut self, expr: &mut Expr) {
            let has_async_generator = match expr {
                Expr::ListComp(ast::ExprListComp { generators, .. })
                | Expr::SetComp(ast::ExprSetComp { generators, .. })
                | Expr::DictComp(ast::ExprDictComp { generators, .. }) => {
                    generators.iter().any(|comp| comp.is_async)
                }
                _ => false,
            };

            if has_async_generator {
                self.found = true;
                return;
            }
            if matches!(expr, Expr::Await(_)) {
                self.found = true;
                return;
            }
            if self.found {
                return;
            }
            walk_expr(self, expr);
        }
    }

    let mut finder = AwaitFinder { found: false };
    let mut expr = expr.clone();
    finder.visit_expr(&mut expr);
    finder.found
}


fn expand_if_chain(mut if_stmt: ast::StmtIf) -> ast::StmtIf {
    let mut else_body: Option<Vec<Stmt>> = None;

    for clause in if_stmt.elif_else_clauses.into_iter().rev() {
        match clause.test {
            Some(test) => {
                let mut nested_if = ast::StmtIf {
                    node_index: ast::AtomicNodeIndex::default(),
                    range: TextRange::default(),
                    test: Box::new(test),
                    body: clause.body,
                    elif_else_clauses: Vec::new(),
                };

                if let Some(body) = else_body.take() {
                    nested_if.elif_else_clauses.push(ast::ElifElseClause {
                        test: None,
                        body,
                        range: TextRange::default(),
                        node_index: ast::AtomicNodeIndex::default(),
                    });
                }

                else_body = Some(vec![Stmt::If(nested_if)]);
            }
            None => {
                else_body = Some(clause.body);
            }
        }
    }

    if let Some(body) = else_body {
        if_stmt.elif_else_clauses = vec![ast::ElifElseClause {
            range: TextRange::default(),
            node_index: ast::AtomicNodeIndex::default(),
            test: None,
            body,
        }];
    } else {
        if_stmt.elif_else_clauses = Vec::new();
    }

    if_stmt
}

impl Transformer for ExprRewriter {
    fn visit_body(&mut self, body: &mut Vec<Stmt>) {
        let stmts = take(body);
        let output = self.process_statements(stmts);
        *body = output;
    }

    fn visit_expr(&mut self, expr: &mut Expr) {
        if let Expr::YieldFrom(yield_from) = expr.clone() {
            let ast::ExprYieldFrom {
                value,
                range,
                node_index,
            } = yield_from;
            let mut value = *value;
            self.visit_expr(&mut value);
            *expr = Expr::YieldFrom(ast::ExprYieldFrom {
                value: Box::new(value),
                range,
                node_index,
            });
            return;
        }
        if let Expr::NumberLiteral(ast::ExprNumberLiteral {
            value: ast::Number::Float(_value),
            range,
            ..
        }) = expr
        {
            let range = *range;
            if let Some(src) = self.ctx.source_slice(range) {
                let src = src.trim();
                let normalized = src.replace('_', "");
                let indent = Indentation::new("    ".to_string());
                let default = Generator::new(&indent, LineEnding::default()).expr(expr);
                if normalized.len() >= 10 && normalized != default {
                    *expr = py_expr!(
                        "__dp__.float_from_literal({literal:literal})",
                        literal = src
                    );
                    return;
                }
            }
        }
        let rewritten = match expr.clone() {
            Expr::Named(named_expr) => {
                let ast::ExprNamed { target, value, .. } = named_expr;
                let target = *target;
                let value = *value;
                let value_expr = self.maybe_placeholder(value);
                let tmp = self.ctx.fresh("tmp");
                let tmp_expr = py_expr!("{tmp:id}", tmp = tmp.as_str());
                self.buf.extend(py_stmt!(
                    "{tmp:id} = {value:expr}",
                    tmp = tmp.as_str(),
                    value = value_expr,
                ));
                self.buf.extend(py_stmt!(
                    "{target:expr} = {tmp:expr}",
                    target = target,
                    tmp = tmp_expr.clone(),
                ));
                tmp_expr
            }
            Expr::If(if_expr) => {
                let tmp = self.ctx.fresh("tmp");
                let ast::ExprIf {
                    test, body, orelse, ..
                } = if_expr;
                let assign = py_stmt!(
                    r#"
if {cond:expr}:
    {tmp:id} = {body:expr}
else:
    {tmp:id} = {orelse:expr}
"#,
                    cond = *test,
                    tmp = tmp.as_str(),
                    body = *body,
                    orelse = *orelse,
                );
                self.buf.extend(assign);
                py_expr!("{tmp:id}", tmp = tmp.as_str())
            }
            Expr::BoolOp(bool_op) => {
                let tmp = self.ctx.fresh("tmp");
                let stmts = expr_boolop_to_stmts(tmp.as_str(), bool_op);
                self.buf.extend(stmts);
                py_expr!("{tmp:id}", tmp = tmp.as_str())
            }
            Expr::Compare(compare) => {
                let tmp = self.ctx.fresh("tmp");
                let stmts = expr_compare_to_stmts(&self.ctx, tmp.as_str(), compare);
                self.buf.extend(stmts);
                py_expr!("{tmp:id}", tmp = tmp.as_str())
            }
            Expr::Lambda(lambda) => {
                rewrite_func_expr::rewrite_lambda(lambda, &self.ctx, &mut self.buf)
            }
            Expr::Generator(generator) => {
                let needs_async =
                    self.comprehension_needs_async(&generator.elt, &generator.generators);
                rewrite_func_expr::rewrite_generator(
                    generator,
                    &self.ctx,
                    needs_async,
                    &mut self.buf,
                )
            }
            Expr::FString(f_string) => rewrite_string::rewrite_fstring(f_string, &self.ctx),
            Expr::TString(t_string) => rewrite_string::rewrite_tstring(t_string, &self.ctx),
            Expr::Slice(ast::ExprSlice {
                lower, upper, step, ..
            }) => {
                fn none_name() -> Expr {
                    py_expr!("None")
                }
                let lower_expr = lower.map(|expr| *expr).unwrap_or_else(none_name);
                let upper_expr = upper.map(|expr| *expr).unwrap_or_else(none_name);
                let step_expr = step.map(|expr| *expr).unwrap_or_else(none_name);
                py_expr!(
                    "__dp__.slice({lower:expr}, {upper:expr}, {step:expr})",
                    lower = lower_expr,
                    upper = upper_expr,
                    step = step_expr,
                )
            }
            Expr::NumberLiteral(ast::ExprNumberLiteral {
                value: ast::Number::Complex { real, imag },
                ..
            }) => {
                let real_expr = Expr::NumberLiteral(ast::ExprNumberLiteral {
                    node_index: ast::AtomicNodeIndex::default(),
                    range: TextRange::default(),
                    value: ast::Number::Float(real),
                });
                let imag_expr = Expr::NumberLiteral(ast::ExprNumberLiteral {
                    node_index: ast::AtomicNodeIndex::default(),
                    range: TextRange::default(),
                    value: ast::Number::Float(imag),
                });
                py_expr!(
                    "complex({real:expr}, {imag:expr})",
                    real = real_expr,
                    imag = imag_expr,
                )
            }
            Expr::Attribute(ast::ExprAttribute {
                value, attr, ctx, ..
            }) if matches!(ctx, ast::ExprContext::Load) && self.options.lower_attributes => {
                let value_expr = *value;
                py_expr!(
                    "getattr({value:expr}, {attr:literal})",
                    value = value_expr,
                    attr = attr.id.as_str(),
                )
            }
            Expr::ListComp(ast::ExprListComp {
                elt, generators, ..
            }) => {
                if self.comprehension_needs_async(&elt, &generators) {
                    self.rewrite_async_list_comp(*elt, generators)
                } else {
                    py_expr!(
                        "__dp__.list({expr:expr})",
                        expr = make_generator(*elt, generators)
                    )
                }
            }
            Expr::SetComp(ast::ExprSetComp {
                elt, generators, ..
            }) => {
                if self.comprehension_needs_async(&elt, &generators) {
                    self.rewrite_async_set_comp(*elt, generators)
                } else {
                    py_expr!(
                        "__dp__.set({expr:expr})",
                        expr = make_generator(*elt, generators)
                    )
                }
            }
            Expr::DictComp(ast::ExprDictComp {
                key,
                value,
                generators,
                ..
            }) => {
                if expr_contains_await(&key)
                    || expr_contains_await(&value)
                    || self.generators_need_async(&generators)
                {
                    self.rewrite_async_dict_comp(*key, *value, generators)
                } else {
                    let tuple = py_expr!("({key:expr}, {value:expr})", key = *key, value = *value,);
                    py_expr!(
                        "__dp__.dict({expr:expr})",
                        expr = make_generator(tuple, generators)
                    )
                }
            }

            // tuple/list/dict unpacking
            Expr::Tuple(tuple)
                if matches!(tuple.ctx, ast::ExprContext::Load)
                    && tuple.elts.iter().any(|elt| matches!(elt, Expr::Starred(_))) =>
            {
                make_tuple_splat(tuple.elts)
            }
            Expr::List(list) if matches!(list.ctx, ast::ExprContext::Load) => {
                let tuple = make_tuple_splat(list.elts);
                py_expr!("__dp__.list({tuple:expr})", tuple = tuple,)
            }
            Expr::Set(ast::ExprSet { elts, .. }) => {
                let tuple = make_tuple(elts);
                py_expr!("__dp__.set({tuple:expr})", tuple = tuple,)
            }
            Expr::Dict(ast::ExprDict { items, .. }) => {
                let mut segments: Vec<Expr> = Vec::new();

                let mut keyed_pairs = Vec::new();
                for item in items.into_iter() {
                    match item {
                        ast::DictItem {
                            key: Some(key),
                            value,
                        } => {
                            keyed_pairs.push(py_expr!(
                                "({key:expr}, {value:expr})",
                                key = key,
                                value = value,
                            ));
                        }
                        ast::DictItem { key: None, value } => {
                            if !keyed_pairs.is_empty() {
                                let tuple = make_tuple(take(&mut keyed_pairs));
                                segments.push(py_expr!("__dp__.dict({tuple:expr})", tuple = tuple));
                            }
                            segments.push(py_expr!("__dp__.dict({mapping:expr})", mapping = value));
                        }
                    }
                }

                if !keyed_pairs.is_empty() {
                    let tuple = make_tuple(take(&mut keyed_pairs));
                    segments.push(py_expr!("__dp__.dict({tuple:expr})", tuple = tuple));
                }

                match segments.len() {
                    0 => {
                        py_expr!("__dp__.dict()")
                    }
                    _ => segments
                        .into_iter()
                        .reduce(|left, right| {
                            py_expr!("{left:expr} | {right:expr}", left = left, right = right)
                        })
                        .expect("segments is non-empty"),
                }
            }
            Expr::BinOp(ast::ExprBinOp {
                left, right, op, ..
            }) => {
                let func_name = match op {
                    Operator::Add => "add",
                    Operator::Sub => "sub",
                    Operator::Mult => "mul",
                    Operator::MatMult => "matmul",
                    Operator::Div => "truediv",
                    Operator::Mod => "mod",
                    Operator::Pow => "pow",
                    Operator::LShift => "lshift",
                    Operator::RShift => "rshift",
                    Operator::BitOr => "or_",
                    Operator::BitXor => "xor",
                    Operator::BitAnd => "and_",
                    Operator::FloorDiv => "floordiv",
                };
                make_binop(func_name, *left, *right)
            }
            Expr::UnaryOp(ast::ExprUnaryOp { operand, op, .. }) => {
                let func_name = match op {
                    UnaryOp::Not => "not_",
                    UnaryOp::Invert => "invert",
                    UnaryOp::USub => "neg",
                    UnaryOp::UAdd => "pos",
                };
                make_unaryop(func_name, *operand)
            }
            Expr::Subscript(ast::ExprSubscript {
                value, slice, ctx, ..
            }) if matches!(ctx, ast::ExprContext::Load) => make_binop("getitem", *value, *slice),
            _ => {
                walk_expr(self, expr);
                return;
            }
        };
        *expr = rewritten;
        self.visit_expr(expr);
    }

    fn visit_stmt(&mut self, stmt: &mut Stmt) {
        let rewritten = self.process_statements(vec![stmt.clone()]);
        *stmt = match rewritten.len() {
            0 => py_stmt!("pass")[0].clone(),
            _ => rewritten[0].clone(),
        };
    }
}

#[cfg(test)]
mod tests {
    crate::transform_fixture_test!("tests_expr.txt");
}
