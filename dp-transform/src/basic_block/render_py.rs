use super::bb_ir::{BbBlock, BbFunction, BbFunctionKind, BbTerm};
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

fn parse_function_skeleton(
    name: &str,
    is_async: bool,
    params: &[String],
) -> Option<ast::StmtFunctionDef> {
    let params = params.join(", ");
    let header = if is_async {
        format!("async def {name}({params}):\n    pass\n")
    } else {
        format!("def {name}({params}):\n    pass\n")
    };
    let mut parsed = parse_module(&header).ok()?.into_syntax().body.body;
    let stmt = *parsed.remove(0);
    match stmt {
        Stmt::FunctionDef(func) => Some(func),
        _ => None,
    }
}

fn make_take_params_stmt(params: &[String]) -> Option<Stmt> {
    if params.is_empty() {
        return None;
    }
    let targets = params.join(", ");
    let values = params
        .iter()
        .map(|name| format!("{name}.take()"))
        .collect::<Vec<_>>()
        .join(", ");
    let source = format!("{targets} = {values}");
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

fn resume_start_label(kind: &BbFunctionKind) -> Option<&str> {
    match kind {
        BbFunctionKind::Generator { resume_label, .. }
        | BbFunctionKind::AsyncGenerator { resume_label, .. } => Some(resume_label.as_str()),
        BbFunctionKind::Function | BbFunctionKind::Coroutine => None,
    }
}

fn is_resume_block_label(kind: &BbFunctionKind, label: &str) -> bool {
    let Some(start) = resume_start_label(kind) else {
        return false;
    };
    label == start || label.contains("_resume_")
}

fn generator_flavor_for_kind(kind: &BbFunctionKind) -> GeneratorFlavor {
    match kind {
        BbFunctionKind::Function | BbFunctionKind::Coroutine => GeneratorFlavor::None,
        BbFunctionKind::Generator { .. } => GeneratorFlavor::Sync,
        BbFunctionKind::AsyncGenerator { .. } => GeneratorFlavor::Async,
    }
}

fn expr_uses_resume_exc(expr: &Expr) -> bool {
    crate::ruff_ast_to_string(expr).contains("_dp_resume_exc")
}

fn term_uses_resume_exc(term: &BbTerm) -> bool {
    match term {
        BbTerm::Jump(_) | BbTerm::TryJump { .. } => false,
        BbTerm::BrIf { test, .. } => expr_uses_resume_exc(test),
        BbTerm::Raise { exc, cause } => {
            exc.as_ref().is_some_and(expr_uses_resume_exc)
                || cause.as_ref().is_some_and(expr_uses_resume_exc)
        }
        BbTerm::Yield { value, .. } | BbTerm::Ret(value) => {
            value.as_ref().is_some_and(expr_uses_resume_exc)
        }
    }
}

fn block_uses_resume_exc(block: &BbBlock) -> bool {
    crate::ruff_ast_to_string(&block.ops).contains("_dp_resume_exc")
        || term_uses_resume_exc(&block.term)
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

fn tuple_expr_for_target_params(names: &[String], flavor: GeneratorFlavor) -> Option<Expr> {
    if matches!(flavor, GeneratorFlavor::None) {
        let mut exprs = Vec::with_capacity(names.len());
        for name in names {
            if name.starts_with("_dp_try_exc_") {
                exprs.push(py_expr!(
                    "locals().get({name:literal}, __dp__.DELETED)",
                    name = name.as_str(),
                ));
            } else {
                exprs.push(name_expr(name.as_str())?);
            }
        }
        return Some(make_tuple(exprs));
    }
    let mut exprs = Vec::with_capacity(names.len());
    for name in names {
        if matches!(
            name.as_str(),
            "_dp_self" | "_dp_send_value" | "_dp_resume_exc"
        ) {
            exprs.push(name_expr(name.as_str())?);
        } else {
            exprs.push(py_expr!(
                "locals().get({name:literal}, __dp_load_local_raw(_dp_self, {name:literal}))",
                name = name.as_str(),
            ));
        }
    }
    Some(make_tuple(exprs))
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
    let resume_pc_by_label: HashMap<String, usize> = match &bb_function.kind {
        BbFunctionKind::Generator { resume_pcs, .. }
        | BbFunctionKind::AsyncGenerator { resume_pcs, .. } => resume_pcs.iter().cloned().collect(),
        BbFunctionKind::Function | BbFunctionKind::Coroutine => HashMap::new(),
    };
    let is_async = is_async_block_function(&bb_function.kind);
    let generator_flavor = generator_flavor_for_kind(&bb_function.kind);
    let mut block_defs = Vec::new();
    for block in &bb_function.blocks {
        let mut block_fn = parse_function_skeleton(block.label.as_str(), is_async, &block.params)?;
        let mut block_body = Vec::new();
        if let Some(take_stmt) = make_take_params_stmt(&block.params) {
            block_body.push(take_stmt);
        }
        let is_resume_block = is_resume_block_label(&bb_function.kind, block.label.as_str());
        if matches!(
            bb_function.kind,
            BbFunctionKind::Generator { .. } | BbFunctionKind::AsyncGenerator { .. }
        ) && is_resume_block
        {
            if !block_uses_resume_exc(block) {
                block_body.push(py_stmt!(
                    "if _dp_resume_exc is not None:\n    return __dp__.raise_(_dp_resume_exc)"
                ));
            }
            if resume_start_label(&bb_function.kind) == Some(block.label.as_str()) {
                let msg = if matches!(bb_function.kind, BbFunctionKind::AsyncGenerator { .. }) {
                    "can't send non-None value to a just-started async generator"
                } else {
                    "can't send non-None value to a just-started generator"
                };
                block_body.push(py_stmt!(
                    "if _dp_resume_exc is None and _dp_send_value is not None:\n    return __dp__.raise_(TypeError({msg:literal}))",
                    msg = msg,
                ));
            }
        }
        block_body.extend(block.ops.clone());
        let body_terminates = matches!(
            block_body.last(),
            Some(Stmt::Return(_)) | Some(Stmt::Raise(_))
        );
        if !body_terminates {
            block_body.extend(terminator_stmts(
                &block.term,
                &block_params,
                &resume_pc_by_label,
                generator_flavor,
            )?);
        }
        block_fn.body = stmt_body_from_stmts(block_body);
        block_defs.push(Stmt::FunctionDef(block_fn));
    }
    for block in &bb_function.blocks {
        let Some(exc_target_label) = block.exc_target_label.as_ref() else {
            continue;
        };
        block_defs.push(py_stmt!(
            "{source:id}._dp_exc_target = {target:id}",
            source = block.label.as_str(),
            target = exc_target_label.as_str(),
        ));
        if let Some(exc_name) = block.exc_name.as_ref() {
            block_defs.push(py_stmt!(
                "{source:id}._dp_exc_name = {exc_name:literal}",
                source = block.label.as_str(),
                exc_name = exc_name.as_str(),
            ));
        }
    }
    Some(block_defs)
}

fn terminator_stmts(
    terminator: &BbTerm,
    block_params: &HashMap<String, Vec<String>>,
    resume_pc_by_label: &HashMap<String, usize>,
    generator_flavor: GeneratorFlavor,
) -> Option<Vec<Stmt>> {
    match terminator {
        BbTerm::Jump(target) => {
            let target_expr = name_expr(target.as_str())?;
            let args = tuple_expr_for_target_params(
                block_params
                    .get(target.as_str())
                    .map(|names| names.as_slice())
                    .unwrap_or(&[]),
                generator_flavor,
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
            let then_args = tuple_expr_for_target_params(
                block_params
                    .get(then_label.as_str())
                    .map(|names| names.as_slice())
                    .unwrap_or(&[]),
                generator_flavor,
            )?;
            let else_args = tuple_expr_for_target_params(
                block_params
                    .get(else_label.as_str())
                    .map(|names| names.as_slice())
                    .unwrap_or(&[]),
                generator_flavor,
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
        BbTerm::TryJump { .. } => {
            panic!("internal error: BbTerm::TryJump must be lowered before Python rendering")
        }
        BbTerm::Yield {
            value,
            resume_label,
        } => {
            let next_state_names = block_params
                .get(resume_label.as_str())
                .map(|names| {
                    names
                        .iter()
                        .filter(|name| {
                            name.as_str() != "_dp_self"
                                && name.as_str() != "_dp_send_value"
                                && name.as_str() != "_dp_resume_exc"
                        })
                        .cloned()
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            let next_pc = resume_pc_by_label.get(resume_label.as_str()).copied()?;
            let yielded_value = value.clone().unwrap_or_else(|| py_expr!("None"));
            match generator_flavor {
                GeneratorFlavor::None => {
                    panic!("internal error: Terminator::Yield emitted for non-generator lowering")
                }
                GeneratorFlavor::Sync | GeneratorFlavor::Async => {
                    let mut stmts = vec![py_stmt!(
                        "_dp_self._pc = {next_pc:literal}",
                        next_pc = next_pc as i64,
                    )];
                    for name in next_state_names {
                        stmts.push(py_stmt!(
                            "__dp_store_local(_dp_self, {name:literal}, {value:id})",
                            name = name.as_str(),
                            value = name.as_str(),
                        ));
                    }
                    stmts.push(py_stmt!(
                        "return __dp__.ret({value:expr})",
                        value = yielded_value,
                    ));
                    Some(stmts)
                }
            }
        }
        BbTerm::Ret(value) => {
            let ret_value = value.clone().unwrap_or_else(|| py_expr!("None"));
            match generator_flavor {
                GeneratorFlavor::None => Some(vec![py_stmt!(
                    "return __dp__.ret({value:expr})",
                    value = ret_value,
                )]),
                GeneratorFlavor::Sync => Some(vec![
                    py_stmt!("_dp_self._pc = __dp__._GEN_PC_DONE"),
                    py_stmt!(
                        "return __dp__.raise_(StopIteration({value:expr}))",
                        value = ret_value,
                    ),
                ]),
                GeneratorFlavor::Async => Some(vec![
                    py_stmt!("_dp_self._pc = __dp__._GEN_PC_DONE"),
                    py_stmt!("return __dp__.raise_(StopAsyncIteration())"),
                ]),
            }
        }
    }
}
