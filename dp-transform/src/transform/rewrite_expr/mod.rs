use std::mem::take;

use ruff_python_ast::{self as ast, Expr, Operator, UnaryOp};
use ruff_python_ast::str_prefix::StringLiteralPrefix;
use ruff_python_codegen::{Generator, Indentation};
use ruff_source_file::LineEnding;
use ruff_text_size::TextRange;

use crate::{py_expr, py_stmt, transform::{ast_rewrite::LoweredExpr, context::Context}};

pub mod comprehension;
pub mod string;
pub mod compare_boolop;
pub mod truthy;


pub fn lower_expr(context: &Context, expr: Expr) -> LoweredExpr {

    match expr {
        Expr::StringLiteral(ast::ExprStringLiteral { value, range, node_index }) => {
            if string_literal_needs_surrogate_decode(context, &value) {
                if let Some(src) = context.source_slice(range) {
                    let expr = py_expr!(
                        "__dp__.decode_surrogate_literal({literal:literal})",
                        literal = src
                    );
                    return LoweredExpr::modified(expr, Vec::new());
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
            let tmp = context.tmpify("tmp", *value);
            let assign = py_stmt!(
                "{target:expr} = {tmp:expr}",
                target = *target,
                tmp = tmp.expr.clone()
            );
            tmp.extend(assign)
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
        Expr::BoolOp(bool_op) => {
            compare_boolop::expr_boolop_to_stmts(context, bool_op)
        }
        Expr::Compare(compare) => {
            compare_boolop::expr_compare_to_stmts(context, compare)
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
            let mut stmts = Vec::new();
            let mut modified = false;

            let func_lowered = lower_expr(context, *func);
            modified |= func_lowered.modified;
            stmts.extend(func_lowered.stmts);
            let func_expr = func_lowered.expr;

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
                        modified |= lowered.modified;
                        stmts.extend(lowered.stmts);
                        new_args.push(Expr::Starred(ast::ExprStarred {
                            value: Box::new(lowered.expr),
                            ctx,
                            range,
                            node_index,
                        }));
                    }
                    other => {
                        let lowered = lower_expr(context, other);
                        modified |= lowered.modified;
                        stmts.extend(lowered.stmts);
                        new_args.push(lowered.expr);
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
                modified |= lowered.modified;
                stmts.extend(lowered.stmts);
                new_keywords.push(ast::Keyword {
                    arg,
                    value: lowered.expr,
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
            if stmts.is_empty() && !modified {
                LoweredExpr::unmodified(new_call)
            } else {
                LoweredExpr::modified(new_call, stmts)
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
        }) => {
            LoweredExpr::unmodified(py_expr!(
                "__dp__.slice({lower:expr}, {upper:expr}, {step:expr})",
                lower = lower.map(|expr| *expr).unwrap_or_else(|| py_expr!("None")),
                upper = upper.map(|expr| *expr).unwrap_or_else(|| py_expr!("None")),
                step = step.map(|expr| *expr).unwrap_or_else(|| py_expr!("None")),
            ))
        }
        Expr::ListComp(ast::ExprListComp { elt, generators, .. }) => {
            comprehension::lower_inline_list_comp(context, *elt, generators)
        }
        Expr::SetComp(ast::ExprSetComp { elt, generators, .. }) => {
            comprehension::lower_inline_set_comp(context, *elt, generators)
        }
        Expr::DictComp(ast::ExprDictComp {
            key,
            value,
            generators,
            ..
        }) => {
            comprehension::lower_inline_dict_comp(context, *key, *value, generators)
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
            let ast::ExprTuple {
                elts,
                ..
            } = tuple;
            let mut stmts = Vec::new();
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
                        stmts.extend(lowered.stmts);
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
                let lowered = lower_expr(context, elt);
                modified |= lowered.modified;
                stmts.extend(lowered.stmts);
                lowered_elts.push(lowered.expr);
            }
            let expr = Expr::Tuple(ast::ExprTuple {
                elts: lowered_elts,
                ctx,
                range,
                node_index,
                parenthesized,
            });
            if stmts.is_empty() && !modified {
                LoweredExpr::unmodified(expr)
            } else {
                LoweredExpr::modified(expr, stmts)
            }
        }
        Expr::List(list) if matches!(list.ctx, ast::ExprContext::Load) => {
            let ast::ExprList { elts, .. } = list;
            let mut stmts = Vec::new();
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
                        stmts.extend(lowered.stmts);
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
                        stmts.extend(lowered.stmts);
                        lowered_elts.push(lowered.expr);
                    }
                }
            }
            let tuple = make_tuple_splat(lowered_elts);
            let expr = py_expr!("__dp__.list({tuple:expr})", tuple = tuple,);
            if stmts.is_empty() && !modified {
                LoweredExpr::unmodified(expr)
            } else {
                LoweredExpr::modified(expr, stmts)
            }
        }
        Expr::Set(ast::ExprSet { elts, .. }) => {
            let mut stmts = Vec::new();
            let mut modified = false;
            let mut lowered_elts = Vec::new();
            for elt in elts {
                let lowered = lower_expr(context, elt);
                modified |= lowered.modified;
                stmts.extend(lowered.stmts);
                lowered_elts.push(lowered.expr);
            }
            let tuple = make_tuple(lowered_elts);
            let expr = py_expr!("__dp__.set({tuple:expr})", tuple = tuple,);
            if stmts.is_empty() && !modified {
                LoweredExpr::unmodified(expr)
            } else {
                LoweredExpr::modified(expr, stmts)
            }
        }
        Expr::Dict(ast::ExprDict { items, .. }) => {
            let mut segments: Vec<Expr> = Vec::new();

            let mut keyed_pairs = Vec::new();
            let mut stmts = Vec::new();
            let mut modified = false;
            for item in items.into_iter() {
                match item {
                    ast::DictItem {
                        key: Some(key),
                        value,
                    } => {
                        let lowered_key = lower_expr(context, key);
                        modified |= lowered_key.modified;
                        stmts.extend(lowered_key.stmts);
                        let lowered_value = lower_expr(context, value);
                        modified |= lowered_value.modified;
                        stmts.extend(lowered_value.stmts);
                        keyed_pairs.push(py_expr!(
                            "({key:expr}, {value:expr})",
                            key = lowered_key.expr,
                            value = lowered_value.expr,
                        ));
                    }
                    ast::DictItem { key: None, value } => {
                        let lowered_value = lower_expr(context, value);
                        modified |= lowered_value.modified;
                        stmts.extend(lowered_value.stmts);
                        if !keyed_pairs.is_empty() {
                            let tuple = make_tuple(take(&mut keyed_pairs));
                            segments.push(py_expr!("__dp__.dict({tuple:expr})", tuple = tuple));
                        }
                        segments.push(py_expr!(
                            "__dp__.dict({mapping:expr})",
                            mapping = lowered_value.expr
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
            if stmts.is_empty() && !modified {
                LoweredExpr::unmodified(expr)
            } else {
                LoweredExpr::modified(expr, stmts)
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
            LoweredExpr::unmodified(make_unaryop(func_name, *operand))
        }
        Expr::Subscript(ast::ExprSubscript {
            value, slice, ctx, ..
        }) if matches!(ctx, ast::ExprContext::Load) => {
            let value_lowered = lower_expr(context, *value);
            let slice_lowered = lower_expr(context, *slice);
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

fn string_literal_needs_surrogate_decode(context: &Context, value: &ast::StringLiteralValue) -> bool {
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

fn is_attribute_chain(expr: &Expr) -> bool {
    match expr {
        Expr::Name(_) => true,
        Expr::Attribute(ast::ExprAttribute { value, .. }) => is_attribute_chain(value.as_ref()),
        _ => false,
    }
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
