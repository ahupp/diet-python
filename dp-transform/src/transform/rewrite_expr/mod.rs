use std::collections::HashSet;
use std::mem::take;

use ruff_python_ast::str_prefix::StringLiteralPrefix;
use ruff_python_ast::{self as ast, Expr, ExprContext, Operator, Stmt, UnaryOp};
use ruff_python_codegen::{Generator, Indentation};
use ruff_source_file::LineEnding;
use ruff_text_size::TextRange;

use crate::template::empty_body;
use crate::transform::ast_rewrite::{push_stmt, BodyBuilder};
use crate::transform::scope::ScopeKind;
use crate::transformer::{walk_expr, Transformer};
use crate::{
    py_expr, py_stmt, py_stmt_typed,
    template::into_body,
    transform::{ast_rewrite::LoweredExpr, context::Context},
};
use ruff_python_ast::Identifier;

pub mod compare_boolop;
pub mod comprehension;
pub mod string;
pub mod truthy;

pub fn lower_expr(context: &Context, expr: Expr) -> LoweredExpr {
    match expr {
        Expr::Attribute(ast::ExprAttribute {
            value,
            attr,
            ctx: ExprContext::Load,
            range: _,
            node_index: _,
        }) if attr.id.as_str() == "f_locals" => {
            let lowered = lower_expr(context, *value);
            let mut body_builder = BodyBuilder::default();
            let value_expr = body_builder.push(lowered);
            let expr = py_expr!("__dp__.frame_locals({value:expr})", value = value_expr);
            return LoweredExpr::modified(expr, body_builder.into_stmt());
        }
        Expr::StringLiteral(ast::ExprStringLiteral {
            value,
            range,
            node_index,
        }) => {
            if string_literal_needs_surrogate_decode(context, &value) {
                if let Some(src) = context.source_slice(range) {
                    let literal_src = if value.is_implicit_concatenated() {
                        format!("({src})")
                    } else {
                        src.to_string()
                    };
                    let expr = py_expr!(
                        "__dp__.decode_surrogate_literal({literal:literal})",
                        literal = literal_src.as_str()
                    );
                    return LoweredExpr::modified(expr, empty_body());
                }
            }
            LoweredExpr::unmodified(Expr::StringLiteral(ast::ExprStringLiteral {
                value,
                range,
                node_index,
            }))
        }
        Expr::Named(named_expr) => {
            let ast::ExprNamed { target, value, .. } = named_expr;
            let mut target_expr = *target.clone();
            match &mut target_expr {
                Expr::Name(ast::ExprName { ctx, .. })
                | Expr::Attribute(ast::ExprAttribute { ctx, .. })
                | Expr::Subscript(ast::ExprSubscript { ctx, .. }) => {
                    *ctx = ast::ExprContext::Load;
                }
                _ => {}
            }
            LoweredExpr::modified(
                py_expr!("{target:expr}", target = target_expr),
                py_stmt!(
                    "{target:expr} = {value:expr}",
                    target = *target,
                    value = value
                ),
            )
        }
        Expr::If(if_expr) => {
            let tmp = context.fresh("tmp");
            let ast::ExprIf {
                test, body, orelse, ..
            } = if_expr;
            let stmts = py_stmt!(
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
            LoweredExpr::modified(py_expr!("{tmp:id}", tmp = tmp.as_str()), stmts)
        }
        Expr::BoolOp(bool_op) => compare_boolop::expr_boolop_to_stmts(context, bool_op),
        Expr::Compare(compare) => compare_boolop::expr_compare_to_stmts(context, compare),
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

            let func_lowered = lower_expr(context, *func);
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
                        let lowered = lower_expr(context, *value);

                        let expr = body_builder.push(lowered);
                        new_args.push(Expr::Starred(ast::ExprStarred {
                            value: Box::new(expr),
                            ctx,
                            range,
                            node_index,
                        }));
                    }
                    other => {
                        let expr = body_builder.push(lower_expr(context, other));
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
                let lowered = lower_expr(context, value);

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
                LoweredExpr::modified(new_call, body_builder.into_stmt())
            }
        }
        Expr::FString(f_string) => {
            LoweredExpr::unmodified(string::rewrite_fstring(f_string, context))
        }
        Expr::TString(t_string) => {
            LoweredExpr::unmodified(string::rewrite_tstring(t_string, context))
        }
        Expr::Slice(ast::ExprSlice {
            lower, upper, step, ..
        }) => LoweredExpr::unmodified(py_expr!(
            "__dp__.slice({lower:expr}, {upper:expr}, {step:expr})",
            lower = lower.map(|expr| *expr).unwrap_or_else(|| py_expr!("None")),
            upper = upper.map(|expr| *expr).unwrap_or_else(|| py_expr!("None")),
            step = step.map(|expr| *expr).unwrap_or_else(|| py_expr!("None")),
        )),
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
        }) => {
            let func_name = context.fresh("lambda");
            let mut func_def: ast::StmtFunctionDef = py_stmt_typed!(
                r#"
def {func:id}():
    pass
"#,
                func = func_name.as_str(),
            );
            if let Some(params) = parameters {
                func_def.parameters = params;
            }
            let return_stmt = py_stmt!("return {value:expr}", value = *body);
            func_def.body = ast::StmtBody {
                body: vec![Box::new(return_stmt)],
                range: TextRange::default(),
                node_index: ast::AtomicNodeIndex::default(),
            };
            LoweredExpr::modified(py_expr!("{func:id}", func = func_name.as_str()), func_def)
        }
        Expr::Generator(ast::ExprGenerator {
            elt, generators, ..
        }) => lower_generator_expr(context, *elt, generators),
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
                    "__dp__.float_from_literal({literal:literal})",
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
                        let lowered = lower_expr(context, *value);
                        modified |= lowered.modified;
                        stmts.push(lowered.stmt);
                        lowered_elts.push(Expr::Starred(ast::ExprStarred {
                            value: Box::new(lowered.expr),
                            ctx: star_ctx,
                            range: star_range,
                            node_index: star_node_index,
                        }));
                    }
                    other => {
                        let lowered = lower_expr(context, other);
                        modified |= lowered.modified;
                        stmts.push(lowered.stmt);
                        lowered_elts.push(lowered.expr);
                    }
                }
            }
            let expr = make_tuple_splat(lowered_elts);
            if stmts.is_empty() && !modified {
                LoweredExpr::unmodified(expr)
            } else {
                LoweredExpr::modified(expr, into_body(stmts))
            }
        }
        Expr::Tuple(ast::ExprTuple {
            elts,
            ctx,
            range,
            node_index,
            parenthesized,
        }) if matches!(ctx, ast::ExprContext::Load) => {
            let mut stmts = empty_body().into();
            let mut modified = false;
            let mut lowered_elts = Vec::new();
            for elt in elts {
                let lowered = lower_expr(context, elt);
                modified |= lowered.modified;
                modified |= push_stmt(&mut stmts, lowered.stmt);
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
                        let expr = body_builder.push(lower_expr(context, *value));
                        lowered_elts.push(Expr::Starred(ast::ExprStarred {
                            value: Box::new(expr),
                            ctx: star_ctx,
                            range: star_range,
                            node_index: star_node_index,
                        }));
                    }
                    other => {
                        let expr = body_builder.push(lower_expr(context, other));
                        lowered_elts.push(expr);
                    }
                }
            }
            let tuple = make_tuple_splat(lowered_elts);
            let expr = py_expr!("__dp__.list({tuple:expr})", tuple = tuple,);
            if !body_builder.modified {
                LoweredExpr::unmodified(expr)
            } else {
                LoweredExpr::modified(expr, body_builder.into_stmt())
            }
        }
        Expr::Set(ast::ExprSet { elts, .. }) => {
            let mut lowered_elts = Vec::new();
            let mut body_builder = BodyBuilder::default();
            for elt in elts {
                let expr = body_builder.push(lower_expr(context, elt));
                lowered_elts.push(expr);
            }
            let tuple = make_tuple(lowered_elts);
            let expr = py_expr!("__dp__.set({tuple:expr})", tuple = tuple,);
            if !body_builder.modified {
                LoweredExpr::unmodified(expr)
            } else {
                LoweredExpr::modified(expr, body_builder.into_stmt())
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
                        let key_expr = body_builder.push(lower_expr(context, key));
                        let value_expr = body_builder.push(lower_expr(context, value));
                        keyed_pairs.push(py_expr!(
                            "({key:expr}, {value:expr})",
                            key = key_expr,
                            value = value_expr,
                        ));
                    }
                    ast::DictItem { key: None, value } => {
                        let value_expr = body_builder.push(lower_expr(context, value));
                        if !keyed_pairs.is_empty() {
                            let tuple = make_tuple(take(&mut keyed_pairs));
                            segments.push(py_expr!("__dp__.dict({tuple:expr})", tuple = tuple));
                        }
                        segments.push(py_expr!(
                            "__dp__.dict({mapping:expr})",
                            mapping = value_expr
                        ));
                    }
                }
            }

            if !keyed_pairs.is_empty() {
                let tuple = make_tuple(take(&mut keyed_pairs));
                segments.push(py_expr!("__dp__.dict({tuple:expr})", tuple = tuple));
            }

            let expr = match segments.len() {
                0 => py_expr!("__dp__.dict()"),
                _ => segments
                    .into_iter()
                    .reduce(|left, right| make_binop("or_", left, right))
                    .expect("segments is non-empty"),
            };
            if !body_builder.modified {
                LoweredExpr::unmodified(expr)
            } else {
                LoweredExpr::modified(expr, body_builder.into_stmt())
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
            let lowered = lower_expr(context, *operand);
            let expr = make_unaryop(func_name, lowered.expr);
            if lowered.modified {
                LoweredExpr::modified(expr, lowered.stmt)
            } else {
                LoweredExpr::unmodified(expr)
            }
        }
        Expr::Subscript(ast::ExprSubscript {
            value, slice, ctx, ..
        }) if matches!(ctx, ast::ExprContext::Load) => {
            let value_lowered = lower_expr(context, *value);
            let slice_lowered = lower_expr(context, *slice);
            let stmts = vec![value_lowered.stmt, slice_lowered.stmt];

            let expr = make_binop("getitem", value_lowered.expr, slice_lowered.expr);
            if stmts.is_empty() && !value_lowered.modified && !slice_lowered.modified {
                LoweredExpr::unmodified(expr)
            } else {
                LoweredExpr::modified(expr, into_body(stmts))
            }
        }
        other => LoweredExpr::unmodified(other),
    }
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

    if !global_targets.is_empty() || !class_targets.is_empty() {
        let mut rewriter = NamedExprRewriter::new(&global_targets, &class_targets);
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
    let iter_lowered = lower_expr(context, iter_expr);
    let iter_value = if outer_async {
        py_expr!(
            "__dp__.aiter({iter:expr})",
            iter = iter_lowered.expr.clone()
        )
    } else {
        py_expr!("__dp__.iter({iter:expr})", iter = iter_lowered.expr.clone())
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
        let for_stmt = if gen.is_async {
            if is_outermost {
                let iter_name = context.fresh("iter");
                let target_tmp = context.fresh("tmp");
                let completed_flag = context.fresh("completed");
                py_stmt!(
                    r#"
{completed_flag:id} = False
{iter_name:id} = {iter:expr}
while not {completed_flag:id}:
    {target_tmp:id} = await __dp__.anext_or_sentinel({iter_name:id})
    if {target_tmp:id} is __dp__.ITER_COMPLETE:
        {completed_flag:id} = True
    else:
        {target:expr} = {target_tmp:id}
        {target_tmp:id} = None
        {body:stmt}
"#,
                    iter_name = iter_name.as_str(),
                    iter = iter,
                    target_tmp = target_tmp.as_str(),
                    completed_flag = completed_flag.as_str(),
                    target = gen.target,
                    body = loop_body,
                )
            } else {
                py_stmt!(
                    r#"
async for {target:expr} in {iter:expr}:
    {body:stmt}
"#,
                    target = gen.target,
                    iter = iter,
                    body = loop_body,
                )
            }
        } else if is_outermost {
            let iter_name = context.fresh("iter");
            let target_tmp = context.fresh("tmp");
            let completed_flag = context.fresh("completed");
            py_stmt!(
                r#"
{completed_flag:id} = False
{iter_name:id} = {iter:expr}
while not {completed_flag:id}:
    try:
        {target_tmp:id} = __dp__.next({iter_name:id})
    except __dp__.builtins.StopIteration:
        {completed_flag:id} = True
    else:
        {target:expr} = {target_tmp:id}
        {target_tmp:id} = None
        {body:stmt}
__dp__.truth({completed_flag:id})
"#,
                iter_name = iter_name.as_str(),
                iter = iter,
                target_tmp = target_tmp.as_str(),
                completed_flag = completed_flag.as_str(),
                target = gen.target,
                body = loop_body,
            )
        } else {
            py_stmt!(
                r#"
for {target:expr} in {iter:expr}:
    {body:stmt}
"#,
                target = gen.target,
                iter = iter,
                body = loop_body,
            )
        };
        body = vec![for_stmt];
    }

    let mut func_body: Vec<Stmt> = Vec::new();
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
    func_def.body = ast::StmtBody {
        body: func_body.into_iter().map(Box::new).collect(),
        range: TextRange::default(),
        node_index: ast::AtomicNodeIndex::default(),
    };

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
    append_stmt(&mut prefix, iter_lowered.stmt);
    prefix.push(func_def.into());

    let call_expr = py_expr!(
        "{func:id}({iter:expr})",
        func = func_name.as_str(),
        iter = iter_value,
    );

    LoweredExpr::modified(call_expr, into_body(prefix))
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

fn append_stmt(stmts: &mut Vec<Stmt>, stmt: Stmt) {
    match stmt {
        Stmt::BodyStmt(ast::StmtBody { body, .. }) => {
            for inner in body {
                append_stmt(stmts, *inner);
            }
        }
        other => stmts.push(other),
    }
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
    global_targets: &'a HashSet<String>,
    class_targets: &'a HashSet<String>,
}

impl<'a> NamedExprRewriter<'a> {
    fn new(global_targets: &'a HashSet<String>, class_targets: &'a HashSet<String>) -> Self {
        Self {
            global_targets,
            class_targets,
        }
    }
}

impl Transformer for NamedExprRewriter<'_> {
    fn visit_expr(&mut self, expr: &mut Expr) {
        match expr {
            Expr::Named(ast::ExprNamed { target, value, .. }) => {
                if let Expr::Name(ast::ExprName { id, .. }) = target.as_ref() {
                    let name = id.as_str();
                    if self.global_targets.contains(name) {
                        self.visit_expr(value.as_mut());
                        *expr = py_expr!(
                            "__dp__.store_global(globals(), {name:literal}, {value:expr})",
                            name = name,
                            value = value.as_ref().clone(),
                        );
                        return;
                    }
                    if self.class_targets.contains(name) {
                        self.visit_expr(value.as_mut());
                        *expr = py_expr!(
                            "__dp__.store_global(_dp_class_ns, {name:literal}, {value:expr})",
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

fn string_literal_needs_surrogate_decode(
    context: &Context,
    value: &ast::StringLiteralValue,
) -> bool {
    for literal in value.iter() {
        if matches!(literal.flags.prefix(), StringLiteralPrefix::Raw { .. }) {
            continue;
        }
        if let Some(content) = context.source_slice(literal.content_range()) {
            if has_surrogate_escape(content) {
                return true;
            }
        }
    }
    false
}

fn has_surrogate_escape(content: &str) -> bool {
    let bytes = content.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] != b'\\' {
            i += 1;
            continue;
        }
        if i + 1 >= bytes.len() {
            break;
        }
        match bytes[i + 1] {
            b'u' => {
                if i + 5 < bytes.len() {
                    if let Some(value) = parse_hex(&bytes[i + 2..i + 6]) {
                        if (0xD800..=0xDFFF).contains(&value) {
                            return true;
                        }
                    }
                    i += 6;
                    continue;
                }
                i += 2;
            }
            b'U' => {
                if i + 9 < bytes.len() {
                    if let Some(value) = parse_hex(&bytes[i + 2..i + 10]) {
                        if (0xD800..=0xDFFF).contains(&value) {
                            return true;
                        }
                    }
                    i += 10;
                    continue;
                }
                i += 2;
            }
            _ => {
                i += 2;
            }
        }
    }
    false
}

fn parse_hex(bytes: &[u8]) -> Option<u32> {
    let mut value: u32 = 0;
    for &b in bytes {
        value <<= 4;
        value |= match b {
            b'0'..=b'9' => (b - b'0') as u32,
            b'a'..=b'f' => (b - b'a' + 10) as u32,
            b'A'..=b'F' => (b - b'A' + 10) as u32,
            _ => return None,
        };
    }
    Some(value)
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
        .reduce(|left, right| make_binop("add", left, right))
        .unwrap_or_else(|| make_tuple(Vec::new()))
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
