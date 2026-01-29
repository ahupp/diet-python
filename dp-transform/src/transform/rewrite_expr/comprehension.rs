use std::collections::{HashMap, HashSet};

use ruff_python_ast::{self as ast};
use ruff_python_ast::{Expr, ExprContext, Stmt};

use crate::template::into_body;
use crate::transform::ast_rewrite::LoweredExpr;

use crate::{py_expr, py_stmt};
use crate::transform::context::Context;
use crate::transformer::{Transformer, walk_expr};
use ruff_python_ast::name::Name;



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
                (&mut renamer).visit_expr(expr);
                return;
            }
            _ => {}
        }
        walk_expr(self, expr);
    }
}

fn rename_loads(mut expr: Expr, renames: &HashMap<String, Name>) -> Expr {
    let mut renamer = LoadRenamer { renames };
    (&mut renamer).visit_expr(&mut expr);
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
        block.push(lowered.stmt);
        block.push(
            py_stmt!(
                r#"
if {test:expr}:
    {body:stmt}
"#,
                test = lowered.expr,
                body = body,
            )
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
    let stmts = match kind {
        InlineCompKind::List => py_stmt!("{name:id} = []", name = result_name.as_str()),
        InlineCompKind::Set => py_stmt!("{name:id} = set()", name = result_name.as_str()),
        InlineCompKind::Dict => py_stmt!("{name:id} = {}", name = result_name.as_str()),
    };
    let mut stmts = vec![stmts];

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
            let mut body = vec![lowered_key.stmt];
            body.push(lowered_value.stmt);
            body.push(py_stmt!(
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
            let mut body = vec![lowered_elt.stmt];
            body.push(py_stmt!(
                "{result:id}.append({value:expr})",
                result = result_name.as_str(),
                value = lowered_elt.expr,
            ));
            body
        }
        InlineCompKind::Set => {
            let elt_expr = rename_loads(elt_or_key, &renames);
            let lowered_elt = super::lower_expr(context, elt_expr);
            let mut body = vec![lowered_elt.stmt];
            body.push(py_stmt!(
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
        };
        body = vec![gen.iter.stmt, for_stmt];
    }

    stmts.extend(body);
    LoweredExpr::modified(result_expr, into_body(stmts))
}


