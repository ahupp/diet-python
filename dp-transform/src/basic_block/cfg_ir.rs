#[derive(Debug, Clone)]
pub struct CfgBlock<L, S, T, M = ()> {
    pub label: L,
    pub body: Vec<S>,
    pub term: T,
    pub meta: M,
}
