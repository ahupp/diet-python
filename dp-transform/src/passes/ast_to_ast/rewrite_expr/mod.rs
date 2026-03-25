use std::collections::HashSet;
use std::mem::take;

use ruff_python_ast::{self as ast, Expr, ExprContext, Operator, Stmt, UnaryOp};
use ruff_python_codegen::{Generator, Indentation};
use ruff_source_file::LineEnding;
use ruff_text_size::TextRange;

use crate::passes::ast_to_ast::ast_rewrite::{push_stmts, BodyBuilder, ExprRewritePass};
use crate::passes::ast_to_ast::expr_utils::{
    make_binop, make_tuple, make_tuple_splat, make_unaryop,
};
use crate::passes::ast_to_ast::scope_helpers::ScopeKind;
use crate::transformer::{walk_expr, Transformer};
use crate::{
    passes::ast_to_ast::{ast_rewrite::LoweredExpr, context::Context},
    py_expr, py_stmt, py_stmt_typed,
};
use ruff_python_ast::Identifier;

pub mod comprehension;
pub mod string;

fn panic_for_deferred_expr(expr: &Expr) -> ! {
    let message = match expr {
        Expr::ListComp(_)
        | Expr::SetComp(_)
        | Expr::DictComp(_)
        | Expr::Lambda(_)
        | Expr::Generator(_) => "helper-scoped expr leaked to lower_expr",
        Expr::If(_) => "expr-if leaked to lower_expr",
        Expr::FString(_) | Expr::TString(_) => "string template leaked to lower_expr",
        other => panic!(
            "unexpected deferred expr leaked to lower_expr: {}",
            crate::ruff_ast_to_string(other)
        ),
    };
    panic!("{message}: {}", crate::ruff_ast_to_string(expr));
}

pub(super) fn lower_expr_nested(context: &Context, expr: Expr) -> LoweredExpr {
    lower_expr_impl(context, expr, true)
}

fn lower_expr_impl(context: &Context, expr: Expr, allow_deferred: bool) -> LoweredExpr {
    match expr {
        Expr::Attribute(ast::ExprAttribute {
            value,
            attr,
            ctx: ExprContext::Load,
            range: _,
            node_index: _,
        }) if attr.id.as_str() == "f_locals" => {
            let lowered = lower_expr_nested(context, *value);
            let mut body_builder = BodyBuilder::default();
            let value_expr = body_builder.push(lowered);
            let expr = py_expr!("__dp_frame_locals({value:expr})", value = value_expr);
            return LoweredExpr::modified(expr, body_builder.into_stmts());
        }
        Expr::Attribute(ast::ExprAttribute {
            value,
            attr,
            ctx: ExprContext::Load,
            range: _,
            node_index: _,
        }) if context.options.lower_attributes => {
            let lowered = lower_expr_nested(context, *value);
            let mut body_builder = BodyBuilder::default();
            let value_expr = body_builder.push(lowered);
            let expr = py_expr!(
                "__dp_getattr({value:expr}, {attr:literal})",
                value = value_expr,
                attr = attr.id.as_str(),
            );
            return LoweredExpr::modified(expr, body_builder.into_stmts());
        }
        Expr::Call(ast::ExprCall {
            func,
            arguments,
            range,
            node_index,
        }) => {
            let ast::Arguments {
                args,
                keywords,
                range: args_range,
                node_index: args_node_index,
            } = arguments;

            let func_lowered = lower_expr_nested(context, *func);
            let mut body_builder = BodyBuilder::default();
            let func_expr = body_builder.push(func_lowered);
            let mut new_args = Vec::new();
            for arg in args.into_vec() {
                match arg {
                    Expr::Starred(ast::ExprStarred {
                        value,
                        ctx,
                        range,
                        node_index,
                    }) => {
                        let lowered = lower_expr_nested(context, *value);

                        let expr = body_builder.push(lowered);
                        new_args.push(Expr::Starred(ast::ExprStarred {
                            value: Box::new(expr),
                            ctx,
                            range,
                            node_index,
                        }));
                    }
                    other => {
                        let expr = body_builder.push(lower_expr_nested(context, other));
                        new_args.push(expr);
                    }
                }
            }

            let mut new_keywords = Vec::new();
            for keyword in keywords.into_vec() {
                let ast::Keyword {
                    arg,
                    value,
                    range,
                    node_index,
                } = keyword;
                let lowered = lower_expr_nested(context, value);

                let expr = body_builder.push(lowered);
                new_keywords.push(ast::Keyword {
                    arg,
                    value: expr,
                    range,
                    node_index,
                });
            }

            let new_call = Expr::Call(ast::ExprCall {
                func: Box::new(func_expr),
                arguments: ast::Arguments {
                    args: new_args.into(),
                    keywords: new_keywords.into(),
                    range: args_range,
                    node_index: args_node_index,
                },
                range,
                node_index,
            });
            if !body_builder.modified {
                LoweredExpr::unmodified(new_call)
            } else {
                LoweredExpr::modified(new_call, body_builder.into_stmts())
            }
        }
        Expr::Slice(ast::ExprSlice {
            lower, upper, step, ..
        }) => LoweredExpr::unmodified(py_expr!(
            "__dp_slice({lower:expr}, {upper:expr}, {step:expr})",
            lower = lower.map(|expr| *expr).unwrap_or_else(|| py_expr!("None")),
            upper = upper.map(|expr| *expr).unwrap_or_else(|| py_expr!("None")),
            step = step.map(|expr| *expr).unwrap_or_else(|| py_expr!("None")),
        )),
        Expr::ListComp(_)
        | Expr::SetComp(_)
        | Expr::DictComp(_)
        | Expr::Lambda(_)
        | Expr::Generator(_)
        | Expr::FString(_)
        | Expr::TString(_)
        | Expr::If(_) => {
            if allow_deferred {
                LoweredExpr::unmodified(expr)
            } else {
                panic_for_deferred_expr(&expr)
            }
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
            LoweredExpr::unmodified(py_expr!(
                "complex({real:expr}, {imag:expr})",
                real = real_expr,
                imag = imag_expr,
            ))
        }
        Expr::NumberLiteral(ast::ExprNumberLiteral {
            value: ast::Number::Float(_value),
            range,
            ..
        }) => {
            let src = context.source_slice(range).expect("missing source slice");
            let src = src.trim();
            let normalized = src.replace('_', "");
            let indent = Indentation::new("    ".to_string());
            let default = Generator::new(&indent, LineEnding::default()).expr(&expr);
            if normalized.len() >= 10 && normalized != default {
                LoweredExpr::unmodified(py_expr!(
                    "__dp_float_from_literal({literal:literal})",
                    literal = src
                ))
            } else {
                LoweredExpr::unmodified(expr)
            }
        }
        // tuple/list/dict unpacking
        Expr::Tuple(tuple)
            if matches!(tuple.ctx, ast::ExprContext::Load)
                && tuple.elts.iter().any(|elt| matches!(elt, Expr::Starred(_))) =>
        {
            let ast::ExprTuple { elts, .. } = tuple;
            let mut stmts = vec![];
            let mut modified = false;
            let mut lowered_elts = Vec::new();
            for elt in elts {
                match elt {
                    Expr::Starred(ast::ExprStarred {
                        value,
                        ctx: star_ctx,
                        range: star_range,
                        node_index: star_node_index,
                    }) => {
                        let lowered = lower_expr_nested(context, *value);
                        modified |= lowered.modified;
                        stmts.extend(lowered.stmts);
                        lowered_elts.push(Expr::Starred(ast::ExprStarred {
                            value: Box::new(lowered.expr),
                            ctx: star_ctx,
                            range: star_range,
                            node_index: star_node_index,
                        }));
                    }
                    other => {
                        let lowered = lower_expr_nested(context, other);
                        modified |= lowered.modified;
                        stmts.extend(lowered.stmts);
                        lowered_elts.push(lowered.expr);
                    }
                }
            }
            let expr = make_tuple_splat(lowered_elts);
            if stmts.is_empty() && !modified {
                LoweredExpr::unmodified(expr)
            } else {
                LoweredExpr::modified(expr, stmts)
            }
        }
        Expr::Tuple(ast::ExprTuple {
            elts,
            ctx,
            range,
            node_index,
            parenthesized,
        }) if matches!(ctx, ast::ExprContext::Load) => {
            let mut stmts = Vec::new();
            let mut modified = false;
            let mut lowered_elts = Vec::new();
            for elt in elts {
                let lowered = lower_expr_nested(context, elt);
                modified |= lowered.modified;
                modified |= push_stmts(&mut stmts, lowered.stmts);
                lowered_elts.push(lowered.expr);
            }
            let expr = Expr::Tuple(ast::ExprTuple {
                elts: lowered_elts,
                ctx,
                range,
                node_index,
                parenthesized,
            });
            if !modified {
                LoweredExpr::unmodified(expr)
            } else {
                LoweredExpr::modified(expr, stmts)
            }
        }
        Expr::List(list) if matches!(list.ctx, ast::ExprContext::Load) => {
            let ast::ExprList { elts, .. } = list;
            let mut lowered_elts = Vec::new();
            let mut body_builder = BodyBuilder::default();
            for elt in elts {
                match elt {
                    Expr::Starred(ast::ExprStarred {
                        value,
                        ctx: star_ctx,
                        range: star_range,
                        node_index: star_node_index,
                    }) => {
                        let expr = body_builder.push(lower_expr_nested(context, *value));
                        lowered_elts.push(Expr::Starred(ast::ExprStarred {
                            value: Box::new(expr),
                            ctx: star_ctx,
                            range: star_range,
                            node_index: star_node_index,
                        }));
                    }
                    other => {
                        let expr = body_builder.push(lower_expr_nested(context, other));
                        lowered_elts.push(expr);
                    }
                }
            }
            let tuple = make_tuple_splat(lowered_elts);
            let expr = py_expr!("__dp_list({tuple:expr})", tuple = tuple,);
            if !body_builder.modified {
                LoweredExpr::unmodified(expr)
            } else {
                LoweredExpr::modified(expr, body_builder.into_stmts())
            }
        }
        Expr::Set(ast::ExprSet { elts, .. }) => {
            let mut lowered_elts = Vec::new();
            let mut body_builder = BodyBuilder::default();
            for elt in elts {
                let expr = body_builder.push(lower_expr_nested(context, elt));
                lowered_elts.push(expr);
            }
            let tuple = make_tuple(lowered_elts);
            let expr = py_expr!("__dp_set({tuple:expr})", tuple = tuple,);
            if !body_builder.modified {
                LoweredExpr::unmodified(expr)
            } else {
                LoweredExpr::modified(expr, body_builder.into_stmts())
            }
        }
        Expr::Dict(ast::ExprDict { items, .. }) => {
            let mut segments: Vec<Expr> = Vec::new();

            let mut keyed_pairs = Vec::new();

            let mut body_builder = BodyBuilder::default();
            for item in items.into_iter() {
                match item {
                    ast::DictItem {
                        key: Some(key),
                        value,
                    } => {
                        let key_expr = body_builder.push(lower_expr_nested(context, key));
                        let value_expr = body_builder.push(lower_expr_nested(context, value));
                        keyed_pairs.push(py_expr!(
                            "({key:expr}, {value:expr})",
                            key = key_expr,
                            value = value_expr,
                        ));
                    }
                    ast::DictItem { key: None, value } => {
                        let value_expr = body_builder.push(lower_expr_nested(context, value));
                        if !keyed_pairs.is_empty() {
                            let tuple = make_tuple(take(&mut keyed_pairs));
                            segments.push(py_expr!("__dp_dict({tuple:expr})", tuple = tuple));
                        }
                        segments.push(py_expr!("__dp_dict({mapping:expr})", mapping = value_expr));
                    }
                }
            }

            if !keyed_pairs.is_empty() {
                let tuple = make_tuple(take(&mut keyed_pairs));
                segments.push(py_expr!("__dp_dict({tuple:expr})", tuple = tuple));
            }

            let expr = match segments.len() {
                0 => py_expr!("__dp_dict()"),
                _ => segments
                    .into_iter()
                    .reduce(|left, right| make_binop("or_", left, right))
                    .expect("segments is non-empty"),
            };
            if !body_builder.modified {
                LoweredExpr::unmodified(expr)
            } else {
                LoweredExpr::modified(expr, body_builder.into_stmts())
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
            LoweredExpr::unmodified(make_binop(func_name, *left, *right))
        }
        Expr::UnaryOp(ast::ExprUnaryOp { operand, op, .. }) => {
            let func_name = match op {
                UnaryOp::Not => "not_",
                UnaryOp::Invert => "invert",
                UnaryOp::USub => "neg",
                UnaryOp::UAdd => "pos",
            };
            let lowered = lower_expr_nested(context, *operand);
            let expr = make_unaryop(func_name, lowered.expr);
            if lowered.modified {
                LoweredExpr::modified(expr, lowered.stmts)
            } else {
                LoweredExpr::unmodified(expr)
            }
        }
        Expr::Subscript(ast::ExprSubscript {
            value, slice, ctx, ..
        }) if matches!(ctx, ast::ExprContext::Load) => {
            let value_lowered = lower_expr_nested(context, *value);
            let slice_lowered = lower_expr_nested(context, *slice);
            let mut stmts = value_lowered.stmts;
            stmts.extend(slice_lowered.stmts);

            let expr = make_binop("getitem", value_lowered.expr, slice_lowered.expr);
            if stmts.is_empty() && !value_lowered.modified && !slice_lowered.modified {
                LoweredExpr::unmodified(expr)
            } else {
                LoweredExpr::modified(expr, stmts)
            }
        }
        other => LoweredExpr::unmodified(other),
    }
}

fn lower_lambda_expr(
    context: &Context,
    parameters: Option<ast::Parameters>,
    body: Expr,
) -> LoweredExpr {
    let func_name = context.fresh("lambda");
    let mut func_def: ast::StmtFunctionDef = py_stmt_typed!(
        r#"
def {func:id}():
    pass
"#,
        func = func_name.as_str(),
    );
    if let Some(params) = parameters {
        func_def.parameters = Box::new(params);
    }
    let return_stmt = py_stmt!("return {value:expr}", value = body);
    func_def.body = vec![return_stmt];
    LoweredExpr::modified(
        py_expr!("{func:id}", func = func_name.as_str()),
        vec![func_def.into()],
    )
}

fn lower_generator_expr(
    context: &Context,
    mut elt: Expr,
    mut generators: Vec<ast::Comprehension>,
) -> LoweredExpr {
    let scope = context.current_scope();
    let named_targets = collect_named_expr_targets(&elt, &generators);
    let mut global_targets: HashSet<String> = HashSet::new();
    let mut class_targets: HashSet<String> = HashSet::new();
    let mut nonlocal_targets: HashSet<String> = HashSet::new();
    let mut dummy_targets: Vec<String> = Vec::new();

    match scope.kind {
        ScopeKind::Module => {
            global_targets = named_targets;
        }
        ScopeKind::Class => {
            class_targets = named_targets;
        }
        ScopeKind::Function => {
            for name in named_targets {
                if scope.globals.contains(&name) {
                    global_targets.insert(name);
                } else {
                    nonlocal_targets.insert(name.clone());
                    if !scope.nonlocals.contains(&name) {
                        dummy_targets.push(name);
                    }
                }
            }
        }
    }

    if !class_targets.is_empty() {
        let mut rewriter = NamedExprRewriter::new(&class_targets);
        rewriter.visit_expr(&mut elt);
        for gen in &mut generators {
            rewriter.visit_expr(&mut gen.iter);
            for if_expr in &mut gen.ifs {
                rewriter.visit_expr(if_expr);
            }
        }
    }

    let first_gen = match generators.first() {
        Some(gen) => gen,
        None => {
            return LoweredExpr::unmodified(Expr::Generator(ast::ExprGenerator {
                elt: Box::new(elt),
                generators,
                parenthesized: true,
                range: TextRange::default(),
                node_index: ast::AtomicNodeIndex::default(),
            }))
        }
    };

    let outer_async = first_gen.is_async;
    let iter_expr = first_gen.iter.clone();
    let iter_lowered = lower_expr_nested(context, iter_expr);
    let iter_value = if outer_async {
        py_expr!("__dp_aiter({iter:expr})", iter = iter_lowered.expr.clone())
    } else {
        py_expr!("__dp_iter({iter:expr})", iter = iter_lowered.expr.clone())
    };

    let func_name = context.fresh("genexpr");
    let iter_param = context.fresh("iter");
    let is_async = genexpr_requires_async(&elt, &generators);
    let mut func_def: ast::StmtFunctionDef = if is_async {
        py_stmt_typed!(
            r#"
async def {func:id}({param:id}):
    pass
"#,
            func = func_name.as_str(),
            param = iter_param.as_str(),
        )
    } else {
        py_stmt_typed!(
            r#"
def {func:id}({param:id}):
    pass
"#,
            func = func_name.as_str(),
            param = iter_param.as_str(),
        )
    };

    let iter_param_expr = py_expr!("{name:id}", name = iter_param.as_str());
    let mut body = vec![py_stmt!("yield {value:expr}", value = elt)];
    let gen_count = generators.len();
    for (rev_index, gen) in generators.into_iter().rev().enumerate() {
        let is_outermost = rev_index + 1 == gen_count;
        let loop_body = wrap_ifs(body, gen.ifs);
        let iter = if is_outermost {
            iter_param_expr.clone()
        } else {
            gen.iter
        };
        let for_stmts = if gen.is_async {
            if is_outermost {
                let iter_name = context.fresh("iter");
                let target_tmp = context.fresh("tmp");
                crate::py_stmts!(
                    r#"
{iter_name:id} = {iter:expr}
while True:
    {target_tmp:id} = await __dp_anext_or_sentinel({iter_name:id})
    if {target_tmp:id} is __dp__.ITER_COMPLETE:
        break
    else:
        {target:expr} = {target_tmp:id}
        {body:stmt}
"#,
                    iter_name = iter_name.as_str(),
                    iter = iter,
                    target_tmp = target_tmp.as_str(),
                    target = gen.target,
                    body = loop_body,
                )
            } else {
                vec![py_stmt!(
                    r#"
async for {target:expr} in {iter:expr}:
    {body:stmt}
"#,
                    target = gen.target,
                    iter = iter,
                    body = loop_body,
                )]
            }
        } else if is_outermost {
            let iter_name = context.fresh("iter");
            let target_tmp = context.fresh("tmp");
            crate::py_stmts!(
                r#"
{iter_name:id} = {iter:expr}
while True:
    {target_tmp:id} = __dp_next_or_sentinel({iter_name:id})
    if {target_tmp:id} is __dp__.ITER_COMPLETE:
        break
    else:
        {target:expr} = {target_tmp:id}
        {body:stmt}
"#,
                iter_name = iter_name.as_str(),
                iter = iter,
                target_tmp = target_tmp.as_str(),
                target = gen.target,
                body = loop_body,
            )
        } else {
            vec![py_stmt!(
                r#"
for {target:expr} in {iter:expr}:
    {body:stmt}
"#,
                target = gen.target,
                iter = iter,
                body = loop_body,
            )]
        };
        body = for_stmts;
    }

    let mut func_body: Vec<Stmt> = Vec::new();
    if !global_targets.is_empty() {
        let mut names = global_targets.into_iter().collect::<Vec<_>>();
        names.sort();
        let names = names
            .into_iter()
            .map(|name| Identifier::new(name, TextRange::default()))
            .collect();
        func_body.push(Stmt::Global(ast::StmtGlobal {
            names,
            range: TextRange::default(),
            node_index: ast::AtomicNodeIndex::default(),
        }));
    }
    if !nonlocal_targets.is_empty() {
        let mut names = nonlocal_targets.into_iter().collect::<Vec<_>>();
        names.sort();
        let names = names
            .into_iter()
            .map(|name| Identifier::new(name, TextRange::default()))
            .collect();
        func_body.push(Stmt::Nonlocal(ast::StmtNonlocal {
            names,
            range: TextRange::default(),
            node_index: ast::AtomicNodeIndex::default(),
        }));
    }
    func_body.extend(body);
    func_def.body = func_body;

    let mut prefix: Vec<Stmt> = Vec::new();
    for name in dummy_targets {
        prefix.push(py_stmt!(
            r#"
if False:
    {name:id} = None
"#,
            name = name.as_str()
        ));
    }
    prefix.extend(iter_lowered.stmts);
    prefix.push(func_def.into());

    let call_expr = py_expr!(
        "{func:id}({iter:expr})",
        func = func_name.as_str(),
        iter = iter_value,
    );

    LoweredExpr::modified(call_expr, prefix)
}

fn genexpr_requires_async(elt: &Expr, generators: &[ast::Comprehension]) -> bool {
    if generators.iter().any(|gen| gen.is_async) {
        return true;
    }
    if expr_requires_async(elt) {
        return true;
    }
    for gen in generators {
        if expr_requires_async(&gen.iter) {
            return true;
        }
        for if_expr in &gen.ifs {
            if expr_requires_async(if_expr) {
                return true;
            }
        }
    }
    false
}

fn comp_is_async(
    elt_or_key: &Expr,
    value: Option<&Expr>,
    generators: &[ast::Comprehension],
) -> bool {
    if generators.iter().any(|gen| gen.is_async) {
        return true;
    }
    if expr_requires_async(elt_or_key) {
        return true;
    }
    if let Some(value) = value {
        if expr_requires_async(value) {
            return true;
        }
    }
    for gen in generators {
        if expr_requires_async(&gen.iter) {
            return true;
        }
        for if_expr in &gen.ifs {
            if expr_requires_async(if_expr) {
                return true;
            }
        }
    }
    false
}

fn expr_requires_async(expr: &Expr) -> bool {
    #[derive(Default)]
    struct AwaitFinder {
        found: bool,
    }

    impl Transformer for AwaitFinder {
        fn visit_expr(&mut self, expr: &mut Expr) {
            if self.found {
                return;
            }
            match expr {
                Expr::Await(_) => {
                    self.found = true;
                    return;
                }
                Expr::ListComp(ast::ExprListComp {
                    elt, generators, ..
                }) => {
                    if comp_is_async(elt.as_ref(), None, generators) {
                        self.found = true;
                    }
                    return;
                }
                Expr::SetComp(ast::ExprSetComp {
                    elt, generators, ..
                }) => {
                    if comp_is_async(elt.as_ref(), None, generators) {
                        self.found = true;
                    }
                    return;
                }
                Expr::DictComp(ast::ExprDictComp {
                    key,
                    value,
                    generators,
                    ..
                }) => {
                    if comp_is_async(key.as_ref(), Some(value.as_ref()), generators) {
                        self.found = true;
                    }
                    return;
                }
                Expr::Lambda(_) | Expr::Generator(_) => {
                    return;
                }
                _ => {}
            }
            walk_expr(self, expr);
        }
    }

    let mut finder = AwaitFinder::default();
    let mut expr = expr.clone();
    finder.visit_expr(&mut expr);
    finder.found
}

fn wrap_ifs(mut body: Vec<Stmt>, ifs: Vec<Expr>) -> Vec<Stmt> {
    for if_expr in ifs.into_iter().rev() {
        body = vec![py_stmt!(
            r#"
if {test:expr}:
    {body:stmt}
"#,
            test = if_expr,
            body = body,
        )];
    }
    body
}

fn collect_named_expr_targets(elt: &Expr, generators: &[ast::Comprehension]) -> HashSet<String> {
    let mut collector = NamedExprTargetCollector::default();
    let mut elt_clone = elt.clone();
    collector.visit_expr(&mut elt_clone);
    for gen in generators {
        let mut iter_clone = gen.iter.clone();
        collector.visit_expr(&mut iter_clone);
        for if_expr in &gen.ifs {
            let mut if_clone = if_expr.clone();
            collector.visit_expr(&mut if_clone);
        }
    }
    collector.names
}

#[derive(Default)]
struct NamedExprTargetCollector {
    names: HashSet<String>,
}

impl Transformer for NamedExprTargetCollector {
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
            Expr::Lambda(_) | Expr::Generator(_) => {
                return;
            }
            _ => {}
        }
        walk_expr(self, expr);
    }
}

struct NamedExprRewriter<'a> {
    class_targets: &'a HashSet<String>,
}

impl<'a> NamedExprRewriter<'a> {
    fn new(class_targets: &'a HashSet<String>) -> Self {
        Self { class_targets }
    }
}

impl Transformer for NamedExprRewriter<'_> {
    fn visit_expr(&mut self, expr: &mut Expr) {
        match expr {
            Expr::Named(ast::ExprNamed { target, value, .. }) => {
                if let Expr::Name(ast::ExprName { id, .. }) = target.as_ref() {
                    let name = id.as_str();
                    if self.class_targets.contains(name) {
                        self.visit_expr(value.as_mut());
                        *expr = py_expr!(
                            "__dp_store_global(_dp_class_ns, {name:literal}, {value:expr})",
                            name = name,
                            value = value.as_ref().clone(),
                        );
                        return;
                    }
                }
                self.visit_expr(target.as_mut());
                self.visit_expr(value.as_mut());
                return;
            }
            Expr::Lambda(_) | Expr::Generator(_) => {
                return;
            }
            _ => {}
        }
        walk_expr(self, expr);
    }
}

pub struct ScopedHelperExprPass;

impl ExprRewritePass for ScopedHelperExprPass {
    fn lower_expr(&self, context: &Context, expr: Expr) -> LoweredExpr {
        match expr {
            Expr::ListComp(ast::ExprListComp {
                elt, generators, ..
            }) => comprehension::lower_list_comp(context, *elt, generators),
            Expr::SetComp(ast::ExprSetComp {
                elt, generators, ..
            }) => comprehension::lower_set_comp(context, *elt, generators),
            Expr::DictComp(ast::ExprDictComp {
                key,
                value,
                generators,
                ..
            }) => comprehension::lower_dict_comp(context, *key, *value, generators),
            Expr::Lambda(ast::ExprLambda {
                parameters, body, ..
            }) => lower_lambda_expr(context, parameters.map(|params| *params), *body),
            Expr::Generator(ast::ExprGenerator {
                elt, generators, ..
            }) => lower_generator_expr(context, *elt, generators),
            other => LoweredExpr::unmodified(other),
        }
    }
}

#[cfg(test)]
mod test;
