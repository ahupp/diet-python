use crate::block_py::param_specs::{collect_param_spec_and_defaults, param_defaults_to_expr};
use crate::block_py::{
    BlockPyCallableSemanticInfo, BlockPyFunction, BlockPyFunctionKind, BlockPyModule,
    ClosureLayout, FunctionNameGen, ModuleNameGen,
};
use crate::passes::ast_to_ast::body::{split_docstring, Suite};
use crate::passes::ast_to_ast::context::Context;
use crate::passes::ast_to_ast::expr_utils::make_dp_tuple;
use crate::passes::ast_to_ast::rewrite_stmt;
use crate::passes::ast_to_ast::semantic::{SemanticAstState, SemanticScope};
use crate::passes::ruff_to_blockpy::recompute_semantic_blockpy_closure_layout;
use crate::passes::RuffBlockPyPass;
use crate::transformer::{walk_expr, walk_stmt, Transformer};
use crate::{py_expr, py_stmt};
use ruff_python_ast::{self as ast, Expr, Stmt};

use super::build_blockpy_callable_def_from_runtime_input;
mod callable_semantic;
use callable_semantic::callable_semantic_info;

struct FunctionScopeFrame {
    scope: Option<SemanticScope>,
    callable_semantic: BlockPyCallableSemanticInfo,
    hoisted_to_parent: Vec<Stmt>,
}

struct BlockPyModuleRewriter<'a> {
    context: &'a Context,
    semantic_state: &'a SemanticAstState,
    module_name_gen: ModuleNameGen,
    function_scope_stack: Vec<FunctionScopeFrame>,
    callable_defs: Vec<BlockPyFunction<RuffBlockPyPass>>,
}

#[derive(Default)]
struct YieldFamilyDetector {
    found: bool,
}

pub(crate) fn rewrite_ast_to_lowered_blockpy_module_plan_with_module(
    context: &Context,
    mut module: Suite,
    semantic_state: &SemanticAstState,
) -> BlockPyModule<RuffBlockPyPass> {
    crate::passes::ast_to_ast::simplify::flatten(&mut module);
    let mut rewriter = BlockPyModuleRewriter {
        context,
        semantic_state,
        module_name_gen: ModuleNameGen::new(0),
        function_scope_stack: Vec::new(),
        callable_defs: Vec::new(),
    };
    let module_init = BlockPyModuleRewriter::root_module_init_stmt(&mut module);
    rewriter.lower_root_function_def(module_init);
    BlockPyModule {
        callable_defs: rewriter.callable_defs,
    }
}

impl Transformer for YieldFamilyDetector {
    fn visit_stmt(&mut self, stmt: &mut Stmt) {
        match stmt {
            Stmt::FunctionDef(_) | Stmt::ClassDef(_) => {}
            other => walk_stmt(self, other),
        }
    }

    fn visit_expr(&mut self, expr: &mut Expr) {
        match expr {
            Expr::Yield(_) | Expr::YieldFrom(_) => {
                self.found = true;
            }
            Expr::Lambda(_)
            | Expr::Generator(_)
            | Expr::ListComp(_)
            | Expr::SetComp(_)
            | Expr::DictComp(_) => {}
            other => walk_expr(self, other),
        }
    }
}

fn function_kind(func: &ast::StmtFunctionDef) -> BlockPyFunctionKind {
    let mut detector = YieldFamilyDetector::default();
    let mut body = func.body.to_vec();
    detector.visit_body(&mut body);
    match (func.is_async, detector.found) {
        (false, false) => BlockPyFunctionKind::Function,
        (false, true) => BlockPyFunctionKind::Generator,
        (true, false) => BlockPyFunctionKind::Coroutine,
        (true, true) => BlockPyFunctionKind::AsyncGenerator,
    }
}

fn try_lower_function_to_blockpy_bundle(
    context: &Context,
    func: &ast::StmtFunctionDef,
    callable_semantic: &BlockPyCallableSemanticInfo,
    name_gen: FunctionNameGen,
) -> BlockPyFunction<RuffBlockPyPass> {
    let (docstring, lowered_input_body) = split_docstring(&func.body);
    let lowered_input_body = lowered_input_body.to_vec();
    let (param_spec, _param_defaults) = collect_param_spec_and_defaults(&func.parameters);

    let end_label = name_gen.next_block_name();

    build_blockpy_callable_def_from_runtime_input(
        context,
        name_gen,
        callable_semantic.names.clone(),
        param_spec,
        &lowered_input_body,
        docstring,
        end_label,
        function_kind(func),
        callable_semantic,
    )
}

// Function-definition rewriting stays in one tree pass, but the instantiation
// machinery is grouped here so the later binding split has one obvious home.
fn capture_items_to_expr(captures: &[(String, Expr)]) -> Expr {
    make_dp_tuple(
        captures
            .iter()
            .map(|(name, value_expr)| {
                make_dp_tuple(vec![
                    py_expr!("{value:literal}", value = name.as_str()),
                    value_expr.clone(),
                ])
            })
            .collect(),
    )
}

fn closure_freevar_capture_items(
    closure_layout: Option<&ClosureLayout>,
    _semantic: &BlockPyCallableSemanticInfo,
) -> Vec<(String, Expr)> {
    closure_layout
        .into_iter()
        .flat_map(|layout| layout.freevars.iter())
        .map(|slot| {
            (
                slot.logical_name.clone(),
                py_expr!(
                    "__dp_cell_ref({name:literal})",
                    name = slot.logical_name.as_str()
                ),
            )
        })
        .collect()
}

fn build_lowered_function_instantiation_expr(
    function_id: crate::block_py::FunctionId,
    closure_layout: Option<&ClosureLayout>,
    semantic: &BlockPyCallableSemanticInfo,
    decorator_exprs: Vec<Expr>,
    param_defaults: &[Expr],
    annotate_fn_expr: Expr,
    kind: BlockPyFunctionKind,
) -> Expr {
    let captures = closure_freevar_capture_items(closure_layout, semantic);
    let capture_expr = capture_items_to_expr(&captures);
    let param_defaults_expr = param_defaults_to_expr(param_defaults);
    let kind_name = match kind {
        BlockPyFunctionKind::Function => "function",
        BlockPyFunctionKind::Coroutine => "coroutine",
        BlockPyFunctionKind::Generator => "generator",
        BlockPyFunctionKind::AsyncGenerator => "async_generator",
    };
    let base_function_expr = py_expr!(
        "__dp_make_function({function_id:literal}, {kind:literal}, {closure:expr}, {param_defaults:expr}, {module_globals:expr}, {annotate_fn:expr})",
        function_id = function_id.0,
        kind = kind_name,
        closure = capture_expr.clone(),
        param_defaults = param_defaults_expr.clone(),
        module_globals = py_expr!("__dp_globals()"),
        annotate_fn = annotate_fn_expr.clone(),
    );
    rewrite_stmt::decorator::rewrite_exprs(decorator_exprs, base_function_expr)
}

#[allow(clippy::too_many_arguments)]
fn rewrite_function_def_stmt_via_blockpy(
    context: &Context,
    parent_hoisted: &mut Vec<Stmt>,
    func: &mut ast::StmtFunctionDef,
    callable_semantic: &BlockPyCallableSemanticInfo,
    function_hoisted: Vec<Stmt>,
    module_name_gen: &mut ModuleNameGen,
    callable_defs: &mut Vec<BlockPyFunction<RuffBlockPyPass>>,
) -> Vec<Stmt> {
    let name_gen = module_name_gen.next_function_name_gen();
    let mut lowered_plan =
        try_lower_function_to_blockpy_bundle(context, func, callable_semantic, name_gen);
    lowered_plan.closure_layout = recompute_semantic_blockpy_closure_layout(&lowered_plan);
    let bind_name = lowered_plan.names.bind_name.clone();
    let (_, param_defaults) = collect_param_spec_and_defaults(&func.parameters);
    let decorated = build_lowered_function_instantiation_expr(
        lowered_plan.function_id,
        lowered_plan.closure_layout.as_ref(),
        &lowered_plan.semantic,
        rewrite_stmt::decorator::collect_exprs(&func.decorator_list),
        &param_defaults,
        py_expr!("None"),
        lowered_plan.kind,
    );
    let binding_stmt = vec![py_stmt!(
        "{name:id} = {value:expr}",
        name = bind_name.as_str(),
        value = decorated
    )];
    callable_defs.push(lowered_plan);
    if bind_name.starts_with("_dp_class_ns_") || bind_name.starts_with("_dp_define_class_") {
        let mut replacement = function_hoisted;
        replacement.extend(binding_stmt);
        replacement
    } else {
        parent_hoisted.extend(function_hoisted);
        binding_stmt
    }
}

impl BlockPyModuleRewriter<'_> {
    fn root_module_init_stmt<'a>(module: &'a mut Suite) -> &'a mut ast::StmtFunctionDef {
        assert_eq!(
            module.len(),
            1,
            "expected root suite with exactly one statement",
        );
        let Stmt::FunctionDef(func) = &mut module[0] else {
            panic!("expected root suite with exactly one function");
        };
        assert!(
            func.parameters.posonlyargs.is_empty()
                && func.parameters.args.is_empty()
                && func.parameters.vararg.is_none()
                && func.parameters.kwonlyargs.is_empty()
                && func.parameters.kwarg.is_none(),
            "expected root function with no parameters",
        );
        func
    }

    fn walk_function_def_with_scope(
        &mut self,
        func: &mut ast::StmtFunctionDef,
    ) -> FunctionScopeFrame {
        let function_scope = self.semantic_state.function_scope(func);
        let parent_scope = self
            .function_scope_stack
            .last()
            .and_then(|frame| frame.scope.as_ref())
            .cloned();
        let callable_semantic = callable_semantic_info(
            self.semantic_state,
            parent_scope.as_ref(),
            function_scope.as_ref(),
            Some(func),
            &func.body,
        );
        self.function_scope_stack.push(FunctionScopeFrame {
            scope: function_scope.clone(),
            callable_semantic,
            hoisted_to_parent: Vec::new(),
        });
        self.visit_body(&mut func.body);
        self.function_scope_stack
            .pop()
            .expect("function scope stack should pop after walking function def")
    }

    fn lower_root_function_def(&mut self, func: &mut ast::StmtFunctionDef) {
        let state = self.walk_function_def_with_scope(func);
        assert!(
            state.hoisted_to_parent.is_empty(),
            "root _dp_module_init should not produce hoisted statements"
        );
        let name_gen = self.module_name_gen.next_function_name_gen();
        let lowered_plan = try_lower_function_to_blockpy_bundle(
            self.context,
            func,
            &state.callable_semantic,
            name_gen,
        );
        self.callable_defs.push(lowered_plan);
    }

    fn rewrite_visited_function_def(
        &mut self,
        func: &mut ast::StmtFunctionDef,
        state: FunctionScopeFrame,
    ) -> Vec<Stmt> {
        let parent_frame = self
            .function_scope_stack
            .last_mut()
            .expect("nested function rewrite should always have a parent hoist buffer");
        let parent_hoisted = &mut parent_frame.hoisted_to_parent;
        rewrite_function_def_stmt_via_blockpy(
            self.context,
            parent_hoisted,
            func,
            &state.callable_semantic,
            state.hoisted_to_parent,
            &mut self.module_name_gen,
            &mut self.callable_defs,
        )
    }
}

impl Transformer for BlockPyModuleRewriter<'_> {
    fn visit_body(&mut self, body: &mut Suite) {
        let mut rewritten = Vec::with_capacity(body.len());
        for stmt in std::mem::take(body) {
            let mut stmt = stmt;
            if let Stmt::FunctionDef(func) = &mut stmt {
                let state = self.walk_function_def_with_scope(func);
                let replacement = self.rewrite_visited_function_def(func, state);
                rewritten.extend(replacement);
                continue;
            }

            self.visit_stmt(&mut stmt);
            rewritten.push(stmt);
        }
        *body = rewritten;
    }

    fn visit_stmt(&mut self, stmt: &mut Stmt) {
        walk_stmt(self, stmt);
    }
}

#[cfg(test)]
mod test;
