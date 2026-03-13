#[derive(Debug, Clone)]
pub struct CfgBlock<L, S, T, M = ()> {
    pub label: L,
    pub body: Vec<S>,
    pub term: T,
    pub meta: M,
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

#[derive(Debug, Clone, Default)]
pub struct CfgModule<F> {
    pub module_init: Option<String>,
    pub callable_defs: Vec<F>,
}
