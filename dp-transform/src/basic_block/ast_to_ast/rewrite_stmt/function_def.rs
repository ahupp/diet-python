use crate::basic_block::ast_to_ast::context::Context;
use crate::basic_block::ast_to_ast::scope::{analyze_module_scope, Scope};
use crate::basic_block::block_py::export::rewrite_function_def_stmt_via_blockpy;
use crate::basic_block::block_py::state::collect_cell_slots;
use crate::basic_block::blockpy_to_bb::LoweredBlockPyModuleBundle;
use crate::basic_block::function_identity::{
    is_module_init_temp_name, FunctionIdentity, FunctionIdentityByNode,
};
use crate::transformer::{walk_stmt, Transformer};
use ruff_python_ast::{self as ast, NodeIndex, Stmt, StmtBody};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

struct FunctionScopeFrame {
    name: String,
    parent_name: Option<String>,
    entering_module_init: bool,
    has_parent_hoisted_scope: bool,
    needs_cell_sync: bool,
    cell_bindings: HashSet<String>,
    hoisted_to_parent: Vec<Stmt>,
}

struct BlockPyModuleRewriter<'a> {
    context: &'a Context,
    module_scope: Arc<Scope>,
    function_identity_by_node: HashMap<NodeIndex, FunctionIdentity>,
    next_block_id: usize,
    reserved_temp_names_stack: Vec<HashSet<String>>,
    used_label_prefixes: HashMap<String, usize>,
    function_scope_stack: Vec<FunctionScopeFrame>,
    lowered_blockpy_module: LoweredBlockPyModuleBundle,
}

pub(crate) fn rewrite_ast_to_lowered_blockpy_module(
    context: &Context,
    module: &mut StmtBody,
    function_identity_by_node: FunctionIdentityByNode,
) -> LoweredBlockPyModuleBundle {
    let module_scope = analyze_module_scope(module);
    let function_identity_by_node = function_identity_by_node
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
        .collect();
    let mut rewriter = BlockPyModuleRewriter {
        context,
        module_scope,
        function_identity_by_node,
        next_block_id: 0,
        reserved_temp_names_stack: Vec::new(),
        used_label_prefixes: HashMap::new(),
        function_scope_stack: Vec::new(),
        lowered_blockpy_module: LoweredBlockPyModuleBundle {
            functions: Vec::new(),
            module_init: Some("_dp_module_init".to_string()),
        },
    };
    rewriter.visit_body(module);
    crate::basic_block::ast_to_ast::simplify::strip_generated_passes(context, module);
    rewriter.lowered_blockpy_module
}

impl BlockPyModuleRewriter<'_> {
    fn walk_function_def_with_scope(&mut self, stmt: &mut Stmt) -> Option<FunctionScopeFrame> {
        let Stmt::FunctionDef(func) = stmt else {
            return None;
        };
        let fn_name = func.name.id.to_string();
        let bind_name = func.name.id.to_string();
        let parent_name = self
            .function_scope_stack
            .last()
            .map(|frame| frame.name.clone());
        let entering_module_init = is_module_init_temp_name(fn_name.as_str());
        let has_parent_hoisted_scope = !self.function_scope_stack.is_empty();
        let cell_bindings = collect_cell_slots(&func.body.body)
            .into_iter()
            .filter_map(|slot| slot.strip_prefix("_dp_cell_").map(str::to_string))
            .collect::<HashSet<_>>();
        let needs_cell_sync = self
            .function_scope_stack
            .last()
            .map(|frame| frame.cell_bindings.contains(bind_name.as_str()))
            .unwrap_or(false);
        self.function_scope_stack.push(FunctionScopeFrame {
            name: fn_name,
            parent_name,
            entering_module_init,
            has_parent_hoisted_scope,
            needs_cell_sync,
            cell_bindings,
            hoisted_to_parent: Vec::new(),
        });
        walk_stmt(self, stmt);
        self.function_scope_stack.pop()
    }

    fn visit_function_def_stmt(&mut self, stmt: &mut Stmt) {
        let Some(state) = self.walk_function_def_with_scope(stmt) else {
            return;
        };
        if let Stmt::FunctionDef(func) = stmt {
            if let Some(replacement) = self.rewrite_visited_function_def(func, state) {
                *stmt = replacement;
            }
        }
    }

    fn rewrite_visited_function_def(
        &mut self,
        func: &mut ast::StmtFunctionDef,
        state: FunctionScopeFrame,
    ) -> Option<Stmt> {
        rewrite_function_def_stmt_via_blockpy(
            self.context,
            &self.module_scope,
            &mut self.lowered_blockpy_module,
            self.function_scope_stack
                .last_mut()
                .map(|frame| &mut frame.hoisted_to_parent),
            &self.function_identity_by_node,
            func,
            state.parent_name.as_deref(),
            state.needs_cell_sync,
            state.entering_module_init,
            state.has_parent_hoisted_scope,
            state.hoisted_to_parent,
            &mut self.reserved_temp_names_stack,
            &mut self.used_label_prefixes,
            &mut self.next_block_id,
        )
    }
}

impl Transformer for BlockPyModuleRewriter<'_> {
    fn visit_stmt(&mut self, stmt: &mut Stmt) {
        if matches!(stmt, Stmt::FunctionDef(_)) {
            self.visit_function_def_stmt(stmt);
            return;
        }

        walk_stmt(self, stmt);
    }
}
