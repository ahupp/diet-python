
I'd like to merge the CfgModule struct into BlockPyModule.  To simplify the typing, lets use a trait with associated types.

trait BlockPyPhase {
    type Expr;
    type Stmt;
    type Term;
    type BlockMeta;
    type Block = CfgBlock<..., Self::BlockMeta>;
    type Function = BlockPyCallable<Self::Expr, Self::Block>;
}

// What you've been calling "semantic"
struct RuffBlockPy;

impl BlockPyPhase for RuffBlockPy {
    type Expr = ruff_python_ast::Expr;
    type Stmt = BlockPyStmt<Expr>;
    type Term = BlockPyTerm<Expr>;
    type BlockMeta = ();
}




There are many places we do a transform manually over BlockPyModule.  Define a pair of traits:

trait BlockPyModuleVisitor<P: BlockPyPhase> {
    fn visit_module(&self, module: &BlockPyModule<P>);
    fn visit_stmt(&self, module: &P::Stmt);
    fn visit_term(&self, module: &P::Term);
    fn visit_expr(&self, module: &P::Expr);
}

and

trait BlockPyModuleMap<PIn: BlockPyPhase, POut: BlockPyPhase> {
    fn map_module(&self, module: BlockPyModule<PIn>) -> BlockPyModule<POut>;
    fn map_fn(&self, func: PIn::Function) -> POut::Function;
    fn map_stmt(&self, stmt: PIn::Stmt) -> POut::Stmt;
    fn map_term(&self, term: PIn::Term) -> POut::Term;
    fn map_expr(&self, expr: PIn::Expr) -> POut::Expr;
}