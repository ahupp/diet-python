use super::operation::OperationDetail;
use super::{
    is_internal_symbol, walk_linear_block, walk_linear_expr, walk_linear_stmt, BlockPyFunction,
    BlockPyLinearModuleVisitor, BlockPyLinearPass, BlockPyNameLike, BlockPyPass, BlockPyStmt, Call,
    CoreBlockPyCallArg, CoreBlockPyExpr, CoreBlockPyExprWithAwaitAndYield,
    CoreBlockPyExprWithYield, CoreBlockPyLiteral, FunctionName, MapExpr, PassBlock, PassExpr,
    RuffExpr,
};
use crate::passes::ast_to_ast::scope_helpers::cell_name;
use ruff_python_ast::{self as ast, Expr};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum BindingTarget {
    Local,
    ModuleGlobal,
    ClassNamespace,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum BlockPyCellBindingKind {
    Owner,
    Capture,
}

#[derive(Debug, Clone, Copy, Default, Eq, PartialEq)]
pub enum BlockPyBindingKind {
    #[default]
    Local,
    Global,
    Cell(BlockPyCellBindingKind),
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct StorageLayout {
    pub freevars: Vec<ClosureSlot>,
    pub cellvars: Vec<ClosureSlot>,
    pub runtime_cells: Vec<ClosureSlot>,
    pub stack_slots: Vec<String>,
}

impl StorageLayout {
    pub fn freevar_slot(&self, slot: u32) -> Option<&ClosureSlot> {
        self.freevars.get(slot as usize)
    }

    pub fn local_cell_slot(&self, slot: u32) -> Option<&ClosureSlot> {
        self.cellvars
            .iter()
            .chain(self.runtime_cells.iter())
            .nth(slot as usize)
    }

    pub fn local_cell_storage_names(&self) -> Vec<String> {
        self.cellvars
            .iter()
            .chain(self.runtime_cells.iter())
            .map(|slot| slot.storage_name.clone())
            .collect()
    }

    pub fn has_freevar_storage_name(&self, storage_name: &str) -> bool {
        self.freevars
            .iter()
            .any(|slot| slot.storage_name == storage_name)
    }

    pub fn has_cellvar_storage_name(&self, storage_name: &str) -> bool {
        self.cellvars
            .iter()
            .any(|slot| slot.storage_name == storage_name)
    }

    pub fn has_storage_name(&self, storage_name: &str) -> bool {
        self.has_freevar_storage_name(storage_name)
            || self.has_cellvar_storage_name(storage_name)
            || self
                .runtime_cells
                .iter()
                .any(|slot| slot.storage_name == storage_name)
    }

    pub fn stack_slots(&self) -> &[String] {
        &self.stack_slots
    }

    pub fn set_stack_slots(&mut self, stack_slots: Vec<String>) {
        self.stack_slots = stack_slots;
    }

    pub fn ensure_stack_slot(&mut self, name: impl Into<String>) {
        let name = name.into();
        if self.stack_slots.iter().any(|existing| existing == &name) {
            return;
        }
        self.stack_slots.push(name);
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClosureSlot {
    pub logical_name: String,
    pub storage_name: String,
    pub init: ClosureInit,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClosureInit {
    InheritedCapture,
    Parameter,
    DeletedSentinel,
    RuntimePcUnstarted,
    RuntimeAbruptKindFallthrough,
    RuntimeNone,
    Deferred,
}

#[derive(Debug, Clone, Copy, Default, Eq, PartialEq)]
pub enum BlockPyCallableScopeKind {
    #[default]
    Function,
    Class,
    Module,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum BlockPyClassBodyFallback {
    Global,
    Cell,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum BlockPyEffectiveBinding {
    Local,
    Global,
    Cell(BlockPyCellBindingKind),
    ClassBody(BlockPyClassBodyFallback),
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum BlockPyBindingPurpose {
    Load,
    Store,
}

#[derive(Debug, Clone, Default)]
pub struct BlockPyCallableSemanticInfo {
    pub names: FunctionName,
    pub scope_kind: BlockPyCallableScopeKind,
    pub bindings: HashMap<String, BlockPyBindingKind>,
    pub local_defs: HashSet<String>,
    pub cell_storage_names: HashMap<String, String>,
    pub cell_capture_source_names: HashMap<String, String>,
    pub owned_cell_source_names: HashSet<String>,
    pub semantic_internal_names: HashSet<String>,
    pub type_param_names: HashSet<String>,
    pub effective_load_bindings: HashMap<String, BlockPyEffectiveBinding>,
    pub effective_store_bindings: HashMap<String, BlockPyEffectiveBinding>,
}

pub(crate) fn derive_effective_binding_for_name(
    name: &str,
    binding: BlockPyBindingKind,
    scope_kind: BlockPyCallableScopeKind,
    type_param_names: &HashSet<String>,
    purpose: BlockPyBindingPurpose,
    honor_internal_name: bool,
) -> BlockPyEffectiveBinding {
    if is_internal_symbol(name) && !honor_internal_name {
        return BlockPyEffectiveBinding::Local;
    }
    match purpose {
        BlockPyBindingPurpose::Load => match (scope_kind, binding) {
            (BlockPyCallableScopeKind::Class, BlockPyBindingKind::Cell(_)) => {
                BlockPyEffectiveBinding::ClassBody(BlockPyClassBodyFallback::Cell)
            }
            (BlockPyCallableScopeKind::Class, BlockPyBindingKind::Local)
            | (BlockPyCallableScopeKind::Class, BlockPyBindingKind::Global) => {
                BlockPyEffectiveBinding::ClassBody(BlockPyClassBodyFallback::Global)
            }
            (_, BlockPyBindingKind::Global) => BlockPyEffectiveBinding::Global,
            (_, BlockPyBindingKind::Cell(kind)) => BlockPyEffectiveBinding::Cell(kind),
            (_, BlockPyBindingKind::Local) => BlockPyEffectiveBinding::Local,
        },
        BlockPyBindingPurpose::Store => {
            if scope_kind == BlockPyCallableScopeKind::Class && type_param_names.contains(name) {
                return match binding {
                    BlockPyBindingKind::Local => BlockPyEffectiveBinding::Local,
                    BlockPyBindingKind::Global => BlockPyEffectiveBinding::Global,
                    BlockPyBindingKind::Cell(kind) => BlockPyEffectiveBinding::Cell(kind),
                };
            }
            match (scope_kind, binding) {
                (BlockPyCallableScopeKind::Class, BlockPyBindingKind::Local) => {
                    BlockPyEffectiveBinding::ClassBody(BlockPyClassBodyFallback::Global)
                }
                (_, BlockPyBindingKind::Global) => BlockPyEffectiveBinding::Global,
                (_, BlockPyBindingKind::Cell(kind)) => BlockPyEffectiveBinding::Cell(kind),
                (_, BlockPyBindingKind::Local) => BlockPyEffectiveBinding::Local,
            }
        }
    }
}

impl BlockPyCallableSemanticInfo {
    pub fn honors_internal_binding(&self, name: &str) -> bool {
        !is_internal_symbol(name) || self.semantic_internal_names.contains(name)
    }

    pub fn binding_kind(&self, name: &str) -> Option<BlockPyBindingKind> {
        self.bindings.get(name).copied()
    }

    pub fn has_local_def(&self, name: &str) -> bool {
        self.local_defs.contains(name)
    }

    pub fn effective_binding(
        &self,
        name: &str,
        purpose: BlockPyBindingPurpose,
    ) -> Option<BlockPyEffectiveBinding> {
        match purpose {
            BlockPyBindingPurpose::Load => self.effective_load_bindings.get(name).copied(),
            BlockPyBindingPurpose::Store => self.effective_store_bindings.get(name).copied(),
        }
    }

    pub fn insert_binding(
        &mut self,
        name: impl Into<String>,
        binding: BlockPyBindingKind,
        honor_internal_name: bool,
        cell_storage_name: Option<String>,
    ) {
        let name = name.into();
        self.bindings.insert(name.clone(), binding);
        if let Some(cell_storage_name) = cell_storage_name {
            self.cell_storage_names
                .insert(name.clone(), cell_storage_name.clone());
            self.cell_capture_source_names
                .insert(name.clone(), cell_storage_name);
        }
        if honor_internal_name {
            self.semantic_internal_names.insert(name.clone());
        }
        self.effective_load_bindings.insert(
            name.clone(),
            derive_effective_binding_for_name(
                name.as_str(),
                binding,
                self.scope_kind,
                &self.type_param_names,
                BlockPyBindingPurpose::Load,
                honor_internal_name,
            ),
        );
        self.effective_store_bindings.insert(
            name.clone(),
            derive_effective_binding_for_name(
                name.as_str(),
                binding,
                self.scope_kind,
                &self.type_param_names,
                BlockPyBindingPurpose::Store,
                honor_internal_name,
            ),
        );
    }

    pub fn resolved_load_binding_kind(&self, name: &str) -> BlockPyBindingKind {
        if let Some(binding) = self.binding_kind(name) {
            if self.honors_internal_binding(name) {
                return binding;
            }
        }
        if is_internal_symbol(name) {
            return BlockPyBindingKind::Local;
        }
        BlockPyBindingKind::Global
    }

    pub fn is_cell_binding(&self, name: &str) -> bool {
        matches!(self.binding_kind(name), Some(BlockPyBindingKind::Cell(_)))
    }

    pub fn cell_storage_name(&self, name: &str) -> String {
        self.cell_storage_names
            .get(name)
            .cloned()
            .unwrap_or_else(|| cell_name(name))
    }

    pub fn cell_capture_source_name(&self, name: &str) -> String {
        self.cell_capture_source_names
            .get(name)
            .cloned()
            .unwrap_or_else(|| cell_name(name))
    }

    pub fn cell_ref_source_name(&self, name: &str) -> String {
        if self.is_cell_binding(name) {
            self.cell_storage_name(name)
        } else {
            self.cell_capture_source_name(name)
        }
    }

    pub fn logical_name_for_cell_capture_source(&self, storage_name: &str) -> Option<String> {
        self.cell_capture_source_names
            .iter()
            .find_map(|(logical_name, current_storage_name)| {
                (current_storage_name == storage_name).then(|| logical_name.clone())
            })
            .or_else(|| self.logical_name_for_cell_storage(storage_name))
    }

    pub fn binding_target_for_name(
        &self,
        name: &str,
        purpose: BlockPyBindingPurpose,
    ) -> BindingTarget {
        if let Some(binding) = self.effective_binding(name, purpose) {
            if self.honors_internal_binding(name) {
                return match binding {
                    BlockPyEffectiveBinding::Global => BindingTarget::ModuleGlobal,
                    BlockPyEffectiveBinding::ClassBody(_) => BindingTarget::ClassNamespace,
                    BlockPyEffectiveBinding::Local | BlockPyEffectiveBinding::Cell(_) => {
                        BindingTarget::Local
                    }
                };
            }
        }
        if is_internal_symbol(name) {
            return BindingTarget::Local;
        }
        match self.effective_binding(name, purpose) {
            Some(BlockPyEffectiveBinding::Global) => BindingTarget::ModuleGlobal,
            Some(BlockPyEffectiveBinding::ClassBody(_)) => BindingTarget::ClassNamespace,
            _ => BindingTarget::Local,
        }
    }

    pub fn owned_cell_storage_names(&self) -> HashSet<String> {
        let mut names = self
            .bindings
            .iter()
            .filter_map(|(name, binding)| {
                matches!(
                    binding,
                    BlockPyBindingKind::Cell(BlockPyCellBindingKind::Owner)
                )
                .then(|| self.cell_storage_name(name.as_str()))
            })
            .collect::<HashSet<_>>();
        names.extend(self.owned_cell_source_names.iter().cloned());
        names
    }

    pub fn captured_cell_logical_names(&self) -> Vec<String> {
        let mut names = self
            .bindings
            .iter()
            .filter_map(|(name, binding)| {
                matches!(
                    binding,
                    BlockPyBindingKind::Cell(BlockPyCellBindingKind::Capture)
                )
                .then(|| name.clone())
            })
            .collect::<Vec<_>>();
        names.sort();
        names
    }

    pub fn local_cell_storage_names(&self) -> HashSet<String> {
        if !matches!(self.scope_kind, BlockPyCallableScopeKind::Function) {
            return HashSet::new();
        }
        self.owned_cell_storage_names()
    }

    pub fn logical_name_for_cell_storage(&self, storage_name: &str) -> Option<String> {
        if let Some(logical_name) = storage_name.strip_prefix("_dp_cell_") {
            return Some(logical_name.to_string());
        }
        self.cell_storage_names
            .iter()
            .find_map(|(logical_name, current_storage_name)| {
                (current_storage_name == storage_name).then(|| logical_name.clone())
            })
    }
}

pub(crate) trait BlockPySemanticExprNode {
    fn walk_child_exprs(&self, _f: &mut impl FnMut(&Self)) {}

    fn root_name_id(&self) -> Option<&str> {
        None
    }

    fn root_string_literal_value(&self) -> Option<String> {
        None
    }

    fn walk_root_loaded_names(&self, _f: &mut impl FnMut(&str)) {}

    fn walk_root_defined_names(&self, _f: &mut impl FnMut(&str)) {}

    fn walk_root_cell_ref_logical_names(&self, _f: &mut impl FnMut(&str)) {}
}

fn walk_assigned_name_targets_in_expr(target: &Expr, f: &mut impl FnMut(&str)) {
    match target {
        Expr::Name(name) => f(name.id.as_str()),
        Expr::Tuple(tuple) => {
            for elt in &tuple.elts {
                walk_assigned_name_targets_in_expr(elt, f);
            }
        }
        Expr::List(list) => {
            for elt in &list.elts {
                walk_assigned_name_targets_in_expr(elt, f);
            }
        }
        Expr::Starred(starred) => walk_assigned_name_targets_in_expr(starred.value.as_ref(), f),
        _ => {}
    }
}

fn walk_operation_loaded_names<E>(detail: &OperationDetail<E>, f: &mut impl FnMut(&str))
where
    E: BlockPySemanticExprNode,
{
    match detail {
        OperationDetail::LoadName(op) => f(op.name.as_str()),
        OperationDetail::Call(call) => {
            if let Some(name) = call.func.as_ref().root_name_id() {
                f(name);
            }
        }
        _ => {}
    }
}

fn operation_root_name_id<E>(detail: &OperationDetail<E>) -> Option<&str>
where
    E: BlockPySemanticExprNode,
{
    match detail {
        OperationDetail::Call(call) => call.func.as_ref().root_name_id(),
        OperationDetail::LoadRuntime(op) => Some(op.name.as_str()),
        OperationDetail::LoadName(op) => Some(op.name.as_str()),
        _ => None,
    }
}

fn walk_operation_cell_ref_logical_names<E>(detail: &OperationDetail<E>, f: &mut impl FnMut(&str)) {
    if let OperationDetail::CellRefForName(op) = detail {
        f(op.logical_name.as_str());
    }
}

fn call_root_cell_ref_logical_name<E>(call: &Call<E>) -> Option<String>
where
    E: BlockPySemanticExprNode,
{
    let helper_name = call.func.as_ref().root_name_id()?;
    if helper_name != "cell_ref" {
        return None;
    }
    let CoreBlockPyCallArg::Positional(arg) = call.args.first()? else {
        return None;
    };
    arg.root_string_literal_value()
}

impl BlockPySemanticExprNode for Expr {
    fn walk_child_exprs(&self, f: &mut impl FnMut(&Self)) {
        let _ = self.clone().map_expr(&mut |child| {
            f(&child);
            child
        });
    }

    fn root_name_id(&self) -> Option<&str> {
        match self {
            Expr::Name(name) => Some(name.id.as_str()),
            Expr::Attribute(ast::ExprAttribute { value, attr, .. }) if matches!(value.as_ref(), Expr::Name(name) if name.id.as_str() == "__soac__") => {
                Some(attr.id.as_str())
            }
            _ => None,
        }
    }

    fn root_string_literal_value(&self) -> Option<String> {
        match self {
            Expr::StringLiteral(literal) => Some(literal.value.to_str().to_string()),
            _ => None,
        }
    }

    fn walk_root_loaded_names(&self, f: &mut impl FnMut(&str)) {
        if let Expr::Name(name) = self {
            if matches!(name.ctx, ast::ExprContext::Load) {
                f(name.id.as_str());
            }
        }
    }

    fn walk_root_defined_names(&self, f: &mut impl FnMut(&str)) {
        if let Expr::Named(named) = self {
            walk_assigned_name_targets_in_expr(named.target.as_ref(), f);
        }
    }

    fn walk_root_cell_ref_logical_names(&self, f: &mut impl FnMut(&str)) {
        let Expr::Call(call) = self else {
            return;
        };
        let Some(Expr::Attribute(ast::ExprAttribute { value, attr, .. })) =
            Some(call.func.as_ref())
        else {
            return;
        };
        if !matches!(value.as_ref(), Expr::Name(name) if name.id.as_str() == "__soac__")
            || attr.id.as_str() != "cell_ref"
        {
            return;
        }
        let Some(ast::Expr::StringLiteral(literal)) = call.arguments.args.first() else {
            return;
        };
        f(literal.value.to_str().as_ref());
    }
}

impl BlockPySemanticExprNode for RuffExpr {
    fn walk_child_exprs(&self, f: &mut impl FnMut(&Self)) {
        let _ = self.clone().map_expr(&mut |child| {
            f(&child);
            child
        });
    }

    fn root_name_id(&self) -> Option<&str> {
        self.0.root_name_id()
    }

    fn root_string_literal_value(&self) -> Option<String> {
        self.0.root_string_literal_value()
    }

    fn walk_root_loaded_names(&self, f: &mut impl FnMut(&str)) {
        self.0.walk_root_loaded_names(f);
    }

    fn walk_root_defined_names(&self, f: &mut impl FnMut(&str)) {
        self.0.walk_root_defined_names(f);
    }

    fn walk_root_cell_ref_logical_names(&self, f: &mut impl FnMut(&str)) {
        self.0.walk_root_cell_ref_logical_names(f);
    }
}

impl BlockPySemanticExprNode for CoreBlockPyExprWithAwaitAndYield {
    fn walk_child_exprs(&self, f: &mut impl FnMut(&Self)) {
        let _ = self.clone().map_expr(&mut |child| {
            f(&child);
            child
        });
    }

    fn root_name_id(&self) -> Option<&str> {
        match self {
            Self::Name(name) => Some(name.id.as_str()),
            Self::Op(operation) => operation_root_name_id(operation),
            _ => None,
        }
    }

    fn root_string_literal_value(&self) -> Option<String> {
        match self {
            Self::Literal(CoreBlockPyLiteral::StringLiteral(literal)) => {
                Some(literal.value.clone())
            }
            _ => None,
        }
    }

    fn walk_root_loaded_names(&self, f: &mut impl FnMut(&str)) {
        match self {
            Self::Name(name) => f(name.id.as_str()),
            Self::Op(operation) => walk_operation_loaded_names(operation, f),
            _ => {}
        }
    }

    fn walk_root_cell_ref_logical_names(&self, f: &mut impl FnMut(&str)) {
        match self {
            Self::Op(operation) => {
                walk_operation_cell_ref_logical_names(operation, f);
                if let OperationDetail::Call(call) = operation {
                    if let Some(name) = call_root_cell_ref_logical_name(call) {
                        f(name.as_str());
                    }
                }
            }
            _ => {}
        }
    }
}

impl BlockPySemanticExprNode for CoreBlockPyExprWithYield {
    fn walk_child_exprs(&self, f: &mut impl FnMut(&Self)) {
        let _ = self.clone().map_expr(&mut |child| {
            f(&child);
            child
        });
    }

    fn root_name_id(&self) -> Option<&str> {
        match self {
            Self::Name(name) => Some(name.id.as_str()),
            Self::Op(operation) => operation_root_name_id(operation),
            _ => None,
        }
    }

    fn root_string_literal_value(&self) -> Option<String> {
        match self {
            Self::Literal(CoreBlockPyLiteral::StringLiteral(literal)) => {
                Some(literal.value.clone())
            }
            _ => None,
        }
    }

    fn walk_root_loaded_names(&self, f: &mut impl FnMut(&str)) {
        match self {
            Self::Name(name) => f(name.id.as_str()),
            Self::Op(operation) => walk_operation_loaded_names(operation, f),
            _ => {}
        }
    }

    fn walk_root_cell_ref_logical_names(&self, f: &mut impl FnMut(&str)) {
        match self {
            Self::Op(operation) => {
                walk_operation_cell_ref_logical_names(operation, f);
                if let OperationDetail::Call(call) = operation {
                    if let Some(name) = call_root_cell_ref_logical_name(call) {
                        f(name.as_str());
                    }
                }
            }
            _ => {}
        }
    }
}

impl<N> BlockPySemanticExprNode for CoreBlockPyExpr<N>
where
    N: BlockPyNameLike,
{
    fn walk_child_exprs(&self, f: &mut impl FnMut(&Self)) {
        let _ = self.clone().map_expr(&mut |child| {
            f(&child);
            child
        });
    }

    fn root_name_id(&self) -> Option<&str> {
        match self {
            Self::Name(name) => Some(name.id_str()),
            Self::Op(operation) => operation_root_name_id(operation),
            _ => None,
        }
    }

    fn root_string_literal_value(&self) -> Option<String> {
        match self {
            Self::Literal(CoreBlockPyLiteral::StringLiteral(literal)) => {
                Some(literal.value.clone())
            }
            _ => None,
        }
    }

    fn walk_root_loaded_names(&self, f: &mut impl FnMut(&str)) {
        match self {
            Self::Name(name) => f(name.id_str()),
            Self::Op(operation) => walk_operation_loaded_names(operation, f),
            _ => {}
        }
    }

    fn walk_root_cell_ref_logical_names(&self, f: &mut impl FnMut(&str)) {
        match self {
            Self::Op(operation) => {
                walk_operation_cell_ref_logical_names(operation, f);
                if let OperationDetail::Call(call) = operation {
                    if let Some(name) = call_root_cell_ref_logical_name(call) {
                        f(name.as_str());
                    }
                }
            }
            _ => {}
        }
    }
}

impl BlockPySemanticExprNode for super::CodegenBlockPyExpr {
    fn walk_child_exprs(&self, f: &mut impl FnMut(&Self)) {
        let _ = self.clone().map_expr(&mut |child| {
            f(&child);
            child
        });
    }

    fn root_name_id(&self) -> Option<&str> {
        match self {
            Self::Name(name) => Some(name.id_str()),
            Self::Op(operation) => operation_root_name_id(operation),
            _ => None,
        }
    }

    fn walk_root_loaded_names(&self, f: &mut impl FnMut(&str)) {
        match self {
            Self::Name(name) => f(name.id_str()),
            Self::Op(operation) => walk_operation_loaded_names(operation, f),
            _ => {}
        }
    }

    fn walk_root_cell_ref_logical_names(&self, f: &mut impl FnMut(&str)) {
        match self {
            Self::Op(operation) => {
                walk_operation_cell_ref_logical_names(operation, f);
                let OperationDetail::Call(call) = operation else {
                    return;
                };
                if let Some(name) = call_root_cell_ref_logical_name(call) {
                    f(name.as_str());
                }
            }
            _ => {}
        }
    }
}

#[derive(Default)]
struct StorageLayoutSemanticCollector {
    used_names: HashSet<String>,
    defined_names: HashSet<String>,
    deleted_names: HashSet<String>,
    cell_ref_logical_names: HashSet<String>,
}

impl<P> BlockPyLinearModuleVisitor<P> for StorageLayoutSemanticCollector
where
    P: BlockPyLinearPass,
    PassExpr<P>: BlockPySemanticExprNode,
{
    fn visit_block(&mut self, block: &PassBlock<P>) {
        if let Some(exc_param) = block.exception_param() {
            self.used_names.insert(exc_param.to_string());
        }
        walk_linear_block::<Self, P>(self, block);
    }

    fn visit_stmt(&mut self, stmt: &crate::block_py::PassStmt<P>) {
        match stmt {
            BlockPyStmt::Assign(assign) => {
                self.defined_names
                    .insert(assign.target.id_str().to_string());
            }
            BlockPyStmt::Delete(delete) => {
                let name = delete.target.id_str().to_string();
                self.used_names.insert(name.clone());
                self.deleted_names.insert(name);
            }
            BlockPyStmt::Expr(_) => {}
        }
        walk_linear_stmt::<Self, P>(self, stmt);
    }

    fn visit_expr(&mut self, expr: &PassExpr<P>) {
        expr.walk_root_loaded_names(&mut |name| {
            self.used_names.insert(name.to_string());
        });
        expr.walk_root_defined_names(&mut |name| {
            self.defined_names.insert(name.to_string());
        });
        expr.walk_root_cell_ref_logical_names(&mut |name| {
            self.cell_ref_logical_names.insert(name.to_string());
        });
        walk_linear_expr::<Self, P>(self, expr);
    }
}

fn is_runtime_closure_name(name: &str) -> bool {
    matches!(name, "_dp_pc" | "_dp_yieldfrom") || name.starts_with("_dp_try_abrupt_kind_")
}

pub(crate) fn compute_storage_layout_from_semantics<P>(
    callable_def: &BlockPyFunction<P>,
) -> Option<StorageLayout>
where
    P: BlockPyLinearPass,
    PassExpr<P>: BlockPySemanticExprNode,
{
    let normalize_capture_name = |name: &str| {
        callable_def
            .semantic
            .logical_name_for_cell_capture_source(name)
            .or_else(|| callable_def.semantic.logical_name_for_cell_storage(name))
            .unwrap_or_else(|| name.to_string())
    };

    let param_names = callable_def.params.names();
    let owned_cell_slot_names = callable_def.semantic.owned_cell_storage_names();
    let mut local_cell_slots = owned_cell_slot_names.iter().cloned().collect::<Vec<_>>();
    local_cell_slots.sort();
    let param_name_set = param_names.iter().cloned().collect::<HashSet<_>>();

    let mut collector = StorageLayoutSemanticCollector::default();
    collector.visit_fn(callable_def);

    let capture_candidate_names = collector
        .used_names
        .iter()
        .chain(collector.defined_names.iter())
        .chain(collector.deleted_names.iter())
        .chain(collector.cell_ref_logical_names.iter())
        .map(|name| normalize_capture_name(name.as_str()))
        .collect::<HashSet<_>>();

    let mut capture_names = callable_def
        .semantic
        .captured_cell_logical_names()
        .into_iter()
        .map(|name| normalize_capture_name(name.as_str()))
        .filter(|name| !is_runtime_closure_name(name.as_str()))
        .filter(|name| capture_candidate_names.contains(name))
        .collect::<Vec<_>>();
    capture_names.extend(
        collector
            .cell_ref_logical_names
            .iter()
            .map(|name| normalize_capture_name(name.as_str()))
            .filter(|logical_name| !is_runtime_closure_name(logical_name.as_str()))
            .filter(|logical_name| !param_name_set.contains(logical_name.as_str()))
            .filter(|logical_name| {
                !owned_cell_slot_names.contains(
                    callable_def
                        .semantic
                        .cell_capture_source_name(logical_name.as_str())
                        .as_str(),
                )
            }),
    );
    capture_names.sort();
    capture_names.dedup();

    build_storage_layout_from_capture_names(
        callable_def,
        capture_names,
        &param_name_set,
        &local_cell_slots,
    )
}

pub(crate) fn build_storage_layout_from_capture_names<P>(
    callable_def: &BlockPyFunction<P>,
    mut capture_names: Vec<String>,
    param_name_set: &HashSet<String>,
    local_cell_slots: &[String],
) -> Option<StorageLayout>
where
    P: BlockPyPass,
{
    capture_names.sort();
    capture_names.dedup();
    let local_cell_slots = local_cell_slots
        .iter()
        .filter(|storage_name| {
            let logical_name = callable_def
                .semantic
                .logical_name_for_cell_storage(storage_name.as_str())
                .unwrap_or_else(|| (*storage_name).clone());
            !is_runtime_closure_name(logical_name.as_str())
        })
        .cloned()
        .collect::<Vec<_>>();

    if capture_names.is_empty() && local_cell_slots.is_empty() {
        return None;
    }

    let freevars = capture_names
        .iter()
        .map(|logical_name| ClosureSlot {
            logical_name: logical_name.clone(),
            storage_name: callable_def
                .semantic
                .cell_storage_name(logical_name.as_str()),
            init: ClosureInit::InheritedCapture,
        })
        .collect::<Vec<_>>();
    let cellvars = local_cell_slots
        .into_iter()
        .map(|storage_name| {
            let logical_name = callable_def
                .semantic
                .logical_name_for_cell_storage(storage_name.as_str())
                .unwrap_or_else(|| storage_name.clone());
            let init = if param_name_set.contains(logical_name.as_str()) {
                ClosureInit::Parameter
            } else {
                ClosureInit::DeletedSentinel
            };
            ClosureSlot {
                logical_name,
                storage_name,
                init,
            }
        })
        .collect::<Vec<_>>();

    Some(StorageLayout {
        freevars,
        cellvars,
        runtime_cells: Vec::new(),
        stack_slots: Vec::new(),
    })
}
