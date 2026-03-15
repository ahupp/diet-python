#[derive(Debug, Clone)]
pub struct CfgBlock<L, S, T, M = ()> {
    pub label: L,
    pub body: Vec<S>,
    pub term: T,
    pub meta: M,
}

impl<L: AsRef<str>, S, T, M> CfgBlock<L, S, T, M> {
    pub fn label_str(&self) -> &str {
        self.label.as_ref()
    }
}

#[derive(Debug, Clone)]
pub struct CfgCallableDef<I, K, P, B> {
    pub function_id: I,
    pub bind_name: String,
    pub display_name: String,
    pub qualname: String,
    pub kind: K,
    pub params: P,
    pub entry_liveins: Vec<String>,
    pub blocks: Vec<B>,
}

impl<I, K, P, L: AsRef<str>, S, T, M> CfgCallableDef<I, K, P, CfgBlock<L, S, T, M>> {
    pub fn entry_block(&self) -> &CfgBlock<L, S, T, M> {
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
    pub module_init: Option<String>,
    pub callable_defs: Vec<F>,
}

impl<F> CfgModule<F> {
    pub fn map_callable_defs<G>(&self, mut f: impl FnMut(&F) -> G) -> CfgModule<G> {
        CfgModule {
            module_init: self.module_init.clone(),
            callable_defs: self.callable_defs.iter().map(&mut f).collect(),
        }
    }
}
