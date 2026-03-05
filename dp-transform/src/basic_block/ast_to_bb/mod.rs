use super::bb_ir::{
    BbBlock, BbExpr, BbFunction, BbFunctionKind, BbModule, BbOp, BbTerm, BindingTarget,
};
use crate::template::{empty_body, into_body};
use crate::transform::context::Context;
use crate::transform::driver::SimplifyExprPass;
use crate::transform::rewrite_import;
use crate::transform::scope::{
    analyze_module_scope, cell_name, is_internal_symbol, BindingKind, BindingUse, Scope, ScopeKind,
};
use crate::transform::{
    ast_rewrite::{rewrite_with_pass, ExprRewritePass, Rewrite, StmtRewritePass},
    rewrite_stmt,
};
use crate::transformer::{walk_expr, walk_stmt, Transformer};
use crate::{py_expr, py_stmt};
use ruff_python_ast::{self as ast, name::Name, Expr, NodeIndex, Stmt, StmtBody};
use ruff_python_codegen::{Generator, Indentation};
use ruff_python_parser::parse_expression;
use ruff_source_file::LineEnding;
use ruff_text_size::TextRange;
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;

mod await_lower;
mod annotation_helpers;
mod bound_names;
mod dataflow;
mod deleted_names;
mod exception_flow;
mod metadata;
mod naming;
mod pre_lower;
mod support;

use await_lower::{coroutine_generator_marker_stmt, lower_coroutine_awaits_to_yield_from};
use annotation_helpers::{
    annotation_helper_exec_binding_stmt, collect_capture_names, ensure_capture_default_params,
    ensure_dp_default_param, is_annotation_helper_name, render_stmt_source,
    rewrite_annotation_helper_defs_as_exec_calls, should_keep_non_lowered_for_annotationlib,
};
use bound_names::{collect_bound_names, collect_explicit_global_or_nonlocal_names};
use dataflow::{
    analyze_block_use_def, build_extra_successors, compute_block_params,
    ensure_try_exception_params,
};
use deleted_names::{collect_deleted_names, rewrite_delete_to_deleted_sentinel, rewrite_deleted_name_loads};
use exception_flow::{
    compute_exception_edge_by_label, contains_return_stmt_in_body,
    contains_return_stmt_in_handlers,
    rewrite_region_returns_to_finally,
};
use metadata::{collect_function_identity_private, display_name_for_function, FunctionIdentity};
use naming::{
    apply_label_rename, fold_constant_brif, fold_jumps_to_trivial_none_return,
    original_function_name, prune_unreachable_blocks, relabel_blocks, sanitize_ident,
};
use pre_lower::{is_simple_index_target, AnnotationHelperForLoweringPass};
pub use pre_lower::{BBSimplifyStmtPass, FunctionIdentityByNode};
use support::{
    has_await_in_stmts, has_dead_stmt_suffixes, has_yield_exprs_in_stmts, is_module_init_temp_name,
    prune_dead_stmt_suffixes, BasicBlockSupportChecker,
};

pub fn collect_function_identity_by_node(
    module: &mut StmtBody,
    module_scope: Arc<Scope>,
) -> FunctionIdentityByNode {
    collect_function_identity_private(module, module_scope)
        .into_iter()
        .map(|(node, identity)| {
            (
                node,
                (
                    identity.bind_name,
                    identity.display_name,
                    identity.qualname,
                    identity.binding_target,
                ),
            )
        })
        .collect()
}

pub fn rewrite_with_function_identity_and_collect_ir(
    context: &Context,
    module: &mut StmtBody,
    function_identity_by_node: FunctionIdentityByNode,
) -> BbModule {
    rewrite_internal(context, module, Some(function_identity_by_node))
}

fn rewrite_internal(
    context: &Context,
    module: &mut StmtBody,
    function_identity_by_node: Option<FunctionIdentityByNode>,
) -> BbModule {
    let function_identity_by_node =
        if let Some(function_identity_by_node) = function_identity_by_node {
            function_identity_by_node
                .into_iter()
                .map(
                    |(node, (bind_name, display_name, qualname, binding_target))| {
                        (
                            node,
                            FunctionIdentity {
                                bind_name,
                                display_name,
                                qualname,
                                binding_target,
                            },
                        )
                    },
                )
                .collect()
        } else {
            let scope = analyze_module_scope(module);
            collect_function_identity_private(module, scope)
        };

    let mut rewriter = BasicBlockRewriter {
        context,
        function_identity_by_node,
        next_block_id: 0,
        used_label_prefixes: HashMap::new(),
        function_stack: Vec::new(),
        function_cell_bindings_stack: Vec::new(),
        module_init_hoisted_blocks: Vec::new(),
        lowered_functions_ir: Vec::new(),
        module_init_function: Some("_dp_module_init".to_string()),
    };
    rewriter.visit_body(module);
    // BB lowering hoists nested lowered block functions into module-init and
    // leaves placeholder `pass` statements at original def sites. Strip them.
    crate::transform::simplify::strip_generated_passes(context, module);
    BbModule {
        functions: rewriter.lowered_functions_ir,
        module_init: Some("_dp_module_init".to_string()),
    }
}

#[derive(Clone)]
enum Terminator {
    Jump(String),
    BrIf {
        test: Expr,
        then_label: String,
        else_label: String,
    },
    BrTable {
        index: Expr,
        targets: Vec<String>,
        default_label: String,
    },
    Raise(ast::StmtRaise),
    TryJump {
        body_label: String,
        except_label: String,
        except_exc_name: Option<String>,
        body_region_labels: Vec<String>,
        except_region_labels: Vec<String>,
        finally_label: Option<String>,
        finally_exc_name: Option<String>,
        finally_region_labels: Vec<String>,
        finally_fallthrough_label: Option<String>,
    },
    Yield {
        value: Option<Expr>,
        resume_label: String,
    },
    Ret(Option<Expr>),
}

impl Terminator {
    fn references_label(&self, label: &str) -> bool {
        match self {
            Terminator::Jump(target) => target == label,
            Terminator::BrIf {
                then_label,
                else_label,
                ..
            } => then_label == label || else_label == label,
            Terminator::BrTable {
                targets,
                default_label,
                ..
            } => default_label == label || targets.iter().any(|target| target == label),
            Terminator::Raise(_) => false,
            Terminator::TryJump {
                body_label,
                except_label,
                finally_label,
                finally_fallthrough_label,
                ..
            } => {
                body_label == label
                    || except_label == label
                    || finally_label.as_deref() == Some(label)
                    || finally_fallthrough_label.as_deref() == Some(label)
            }
            Terminator::Yield { resume_label, .. } => resume_label == label,
            Terminator::Ret(_) => false,
        }
    }
}

#[derive(Clone)]
struct Block {
    label: String,
    body: Vec<Stmt>,
    terminator: Terminator,
}

impl Block {
    fn successors(&self) -> Vec<String> {
        match &self.terminator {
            Terminator::Jump(target) => vec![target.clone()],
            Terminator::BrIf {
                then_label,
                else_label,
                ..
            } => vec![then_label.clone(), else_label.clone()],
            Terminator::BrTable {
                targets,
                default_label,
                ..
            } => {
                let mut out = targets.clone();
                out.push(default_label.clone());
                out
            }
            Terminator::Raise(_) => Vec::new(),
            Terminator::TryJump {
                body_label,
                except_label,
                finally_label,
                finally_fallthrough_label,
                ..
            } => {
                let mut out = vec![body_label.clone(), except_label.clone()];
                if let Some(finally_label) = finally_label {
                    out.push(finally_label.clone());
                }
                if let Some(finally_fallthrough_label) = finally_fallthrough_label {
                    out.push(finally_fallthrough_label.clone());
                }
                out
            }
            Terminator::Yield { resume_label, .. } => vec![resume_label.clone()],
            Terminator::Ret(_) => Vec::new(),
        }
    }
}

struct BasicBlockRewriter<'a> {
    context: &'a Context,
    function_identity_by_node: HashMap<NodeIndex, FunctionIdentity>,
    next_block_id: usize,
    used_label_prefixes: HashMap<String, usize>,
    function_stack: Vec<String>,
    function_cell_bindings_stack: Vec<HashSet<String>>,
    module_init_hoisted_blocks: Vec<Vec<Stmt>>,
    lowered_functions_ir: Vec<BbFunction>,
    module_init_function: Option<String>,
}

struct LoopContext {
    continue_label: String,
    break_label: String,
}

impl BasicBlockRewriter<'_> {
    fn next_temp(&mut self, prefix: &str) -> String {
        let current = self.next_block_id;
        self.next_block_id += 1;
        format!("_dp_{prefix}_{current}")
    }

    fn emit_target_assignments(&mut self, target: &Expr, value: Expr, out: &mut Vec<Stmt>) {
        match target {
            Expr::Tuple(tuple) => self.emit_sequence_target_assignments(&tuple.elts, value, out),
            Expr::List(list) => self.emit_sequence_target_assignments(&list.elts, value, out),
            Expr::Subscript(ast::ExprSubscript {
                value: obj, slice, ..
            }) => out.push(py_stmt!(
                "__dp_setitem({obj:expr}, {slice:expr}, {value:expr})",
                obj = if let Expr::Name(name) = obj.as_ref() {
                    py_expr!(
                        "__dp_load_deleted_name({name:literal}, {value:expr})",
                        name = name.id.as_str(),
                        value = *obj.clone(),
                    )
                } else {
                    *obj.clone()
                },
                slice = *slice.clone(),
                value = value,
            )),
            Expr::Attribute(ast::ExprAttribute {
                value: obj, attr, ..
            }) => out.push(py_stmt!(
                "__dp_setattr({obj:expr}, {name:literal}, {value:expr})",
                obj = if let Expr::Name(name) = obj.as_ref() {
                    py_expr!(
                        "__dp_load_deleted_name({name:literal}, {value:expr})",
                        name = name.id.as_str(),
                        value = *obj.clone(),
                    )
                } else {
                    *obj.clone()
                },
                name = attr.as_str(),
                value = value,
            )),
            Expr::Name(_) => out.push(py_stmt!(
                "{target:expr} = {value:expr}",
                target = target.clone(),
                value = value,
            )),
            other => {
                panic!("unsupported assignment target in BB direct emit: {other:?}");
            }
        }
    }

    fn emit_sequence_target_assignments(
        &mut self,
        elts: &[Expr],
        value: Expr,
        out: &mut Vec<Stmt>,
    ) {
        let mut starred_index = None;
        for (idx, elt) in elts.iter().enumerate() {
            if matches!(elt, Expr::Starred(_)) {
                if starred_index.is_some() {
                    panic!("unsupported starred assignment target");
                }
                starred_index = Some(idx);
            }
        }

        if let Some(starred_index) = starred_index {
            let unpacked_name = self.next_temp("tmp");
            let mut spec_elts = Vec::new();
            for elt in elts {
                if matches!(elt, Expr::Starred(_)) {
                    spec_elts.push(py_expr!("False"));
                } else {
                    spec_elts.push(py_expr!("True"));
                }
            }
            let spec_expr = make_dp_tuple(spec_elts);
            out.push(py_stmt!(
                "{tmp:id} = __dp_unpack({value:expr}, {spec:expr})",
                tmp = unpacked_name.as_str(),
                value = value,
                spec = spec_expr,
            ));
            let unpacked_expr = py_expr!("{tmp:id}", tmp = unpacked_name.as_str());
            for (idx, elt) in elts.iter().enumerate() {
                match elt {
                    Expr::Starred(starred) if idx == starred_index => {
                        let starred_value = py_expr!(
                            "__dp_list(__dp_getitem({tmp:expr}, {idx:literal}))",
                            tmp = unpacked_expr.clone(),
                            idx = idx as i64,
                        );
                        self.emit_target_assignments(starred.value.as_ref(), starred_value, out);
                    }
                    _ => {
                        let element_value = py_expr!(
                            "__dp_getitem({tmp:expr}, {idx:literal})",
                            tmp = unpacked_expr.clone(),
                            idx = idx as i64,
                        );
                        self.emit_target_assignments(elt, element_value, out);
                    }
                }
            }
            out.push(py_stmt!("{tmp:id} = None", tmp = unpacked_name.as_str()));
            return;
        }

        for (idx, elt) in elts.iter().enumerate() {
            let element_value = py_expr!(
                "{value:expr}[{idx:literal}]",
                value = value.clone(),
                idx = idx as i64,
            );
            self.emit_target_assignments(elt, element_value, out);
        }
    }

    fn try_lower_function(&mut self, func: &ast::StmtFunctionDef) -> Option<LoweredFunction> {
        if should_keep_non_lowered_for_annotationlib(func) {
            return None;
        }
        if func.name.id.as_str().starts_with("_dp_bb_") {
            return None;
        }
        let is_generated_genexpr = func.name.id.as_str().contains("_dp_genexpr_");
        // Keep generated annotation helpers in their lexical scope. BB-lowering
        // and hoisting them out of class/module init can break name resolution
        // for class-local symbols (for example, `T` in `value: T`).
        if is_annotation_helper_name(func.name.id.as_str()) {
            return None;
        }
        let (_, lowered_input_body) = split_docstring(&func.body);
        let lowered_input_body = flatten_stmt_boxes(&lowered_input_body);
        let lowered_input_body =
            if should_strip_nonlocal_for_bb(func.name.id.as_str()) || is_generated_genexpr {
                strip_nonlocal_directives(lowered_input_body)
            } else {
                lowered_input_body
            };
        let param_names = collect_parameter_names(&func.parameters);
        let has_yield_original = has_yield_exprs_in_stmts(&lowered_input_body);
        let mut runtime_input_body = prune_dead_stmt_suffixes(&lowered_input_body);
        let original_runtime_input_body = runtime_input_body.clone();
        // Keep await->yield-from lowering in the dedicated async pass for all
        // async functions so no `await` reaches BB IR/JIT planning.
        if func.is_async {
            lower_coroutine_awaits_to_yield_from(&mut runtime_input_body);
        }
        let mut coroutine_via_generator = func.is_async && !has_yield_original;
        if coroutine_via_generator {
            if has_await_in_stmts(&runtime_input_body) {
                coroutine_via_generator = false;
                runtime_input_body = original_runtime_input_body;
            } else if !has_yield_exprs_in_stmts(&runtime_input_body) {
                runtime_input_body.insert(0, coroutine_generator_marker_stmt());
            }
        }
        let mut outer_scope_names = collect_bound_names(&runtime_input_body);
        outer_scope_names.extend(param_names.iter().cloned());
        let runtime_input_body =
            rewrite_annotation_helper_defs_as_exec_calls(runtime_input_body, &outer_scope_names);
        let mut outer_scope_names = collect_bound_names(&runtime_input_body);
        outer_scope_names.extend(param_names.iter().cloned());
        let unbound_local_names = if has_dead_stmt_suffixes(&lowered_input_body) {
            self.always_unbound_local_names(&lowered_input_body, &runtime_input_body, &param_names)
        } else {
            HashSet::new()
        };
        let deleted_names = collect_deleted_names(&runtime_input_body);
        let cell_slots = collect_cell_slots(&runtime_input_body);
        let has_yield = has_yield_exprs_in_stmts(&runtime_input_body);
        let has_await = has_await_in_stmts(&runtime_input_body);
        if func.is_async && has_await {
            return None;
        }
        if has_yield && has_await && !func.is_async {
            return None;
        }
        if !has_yield {
            let mut checker = BasicBlockSupportChecker {
                allow_await: func.is_async,
                ..Default::default()
            };
            let mut body_for_check = stmt_body_from_stmts(
                runtime_input_body
                    .iter()
                    .map(|stmt| stmt.as_ref().clone())
                    .collect(),
            );
            checker.visit_body(&mut body_for_check);
            if !checker.supported {
                return None;
            }
        }

        let end_label = self.next_label(func.name.id.as_str());
        let mut blocks = Vec::new();
        let mut entry_label = self.lower_stmt_sequence(
            func.name.id.as_str(),
            &runtime_input_body,
            end_label.clone(),
            &mut blocks,
            None,
            &cell_slots,
            &outer_scope_names,
        );
        let needs_end_block = entry_label == end_label
            || blocks
                .iter()
                .any(|block| block.terminator.references_label(end_label.as_str()));
        if needs_end_block {
            blocks.push(Block {
                label: end_label,
                body: Vec::new(),
                terminator: Terminator::Ret(None),
            });
        }
        fold_jumps_to_trivial_none_return(&mut blocks);
        fold_constant_brif(&mut blocks);
        prune_unreachable_blocks(entry_label.as_str(), &mut blocks);
        let label_prefix = self.next_label_prefix(func.name.id.as_str());
        entry_label = relabel_blocks(label_prefix.as_str(), entry_label.as_str(), &mut blocks);
        let mut done_block_label: Option<String> = None;
        let mut invalid_block_label: Option<String> = None;
        let mut generator_uncaught_label: Option<String> = None;
        let mut generator_uncaught_exc_name: Option<String> = None;
        let mut generator_uncaught_set_done_label: Option<String> = None;
        let mut generator_uncaught_raise_label: Option<String> = None;
        let mut generator_resume_entry_label: Option<String> = None;
        let mut generator_resume_order: Vec<String> = Vec::new();
        let mut generator_dispatch_only_labels: HashSet<String> = HashSet::new();
        let mut generator_throw_passthrough_labels: HashSet<String> = HashSet::new();
        let is_async_generator_runtime = func.is_async && !coroutine_via_generator;
        if has_yield {
            let done_label = format!("{label_prefix}_done");
            let invalid_label = format!("{label_prefix}_invalid");
            let uncaught_label = format!("{label_prefix}_uncaught");
            let uncaught_exc_name = self.next_temp("uncaught_exc");
            let invalid_msg = if is_async_generator_runtime {
                "invalid async generator pc: {}"
            } else {
                "invalid generator pc: {}"
            };
            let invalid_raise_stmt = match py_stmt!(
                "raise RuntimeError({msg:literal}.format(__dp_getattr(_dp_self, \"_pc\")))",
                msg = invalid_msg,
            ) {
                Stmt::Raise(stmt) => stmt,
                _ => unreachable!("expected raise statement"),
            };
            blocks.insert(
                0,
                Block {
                    label: done_label.clone(),
                    body: Vec::new(),
                    terminator: Terminator::Ret(None),
                },
            );
            blocks.insert(
                1,
                Block {
                    label: invalid_label.clone(),
                    body: Vec::new(),
                    terminator: Terminator::Raise(invalid_raise_stmt),
                },
            );
            let uncaught_raise_stmt = raise_stmt_from_name(uncaught_exc_name.as_str());
            let uncaught_helper_name = if is_async_generator_runtime {
                "raise_uncaught_async_generator_exception"
            } else {
                "raise_uncaught_generator_exception"
            };
            let uncaught_set_done_label = format!("{label_prefix}_uncaught_set_done");
            let uncaught_raise_label = format!("{label_prefix}_uncaught_raise");
            generator_uncaught_set_done_label = Some(uncaught_set_done_label.clone());
            generator_uncaught_raise_label = Some(uncaught_raise_label.clone());
            blocks.insert(
                2,
                Block {
                    label: uncaught_raise_label.clone(),
                    body: Vec::new(),
                    terminator: Terminator::Raise(uncaught_raise_stmt),
                },
            );
            blocks.insert(
                2,
                Block {
                    label: uncaught_set_done_label.clone(),
                    body: vec![
                        py_stmt!("__dp_setattr(_dp_self, \"_pc\", __dp__._GEN_PC_DONE)"),
                        py_stmt!(
                            "__dp_{helper:id}({exc:id})",
                            helper = uncaught_helper_name,
                            exc = uncaught_exc_name.as_str(),
                        ),
                    ],
                    terminator: Terminator::Jump(uncaught_raise_label.clone()),
                },
            );
            blocks.insert(
                2,
                Block {
                    label: uncaught_label.clone(),
                    body: Vec::new(),
                    terminator: Terminator::BrIf {
                        test: py_expr!(
                            "__dp_ne(__dp_getattr(_dp_self, \"_pc\"), __dp__._GEN_PC_DONE)"
                        ),
                        then_label: uncaught_set_done_label.clone(),
                        else_label: uncaught_raise_label.clone(),
                    },
                },
            );
            generator_throw_passthrough_labels.insert(uncaught_set_done_label);
            generator_throw_passthrough_labels.insert(uncaught_raise_label);
            done_block_label = Some(done_label);
            invalid_block_label = Some(invalid_label);
            generator_uncaught_label = Some(uncaught_label);
            generator_uncaught_exc_name = Some(uncaught_exc_name.clone());

            let mut resume_labels: HashSet<String> = HashSet::new();
            resume_labels.insert(entry_label.clone());
            for block in &blocks {
                if let Terminator::Yield { resume_label, .. } = &block.terminator {
                    resume_labels.insert(resume_label.clone());
                }
            }

            let mut rename: HashMap<String, String> = HashMap::new();
            let mut next_resume = 0usize;
            let mut next_internal = 0usize;
            for block in &blocks {
                if done_block_label.as_deref() == Some(block.label.as_str())
                    || invalid_block_label.as_deref() == Some(block.label.as_str())
                    || generator_uncaught_label.as_deref() == Some(block.label.as_str())
                    || generator_uncaught_set_done_label.as_deref() == Some(block.label.as_str())
                    || generator_uncaught_raise_label.as_deref() == Some(block.label.as_str())
                {
                    continue;
                }
                let new_name = if resume_labels.contains(block.label.as_str()) {
                    let name = format!("{label_prefix}_resume_{next_resume}");
                    next_resume += 1;
                    name
                } else {
                    let name = format!("{label_prefix}_internal_{next_internal}");
                    next_internal += 1;
                    name
                };
                rename.insert(block.label.clone(), new_name);
            }
            entry_label = apply_label_rename(entry_label.as_str(), &rename, &mut blocks);
            generator_resume_entry_label = Some(entry_label.clone());

            let mut resume_order = vec![entry_label.clone()];
            for block in &blocks {
                if let Terminator::Yield { resume_label, .. } = &block.terminator {
                    if !resume_order.iter().any(|label| label == resume_label) {
                        resume_order.push(resume_label.clone());
                    }
                }
            }
            generator_resume_order = resume_order.clone();

            let done_label = done_block_label
                .clone()
                .expect("generator lowering requires done block label");
            let invalid_label = invalid_block_label
                .clone()
                .expect("generator lowering requires invalid block label");

            let resume_throw_done_label = format!("{label_prefix}_dispatch_throw_done");
            let resume_throw_unstarted_label = format!("{label_prefix}_dispatch_throw_unstarted");
            generator_dispatch_only_labels.insert(resume_throw_done_label.clone());
            generator_dispatch_only_labels.insert(resume_throw_unstarted_label.clone());
            generator_throw_passthrough_labels.insert(resume_throw_done_label.clone());
            generator_throw_passthrough_labels.insert(resume_throw_unstarted_label.clone());
            let throw_resume_exc_stmt = match py_stmt!("raise _dp_resume_exc") {
                Stmt::Raise(stmt) => stmt,
                _ => unreachable!("expected raise statement"),
            };
            blocks.push(Block {
                label: resume_throw_done_label.clone(),
                body: Vec::new(),
                terminator: Terminator::Raise(throw_resume_exc_stmt.clone()),
            });
            blocks.push(Block {
                label: resume_throw_unstarted_label.clone(),
                body: Vec::new(),
                terminator: Terminator::Raise(throw_resume_exc_stmt),
            });

            let resume_send_label = format!("{label_prefix}_dispatch_send");
            let resume_throw_label = format!("{label_prefix}_dispatch_throw");
            let resume_dispatch_label = format!("{label_prefix}_dispatch");
            let resume_send_table_label = format!("{label_prefix}_dispatch_send_table");
            let resume_throw_table_label = format!("{label_prefix}_dispatch_throw_table");
            let resume_invalid_table_label = format!("{label_prefix}_dispatch_invalid");
            generator_dispatch_only_labels.insert(resume_send_label.clone());
            generator_dispatch_only_labels.insert(resume_throw_label.clone());
            generator_dispatch_only_labels.insert(resume_dispatch_label.clone());
            generator_dispatch_only_labels.insert(resume_send_table_label.clone());
            generator_dispatch_only_labels.insert(resume_throw_table_label.clone());
            generator_dispatch_only_labels.insert(resume_invalid_table_label.clone());

            let mut send_table_targets = Vec::with_capacity(resume_order.len());
            let mut throw_table_targets = Vec::with_capacity(resume_order.len());
            for (pc, resume_target) in resume_order.iter().enumerate() {
                let send_dispatch_target_label =
                    format!("{label_prefix}_dispatch_send_target_{pc}");
                generator_dispatch_only_labels.insert(send_dispatch_target_label.clone());
                blocks.push(Block {
                    label: send_dispatch_target_label.clone(),
                    body: Vec::new(),
                    terminator: Terminator::Jump(resume_target.clone()),
                });
                send_table_targets.push(send_dispatch_target_label);

                let throw_dispatch_target_label =
                    format!("{label_prefix}_dispatch_throw_target_{pc}");
                generator_dispatch_only_labels.insert(throw_dispatch_target_label.clone());
                let throw_target = if pc == 0 {
                    resume_throw_unstarted_label.clone()
                } else {
                    // Route throw() back through the canonical resume entry for
                    // this pc and let the lowered block graph branch on
                    // `_dp_resume_exc` internally. This keeps send/throw
                    // dispatch semantics aligned and avoids mismatched throw
                    // pre-dispatch tables for yield-from paths.
                    resume_target.clone()
                };
                blocks.push(Block {
                    label: throw_dispatch_target_label.clone(),
                    body: Vec::new(),
                    terminator: Terminator::Jump(throw_target),
                });
                throw_table_targets.push(throw_dispatch_target_label);
            }
            blocks.push(Block {
                label: resume_invalid_table_label.clone(),
                body: Vec::new(),
                terminator: Terminator::Jump(invalid_label.clone()),
            });

            blocks.push(Block {
                label: resume_send_table_label.clone(),
                body: Vec::new(),
                terminator: Terminator::BrTable {
                    index: py_expr!("__dp_getattr(_dp_self, \"_pc\")"),
                    targets: send_table_targets,
                    default_label: resume_invalid_table_label.clone(),
                },
            });
            blocks.push(Block {
                label: resume_throw_table_label.clone(),
                body: Vec::new(),
                terminator: Terminator::BrTable {
                    index: py_expr!("__dp_getattr(_dp_self, \"_pc\")"),
                    targets: throw_table_targets,
                    default_label: resume_invalid_table_label,
                },
            });
            blocks.push(Block {
                label: resume_send_label.clone(),
                body: Vec::new(),
                terminator: Terminator::BrIf {
                    test: py_expr!("__dp_eq(__dp_getattr(_dp_self, \"_pc\"), __dp__._GEN_PC_DONE)"),
                    then_label: done_label,
                    else_label: resume_send_table_label,
                },
            });
            blocks.push(Block {
                label: resume_throw_label.clone(),
                body: Vec::new(),
                terminator: Terminator::BrIf {
                    test: py_expr!("__dp_eq(__dp_getattr(_dp_self, \"_pc\"), __dp__._GEN_PC_DONE)"),
                    then_label: resume_throw_done_label,
                    else_label: resume_throw_table_label,
                },
            });
            blocks.push(Block {
                label: resume_dispatch_label.clone(),
                body: Vec::new(),
                terminator: Terminator::BrIf {
                    test: py_expr!("__dp_is_(_dp_resume_exc, None)"),
                    then_label: resume_send_label,
                    else_label: resume_throw_label,
                },
            });
            entry_label = resume_dispatch_label;
        }

        if !deleted_names.is_empty() {
            rewrite_deleted_name_loads(&mut blocks, &deleted_names, &unbound_local_names);
        } else if !unbound_local_names.is_empty() {
            rewrite_deleted_name_loads(&mut blocks, &HashSet::new(), &unbound_local_names);
        }

        let exception_edges = compute_exception_edge_by_label(&blocks);
        let mut exception_edges = exception_edges;
        if has_yield {
            if let (Some(uncaught_label), Some(uncaught_exc_name)) = (
                generator_uncaught_label.as_ref(),
                generator_uncaught_exc_name.as_ref(),
            ) {
                for block in &blocks {
                    let label = block.label.as_str();
                    if done_block_label.as_deref() == Some(label)
                        || invalid_block_label.as_deref() == Some(label)
                        || Some(label) == generator_uncaught_label.as_deref()
                        || generator_throw_passthrough_labels.contains(label)
                    {
                        continue;
                    }
                    exception_edges.entry(block.label.clone()).or_insert((
                        Some(uncaught_label.clone()),
                        Some(uncaught_exc_name.clone()),
                    ));
                }
            }
        }
        let state_vars = collect_state_vars(
            &param_names,
            &blocks,
            is_module_init_temp_name(func.name.id.as_str()),
        );
        let mut extra_successors = build_extra_successors(&blocks);
        for (source, (target, _)) in &exception_edges {
            let Some(target) = target.as_ref() else {
                continue;
            };
            let successors = extra_successors.entry(source.clone()).or_default();
            if !successors.iter().any(|existing| existing == target) {
                successors.push(target.clone());
            }
        }
        let mut block_params = compute_block_params(&blocks, &state_vars, &extra_successors);
        if has_yield {
            // Generator/async-generator runtime dispatch passes state through
            // block args; keep `_dp_self` threaded even when local liveness
            // for a specific block would otherwise drop it.
            //
            // `_dp_try_exc_*` carries active exception context for resumed
            // `throw()` semantics across yields; keep it threaded through
            // generator blocks even if not referenced syntactically.
            let try_exc_names = state_vars
                .iter()
                .filter(|name| name.starts_with("_dp_try_exc_"))
                .cloned()
                .collect::<Vec<_>>();
            for block in &blocks {
                let params = block_params.entry(block.label.clone()).or_default();
                params.retain(|name| {
                    name != "_dp_self" && name != "_dp_send_value" && name != "_dp_resume_exc"
                });
                params.insert(0, "_dp_self".to_string());
                params.insert(1, "_dp_send_value".to_string());
                params.insert(2, "_dp_resume_exc".to_string());
                if generator_dispatch_only_labels.contains(block.label.as_str()) {
                    params.truncate(3);
                    continue;
                }
                if block.label != entry_label {
                    for exc_name in &try_exc_names {
                        if !params.iter().any(|name| name == exc_name) {
                            params.push(exc_name.clone());
                        }
                    }
                }
            }
            if !try_exc_names.is_empty() {
                if let Some(entry_block) = blocks.iter_mut().find(|block| {
                    block.label.as_str()
                        == generator_resume_entry_label
                            .as_deref()
                            .unwrap_or(entry_label.as_str())
                }) {
                    for exc_name in try_exc_names.iter().rev() {
                        entry_block.body.insert(
                            0,
                            py_stmt!("{name:id} = __dp_DELETED", name = exc_name.as_str(),),
                        );
                    }
                }
            }
        }
        ensure_try_exception_params(&blocks, &mut block_params);
        if let (Some(uncaught_label), Some(uncaught_exc_name)) = (
            generator_uncaught_label.as_ref(),
            generator_uncaught_exc_name.as_ref(),
        ) {
            let params = block_params.entry(uncaught_label.clone()).or_default();
            params.retain(|name| name != uncaught_exc_name);
            params.push(uncaught_exc_name.clone());
            if let Some(uncaught_set_done_label) = generator_uncaught_set_done_label.as_ref() {
                let params = block_params
                    .entry(uncaught_set_done_label.clone())
                    .or_default();
                params.retain(|name| name != uncaught_exc_name);
                params.push(uncaught_exc_name.clone());
            }
            if let Some(uncaught_raise_label) = generator_uncaught_raise_label.as_ref() {
                let params = block_params
                    .entry(uncaught_raise_label.clone())
                    .or_default();
                params.retain(|name| name != uncaught_exc_name);
                params.push(uncaught_exc_name.clone());
            }
        }
        let state_entry_label = generator_resume_entry_label
            .as_deref()
            .unwrap_or(entry_label.as_str());
        let entry_params = block_params
            .get(state_entry_label)
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .filter(|name| {
                name != "_dp_self" && name != "_dp_send_value" && name != "_dp_resume_exc"
            })
            .collect::<Vec<_>>();
        let extra_state_vars: Vec<String> = entry_params
            .iter()
            .filter(|name| !param_names.iter().any(|param| param == *name))
            .cloned()
            .collect();
        let target_labels = blocks
            .iter()
            .map(|block| block.label.clone())
            .collect::<Vec<_>>();
        let resume_pcs = if has_yield {
            generator_resume_order
                .iter()
                .enumerate()
                .map(|(idx, label)| (label.clone(), idx))
                .collect::<Vec<_>>()
        } else {
            Vec::new()
        };
        if has_yield {
            lower_generator_yield_terms_to_explicit_return(
                &mut blocks,
                &block_params,
                &resume_pcs,
                is_async_generator_runtime,
            );
        }
        let lowered_is_async = is_async_generator_runtime;
        let mut state_order = entry_params.clone();
        for name in extra_state_vars {
            if !state_order.iter().any(|existing| existing == &name) {
                state_order.push(name);
            }
        }
        let simplify_expr_pass = SimplifyExprPass;
        let ir_blocks = blocks
            .iter()
            .map(|block| {
                let mut normalized_body_stmt = stmt_body_from_stmts(block.body.clone());
                rewrite_with_pass(
                    self.context,
                    None,
                    Some(&simplify_expr_pass),
                    &mut normalized_body_stmt,
                );
                let mut normalized_body = flatten_stmt_boxes(&normalized_body_stmt.body)
                    .into_iter()
                    .map(|stmt| *stmt)
                    .collect::<Vec<_>>();
                let mut normalized_term = block.terminator.clone();
                simplify_terminator_exprs(
                    self.context,
                    &simplify_expr_pass,
                    &mut normalized_term,
                    &mut normalized_body,
                );
                let (exc_target_label, exc_name) = exception_edges
                    .get(block.label.as_str())
                    .cloned()
                    .unwrap_or((None, None));
                let mut local_defs = Vec::new();
                let mut ops = Vec::new();
                let mut pending = VecDeque::from(normalized_body);
                while let Some(stmt) = pending.pop_front() {
                    match stmt {
                        Stmt::FunctionDef(func_def)
                            if func_def.name.id.as_str().starts_with("_dp_bb_") =>
                        {
                            local_defs.push(func_def);
                        }
                        Stmt::Assign(assign)
                            if rewrite_stmt::assign_del::should_rewrite_targets(
                                &assign.targets,
                            ) =>
                        {
                            let rewritten =
                                rewrite_stmt::assign_del::rewrite_assign(self.context, assign);
                            let rewritten_stmt = match rewritten {
                                Rewrite::Unmodified(stmt) | Rewrite::Walk(stmt) => stmt,
                            };
                            let mut lowered = Vec::new();
                            flatten_stmt(&rewritten_stmt, &mut lowered);
                            for lowered_stmt in lowered.into_iter().rev() {
                                pending.push_front(*lowered_stmt);
                            }
                        }
                        other => {
                            if let Some(op) = BbOp::from_stmt(other) {
                                ops.push(op);
                            }
                        }
                    }
                }
                BbBlock {
                    label: block.label.clone(),
                    params: block_params
                        .get(block.label.as_str())
                        .cloned()
                        .unwrap_or_default(),
                    local_defs,
                    ops,
                    exc_target_label,
                    exc_name,
                    term: bb_term_from_terminator(&normalized_term),
                }
            })
            .collect::<Vec<_>>();

        let resume_entry_label = generator_resume_entry_label
            .clone()
            .unwrap_or_else(|| entry_label.clone());
        Some(LoweredFunction {
            blocks: ir_blocks,
            entry_label,
            entry_params: state_order,
            local_cell_slots: cell_slots.clone(),
            param_specs: BbExpr::from_expr(make_param_specs_expr(func.parameters.as_ref())),
            param_names,
            coroutine_wrapper: coroutine_via_generator,
            kind: if has_yield {
                if lowered_is_async {
                    LoweredKind::AsyncGenerator {
                        resume_label: resume_entry_label.clone(),
                        target_labels,
                        resume_pcs,
                    }
                } else {
                    LoweredKind::Generator {
                        resume_label: resume_entry_label.clone(),
                        target_labels,
                        resume_pcs,
                    }
                }
            } else if lowered_is_async {
                LoweredKind::Coroutine
            } else {
                LoweredKind::Function
            },
        })
    }

    fn lower_stmt_sequence(
        &mut self,
        fn_name: &str,
        stmts: &[Box<Stmt>],
        cont_label: String,
        blocks: &mut Vec<Block>,
        loop_ctx: Option<&LoopContext>,
        cell_slots: &HashSet<String>,
        outer_scope_names: &HashSet<String>,
    ) -> String {
        if stmts.is_empty() {
            return cont_label;
        }

        let mut linear = Vec::new();
        let mut index = 0;
        while index < stmts.len() {
            match stmts[index].as_ref() {
                Stmt::Expr(ast::StmtExpr { value, .. }) => {
                    if let Expr::Yield(yield_expr) = value.as_ref() {
                        let resume_label = self.lower_stmt_sequence(
                            fn_name,
                            &stmts[index + 1..],
                            cont_label.clone(),
                            blocks,
                            loop_ctx,
                            cell_slots,
                            outer_scope_names,
                        );
                        let resume_raise_label = self.next_label(fn_name);
                        let resume_dispatch_label = self.next_label(fn_name);
                        blocks.push(Block {
                            label: resume_raise_label.clone(),
                            body: Vec::new(),
                            terminator: Terminator::Raise(raise_stmt_from_name("_dp_resume_exc")),
                        });
                        blocks.push(Block {
                            label: resume_dispatch_label.clone(),
                            body: Vec::new(),
                            terminator: Terminator::BrIf {
                                test: py_expr!("__dp_is_not(_dp_resume_exc, None)"),
                                then_label: resume_raise_label,
                                else_label: resume_label,
                            },
                        });
                        let label = self.next_label(fn_name);
                        blocks.push(Block {
                            label: label.clone(),
                            body: linear,
                            terminator: Terminator::Yield {
                                value: yield_expr.value.as_ref().map(|expr| *expr.clone()),
                                resume_label: resume_dispatch_label,
                            },
                        });
                        return label;
                    }
                    if let Expr::YieldFrom(yield_from_expr) = value.as_ref() {
                        let rest_entry = self.lower_stmt_sequence(
                            fn_name,
                            &stmts[index + 1..],
                            cont_label.clone(),
                            blocks,
                            loop_ctx,
                            cell_slots,
                            outer_scope_names,
                        );
                        let (yield_from_entry, _result_name) = self.lower_yield_from_direct(
                            fn_name,
                            *yield_from_expr.value.clone(),
                            rest_entry,
                            false,
                            blocks,
                        );
                        let label = self.next_label(fn_name);
                        blocks.push(Block {
                            label: label.clone(),
                            body: linear,
                            terminator: Terminator::Jump(yield_from_entry),
                        });
                        return label;
                    }
                    linear.push(stmts[index].as_ref().clone());
                    index += 1;
                }
                Stmt::Pass(_) => {
                    linear.push(stmts[index].as_ref().clone());
                    index += 1;
                }
                Stmt::FunctionDef(func_def) => {
                    if func_def.name.id.as_str().starts_with("_dp_bb_") {
                        linear.push(stmts[index].as_ref().clone());
                    } else {
                        linear.extend(self.lower_non_bb_def_stmt_to_exec_binding(
                            func_def,
                            cell_slots,
                            outer_scope_names,
                        ));
                    }
                    index += 1;
                }
                Stmt::Assign(assign_stmt) => {
                    if let Expr::Yield(yield_expr) = assign_stmt.value.as_ref() {
                        let rest_entry = self.lower_stmt_sequence(
                            fn_name,
                            &stmts[index + 1..],
                            cont_label.clone(),
                            blocks,
                            loop_ctx,
                            cell_slots,
                            outer_scope_names,
                        );
                        let resume_assign_label = self.next_label(fn_name);
                        let resume_raise_label = self.next_label(fn_name);
                        let resume_label = self.next_label(fn_name);
                        let mut resume_assign = assign_stmt.clone();
                        resume_assign.value =
                            Box::new(py_expr!("{sent:id}", sent = "_dp_send_value"));
                        let mut resume_body = vec![Stmt::Assign(resume_assign.clone())];
                        for target in &resume_assign.targets {
                            resume_body.extend(sync_target_cells_stmts(target, cell_slots));
                        }
                        blocks.push(Block {
                            label: resume_assign_label.clone(),
                            body: resume_body,
                            terminator: Terminator::Jump(rest_entry),
                        });
                        blocks.push(Block {
                            label: resume_raise_label.clone(),
                            body: Vec::new(),
                            terminator: Terminator::Raise(raise_stmt_from_name("_dp_resume_exc")),
                        });
                        blocks.push(Block {
                            label: resume_label.clone(),
                            body: Vec::new(),
                            terminator: Terminator::BrIf {
                                test: py_expr!("__dp_is_not(_dp_resume_exc, None)"),
                                then_label: resume_raise_label,
                                else_label: resume_assign_label,
                            },
                        });

                        let label = self.next_label(fn_name);
                        blocks.push(Block {
                            label: label.clone(),
                            body: linear,
                            terminator: Terminator::Yield {
                                value: yield_expr.value.as_ref().map(|expr| *expr.clone()),
                                resume_label,
                            },
                        });
                        return label;
                    }
                    if let Expr::YieldFrom(yield_from_expr) = assign_stmt.value.as_ref() {
                        let rest_entry = self.lower_stmt_sequence(
                            fn_name,
                            &stmts[index + 1..],
                            cont_label.clone(),
                            blocks,
                            loop_ctx,
                            cell_slots,
                            outer_scope_names,
                        );
                        let assign_result_label = self.next_label(fn_name);
                        let (yield_from_entry, result_name) = self.lower_yield_from_direct(
                            fn_name,
                            *yield_from_expr.value.clone(),
                            assign_result_label.clone(),
                            true,
                            blocks,
                        );
                        let result_name = result_name
                            .expect("yield-from assignment lowering requires yielded result");
                        let result_expr = py_expr!("{value:id}", value = result_name.as_str());
                        let mut final_assign = assign_stmt.clone();
                        final_assign.value = Box::new(result_expr);
                        let mut assign_body = vec![Stmt::Assign(final_assign.clone())];
                        for target in &final_assign.targets {
                            assign_body.extend(sync_target_cells_stmts(target, cell_slots));
                        }
                        blocks.push(Block {
                            label: assign_result_label,
                            body: assign_body,
                            terminator: Terminator::Jump(rest_entry),
                        });
                        let label = self.next_label(fn_name);
                        blocks.push(Block {
                            label: label.clone(),
                            body: linear,
                            terminator: Terminator::Jump(yield_from_entry),
                        });
                        return label;
                    }
                    linear.push(stmts[index].as_ref().clone());
                    index += 1;
                }
                Stmt::Raise(raise_stmt) => {
                    let label = self.next_label(fn_name);
                    blocks.push(Block {
                        label: label.clone(),
                        body: linear,
                        terminator: Terminator::Raise(raise_stmt.clone()),
                    });
                    return label;
                }
                Stmt::Delete(delete_stmt) => {
                    linear.extend(rewrite_delete_to_deleted_sentinel(delete_stmt));
                    index += 1;
                }
                Stmt::Return(ret) => {
                    if let Some(value) = ret.value.as_ref() {
                        if let Expr::Yield(yield_expr) = value.as_ref() {
                            let resume_raise_label = self.next_label(fn_name);
                            let resume_return_label = self.next_label(fn_name);
                            let resume_dispatch_label = self.next_label(fn_name);

                            blocks.push(Block {
                                label: resume_raise_label.clone(),
                                body: Vec::new(),
                                terminator: Terminator::Raise(raise_stmt_from_name(
                                    "_dp_resume_exc",
                                )),
                            });
                            blocks.push(Block {
                                label: resume_return_label.clone(),
                                body: Vec::new(),
                                terminator: Terminator::Ret(Some(py_expr!(
                                    "{sent:id}",
                                    sent = "_dp_send_value"
                                ))),
                            });
                            blocks.push(Block {
                                label: resume_dispatch_label.clone(),
                                body: Vec::new(),
                                terminator: Terminator::BrIf {
                                    test: py_expr!("__dp_is_not(_dp_resume_exc, None)"),
                                    then_label: resume_raise_label,
                                    else_label: resume_return_label,
                                },
                            });

                            let label = self.next_label(fn_name);
                            blocks.push(Block {
                                label: label.clone(),
                                body: linear,
                                terminator: Terminator::Yield {
                                    value: yield_expr.value.as_ref().map(|expr| *expr.clone()),
                                    resume_label: resume_dispatch_label,
                                },
                            });
                            return label;
                        }
                        if let Expr::YieldFrom(yield_from_expr) = value.as_ref() {
                            let return_label = self.next_label(fn_name);
                            let (yield_from_entry, result_name) = self.lower_yield_from_direct(
                                fn_name,
                                *yield_from_expr.value.clone(),
                                return_label.clone(),
                                true,
                                blocks,
                            );
                            let result_name = result_name
                                .expect("yield-from return lowering requires yielded result");
                            let result_expr = py_expr!("{value:id}", value = result_name.as_str());
                            blocks.push(Block {
                                label: return_label,
                                body: Vec::new(),
                                terminator: Terminator::Ret(Some(result_expr)),
                            });
                            let label = self.next_label(fn_name);
                            blocks.push(Block {
                                label: label.clone(),
                                body: linear,
                                terminator: Terminator::Jump(yield_from_entry),
                            });
                            return label;
                        }
                    }
                    let label = self.next_label(fn_name);
                    blocks.push(Block {
                        label: label.clone(),
                        body: linear,
                        terminator: Terminator::Ret(ret.value.as_ref().map(|expr| *expr.clone())),
                    });
                    return label;
                }
                Stmt::If(if_stmt) => {
                    let then_body = flatten_stmt_boxes(&if_stmt.body.body);
                    let else_body = flatten_stmt_boxes(&extract_else_body(if_stmt));
                    let rest_entry = self.lower_stmt_sequence(
                        fn_name,
                        &stmts[index + 1..],
                        cont_label.clone(),
                        blocks,
                        loop_ctx,
                        cell_slots,
                        outer_scope_names,
                    );
                    let then_entry = self.lower_stmt_sequence(
                        fn_name,
                        &then_body,
                        rest_entry.clone(),
                        blocks,
                        loop_ctx,
                        cell_slots,
                        outer_scope_names,
                    );
                    let else_entry = self.lower_stmt_sequence(
                        fn_name,
                        &else_body,
                        rest_entry,
                        blocks,
                        loop_ctx,
                        cell_slots,
                        outer_scope_names,
                    );
                    let label = self.next_label(fn_name);
                    blocks.push(Block {
                        label: label.clone(),
                        body: linear,
                        terminator: Terminator::BrIf {
                            test: *if_stmt.test.clone(),
                            then_label: then_entry,
                            else_label: else_entry,
                        },
                    });
                    return label;
                }
                Stmt::While(while_stmt) => {
                    let rest_entry = self.lower_stmt_sequence(
                        fn_name,
                        &stmts[index + 1..],
                        cont_label.clone(),
                        blocks,
                        loop_ctx,
                        cell_slots,
                        outer_scope_names,
                    );

                    let test_label = self.next_label(fn_name);

                    let else_body = flatten_stmt_boxes(&while_stmt.orelse.body);
                    let cond_false_entry = if else_body.is_empty() {
                        rest_entry.clone()
                    } else {
                        self.lower_stmt_sequence(
                            fn_name,
                            &else_body,
                            rest_entry.clone(),
                            blocks,
                            loop_ctx,
                            cell_slots,
                            outer_scope_names,
                        )
                    };

                    let body = flatten_stmt_boxes(&while_stmt.body.body);
                    let loop_ctx = LoopContext {
                        continue_label: test_label.clone(),
                        break_label: rest_entry,
                    };
                    let body_entry = self.lower_stmt_sequence(
                        fn_name,
                        &body,
                        test_label.clone(),
                        blocks,
                        Some(&loop_ctx),
                        cell_slots,
                        outer_scope_names,
                    );

                    blocks.push(Block {
                        label: test_label.clone(),
                        body: Vec::new(),
                        terminator: Terminator::BrIf {
                            test: *while_stmt.test.clone(),
                            then_label: body_entry,
                            else_label: cond_false_entry,
                        },
                    });

                    if linear.is_empty() {
                        return test_label;
                    }
                    let linear_label = self.next_label(fn_name);
                    blocks.push(Block {
                        label: linear_label.clone(),
                        body: linear,
                        terminator: Terminator::Jump(test_label),
                    });
                    return linear_label;
                }
                Stmt::For(for_stmt) => {
                    let rest_entry = self.lower_stmt_sequence(
                        fn_name,
                        &stmts[index + 1..],
                        cont_label.clone(),
                        blocks,
                        loop_ctx,
                        cell_slots,
                        outer_scope_names,
                    );

                    let else_body = flatten_stmt_boxes(&for_stmt.orelse.body);
                    let exhausted_entry = if else_body.is_empty() {
                        rest_entry.clone()
                    } else {
                        self.lower_stmt_sequence(
                            fn_name,
                            &else_body,
                            rest_entry.clone(),
                            blocks,
                            loop_ctx,
                            cell_slots,
                            outer_scope_names,
                        )
                    };

                    let iter_name = self.next_temp("iter");
                    let Some(iter_expr) = name_expr(iter_name.as_str()) else {
                        return cont_label;
                    };
                    let tmp_name = self.next_temp("tmp");
                    let Some(tmp_expr) = name_expr(tmp_name.as_str()) else {
                        return cont_label;
                    };

                    let loop_check_label = self.next_label(fn_name);
                    let loop_continue_label = if for_stmt.is_async {
                        let await_value = py_expr!(
                            "__dp_await_iter(__dp_anext_or_sentinel({iter:expr}))",
                            iter = iter_expr.clone(),
                        );
                        let fetch_done_label = self.next_label(fn_name);
                        let (fetch_entry_label, fetch_result_name) = self.lower_yield_from_direct(
                            fn_name,
                            await_value,
                            fetch_done_label.clone(),
                            true,
                            blocks,
                        );
                        let fetch_result_name = fetch_result_name
                            .expect("async-for fetch lowering requires yielded result");
                        blocks.push(Block {
                            label: fetch_done_label,
                            body: vec![py_stmt!(
                                "{tmp:id} = {value:id}",
                                tmp = tmp_name.as_str(),
                                value = fetch_result_name.as_str(),
                            )],
                            terminator: Terminator::Jump(loop_check_label.clone()),
                        });
                        fetch_entry_label
                    } else {
                        loop_check_label.clone()
                    };
                    let body = flatten_stmt_boxes(&for_stmt.body.body);
                    let loop_ctx = LoopContext {
                        continue_label: loop_continue_label.clone(),
                        break_label: rest_entry,
                    };
                    let body_entry = self.lower_stmt_sequence(
                        fn_name,
                        &body,
                        loop_continue_label.clone(),
                        blocks,
                        Some(&loop_ctx),
                        cell_slots,
                        outer_scope_names,
                    );

                    let assign_label = self.next_label(fn_name);
                    let mut assign_body = Vec::new();
                    if is_simple_index_target(for_stmt.target.as_ref()) {
                        self.emit_target_assignments(
                            for_stmt.target.as_ref(),
                            tmp_expr.clone(),
                            &mut assign_body,
                        );
                    } else {
                        // Normalize complex assignment targets at the lowering site so
                        // BbOp::Assign only ever sees name targets.
                        let rewritten = rewrite_stmt::assign_del::rewrite_assign(
                            self.context,
                            ast::StmtAssign {
                                range: TextRange::default(),
                                node_index: ast::AtomicNodeIndex::default(),
                                targets: vec![*for_stmt.target.clone()],
                                value: Box::new(tmp_expr.clone()),
                            },
                        );
                        let rewritten_stmt = match rewritten {
                            Rewrite::Unmodified(stmt) | Rewrite::Walk(stmt) => stmt,
                        };
                        let mut lowered = Vec::new();
                        flatten_stmt(&rewritten_stmt, &mut lowered);
                        assign_body.extend(lowered.into_iter().map(|stmt| *stmt));
                    }
                    assign_body.extend(sync_target_cells_stmts(
                        for_stmt.target.as_ref(),
                        cell_slots,
                    ));
                    assign_body.push(py_stmt!("{tmp:id} = None", tmp = tmp_name.as_str()));
                    blocks.push(Block {
                        label: assign_label.clone(),
                        body: assign_body,
                        terminator: Terminator::Jump(body_entry),
                    });

                    let exhausted_test = py_expr!(
                        "__dp_is_({value:expr}, __dp__.ITER_COMPLETE)",
                        value = tmp_expr.clone(),
                    );
                    let check_body = if for_stmt.is_async {
                        Vec::new()
                    } else {
                        vec![py_stmt!(
                            "{tmp:id} = __dp_next_or_sentinel({iter:expr})",
                            tmp = tmp_name.as_str(),
                            iter = iter_expr.clone(),
                        )]
                    };
                    blocks.push(Block {
                        label: loop_check_label.clone(),
                        body: check_body,
                        terminator: Terminator::BrIf {
                            test: exhausted_test,
                            then_label: exhausted_entry,
                            else_label: assign_label,
                        },
                    });

                    let mut setup_body = linear;
                    if for_stmt.is_async {
                        setup_body.push(py_stmt!(
                            "{iter:id} = __dp_aiter({iterable:expr})",
                            iter = iter_name.as_str(),
                            iterable = *for_stmt.iter.clone(),
                        ));
                    } else {
                        setup_body.push(py_stmt!(
                            "{iter:id} = __dp_iter({iterable:expr})",
                            iter = iter_name.as_str(),
                            iterable = *for_stmt.iter.clone(),
                        ));
                    }
                    let setup_label = self.next_label(fn_name);
                    blocks.push(Block {
                        label: setup_label.clone(),
                        body: setup_body,
                        terminator: Terminator::Jump(loop_continue_label),
                    });
                    return setup_label;
                }
                Stmt::Try(try_stmt) => {
                    if try_stmt.is_star {
                        let rewritten_try =
                            match rewrite_stmt::exception::rewrite_try(try_stmt.clone()) {
                                Rewrite::Walk(stmt) | Rewrite::Unmodified(stmt) => stmt,
                            };
                        let mut expanded = match rewritten_try {
                            Stmt::BodyStmt(body) => body.body,
                            stmt => vec![Box::new(stmt)],
                        };
                        expanded.extend(stmts[index + 1..].iter().cloned());
                        let expanded_entry = self.lower_stmt_sequence(
                            fn_name,
                            &expanded,
                            cont_label.clone(),
                            blocks,
                            loop_ctx,
                            cell_slots,
                            outer_scope_names,
                        );
                        let label = self.next_label(fn_name);
                        blocks.push(Block {
                            label: label.clone(),
                            body: linear,
                            terminator: Terminator::Jump(expanded_entry),
                        });
                        return label;
                    }

                    let rest_entry = self.lower_stmt_sequence(
                        fn_name,
                        &stmts[index + 1..],
                        cont_label.clone(),
                        blocks,
                        loop_ctx,
                        cell_slots,
                        outer_scope_names,
                    );

                    let has_finally = !try_stmt.finalbody.body.is_empty();
                    let needs_finally_return_flow = has_finally
                        && (contains_return_stmt_in_body(&try_stmt.body.body)
                            || contains_return_stmt_in_handlers(&try_stmt.handlers)
                            || contains_return_stmt_in_body(&try_stmt.orelse.body));
                    let mut finally_exc_name: Option<String> = None;
                    let mut finally_reason_name: Option<String> = None;
                    let mut finally_return_value_name: Option<String> = None;
                    let (finally_label, finally_region_labels, finally_fallthrough_label) =
                        if has_finally {
                            let reason_name = if needs_finally_return_flow {
                                let name = self.next_temp("try_reason");
                                finally_reason_name = Some(name.clone());
                                Some(name)
                            } else {
                                None
                            };
                            let return_name = if needs_finally_return_flow {
                                let name = self.next_temp("try_value");
                                finally_return_value_name = Some(name.clone());
                                Some(name)
                            } else {
                                None
                            };
                            let finally_dispatch_label = if needs_finally_return_flow {
                                Some(self.next_label(fn_name))
                            } else {
                                None
                            };
                            let finally_return_label = if needs_finally_return_flow {
                                Some(self.next_label(fn_name))
                            } else {
                                None
                            };
                            let finally_cont_label = finally_dispatch_label
                                .clone()
                                .unwrap_or_else(|| rest_entry.clone());

                            let finally_region_start = blocks.len();
                            let mut finally_body = flatten_stmt_boxes(&try_stmt.finalbody.body);
                            let finally_exc_candidate = self.next_temp("try_exc");
                            finally_body = rewrite_exception_accesses(
                                finally_body,
                                finally_exc_candidate.as_str(),
                            );
                            finally_body.push(Box::new(py_stmt!(
                                "if __dp_is_not({exc:id}, None):\n    raise {exc:id}",
                                exc = finally_exc_candidate.as_str(),
                            )));
                            finally_exc_name = Some(finally_exc_candidate);
                            let finally_label = self.lower_stmt_sequence(
                                fn_name,
                                &finally_body,
                                finally_cont_label,
                                blocks,
                                loop_ctx,
                                cell_slots,
                                outer_scope_names,
                            );
                            let finally_region_labels = blocks[finally_region_start..]
                                .iter()
                                .map(|block| block.label.clone())
                                .collect::<Vec<_>>();
                            if let (
                                Some(finally_return_label),
                                Some(finally_dispatch_label),
                                Some(return_name),
                                Some(reason_name),
                            ) = (
                                finally_return_label,
                                finally_dispatch_label.clone(),
                                return_name,
                                reason_name,
                            ) {
                                blocks.push(Block {
                                    label: finally_return_label.clone(),
                                    body: Vec::new(),
                                    terminator: Terminator::Ret(Some(py_expr!(
                                        "{name:id}",
                                        name = return_name.as_str(),
                                    ))),
                                });
                                blocks.push(Block {
                                    label: finally_dispatch_label.clone(),
                                    body: Vec::new(),
                                    terminator: Terminator::BrIf {
                                        test: py_expr!(
                                            "__dp_eq({reason:id}, 'return')",
                                            reason = reason_name.as_str(),
                                        ),
                                        then_label: finally_return_label,
                                        else_label: rest_entry.clone(),
                                    },
                                });
                            }
                            (
                                Some(finally_label),
                                finally_region_labels,
                                Some(finally_dispatch_label.unwrap_or_else(|| rest_entry.clone())),
                            )
                        } else {
                            (None, Vec::new(), None)
                        };
                    let pass_target = finally_label.clone().unwrap_or_else(|| rest_entry.clone());

                    let body_region_start = blocks.len();
                    let body_pass_label = self.next_label(fn_name);
                    let mut body_pass_stmts = Vec::new();
                    if let Some(reason_name) = finally_reason_name.as_ref() {
                        body_pass_stmts.push(py_stmt!(
                            "{reason:id} = None",
                            reason = reason_name.as_str(),
                        ));
                    }
                    if let Some(exc_name) = finally_exc_name.as_ref() {
                        body_pass_stmts.push(py_stmt!("{exc:id} = None", exc = exc_name.as_str(),));
                    }
                    blocks.push(Block {
                        label: body_pass_label.clone(),
                        body: body_pass_stmts,
                        terminator: Terminator::Jump(pass_target.clone()),
                    });

                    let else_body = flatten_stmt_boxes(&try_stmt.orelse.body);
                    let else_entry = self.lower_stmt_sequence(
                        fn_name,
                        &else_body,
                        body_pass_label,
                        blocks,
                        loop_ctx,
                        cell_slots,
                        outer_scope_names,
                    );

                    let try_body = flatten_stmt_boxes(&try_stmt.body.body);
                    let body_label = self.lower_stmt_sequence(
                        fn_name,
                        &try_body,
                        else_entry,
                        blocks,
                        loop_ctx,
                        cell_slots,
                        outer_scope_names,
                    );
                    let body_region_labels = blocks[body_region_start..]
                        .iter()
                        .map(|block| block.label.clone())
                        .collect::<Vec<_>>();

                    let except_region_start = blocks.len();
                    let except_pass_label = self.next_label(fn_name);
                    let except_exc_name = self.next_temp("try_exc");
                    let mut except_pass_stmts = Vec::new();
                    if let Some(reason_name) = finally_reason_name.as_ref() {
                        except_pass_stmts.push(py_stmt!(
                            "{reason:id} = None",
                            reason = reason_name.as_str(),
                        ));
                    }
                    if let Some(exc_name) = finally_exc_name.as_ref() {
                        except_pass_stmts
                            .push(py_stmt!("{exc:id} = None", exc = exc_name.as_str(),));
                    }
                    except_pass_stmts.push(py_stmt!(
                        "{exc:id} = __dp_DELETED",
                        exc = except_exc_name.as_str(),
                    ));
                    blocks.push(Block {
                        label: except_pass_label.clone(),
                        body: except_pass_stmts,
                        terminator: Terminator::Jump(pass_target),
                    });
                    let except_body = try_stmt
                        .handlers
                        .first()
                        .map(|handler| {
                            let ast::ExceptHandler::ExceptHandler(handler) = handler;
                            flatten_stmt_boxes(&handler.body.body)
                        })
                        .unwrap_or_else(|| {
                            vec![Box::new(py_stmt!(
                                "raise {exc:id}",
                                exc = except_exc_name.as_str(),
                            ))]
                        });
                    let except_body =
                        rewrite_exception_accesses(except_body, except_exc_name.as_str());
                    let except_label = self.lower_stmt_sequence(
                        fn_name,
                        &except_body,
                        except_pass_label,
                        blocks,
                        loop_ctx,
                        cell_slots,
                        outer_scope_names,
                    );
                    let except_region_labels = blocks[except_region_start..]
                        .iter()
                        .map(|block| block.label.clone())
                        .collect::<Vec<_>>();

                    if let (Some(reason_name), Some(return_name), Some(finally_target)) = (
                        finally_reason_name.as_ref(),
                        finally_return_value_name.as_ref(),
                        finally_label.as_ref(),
                    ) {
                        let finally_exc_name = finally_exc_name.as_deref();
                        rewrite_region_returns_to_finally(
                            blocks,
                            &body_region_labels,
                            reason_name.as_str(),
                            return_name.as_str(),
                            finally_target.as_str(),
                            finally_exc_name,
                        );
                        rewrite_region_returns_to_finally(
                            blocks,
                            &except_region_labels,
                            reason_name.as_str(),
                            return_name.as_str(),
                            finally_target.as_str(),
                            finally_exc_name,
                        );
                    }

                    let label = self.next_label(fn_name);
                    blocks.push(Block {
                        label: label.clone(),
                        body: linear,
                        terminator: Terminator::TryJump {
                            body_label,
                            except_label,
                            except_exc_name: Some(except_exc_name),
                            body_region_labels,
                            except_region_labels,
                            finally_label,
                            finally_exc_name,
                            finally_region_labels,
                            finally_fallthrough_label,
                        },
                    });
                    return label;
                }
                Stmt::Break(_) => {
                    let Some(loop_ctx) = loop_ctx else {
                        return cont_label;
                    };
                    let label = self.next_label(fn_name);
                    blocks.push(Block {
                        label: label.clone(),
                        body: linear,
                        terminator: Terminator::Jump(loop_ctx.break_label.clone()),
                    });
                    return label;
                }
                Stmt::Continue(_) => {
                    let Some(loop_ctx) = loop_ctx else {
                        return cont_label;
                    };
                    let label = self.next_label(fn_name);
                    blocks.push(Block {
                        label: label.clone(),
                        body: linear,
                        terminator: Terminator::Jump(loop_ctx.continue_label.clone()),
                    });
                    return label;
                }
                _ => return cont_label,
            }
        }

        let label = self.next_label(fn_name);
        blocks.push(Block {
            label: label.clone(),
            body: linear,
            terminator: Terminator::Jump(cont_label),
        });
        label
    }

    fn lower_non_bb_def_stmt_to_exec_binding(
        &self,
        func_def: &ast::StmtFunctionDef,
        cell_slots: &HashSet<String>,
        outer_scope_names: &HashSet<String>,
    ) -> Vec<Stmt> {
        let mut source_fn = func_def.clone();
        let bind_name = source_fn.name.id.to_string();
        ensure_dp_default_param(&mut source_fn);
        let capture_names = collect_capture_names(&source_fn, Some(outer_scope_names));
        ensure_capture_default_params(&mut source_fn, &capture_names);
        let source = render_stmt_source(&Stmt::FunctionDef(source_fn));
        let captures = make_dp_tuple(
            capture_names
                .iter()
                .map(|name| {
                    py_expr!(
                        "({name:literal}, {value:id})",
                        name = name.as_str(),
                        value = name.as_str(),
                    )
                })
                .collect(),
        );
        let base_value = py_expr!(
            "__dp_exec_function_def_source({source:literal}, __dp_globals(), {captures:expr}, {name:literal})",
            source = source.as_str(),
            captures = captures,
            name = bind_name.as_str(),
        );
        let mut out = vec![py_stmt!(
            "{name:id} = {value:expr}",
            name = bind_name.as_str(),
            value = base_value,
        )];
        let target_expr = py_expr!("{name:id}", name = bind_name.as_str());
        out.extend(sync_target_cells_stmts(&target_expr, cell_slots));
        out
    }

    fn lower_yield_from_direct(
        &mut self,
        fn_name: &str,
        value: Expr,
        after_label: String,
        capture_result: bool,
        blocks: &mut Vec<Block>,
    ) -> (String, Option<String>) {
        let iter_name = self.next_temp("yield_from_iter");
        let yielded_name = self.next_temp("yield_from_y");
        let sent_name = self.next_temp("yield_from_sent");
        let result_name = if capture_result {
            Some(self.next_temp("yield_from_result"))
        } else {
            None
        };
        let stop_name = self.next_temp("yield_from_stop");
        let exc_name = self.next_temp("yield_from_exc");
        let raise_name = self.next_temp("yield_from_raise");
        let close_name = self.next_temp("yield_from_close");
        let throw_name = self.next_temp("yield_from_throw");

        let init_try_label = self.next_label(fn_name);
        let next_body_label = self.next_label(fn_name);
        let stop_except_label = self.next_label(fn_name);

        let stop_done_label = self.next_label(fn_name);
        let raise_stop_label = self.next_label(fn_name);
        let clear_done_label = self.next_label(fn_name);
        let clear_raise_label = self.next_label(fn_name);

        let yield_label = self.next_label(fn_name);
        let resume_label = self.next_label(fn_name);
        let exc_dispatch_label = self.next_label(fn_name);
        let genexit_close_lookup_label = self.next_label(fn_name);
        let genexit_call_close_label = self.next_label(fn_name);
        let raise_exc_label = self.next_label(fn_name);
        let lookup_throw_label = self.next_label(fn_name);
        let throw_try_label = self.next_label(fn_name);
        let throw_body_label = self.next_label(fn_name);

        let send_try_label = self.next_label(fn_name);
        let send_dispatch_label = self.next_label(fn_name);
        let send_call_body_label = self.next_label(fn_name);

        blocks.push(Block {
            label: init_try_label.clone(),
            body: vec![
                py_stmt!(
                    "{iter_name:id} = iter({iter_expr:expr})",
                    iter_name = iter_name.as_str(),
                    iter_expr = value,
                ),
                py_stmt!(
                    "__dp_setattr(_dp_self, \"gi_yieldfrom\", {iter_name:id})",
                    iter_name = iter_name.as_str(),
                ),
            ],
            terminator: Terminator::TryJump {
                body_label: next_body_label.clone(),
                except_label: stop_except_label.clone(),
                except_exc_name: Some(stop_name.clone()),
                body_region_labels: vec![next_body_label.clone()],
                except_region_labels: vec![
                    stop_except_label.clone(),
                    stop_done_label.clone(),
                    raise_stop_label.clone(),
                ],
                finally_label: None,
                finally_exc_name: None,
                finally_region_labels: Vec::new(),
                finally_fallthrough_label: None,
            },
        });
        blocks.push(Block {
            label: next_body_label.clone(),
            body: vec![py_stmt!(
                "{yielded:id} = next(__dp_getattr(_dp_self, \"gi_yieldfrom\"))",
                yielded = yielded_name.as_str(),
            )],
            terminator: Terminator::Jump(yield_label.clone()),
        });
        blocks.push(Block {
            label: stop_except_label.clone(),
            body: Vec::new(),
            terminator: Terminator::BrIf {
                test: py_expr!(
                    "__dp_exception_matches({stop:id}, StopIteration)",
                    stop = stop_name.as_str(),
                ),
                then_label: stop_done_label.clone(),
                else_label: raise_stop_label.clone(),
            },
        });
        blocks.push(Block {
            label: stop_done_label.clone(),
            body: if let Some(result_name) = result_name.as_ref() {
                vec![py_stmt!(
                    "{result:id} = {stop:id}.value",
                    result = result_name.as_str(),
                    stop = stop_name.as_str(),
                )]
            } else {
                Vec::new()
            },
            terminator: Terminator::Jump(clear_done_label.clone()),
        });
        blocks.push(Block {
            label: clear_done_label,
            body: vec![py_stmt!("__dp_setattr(_dp_self, \"gi_yieldfrom\", None)")],
            terminator: Terminator::Jump(after_label),
        });
        blocks.push(Block {
            label: raise_stop_label.clone(),
            body: vec![py_stmt!(
                "{raise:id} = {stop:id}",
                raise = raise_name.as_str(),
                stop = stop_name.as_str(),
            )],
            terminator: Terminator::Jump(clear_raise_label.clone()),
        });
        blocks.push(Block {
            label: yield_label.clone(),
            body: Vec::new(),
            terminator: Terminator::Yield {
                value: Some(py_expr!("{yielded:id}", yielded = yielded_name.as_str(),)),
                resume_label: resume_label.clone(),
            },
        });
        blocks.push(Block {
            label: resume_label,
            body: vec![
                py_stmt!(
                    "{sent:id} = {resume:id}",
                    sent = sent_name.as_str(),
                    resume = "_dp_send_value",
                ),
                py_stmt!(
                    "{exc:id} = {resume:id}",
                    exc = exc_name.as_str(),
                    resume = "_dp_resume_exc",
                ),
                py_stmt!("{resume:id} = None", resume = "_dp_resume_exc",),
            ],
            terminator: Terminator::BrIf {
                test: py_expr!("__dp_is_not({exc:id}, None)", exc = exc_name.as_str()),
                then_label: exc_dispatch_label.clone(),
                else_label: send_try_label.clone(),
            },
        });
        blocks.push(Block {
            label: exc_dispatch_label,
            body: Vec::new(),
            terminator: Terminator::BrIf {
                test: py_expr!(
                    "__dp_exception_matches({exc:id}, GeneratorExit)",
                    exc = exc_name.as_str(),
                ),
                then_label: genexit_close_lookup_label.clone(),
                else_label: lookup_throw_label.clone(),
            },
        });
        blocks.push(Block {
            label: genexit_close_lookup_label,
            body: vec![py_stmt!(
                "{close:id} = getattr(__dp_getattr(_dp_self, \"gi_yieldfrom\"), \"close\", None)",
                close = close_name.as_str(),
            )],
            terminator: Terminator::BrIf {
                test: py_expr!("__dp_is_not({close:id}, None)", close = close_name.as_str()),
                then_label: genexit_call_close_label.clone(),
                else_label: raise_exc_label.clone(),
            },
        });
        blocks.push(Block {
            label: genexit_call_close_label,
            body: vec![py_stmt!("{close:id}()", close = close_name.as_str())],
            terminator: Terminator::Jump(raise_exc_label.clone()),
        });
        blocks.push(Block {
            label: raise_exc_label.clone(),
            body: vec![py_stmt!(
                "{raise:id} = {exc:id}",
                raise = raise_name.as_str(),
                exc = exc_name.as_str(),
            )],
            terminator: Terminator::Jump(clear_raise_label.clone()),
        });
        blocks.push(Block {
            label: clear_raise_label,
            body: vec![py_stmt!("__dp_setattr(_dp_self, \"gi_yieldfrom\", None)")],
            terminator: Terminator::Raise(raise_stmt_from_name(raise_name.as_str())),
        });
        blocks.push(Block {
            label: lookup_throw_label,
            body: vec![py_stmt!(
                "{throw:id} = getattr(__dp_getattr(_dp_self, \"gi_yieldfrom\"), \"throw\", None)",
                throw = throw_name.as_str(),
            )],
            terminator: Terminator::BrIf {
                test: py_expr!("__dp_is_({throw:id}, None)", throw = throw_name.as_str()),
                then_label: raise_exc_label,
                else_label: throw_try_label.clone(),
            },
        });
        blocks.push(Block {
            label: throw_try_label,
            body: Vec::new(),
            terminator: Terminator::TryJump {
                body_label: throw_body_label.clone(),
                except_label: stop_except_label.clone(),
                except_exc_name: Some(stop_name.clone()),
                body_region_labels: vec![throw_body_label.clone()],
                except_region_labels: vec![
                    stop_except_label.clone(),
                    stop_done_label.clone(),
                    raise_stop_label.clone(),
                ],
                finally_label: None,
                finally_exc_name: None,
                finally_region_labels: Vec::new(),
                finally_fallthrough_label: None,
            },
        });
        blocks.push(Block {
            label: throw_body_label,
            body: vec![py_stmt!(
                "{yielded:id} = {throw:id}({exc:id})",
                yielded = yielded_name.as_str(),
                throw = throw_name.as_str(),
                exc = exc_name.as_str(),
            )],
            terminator: Terminator::Jump(yield_label.clone()),
        });
        blocks.push(Block {
            label: send_try_label,
            body: Vec::new(),
            terminator: Terminator::TryJump {
                body_label: send_dispatch_label.clone(),
                except_label: stop_except_label.clone(),
                except_exc_name: Some(stop_name.clone()),
                body_region_labels: vec![
                    send_dispatch_label.clone(),
                    next_body_label.clone(),
                    send_call_body_label.clone(),
                ],
                except_region_labels: vec![
                    stop_except_label.clone(),
                    stop_done_label.clone(),
                    raise_stop_label.clone(),
                ],
                finally_label: None,
                finally_exc_name: None,
                finally_region_labels: Vec::new(),
                finally_fallthrough_label: None,
            },
        });
        blocks.push(Block {
            label: send_dispatch_label,
            body: Vec::new(),
            terminator: Terminator::BrIf {
                test: py_expr!("__dp_is_({sent:id}, None)", sent = sent_name.as_str()),
                then_label: next_body_label,
                else_label: send_call_body_label.clone(),
            },
        });
        blocks.push(Block {
            label: send_call_body_label,
            body: vec![py_stmt!(
                "{yielded:id} = __dp_getattr(_dp_self, \"gi_yieldfrom\").send({sent:id})",
                yielded = yielded_name.as_str(),
                sent = sent_name.as_str(),
            )],
            terminator: Terminator::Jump(yield_label),
        });
        (init_try_label, result_name)
    }

    fn next_label(&mut self, fn_name: &str) -> String {
        let current = self.next_block_id;
        self.next_block_id += 1;
        format!("_dp_bb_{}_{}", sanitize_ident(fn_name), current)
    }

    fn next_label_prefix(&mut self, fn_name: &str) -> String {
        let base = sanitize_ident(original_function_name(fn_name).as_str());
        let count = self.used_label_prefixes.entry(base.clone()).or_insert(0);
        let suffix = if *count == 0 {
            String::new()
        } else {
            format!("_{}", *count)
        };
        *count += 1;
        format!("_dp_bb_{base}{suffix}")
    }

    fn function_identity_for(&self, func: &ast::StmtFunctionDef) -> FunctionIdentity {
        if is_module_init_temp_name(func.name.id.as_str()) {
            return FunctionIdentity {
                bind_name: "_dp_module_init".to_string(),
                display_name: "_dp_module_init".to_string(),
                qualname: "_dp_module_init".to_string(),
                binding_target: BindingTarget::ModuleGlobal,
            };
        }
        let node_index = func.node_index.load();
        if let Some(identity) = self.function_identity_by_node.get(&node_index) {
            return identity.clone();
        }
        let bind_name = func.name.id.to_string();
        let display_name = display_name_for_function(bind_name.as_str()).to_string();
        FunctionIdentity {
            bind_name: bind_name.clone(),
            display_name,
            qualname: bind_name,
            binding_target: self.default_binding_target_for_name(func.name.id.as_str()),
        }
    }

    fn build_def_expr_from_bb(
        &self,
        bb_function: &BbFunction,
        doc_expr: Option<Expr>,
        annotate_fn_expr: Option<Expr>,
    ) -> Option<Expr> {
        let entry_label = bb_function.entry.as_str();
        let entry_ref_expr = py_expr!("{entry:literal}", entry = entry_label);
        let param_names: HashSet<&str> =
            bb_function.param_names.iter().map(String::as_str).collect();
        let locally_assigned: HashSet<&str> = bb_function
            .blocks
            .iter()
            .flat_map(|block| block.ops.iter())
            .filter_map(|op| match op {
                BbOp::Assign(assign) => Some(assign.target.id.as_str()),
                _ => None,
            })
            .collect();
        let mut closure_items = Vec::new();
        for entry_name in &bb_function.entry_params {
            if param_names.contains(entry_name.as_str()) {
                closure_items.push(py_expr!("{value:literal}", value = entry_name.as_str(),));
            } else if entry_name == "_dp_classcell"
                || (entry_name.starts_with("_dp_cell_")
                    && !bb_function
                        .local_cell_slots
                        .iter()
                        .any(|slot| slot == entry_name))
            {
                let value = name_expr(entry_name.as_str())?;
                closure_items.push(make_dp_tuple(vec![
                    py_expr!("{value:literal}", value = entry_name.as_str()),
                    value,
                ]));
            } else if !entry_name.starts_with("_dp_")
                && !locally_assigned.contains(entry_name.as_str())
            {
                let value = name_expr(entry_name.as_str())?;
                closure_items.push(make_dp_tuple(vec![
                    py_expr!("{value:literal}", value = entry_name.as_str()),
                    value,
                ]));
            } else {
                closure_items.push(py_expr!("{value:literal}", value = entry_name.as_str(),));
            }
        }
        let closure = make_dp_tuple(closure_items);
        let doc = doc_expr.unwrap_or_else(|| py_expr!("None"));
        let annotate_fn = annotate_fn_expr.unwrap_or_else(|| py_expr!("None"));
        match &bb_function.kind {
            BbFunctionKind::Function => Some(py_expr!(
                "__dp_def_fn({entry:expr}, {name:literal}, {qualname:literal}, {closure:expr}, {params:expr}, {module_globals:expr}, {module_name:expr}, {doc:expr}, {annotate_fn:expr})",
                entry = entry_ref_expr.clone(),
                name = bb_function.display_name.as_str(),
                qualname = bb_function.qualname.as_str(),
                closure = closure,
                params = bb_function.param_specs.to_expr(),
                module_globals = py_expr!("__dp_globals()"),
                module_name = py_expr!("__name__"),
                doc = doc.clone(),
                annotate_fn = annotate_fn.clone(),
            )),
            BbFunctionKind::Coroutine => Some(py_expr!(
                "__dp_def_coro({entry:expr}, {name:literal}, {qualname:literal}, {closure:expr}, {params:expr}, {module_globals:expr}, {module_name:expr}, {doc:expr}, {annotate_fn:expr})",
                entry = entry_ref_expr.clone(),
                name = bb_function.display_name.as_str(),
                qualname = bb_function.qualname.as_str(),
                closure = closure,
                params = bb_function.param_specs.to_expr(),
                module_globals = py_expr!("__dp_globals()"),
                module_name = py_expr!("__name__"),
                doc = doc.clone(),
                annotate_fn = annotate_fn.clone(),
            )),
            BbFunctionKind::AsyncGenerator {
                ..
            } => {
                Some(py_expr!(
                    "__dp_def_async_gen({resume:expr}, {name:literal}, {qualname:literal}, {closure:expr}, {params:expr}, __dp_globals(), __name__, {doc:expr}, {annotate_fn:expr})",
                    resume = entry_ref_expr.clone(),
                    name = bb_function.display_name.as_str(),
                    qualname = bb_function.qualname.as_str(),
                    closure = closure,
                    params = bb_function.param_specs.to_expr(),
                    doc = doc.clone(),
                    annotate_fn = annotate_fn.clone(),
                ))
            }
            BbFunctionKind::Generator {
                ..
            } => {
                let helper_name = if bb_function.is_coroutine {
                    "__dp_def_coro_from_gen"
                } else {
                    "__dp_def_gen"
                };
                Some(py_expr!(
                    "{helper:id}({resume:expr}, {name:literal}, {qualname:literal}, {closure:expr}, {params:expr}, __dp_globals(), __name__, {doc:expr}, {annotate_fn:expr})",
                    helper = helper_name,
                    resume = entry_ref_expr,
                    name = bb_function.display_name.as_str(),
                    qualname = bb_function.qualname.as_str(),
                    closure = closure,
                    params = bb_function.param_specs.to_expr(),
                    doc = doc,
                    annotate_fn = annotate_fn,
                ))
            }
        }
    }

    fn build_lowered_binding_stmt(
        &self,
        func: &ast::StmtFunctionDef,
        bb_function: &BbFunction,
    ) -> Option<Stmt> {
        let identity = self.function_identity_for(func);
        let target = self.resolved_binding_target(&identity);
        let bind_name = identity.bind_name.as_str();

        let annotation_entries = function_annotation_entries(func);
        let annotate_helper_stmt = if annotation_entries.is_empty() {
            None
        } else {
            // Keep helper name in __annotate__ family so BB lowering keeps it in lexical scope.
            let annotate_helper_name = format!("_dp_fn___annotate___{bind_name}");
            let helper_stmt = rewrite_stmt::annotation::build_annotate_fn(
                annotation_entries,
                annotate_helper_name.as_str(),
            );
            let helper_stmt = match helper_stmt {
                Stmt::FunctionDef(helper_fn) => annotation_helper_exec_binding_stmt(
                    helper_fn,
                    annotate_helper_name.as_str(),
                    None,
                ),
                other => other,
            };
            Some((annotate_helper_name.clone(), helper_stmt))
        };

        let annotate_fn_expr = match annotate_helper_stmt.as_ref() {
            Some((helper_name, _)) => Some(name_expr(helper_name.as_str())?),
            None => None,
        };
        let doc_expr = function_docstring_expr(func);

        let base_expr = self.build_def_expr_from_bb(bb_function, doc_expr, annotate_fn_expr)?;
        let decorated = rewrite_stmt::decorator::rewrite(func.decorator_list.clone(), base_expr);
        let binding_stmt = self.make_binding_stmt(target, bind_name, decorated);
        let mut stmts = Vec::new();
        if let Some((_, helper_stmt)) = annotate_helper_stmt {
            stmts.push(helper_stmt);
        }
        stmts.push(binding_stmt);
        if target == BindingTarget::Local && self.needs_cell_sync(bind_name) {
            let cell = cell_name(bind_name);
            stmts.push(py_stmt!(
                "__dp_store_cell({cell:id}, {name:id})",
                cell = cell.as_str(),
                name = bind_name,
            ));
        }
        if stmts.len() == 1 {
            stmts.into_iter().next()
        } else {
            Some(into_body(stmts))
        }
    }

    fn default_binding_target_for_name(&self, bind_name: &str) -> BindingTarget {
        match self.function_stack.last().map(String::as_str) {
            Some(parent) if is_module_init_temp_name(parent) => {
                if is_internal_symbol(bind_name) {
                    BindingTarget::Local
                } else {
                    BindingTarget::ModuleGlobal
                }
            }
            Some(parent) if parent.starts_with("_dp_class_ns_") => {
                if is_internal_symbol(bind_name) {
                    BindingTarget::Local
                } else {
                    BindingTarget::ClassNamespace
                }
            }
            _ => BindingTarget::Local,
        }
    }

    fn make_binding_stmt(&self, target: BindingTarget, bind_name: &str, value: Expr) -> Stmt {
        match target {
            BindingTarget::Local => {
                py_stmt!("{name:id} = {value:expr}", name = bind_name, value = value,)
            }
            BindingTarget::ModuleGlobal => py_stmt!(
                "__dp_store_global(globals(), {name:literal}, {value:expr})",
                name = bind_name,
                value = value,
            ),
            BindingTarget::ClassNamespace => py_stmt!(
                "__dp_setitem(_dp_class_ns, {name:literal}, {value:expr})",
                name = bind_name,
                value = value,
            ),
        }
    }

    fn needs_cell_sync(&self, bind_name: &str) -> bool {
        self.function_cell_bindings_stack
            .last()
            .map(|cells| cells.contains(bind_name))
            .unwrap_or(false)
    }

    fn resolved_binding_target(&self, identity: &FunctionIdentity) -> BindingTarget {
        if identity.binding_target == BindingTarget::Local
            && identity.qualname == identity.bind_name
            && !is_internal_symbol(identity.bind_name.as_str())
        {
            // Explicit `global` in nested scopes can still surface here as
            // local after lowering; global-qualname defs must bind to globals.
            BindingTarget::ModuleGlobal
        } else {
            identity.binding_target
        }
    }

    fn build_non_lowered_binding_stmt(&mut self, func: &mut ast::StmtFunctionDef) -> Option<Stmt> {
        let identity = self.function_identity_for(func);
        let bind_name = identity.bind_name.to_string();
        let target = self.resolved_binding_target(&identity);

        if target == BindingTarget::Local {
            if self.needs_cell_sync(bind_name.as_str()) {
                let cell = cell_name(bind_name.as_str());
                return Some(py_stmt!(
                    "__dp_store_cell({cell:id}, {name:id})",
                    cell = cell.as_str(),
                    name = bind_name.as_str(),
                ));
            }
            return None;
        }

        // For non-local bindings, define under an internal temporary name and
        // bind the user-visible name explicitly. This preserves class-scope
        // lookup semantics (`open` should resolve to builtins inside
        // `Wrapper.open`) and honors `global` directives in nested scopes.
        let mut local_name = func.name.id.to_string();
        if !is_internal_symbol(local_name.as_str())
            && !is_annotation_helper_name(bind_name.as_str())
        {
            local_name = self.next_temp("fn_local");
            func.name.id = Name::new(local_name.as_str());
        }

        let decorators = std::mem::take(&mut func.decorator_list);
        let updated = py_expr!(
            "__dp_update_fn({name:id}, {qualname:literal}, {display_name:literal})",
            name = local_name.as_str(),
            qualname = identity.qualname.as_str(),
            display_name = identity.display_name.as_str(),
        );
        let value = rewrite_stmt::decorator::rewrite(decorators, updated);
        Some(self.make_binding_stmt(target, bind_name.as_str(), value))
    }

    fn always_unbound_local_names(
        &self,
        lowered_input_body: &[Box<Stmt>],
        runtime_body: &[Box<Stmt>],
        param_names: &[String],
    ) -> HashSet<String> {
        let original_bound_names = collect_bound_names(lowered_input_body);
        let runtime_bound_names = collect_bound_names(runtime_body);
        let explicit_global_or_nonlocal =
            collect_explicit_global_or_nonlocal_names(lowered_input_body);
        original_bound_names
            .into_iter()
            .filter_map(|name| {
                if param_names.iter().any(|param| param == &name) {
                    return None;
                }
                if is_internal_symbol(name.as_str()) {
                    return None;
                }
                if runtime_bound_names.contains(name.as_str()) {
                    return None;
                }
                if explicit_global_or_nonlocal.contains(name.as_str()) {
                    return None;
                }
                Some(name)
            })
            .collect()
    }
}

struct LoweredFunction {
    blocks: Vec<BbBlock>,
    entry_label: String,
    entry_params: Vec<String>,
    local_cell_slots: HashSet<String>,
    param_specs: BbExpr,
    param_names: Vec<String>,
    coroutine_wrapper: bool,
    kind: LoweredKind,
}

#[derive(Clone)]
enum LoweredKind {
    Function,
    Coroutine,
    AsyncGenerator {
        resume_label: String,
        target_labels: Vec<String>,
        resume_pcs: Vec<(String, usize)>,
    },
    Generator {
        resume_label: String,
        target_labels: Vec<String>,
        resume_pcs: Vec<(String, usize)>,
    },
}

fn simplify_expr_for_bb_term(
    context: &Context,
    pass: &SimplifyExprPass,
    expr: &mut Expr,
    body: &mut Vec<Stmt>,
) {
    let lowered = pass.lower_expr(context, expr.clone());
    if lowered.modified {
        let mut lowered_stmts = Vec::new();
        flatten_stmt(&lowered.stmt, &mut lowered_stmts);
        body.extend(lowered_stmts.into_iter().map(|stmt| *stmt));
    }
    *expr = lowered.expr;
}

fn simplify_terminator_exprs(
    context: &Context,
    pass: &SimplifyExprPass,
    terminator: &mut Terminator,
    body: &mut Vec<Stmt>,
) {
    match terminator {
        Terminator::BrIf { test, .. } => simplify_expr_for_bb_term(context, pass, test, body),
        Terminator::BrTable { index, .. } => {
            simplify_expr_for_bb_term(context, pass, index, body);
        }
        Terminator::Raise(raise_stmt) => {
            if let Some(exc) = raise_stmt.exc.as_mut() {
                simplify_expr_for_bb_term(context, pass, exc, body);
            }
            if let Some(cause) = raise_stmt.cause.as_mut() {
                simplify_expr_for_bb_term(context, pass, cause, body);
            }
        }
        Terminator::Yield { value, .. } => {
            if let Some(value) = value.as_mut() {
                simplify_expr_for_bb_term(context, pass, value, body);
            }
        }
        Terminator::Ret(value) => {
            if let Some(value) = value.as_mut() {
                simplify_expr_for_bb_term(context, pass, value, body);
            }
        }
        Terminator::Jump(_) | Terminator::TryJump { .. } => {}
    }
}

fn bb_term_from_terminator(terminator: &Terminator) -> BbTerm {
    match terminator {
        Terminator::Jump(target) => BbTerm::Jump(target.clone()),
        Terminator::BrIf {
            test,
            then_label,
            else_label,
        } => BbTerm::BrIf {
            test: BbExpr::from_expr(test.clone()),
            then_label: then_label.clone(),
            else_label: else_label.clone(),
        },
        Terminator::BrTable {
            index,
            targets,
            default_label,
        } => BbTerm::BrTable {
            index: BbExpr::from_expr(index.clone()),
            targets: targets.clone(),
            default_label: default_label.clone(),
        },
        Terminator::Raise(ast::StmtRaise { exc, cause, .. }) => BbTerm::Raise {
            exc: exc.as_ref().map(|expr| BbExpr::from_expr(*expr.clone())),
            cause: cause.as_ref().map(|expr| BbExpr::from_expr(*expr.clone())),
        },
        Terminator::TryJump {
            body_label,
            except_label,
            except_exc_name,
            body_region_labels,
            except_region_labels,
            finally_label,
            finally_exc_name,
            finally_region_labels,
            finally_fallthrough_label,
        } => BbTerm::TryJump {
            body_label: body_label.clone(),
            except_label: except_label.clone(),
            except_exc_name: except_exc_name.clone(),
            body_region_labels: body_region_labels.clone(),
            except_region_labels: except_region_labels.clone(),
            finally_label: finally_label.clone(),
            finally_exc_name: finally_exc_name.clone(),
            finally_region_labels: finally_region_labels.clone(),
            finally_fallthrough_label: finally_fallthrough_label.clone(),
        },
        Terminator::Yield { .. } => {
            panic!("internal error: Terminator::Yield must be lowered before BB IR export")
        }
        Terminator::Ret(value) => BbTerm::Ret(value.clone().map(BbExpr::from_expr)),
    }
}

fn raise_done_stmt(is_async: bool, value: Option<Expr>) -> ast::StmtRaise {
    if is_async {
        match py_stmt!("raise StopAsyncIteration()") {
            Stmt::Raise(stmt) => stmt,
            _ => unreachable!("expected raise statement"),
        }
    } else if let Some(value) = value {
        match py_stmt!("raise StopIteration({value:expr})", value = value) {
            Stmt::Raise(stmt) => stmt,
            _ => unreachable!("expected raise statement"),
        }
    } else {
        match py_stmt!("raise StopIteration()") {
            Stmt::Raise(stmt) => stmt,
            _ => unreachable!("expected raise statement"),
        }
    }
}

fn lower_generator_yield_terms_to_explicit_return(
    blocks: &mut [Block],
    block_params: &HashMap<String, Vec<String>>,
    resume_pcs: &[(String, usize)],
    is_async: bool,
) {
    let resume_pc_by_label = resume_pcs
        .iter()
        .cloned()
        .collect::<HashMap<String, usize>>();

    // Existing Ret terminators in generator functions represent completion.
    // Rewrite them to explicit completion exceptions so Ret can represent
    // suspension value returns uniformly.
    for block in blocks.iter_mut() {
        if let Terminator::Ret(value) = &block.terminator {
            block.body.push(py_stmt!(
                "__dp_setattr(_dp_self, \"_pc\", __dp__._GEN_PC_DONE)"
            ));
            block.terminator = Terminator::Raise(raise_done_stmt(is_async, value.clone()));
        }
    }

    // Rewrite yield terminators to explicit state updates plus Ret(value).
    for block in blocks.iter_mut() {
        let (yield_value, resume_label) = match &block.terminator {
            Terminator::Yield {
                value,
                resume_label,
            } => (value.clone(), resume_label.clone()),
            _ => continue,
        };
        let next_pc = *resume_pc_by_label
            .get(resume_label.as_str())
            .unwrap_or_else(|| panic!("missing resume pc for label: {resume_label}"));
        block.body.push(py_stmt!(
            "__dp_setattr(_dp_self, \"_pc\", {next_pc:literal})",
            next_pc = next_pc as i64,
        ));
        let next_state_names = block_params
            .get(resume_label.as_str())
            .cloned()
            .unwrap_or_default();
        for name in next_state_names {
            if matches!(
                name.as_str(),
                "_dp_self" | "_dp_send_value" | "_dp_resume_exc"
            ) {
                continue;
            }
            block.body.push(py_stmt!(
                "__dp_store_local(_dp_self, {name:literal}, {value:id})",
                name = name.as_str(),
                value = name.as_str(),
            ));
        }
        block.terminator = Terminator::Ret(yield_value);
    }
}

fn bb_function_kind_from(kind: &LoweredKind) -> BbFunctionKind {
    match kind {
        LoweredKind::Function => BbFunctionKind::Function,
        LoweredKind::Coroutine => BbFunctionKind::Coroutine,
        LoweredKind::Generator {
            resume_label,
            target_labels,
            resume_pcs,
        } => BbFunctionKind::Generator {
            resume_label: resume_label.clone(),
            target_labels: target_labels.clone(),
            resume_pcs: resume_pcs.clone(),
        },
        LoweredKind::AsyncGenerator {
            resume_label,
            target_labels,
            resume_pcs,
        } => BbFunctionKind::AsyncGenerator {
            resume_label: resume_label.clone(),
            target_labels: target_labels.clone(),
            resume_pcs: resume_pcs.clone(),
        },
    }
}

impl Transformer for BasicBlockRewriter<'_> {
    fn visit_stmt(&mut self, stmt: &mut Stmt) {
        if let Stmt::FunctionDef(func) = stmt {
            let fn_name = func.name.id.to_string();
            let entering_module_init = is_module_init_temp_name(fn_name.as_str());
            self.module_init_hoisted_blocks.push(Vec::new());
            let function_cell_bindings = collect_cell_slots(&func.body.body)
                .into_iter()
                .filter_map(|slot| slot.strip_prefix("_dp_cell_").map(str::to_string))
                .collect::<HashSet<_>>();
            self.function_stack.push(fn_name);
            self.function_cell_bindings_stack
                .push(function_cell_bindings);
            walk_stmt(self, stmt);
            self.function_stack.pop();
            self.function_cell_bindings_stack.pop();
            let mut function_hoisted = self.module_init_hoisted_blocks.pop().unwrap_or_default();

            if let Stmt::FunctionDef(func) = stmt {
                if let Some(lowered) = self.try_lower_function(func) {
                    let identity = self.function_identity_for(func);
                    let resolved_target = self.resolved_binding_target(&identity);
                    let mut local_cell_slots =
                        lowered.local_cell_slots.iter().cloned().collect::<Vec<_>>();
                    local_cell_slots.sort();
                    let bb_function = BbFunction {
                        bind_name: identity.bind_name.clone(),
                        display_name: identity.display_name.clone(),
                        qualname: identity.qualname.clone(),
                        binding_target: resolved_target,
                        is_coroutine: lowered.coroutine_wrapper,
                        kind: bb_function_kind_from(&lowered.kind),
                        entry: lowered.entry_label.clone(),
                        param_names: lowered.param_names.clone(),
                        entry_params: lowered.entry_params.clone(),
                        param_specs: lowered.param_specs.clone(),
                        local_cell_slots,
                        blocks: lowered.blocks.clone(),
                    };
                    self.lowered_functions_ir.push(bb_function.clone());
                    if self.module_init_function.is_none()
                        && identity.bind_name.as_str() == "_dp_module_init"
                    {
                        self.module_init_function = Some(identity.bind_name.clone());
                    }
                    let binding_stmt = self
                        .build_lowered_binding_stmt(func, &bb_function)
                        .expect("failed to build BB function binding");
                    let keep_local_blocks = !entering_module_init
                        && !self.module_init_hoisted_blocks.is_empty()
                        && (identity.bind_name.starts_with("_dp_class_ns_")
                            || identity.bind_name.starts_with("_dp_define_class_"));
                    if entering_module_init {
                        let mut lowered_defs = function_hoisted;
                        lowered_defs.push(binding_stmt);
                        *stmt = into_body(lowered_defs);
                    } else if keep_local_blocks {
                        let mut body = function_hoisted;
                        body.push(binding_stmt);
                        *stmt = into_body(body);
                    } else if !self.module_init_hoisted_blocks.is_empty() {
                        if let Some(hoisted) = self.module_init_hoisted_blocks.last_mut() {
                            hoisted.append(&mut function_hoisted);
                        }
                        *stmt = binding_stmt;
                    } else {
                        let mut body = function_hoisted;
                        body.push(binding_stmt);
                        *stmt = into_body(body);
                    }
                } else {
                    if should_keep_non_lowered_for_annotationlib(func) {
                        rewrite_with_pass(
                            self.context,
                            Some(&AnnotationHelperForLoweringPass),
                            None,
                            &mut func.body,
                        );
                        ensure_dp_default_param(func);
                    }
                    let non_lowered_binding = self.build_non_lowered_binding_stmt(func);
                    if let Some(binding_stmt) = non_lowered_binding {
                        let mut body = Vec::new();
                        body.append(&mut function_hoisted);
                        body.push(Stmt::FunctionDef(func.clone()));
                        body.push(binding_stmt);
                        *stmt = into_body(body);
                    } else if !function_hoisted.is_empty() {
                        let mut new_body = function_hoisted
                            .into_iter()
                            .map(Box::new)
                            .collect::<Vec<_>>();
                        new_body.extend(std::mem::take(&mut func.body.body));
                        func.body.body = new_body;
                    }
                }
            }
            return;
        }

        walk_stmt(self, stmt);
    }
}

fn walk_stmt_body<V: Transformer + ?Sized>(visitor: &mut V, body: &mut StmtBody) {
    for stmt in body.body.iter_mut() {
        visitor.visit_stmt(stmt.as_mut());
    }
}

#[derive(Default)]
struct LoadNameCollector {
    names: HashSet<String>,
}

impl Transformer for LoadNameCollector {
    fn visit_expr(&mut self, expr: &mut Expr) {
        if let Expr::Name(name) = expr {
            if matches!(name.ctx, ast::ExprContext::Load) {
                self.names.insert(name.id.to_string());
            }
        }
        walk_expr(self, expr);
    }
}

fn split_docstring(body: &StmtBody) -> (Option<Stmt>, Vec<Box<Stmt>>) {
    let mut rest = body.body.clone();
    let Some(first) = rest.first() else {
        return (None, rest);
    };
    if matches!(
        first.as_ref(),
        Stmt::Expr(ast::StmtExpr { value, .. }) if matches!(value.as_ref(), Expr::StringLiteral(_))
    ) {
        let first_stmt = *rest.remove(0);
        return (Some(first_stmt), rest);
    }
    (None, rest)
}

fn function_docstring_expr(func: &ast::StmtFunctionDef) -> Option<Expr> {
    let (docstring, _) = split_docstring(&func.body);
    let Some(Stmt::Expr(expr_stmt)) = docstring else {
        return None;
    };
    Some(*expr_stmt.value)
}

fn function_annotation_entries(func: &ast::StmtFunctionDef) -> Vec<(String, Expr, String)> {
    let mut entries = Vec::new();
    let parameters = func.parameters.as_ref();

    for param in &parameters.posonlyargs {
        if let Some(annotation) = param.parameter.annotation.as_ref() {
            entries.push((
                param.parameter.name.id.to_string(),
                *annotation.clone(),
                annotation_expr_string(annotation),
            ));
        }
    }
    for param in &parameters.args {
        if let Some(annotation) = param.parameter.annotation.as_ref() {
            entries.push((
                param.parameter.name.id.to_string(),
                *annotation.clone(),
                annotation_expr_string(annotation),
            ));
        }
    }
    if let Some(vararg) = &parameters.vararg {
        if let Some(annotation) = vararg.annotation.as_ref() {
            entries.push((
                vararg.name.id.to_string(),
                *annotation.clone(),
                annotation_expr_string(annotation),
            ));
        }
    }
    for param in &parameters.kwonlyargs {
        if let Some(annotation) = param.parameter.annotation.as_ref() {
            entries.push((
                param.parameter.name.id.to_string(),
                *annotation.clone(),
                annotation_expr_string(annotation),
            ));
        }
    }
    if let Some(kwarg) = &parameters.kwarg {
        if let Some(annotation) = kwarg.annotation.as_ref() {
            entries.push((
                kwarg.name.id.to_string(),
                *annotation.clone(),
                annotation_expr_string(annotation),
            ));
        }
    }
    if let Some(returns) = func.returns.as_ref() {
        entries.push((
            "return".to_string(),
            *returns.clone(),
            annotation_expr_string(returns),
        ));
    }

    entries
}

fn annotation_expr_string(expr: &Expr) -> String {
    Generator::new(&Indentation::new("    ".to_string()), LineEnding::default()).expr(expr)
}

fn flatten_stmt_boxes(stmts: &[Box<Stmt>]) -> Vec<Box<Stmt>> {
    let mut out = Vec::new();
    for stmt in stmts {
        flatten_stmt(stmt.as_ref(), &mut out);
    }
    out
}

fn strip_nonlocal_directives(stmts: Vec<Box<Stmt>>) -> Vec<Box<Stmt>> {
    stmts
        .into_iter()
        .filter(|stmt| !matches!(stmt.as_ref(), Stmt::Nonlocal(_)))
        .collect()
}

fn should_strip_nonlocal_for_bb(fn_name: &str) -> bool {
    // Generated helper functions (comprehensions/lambdas/etc.) are prefixed
    // `_dp_fn__dp_...` and currently rely on their existing non-BB lowering
    // behavior for closure propagation. Keep nonlocal directives there.
    !fn_name.starts_with("_dp_fn__dp_")
}

fn flatten_stmt(stmt: &Stmt, out: &mut Vec<Box<Stmt>>) {
    if let Stmt::BodyStmt(body) = stmt {
        for child in &body.body {
            flatten_stmt(child.as_ref(), out);
        }
        return;
    }
    out.push(Box::new(stmt.clone()));
}

fn extract_else_body(if_stmt: &ast::StmtIf) -> Vec<Box<Stmt>> {
    if if_stmt.elif_else_clauses.is_empty() {
        return Vec::new();
    }
    if_stmt
        .elif_else_clauses
        .first()
        .map(|clause| clause.body.body.clone())
        .unwrap_or_default()
}

fn collect_parameter_names(parameters: &ast::Parameters) -> Vec<String> {
    let mut names = Vec::new();
    for param in &parameters.posonlyargs {
        names.push(param.parameter.name.id.to_string());
    }
    for param in &parameters.args {
        names.push(param.parameter.name.id.to_string());
    }
    if let Some(vararg) = &parameters.vararg {
        names.push(vararg.name.id.to_string());
    }
    for param in &parameters.kwonlyargs {
        names.push(param.parameter.name.id.to_string());
    }
    if let Some(kwarg) = &parameters.kwarg {
        names.push(kwarg.name.id.to_string());
    }
    names
}

fn collect_state_vars(
    param_names: &[String],
    blocks: &[Block],
    module_init_mode: bool,
) -> Vec<String> {
    let mut defs_anywhere = HashSet::new();
    let mut injected_exception_names = HashSet::new();
    for block in blocks {
        for stmt in &block.body {
            defs_anywhere.extend(assigned_names_in_stmt(stmt));
        }
        if let Terminator::TryJump {
            except_exc_name,
            finally_exc_name,
            ..
        } = &block.terminator
        {
            if let Some(name) = except_exc_name.as_ref() {
                injected_exception_names.insert(name.clone());
            }
            if let Some(name) = finally_exc_name.as_ref() {
                injected_exception_names.insert(name.clone());
            }
        }
    }

    let mut state = param_names.to_vec();
    for block in blocks {
        let (uses, defs) = analyze_block_use_def(block);
        let mut names = defs.into_iter().collect::<Vec<_>>();
        for name in uses {
            let is_special_runtime_state = name == "_dp_self"
                || name.starts_with("_dp_cell_")
                || name.starts_with("_dp_try_exc_")
                || name == "_dp_classcell";
            let is_known_local = defs_anywhere.contains(name.as_str())
                || injected_exception_names.contains(name.as_str())
                || param_names.iter().any(|param| param == &name);
            let include = if module_init_mode {
                is_special_runtime_state || is_known_local
            } else {
                is_special_runtime_state || is_known_local
            };
            if include {
                names.push(name);
            }
        }
        names.sort();
        names.dedup();
        for name in names {
            if !state.iter().any(|existing| existing == &name) {
                state.push(name);
            }
        }
    }
    state
}

fn load_names_in_stmt(stmt: &Stmt) -> HashSet<String> {
    match stmt {
        Stmt::Expr(expr_stmt) => load_names_in_expr(expr_stmt.value.as_ref()),
        Stmt::Assign(assign) => load_names_in_expr(assign.value.as_ref()),
        Stmt::Raise(raise_stmt) => {
            let mut names = HashSet::new();
            if let Some(exc) = raise_stmt.exc.as_ref() {
                names.extend(load_names_in_expr(exc.as_ref()));
            }
            if let Some(cause) = raise_stmt.cause.as_ref() {
                names.extend(load_names_in_expr(cause.as_ref()));
            }
            names
        }
        Stmt::If(if_stmt) => {
            let mut names = load_names_in_expr(if_stmt.test.as_ref());
            for stmt in &if_stmt.body.body {
                names.extend(load_names_in_stmt(stmt.as_ref()));
            }
            for clause in &if_stmt.elif_else_clauses {
                if let Some(test) = clause.test.as_ref() {
                    names.extend(load_names_in_expr(test));
                }
                for stmt in &clause.body.body {
                    names.extend(load_names_in_stmt(stmt.as_ref()));
                }
            }
            names
        }
        Stmt::While(while_stmt) => {
            let mut names = load_names_in_expr(while_stmt.test.as_ref());
            for stmt in &while_stmt.body.body {
                names.extend(load_names_in_stmt(stmt.as_ref()));
            }
            for stmt in &while_stmt.orelse.body {
                names.extend(load_names_in_stmt(stmt.as_ref()));
            }
            names
        }
        Stmt::For(for_stmt) => {
            let mut names = load_names_in_expr(for_stmt.iter.as_ref());
            names.extend(load_names_in_expr(for_stmt.target.as_ref()));
            for stmt in &for_stmt.body.body {
                names.extend(load_names_in_stmt(stmt.as_ref()));
            }
            for stmt in &for_stmt.orelse.body {
                names.extend(load_names_in_stmt(stmt.as_ref()));
            }
            names
        }
        Stmt::Try(try_stmt) => {
            let mut names = HashSet::new();
            let mut defs = HashSet::new();
            for stmt in &try_stmt.body.body {
                names.extend(load_names_in_stmt(stmt.as_ref()));
                defs.extend(assigned_names_in_stmt(stmt.as_ref()));
            }
            for handler in &try_stmt.handlers {
                let ast::ExceptHandler::ExceptHandler(handler) = handler;
                if let Some(type_) = handler.type_.as_ref() {
                    names.extend(load_names_in_expr(type_.as_ref()));
                }
                for stmt in &handler.body.body {
                    names.extend(load_names_in_stmt(stmt.as_ref()));
                    defs.extend(assigned_names_in_stmt(stmt.as_ref()));
                }
            }
            for stmt in &try_stmt.orelse.body {
                names.extend(load_names_in_stmt(stmt.as_ref()));
                defs.extend(assigned_names_in_stmt(stmt.as_ref()));
            }
            for stmt in &try_stmt.finalbody.body {
                names.extend(load_names_in_stmt(stmt.as_ref()));
                defs.extend(assigned_names_in_stmt(stmt.as_ref()));
            }
            names.retain(|name| {
                !defs.contains(name) || name.starts_with("_dp_cell_") || name == "_dp_classcell"
            });
            names
        }
        Stmt::Delete(delete_stmt) => {
            let mut names = HashSet::new();
            for target in &delete_stmt.targets {
                names.extend(load_names_in_expr(target));
            }
            names
        }
        Stmt::FunctionDef(func_def) => {
            // A function definition evaluates only header-time expressions
            // (decorators/defaults/annotations/type params) when the `def`
            // statement runs.
            let mut header_only = func_def.clone();
            header_only.body.body.clear();
            let mut stmt = Stmt::FunctionDef(header_only);
            let mut collector = LoadNameCollector::default();
            collector.visit_stmt(&mut stmt);
            let mut names = collector.names;

            if !func_def.name.id.as_str().starts_with("_dp_bb_") {
                // Nested transformed non-BB helper functions can require outer
                // closure cells at definition time so the created function
                // captures those cells. BB helper defs thread cells explicitly
                // via parameters/closure tuples and should not force the outer
                // function's entry-params.
                let mut full_stmt = Stmt::FunctionDef(func_def.clone());
                let mut body_collector = LoadNameCollector::default();
                body_collector.visit_stmt(&mut full_stmt);
                for name in body_collector.names {
                    if name.starts_with("_dp_cell_") {
                        names.insert(name);
                    }
                }
            }

            names
        }
        Stmt::Return(ret) => ret
            .value
            .as_ref()
            .map(|value| load_names_in_expr(value.as_ref()))
            .unwrap_or_default(),
        _ => HashSet::new(),
    }
}

fn load_names_in_terminator(terminator: &Terminator) -> HashSet<String> {
    match terminator {
        Terminator::BrIf { test, .. } => load_names_in_expr(test),
        Terminator::BrTable { index, .. } => load_names_in_expr(index),
        Terminator::Raise(raise_stmt) => {
            let mut names = HashSet::new();
            if let Some(exc) = raise_stmt.exc.as_ref() {
                names.extend(load_names_in_expr(exc.as_ref()));
            }
            if let Some(cause) = raise_stmt.cause.as_ref() {
                names.extend(load_names_in_expr(cause.as_ref()));
            }
            names
        }
        Terminator::TryJump { .. } => HashSet::new(),
        Terminator::Yield { value, .. } => {
            value.as_ref().map(load_names_in_expr).unwrap_or_default()
        }
        Terminator::Ret(Some(value)) => load_names_in_expr(value),
        Terminator::Jump(_) | Terminator::Ret(None) => HashSet::new(),
    }
}

fn assigned_names_in_stmt(stmt: &Stmt) -> HashSet<String> {
    let mut names = HashSet::new();
    match stmt {
        // TODO(#2): model `del` as a kill-set in BB liveness instead of only
        // tracking defs. Without kills, deleted locals can be threaded across
        // block boundaries and incorrectly remain live.
        Stmt::Assign(assign) => {
            for target in &assign.targets {
                collect_assigned_names(target, &mut names);
            }
        }
        Stmt::If(if_stmt) => {
            for stmt in &if_stmt.body.body {
                names.extend(assigned_names_in_stmt(stmt.as_ref()));
            }
            for clause in &if_stmt.elif_else_clauses {
                for stmt in &clause.body.body {
                    names.extend(assigned_names_in_stmt(stmt.as_ref()));
                }
            }
        }
        Stmt::While(while_stmt) => {
            for stmt in &while_stmt.body.body {
                names.extend(assigned_names_in_stmt(stmt.as_ref()));
            }
            for stmt in &while_stmt.orelse.body {
                names.extend(assigned_names_in_stmt(stmt.as_ref()));
            }
        }
        Stmt::For(for_stmt) => {
            collect_assigned_names(for_stmt.target.as_ref(), &mut names);
            for stmt in &for_stmt.body.body {
                names.extend(assigned_names_in_stmt(stmt.as_ref()));
            }
            for stmt in &for_stmt.orelse.body {
                names.extend(assigned_names_in_stmt(stmt.as_ref()));
            }
        }
        Stmt::Try(try_stmt) => {
            for stmt in &try_stmt.body.body {
                names.extend(assigned_names_in_stmt(stmt.as_ref()));
            }
            for handler in &try_stmt.handlers {
                let ast::ExceptHandler::ExceptHandler(handler) = handler;
                for stmt in &handler.body.body {
                    names.extend(assigned_names_in_stmt(stmt.as_ref()));
                }
            }
            for stmt in &try_stmt.orelse.body {
                names.extend(assigned_names_in_stmt(stmt.as_ref()));
            }
            for stmt in &try_stmt.finalbody.body {
                names.extend(assigned_names_in_stmt(stmt.as_ref()));
            }
        }
        Stmt::FunctionDef(func_def) => {
            names.insert(func_def.name.id.to_string());
        }
        _ => {}
    }
    names
}

fn collect_assigned_names(target: &Expr, names: &mut HashSet<String>) {
    match target {
        Expr::Name(name) => {
            names.insert(name.id.to_string());
        }
        Expr::Tuple(tuple) => {
            for elt in &tuple.elts {
                collect_assigned_names(elt, names);
            }
        }
        Expr::List(list) => {
            for elt in &list.elts {
                collect_assigned_names(elt, names);
            }
        }
        Expr::Starred(starred) => collect_assigned_names(starred.value.as_ref(), names),
        _ => {}
    }
}

fn collect_cell_slots(stmts: &[Box<Stmt>]) -> HashSet<String> {
    let mut slots = HashSet::new();
    for stmt in stmts {
        let mut names = assigned_names_in_stmt(stmt.as_ref());
        for name in names.drain() {
            if name.starts_with("_dp_cell_") {
                slots.insert(name);
            }
        }
    }
    slots
}

fn sync_target_cells_stmts(target: &Expr, cell_slots: &HashSet<String>) -> Vec<Stmt> {
    let mut names = HashSet::new();
    collect_assigned_names(target, &mut names);
    let mut names = names.into_iter().collect::<Vec<_>>();
    names.sort();

    names
        .into_iter()
        .filter_map(|name| {
            let cell = cell_name(name.as_str());
            if !cell_slots.contains(cell.as_str()) {
                return None;
            }
            Some(py_stmt!(
                "__dp_store_cell({cell:id}, {value:id})",
                cell = cell.as_str(),
                value = name.as_str(),
            ))
        })
        .collect()
}

fn block_starts_with_resume_value_assign(block: &Block) -> bool {
    let Some(Stmt::Assign(assign)) = block.body.first() else {
        return false;
    };
    matches!(
        assign.value.as_ref(),
        Expr::Name(name) if matches!(name.id.as_str(), "_dp_send_value" | "_dp_resume_exc")
    )
}

fn compute_throw_dispatch_by_label(blocks: &[Block], entry_label: &str) -> HashMap<String, String> {
    let mut best: HashMap<String, (usize, String)> = HashMap::new();
    for block in blocks {
        if block.label.as_str() == entry_label {
            continue;
        }
        let Terminator::TryJump {
            body_region_labels, ..
        } = &block.terminator
        else {
            continue;
        };
        let rank = body_region_labels.len();
        for label in body_region_labels {
            let update = match best.get(label.as_str()) {
                Some((best_rank, _)) => rank < *best_rank,
                None => true,
            };
            if update {
                best.insert(label.clone(), (rank, block.label.clone()));
            }
        }
    }
    best.into_iter()
        .map(|(label, (_, dispatch))| (label, dispatch))
        .collect()
}

fn compute_resume_throw_dispatch_by_label(blocks: &[Block]) -> HashMap<String, String> {
    let mut out = HashMap::new();
    for block in blocks {
        let Terminator::BrIf {
            test,
            then_label,
            else_label,
        } = &block.terminator
        else {
            continue;
        };
        let Expr::Call(call) = test else {
            continue;
        };
        if !call.arguments.keywords.is_empty() || call.arguments.args.len() != 2 {
            continue;
        }
        let Expr::Name(func_name) = call.func.as_ref() else {
            continue;
        };
        let Expr::Name(first_arg) = &call.arguments.args[0] else {
            continue;
        };
        if first_arg.id.as_str() != "_dp_resume_exc" {
            continue;
        }
        let Expr::NoneLiteral(_) = &call.arguments.args[1] else {
            continue;
        };

        let target = match func_name.id.as_str() {
            "__dp_is_" => else_label.clone(),
            "__dp_is_not" => then_label.clone(),
            _ => continue,
        };
        out.insert(block.label.clone(), target);
    }
    out
}

fn load_names_in_expr(expr: &Expr) -> HashSet<String> {
    let mut expr = expr.clone();
    let mut collector = LoadNameCollector::default();
    collector.visit_expr(&mut expr);
    collector.names
}

fn stmt_body_from_stmts(stmts: Vec<Stmt>) -> StmtBody {
    StmtBody {
        body: stmts.into_iter().map(Box::new).collect(),
        range: TextRange::default(),
        node_index: ast::AtomicNodeIndex::default(),
    }
}

fn push_param_specs(
    specs: &mut Vec<Expr>,
    name: &str,
    prefix: &str,
    _annotation: Option<&Expr>,
    default: Option<&Expr>,
) {
    let label = format!("{prefix}{name}");
    let annotation_expr = py_expr!("None");
    let default_expr = default
        .cloned()
        .unwrap_or_else(|| py_expr!("__dp__.NO_DEFAULT"));
    specs.push(make_dp_tuple(vec![
        py_expr!("{value:literal}", value = label.as_str()),
        annotation_expr,
        default_expr,
    ]));
}

fn make_param_specs_expr(parameters: &ast::Parameters) -> Expr {
    let mut specs = Vec::new();
    for param in &parameters.posonlyargs {
        push_param_specs(
            &mut specs,
            param.parameter.name.id.as_str(),
            "/",
            param.parameter.annotation.as_deref(),
            param.default.as_deref(),
        );
    }
    for param in &parameters.args {
        push_param_specs(
            &mut specs,
            param.parameter.name.id.as_str(),
            "",
            param.parameter.annotation.as_deref(),
            param.default.as_deref(),
        );
    }
    if let Some(param) = &parameters.vararg {
        push_param_specs(
            &mut specs,
            param.name.id.as_str(),
            "*",
            param.annotation.as_deref(),
            None,
        );
    }
    for param in &parameters.kwonlyargs {
        push_param_specs(
            &mut specs,
            param.parameter.name.id.as_str(),
            "kw:",
            param.parameter.annotation.as_deref(),
            param.default.as_deref(),
        );
    }
    if let Some(param) = &parameters.kwarg {
        push_param_specs(
            &mut specs,
            param.name.id.as_str(),
            "**",
            param.annotation.as_deref(),
            None,
        );
    }
    make_dp_tuple(specs)
}

fn make_dp_tuple(items: Vec<Expr>) -> Expr {
    let Expr::Call(mut call) = py_expr!("__dp_tuple()") else {
        panic!("expected call expression for __dp_tuple");
    };
    call.arguments.args = items.into();
    Expr::Call(call)
}

fn raise_stmt_from_name(name: &str) -> ast::StmtRaise {
    match py_stmt!("raise {exc:id}", exc = name) {
        Stmt::Raise(raise_stmt) => raise_stmt,
        _ => unreachable!("expected raise statement"),
    }
}

fn rewrite_exception_accesses(mut body: Vec<Box<Stmt>>, exc_name: &str) -> Vec<Box<Stmt>> {
    let mut rewriter = ExceptExceptionRewriter {
        exception_name: exc_name.to_string(),
    };
    for stmt in body.iter_mut() {
        rewriter.visit_stmt(stmt.as_mut());
    }
    body
}

fn body_uses_name(body: &[Box<Stmt>], name: &str) -> bool {
    body.iter()
        .any(|stmt| load_names_in_stmt(stmt.as_ref()).contains(name))
}

struct ExceptExceptionRewriter {
    exception_name: String,
}

impl ExceptExceptionRewriter {
    fn exception_name_expr(&self) -> Expr {
        py_expr!("{name:id}", name = self.exception_name.as_str())
    }
}

impl Transformer for ExceptExceptionRewriter {
    fn visit_stmt(&mut self, stmt: &mut Stmt) {
        match stmt {
            Stmt::FunctionDef(_) | Stmt::ClassDef(_) => {}
            Stmt::Raise(raise_stmt) if raise_stmt.exc.is_none() && raise_stmt.cause.is_none() => {
                raise_stmt.exc = Some(Box::new(self.exception_name_expr()));
            }
            _ => walk_stmt(self, stmt),
        }
    }

    fn visit_expr(&mut self, expr: &mut Expr) {
        if let Expr::Call(call) = expr {
            if call.arguments.args.is_empty() && call.arguments.keywords.is_empty() {
                if is_dp_lookup_call(call.func.as_ref(), "current_exception") {
                    *expr = self.exception_name_expr();
                    return;
                }
                if is_dp_lookup_call(call.func.as_ref(), "exc_info") {
                    *expr = py_expr!(
                        "__dp_exc_info_from_exception({exc:id})",
                        exc = self.exception_name.as_str(),
                    );
                    return;
                }
            }
        }
        walk_expr(self, expr);
    }
}

fn is_dp_lookup_call(func: &Expr, attr_name: &str) -> bool {
    if matches!(
        func,
        Expr::Name(name) if name.id.as_str() == format!("__dp_{attr_name}")
    ) {
        return true;
    }
    if let Expr::Attribute(attr) = func {
        if attr.attr.as_str() == attr_name {
            if let Expr::Name(module) = attr.value.as_ref() {
                return module.id.as_str() == "__dp__";
            }
        }
    }
    if let Expr::Call(call) = func {
        if !call.arguments.keywords.is_empty() || call.arguments.args.len() != 2 {
            return false;
        }
        if !matches!(
            call.func.as_ref(),
            Expr::Name(name) if name.id.as_str() == "__dp_getattr"
        ) {
            return false;
        }
        let base_matches = matches!(
            &call.arguments.args[0],
            Expr::Name(base) if base.id.as_str() == "__dp__"
        );
        if !base_matches {
            return false;
        }
        return expr_static_str(&call.arguments.args[1]).as_deref() == Some(attr_name);
    }
    false
}

fn expr_static_str(expr: &Expr) -> Option<String> {
    match expr {
        Expr::StringLiteral(value) => Some(value.value.to_str().to_string()),
        Expr::Call(call)
            if call.arguments.keywords.is_empty()
                && call.arguments.args.len() == 1
                && matches!(
                    call.func.as_ref(),
                    Expr::Name(name)
                        if matches!(
                            name.id.as_str(),
                            "__dp_decode_literal_bytes" | "__dp_decode_literal_source_bytes"
                        )
                ) =>
        {
            match &call.arguments.args[0] {
                Expr::BytesLiteral(bytes) => {
                    let value: std::borrow::Cow<[u8]> = (&bytes.value).into();
                    String::from_utf8(value.into_owned()).ok()
                }
                _ => None,
            }
        }
        _ => None,
    }
}

fn name_expr(name: &str) -> Option<Expr> {
    parse_expression(name)
        .ok()
        .map(|expr| *expr.into_syntax().body)
}

#[cfg(test)]
mod tests {
    use super::{BbExpr, BbFunction, BbOp, BbTerm};
    use crate::{
        py_expr, transform::Options, transform_str_to_bb_ir_with_options,
        transform_str_to_ruff_with_options,
    };

    fn contains_dp_call(lowered: &str, name: &str) -> bool {
        lowered.contains(&format!("__dp_{name}("))
            || lowered.contains(&format!("__dp_getattr(__dp__, \"{name}\")("))
            || lowered.contains(&format!(
                "__dp_getattr(__dp__, __dp_decode_literal_bytes(b\"{name}\"))("
            ))
    }

    fn function_by_name<'a>(bb_module: &'a super::BbModule, bind_name: &str) -> &'a BbFunction {
        bb_module
            .functions
            .iter()
            .find(|func| func.bind_name == bind_name)
            .unwrap_or_else(|| panic!("missing lowered function {bind_name}; got {:?}", bb_module))
    }

    fn expr_text(expr: &BbExpr) -> String {
        crate::ruff_ast_to_string(&expr.to_expr())
    }

    fn block_uses_text(block: &super::BbBlock, needle: &str) -> bool {
        block.ops.iter().any(|op| match op {
            BbOp::Assign(assign) => expr_text(&assign.value).contains(needle),
            BbOp::Expr(expr) => expr_text(&expr.value).contains(needle),
            BbOp::Delete(delete) => delete
                .targets
                .iter()
                .any(|expr| expr_text(expr).contains(needle)),
        }) || match &block.term {
            BbTerm::BrIf { test, .. } => expr_text(test).contains(needle),
            BbTerm::BrTable { index, .. } => expr_text(index).contains(needle),
            BbTerm::Raise { exc, cause } => {
                exc.as_ref()
                    .is_some_and(|value| expr_text(value).contains(needle))
                    || cause
                        .as_ref()
                        .is_some_and(|value| expr_text(value).contains(needle))
            }
            BbTerm::Ret(value) => value
                .as_ref()
                .is_some_and(|ret| expr_text(ret).contains(needle)),
            _ => false,
        }
    }

    #[test]
    fn lowers_simple_if_function_into_basic_blocks() {
        let source = r#"
def foo(a, b):
    c = a + b
    if c > 5:
        print("hi", c)
    else:
        d = b + 1
        print(d)
"#;

        let options = Options {
            inject_import: false,
            ..Options::for_test()
        };
        let bb_module = transform_str_to_bb_ir_with_options(source, options)
            .expect("transform should succeed")
            .expect("bb module should be available");
        let foo = function_by_name(&bb_module, "foo");
        assert!(foo.blocks.len() >= 3, "{foo:?}");
        assert!(
            foo.blocks
                .iter()
                .any(|block| matches!(block.term, BbTerm::BrIf { .. })),
            "{foo:?}"
        );
    }

    #[test]
    fn exposes_bb_ir_for_lowered_functions() {
        let source = r#"
def foo(a, b):
    if a:
        return b
    return a
"#;

        let options = Options {
            inject_import: false,
            ..Options::for_test()
        };
        let bb_module = transform_str_to_bb_ir_with_options(source, options)
            .expect("transform should succeed")
            .expect("bb module should be available");
        let foo = bb_module
            .functions
            .iter()
            .find(|func| func.bind_name == "foo")
            .expect("foo should be lowered");
        assert!(foo.entry.starts_with("_dp_bb_"), "{:?}", foo.entry);
        assert!(!foo.blocks.is_empty());
    }

    #[test]
    fn lowers_while_break_continue_into_basic_blocks() {
        let source = r#"
def run(limit):
    i = 0
    out = []
    while i < limit:
        i = i + 1
        if i == 2:
            continue
        if i == 5:
            break
        out.append(i)
    else:
        out.append(99)
    return out, i
"#;

        let options = Options {
            inject_import: false,
            ..Options::for_test()
        };
        let bb_module = transform_str_to_bb_ir_with_options(source, options)
            .expect("transform should succeed")
            .expect("bb module should be available");
        let run = function_by_name(&bb_module, "run");
        assert!(
            run.blocks
                .iter()
                .any(|block| matches!(block.term, BbTerm::BrIf { .. })),
            "{run:?}"
        );
        assert!(
            run.blocks
                .iter()
                .any(|block| matches!(block.term, BbTerm::Jump(_))),
            "{run:?}"
        );
    }

    #[test]
    fn lowers_for_else_break_into_basic_blocks() {
        let source = r#"
def run(items):
    out = []
    for x in items:
        if x == 2:
            break
        out.append(x)
    else:
        out.append(99)
    return out
"#;

        let options = Options {
            inject_import: false,
            ..Options::for_test()
        };
        let bb_module = transform_str_to_bb_ir_with_options(source, options)
            .expect("transform should succeed")
            .expect("bb module should be available");
        let run = function_by_name(&bb_module, "run");
        assert!(
            run.blocks
                .iter()
                .any(|block| block_uses_text(block, "__dp_next_or_sentinel")),
            "{run:?}"
        );
        assert!(
            run.blocks
                .iter()
                .any(|block| block_uses_text(block, "__dp_iter")),
            "{run:?}"
        );
        assert!(
            run.blocks
                .iter()
                .any(|block| matches!(block.term, BbTerm::BrIf { .. })),
            "{run:?}"
        );
    }

    #[test]
    fn lowers_async_for_else_directly_without_completed_flag() {
        let source = r#"
async def run():
    async for x in ait:
        body()
    else:
        done()
"#;

        let options = Options {
            inject_import: false,
            ..Options::for_test()
        };
        let bb_module = transform_str_to_bb_ir_with_options(source, options)
            .expect("transform should succeed")
            .expect("bb module should be available");
        let run = function_by_name(&bb_module, "run");
        let debug = format!("{run:?}");
        assert!(
            run.blocks
                .iter()
                .any(|block| block_uses_text(block, "__dp_anext_or_sentinel")),
            "{run:?}"
        );
        assert!(
            run.blocks
                .iter()
                .any(|block| block_uses_text(block, "__dp_aiter")),
            "{run:?}"
        );
        assert!(!debug.contains("_dp_completed_"), "{debug}");
    }

    #[test]
    fn omits_synthetic_end_block_when_unreachable() {
        let source = r#"
def f():
    return 1
"#;

        let options = Options {
            inject_import: false,
            ..Options::for_test()
        };
        let bb_module = transform_str_to_bb_ir_with_options(source, options)
            .expect("transform should succeed")
            .expect("bb module should be available");
        let f = function_by_name(&bb_module, "f");
        assert!(f.entry == "_dp_bb_f_start", "{f:?}");
        assert!(
            !f.blocks.iter().any(|block| block.label == "_dp_bb_f_0"),
            "{f:?}"
        );
    }

    #[test]
    fn folds_jump_to_trivial_none_return() {
        let source = r#"
def f():
    x = 1
"#;

        let options = Options {
            inject_import: false,
            ..Options::for_test()
        };
        let bb_module = transform_str_to_bb_ir_with_options(source, options)
            .expect("transform should succeed")
            .expect("bb module should be available");
        let f = function_by_name(&bb_module, "f");
        assert!(
            f.blocks
                .iter()
                .any(|block| matches!(block.term, BbTerm::Ret(None))),
            "{f:?}"
        );
        assert!(
            !f.blocks
                .iter()
                .any(|block| matches!(block.term, BbTerm::Jump(_))),
            "{f:?}"
        );
    }

    #[test]
    fn lowers_outer_with_nested_nonlocal_inner() {
        let source = r#"
def outer():
    x = 5
    def inner():
        nonlocal x
        x = 2
        return x
    return inner()
"#;

        let options = Options {
            inject_import: false,
            ..Options::for_test()
        };
        let bb_module = transform_str_to_bb_ir_with_options(source, options)
            .expect("transform should succeed")
            .expect("bb module should be available");
        let outer = function_by_name(&bb_module, "outer");
        let inner = function_by_name(&bb_module, "inner");
        assert!(outer.entry == "_dp_bb_outer_start", "{outer:?}");
        assert!(inner.entry == "_dp_bb_inner_start", "{inner:?}");
        assert!(
            outer
                .blocks
                .iter()
                .any(|block| block_uses_text(block, "_dp_cell_x")),
            "{outer:?}"
        );
    }

    #[test]
    fn lowers_try_finally_with_return_via_dispatch() {
        let source = r#"
def f(x):
    try:
        if x:
            return 1
    finally:
        cleanup()
    return 2
"#;

        let options = Options {
            inject_import: false,
            ..Options::for_test()
        };
        let bb_module = transform_str_to_bb_ir_with_options(source, options)
            .expect("transform should succeed")
            .expect("bb module should be available");
        let f = function_by_name(&bb_module, "f");
        assert!(
            f.blocks
                .iter()
                .any(|block| block.exc_target_label.is_some()),
            "{f:?}"
        );
        let debug = format!("{f:?}");
        assert!(!debug.contains("finally:"), "{debug}");
    }

    #[test]
    fn lowers_plain_try_except_with_try_jump_dispatch() {
        let source = r#"
try:
    print(1)
except Exception:
    print(2)
"#;

        let options = Options {
            inject_import: false,
            ..Options::for_test()
        };
        let bb_module = transform_str_to_bb_ir_with_options(source, options)
            .expect("transform should succeed")
            .expect("bb module should be available");
        let module_init = bb_module
            .module_init
            .as_ref()
            .expect("module init should be present");
        let init_fn = function_by_name(&bb_module, module_init);
        assert!(
            init_fn
                .blocks
                .iter()
                .any(|block| block.exc_target_label.is_some()),
            "{init_fn:?}"
        );
    }

    #[test]
    fn lowers_try_star_except_star_via_exceptiongroup_split() {
        let source = r#"
def f():
    try:
        raise ExceptionGroup("eg", [ValueError(1)])
    except* ValueError as exc:
        return exc
"#;

        let options = Options {
            inject_import: false,
            ..Options::for_test()
        };
        let bb_module = transform_str_to_bb_ir_with_options(source, options)
            .expect("transform should succeed")
            .expect("bb module should be available");
        let f = function_by_name(&bb_module, "f");
        assert!(
            f.blocks
                .iter()
                .any(|block| block_uses_text(block, "__dp_exceptiongroup_split")),
            "{f:?}"
        );
        assert!(
            f.blocks
                .iter()
                .any(|block| block.exc_target_label.is_some()),
            "{f:?}"
        );
    }

    #[test]
    fn dead_tail_local_binding_still_raises_unbound() {
        let source = r#"
def f():
    print(x)
    return
    x = 1
"#;

        let options = Options {
            inject_import: false,
            ..Options::for_test()
        };
        let bb_module = transform_str_to_bb_ir_with_options(source, options)
            .expect("transform should succeed")
            .expect("bb module should be available");
        let f = function_by_name(&bb_module, "f");
        let debug = format!("{f:?}");
        assert!(debug.contains("load_deleted_name"), "{debug}");
        assert!(debug.contains("DELETED"), "{debug}");
        assert!(!debug.contains("x = 1"), "{debug}");
    }

    #[test]
    fn matches_dp_lookup_call_with_decoded_name_arg() {
        let expr =
            py_expr!("__dp_getattr(__dp__, __dp_decode_literal_bytes(b\"current_exception\"))");
        assert!(super::is_dp_lookup_call(&expr, "current_exception"));
    }
}
