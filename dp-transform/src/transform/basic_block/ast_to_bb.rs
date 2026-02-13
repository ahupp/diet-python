use super::render_py;
use crate::bb_ir::{BbBindingTarget, BbBlock, BbFunction, BbFunctionKind, BbModule, BbTerm};
use crate::template::{empty_body, into_body};
use crate::transform::context::Context;
use crate::transform::rewrite_import;
use crate::transform::scope::{
    analyze_module_scope, cell_name, is_internal_symbol, BindingKind, BindingUse, Scope, ScopeKind,
};
use crate::transform::util::strip_synthetic_module_init_qualname;
use crate::transform::{
    ast_rewrite::{rewrite_with_pass, Rewrite, StmtRewritePass},
    rewrite_expr::make_tuple,
    rewrite_stmt,
};
use crate::transformer::{walk_expr, walk_stmt, Transformer};
use crate::{py_expr, py_stmt};
use ruff_python_ast::{self as ast, name::Name, Expr, NodeIndex, Stmt, StmtBody};
use ruff_python_codegen::{Generator, Indentation};
use ruff_python_parser::parse_expression;
use ruff_source_file::LineEnding;
use ruff_text_size::TextRange;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

pub struct BBSimplifyStmtPass;
struct AnnotationHelperForLoweringPass;
pub type FunctionIdentityByNode = HashMap<NodeIndex, (String, String, String, BindingTarget)>;

pub(crate) fn lower_stmt_default(context: &Context, stmt: Stmt) -> Rewrite {
    match stmt {
        Stmt::With(with) => rewrite_stmt::with::rewrite(context, with),
        Stmt::While(while_stmt) => rewrite_stmt::loop_cond::rewrite_while(context, while_stmt),
        Stmt::For(for_stmt) => rewrite_stmt::loop_cond::rewrite_for(context, for_stmt),
        Stmt::Try(try_stmt) => rewrite_stmt::exception::rewrite_try(try_stmt),
        Stmt::If(if_stmt) => rewrite_stmt::loop_cond::expand_if_chain(if_stmt),
        Stmt::Assert(assert) => rewrite_stmt::assert::rewrite(assert),
        Stmt::Match(match_stmt) => rewrite_stmt::match_case::rewrite(context, match_stmt),
        Stmt::Import(import) => rewrite_import::rewrite(import),
        Stmt::ImportFrom(import_from) => rewrite_import::rewrite_from(context, import_from),
        Stmt::Assign(assign) => {
            if is_take_args_unpack_assign(&assign) {
                Rewrite::Unmodified(Stmt::Assign(assign))
            } else {
                rewrite_stmt::assign_del::rewrite_assign(context, assign)
            }
        }
        Stmt::AugAssign(aug) => rewrite_stmt::assign_del::rewrite_aug_assign(context, aug),
        Stmt::Delete(del) => rewrite_stmt::assign_del::rewrite_delete(del),
        Stmt::Raise(raise) => rewrite_stmt::exception::rewrite_raise(raise),
        Stmt::TypeAlias(type_alias) => {
            rewrite_stmt::type_alias::rewrite_type_alias(context, type_alias)
        }
        Stmt::AnnAssign(_) => {
            panic!("should be removed by rewrite_ann_assign_to_dunder_annotate")
        }
        other => Rewrite::Unmodified(other),
    }
}

fn is_take_args_unpack_assign(assign: &ast::StmtAssign) -> bool {
    if assign.targets.len() != 1 {
        return false;
    }
    let target = &assign.targets[0];
    let is_unpack_target = match target {
        Expr::Tuple(ast::ExprTuple { elts, .. }) | Expr::List(ast::ExprList { elts, .. }) => {
            !elts.is_empty() && elts.iter().all(|elt| matches!(elt, Expr::Name(_)))
        }
        _ => false,
    };
    if !is_unpack_target {
        return false;
    }

    let Expr::Call(call) = assign.value.as_ref() else {
        return false;
    };
    if !call.arguments.keywords.is_empty() || call.arguments.args.len() != 1 {
        return false;
    }
    let Expr::Name(arg_name) = &call.arguments.args[0] else {
        return false;
    };
    if arg_name.id.as_str() != "_dp_args_ptr" {
        return false;
    }
    let Expr::Attribute(attr) = call.func.as_ref() else {
        return false;
    };
    let Expr::Name(module_name) = attr.value.as_ref() else {
        return false;
    };
    module_name.id.as_str() == "__dp__" && attr.attr.as_str() == "take_args"
}

pub(crate) fn lower_stmt_bb(context: &Context, stmt: Stmt) -> Rewrite {
    match stmt {
        Stmt::With(with_stmt) => rewrite_with_for_bb(context, with_stmt),
        Stmt::Try(try_stmt) => lower_stmt_default(context, Stmt::Try(try_stmt)),
        Stmt::For(for_stmt) => {
            let in_async_fn = context.current_scope().in_async_function;
            if in_async_fn || for_stmt.is_async {
                lower_stmt_default(context, Stmt::For(for_stmt))
            } else {
                Rewrite::Unmodified(Stmt::For(for_stmt))
            }
        }
        other => lower_stmt_default(context, other),
    }
}

impl StmtRewritePass for AnnotationHelperForLoweringPass {
    fn lower_stmt(&self, context: &Context, stmt: Stmt) -> Rewrite {
        match stmt {
            Stmt::For(for_stmt) => lower_stmt_default(context, Stmt::For(for_stmt)),
            other => Rewrite::Unmodified(other),
        }
    }
}

fn rewrite_with_for_bb(context: &Context, with_stmt: ast::StmtWith) -> Rewrite {
    if with_stmt.is_async {
        return rewrite_stmt::with::rewrite(context, with_stmt);
    }
    if with_stmt.items.is_empty() {
        return Rewrite::Unmodified(with_stmt.into());
    }

    let ast::StmtWith { items, body, .. } = with_stmt;
    let mut body: Stmt = body.into();

    for ast::WithItem {
        context_expr,
        optional_vars,
        ..
    } in items.into_iter().rev()
    {
        let target = optional_vars.map(|var| *var);
        let exit_name = context.fresh("with_exit");
        let ok_name = context.fresh("with_ok");
        let body_needs_transfer_safe_cleanup = contains_control_transfer_stmt(&body);

        let ctx_placeholder = context.maybe_placeholder_lowered(context_expr);
        let ctx_cleanup = if ctx_placeholder.modified {
            py_stmt!("{ctx:expr} = None", ctx = ctx_placeholder.expr.clone())
        } else {
            empty_body().into()
        };

        let enter_stmt = if let Some(target) = target {
            py_stmt!(
                "{target:expr} = __dp__.contextmanager_enter({ctx:expr})",
                target = target,
                ctx = ctx_placeholder.expr.clone(),
            )
        } else {
            py_stmt!(
                "__dp__.contextmanager_enter({ctx:expr})",
                ctx = ctx_placeholder.expr.clone(),
            )
        };

        body = if body_needs_transfer_safe_cleanup {
            py_stmt!(
                r#"
{ctx_placeholder_stmt:stmt}
{exit_name:id} = __dp__.contextmanager_get_exit({ctx_placeholder_expr_1:expr})
{enter_stmt:stmt}
{ok_name:id} = True
try:
    {body:stmt}
except BaseException:
    {ok_name:id} = False
    __dp__.contextmanager_exit({exit_name:id}, __dp__.exc_info())
finally:
    if {ok_name:id}:
        __dp__.contextmanager_exit({exit_name:id}, None)
    {exit_name:id} = None
    {ctx_cleanup:stmt}
"#,
                ctx_placeholder_stmt = ctx_placeholder.stmt,
                ctx_placeholder_expr_1 = ctx_placeholder.expr.clone(),
                enter_stmt = enter_stmt,
                body = body,
                exit_name = exit_name.as_str(),
                ok_name = ok_name.as_str(),
                ctx_cleanup = ctx_cleanup,
            )
        } else {
            py_stmt!(
                r#"
{ctx_placeholder_stmt:stmt}
{exit_name:id} = __dp__.contextmanager_get_exit({ctx_placeholder_expr_1:expr})
{enter_stmt:stmt}
{ok_name:id} = True
try:
    {body:stmt}
except BaseException:
    {ok_name:id} = False
    __dp__.contextmanager_exit({exit_name:id}, __dp__.exc_info())
if {ok_name:id}:
    __dp__.contextmanager_exit({exit_name:id}, None)
{exit_name:id} = None
{ctx_cleanup:stmt}
"#,
                ctx_placeholder_stmt = ctx_placeholder.stmt,
                ctx_placeholder_expr_1 = ctx_placeholder.expr.clone(),
                enter_stmt = enter_stmt,
                body = body,
                exit_name = exit_name.as_str(),
                ok_name = ok_name.as_str(),
                ctx_cleanup = ctx_cleanup,
            )
        };
    }

    Rewrite::Walk(body)
}

fn contains_control_transfer_stmt(stmt: &Stmt) -> bool {
    let mut probe = stmt.clone();
    let mut visitor = ControlTransferVisitor { found: false };
    visitor.visit_stmt(&mut probe);
    visitor.found
}

struct ControlTransferVisitor {
    found: bool,
}

fn is_simple_index_target(target: &Expr) -> bool {
    match target {
        Expr::Name(_) => true,
        Expr::Tuple(tuple) => tuple.elts.iter().all(is_simple_index_target),
        Expr::List(list) => list.elts.iter().all(is_simple_index_target),
        Expr::Starred(_) => false,
        _ => false,
    }
}

impl Transformer for ControlTransferVisitor {
    fn visit_stmt(&mut self, stmt: &mut Stmt) {
        if self.found {
            return;
        }
        match stmt {
            Stmt::FunctionDef(_) | Stmt::ClassDef(_) => {}
            Stmt::Return(_) | Stmt::Break(_) | Stmt::Continue(_) => {
                self.found = true;
            }
            _ => walk_stmt(self, stmt),
        }
    }
}

impl StmtRewritePass for BBSimplifyStmtPass {
    fn lower_stmt(&self, context: &Context, stmt: Stmt) -> Rewrite {
        lower_stmt_bb(context, stmt)
    }
}

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
        module_init_function: None,
    };
    rewriter.visit_body(module);
    // BB lowering hoists nested lowered block functions into module-init and
    // leaves placeholder `pass` statements at original def sites. Strip them.
    crate::transform::simplify::strip_generated_passes(context, module);
    BbModule {
        functions: rewriter.lowered_functions_ir,
        module_init: rewriter.module_init_function,
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
    Raise(ast::StmtRaise),
    TryJump {
        body_label: String,
        except_label: String,
        body_region_labels: Vec<String>,
        except_region_labels: Vec<String>,
        finally_label: Option<String>,
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
                "__dp__.setitem({obj:expr}, {slice:expr}, {value:expr})",
                obj = *obj.clone(),
                slice = *slice.clone(),
                value = value,
            )),
            Expr::Attribute(ast::ExprAttribute {
                value: obj, attr, ..
            }) => out.push(py_stmt!(
                "__dp__.setattr({obj:expr}, {name:literal}, {value:expr})",
                obj = *obj.clone(),
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
            let spec_expr = make_tuple(spec_elts);
            out.push(py_stmt!(
                "{tmp:id} = __dp__.unpack({value:expr}, {spec:expr})",
                tmp = unpacked_name.as_str(),
                value = value,
                spec = spec_expr,
            ));
            let unpacked_expr = py_expr!("{tmp:id}", tmp = unpacked_name.as_str());
            for (idx, elt) in elts.iter().enumerate() {
                match elt {
                    Expr::Starred(starred) if idx == starred_index => {
                        let starred_value = py_expr!(
                            "__dp__.list(__dp__.getitem({tmp:expr}, {idx:literal}))",
                            tmp = unpacked_expr.clone(),
                            idx = idx as i64,
                        );
                        self.emit_target_assignments(starred.value.as_ref(), starred_value, out);
                    }
                    _ => {
                        let element_value = py_expr!(
                            "__dp__.getitem({tmp:expr}, {idx:literal})",
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
        let runtime_input_body = prune_dead_stmt_suffixes(&lowered_input_body);
        let param_names = collect_parameter_names(&func.parameters);
        let unbound_local_names = if has_dead_stmt_suffixes(&lowered_input_body) {
            self.always_unbound_local_names(&lowered_input_body, &runtime_input_body, &param_names)
        } else {
            HashSet::new()
        };
        let deleted_names = collect_deleted_names(&runtime_input_body);
        let cell_slots = collect_cell_slots(&runtime_input_body);
        let has_yield = has_yield_exprs_in_stmts(&lowered_input_body);
        let has_await = has_await_in_stmts(&lowered_input_body);
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
        if has_yield {
            let done_label = format!("{label_prefix}_done");
            let invalid_label = format!("{label_prefix}_invalid");
            let invalid_msg = if func.is_async {
                "invalid async generator pc: {}"
            } else {
                "invalid generator pc: {}"
            };
            let invalid_raise_stmt = match py_stmt!(
                "raise RuntimeError({msg:literal}.format(_dp_state['pc']))",
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
            done_block_label = Some(done_label);
            invalid_block_label = Some(invalid_label);

            let throw_dispatch_by_label =
                compute_throw_dispatch_by_label(&blocks, entry_label.as_str());
            let mut resume_labels: HashSet<String> = HashSet::new();
            resume_labels.insert(entry_label.clone());
            for block in &blocks {
                if let Terminator::Yield { resume_label, .. } = &block.terminator {
                    resume_labels.insert(resume_label.clone());
                }
            }
            for dispatch_label in throw_dispatch_by_label.values() {
                resume_labels.insert(dispatch_label.clone());
            }

            let mut rename: HashMap<String, String> = HashMap::new();
            let mut next_resume = 0usize;
            let mut next_internal = 0usize;
            for block in &blocks {
                if done_block_label.as_deref() == Some(block.label.as_str())
                    || invalid_block_label.as_deref() == Some(block.label.as_str())
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
        }

        if !deleted_names.is_empty() {
            rewrite_deleted_name_loads(&mut blocks, &deleted_names, &unbound_local_names);
        } else if !unbound_local_names.is_empty() {
            rewrite_deleted_name_loads(&mut blocks, &HashSet::new(), &unbound_local_names);
        }

        let state_vars = collect_state_vars(
            &param_names,
            &blocks,
            is_module_init_temp_name(func.name.id.as_str()),
        );
        let extra_successors = build_extra_successors(&blocks);
        let mut block_params = compute_block_params(&blocks, &state_vars, &extra_successors);
        if has_yield {
            // Generator/async-generator runtime dispatch passes state through
            // block args; keep `_dp_state` threaded even when local liveness
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
                    name != "_dp_state" && name != "_dp_send_value" && name != "_dp_resume_exc"
                });
                params.insert(0, "_dp_state".to_string());
                params.insert(1, "_dp_send_value".to_string());
                params.insert(2, "_dp_resume_exc".to_string());
                if block.label != entry_label {
                    for exc_name in &try_exc_names {
                        if !params.iter().any(|name| name == exc_name) {
                            params.push(exc_name.clone());
                        }
                    }
                }
            }
            if !try_exc_names.is_empty() {
                if let Some(entry_block) = blocks
                    .iter_mut()
                    .find(|block| block.label.as_str() == entry_label.as_str())
                {
                    for exc_name in try_exc_names.iter().rev() {
                        entry_block.body.insert(
                            0,
                            py_stmt!("{name:id} = __dp__.DELETED", name = exc_name.as_str(),),
                        );
                    }
                }
            }
        }
        let entry_params = block_params
            .get(entry_label.as_str())
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .filter(|name| {
                name != "_dp_state" && name != "_dp_send_value" && name != "_dp_resume_exc"
            })
            .collect::<Vec<_>>();
        let extra_state_vars: Vec<String> = entry_params
            .iter()
            .filter(|name| !param_names.iter().any(|param| param == *name))
            .cloned()
            .collect();
        let block_pc_by_label: HashMap<String, usize> = blocks
            .iter()
            .enumerate()
            .map(|(idx, block)| (block.label.clone(), idx))
            .collect();
        let start_pc = block_pc_by_label
            .get(entry_label.as_str())
            .copied()
            .unwrap_or(0);
        let target_labels = blocks
            .iter()
            .map(|block| block.label.clone())
            .collect::<Vec<_>>();
        let throw_dispatch_by_label =
            compute_throw_dispatch_by_label(&blocks, entry_label.as_str());
        let mut throw_dispatch_pcs: Vec<Option<usize>> = Vec::new();
        let lowered_is_async = func.is_async;
        for block in &blocks {
            let skip_resume_hook = block_starts_with_resume_value_assign(&block)
                || matches!(block.terminator, Terminator::TryJump { .. })
                || done_block_label.as_deref() == Some(block.label.as_str())
                || invalid_block_label.as_deref() == Some(block.label.as_str());
            let dispatch_pc = if has_yield && !skip_resume_hook {
                throw_dispatch_by_label
                    .get(block.label.as_str())
                    .and_then(|dispatch_label| block_pc_by_label.get(dispatch_label).copied())
            } else {
                None
            };
            throw_dispatch_pcs.push(dispatch_pc);
        }
        let mut state_order = entry_params.clone();
        for name in extra_state_vars {
            if !state_order.iter().any(|existing| existing == &name) {
                state_order.push(name);
            }
        }

        let ir_blocks = blocks
            .iter()
            .map(|block| BbBlock {
                label: block.label.clone(),
                params: block_params
                    .get(block.label.as_str())
                    .cloned()
                    .unwrap_or_default(),
                ops: block.body.clone(),
                term: bb_term_from_terminator(&block.terminator),
            })
            .collect::<Vec<_>>();

        Some(LoweredFunction {
            blocks: ir_blocks,
            entry_label,
            entry_params: state_order,
            local_cell_slots: cell_slots.clone(),
            param_specs: make_param_specs_expr(func.parameters.as_ref()),
            param_names,
            kind: if has_yield {
                if lowered_is_async {
                    LoweredKind::AsyncGenerator {
                        start_pc,
                        target_labels,
                        throw_dispatch_pcs,
                    }
                } else {
                    LoweredKind::Generator {
                        start_pc,
                        target_labels,
                        throw_dispatch_pcs,
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
                        let label = self.next_label(fn_name);
                        let resume_label = self.lower_stmt_sequence(
                            fn_name,
                            &stmts[index + 1..],
                            cont_label.clone(),
                            blocks,
                            loop_ctx,
                            cell_slots,
                        );
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
                    if let Expr::YieldFrom(yield_from_expr) = value.as_ref() {
                        let rest_entry = self.lower_stmt_sequence(
                            fn_name,
                            &stmts[index + 1..],
                            cont_label.clone(),
                            blocks,
                            loop_ctx,
                            cell_slots,
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
                Stmt::Pass(_) | Stmt::FunctionDef(_) => {
                    linear.push(stmts[index].as_ref().clone());
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
                        );
                        let resume_label = self.next_label(fn_name);
                        let mut resume_assign = assign_stmt.clone();
                        resume_assign.value =
                            Box::new(py_expr!("{sent:id}", sent = "_dp_send_value"));
                        let mut resume_body = vec![Stmt::Assign(resume_assign.clone())];
                        for target in &resume_assign.targets {
                            resume_body.extend(sync_target_cells_stmts(target, cell_slots));
                        }
                        blocks.push(Block {
                            label: resume_label.clone(),
                            body: resume_body,
                            terminator: Terminator::Jump(rest_entry),
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
                    );
                    let then_entry = self.lower_stmt_sequence(
                        fn_name,
                        &then_body,
                        rest_entry.clone(),
                        blocks,
                        loop_ctx,
                        cell_slots,
                    );
                    let else_entry = self.lower_stmt_sequence(
                        fn_name, &else_body, rest_entry, blocks, loop_ctx, cell_slots,
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
                    if for_stmt.is_async {
                        return cont_label;
                    }

                    let rest_entry = self.lower_stmt_sequence(
                        fn_name,
                        &stmts[index + 1..],
                        cont_label.clone(),
                        blocks,
                        loop_ctx,
                        cell_slots,
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
                    let body = flatten_stmt_boxes(&for_stmt.body.body);
                    let loop_ctx = LoopContext {
                        continue_label: loop_check_label.clone(),
                        break_label: rest_entry,
                    };
                    let body_entry = self.lower_stmt_sequence(
                        fn_name,
                        &body,
                        loop_check_label.clone(),
                        blocks,
                        Some(&loop_ctx),
                        cell_slots,
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
                        // Keep complex destructuring (subscript/attribute/starred) in
                        // canonical assignment form so the normal assignment lowerer
                        // can preserve Python assignment semantics.
                        assign_body.push(py_stmt!(
                            "{target:expr} = {value:expr}",
                            target = *for_stmt.target.clone(),
                            value = tmp_expr.clone(),
                        ));
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
                        "__dp__.is_({value:expr}, __dp__.ITER_COMPLETE)",
                        value = tmp_expr.clone(),
                    );
                    blocks.push(Block {
                        label: loop_check_label.clone(),
                        body: vec![py_stmt!(
                            "{tmp:id} = __dp__.next_or_sentinel({iter:expr})",
                            tmp = tmp_name.as_str(),
                            iter = iter_expr.clone(),
                        )],
                        terminator: Terminator::BrIf {
                            test: exhausted_test,
                            then_label: exhausted_entry,
                            else_label: assign_label,
                        },
                    });

                    let mut setup_body = linear;
                    setup_body.push(py_stmt!(
                        "{iter:id} = __dp__.iter({iterable:expr})",
                        iter = iter_name.as_str(),
                        iterable = *for_stmt.iter.clone(),
                    ));
                    let setup_label = self.next_label(fn_name);
                    blocks.push(Block {
                        label: setup_label.clone(),
                        body: setup_body,
                        terminator: Terminator::Jump(loop_check_label),
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
                    );

                    let has_finally = !try_stmt.finalbody.body.is_empty();
                    let (finally_label, finally_region_labels, finally_fallthrough_label) =
                        if has_finally {
                            let finally_region_start = blocks.len();
                            let finally_body = flatten_stmt_boxes(&try_stmt.finalbody.body);
                            let finally_label = self.lower_stmt_sequence(
                                fn_name,
                                &finally_body,
                                rest_entry.clone(),
                                blocks,
                                loop_ctx,
                                cell_slots,
                            );
                            let finally_region_labels = blocks[finally_region_start..]
                                .iter()
                                .map(|block| block.label.clone())
                                .collect::<Vec<_>>();
                            (
                                Some(finally_label),
                                finally_region_labels,
                                Some(rest_entry.clone()),
                            )
                        } else {
                            (None, Vec::new(), None)
                        };
                    let pass_target = finally_label.clone().unwrap_or_else(|| rest_entry.clone());

                    let body_region_start = blocks.len();
                    let body_pass_label = self.next_label(fn_name);
                    blocks.push(Block {
                        label: body_pass_label.clone(),
                        body: Vec::new(),
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
                    );

                    let try_body = flatten_stmt_boxes(&try_stmt.body.body);
                    let body_label = self.lower_stmt_sequence(
                        fn_name, &try_body, else_entry, blocks, loop_ctx, cell_slots,
                    );
                    let body_region_labels = blocks[body_region_start..]
                        .iter()
                        .map(|block| block.label.clone())
                        .collect::<Vec<_>>();

                    let except_region_start = blocks.len();
                    let except_pass_label = self.next_label(fn_name);
                    let except_exc_name = self.next_temp("try_exc");
                    blocks.push(Block {
                        label: except_pass_label.clone(),
                        body: vec![py_stmt!(
                            "{exc:id} = __dp__.DELETED",
                            exc = except_exc_name.as_str(),
                        )],
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
                            vec![Box::new(py_stmt!("raise __dp__.current_exception()"))]
                        });
                    let except_body =
                        capture_except_exception(except_body, except_exc_name.as_str());
                    let except_label = self.lower_stmt_sequence(
                        fn_name,
                        &except_body,
                        except_pass_label,
                        blocks,
                        loop_ctx,
                        cell_slots,
                    );
                    let except_region_labels = blocks[except_region_start..]
                        .iter()
                        .map(|block| block.label.clone())
                        .collect::<Vec<_>>();

                    let label = self.next_label(fn_name);
                    blocks.push(Block {
                        label: label.clone(),
                        body: linear,
                        terminator: Terminator::TryJump {
                            body_label,
                            except_label,
                            body_region_labels,
                            except_region_labels,
                            finally_label,
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
                    "__dp__.setitem(_dp_state, \"gi_yieldfrom\", {iter_name:id})",
                    iter_name = iter_name.as_str(),
                ),
            ],
            terminator: Terminator::TryJump {
                body_label: next_body_label.clone(),
                except_label: stop_except_label.clone(),
                body_region_labels: vec![next_body_label.clone()],
                except_region_labels: vec![
                    stop_except_label.clone(),
                    stop_done_label.clone(),
                    raise_stop_label.clone(),
                ],
                finally_label: None,
                finally_region_labels: Vec::new(),
                finally_fallthrough_label: None,
            },
        });
        blocks.push(Block {
            label: next_body_label.clone(),
            body: vec![py_stmt!(
                "{yielded:id} = next({iter:id})",
                yielded = yielded_name.as_str(),
                iter = iter_name.as_str(),
            )],
            terminator: Terminator::Jump(yield_label.clone()),
        });
        blocks.push(Block {
            label: stop_except_label.clone(),
            body: vec![py_stmt!(
                "{stop:id} = __dp__.current_exception()",
                stop = stop_name.as_str(),
            )],
            terminator: Terminator::BrIf {
                test: py_expr!(
                    "__dp__.exception_matches({stop:id}, StopIteration)",
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
            body: vec![py_stmt!("__dp__.setitem(_dp_state, \"gi_yieldfrom\", None)")],
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
                py_stmt!(
                    "{resume:id} = None",
                    resume = "_dp_resume_exc",
                ),
            ],
            terminator: Terminator::BrIf {
                test: py_expr!("{exc:id} is not None", exc = exc_name.as_str()),
                then_label: exc_dispatch_label.clone(),
                else_label: send_dispatch_label.clone(),
            },
        });
        blocks.push(Block {
            label: exc_dispatch_label,
            body: Vec::new(),
            terminator: Terminator::BrIf {
                test: py_expr!(
                    "__dp__.exception_matches({exc:id}, GeneratorExit)",
                    exc = exc_name.as_str(),
                ),
                then_label: genexit_close_lookup_label.clone(),
                else_label: lookup_throw_label.clone(),
            },
        });
        blocks.push(Block {
            label: genexit_close_lookup_label,
            body: vec![py_stmt!(
                "{close:id} = getattr({iter:id}, \"close\", None)",
                close = close_name.as_str(),
                iter = iter_name.as_str(),
            )],
            terminator: Terminator::BrIf {
                test: py_expr!("{close:id} is not None", close = close_name.as_str()),
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
            body: vec![py_stmt!("__dp__.setitem(_dp_state, \"gi_yieldfrom\", None)")],
            terminator: Terminator::Raise(raise_stmt_from_name(raise_name.as_str())),
        });
        blocks.push(Block {
            label: lookup_throw_label,
            body: vec![py_stmt!(
                "{throw:id} = getattr({iter:id}, \"throw\", None)",
                throw = throw_name.as_str(),
                iter = iter_name.as_str(),
            )],
            terminator: Terminator::BrIf {
                test: py_expr!("{throw:id} is None", throw = throw_name.as_str()),
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
                body_region_labels: vec![throw_body_label.clone()],
                except_region_labels: vec![
                    stop_except_label.clone(),
                    stop_done_label.clone(),
                    raise_stop_label.clone(),
                ],
                finally_label: None,
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
                finally_region_labels: Vec::new(),
                finally_fallthrough_label: None,
            },
        });
        blocks.push(Block {
            label: send_dispatch_label,
            body: Vec::new(),
            terminator: Terminator::BrIf {
                test: py_expr!("{sent:id} is None", sent = sent_name.as_str()),
                then_label: next_body_label,
                else_label: send_call_body_label.clone(),
            },
        });
        blocks.push(Block {
            label: send_call_body_label,
            body: vec![py_stmt!(
                "{yielded:id} = {iter:id}.send({sent:id})",
                yielded = yielded_name.as_str(),
                iter = iter_name.as_str(),
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

    fn build_def_expr_from_bb(&self, bb_function: &BbFunction) -> Option<Expr> {
        let entry = name_expr(bb_function.entry.as_str())?;
        let param_names: HashSet<&str> =
            bb_function.param_names.iter().map(String::as_str).collect();
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
                closure_items.push(make_tuple(vec![
                    py_expr!("{value:literal}", value = entry_name.as_str()),
                    value,
                ]));
            } else {
                closure_items.push(py_expr!("{value:literal}", value = entry_name.as_str(),));
            }
        }
        let closure = make_tuple(closure_items);
        match &bb_function.kind {
            BbFunctionKind::Function => Some(py_expr!(
                "__dp__.def_fn({entry:expr}, {name:literal}, {qualname:literal}, {closure:expr}, {params:expr}, {module_name:expr})",
                entry = entry,
                name = bb_function.display_name.as_str(),
                qualname = bb_function.qualname.as_str(),
                closure = closure,
                params = bb_function.param_specs.clone(),
                module_name = py_expr!("__name__"),
            )),
            BbFunctionKind::Coroutine => Some(py_expr!(
                "__dp__.def_coro({entry:expr}, {name:literal}, {qualname:literal}, {closure:expr}, {params:expr}, {module_name:expr})",
                entry = entry,
                name = bb_function.display_name.as_str(),
                qualname = bb_function.qualname.as_str(),
                closure = closure,
                params = bb_function.param_specs.clone(),
                module_name = py_expr!("__name__"),
            )),
            BbFunctionKind::AsyncGenerator {
                start_pc,
                target_labels,
                throw_dispatch_pcs,
            } => {
                let target_exprs = target_labels
                    .iter()
                    .map(|label| name_expr(label.as_str()))
                    .collect::<Option<Vec<_>>>()?;
                let targets = make_tuple(target_exprs);
                let throw_dispatch_pcs_expr = make_tuple(
                    throw_dispatch_pcs
                        .iter()
                        .map(|pc| py_expr!("{value:literal}", value = pc.map(|p| p as i64).unwrap_or(-1)))
                        .collect(),
                );
                Some(py_expr!(
                    "__dp__.def_async_gen({start_pc:literal}, {targets:expr}, {throw_dispatch_pcs:expr}, {name:literal}, {qualname:literal}, {closure:expr}, {params:expr}, __name__)",
                    start_pc = *start_pc as i64,
                    targets = targets,
                    throw_dispatch_pcs = throw_dispatch_pcs_expr,
                    name = bb_function.display_name.as_str(),
                    qualname = bb_function.qualname.as_str(),
                    closure = closure,
                    params = bb_function.param_specs.clone(),
                ))
            }
            BbFunctionKind::Generator {
                start_pc,
                target_labels,
                throw_dispatch_pcs,
            } => {
                let target_exprs = target_labels
                    .iter()
                    .map(|label| name_expr(label.as_str()))
                    .collect::<Option<Vec<_>>>()?;
                let targets = make_tuple(target_exprs);
                let throw_dispatch_pcs_expr = make_tuple(
                    throw_dispatch_pcs
                        .iter()
                        .map(|pc| py_expr!("{value:literal}", value = pc.map(|p| p as i64).unwrap_or(-1)))
                        .collect(),
                );
                Some(py_expr!(
                    "__dp__.def_gen({start_pc:literal}, {targets:expr}, {throw_dispatch_pcs:expr}, {name:literal}, {qualname:literal}, {closure:expr}, {params:expr}, __name__)",
                    start_pc = *start_pc as i64,
                    targets = targets,
                    throw_dispatch_pcs = throw_dispatch_pcs_expr,
                    name = bb_function.display_name.as_str(),
                    qualname = bb_function.qualname.as_str(),
                    closure = closure,
                    params = bb_function.param_specs.clone(),
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
            Some((
                annotate_helper_name.clone(),
                rewrite_stmt::annotation::build_annotate_fn(
                    annotation_entries,
                    annotate_helper_name.as_str(),
                ),
            ))
        };

        let annotate_fn_expr = match annotate_helper_stmt.as_ref() {
            Some((helper_name, _)) => Some(name_expr(helper_name.as_str())?),
            None => None,
        };
        let doc_expr = function_docstring_expr(func);

        let base_expr = self.build_def_expr_from_bb(bb_function)?;
        let base_expr = maybe_wrap_function_metadata_expr(base_expr, doc_expr, annotate_fn_expr);
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
                "__dp__.store_cell({cell:id}, {name:id})",
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
                "__dp__.store_global(globals(), {name:literal}, {value:expr})",
                name = bind_name,
                value = value,
            ),
            BindingTarget::ClassNamespace => py_stmt!(
                "__dp__.setitem(_dp_class_ns, {name:literal}, {value:expr})",
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
                    "__dp__.store_cell({cell:id}, {name:id})",
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
            "__dp__.update_fn({name:id}, {qualname:literal}, {display_name:literal})",
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

fn is_annotation_helper_name(name: &str) -> bool {
    name.contains("__annotate_func__") || name.contains("__annotate__")
}

fn should_keep_non_lowered_for_annotationlib(func: &ast::StmtFunctionDef) -> bool {
    // annotationlib.call_annotate_function rebuilds callables via FunctionType(..., fake_globals).
    // BB-lowered wrappers delegate into pre-bound block function objects, so fake globals do not
    // apply to the annotation expression evaluation. Keep likely annotate callables in regular
    // function form so fake-globals execution can observe transformed expressions directly.
    let params = func.parameters.as_ref();
    let Some(first) = params.posonlyargs.first() else {
        return false;
    };
    first.parameter.name.id.as_str() == "format"
}

fn ensure_dp_default_param(func: &mut ast::StmtFunctionDef) {
    if function_has_global_or_nonlocal_dp(func) {
        return;
    }
    if collect_parameter_names(func.parameters.as_ref())
        .iter()
        .any(|name| name == "__dp__")
    {
        return;
    }
    let template = py_stmt!(
        r#"
def _dp_template(*, __dp__=__dp__):
    pass
"#
    );
    let kwonly = match template {
        Stmt::FunctionDef(template_fn) => template_fn
            .parameters
            .kwonlyargs
            .first()
            .cloned()
            .expect("template kwonly param missing"),
        _ => unreachable!("template did not parse as function"),
    };
    func.parameters.kwonlyargs.push(kwonly);
}

fn function_has_global_or_nonlocal_dp(func: &ast::StmtFunctionDef) -> bool {
    func.body.body.iter().any(|stmt| match stmt.as_ref() {
        Stmt::Global(global_stmt) => global_stmt
            .names
            .iter()
            .any(|name| name.id.as_str() == "__dp__"),
        Stmt::Nonlocal(nonlocal_stmt) => nonlocal_stmt
            .names
            .iter()
            .any(|name| name.id.as_str() == "__dp__"),
        _ => false,
    })
}

struct LoweredFunction {
    blocks: Vec<BbBlock>,
    entry_label: String,
    entry_params: Vec<String>,
    local_cell_slots: HashSet<String>,
    param_specs: Expr,
    param_names: Vec<String>,
    kind: LoweredKind,
}

#[derive(Clone)]
enum LoweredKind {
    Function,
    Coroutine,
    AsyncGenerator {
        start_pc: usize,
        target_labels: Vec<String>,
        throw_dispatch_pcs: Vec<Option<usize>>,
    },
    Generator {
        start_pc: usize,
        target_labels: Vec<String>,
        throw_dispatch_pcs: Vec<Option<usize>>,
    },
}

fn bb_term_from_terminator(terminator: &Terminator) -> BbTerm {
    match terminator {
        Terminator::Jump(target) => BbTerm::Jump(target.clone()),
        Terminator::BrIf {
            test,
            then_label,
            else_label,
        } => BbTerm::BrIf {
            test: test.clone(),
            then_label: then_label.clone(),
            else_label: else_label.clone(),
        },
        Terminator::Raise(ast::StmtRaise { exc, cause, .. }) => BbTerm::Raise {
            exc: exc.as_ref().map(|expr| *expr.clone()),
            cause: cause.as_ref().map(|expr| *expr.clone()),
        },
        Terminator::TryJump {
            body_label,
            except_label,
            body_region_labels,
            except_region_labels,
            finally_label,
            finally_region_labels,
            finally_fallthrough_label,
        } => BbTerm::TryJump {
            body_label: body_label.clone(),
            except_label: except_label.clone(),
            body_region_labels: body_region_labels.clone(),
            except_region_labels: except_region_labels.clone(),
            finally_label: finally_label.clone(),
            finally_region_labels: finally_region_labels.clone(),
            finally_fallthrough_label: finally_fallthrough_label.clone(),
        },
        Terminator::Yield {
            value,
            resume_label,
        } => BbTerm::Yield {
            value: value.clone(),
            resume_label: resume_label.clone(),
        },
        Terminator::Ret(value) => BbTerm::Ret(value.clone()),
    }
}

fn bb_binding_target_from(target: BindingTarget) -> BbBindingTarget {
    match target {
        BindingTarget::Local => BbBindingTarget::Local,
        BindingTarget::ModuleGlobal => BbBindingTarget::ModuleGlobal,
        BindingTarget::ClassNamespace => BbBindingTarget::ClassNamespace,
    }
}

fn bb_function_kind_from(kind: &LoweredKind) -> BbFunctionKind {
    match kind {
        LoweredKind::Function => BbFunctionKind::Function,
        LoweredKind::Coroutine => BbFunctionKind::Coroutine,
        LoweredKind::Generator {
            start_pc,
            target_labels,
            throw_dispatch_pcs,
        } => BbFunctionKind::Generator {
            start_pc: *start_pc,
            target_labels: target_labels.clone(),
            throw_dispatch_pcs: throw_dispatch_pcs.clone(),
        },
        LoweredKind::AsyncGenerator {
            start_pc,
            target_labels,
            throw_dispatch_pcs,
        } => BbFunctionKind::AsyncGenerator {
            start_pc: *start_pc,
            target_labels: target_labels.clone(),
            throw_dispatch_pcs: throw_dispatch_pcs.clone(),
        },
    }
}

#[derive(Clone)]
struct FunctionIdentity {
    bind_name: String,
    display_name: String,
    qualname: String,
    binding_target: BindingTarget,
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum BindingTarget {
    Local,
    ModuleGlobal,
    ClassNamespace,
}

fn display_name_for_function(raw_name: &str) -> &str {
    if raw_name.starts_with("_dp_lambda_") {
        "<lambda>"
    } else if raw_name.starts_with("_dp_genexpr_") {
        "<genexpr>"
    } else if raw_name.starts_with("_dp_listcomp_") {
        "<listcomp>"
    } else if raw_name.starts_with("_dp_setcomp_") {
        "<setcomp>"
    } else if raw_name.starts_with("_dp_dictcomp_") {
        "<dictcomp>"
    } else {
        raw_name
    }
}

fn normalize_qualname(raw_qualname: &str, raw_name: &str, display_name: &str) -> String {
    let raw_qualname = strip_synthetic_module_init_qualname(raw_qualname);
    let should_replace_tail = matches!(display_name, "<lambda>" | "<genexpr>");
    if raw_name == display_name || !should_replace_tail {
        return raw_qualname;
    }
    match raw_qualname.rsplit_once('.') {
        Some((prefix, _)) => format!("{prefix}.{display_name}"),
        None => display_name.to_string(),
    }
}

fn collect_function_identity_private(
    module: &mut StmtBody,
    module_scope: Arc<Scope>,
) -> HashMap<NodeIndex, FunctionIdentity> {
    fn binding_target_for_scope(scope: &Scope, bind_name: &str) -> BindingTarget {
        if is_internal_symbol(bind_name) {
            return BindingTarget::Local;
        }
        let binding = scope.binding_in_scope(bind_name, BindingUse::Load);
        match (scope.kind(), binding) {
            (ScopeKind::Class, BindingKind::Local) => BindingTarget::ClassNamespace,
            (_, BindingKind::Global) => BindingTarget::ModuleGlobal,
            _ => BindingTarget::Local,
        }
    }

    struct Collector {
        scope_stack: Vec<Arc<Scope>>,
        out: HashMap<NodeIndex, FunctionIdentity>,
    }

    impl Transformer for Collector {
        fn visit_stmt(&mut self, stmt: &mut Stmt) {
            match stmt {
                Stmt::FunctionDef(func) => {
                    let node_index = func.node_index.load();
                    if node_index != NodeIndex::NONE {
                        let bind_name = func.name.id.to_string();
                        let display_name =
                            display_name_for_function(bind_name.as_str()).to_string();
                        let parent_scope = self
                            .scope_stack
                            .last()
                            .expect("missing scope while collecting function identity");
                        let child_scope = parent_scope.tree.scope_for_def(func).ok();
                        let qualname = child_scope
                            .as_ref()
                            .map(|scope| {
                                normalize_qualname(
                                    scope.qualnamer.qualname.as_str(),
                                    bind_name.as_str(),
                                    display_name.as_str(),
                                )
                            })
                            .unwrap_or_else(|| bind_name.clone());
                        self.out.insert(
                            node_index,
                            FunctionIdentity {
                                bind_name: bind_name.clone(),
                                display_name,
                                qualname,
                                binding_target: binding_target_for_scope(
                                    parent_scope.as_ref(),
                                    bind_name.as_str(),
                                ),
                            },
                        );
                        if let Some(child_scope) = child_scope {
                            self.scope_stack.push(child_scope);
                            walk_stmt(self, stmt);
                            self.scope_stack.pop();
                            return;
                        }
                    }
                    walk_stmt(self, stmt);
                }
                Stmt::ClassDef(class_def) => {
                    let parent_scope = self
                        .scope_stack
                        .last()
                        .expect("missing scope while collecting class scope");
                    if let Ok(child_scope) = parent_scope.tree.scope_for_def(class_def) {
                        self.scope_stack.push(child_scope);
                        walk_stmt(self, stmt);
                        self.scope_stack.pop();
                        return;
                    }
                    walk_stmt(self, stmt);
                }
                _ => walk_stmt(self, stmt),
            }
        }
    }

    let mut module = module.clone();
    let mut collector = Collector {
        scope_stack: vec![module_scope.clone()],
        out: HashMap::new(),
    };
    collector.visit_body(&mut module);
    collector.out
}

impl Transformer for BasicBlockRewriter<'_> {
    fn visit_stmt(&mut self, stmt: &mut Stmt) {
        if let Stmt::FunctionDef(func) = stmt {
            let fn_name = func.name.id.to_string();
            let entering_module_init = is_module_init_temp_name(fn_name.as_str());
            if entering_module_init {
                self.module_init_hoisted_blocks.push(Vec::new());
            }
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
            let mut module_init_hoisted = Vec::new();
            if entering_module_init {
                module_init_hoisted = self.module_init_hoisted_blocks.pop().unwrap_or_default();
            }

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
                        binding_target: bb_binding_target_from(resolved_target),
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
                    let block_defs = render_py::render_block_defs_from_bb(&bb_function)
                        .expect("failed to render BB function blocks");
                    let lowered_entry_label = bb_function.entry.clone();
                    let binding_stmt = self
                        .build_lowered_binding_stmt(func, &bb_function)
                        .expect("failed to build BB function binding");
                    let keep_local_blocks = func.name.id.as_str().starts_with("_dp_class_ns_")
                        || func.name.id.as_str().starts_with("_dp_define_class_");
                    if entering_module_init {
                        let mut lowered_defs = module_init_hoisted;
                        lowered_defs.extend(block_defs);
                        lowered_defs.push(binding_stmt);
                        lowered_defs.push(py_stmt!(
                            "del {entry:id}",
                            entry = lowered_entry_label.as_str(),
                        ));
                        *stmt = into_body(lowered_defs);
                    } else if keep_local_blocks {
                        let mut body = block_defs;
                        body.push(binding_stmt);
                        *stmt = into_body(body);
                    } else if !self.module_init_hoisted_blocks.is_empty() {
                        if let Some(hoisted) = self.module_init_hoisted_blocks.last_mut() {
                            hoisted.extend(block_defs);
                        }
                        *stmt = binding_stmt;
                    } else {
                        let mut body = block_defs;
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
                        if entering_module_init {
                            body.extend(module_init_hoisted.clone());
                        }
                        body.push(Stmt::FunctionDef(func.clone()));
                        body.push(binding_stmt);
                        *stmt = into_body(body);
                    } else if entering_module_init && !module_init_hoisted.is_empty() {
                        let mut new_body = module_init_hoisted
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

fn is_module_init_temp_name(name: &str) -> bool {
    name == "_dp_module_init" || name.starts_with("_dp_fn__dp_module_init_")
}

struct BasicBlockSupportChecker {
    supported: bool,
    loop_depth: usize,
    allow_await: bool,
}

impl Default for BasicBlockSupportChecker {
    fn default() -> Self {
        Self {
            supported: true,
            loop_depth: 0,
            allow_await: false,
        }
    }
}

impl BasicBlockSupportChecker {
    fn mark_unsupported(&mut self) {
        self.supported = false;
    }

    fn panic_stmt(&self, message: &str, stmt: &Stmt) -> ! {
        let rendered = crate::ruff_ast_to_string(stmt);
        panic!(
            "BB lowering invariant violated: {message}\nstmt:\n{}",
            rendered.trim_end()
        );
    }
}

impl Transformer for BasicBlockSupportChecker {
    fn visit_body(&mut self, body: &mut StmtBody) {
        if !self.supported {
            return;
        }
        if has_dead_stmt_after_terminator(body) {
            self.mark_unsupported();
            return;
        }
        walk_stmt_body(self, body);
    }

    fn visit_stmt(&mut self, stmt: &mut Stmt) {
        if !self.supported {
            return;
        }
        match stmt {
            Stmt::Expr(_)
            | Stmt::Pass(_)
            | Stmt::Assign(_)
            | Stmt::Delete(_)
            | Stmt::Return(_)
            | Stmt::Raise(_) => {
                walk_stmt(self, stmt);
            }
            Stmt::FunctionDef(_) => {
                // A nested function definition is executable as a linear
                // statement in the parent CFG. We intentionally don't inspect
                // its body here; nested-function support is validated when the
                // nested function itself is visited for BB lowering.
            }
            Stmt::BodyStmt(_) => walk_stmt(self, stmt),
            Stmt::If(if_stmt) => {
                if if_stmt
                    .elif_else_clauses
                    .iter()
                    .any(|clause| clause.test.is_some())
                {
                    self.panic_stmt("`elif` chain reached support checker", stmt);
                }
                walk_stmt(self, stmt);
            }
            Stmt::While(while_stmt) => {
                self.visit_expr(while_stmt.test.as_mut());
                self.loop_depth += 1;
                self.visit_body(&mut while_stmt.body);
                self.loop_depth -= 1;
                self.visit_body(&mut while_stmt.orelse);
            }
            Stmt::For(for_stmt) => {
                if for_stmt.is_async {
                    self.mark_unsupported();
                    return;
                }
                self.visit_expr(for_stmt.iter.as_mut());
                self.loop_depth += 1;
                self.visit_body(&mut for_stmt.body);
                self.loop_depth -= 1;
                self.visit_body(&mut for_stmt.orelse);
            }
            Stmt::Try(try_stmt) => {
                self.visit_body(&mut try_stmt.body);
                for handler in try_stmt.handlers.iter_mut() {
                    let ast::ExceptHandler::ExceptHandler(handler) = handler;
                    if let Some(type_) = handler.type_.as_mut() {
                        self.visit_expr(type_.as_mut());
                    }
                    self.visit_body(&mut handler.body);
                }
                self.visit_body(&mut try_stmt.orelse);
                self.visit_body(&mut try_stmt.finalbody);
            }
            Stmt::Break(_) | Stmt::Continue(_) => {
                if self.loop_depth == 0 {
                    self.panic_stmt(
                        "`break`/`continue` outside loop reached support checker",
                        stmt,
                    );
                }
            }
            _ => self.panic_stmt("unsupported statement kind reached support checker", stmt),
        }
    }

    fn visit_expr(&mut self, expr: &mut Expr) {
        if !self.supported {
            return;
        }
        match expr {
            Expr::Await(_) => {
                if !self.allow_await {
                    self.mark_unsupported();
                    return;
                }
            }
            Expr::Yield(_) | Expr::YieldFrom(_) => {
                self.mark_unsupported();
                return;
            }
            _ => {}
        }
        walk_expr(self, expr);
    }
}

fn has_dead_stmt_after_terminator(body: &StmtBody) -> bool {
    let mut terminated = false;
    for stmt in &body.body {
        if terminated {
            return true;
        }
        terminated = matches!(
            stmt.as_ref(),
            Stmt::Return(_) | Stmt::Raise(_) | Stmt::Break(_) | Stmt::Continue(_)
        );
    }
    false
}

fn has_dead_stmt_suffixes(stmts: &[Box<Stmt>]) -> bool {
    let mut terminated = false;
    for stmt in stmts {
        let stmt = stmt.as_ref();
        if terminated {
            return true;
        }
        if has_dead_stmt_suffixes_in_stmt(stmt) {
            return true;
        }
        if matches!(
            stmt,
            Stmt::Return(_) | Stmt::Raise(_) | Stmt::Break(_) | Stmt::Continue(_)
        ) {
            terminated = true;
        }
    }
    false
}

fn has_dead_stmt_suffixes_in_stmt(stmt: &Stmt) -> bool {
    match stmt {
        Stmt::BodyStmt(body) => has_dead_stmt_suffixes(&body.body),
        Stmt::If(if_stmt) => {
            has_dead_stmt_suffixes(&if_stmt.body.body)
                || if_stmt
                    .elif_else_clauses
                    .iter()
                    .any(|clause| has_dead_stmt_suffixes(&clause.body.body))
        }
        Stmt::While(while_stmt) => {
            has_dead_stmt_suffixes(&while_stmt.body.body)
                || has_dead_stmt_suffixes(&while_stmt.orelse.body)
        }
        Stmt::For(for_stmt) => {
            has_dead_stmt_suffixes(&for_stmt.body.body)
                || has_dead_stmt_suffixes(&for_stmt.orelse.body)
        }
        Stmt::Try(try_stmt) => {
            has_dead_stmt_suffixes(&try_stmt.body.body)
                || try_stmt.handlers.iter().any(|handler| {
                    let ast::ExceptHandler::ExceptHandler(handler) = handler;
                    has_dead_stmt_suffixes(&handler.body.body)
                })
                || has_dead_stmt_suffixes(&try_stmt.orelse.body)
                || has_dead_stmt_suffixes(&try_stmt.finalbody.body)
        }
        _ => false,
    }
}

fn prune_dead_stmt_suffixes(stmts: &[Box<Stmt>]) -> Vec<Box<Stmt>> {
    let mut out = Vec::new();
    for stmt in stmts {
        let mut stmt = stmt.as_ref().clone();
        prune_dead_stmt_suffixes_in_stmt(&mut stmt);
        let terminates = matches!(
            stmt,
            Stmt::Return(_) | Stmt::Raise(_) | Stmt::Break(_) | Stmt::Continue(_)
        );
        out.push(Box::new(stmt));
        if terminates {
            break;
        }
    }
    out
}

fn prune_dead_stmt_suffixes_in_stmt(stmt: &mut Stmt) {
    match stmt {
        Stmt::BodyStmt(body) => {
            body.body = prune_dead_stmt_suffixes(&body.body);
        }
        Stmt::If(if_stmt) => {
            if_stmt.body.body = prune_dead_stmt_suffixes(&if_stmt.body.body);
            for clause in &mut if_stmt.elif_else_clauses {
                clause.body.body = prune_dead_stmt_suffixes(&clause.body.body);
            }
        }
        Stmt::While(while_stmt) => {
            while_stmt.body.body = prune_dead_stmt_suffixes(&while_stmt.body.body);
            while_stmt.orelse.body = prune_dead_stmt_suffixes(&while_stmt.orelse.body);
        }
        Stmt::For(for_stmt) => {
            for_stmt.body.body = prune_dead_stmt_suffixes(&for_stmt.body.body);
            for_stmt.orelse.body = prune_dead_stmt_suffixes(&for_stmt.orelse.body);
        }
        Stmt::Try(try_stmt) => {
            try_stmt.body.body = prune_dead_stmt_suffixes(&try_stmt.body.body);
            for handler in &mut try_stmt.handlers {
                let ast::ExceptHandler::ExceptHandler(handler) = handler;
                handler.body.body = prune_dead_stmt_suffixes(&handler.body.body);
            }
            try_stmt.orelse.body = prune_dead_stmt_suffixes(&try_stmt.orelse.body);
            try_stmt.finalbody.body = prune_dead_stmt_suffixes(&try_stmt.finalbody.body);
        }
        _ => {}
    }
}

#[derive(Default)]
struct YieldLikeProbe {
    has_yield: bool,
    has_yield_from: bool,
    has_await: bool,
}

impl Transformer for YieldLikeProbe {
    fn visit_stmt(&mut self, stmt: &mut Stmt) {
        if matches!(stmt, Stmt::FunctionDef(_) | Stmt::ClassDef(_)) {
            return;
        }
        walk_stmt(self, stmt);
    }

    fn visit_expr(&mut self, expr: &mut Expr) {
        match expr {
            Expr::Yield(_) => self.has_yield = true,
            Expr::YieldFrom(_) => self.has_yield_from = true,
            Expr::Await(_) => self.has_await = true,
            _ => {}
        }
        walk_expr(self, expr);
    }
}

fn has_yield_exprs_in_stmts(stmts: &[Box<Stmt>]) -> bool {
    let mut probe = YieldLikeProbe::default();
    for stmt in stmts {
        let mut stmt = stmt.as_ref().clone();
        probe.visit_stmt(&mut stmt);
        if probe.has_yield || probe.has_yield_from {
            return true;
        }
    }
    false
}

fn has_await_in_stmts(stmts: &[Box<Stmt>]) -> bool {
    let mut probe = YieldLikeProbe::default();
    for stmt in stmts {
        let mut stmt = stmt.as_ref().clone();
        probe.visit_stmt(&mut stmt);
        if probe.has_await {
            return true;
        }
    }
    false
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

fn maybe_wrap_function_metadata_expr(
    base_expr: Expr,
    doc_expr: Option<Expr>,
    annotate_fn_expr: Option<Expr>,
) -> Expr {
    if doc_expr.is_none() && annotate_fn_expr.is_none() {
        return base_expr;
    }

    py_expr!(
        "__dp__.apply_fn_metadata({fn_obj:expr}, {doc:expr}, {annotate_fn:expr})",
        fn_obj = base_expr,
        doc = doc_expr.unwrap_or_else(|| py_expr!("None")),
        annotate_fn = annotate_fn_expr.unwrap_or_else(|| py_expr!("None")),
    )
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
    for block in blocks {
        for stmt in &block.body {
            defs_anywhere.extend(assigned_names_in_stmt(stmt));
        }
    }

    let mut state = param_names.to_vec();
    for block in blocks {
        let (uses, defs) = analyze_block_use_def(block);
        let mut names = defs.into_iter().collect::<Vec<_>>();
        for name in uses {
            let is_special_runtime_state =
                name == "_dp_state" || name.starts_with("_dp_cell_") || name == "_dp_classcell";
            let is_known_local = defs_anywhere.contains(name.as_str())
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

fn build_extra_successors(blocks: &[Block]) -> HashMap<String, Vec<String>> {
    let mut extra = HashMap::new();
    for block in blocks {
        if let Terminator::TryJump {
            body_region_labels,
            except_region_labels,
            finally_label: Some(finally_label),
            ..
        } = &block.terminator
        {
            for label in body_region_labels.iter().chain(except_region_labels.iter()) {
                extra
                    .entry(label.clone())
                    .or_insert_with(Vec::new)
                    .push(finally_label.clone());
            }
        }
    }
    extra
}

fn compute_block_params(
    blocks: &[Block],
    state_order: &[String],
    extra_successors: &HashMap<String, Vec<String>>,
) -> HashMap<String, Vec<String>> {
    let label_to_index: HashMap<&str, usize> = blocks
        .iter()
        .enumerate()
        .map(|(idx, block)| (block.label.as_str(), idx))
        .collect();
    let analyses: Vec<(HashSet<String>, HashSet<String>)> =
        blocks.iter().map(analyze_block_use_def).collect();
    let mut live_in: Vec<HashSet<String>> = vec![HashSet::new(); blocks.len()];
    let mut live_out: Vec<HashSet<String>> = vec![HashSet::new(); blocks.len()];

    let mut changed = true;
    while changed {
        changed = false;
        for (idx, block) in blocks.iter().enumerate().rev() {
            let mut out = HashSet::new();
            for succ in block.successors() {
                if let Some(succ_idx) = label_to_index.get(succ.as_str()) {
                    out.extend(live_in[*succ_idx].iter().cloned());
                }
            }
            if let Some(extra) = extra_successors.get(block.label.as_str()) {
                for succ in extra {
                    if let Some(succ_idx) = label_to_index.get(succ.as_str()) {
                        out.extend(live_in[*succ_idx].iter().cloned());
                    }
                }
            }
            let (uses, defs) = &analyses[idx];
            let mut incoming = uses.clone();
            for name in &out {
                if !defs.contains(name) {
                    incoming.insert(name.clone());
                }
            }
            if incoming != live_in[idx] || out != live_out[idx] {
                changed = true;
                live_in[idx] = incoming;
                live_out[idx] = out;
            }
        }
    }

    let mut params = HashMap::new();
    for (idx, block) in blocks.iter().enumerate() {
        let ordered = state_order
            .iter()
            .filter(|name| live_in[idx].contains(name.as_str()))
            .cloned()
            .collect::<Vec<_>>();
        params.insert(block.label.clone(), ordered);
    }
    params
}

fn analyze_block_use_def(block: &Block) -> (HashSet<String>, HashSet<String>) {
    let mut uses = HashSet::new();
    let mut defs = HashSet::new();

    for stmt in &block.body {
        for name in load_names_in_stmt(stmt) {
            if !defs.contains(name.as_str()) {
                uses.insert(name);
            }
        }
        for name in assigned_names_in_stmt(stmt) {
            defs.insert(name);
        }
    }

    for name in assigned_names_in_terminator(&block.terminator) {
        defs.insert(name);
    }

    for name in load_names_in_terminator(&block.terminator) {
        if !defs.contains(name.as_str()) {
            uses.insert(name);
        }
    }

    (uses, defs)
}

fn assigned_names_in_terminator(terminator: &Terminator) -> HashSet<String> {
    match terminator {
        Terminator::Jump(_)
        | Terminator::BrIf { .. }
        | Terminator::Raise(_)
        | Terminator::TryJump { .. }
        | Terminator::Yield { .. }
        | Terminator::Ret(_) => HashSet::new(),
    }
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

            // Nested transformed functions can require outer closure cells at
            // definition time so the created function captures those cells.
            // We only care about transformed cell names, not generic body
            // loads.
            let mut full_stmt = Stmt::FunctionDef(func_def.clone());
            let mut body_collector = LoadNameCollector::default();
            body_collector.visit_stmt(&mut full_stmt);
            for name in body_collector.names {
                if name.starts_with("_dp_cell_") {
                    names.insert(name);
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

fn collect_deleted_names(stmts: &[Box<Stmt>]) -> HashSet<String> {
    let mut names = HashSet::new();
    for stmt in stmts {
        collect_deleted_names_in_stmt(stmt.as_ref(), &mut names);
    }
    names
}

fn collect_deleted_names_in_stmt(stmt: &Stmt, names: &mut HashSet<String>) {
    match stmt {
        Stmt::Delete(delete_stmt) => {
            for target in &delete_stmt.targets {
                collect_deleted_names_in_target(target, names);
            }
        }
        Stmt::If(if_stmt) => {
            for stmt in &if_stmt.body.body {
                collect_deleted_names_in_stmt(stmt.as_ref(), names);
            }
            for clause in &if_stmt.elif_else_clauses {
                for stmt in &clause.body.body {
                    collect_deleted_names_in_stmt(stmt.as_ref(), names);
                }
            }
        }
        Stmt::While(while_stmt) => {
            for stmt in &while_stmt.body.body {
                collect_deleted_names_in_stmt(stmt.as_ref(), names);
            }
            for stmt in &while_stmt.orelse.body {
                collect_deleted_names_in_stmt(stmt.as_ref(), names);
            }
        }
        Stmt::For(for_stmt) => {
            for stmt in &for_stmt.body.body {
                collect_deleted_names_in_stmt(stmt.as_ref(), names);
            }
            for stmt in &for_stmt.orelse.body {
                collect_deleted_names_in_stmt(stmt.as_ref(), names);
            }
        }
        Stmt::Try(try_stmt) => {
            for stmt in &try_stmt.body.body {
                collect_deleted_names_in_stmt(stmt.as_ref(), names);
            }
            for handler in &try_stmt.handlers {
                let ast::ExceptHandler::ExceptHandler(handler) = handler;
                for stmt in &handler.body.body {
                    collect_deleted_names_in_stmt(stmt.as_ref(), names);
                }
            }
            for stmt in &try_stmt.orelse.body {
                collect_deleted_names_in_stmt(stmt.as_ref(), names);
            }
            for stmt in &try_stmt.finalbody.body {
                collect_deleted_names_in_stmt(stmt.as_ref(), names);
            }
        }
        Stmt::FunctionDef(_) | Stmt::ClassDef(_) => {}
        _ => {}
    }
}

fn collect_deleted_names_in_target(target: &Expr, names: &mut HashSet<String>) {
    match target {
        Expr::Name(name) => {
            names.insert(name.id.to_string());
        }
        Expr::Tuple(tuple) => {
            for elt in &tuple.elts {
                collect_deleted_names_in_target(elt, names);
            }
        }
        Expr::List(list) => {
            for elt in &list.elts {
                collect_deleted_names_in_target(elt, names);
            }
        }
        Expr::Starred(starred) => collect_deleted_names_in_target(starred.value.as_ref(), names),
        _ => {}
    }
}

fn rewrite_delete_to_deleted_sentinel(delete_stmt: &ast::StmtDelete) -> Vec<Stmt> {
    let mut out = Vec::new();
    for target in &delete_stmt.targets {
        rewrite_delete_target_to_deleted_sentinel(target, &mut out);
    }
    out
}

fn rewrite_delete_target_to_deleted_sentinel(target: &Expr, out: &mut Vec<Stmt>) {
    match target {
        Expr::Name(name) => {
            out.push(py_stmt!(
                "{name:id} = __dp__.DELETED",
                name = name.id.as_str(),
            ));
        }
        Expr::Tuple(tuple) => {
            for elt in &tuple.elts {
                rewrite_delete_target_to_deleted_sentinel(elt, out);
            }
        }
        Expr::List(list) => {
            for elt in &list.elts {
                rewrite_delete_target_to_deleted_sentinel(elt, out);
            }
        }
        Expr::Starred(starred) => {
            rewrite_delete_target_to_deleted_sentinel(starred.value.as_ref(), out);
        }
        _ => out.push(py_stmt!("del {target:expr}", target = target.clone())),
    }
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
                "__dp__.store_cell({cell:id}, {value:id})",
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

fn rewrite_deleted_name_loads(
    blocks: &mut [Block],
    deleted_names: &HashSet<String>,
    always_unbound_names: &HashSet<String>,
) {
    let mut rewriter = DeletedNameLoadRewriter {
        deleted_names,
        always_unbound_names,
    };
    for block in blocks {
        for stmt in block.body.iter_mut() {
            rewriter.visit_stmt(stmt);
        }
        match &mut block.terminator {
            Terminator::BrIf { test, .. } => rewriter.visit_expr(test),
            Terminator::Raise(raise_stmt) => {
                if let Some(exc) = raise_stmt.exc.as_mut() {
                    rewriter.visit_expr(exc.as_mut());
                }
                if let Some(cause) = raise_stmt.cause.as_mut() {
                    rewriter.visit_expr(cause.as_mut());
                }
            }
            Terminator::Yield { value, .. } => {
                if let Some(value) = value {
                    rewriter.visit_expr(value);
                }
            }
            Terminator::Ret(Some(value)) => rewriter.visit_expr(value),
            Terminator::Jump(_) | Terminator::TryJump { .. } | Terminator::Ret(None) => {}
        }
    }
}

struct DeletedNameLoadRewriter<'a> {
    deleted_names: &'a HashSet<String>,
    always_unbound_names: &'a HashSet<String>,
}

impl Transformer for DeletedNameLoadRewriter<'_> {
    fn visit_stmt(&mut self, stmt: &mut Stmt) {
        match stmt {
            Stmt::FunctionDef(_) | Stmt::ClassDef(_) | Stmt::Delete(_) => {}
            Stmt::Expr(expr_stmt) => self.visit_expr(expr_stmt.value.as_mut()),
            Stmt::Assign(assign) => {
                // Only loads are rewritten; assignment targets are intentionally
                // excluded, even if their AST context is unexpectedly Load.
                self.visit_expr(assign.value.as_mut());
            }
            Stmt::AugAssign(aug_assign) => {
                self.visit_expr(aug_assign.target.as_mut());
                self.visit_expr(aug_assign.value.as_mut());
            }
            Stmt::Return(ret) => {
                if let Some(value) = ret.value.as_mut() {
                    self.visit_expr(value.as_mut());
                }
            }
            Stmt::Raise(raise_stmt) => {
                if let Some(exc) = raise_stmt.exc.as_mut() {
                    self.visit_expr(exc.as_mut());
                }
                if let Some(cause) = raise_stmt.cause.as_mut() {
                    self.visit_expr(cause.as_mut());
                }
            }
            Stmt::If(if_stmt) => {
                self.visit_expr(if_stmt.test.as_mut());
                self.visit_body(&mut if_stmt.body);
                for clause in if_stmt.elif_else_clauses.iter_mut() {
                    if let Some(test) = clause.test.as_mut() {
                        self.visit_expr(test);
                    }
                    self.visit_body(&mut clause.body);
                }
            }
            Stmt::While(while_stmt) => {
                self.visit_expr(while_stmt.test.as_mut());
                self.visit_body(&mut while_stmt.body);
                self.visit_body(&mut while_stmt.orelse);
            }
            Stmt::For(for_stmt) => {
                self.visit_expr(for_stmt.iter.as_mut());
                self.visit_body(&mut for_stmt.body);
                self.visit_body(&mut for_stmt.orelse);
            }
            Stmt::Try(try_stmt) => {
                self.visit_body(&mut try_stmt.body);
                for handler in try_stmt.handlers.iter_mut() {
                    let ast::ExceptHandler::ExceptHandler(handler) = handler;
                    if let Some(type_) = handler.type_.as_mut() {
                        self.visit_expr(type_.as_mut());
                    }
                    self.visit_body(&mut handler.body);
                }
                self.visit_body(&mut try_stmt.orelse);
                self.visit_body(&mut try_stmt.finalbody);
            }
            _ => walk_stmt(self, stmt),
        }
    }

    fn visit_expr(&mut self, expr: &mut Expr) {
        if let Expr::Name(name) = expr {
            if matches!(name.ctx, ast::ExprContext::Load) {
                let always_unbound = self.always_unbound_names.contains(name.id.as_str());
                let deleted = self.deleted_names.contains(name.id.as_str());
                if !always_unbound && !deleted {
                    walk_expr(self, expr);
                    return;
                }
                let value = if always_unbound {
                    py_expr!("__dp__.DELETED")
                } else {
                    Expr::Name(name.clone())
                };
                let name_value = name.id.to_string();
                *expr = py_expr!(
                    "__dp__.load_deleted_name({name:literal}, {value:expr})",
                    name = name_value.as_str(),
                    value = value,
                );
                return;
            }
        }
        walk_expr(self, expr);
    }
}

fn collect_bound_names(stmts: &[Box<Stmt>]) -> HashSet<String> {
    let mut names = HashSet::new();
    for stmt in stmts {
        collect_bound_names_in_stmt(stmt.as_ref(), &mut names);
    }
    names
}

fn collect_bound_names_in_stmt(stmt: &Stmt, names: &mut HashSet<String>) {
    match stmt {
        Stmt::Assign(assign) => {
            for target in &assign.targets {
                collect_assigned_names(target, names);
            }
        }
        Stmt::AugAssign(aug) => collect_assigned_names(aug.target.as_ref(), names),
        Stmt::AnnAssign(ann) => collect_assigned_names(ann.target.as_ref(), names),
        Stmt::For(for_stmt) => {
            collect_assigned_names(for_stmt.target.as_ref(), names);
            for child in &for_stmt.body.body {
                collect_bound_names_in_stmt(child.as_ref(), names);
            }
            for child in &for_stmt.orelse.body {
                collect_bound_names_in_stmt(child.as_ref(), names);
            }
        }
        Stmt::While(while_stmt) => {
            for child in &while_stmt.body.body {
                collect_bound_names_in_stmt(child.as_ref(), names);
            }
            for child in &while_stmt.orelse.body {
                collect_bound_names_in_stmt(child.as_ref(), names);
            }
        }
        Stmt::If(if_stmt) => {
            for child in &if_stmt.body.body {
                collect_bound_names_in_stmt(child.as_ref(), names);
            }
            for clause in &if_stmt.elif_else_clauses {
                for child in &clause.body.body {
                    collect_bound_names_in_stmt(child.as_ref(), names);
                }
            }
        }
        Stmt::Try(try_stmt) => {
            for child in &try_stmt.body.body {
                collect_bound_names_in_stmt(child.as_ref(), names);
            }
            for handler in &try_stmt.handlers {
                let ast::ExceptHandler::ExceptHandler(handler) = handler;
                if let Some(name) = handler.name.as_ref() {
                    names.insert(name.id.to_string());
                }
                for child in &handler.body.body {
                    collect_bound_names_in_stmt(child.as_ref(), names);
                }
            }
            for child in &try_stmt.orelse.body {
                collect_bound_names_in_stmt(child.as_ref(), names);
            }
            for child in &try_stmt.finalbody.body {
                collect_bound_names_in_stmt(child.as_ref(), names);
            }
        }
        Stmt::With(with_stmt) => {
            for item in &with_stmt.items {
                if let Some(optional_vars) = item.optional_vars.as_ref() {
                    collect_assigned_names(optional_vars.as_ref(), names);
                }
            }
            for child in &with_stmt.body.body {
                collect_bound_names_in_stmt(child.as_ref(), names);
            }
        }
        Stmt::Delete(delete_stmt) => {
            for target in &delete_stmt.targets {
                collect_assigned_names(target, names);
            }
        }
        Stmt::FunctionDef(func_def) => {
            names.insert(func_def.name.id.to_string());
        }
        Stmt::ClassDef(class_def) => {
            names.insert(class_def.name.id.to_string());
        }
        _ => {}
    }
}

fn collect_explicit_global_or_nonlocal_names(stmts: &[Box<Stmt>]) -> HashSet<String> {
    let mut names = HashSet::new();
    for stmt in stmts {
        collect_explicit_global_or_nonlocal_names_in_stmt(stmt.as_ref(), &mut names);
    }
    names
}

fn collect_explicit_global_or_nonlocal_names_in_stmt(stmt: &Stmt, names: &mut HashSet<String>) {
    match stmt {
        Stmt::Global(global_stmt) => {
            for name in &global_stmt.names {
                names.insert(name.id.to_string());
            }
        }
        Stmt::Nonlocal(nonlocal_stmt) => {
            for name in &nonlocal_stmt.names {
                names.insert(name.id.to_string());
            }
        }
        Stmt::If(if_stmt) => {
            for child in &if_stmt.body.body {
                collect_explicit_global_or_nonlocal_names_in_stmt(child.as_ref(), names);
            }
            for clause in &if_stmt.elif_else_clauses {
                for child in &clause.body.body {
                    collect_explicit_global_or_nonlocal_names_in_stmt(child.as_ref(), names);
                }
            }
        }
        Stmt::While(while_stmt) => {
            for child in &while_stmt.body.body {
                collect_explicit_global_or_nonlocal_names_in_stmt(child.as_ref(), names);
            }
            for child in &while_stmt.orelse.body {
                collect_explicit_global_or_nonlocal_names_in_stmt(child.as_ref(), names);
            }
        }
        Stmt::For(for_stmt) => {
            for child in &for_stmt.body.body {
                collect_explicit_global_or_nonlocal_names_in_stmt(child.as_ref(), names);
            }
            for child in &for_stmt.orelse.body {
                collect_explicit_global_or_nonlocal_names_in_stmt(child.as_ref(), names);
            }
        }
        Stmt::Try(try_stmt) => {
            for child in &try_stmt.body.body {
                collect_explicit_global_or_nonlocal_names_in_stmt(child.as_ref(), names);
            }
            for handler in &try_stmt.handlers {
                let ast::ExceptHandler::ExceptHandler(handler) = handler;
                for child in &handler.body.body {
                    collect_explicit_global_or_nonlocal_names_in_stmt(child.as_ref(), names);
                }
            }
            for child in &try_stmt.orelse.body {
                collect_explicit_global_or_nonlocal_names_in_stmt(child.as_ref(), names);
            }
            for child in &try_stmt.finalbody.body {
                collect_explicit_global_or_nonlocal_names_in_stmt(child.as_ref(), names);
            }
        }
        Stmt::With(with_stmt) => {
            for child in &with_stmt.body.body {
                collect_explicit_global_or_nonlocal_names_in_stmt(child.as_ref(), names);
            }
        }
        _ => {}
    }
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
    specs.push(make_tuple(vec![
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
    make_tuple(specs)
}

fn raise_stmt_from_name(name: &str) -> ast::StmtRaise {
    match py_stmt!("raise {exc:id}", exc = name) {
        Stmt::Raise(raise_stmt) => raise_stmt,
        _ => unreachable!("expected raise statement"),
    }
}

fn sanitize_ident(raw: &str) -> String {
    raw.chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '_' {
                ch
            } else {
                '_'
            }
        })
        .collect()
}

fn original_function_name(fn_name: &str) -> String {
    let Some(rest) = fn_name.strip_prefix("_dp_fn_") else {
        return fn_name.to_string();
    };
    let Some((prefix, trailing)) = rest.rsplit_once('_') else {
        return rest.to_string();
    };
    if !trailing.is_empty() && trailing.chars().all(|ch| ch.is_ascii_digit()) {
        prefix.to_string()
    } else {
        rest.to_string()
    }
}

fn apply_label_rename(
    entry_label: &str,
    rename: &HashMap<String, String>,
    blocks: &mut [Block],
) -> String {
    let known_labels: HashSet<String> = blocks.iter().map(|block| block.label.clone()).collect();

    for block in blocks.iter_mut() {
        let new_label = rename
            .get(block.label.as_str())
            .cloned()
            .unwrap_or_else(|| block.label.clone());
        block.label = new_label;
        let mut body_renamer = LabelNameRenamer { rename };
        for stmt in block.body.iter_mut() {
            body_renamer.visit_stmt(stmt);
        }
        match &mut block.terminator {
            Terminator::Jump(target) => {
                if let Some(renamed) = rename.get(target.as_str()) {
                    *target = renamed.clone();
                } else if !known_labels.contains(target.as_str()) {
                    panic!("missing renamed jump target: {target}");
                }
            }
            Terminator::BrIf {
                then_label,
                else_label,
                ..
            } => {
                if let Some(renamed) = rename.get(then_label.as_str()) {
                    *then_label = renamed.clone();
                } else if !known_labels.contains(then_label.as_str()) {
                    panic!("missing renamed true target: {then_label}");
                }
                if let Some(renamed) = rename.get(else_label.as_str()) {
                    *else_label = renamed.clone();
                } else if !known_labels.contains(else_label.as_str()) {
                    panic!("missing renamed false target: {else_label}");
                }
            }
            Terminator::Raise(raise_stmt) => {
                if let Some(exc) = raise_stmt.exc.as_mut() {
                    body_renamer.visit_expr(exc.as_mut());
                }
                if let Some(cause) = raise_stmt.cause.as_mut() {
                    body_renamer.visit_expr(cause.as_mut());
                }
            }
            Terminator::TryJump {
                body_label,
                except_label,
                body_region_labels,
                except_region_labels,
                finally_label,
                finally_region_labels,
                finally_fallthrough_label,
            } => {
                if let Some(renamed) = rename.get(body_label.as_str()) {
                    *body_label = renamed.clone();
                } else if !known_labels.contains(body_label.as_str()) {
                    panic!("missing renamed try body target: {body_label}");
                }
                if let Some(renamed) = rename.get(except_label.as_str()) {
                    *except_label = renamed.clone();
                } else if !known_labels.contains(except_label.as_str()) {
                    panic!("missing renamed except target: {except_label}");
                }
                let mut renamed_body_region = Vec::new();
                for label in body_region_labels.iter() {
                    if let Some(renamed) = rename.get(label.as_str()) {
                        renamed_body_region.push(renamed.clone());
                    } else if known_labels.contains(label.as_str()) {
                        renamed_body_region.push(label.clone());
                    }
                }
                *body_region_labels = renamed_body_region;

                let mut renamed_except_region = Vec::new();
                for label in except_region_labels.iter() {
                    if let Some(renamed) = rename.get(label.as_str()) {
                        renamed_except_region.push(renamed.clone());
                    } else if known_labels.contains(label.as_str()) {
                        renamed_except_region.push(label.clone());
                    }
                }
                *except_region_labels = renamed_except_region;

                if let Some(finally_label_value) = finally_label.as_mut() {
                    if let Some(renamed) = rename.get(finally_label_value.as_str()) {
                        *finally_label_value = renamed.clone();
                    }
                }
                let mut renamed_finally_region = Vec::new();
                for label in finally_region_labels.iter() {
                    if let Some(renamed) = rename.get(label.as_str()) {
                        renamed_finally_region.push(renamed.clone());
                    } else if known_labels.contains(label.as_str()) {
                        renamed_finally_region.push(label.clone());
                    }
                }
                *finally_region_labels = renamed_finally_region;

                if let Some(finally_fallthrough_label_value) = finally_fallthrough_label.as_mut() {
                    if let Some(renamed) = rename.get(finally_fallthrough_label_value.as_str()) {
                        *finally_fallthrough_label_value = renamed.clone();
                    }
                }
            }
            Terminator::Yield { resume_label, .. } => {
                if let Some(renamed) = rename.get(resume_label.as_str()) {
                    *resume_label = renamed.clone();
                } else if !known_labels.contains(resume_label.as_str()) {
                    panic!("missing renamed yield resume target: {resume_label}");
                }
            }
            Terminator::Ret(_) => {}
        }
    }

    rename
        .get(entry_label)
        .cloned()
        .unwrap_or_else(|| entry_label.to_string())
}

fn relabel_blocks(prefix: &str, entry_label: &str, blocks: &mut [Block]) -> String {
    let mut rename = HashMap::new();
    rename.insert(entry_label.to_string(), format!("{prefix}_start"));

    let mut next_id = 0usize;
    for block in blocks.iter() {
        if rename.contains_key(block.label.as_str()) {
            continue;
        }
        rename.insert(block.label.clone(), format!("{prefix}_{next_id}"));
        next_id += 1;
    }

    apply_label_rename(entry_label, &rename, blocks)
}

struct LabelNameRenamer<'a> {
    rename: &'a HashMap<String, String>,
}

impl Transformer for LabelNameRenamer<'_> {
    fn visit_expr(&mut self, expr: &mut Expr) {
        if let Expr::Name(name) = expr {
            if let Some(rewritten) = self.rename.get(name.id.as_str()) {
                name.id = rewritten.as_str().into();
            }
        }
        walk_expr(self, expr);
    }
}

fn fold_jumps_to_trivial_none_return(blocks: &mut [Block]) {
    let trivial_ret_none_labels: HashSet<String> = blocks
        .iter()
        .filter(|block| block.body.is_empty() && matches!(block.terminator, Terminator::Ret(None)))
        .map(|block| block.label.clone())
        .collect();

    for block in blocks.iter_mut() {
        let jump_target = match &block.terminator {
            Terminator::Jump(target) => Some(target.clone()),
            _ => None,
        };
        if let Some(target) = jump_target {
            if trivial_ret_none_labels.contains(target.as_str()) {
                block.terminator = Terminator::Ret(None);
            }
        }
    }
}

fn fold_constant_brif(blocks: &mut [Block]) {
    for block in blocks.iter_mut() {
        let jump_target = match &block.terminator {
            Terminator::BrIf {
                test,
                then_label,
                else_label,
            } => match test {
                Expr::BooleanLiteral(boolean) => {
                    if boolean.value {
                        Some(then_label.clone())
                    } else {
                        Some(else_label.clone())
                    }
                }
                _ => None,
            },
            _ => None,
        };
        if let Some(target) = jump_target {
            block.terminator = Terminator::Jump(target);
        }
    }
}

fn prune_unreachable_blocks(entry_label: &str, blocks: &mut Vec<Block>) {
    let index_by_label: HashMap<String, usize> = blocks
        .iter()
        .enumerate()
        .map(|(idx, block)| (block.label.clone(), idx))
        .collect();

    let mut worklist = vec![entry_label.to_string()];
    let mut reachable = HashSet::new();
    while let Some(label) = worklist.pop() {
        if !reachable.insert(label.clone()) {
            continue;
        }
        let Some(idx) = index_by_label.get(label.as_str()) else {
            continue;
        };
        for succ in blocks[*idx].successors() {
            worklist.push(succ);
        }
    }

    blocks.retain(|block| reachable.contains(block.label.as_str()));
}

fn capture_except_exception(mut body: Vec<Box<Stmt>>, exc_name: &str) -> Vec<Box<Stmt>> {
    let mut out = Vec::with_capacity(body.len() + 1);
    out.push(Box::new(py_stmt!(
        "{exc:id} = __dp__.current_exception()",
        exc = exc_name,
    )));
    let mut rewriter = ExceptExceptionRewriter {
        exception_name: exc_name.to_string(),
    };
    for stmt in body.iter_mut() {
        rewriter.visit_stmt(stmt.as_mut());
    }
    out.extend(body);
    out
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
                if let Expr::Attribute(attr) = call.func.as_ref() {
                    if attr.attr.as_str() == "current_exception" {
                        if let Expr::Name(module) = attr.value.as_ref() {
                            if module.id.as_str() == "__dp__" {
                                *expr = self.exception_name_expr();
                                return;
                            }
                        }
                    }
                }
            }
        }
        walk_expr(self, expr);
    }
}

fn name_expr(name: &str) -> Option<Expr> {
    parse_expression(name)
        .ok()
        .map(|expr| *expr.into_syntax().body)
}

#[cfg(test)]
mod tests {
    use crate::{
        transform::Options, transform_str_to_bb_ir_with_options, transform_str_to_ruff_with_options,
    };

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
        let lowered = transform_str_to_ruff_with_options(source, options)
            .expect("transform should succeed")
            .to_string();

        assert!(lowered.contains("__dp__.def_fn("), "{lowered}");
        assert!(lowered.contains("__dp__.brif("), "{lowered}");
        assert!(lowered.contains("def _dp_bb_"), "{lowered}");
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
        let lowered = transform_str_to_ruff_with_options(source, options)
            .expect("transform should succeed")
            .to_string();

        assert!(lowered.contains("__dp__.def_fn("), "{lowered}");
        assert!(lowered.contains("__dp__.brif("), "{lowered}");
        assert!(lowered.contains("__dp__.jump("), "{lowered}");
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
        let lowered = transform_str_to_ruff_with_options(source, options)
            .expect("transform should succeed")
            .to_string();

        assert!(lowered.contains("__dp__.next_or_sentinel("), "{lowered}");
        assert!(lowered.contains("__dp__.iter("), "{lowered}");
        assert!(lowered.contains("__dp__.brif("), "{lowered}");
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
        let lowered = transform_str_to_ruff_with_options(source, options)
            .expect("transform should succeed")
            .to_string();

        assert!(lowered.contains("__dp__.def_fn("), "{lowered}");
        assert!(lowered.contains("def _dp_bb_f_start"), "{lowered}");
        assert!(!lowered.contains("def _dp_bb_f_0"), "{lowered}");
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
        let lowered = transform_str_to_ruff_with_options(source, options)
            .expect("transform should succeed")
            .to_string();

        assert!(lowered.contains("__dp__.ret(None)"), "{lowered}");
        assert!(!lowered.contains("__dp__.jump("), "{lowered}");
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
        let lowered = transform_str_to_ruff_with_options(source, options)
            .expect("transform should succeed")
            .to_string();

        assert!(lowered.contains("_dp_bb_outer_start"), "{lowered}");
        assert!(lowered.contains("def _dp_bb_inner_start"), "{lowered}");
        assert!(
            lowered.contains("(\"_dp_cell_x\", _dp_cell_x)"),
            "{lowered}"
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
        let lowered = transform_str_to_ruff_with_options(source, options)
            .expect("transform should succeed")
            .to_string();

        assert!(lowered.contains("__dp__.try_jump_term("), "{lowered}");
        assert!(!lowered.contains("finally:"), "{lowered}");
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
        let lowered = transform_str_to_ruff_with_options(source, options)
            .expect("transform should succeed")
            .to_string();

        assert!(lowered.contains("__dp__.try_jump_term("), "{lowered}");
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
        let lowered = transform_str_to_ruff_with_options(source, options)
            .expect("transform should succeed")
            .to_string();

        assert!(
            lowered.contains("__dp__.exceptiongroup_split("),
            "{lowered}"
        );
        assert!(lowered.contains("__dp__.try_jump_term("), "{lowered}");
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
        let lowered = transform_str_to_ruff_with_options(source, options)
            .expect("transform should succeed")
            .to_string();

        assert!(
            lowered.contains("__dp__.load_deleted_name(\"x\", __dp__.DELETED)"),
            "{lowered}"
        );
        assert!(!lowered.contains("x = 1"), "{lowered}");
    }
}
