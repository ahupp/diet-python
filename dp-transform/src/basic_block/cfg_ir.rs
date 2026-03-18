use super::block_py::BlockPyLabel;
use super::lowered_ir::FunctionId;
use super::param_specs::ParamSpec;

#[derive(Debug, Clone)]
pub struct CfgBlock<S, T, M = ()> {
    pub label: BlockPyLabel,
    pub body: Vec<S>,
    pub term: T,
    pub meta: M,
}

impl<S, T, M> CfgBlock<S, T, M> {
    pub fn label_str(&self) -> &str {
        self.label.as_str()
    }
}

#[derive(Debug, Clone)]
pub struct CfgCallableDef<K, D, B> {
    pub function_id: FunctionId,
    pub bind_name: String,
    pub display_name: String,
    pub qualname: String,
    pub kind: K,
    pub params: ParamSpec,
    pub param_defaults: Vec<D>,
    pub entry_liveins: Vec<String>,
    pub blocks: Vec<B>,
}

impl<K, D, S, T, M> CfgCallableDef<K, D, CfgBlock<S, T, M>> {
    pub fn entry_block(&self) -> &CfgBlock<S, T, M> {
        self.blocks
            .first()
            .expect("CfgCallableDef should have at least one block")
    }

    pub fn entry_label(&self) -> &str {
        self.entry_block().label_str()
    }
}

#[derive(Debug, Clone, Default)]
pub struct CfgModule<F> {
    pub callable_defs: Vec<F>,
}

impl<F> CfgModule<F> {
    pub fn map_callable_defs<G>(&self, mut f: impl FnMut(&F) -> G) -> CfgModule<G> {
        CfgModule {
            callable_defs: self.callable_defs.iter().map(&mut f).collect(),
        }
    }
}
