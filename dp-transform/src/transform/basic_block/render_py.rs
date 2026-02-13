use crate::bb_ir::{BbFunction, BbFunctionKind, BbTerm};
use crate::transform::rewrite_expr::make_tuple;
use crate::{py_expr, py_stmt};
use ruff_python_ast::{self as ast, Expr, Stmt, StmtBody};
use ruff_python_parser::{parse_expression, parse_module};
use ruff_text_size::TextRange;
use std::collections::HashMap;

#[derive(Copy, Clone, Eq, PartialEq)]
enum GeneratorFlavor {
    None,
    Sync,
    Async,
}

fn parse_function_skeleton(name: &str, is_async: bool) -> Option<ast::StmtFunctionDef> {
    let header = if is_async {
        format!("async def {name}(_dp_args_ptr):\n    pass\n")
    } else {
        format!("def {name}(_dp_args_ptr):\n    pass\n")
    };
    let mut parsed = parse_module(&header).ok()?.into_syntax().body.body;
    let stmt = *parsed.remove(0);
    match stmt {
        Stmt::FunctionDef(func) => Some(func),
        _ => None,
    }
}

fn make_take_args_stmt(args: &[String]) -> Option<Stmt> {
    if args.is_empty() {
        return Some(py_stmt!("__dp__.take_args(_dp_args_ptr)"));
    }
    if args.len() == 1 {
        return Some(py_stmt!(
            "{name:id} = __dp__.take_arg1(_dp_args_ptr)",
            name = args[0].as_str(),
        ));
    }
    let targets = args.join(", ");
    let source = format!("{targets} = __dp__.take_args(_dp_args_ptr)");
    let mut module = parse_module(&source).ok()?.into_syntax().body.body;
    if module.len() != 1 {
        return None;
    }
    Some(*module.remove(0))
}

fn is_async_block_function(kind: &BbFunctionKind) -> bool {
    matches!(
        kind,
        BbFunctionKind::Coroutine | BbFunctionKind::AsyncGenerator { .. }
    )
}

fn generator_flavor_for_kind(kind: &BbFunctionKind) -> GeneratorFlavor {
    match kind {
        BbFunctionKind::Function | BbFunctionKind::Coroutine => GeneratorFlavor::None,
        BbFunctionKind::Generator { .. } => GeneratorFlavor::Sync,
        BbFunctionKind::AsyncGenerator { .. } => GeneratorFlavor::Async,
    }
}

fn name_expr(name: &str) -> Option<Expr> {
    parse_expression(name)
        .ok()
        .map(|expr| *expr.into_syntax().body)
}

fn tuple_expr_from_names(names: &[String]) -> Option<Expr> {
    let src = match names.len() {
        0 => "()".to_string(),
        1 => format!("({},)", names[0]),
        _ => format!("({})", names.join(", ")),
    };
    parse_expression(&src)
        .ok()
        .map(|expr| *expr.into_syntax().body)
}

fn stmt_body_from_stmts(stmts: Vec<Stmt>) -> StmtBody {
    StmtBody {
        body: stmts.into_iter().map(Box::new).collect(),
        range: TextRange::default(),
        node_index: ast::AtomicNodeIndex::default(),
    }
}

fn raise_stmt_from_name(name: &str) -> ast::StmtRaise {
    match py_stmt!("raise {exc:id}", exc = name) {
        Stmt::Raise(raise_stmt) => raise_stmt,
        _ => unreachable!("expected raise statement"),
    }
}

pub(super) fn render_block_defs_from_bb(bb_function: &BbFunction) -> Option<Vec<Stmt>> {
    let block_params: HashMap<String, Vec<String>> = bb_function
        .blocks
        .iter()
        .map(|block| (block.label.clone(), block.params.clone()))
        .collect();
    let block_pc_by_label: HashMap<String, usize> = bb_function
        .blocks
        .iter()
        .enumerate()
        .map(|(idx, block)| (block.label.clone(), idx))
        .collect();
    let is_async = is_async_block_function(&bb_function.kind);
    let generator_flavor = generator_flavor_for_kind(&bb_function.kind);
    let mut block_defs = Vec::new();
    for block in &bb_function.blocks {
        let mut block_fn = parse_function_skeleton(block.label.as_str(), is_async)?;
        let mut block_body = block.ops.clone();
        if !block.params.is_empty() {
            block_body.insert(0, make_take_args_stmt(&block.params)?);
        }
        block_body.extend(terminator_stmts(
            &block.term,
            &block_params,
            &block_pc_by_label,
            is_async,
            generator_flavor,
        )?);
        block_fn.body = stmt_body_from_stmts(block_body);
        block_defs.push(Stmt::FunctionDef(block_fn));
    }
    Some(block_defs)
}

fn terminator_stmts(
    terminator: &BbTerm,
    block_params: &HashMap<String, Vec<String>>,
    block_pc_by_label: &HashMap<String, usize>,
    is_async: bool,
    generator_flavor: GeneratorFlavor,
) -> Option<Vec<Stmt>> {
    match terminator {
        BbTerm::Jump(target) => {
            let target_expr = name_expr(target.as_str())?;
            let args = tuple_expr_from_names(
                block_params
                    .get(target.as_str())
                    .map(|names| names.as_slice())
                    .unwrap_or(&[]),
            )?;
            Some(vec![py_stmt!(
                "return __dp__.jump({target:expr}, {args:expr})",
                target = target_expr,
                args = args,
            )])
        }
        BbTerm::BrIf {
            test,
            then_label,
            else_label,
        } => {
            let then_target_expr = name_expr(then_label.as_str())?;
            let else_target_expr = name_expr(else_label.as_str())?;
            let then_args = tuple_expr_from_names(
                block_params
                    .get(then_label.as_str())
                    .map(|names| names.as_slice())
                    .unwrap_or(&[]),
            )?;
            let else_args = tuple_expr_from_names(
                block_params
                    .get(else_label.as_str())
                    .map(|names| names.as_slice())
                    .unwrap_or(&[]),
            )?;
            Some(vec![py_stmt!(
                "return __dp__.brif({test:expr}, {then_target:expr}, {then_args:expr}, {else_target:expr}, {else_args:expr})",
                test = test.clone(),
                then_target = then_target_expr,
                then_args = then_args,
                else_target = else_target_expr,
                else_args = else_args,
            )])
        }
        BbTerm::Raise { exc, cause } => {
            if cause.is_none() {
                if let Some(exc) = exc.as_ref() {
                    return Some(vec![py_stmt!(
                        "return __dp__.raise_({exc:expr})",
                        exc = exc.clone(),
                    )]);
                }
            }
            let mut raise_stmt = raise_stmt_from_name("None");
            raise_stmt.exc = exc.clone().map(Box::new);
            raise_stmt.cause = cause.clone().map(Box::new);
            Some(vec![Stmt::Raise(raise_stmt)])
        }
        BbTerm::TryJump {
            body_label,
            except_label,
            body_region_labels,
            except_region_labels,
            finally_label,
            finally_region_labels,
            finally_fallthrough_label,
        } => {
            let body_target_expr = name_expr(body_label.as_str())?;
            let body_args = tuple_expr_from_names(
                block_params
                    .get(body_label.as_str())
                    .map(|names| names.as_slice())
                    .unwrap_or(&[]),
            )?;
            let except_target_expr = name_expr(except_label.as_str())?;
            let except_args = tuple_expr_from_names(
                block_params
                    .get(except_label.as_str())
                    .map(|names| names.as_slice())
                    .unwrap_or(&[]),
            )?;
            let body_region_targets = make_tuple(
                body_region_labels
                    .iter()
                    .map(|label| name_expr(label.as_str()))
                    .collect::<Option<Vec<_>>>()?,
            );
            let except_region_targets = make_tuple(
                except_region_labels
                    .iter()
                    .map(|label| name_expr(label.as_str()))
                    .collect::<Option<Vec<_>>>()?,
            );
            let finally_target_expr = finally_label
                .as_ref()
                .and_then(|label| name_expr(label.as_str()))
                .unwrap_or_else(|| py_expr!("None"));
            let finally_args = if let Some(finally_label_name) = finally_label.as_ref() {
                tuple_expr_from_names(
                    block_params
                        .get(finally_label_name.as_str())
                        .map(|names| names.as_slice())
                        .unwrap_or(&[]),
                )?
            } else {
                tuple_expr_from_names(&[])?
            };
            let finally_region_targets = make_tuple(
                finally_region_labels
                    .iter()
                    .map(|label| name_expr(label.as_str()))
                    .collect::<Option<Vec<_>>>()?,
            );
            let finally_fallthrough_target_expr = finally_fallthrough_label
                .as_ref()
                .and_then(|label| {
                    if block_params.contains_key(label.as_str()) {
                        name_expr(label.as_str())
                    } else {
                        None
                    }
                })
                .unwrap_or_else(|| py_expr!("None"));
            if is_async {
                Some(vec![py_stmt!(
                    "return await __dp__.try_jump_term_async({body_target:expr}, {body_args:expr}, {body_region_targets:expr}, {except_target:expr}, {except_args:expr}, {except_region_targets:expr}, {finally_target:expr}, {finally_args:expr}, {finally_region_targets:expr}, {finally_fallthrough_target:expr})",
                    body_target = body_target_expr,
                    body_args = body_args,
                    body_region_targets = body_region_targets,
                    except_target = except_target_expr,
                    except_args = except_args,
                    except_region_targets = except_region_targets,
                    finally_target = finally_target_expr,
                    finally_args = finally_args,
                    finally_region_targets = finally_region_targets,
                    finally_fallthrough_target = finally_fallthrough_target_expr,
                )])
            } else {
                Some(vec![py_stmt!(
                    "return __dp__.try_jump_term({body_target:expr}, {body_args:expr}, {body_region_targets:expr}, {except_target:expr}, {except_args:expr}, {except_region_targets:expr}, {finally_target:expr}, {finally_args:expr}, {finally_region_targets:expr}, {finally_fallthrough_target:expr})",
                    body_target = body_target_expr,
                    body_args = body_args,
                    body_region_targets = body_region_targets,
                    except_target = except_target_expr,
                    except_args = except_args,
                    except_region_targets = except_region_targets,
                    finally_target = finally_target_expr,
                    finally_args = finally_args,
                    finally_region_targets = finally_region_targets,
                    finally_fallthrough_target = finally_fallthrough_target_expr,
                )])
            }
        }
        BbTerm::Yield {
            value,
            resume_label,
        } => {
            let next_state_args = tuple_expr_from_names(
                block_params
                    .get(resume_label.as_str())
                    .map(|names| {
                        names
                            .iter()
                            .filter(|name| {
                                name.as_str() != "_dp_send_value"
                                    && name.as_str() != "_dp_resume_exc"
                            })
                            .cloned()
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default()
                    .as_slice(),
            )?;
            let next_pc = block_pc_by_label.get(resume_label.as_str()).copied()?;
            let yielded_value = value.clone().unwrap_or_else(|| py_expr!("None"));
            match generator_flavor {
                GeneratorFlavor::None => {
                    panic!("internal error: Terminator::Yield emitted for non-generator lowering")
                }
                GeneratorFlavor::Sync | GeneratorFlavor::Async => Some(vec![
                    py_stmt!(
                        "__dp__.setitem(_dp_state, \"pc\", {next_pc:literal})",
                        // Generator runtime reserves only synthetic pc=0 (unstarted).
                        // done/invalid are emitted as explicit lowered blocks.
                        next_pc = (next_pc as i64) + 1,
                    ),
                    py_stmt!(
                        "__dp__.setitem(_dp_state, \"args\", {next_state_args:expr})",
                        next_state_args = next_state_args,
                    ),
                    py_stmt!("return __dp__.ret({value:expr})", value = yielded_value,),
                ]),
            }
        }
        BbTerm::Ret(value) => {
            let ret_value = value.clone().unwrap_or_else(|| py_expr!("None"));
            match generator_flavor {
                GeneratorFlavor::None => Some(vec![py_stmt!(
                    "return __dp__.ret({value:expr})",
                    value = ret_value,
                )]),
                GeneratorFlavor::Sync | GeneratorFlavor::Async => Some(vec![
                    py_stmt!(
                        "__dp__.setitem(_dp_state, \"pc\", __dp__._GEN_PC_DONE)"
                    ),
                    py_stmt!(
                        "return __dp__.ret({value:expr})",
                        value = ret_value,
                    ),
                ]),
            }
        }
    }
}
