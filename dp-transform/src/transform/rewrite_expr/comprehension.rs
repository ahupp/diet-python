use std::collections::{HashMap, HashSet};

use ruff_python_ast::{self as ast};
use ruff_python_ast::{Expr, ExprContext, Stmt};

use crate::transform::ast_rewrite::LoweredExpr;
use crate::transform::scope::{Scope, ScopeKind};
use crate::{py_expr, py_stmt};
use crate::transform::context::{Context};
use crate::body_transform::{walk_expr, walk_stmt, Transformer};
use ruff_python_ast::name::Name;



fn rewrite_named_expr_targets(
    func_def: &mut ast::StmtFunctionDef,
    global_targets: &[String],
    nonlocal_targets: &[String],
) {
    if global_targets.is_empty() && nonlocal_targets.is_empty() {
        return;
    }
    if !global_targets.is_empty() {
        let insert_at = match func_def.body.first() {
            Some(Stmt::Expr(ast::StmtExpr { value, .. }))
                if matches!(value.as_ref(), Expr::StringLiteral(_)) => 1,
            _ => 0,
        };
        func_def
            .body
            .splice(insert_at..insert_at, py_stmt!("__globals__ = globals()"));
    }
    let mut rewriter = NamedExprTargetRewriter::new(global_targets, nonlocal_targets);
    rewriter.visit_body(&mut func_def.body);
}

struct NamedExprTargetRewriter {
    globals: HashSet<String>,
    cells: HashSet<String>,
}

impl NamedExprTargetRewriter {
    fn new(global_targets: &[String], nonlocal_targets: &[String]) -> Self {
        Self {
            globals: global_targets.iter().cloned().collect(),
            cells: nonlocal_targets.iter().cloned().collect(),
        }
    }
}

impl Transformer for NamedExprTargetRewriter {
    fn visit_stmt(&mut self, stmt: &mut Stmt) {
        if let Stmt::Assign(ast::StmtAssign { targets, value, .. }) = stmt {
            if targets.len() == 1 {
                if let Expr::Name(ast::ExprName { id, .. }) = &targets[0] {
                    if self.globals.contains(id.as_str()) {
                        self.visit_expr(value.as_mut());
                        *stmt = py_stmt!(
                            "__dp__.store_global(globals(), {name:literal}, {value:expr})",
                            name = id.as_str(),
                            value = value.clone()
                        )
                        .remove(0);
                        return;
                    }
                    if self.cells.contains(id.as_str()) {
                        self.visit_expr(value.as_mut());
                        *stmt = py_stmt!(
                            "__dp__.store_cell({cell:id}, {value:expr})",
                            cell = id.as_str(),
                            value = value.clone()
                        )
                        .remove(0);
                        return;
                    }
                }
            }
        }
        walk_stmt(self, stmt);
    }

    fn visit_expr(&mut self, expr: &mut Expr) {
        match expr {
            Expr::Call(ast::ExprCall { func, .. }) => {
                if is_dp_attr(func.as_ref(), "load_cell")
                    || is_dp_attr(func.as_ref(), "load_global")
                    || is_dp_attr(func.as_ref(), "store_cell")
                    || is_dp_attr(func.as_ref(), "store_global")
                {
                    return;
                }
            }
            Expr::Named(ast::ExprNamed { target, value, .. }) => {
                if let Expr::Name(ast::ExprName { id, .. }) = target.as_ref() {
                    if self.globals.contains(id.as_str()) {
                        self.visit_expr(value);
                        *expr = py_expr!(
                            "__dp__.store_global(globals(), {name:literal}, {value:expr})",
                            name = id.as_str(),
                            value = *value.clone()
                        );
                        return;
                    }
                    if self.cells.contains(id.as_str()) {
                        self.visit_expr(value);
                        *expr = py_expr!(
                            "__dp__.store_cell({cell:id}, {value:expr})",
                            cell = id.as_str(),
                            value = *value.clone()
                        );
                        return;
                    }
                }
            }
            _ => {}
        }
        if let Expr::Name(ast::ExprName { id, ctx, .. }) = expr {
            if matches!(ctx, ast::ExprContext::Load) {
                if self.globals.contains(id.as_str()) {
                    *expr = py_expr!(
                        "__dp__.load_global(globals(), {name:literal})",
                        name = id.as_str()
                    );
                    return;
                }
                if self.cells.contains(id.as_str()) {
                    *expr = py_expr!("__dp__.load_cell({name:id})", name = id.as_str());
                    return;
                }
            }
        }
        walk_expr(self, expr);
    }
}

fn is_dp_attr(expr: &Expr, attr: &str) -> bool {
    matches!(
        expr,
        Expr::Attribute(ast::ExprAttribute { value, attr: name, .. })
            if matches!(value.as_ref(), Expr::Name(ast::ExprName { id, .. }) if id.as_str() == "__dp__")
                && name.as_str() == attr
    )
}

#[derive(Clone, Copy)]
pub(crate) enum InlineCompKind {
    List,
    Set,
    Dict,
}

struct LoadRenamer<'a> {
    renames: &'a HashMap<String, Name>,
}

impl Transformer for LoadRenamer<'_> {
    fn visit_expr(&mut self, expr: &mut Expr) {
        if let Expr::Name(ast::ExprName { id, ctx, .. }) = expr {
            if matches!(ctx, ExprContext::Load) {
                if let Some(new) = self.renames.get(id.as_str()) {
                    *id = new.clone();
                }
                return;
            }
        }
        walk_expr(self, expr);
    }
}

struct TargetRenamer<'a> {
    context: &'a Context,
    renames: &'a mut HashMap<String, Name>,
    bound_here: HashSet<String>,
}

impl<'a> TargetRenamer<'a> {
    fn new(context: &'a Context, renames: &'a mut HashMap<String, Name>) -> Self {
        Self {
            context,
            renames,
            bound_here: HashSet::new(),
        }
    }
}

impl Transformer for TargetRenamer<'_> {
    fn visit_expr(&mut self, expr: &mut Expr) {
        match expr {
            Expr::Name(ast::ExprName { id, ctx, .. }) => {
                if matches!(ctx, ExprContext::Store) {
                    let key = id.as_str().to_string();
                    if self.bound_here.contains(&key) {
                        if let Some(existing) = self.renames.get(&key).cloned() {
                            *id = existing;
                        }
                    } else {
                        let fresh = Name::new(self.context.fresh("tmp"));
                        self.renames.insert(key.clone(), fresh.clone());
                        self.bound_here.insert(key);
                        *id = fresh;
                    }
                    return;
                }
                if matches!(ctx, ExprContext::Load) {
                    if let Some(new) = self.renames.get(id.as_str()) {
                        *id = new.clone();
                    }
                    return;
                }
            }
            Expr::Generator(_)
            | Expr::ListComp(_)
            | Expr::SetComp(_)
            | Expr::DictComp(_) => {
                let mut renamer = LoadRenamer { renames: self.renames };
                renamer.visit_expr(expr);
                return;
            }
            _ => {}
        }
        walk_expr(self, expr);
    }
}

fn rename_loads(mut expr: Expr, renames: &HashMap<String, Name>) -> Expr {
    let mut renamer = LoadRenamer { renames };
    renamer.visit_expr(&mut expr);
    expr
}

struct LoweredGenerator {
    target: Expr,
    iter: LoweredExpr,
    ifs: Vec<LoweredExpr>,
    is_async: bool,
}

fn wrap_ifs(mut body: Vec<Stmt>, ifs: Vec<LoweredExpr>) -> Vec<Stmt> {
    for lowered in ifs.into_iter().rev() {
        let mut block = Vec::new();
        block.extend(lowered.stmts);
        block.push(
            py_stmt!(
                r#"
if {test:expr}:
    {body:stmt}
"#,
                test = lowered.expr,
                body = body,
            )
            .remove(0),
        );
        body = block;
    }
    body
}

pub(crate) fn lower_inline_list_comp(
    context: &Context,
    elt: Expr,
    generators: Vec<ast::Comprehension>,
) -> LoweredExpr {
    lower_inline(context, InlineCompKind::List, elt, None, generators)
}

pub(crate) fn lower_inline_set_comp(
    context: &Context,
    elt: Expr,
    generators: Vec<ast::Comprehension>,
) -> LoweredExpr {
    lower_inline(context, InlineCompKind::Set, elt, None, generators)
}

pub(crate) fn lower_inline_dict_comp(
    context: &Context,
    key: Expr,
    value: Expr,
    generators: Vec<ast::Comprehension>,
) -> LoweredExpr {
    lower_inline(context, InlineCompKind::Dict, key, Some(value), generators)
}

fn lower_inline(
    context: &Context,
    kind: InlineCompKind,
    elt_or_key: Expr,
    value: Option<Expr>,
    generators: Vec<ast::Comprehension>,
) -> LoweredExpr {
    let result_name = context.fresh("tmp");
    let result_expr = py_expr!("{name:id}", name = result_name.as_str());
    let mut stmts = match kind {
        InlineCompKind::List => py_stmt!("{name:id} = []", name = result_name.as_str()),
        InlineCompKind::Set => py_stmt!("{name:id} = set()", name = result_name.as_str()),
        InlineCompKind::Dict => py_stmt!("{name:id} = {}", name = result_name.as_str()),
    };

    let mut renames: HashMap<String, Name> = HashMap::new();
    let mut lowered_gens = Vec::with_capacity(generators.len());

    for gen in generators {
        let iter_expr = rename_loads(gen.iter, &renames);
        let iter_lowered = super::lower_expr(context, iter_expr);

        let mut target_expr = gen.target;
        let mut target_renamer = TargetRenamer::new(context, &mut renames);
        target_renamer.visit_expr(&mut target_expr);

        let mut ifs = Vec::with_capacity(gen.ifs.len());
        for if_expr in gen.ifs {
            let if_expr = rename_loads(if_expr, &renames);
            ifs.push(super::lower_expr(context, if_expr));
        }

        lowered_gens.push(LoweredGenerator {
            target: target_expr,
            iter: iter_lowered,
            ifs,
            is_async: gen.is_async,
        });
    }

    let mut body = match kind {
        InlineCompKind::Dict => {
            let key_expr = rename_loads(elt_or_key, &renames);
            let value_expr = rename_loads(
                value.expect("dict comprehension expects value"),
                &renames,
            );
            let lowered_key = super::lower_expr(context, key_expr);
            let lowered_value = super::lower_expr(context, value_expr);
            let mut body = lowered_key.stmts;
            body.extend(lowered_value.stmts);
            body.extend(py_stmt!(
                "__dp__.setitem({result:id}, {key:expr}, {value:expr})",
                result = result_name.as_str(),
                key = lowered_key.expr,
                value = lowered_value.expr,
            ));
            body
        }
        InlineCompKind::List => {
            let elt_expr = rename_loads(elt_or_key, &renames);
            let lowered_elt = super::lower_expr(context, elt_expr);
            let mut body = lowered_elt.stmts;
            body.extend(py_stmt!(
                "{result:id}.append({value:expr})",
                result = result_name.as_str(),
                value = lowered_elt.expr,
            ));
            body
        }
        InlineCompKind::Set => {
            let elt_expr = rename_loads(elt_or_key, &renames);
            let lowered_elt = super::lower_expr(context, elt_expr);
            let mut body = lowered_elt.stmts;
            body.extend(py_stmt!(
                "{result:id}.add({value:expr})",
                result = result_name.as_str(),
                value = lowered_elt.expr,
            ));
            body
        }
    };

    for gen in lowered_gens.into_iter().rev() {
        body = wrap_ifs(body, gen.ifs);
        let for_stmt = if gen.is_async {
            py_stmt!(
                r#"
async for {target:expr} in {iter:expr}:
    {body:stmt}
"#,
                target = gen.target,
                iter = gen.iter.expr,
                body = body,
            )
            .remove(0)
        } else {
            py_stmt!(
                r#"
for {target:expr} in {iter:expr}:
    {body:stmt}
"#,
                target = gen.target,
                iter = gen.iter.expr,
                body = body,
            )
            .remove(0)
        };
        let mut new_body = gen.iter.stmts;
        new_body.push(for_stmt);
        body = new_body;
    }

    stmts.extend(body);
    LoweredExpr::modified(result_expr, stmts)
}

pub(crate) fn rewrite_lambda(lambda: ast::ExprLambda, ctx: &Context, scope: &Scope) -> LoweredExpr {
    let func_name = ctx.fresh("lambda");

    let ast::ExprLambda {
        parameters, body, ..
    } = lambda;

    let updated_parameters = parameters
        .map(|params| *params)
        .unwrap_or_default();

    let mut buf = Vec::new();

    // TODO: qualname
    let mut func_def = py_stmt!(
        r#"
def {func_name:id}():
    return {body:expr}
"#,
        func_name = func_name.as_str(),
        body = *body,
    );

    if let Stmt::FunctionDef(ast::StmtFunctionDef {
        ref mut parameters, ..
    }) = &mut func_def[0]
    {
        *parameters = Box::new(updated_parameters);
    }

    buf.extend(func_def);

    LoweredExpr::modified(py_expr!("{func:id}", func = func_name.as_str()), buf)
}

pub(crate) fn rewrite_generator(
    generator: ast::ExprGenerator,
    ctx: &Context,
    scope: &Scope,
) -> LoweredExpr {

    let needs_async = comprehension_needs_async(&generator.elt, &generator.generators);
    let ast::ExprGenerator {
        elt, generators, ..
    } = generator;

    let named_expr_targets = collect_named_expr_targets(&elt, &generators, true);
    let (global_targets, nonlocal_targets) =
        classify_named_expr_targets(&named_expr_targets, scope);

    let first_iter_expr = generators
        .first()
        .expect("generator expects at least one comprehension")
        .iter
        .clone();

    let func_name = ctx.fresh("gen");

    let param_name = Name::new(ctx.fresh("iter"));

    let mut body = py_stmt!("yield {value:expr}", value = *elt);

    for comp in generators.iter().rev() {
        let mut inner = body;
        for if_expr in comp.ifs.iter().rev() {
            inner = py_stmt!(
                r#"
if {test:expr}:
    {body:stmt}
"#,
                test = if_expr.clone(),
                body = inner,
            )
        }
        body = if comp.is_async {
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
        };
    }

    if let Stmt::For(ast::StmtFor { iter, .. }) = body.first_mut().unwrap() {
        *iter = Box::new(py_expr!("{name:id}", name = param_name.as_str()));
    }

    // TODO: qualname
    let mut func_def = if needs_async {
        py_stmt!(
            r#"
async def {func:id}({param:id}):
    {body:stmt}
"#,
            func = func_name.as_str(),
            param = param_name.as_str(),
            body = body,
        )
    } else {
        py_stmt!(
            r#"
def {func:id}({param:id}):
    {body:stmt}
"#,
            func = func_name.as_str(),
            param = param_name.as_str(),
            body = body,
        )
    };

    if let Stmt::FunctionDef(func_def_stmt) = &mut func_def[0] {
        rewrite_named_expr_targets(func_def_stmt, &global_targets, &nonlocal_targets);
    }

    let expr = py_expr!(
        "{func:id}({iter:expr})",
        iter = first_iter_expr,
        func = func_name.as_str(),
    );
    LoweredExpr::modified(expr, func_def) 
}


fn generators_need_async(generators: &[ast::Comprehension]) -> bool {
    generators.iter().any(|comp| {
        comp.is_async
            || expr_contains_await(&comp.iter)
            || comp.ifs.iter().any(expr_contains_await)
    })
}

pub fn comprehension_needs_async(elt: &Expr, generators: &[ast::Comprehension]) -> bool {
    expr_contains_await(elt) || generators_need_async(generators)
}

pub fn rewrite(
    context: &Context,
    scope: &Scope,
    elt: Expr,
    generators: Vec<ast::Comprehension>,
    container_type: &str,
    append_fn: &str,
) -> LoweredExpr {
    let mut needs_async = comprehension_needs_async(&elt, &generators);
    let first_iter_expr = generators
        .first()
        .expect("comprehension expects at least one generator")
        .iter
        .clone();
    let lowered_iter = super::lower_expr(context, first_iter_expr);

    let func_name = context.fresh("comp");
    let param_name = Name::new(context.fresh("iter"));
    let result_name = context.fresh("result");


    let named_expr_targets = collect_named_expr_targets(&elt, &generators, true);
    let (global_targets, nonlocal_targets) =
        classify_named_expr_targets(&named_expr_targets, scope);

    let mut body = if container_type == "dict" {
        let (key, value) = match elt {
            Expr::Tuple(ast::ExprTuple { mut elts, .. }) if elts.len() == 2 => {
                (elts.remove(0), elts.remove(0))
            }
            _ => unreachable!("dict comprehension expects tuple key/value"),
        };
        py_stmt!(
            "__dp__.setitem({result:id}, {key:expr}, {value:expr})",
            result = result_name.as_str(),
            key = key,
            value = value,
        )
    } else {
        py_stmt!(
            "{result:id}.{append_fn:id}({value:expr})",
            result = result_name.as_str(),
            append_fn = append_fn,
            value = elt,
        )
    };

    for comp in generators.iter().rev() {
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
        body = if comp.is_async {
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
        };
    }

    if let Stmt::For(ast::StmtFor { iter, .. }) = body.first_mut().unwrap() {
        *iter = Box::new(py_expr!("{name:id}", name = param_name.as_str()));
    }


    let mut func_def = py_stmt!(
        r#"
def {func:id}({param:id}):
    {result:id} = __dp__.{container_type:id}()
    {body:stmt}
    return {result:id}
"#,
        func = func_name.as_str(),
        param = param_name.as_str(),
        body = body,
        result = result_name.as_str(),
        container_type = container_type,
    );
    if let Stmt::FunctionDef(func_def_stmt) = &mut func_def[0] {
        rewrite_named_expr_targets(func_def_stmt, &global_targets, &nonlocal_targets);
        asyncify_internal_function(func_def_stmt);
        if needs_async {
            func_def_stmt.is_async = true;
        }
        needs_async = func_def_stmt.is_async;
    }

    let call_expr = py_expr!(
        "{func:id}({iter:expr})",
        iter = lowered_iter.expr,
        func = func_name.as_str(),
    );
    let expr = if needs_async {
        py_expr!("await {value:expr}", value = call_expr)
    } else {
        call_expr
    };

    let mut stmts = func_def;
    stmts.extend(lowered_iter.stmts);
    LoweredExpr::modified(expr, stmts)
}

fn collect_named_expr_targets(
    elt: &Expr,
    generators: &[ast::Comprehension],
    skip_first_iter: bool,
) -> HashSet<String> {
    let mut collector = NamedExprCollector {
        names: HashSet::new(),
    };

    let mut elt_clone = elt.clone();
    collector.visit_expr(&mut elt_clone);

    for (index, comp) in generators.iter().enumerate() {
        for if_expr in &comp.ifs {
            let mut if_clone = if_expr.clone();
            collector.visit_expr(&mut if_clone);
        }
        if !skip_first_iter || index > 0 {
            let mut iter_clone = comp.iter.clone();
            collector.visit_expr(&mut iter_clone);
        }
    }

    collector.names
}

fn classify_named_expr_targets(
    names: &HashSet<String>,
    scope: &Scope,
) -> (Vec<String>, Vec<String>) {
    if names.is_empty() {
        return (Vec::new(), Vec::new());
    }

    if matches!(scope.kind(), ScopeKind::Module) {
        let mut globals = names.iter().cloned().collect::<Vec<_>>();
        globals.sort();
        return (globals, Vec::new());
    }

    let mut global_targets = Vec::new();
    let mut nonlocal_targets = Vec::new();

    for name in names {
        if scope.is_global(name)
            || unmangle_private_name(name)
                .is_some_and(|unmangled| scope.is_global(unmangled.as_str()))
        {
            global_targets.push(name.clone());
        } else {
            nonlocal_targets.push(name.clone());
        }
    }

    global_targets.sort();
    nonlocal_targets.sort();
    (global_targets, nonlocal_targets)
}

fn unmangle_private_name(name: &str) -> Option<String> {
    if !name.starts_with('_') {
        return None;
    }
    let rest = &name[1..];
    let sep = rest.find("__")?;
    if sep == 0 {
        return None;
    }
    Some(format!("__{}", &rest[sep + 2..]))
}

struct NamedExprCollector {
    names: HashSet<String>,
}

impl Transformer for NamedExprCollector {
    fn visit_expr(&mut self, expr: &mut Expr) {
        match expr {
            Expr::Named(ast::ExprNamed { target, value, .. }) => {
                if let Expr::Name(ast::ExprName { id, .. }) = target.as_ref() {
                    self.names.insert(id.as_str().to_string());
                } else {
                    self.visit_expr(target);
                }
                self.visit_expr(value);
                return;
            }
            Expr::Lambda(_)
            | Expr::Generator(_)
            | Expr::ListComp(_)
            | Expr::SetComp(_)
            | Expr::DictComp(_) => {
                return;
            }
            _ => {}
        }

        walk_expr(self, expr);
    }
}

fn asyncify_internal_function(func_def: &mut ast::StmtFunctionDef) -> bool {
    if func_def.is_async || !func_def.name.id.as_str().starts_with("_dp_") {
        return func_def.is_async;
    }

    struct AwaitFinder {
        found: bool,
    }

    impl Transformer for AwaitFinder {
        fn visit_stmt(&mut self, stmt: &mut Stmt) {
            if self.found {
                return;
            }
            match stmt {
                Stmt::FunctionDef(_) | Stmt::ClassDef(_) => return,
                Stmt::For(ast::StmtFor { is_async, .. }) => {
                    if *is_async {
                        self.found = true;
                        return;
                    }
                }
                _ => {}
            }
            walk_stmt(self, stmt);
        }

        fn visit_expr(&mut self, expr: &mut Expr) {
            if self.found {
                return;
            }
            if matches!(expr, Expr::Await(_)) {
                self.found = true;
                return;
            }
            if matches!(expr, Expr::Lambda(_)) {
                return;
            }
            walk_expr(self, expr);
        }
    }

    let mut finder = AwaitFinder { found: false };
    for stmt in &mut func_def.body {
        finder.visit_stmt(stmt);
        if finder.found {
            break;
        }
    }

    if finder.found {
        func_def.is_async = true;
    }
    func_def.is_async
}

fn expr_contains_await(expr: &Expr) -> bool {
    struct AwaitFinder {
        found: bool,
    }

    impl Transformer for AwaitFinder {
        fn visit_expr(&mut self, expr: &mut Expr) {
            if self.found {
                return;
            }

            let has_async_generator = match expr {
                Expr::ListComp(ast::ExprListComp { generators, .. })
                | Expr::SetComp(ast::ExprSetComp { generators, .. })
                | Expr::DictComp(ast::ExprDictComp { generators, .. }) => {
                    generators.iter().any(|comp| comp.is_async)
                }
                _ => false,
            };

            if matches!(expr, Expr::Generator(_)) {
                return;
            }

            if has_async_generator || matches!(expr, Expr::Await(_)) {
                self.found = true;
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
