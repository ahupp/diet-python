use self::param_specs::ParamSpec;
use crate::passes::ast_to_ast::scope_helpers::cell_name;
use crate::passes::{BbBlockPyPass, PreparedBbBlockPyPass};
use crate::py_expr;
pub use ruff_python_ast::Expr;
use ruff_python_ast::{self as ast, ExprName};
use std::collections::{HashMap, HashSet};
use std::fmt;

pub(crate) mod cfg;
mod convert;
pub(crate) mod dataflow;
pub(crate) mod exception;
pub mod intrinsics;
mod name_gen;
pub(crate) mod param_specs;
pub mod pretty;
pub(crate) mod state;
pub use convert::{BlockPyModuleMap, BlockPyModuleTryMap};
pub use name_gen::{FunctionNameGen, ModuleNameGen};

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct BlockPyLabel(pub String);

impl From<String> for BlockPyLabel {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl From<&str> for BlockPyLabel {
    fn from(value: &str) -> Self {
        Self(value.to_string())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct FunctionId(pub usize);

impl FunctionId {
    pub fn plan_qualname(self, qualname: &str) -> String {
        format!("{qualname}::__dp_fn_{}", self.0)
    }
}

fn is_internal_symbol(name: &str) -> bool {
    name.starts_with("_dp_") || name.starts_with("__dp_") || name == "__dp__"
}

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClosureLayout {
    pub freevars: Vec<ClosureSlot>,
    pub cellvars: Vec<ClosureSlot>,
    pub runtime_cells: Vec<ClosureSlot>,
}

impl ClosureLayout {
    pub fn ambient_storage_names(&self) -> Vec<String> {
        self.freevars
            .iter()
            .chain(self.cellvars.iter())
            .chain(self.runtime_cells.iter())
            .filter(|slot| matches!(slot.init, ClosureInit::InheritedCapture))
            .map(|slot| slot.storage_name.clone())
            .collect()
    }

    pub fn local_cell_storage_names(&self) -> Vec<String> {
        self.cellvars
            .iter()
            .chain(self.runtime_cells.iter())
            .map(|slot| slot.storage_name.clone())
            .collect()
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
    RuntimeNone,
    Deferred,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum BlockPyFunctionKind {
    Function,
    Coroutine,
    Generator,
    AsyncGenerator,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum ResumeAbiParam {
    SelfValue,
    SendValue,
    ResumeExc,
    TransportSent,
}

impl ResumeAbiParam {
    pub(crate) fn name(self) -> &'static str {
        match self {
            ResumeAbiParam::SelfValue => "_dp_self",
            ResumeAbiParam::SendValue => "_dp_send_value",
            ResumeAbiParam::ResumeExc => "_dp_resume_exc",
            ResumeAbiParam::TransportSent => "_dp_transport_sent",
        }
    }

    pub(crate) fn from_name(name: &str) -> Option<Self> {
        match name {
            "_dp_self" => Some(ResumeAbiParam::SelfValue),
            "_dp_send_value" => Some(ResumeAbiParam::SendValue),
            "_dp_resume_exc" => Some(ResumeAbiParam::ResumeExc),
            "_dp_transport_sent" => Some(ResumeAbiParam::TransportSent),
            _ => None,
        }
    }
}

const GENERATOR_RESUME_ABI_PARAMS: [ResumeAbiParam; 3] = [
    ResumeAbiParam::SelfValue,
    ResumeAbiParam::SendValue,
    ResumeAbiParam::ResumeExc,
];

const ASYNC_GENERATOR_RESUME_ABI_PARAMS: [ResumeAbiParam; 4] = [
    ResumeAbiParam::SelfValue,
    ResumeAbiParam::SendValue,
    ResumeAbiParam::ResumeExc,
    ResumeAbiParam::TransportSent,
];

pub(crate) fn resume_abi_params(kind: BlockPyFunctionKind) -> &'static [ResumeAbiParam] {
    match kind {
        BlockPyFunctionKind::Function => &[],
        BlockPyFunctionKind::Coroutine | BlockPyFunctionKind::Generator => {
            &GENERATOR_RESUME_ABI_PARAMS
        }
        BlockPyFunctionKind::AsyncGenerator => &ASYNC_GENERATOR_RESUME_ABI_PARAMS,
    }
}

pub(crate) fn is_resume_abi_param_name(name: &str) -> bool {
    ResumeAbiParam::from_name(name).is_some()
}

#[derive(Debug, Clone)]
pub struct CfgBlock<S, T> {
    pub label: BlockPyLabel,
    pub body: Vec<S>,
    pub term: T,
    pub params: Vec<BlockParam>,
    pub exc_edge: Option<BlockPyEdge>,
}

impl<S, T> CfgBlock<S, T> {
    pub fn label_str(&self) -> &str {
        self.label.as_str()
    }

    pub fn ensure_param(&mut self, name: impl Into<String>, role: BlockParamRole) {
        let name = name.into();
        if self.params.iter().any(|param| param.name == name) {
            return;
        }
        self.params.push(BlockParam { name, role });
    }

    pub fn set_exception_param(&mut self, name: impl Into<String>) {
        let name = name.into();
        for param in &mut self.params {
            if param.role == BlockParamRole::Exception && param.name != name {
                param.role = BlockParamRole::Local;
            }
        }
        if let Some(param) = self.params.iter_mut().find(|param| param.name == name) {
            param.role = BlockParamRole::Exception;
            return;
        }
        self.params.push(BlockParam {
            name,
            role: BlockParamRole::Exception,
        });
    }

    pub fn exception_param(&self) -> Option<&str> {
        self.params
            .iter()
            .find(|param| param.role == BlockParamRole::Exception)
            .map(|param| param.name.as_str())
    }

    pub fn param_names(&self) -> impl Iterator<Item = &str> {
        self.params.iter().map(|param| param.name.as_str())
    }

    pub fn param_name_vec(&self) -> Vec<String> {
        self.param_names().map(ToString::to_string).collect()
    }

    pub fn bb_params(&self) -> impl Iterator<Item = &BlockParam> {
        [
            BlockParamRole::Exception,
            BlockParamRole::Local,
            BlockParamRole::AbruptKind,
            BlockParamRole::AbruptPayload,
        ]
        .into_iter()
        .flat_map(|role| self.params.iter().filter(move |param| param.role == role))
    }

    pub fn bb_param_names(&self) -> impl Iterator<Item = &str> {
        self.bb_params().map(|param| param.name.as_str())
    }
}

pub(crate) fn move_entry_block_to_front<S, T>(blocks: &mut Vec<CfgBlock<S, T>>, entry_label: &str) {
    if let Some(entry_index) = blocks
        .iter()
        .position(|block| block.label.as_str() == entry_label)
    {
        if entry_index != 0 {
            let entry_block = blocks.remove(entry_index);
            blocks.insert(0, entry_block);
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct BlockPyModule<P: BlockPyPass> {
    pub callable_defs: Vec<BlockPyFunction<P>>,
}

impl<P: BlockPyPass> BlockPyModule<P> {
    pub fn map_callable_defs<Q: BlockPyPass>(
        self,
        mut f: impl FnMut(BlockPyFunction<P>) -> BlockPyFunction<Q>,
    ) -> BlockPyModule<Q> {
        BlockPyModule {
            callable_defs: self.callable_defs.into_iter().map(&mut f).collect(),
        }
    }
}

#[derive(Debug, Clone)]
pub enum CoreBlockPyExprWithAwaitAndYield {
    Name(ast::ExprName),
    Literal(CoreBlockPyLiteral),
    Call(CoreBlockPyCall<CoreBlockPyExprWithAwaitAndYield>),
    Intrinsic(IntrinsicCall<CoreBlockPyExprWithAwaitAndYield>),
    Await(CoreBlockPyAwait<CoreBlockPyExprWithAwaitAndYield>),
    Yield(CoreBlockPyYield<CoreBlockPyExprWithAwaitAndYield>),
    YieldFrom(CoreBlockPyYieldFrom<CoreBlockPyExprWithAwaitAndYield>),
}

#[derive(Debug, Clone)]
pub enum CoreBlockPyExprWithYield {
    Name(ast::ExprName),
    Literal(CoreBlockPyLiteral),
    Call(CoreBlockPyCall<CoreBlockPyExprWithYield>),
    Intrinsic(IntrinsicCall<CoreBlockPyExprWithYield>),
    Yield(CoreBlockPyYield<CoreBlockPyExprWithYield>),
    YieldFrom(CoreBlockPyYieldFrom<CoreBlockPyExprWithYield>),
}

#[derive(Debug, Clone)]
pub enum CoreBlockPyExpr {
    Name(ast::ExprName),
    Literal(CoreBlockPyLiteral),
    Call(CoreBlockPyCall<CoreBlockPyExpr>),
    Intrinsic(IntrinsicCall<CoreBlockPyExpr>),
}

#[derive(Debug, Clone)]
pub enum CoreBlockPyLiteral {
    StringLiteral(CoreStringLiteral),
    BytesLiteral(CoreBytesLiteral),
    NumberLiteral(CoreNumberLiteral),
}

#[derive(Debug, Clone)]
pub struct CoreStringLiteral {
    pub node_index: ast::AtomicNodeIndex,
    pub range: ruff_text_size::TextRange,
    pub value: String,
}

#[derive(Debug, Clone)]
pub struct CoreBytesLiteral {
    pub node_index: ast::AtomicNodeIndex,
    pub range: ruff_text_size::TextRange,
    pub value: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct CoreNumberLiteral {
    pub node_index: ast::AtomicNodeIndex,
    pub range: ruff_text_size::TextRange,
    pub value: CoreNumberLiteralValue,
}

#[derive(Debug, Clone)]
pub enum CoreNumberLiteralValue {
    Int(ast::Int),
    Float(f64),
}

#[derive(Debug, Clone)]
pub struct CoreBlockPyCall<E> {
    pub node_index: ast::AtomicNodeIndex,
    pub range: ruff_text_size::TextRange,
    pub func: Box<E>,
    pub args: Vec<CoreBlockPyCallArg<E>>,
    pub keywords: Vec<CoreBlockPyKeywordArg<E>>,
}

#[derive(Debug, Clone)]
pub struct IntrinsicCall<E> {
    pub intrinsic: &'static dyn intrinsics::Intrinsic,
    pub node_index: ast::AtomicNodeIndex,
    pub range: ruff_text_size::TextRange,
    pub args: Vec<CoreBlockPyCallArg<E>>,
    pub keywords: Vec<CoreBlockPyKeywordArg<E>>,
}

pub(crate) trait CoreCallLikeExpr: Sized {
    fn from_name(name: ast::ExprName) -> Self;

    fn from_call(call: CoreBlockPyCall<Self>) -> Self;

    fn from_intrinsic(call: IntrinsicCall<Self>) -> Self;
}

impl CoreCallLikeExpr for CoreBlockPyExprWithAwaitAndYield {
    fn from_name(name: ast::ExprName) -> Self {
        Self::Name(name)
    }

    fn from_call(call: CoreBlockPyCall<Self>) -> Self {
        Self::Call(call)
    }

    fn from_intrinsic(call: IntrinsicCall<Self>) -> Self {
        Self::Intrinsic(call)
    }
}

impl CoreCallLikeExpr for CoreBlockPyExprWithYield {
    fn from_name(name: ast::ExprName) -> Self {
        Self::Name(name)
    }

    fn from_call(call: CoreBlockPyCall<Self>) -> Self {
        Self::Call(call)
    }

    fn from_intrinsic(call: IntrinsicCall<Self>) -> Self {
        Self::Intrinsic(call)
    }
}

impl CoreCallLikeExpr for CoreBlockPyExpr {
    fn from_name(name: ast::ExprName) -> Self {
        Self::Name(name)
    }

    fn from_call(call: CoreBlockPyCall<Self>) -> Self {
        Self::Call(call)
    }

    fn from_intrinsic(call: IntrinsicCall<Self>) -> Self {
        Self::Intrinsic(call)
    }
}

pub(crate) fn core_call_expr_with_meta<E: CoreCallLikeExpr>(
    func: E,
    node_index: ast::AtomicNodeIndex,
    range: ruff_text_size::TextRange,
    args: Vec<CoreBlockPyCallArg<E>>,
    keywords: Vec<CoreBlockPyKeywordArg<E>>,
) -> E {
    E::from_call(CoreBlockPyCall {
        node_index,
        range,
        func: Box::new(func),
        args,
        keywords,
    })
}

pub(crate) fn core_named_call_expr_with_meta<E: CoreCallLikeExpr>(
    func_name: &str,
    node_index: ast::AtomicNodeIndex,
    range: ruff_text_size::TextRange,
    args: Vec<CoreBlockPyCallArg<E>>,
    keywords: Vec<CoreBlockPyKeywordArg<E>>,
) -> E {
    core_call_expr_with_meta(
        E::from_name(ExprName {
            id: func_name.into(),
            ctx: ast::ExprContext::Load,
            range,
            node_index: node_index.clone(),
        }),
        node_index,
        range,
        args,
        keywords,
    )
}

pub(crate) fn core_intrinsic_expr_with_meta<E: CoreCallLikeExpr>(
    intrinsic: &'static dyn intrinsics::Intrinsic,
    node_index: ast::AtomicNodeIndex,
    range: ruff_text_size::TextRange,
    args: Vec<CoreBlockPyCallArg<E>>,
    keywords: Vec<CoreBlockPyKeywordArg<E>>,
) -> E {
    E::from_intrinsic(IntrinsicCall {
        intrinsic,
        node_index,
        range,
        args,
        keywords,
    })
}

pub(crate) fn core_positional_intrinsic_expr_with_meta<E: CoreCallLikeExpr>(
    intrinsic: &'static dyn intrinsics::Intrinsic,
    node_index: ast::AtomicNodeIndex,
    range: ruff_text_size::TextRange,
    args: Vec<E>,
) -> E {
    core_intrinsic_expr_with_meta(
        intrinsic,
        node_index,
        range,
        args.into_iter()
            .map(CoreBlockPyCallArg::Positional)
            .collect(),
        Vec::new(),
    )
}

pub(crate) fn core_positional_call_expr_with_meta<E: CoreCallLikeExpr>(
    func_name: &str,
    node_index: ast::AtomicNodeIndex,
    range: ruff_text_size::TextRange,
    args: Vec<E>,
) -> E {
    core_named_call_expr_with_meta(
        func_name,
        node_index,
        range,
        args.into_iter()
            .map(CoreBlockPyCallArg::Positional)
            .collect(),
        Vec::new(),
    )
}

#[derive(Debug, Clone)]
pub enum CoreBlockPyCallArg<E = CoreBlockPyExprWithAwaitAndYield> {
    Positional(E),
    Starred(E),
}

#[derive(Debug, Clone)]
pub enum CoreBlockPyKeywordArg<E = CoreBlockPyExprWithAwaitAndYield> {
    Named { arg: ast::Identifier, value: E },
    Starred(E),
}

#[derive(Debug, Clone)]
pub struct CoreBlockPyAwait<E> {
    pub node_index: ast::AtomicNodeIndex,
    pub range: ruff_text_size::TextRange,
    pub value: Box<E>,
}

#[derive(Debug, Clone)]
pub struct CoreBlockPyYield<E> {
    pub node_index: ast::AtomicNodeIndex,
    pub range: ruff_text_size::TextRange,
    pub value: Option<Box<E>>,
}

#[derive(Debug, Clone)]
pub struct CoreBlockPyYieldFrom<E> {
    pub node_index: ast::AtomicNodeIndex,
    pub range: ruff_text_size::TextRange,
    pub value: Box<E>,
}

#[derive(Debug, Clone)]
pub struct BlockPyCallableFacts {
    pub deleted_names: HashSet<String>,
    pub unbound_local_names: HashSet<String>,
}

impl Default for BlockPyCallableFacts {
    fn default() -> Self {
        Self {
            deleted_names: HashSet::new(),
            unbound_local_names: HashSet::new(),
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct FunctionName {
    pub bind_name: String,
    pub fn_name: String,
    pub display_name: String,
    pub qualname: String,
}

impl FunctionName {
    pub fn new(
        bind_name: impl Into<String>,
        fn_name: impl Into<String>,
        display_name: impl Into<String>,
        qualname: impl Into<String>,
    ) -> Self {
        Self {
            bind_name: bind_name.into(),
            fn_name: fn_name.into(),
            display_name: display_name.into(),
            qualname: qualname.into(),
        }
    }
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
    pub cell_storage_names: HashMap<String, String>,
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

    pub fn local_cell_storage_names(&self) -> HashSet<String> {
        if !matches!(self.scope_kind, BlockPyCallableScopeKind::Function) {
            return HashSet::new();
        }
        self.bindings
            .iter()
            .filter_map(|(name, binding)| {
                matches!(
                    binding,
                    BlockPyBindingKind::Cell(BlockPyCellBindingKind::Owner)
                )
                .then(|| cell_name(name.as_str()))
            })
            .collect()
    }
}

#[derive(Debug)]
pub struct BlockPyFunction<P: BlockPyPass> {
    pub function_id: FunctionId,
    pub name_gen: FunctionNameGen,
    pub names: FunctionName,
    pub kind: BlockPyFunctionKind,
    pub params: ParamSpec,
    pub blocks: Vec<CfgBlock<P::Stmt, BlockPyTerm<P::Expr>>>,
    pub doc: Option<String>,
    pub closure_layout: Option<ClosureLayout>,
    pub facts: BlockPyCallableFacts,
    pub semantic: BlockPyCallableSemanticInfo,
}

impl<P: BlockPyPass> Clone for BlockPyFunction<P> {
    fn clone(&self) -> Self {
        Self {
            function_id: self.function_id,
            // Share the allocator state so cloned analysis/rendering snapshots
            // cannot accidentally reissue duplicate generated names.
            name_gen: self.name_gen.share(),
            names: self.names.clone(),
            kind: self.kind,
            params: self.params.clone(),
            blocks: self.blocks.clone(),
            doc: self.doc.clone(),
            closure_layout: self.closure_layout.clone(),
            facts: self.facts.clone(),
            semantic: self.semantic.clone(),
        }
    }
}

impl<P: BlockPyPass> BlockPyFunction<P> {
    pub fn lowered_kind(&self) -> &BlockPyFunctionKind {
        &self.kind
    }

    pub fn closure_layout(&self) -> &Option<ClosureLayout> {
        &self.closure_layout
    }

    pub fn local_cell_slots(&self) -> Vec<String> {
        self.closure_layout
            .as_ref()
            .map(ClosureLayout::local_cell_storage_names)
            .unwrap_or_default()
    }

    pub fn entry_block(&self) -> &PassBlock<P> {
        self.blocks
            .first()
            .expect("BlockPyFunction should have at least one block")
    }

    pub fn map_blocks<Q: BlockPyPass>(
        self,
        mut f: impl FnMut(PassBlock<P>) -> PassBlock<Q>,
    ) -> BlockPyFunction<Q>
    where
        Q: BlockPyPass<Expr = P::Expr>,
    {
        BlockPyFunction {
            function_id: self.function_id,
            name_gen: self.name_gen,
            names: self.names,
            kind: self.kind,
            params: self.params,
            blocks: self.blocks.into_iter().map(&mut f).collect(),
            doc: self.doc,
            closure_layout: self.closure_layout,
            facts: self.facts,
            semantic: self.semantic,
        }
    }
}

pub trait BlockPyNormalizedStmt {
    fn assert_blockpy_normalized(&self);
}

pub trait IntoBlockPyStmt<E>: Clone + fmt::Debug {
    fn into_stmt(self) -> BlockPyStmt<E>;
}

pub trait BlockPyPass: Clone + fmt::Debug {
    type Expr: Clone + fmt::Debug + Into<Expr>;
    type Stmt: BlockPyNormalizedStmt + IntoBlockPyStmt<Self::Expr>;
}

pub type PassExpr<P> = <P as BlockPyPass>::Expr;
pub type PassBlock<P> = CfgBlock<<P as BlockPyPass>::Stmt, BlockPyTerm<PassExpr<P>>>;
pub type BbBlock = PassBlock<BbBlockPyPass>;
pub type PreparedBbBlock = PassBlock<PreparedBbBlockPyPass>;

pub type BlockPyCfgBlock<S, T> = CfgBlock<S, T>;
pub type BlockPyBlock<E = Expr> = BlockPyCfgBlock<BlockPyStmt<E>, BlockPyTerm<E>>;
pub type BlockPyStructuredIf<E = Expr> = BlockPyIf<E, BlockPyStmt<E>, BlockPyTerm<E>>;

pub trait BlockPyJumpTerm<L> {
    fn jump_term(target: L) -> Self;
}

pub trait BlockPyFallthroughTerm<L>: BlockPyJumpTerm<L> {
    fn implicit_function_return() -> Self;
}

pub(crate) trait ImplicitNoneExpr {
    fn implicit_none_expr() -> Self;
    fn is_implicit_none_expr(expr: &Self) -> bool;
}

fn implicit_none_name() -> ast::ExprName {
    let Expr::Name(name) = py_expr!("__dp_NONE") else {
        unreachable!();
    };
    name
}

pub fn assert_blockpy_block_normalized<S: BlockPyNormalizedStmt, T>(block: &CfgBlock<S, T>) {
    for stmt in &block.body {
        stmt.assert_blockpy_normalized();
    }
}

#[derive(Debug, Clone)]
pub struct BlockPyCfgFragment<S, T> {
    pub body: Vec<S>,
    pub term: Option<T>,
}

pub type BlockPyStmtFragment<E = Expr> = BlockPyCfgFragment<BlockPyStmt<E>, BlockPyTerm<E>>;

impl<S: BlockPyNormalizedStmt, T> BlockPyCfgFragment<S, T> {
    pub fn assert_normalized(&self) {
        for stmt in &self.body {
            stmt.assert_blockpy_normalized();
        }
    }
}

impl<S: BlockPyNormalizedStmt, T> BlockPyCfgFragment<S, T> {
    pub fn from_stmts(stmts: Vec<S>) -> Self {
        Self::with_term(stmts, None)
    }

    pub fn with_term(body: Vec<S>, term: impl Into<Option<T>>) -> Self {
        let fragment = BlockPyCfgFragment {
            body,
            term: term.into(),
        };
        fragment.assert_normalized();
        fragment
    }
}

impl<S: BlockPyNormalizedStmt, T: BlockPyJumpTerm<BlockPyLabel>> BlockPyCfgFragment<S, T> {
    pub fn jump(target: BlockPyLabel) -> Self {
        Self::with_term(Vec::new(), Some(T::jump_term(target)))
    }
}

#[derive(Debug, Clone)]
pub struct BlockPyCfgFragmentBuilder<S, T> {
    body: Vec<S>,
    term: Option<T>,
}

pub type BlockPyStmtFragmentBuilder<E = Expr> =
    BlockPyCfgFragmentBuilder<BlockPyStmt<E>, BlockPyTerm<E>>;

impl<S: BlockPyNormalizedStmt, T> BlockPyCfgFragmentBuilder<S, T> {
    pub fn new() -> Self {
        Self {
            body: Vec::new(),
            term: None,
        }
    }

    pub fn push_stmt(&mut self, stmt: S) {
        assert!(
            self.term.is_none(),
            "cannot append BlockPyStmt after stmt-fragment terminator"
        );
        stmt.assert_blockpy_normalized();
        self.body.push(stmt);
    }

    pub fn extend<I>(&mut self, stmts: I)
    where
        I: IntoIterator<Item = S>,
    {
        for stmt in stmts {
            self.push_stmt(stmt);
        }
    }

    pub fn set_term(&mut self, term: T) {
        assert!(
            self.term.is_none(),
            "cannot replace existing stmt-fragment terminator"
        );
        self.term = Some(term);
    }

    pub fn finish(self) -> BlockPyCfgFragment<S, T> {
        let fragment = BlockPyCfgFragment {
            body: self.body,
            term: self.term,
        };
        fragment.assert_normalized();
        fragment
    }
}

#[derive(Debug, Clone)]
pub struct BlockPyCfgBlockBuilder<S, T> {
    label: BlockPyLabel,
    params: Vec<BlockParam>,
    exc_edge: Option<BlockPyEdge>,
    fragment: BlockPyCfgFragmentBuilder<S, T>,
}

pub type BlockPyBlockBuilder<E = Expr> = BlockPyCfgBlockBuilder<BlockPyStmt<E>, BlockPyTerm<E>>;

impl<S: BlockPyNormalizedStmt, T: BlockPyFallthroughTerm<BlockPyLabel>>
    BlockPyCfgBlockBuilder<S, T>
{
    pub fn new(label: BlockPyLabel) -> Self {
        Self {
            label,
            params: Vec::new(),
            exc_edge: None,
            fragment: BlockPyCfgFragmentBuilder::new(),
        }
    }

    pub fn with_exc_param(mut self, exc_param: Option<String>) -> Self {
        if let Some(exc_param) = exc_param {
            if self.params.iter().any(|param| param.name == exc_param) {
                for param in &mut self.params {
                    if param.role == BlockParamRole::Exception && param.name != exc_param {
                        param.role = BlockParamRole::Local;
                    }
                    if param.name == exc_param {
                        param.role = BlockParamRole::Exception;
                    }
                }
            } else {
                for param in &mut self.params {
                    if param.role == BlockParamRole::Exception {
                        param.role = BlockParamRole::Local;
                    }
                }
                self.params.push(BlockParam {
                    name: exc_param,
                    role: BlockParamRole::Exception,
                });
            }
        }
        self
    }

    pub fn with_params(mut self, params: Vec<BlockParam>) -> Self {
        for param in params {
            if !self
                .params
                .iter()
                .any(|existing| existing.name == param.name)
            {
                self.params.push(param);
            }
        }
        self
    }

    pub fn push_stmt(&mut self, stmt: S) {
        self.fragment.push_stmt(stmt);
    }

    pub fn extend<I>(&mut self, stmts: I)
    where
        I: IntoIterator<Item = S>,
    {
        self.fragment.extend(stmts);
    }

    pub fn set_term(&mut self, term: T) {
        self.fragment.set_term(term);
    }

    pub fn finish(self, fallthrough_target: Option<BlockPyLabel>) -> BlockPyCfgBlock<S, T> {
        let fragment = self.fragment.finish();
        let block = BlockPyCfgBlock {
            label: self.label,
            body: fragment.body,
            term: fragment.term.unwrap_or_else(|| match fallthrough_target {
                Some(target) => T::jump_term(target),
                None => T::implicit_function_return(),
            }),
            params: self.params,
            exc_edge: self.exc_edge,
        };
        assert_blockpy_block_normalized(&block);
        block
    }
}

#[derive(Debug, Clone)]
pub enum BlockPyStmt<E = Expr> {
    Assign(BlockPyAssign<E>),
    Expr(E),
    Delete(BlockPyDelete),
    If(BlockPyStructuredIf<E>),
}

impl<E: std::fmt::Debug> BlockPyStmt<E> {
    pub fn assert_normalized(&self) {
        if let Self::If(if_stmt) = self {
            if_stmt.body.assert_normalized();
            if_stmt.orelse.assert_normalized();
        }
    }
}

impl<E: std::fmt::Debug> BlockPyNormalizedStmt for BlockPyStmt<E> {
    fn assert_blockpy_normalized(&self) {
        self.assert_normalized();
    }
}

#[derive(Debug, Clone)]
pub enum BbStmt {
    Assign(BlockPyAssign<CoreBlockPyExpr>),
    Expr(CoreBlockPyExpr),
    Delete(BlockPyDelete),
}

impl From<BlockPyAssign<CoreBlockPyExpr>> for BbStmt {
    fn from(value: BlockPyAssign<CoreBlockPyExpr>) -> Self {
        Self::Assign(value)
    }
}

impl From<CoreBlockPyExpr> for BbStmt {
    fn from(value: CoreBlockPyExpr) -> Self {
        Self::Expr(value)
    }
}

impl From<BlockPyDelete> for BbStmt {
    fn from(value: BlockPyDelete) -> Self {
        Self::Delete(value)
    }
}

impl From<BlockPyStmt<CoreBlockPyExpr>> for BbStmt {
    fn from(value: BlockPyStmt<CoreBlockPyExpr>) -> Self {
        match value {
            BlockPyStmt::Assign(assign) => Self::Assign(assign),
            BlockPyStmt::Expr(expr) => Self::Expr(expr),
            BlockPyStmt::Delete(delete) => Self::Delete(delete),
            BlockPyStmt::If(_) => panic!("structured BlockPy If reached BbStmt conversion"),
        }
    }
}

impl IntoBlockPyStmt<CoreBlockPyExpr> for BbStmt {
    fn into_stmt(self) -> BlockPyStmt<CoreBlockPyExpr> {
        match self {
            BbStmt::Assign(assign) => BlockPyStmt::Assign(assign),
            BbStmt::Expr(expr) => BlockPyStmt::Expr(expr),
            BbStmt::Delete(delete) => BlockPyStmt::Delete(delete),
        }
    }
}

impl BlockPyNormalizedStmt for BbStmt {
    fn assert_blockpy_normalized(&self) {}
}

impl<E: Clone + fmt::Debug> IntoBlockPyStmt<E> for BlockPyStmt<E> {
    fn into_stmt(self) -> BlockPyStmt<E> {
        self
    }
}

#[derive(Debug, Clone)]
pub enum BlockPyTerm<E = Expr> {
    Jump(BlockPyEdge),
    IfTerm(BlockPyIfTerm<E>),
    BranchTable(BlockPyBranchTable<E>),
    Raise(BlockPyRaise<E>),
    Return(E),
}

#[derive(Debug, Clone)]
pub struct BlockPyAssign<E = Expr> {
    pub target: ExprName,
    pub value: E,
}

#[derive(Debug, Clone)]
pub struct BlockPyDelete {
    pub target: ExprName,
}

#[derive(Debug, Clone)]
pub struct BlockPyIf<E = Expr, S = BlockPyStmt<E>, T = BlockPyTerm<E>> {
    pub test: E,
    pub body: BlockPyCfgFragment<S, T>,
    pub orelse: BlockPyCfgFragment<S, T>,
}

#[derive(Debug, Clone)]
pub struct BlockPyIfTerm<E = Expr> {
    pub test: E,
    pub then_label: BlockPyLabel,
    pub else_label: BlockPyLabel,
}

#[derive(Debug, Clone)]
pub struct BlockPyBranchTable<E = Expr> {
    pub index: E,
    pub targets: Vec<BlockPyLabel>,
    pub default_label: BlockPyLabel,
}

#[derive(Debug, Clone)]
pub struct BlockPyRaise<E = Expr> {
    pub exc: Option<E>,
}

#[derive(Debug, Clone)]
pub struct BlockPyEdge {
    pub target: BlockPyLabel,
    pub args: Vec<BlockArg>,
}

impl BlockPyEdge {
    pub fn new(target: BlockPyLabel) -> Self {
        Self {
            target,
            args: Vec::new(),
        }
    }

    pub fn with_args(target: BlockPyLabel, args: Vec<BlockArg>) -> Self {
        Self { target, args }
    }

    pub fn as_str(&self) -> &str {
        self.target.as_str()
    }
}

impl From<BlockPyLabel> for BlockPyEdge {
    fn from(value: BlockPyLabel) -> Self {
        Self::new(value)
    }
}

impl From<&str> for BlockPyEdge {
    fn from(value: &str) -> Self {
        Self::new(BlockPyLabel::from(value))
    }
}

impl From<String> for BlockPyEdge {
    fn from(value: String) -> Self {
        Self::new(BlockPyLabel::from(value))
    }
}

#[derive(Debug, Clone)]
pub enum BlockArg {
    Name(String),
    None,
    CurrentException,
    AbruptKind(AbruptKind),
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum AbruptKind {
    Fallthrough,
    Return,
    Exception,
    Break,
    Continue,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum BlockParamRole {
    Local,
    Exception,
    AbruptKind,
    AbruptPayload,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct BlockParam {
    pub name: String,
    pub role: BlockParamRole,
}

pub trait BlockPyModuleVisitor<P>
where
    P: BlockPyPass,
{
    fn visit_module(&mut self, module: &BlockPyModule<P>) {
        walk_module(self, module);
    }

    fn visit_fn(&mut self, func: &BlockPyFunction<P>) {
        walk_fn(self, func);
    }

    fn visit_block(&mut self, block: &PassBlock<P>) {
        walk_block(self, block);
    }

    fn visit_fragment(
        &mut self,
        fragment: &BlockPyCfgFragment<BlockPyStmt<PassExpr<P>>, BlockPyTerm<PassExpr<P>>>,
    ) {
        walk_fragment(self, fragment);
    }

    fn visit_stmt(&mut self, stmt: &BlockPyStmt<PassExpr<P>>) {
        walk_stmt(self, stmt);
    }

    fn visit_term(&mut self, term: &BlockPyTerm<PassExpr<P>>) {
        walk_term(self, term);
    }

    fn visit_label(&mut self, label: &BlockPyLabel) {
        walk_label::<Self, P>(self, label);
    }

    fn visit_expr(&mut self, _expr: &PassExpr<P>) {}
}

pub fn walk_module<V, P>(visitor: &mut V, module: &BlockPyModule<P>)
where
    V: BlockPyModuleVisitor<P> + ?Sized,
    P: BlockPyPass,
{
    for function in &module.callable_defs {
        visitor.visit_fn(function);
    }
}

pub fn walk_fn<V, P>(visitor: &mut V, func: &BlockPyFunction<P>)
where
    V: BlockPyModuleVisitor<P> + ?Sized,
    P: BlockPyPass,
{
    for block in &func.blocks {
        visitor.visit_block(block);
    }
}

pub fn walk_block<V, P>(visitor: &mut V, block: &PassBlock<P>)
where
    V: BlockPyModuleVisitor<P> + ?Sized,
    P: BlockPyPass,
{
    for stmt in &block.body {
        let stmt = stmt.clone().into_stmt();
        visitor.visit_stmt(&stmt);
    }
    if let Some(exc_edge) = &block.exc_edge {
        visitor.visit_label(&exc_edge.target);
    }
    visitor.visit_term(&block.term);
}

pub fn walk_fragment<V, P>(
    visitor: &mut V,
    fragment: &BlockPyCfgFragment<BlockPyStmt<PassExpr<P>>, BlockPyTerm<PassExpr<P>>>,
) where
    V: BlockPyModuleVisitor<P> + ?Sized,
    P: BlockPyPass,
{
    for stmt in &fragment.body {
        visitor.visit_stmt(stmt);
    }
    if let Some(term) = &fragment.term {
        visitor.visit_term(term);
    }
}

pub fn walk_stmt<V, P>(visitor: &mut V, stmt: &BlockPyStmt<PassExpr<P>>)
where
    V: BlockPyModuleVisitor<P> + ?Sized,
    P: BlockPyPass,
{
    match stmt {
        BlockPyStmt::Assign(assign) => visitor.visit_expr(&assign.value),
        BlockPyStmt::Expr(expr) => visitor.visit_expr(expr),
        BlockPyStmt::Delete(_) => {}
        BlockPyStmt::If(if_stmt) => {
            visitor.visit_expr(&if_stmt.test);
            visitor.visit_fragment(&if_stmt.body);
            visitor.visit_fragment(&if_stmt.orelse);
        }
    }
}

pub fn walk_label<V, P>(visitor: &mut V, label: &BlockPyLabel)
where
    V: BlockPyModuleVisitor<P> + ?Sized,
    P: BlockPyPass,
{
    let _ = visitor;
    let _ = label;
}

pub fn walk_term<V, P>(visitor: &mut V, term: &BlockPyTerm<PassExpr<P>>)
where
    V: BlockPyModuleVisitor<P> + ?Sized,
    P: BlockPyPass,
{
    match term {
        BlockPyTerm::Jump(edge) => {
            visitor.visit_label(&edge.target);
        }
        BlockPyTerm::IfTerm(if_term) => {
            visitor.visit_expr(&if_term.test);
            visitor.visit_label(&if_term.then_label);
            visitor.visit_label(&if_term.else_label);
        }
        BlockPyTerm::BranchTable(branch) => {
            visitor.visit_expr(&branch.index);
            for target in &branch.targets {
                visitor.visit_label(target);
            }
            visitor.visit_label(&branch.default_label);
        }
        BlockPyTerm::Raise(raise_stmt) => {
            if let Some(exc) = &raise_stmt.exc {
                visitor.visit_expr(exc);
            }
        }
        BlockPyTerm::Return(value) => visitor.visit_expr(value),
    }
}

impl<E> BlockPyJumpTerm<BlockPyLabel> for BlockPyTerm<E> {
    fn jump_term(target: BlockPyLabel) -> Self {
        Self::Jump(BlockPyEdge::new(target))
    }
}

impl ImplicitNoneExpr for Expr {
    fn implicit_none_expr() -> Self {
        py_expr!("__dp_NONE")
    }

    fn is_implicit_none_expr(expr: &Self) -> bool {
        matches!(expr, Expr::Name(name) if name.id.as_str() == "__dp_NONE")
    }
}

impl ImplicitNoneExpr for CoreBlockPyExprWithAwaitAndYield {
    fn implicit_none_expr() -> Self {
        Self::Name(implicit_none_name())
    }

    fn is_implicit_none_expr(expr: &Self) -> bool {
        matches!(expr, CoreBlockPyExprWithAwaitAndYield::Name(name) if name.id.as_str() == "__dp_NONE")
    }
}

impl ImplicitNoneExpr for CoreBlockPyExprWithYield {
    fn implicit_none_expr() -> Self {
        Self::Name(implicit_none_name())
    }

    fn is_implicit_none_expr(expr: &Self) -> bool {
        matches!(expr, CoreBlockPyExprWithYield::Name(name) if name.id.as_str() == "__dp_NONE")
    }
}

impl ImplicitNoneExpr for CoreBlockPyExpr {
    fn implicit_none_expr() -> Self {
        Self::Name(implicit_none_name())
    }

    fn is_implicit_none_expr(expr: &Self) -> bool {
        matches!(expr, CoreBlockPyExpr::Name(name) if name.id.as_str() == "__dp_NONE")
    }
}

impl<E: ImplicitNoneExpr> BlockPyFallthroughTerm<BlockPyLabel> for BlockPyTerm<E> {
    fn implicit_function_return() -> Self {
        Self::Return(E::implicit_none_expr())
    }
}

impl<PIn> BlockPyModule<PIn>
where
    PIn: BlockPyPass,
{
    pub fn visit_module(&self, visitor: &mut impl BlockPyModuleVisitor<PIn>) {
        visitor.visit_module(self);
    }
}

#[cfg(test)]
mod test;

impl BlockPyLabel {
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl fmt::Display for BlockPyLabel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl PartialEq<&str> for BlockPyLabel {
    fn eq(&self, other: &&str) -> bool {
        self.as_str() == *other
    }
}
