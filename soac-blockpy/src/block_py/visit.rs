use super::*;

pub(crate) fn instr_any<I, F>(instr: &I, mut predicate: F) -> bool
where
    I: Instr + ChildVisitable<I>,
    F: FnMut(&I) -> bool,
{
    fn instr_any_impl<I, F>(instr: &I, predicate: &mut F) -> bool
    where
        I: Instr + ChildVisitable<I>,
        F: FnMut(&I) -> bool,
    {
        if predicate(instr) {
            return true;
        }

        struct AnyChildVisitor<'a, I, F> {
            predicate: &'a mut F,
            found: bool,
            _marker: std::marker::PhantomData<fn(&I)>,
        }

        impl<I, F> VisitInstr<I> for AnyChildVisitor<'_, I, F>
        where
            I: Instr + ChildVisitable<I>,
            F: FnMut(&I) -> bool,
        {
            fn visit_instr(&mut self, expr: &I) {
                if !self.found && instr_any_impl(expr, self.predicate) {
                    self.found = true;
                }
            }
        }

        let mut visitor = AnyChildVisitor {
            predicate,
            found: false,
            _marker: std::marker::PhantomData,
        };
        instr.visit_children(&mut visitor);
        visitor.found
    }

    instr_any_impl(instr, &mut predicate)
}

pub trait VisitInstr<I: Instr> {
    fn visit_instr(&mut self, expr: &I)
    where
        I: ChildVisitable<I>,
    {
        walk_expr(self, expr);
    }
}

pub trait VisitMutInstr<I: Instr> {
    fn visit_instr_mut(&mut self, expr: &mut I)
    where
        I: ChildVisitable<I>,
    {
        walk_expr_mut(self, expr);
    }
}

pub(crate) trait VisitTerm<I: Instr> {
    fn visit_term(&mut self, term: &BlockTerm<I>)
    where
        Self: VisitInstr<I>,
        I: ChildVisitable<I>,
    {
        walk_term(self, term);
    }

    fn visit_edge(&mut self, edge: &BlockEdge) {
        walk_edge(self, edge);
    }

    fn visit_label(&mut self, label: &BlockLabel) {
        let _ = label;
    }

    fn visit_block_arg(&mut self, arg: &BlockArg) {
        let _ = arg;
    }

    fn visit_if_term(&mut self, if_term: &TermIf<I>)
    where
        Self: VisitInstr<I>,
        I: ChildVisitable<I>,
    {
        walk_if_term(self, if_term);
    }

    fn visit_branch_table_term(&mut self, branch: &TermBranchTable<I>)
    where
        Self: VisitInstr<I>,
        I: ChildVisitable<I>,
    {
        walk_branch_table_term(self, branch);
    }

    fn visit_raise_term(&mut self, raise_term: &TermRaise<I>)
    where
        Self: VisitInstr<I>,
        I: ChildVisitable<I>,
    {
        walk_raise_term(self, raise_term);
    }

    fn visit_return_term(&mut self, value: &I)
    where
        Self: VisitInstr<I>,
        I: ChildVisitable<I>,
    {
        self.visit_instr(value);
    }
}

pub(crate) trait VisitMutTerm<I: Instr> {
    fn visit_term_mut(&mut self, term: &mut BlockTerm<I>)
    where
        Self: VisitMutInstr<I>,
        I: ChildVisitable<I>,
    {
        walk_term_mut(self, term);
    }

    fn visit_edge_mut(&mut self, edge: &mut BlockEdge) {
        walk_edge_mut(self, edge);
    }

    fn visit_label_mut(&mut self, label: &mut BlockLabel) {
        let _ = label;
    }

    fn visit_block_arg_mut(&mut self, arg: &mut BlockArg) {
        let _ = arg;
    }

    fn visit_if_term_mut(&mut self, if_term: &mut TermIf<I>)
    where
        Self: VisitMutInstr<I>,
        I: ChildVisitable<I>,
    {
        walk_if_term_mut(self, if_term);
    }

    fn visit_branch_table_term_mut(&mut self, branch: &mut TermBranchTable<I>)
    where
        Self: VisitMutInstr<I>,
        I: ChildVisitable<I>,
    {
        walk_branch_table_term_mut(self, branch);
    }

    fn visit_raise_term_mut(&mut self, raise_term: &mut TermRaise<I>)
    where
        Self: VisitMutInstr<I>,
        I: ChildVisitable<I>,
    {
        walk_raise_term_mut(self, raise_term);
    }

    fn visit_return_term_mut(&mut self, value: &mut I)
    where
        Self: VisitMutInstr<I>,
        I: ChildVisitable<I>,
    {
        self.visit_instr_mut(value);
    }
}

pub(crate) trait VisitBlock<I: Instr>:
    VisitInstr<I> + VisitTerm<I>
{
    fn visit_block(&mut self, block: &Block<I, I>)
    where
        I: ChildVisitable<I>,
    {
        walk_block(self, block);
    }

    fn visit_block_param(&mut self, param: &BlockParam) {
        let _ = param;
    }

    fn visit_stmt(&mut self, stmt: &I)
    where
        I: ChildVisitable<I>,
    {
        self.visit_instr(stmt);
    }

    fn visit_exception_edge(&mut self, edge: &BlockEdge) {
        self.visit_edge(edge);
    }
}

pub(crate) trait VisitMutBlock<I: Instr>:
    VisitMutInstr<I> + VisitMutTerm<I>
{
    fn visit_block_mut(&mut self, block: &mut Block<I, I>)
    where
        I: ChildVisitable<I>,
    {
        walk_block_mut(self, block);
    }

    fn visit_block_param_mut(&mut self, param: &mut BlockParam) {
        let _ = param;
    }

    fn visit_stmt_mut(&mut self, stmt: &mut I)
    where
        I: ChildVisitable<I>,
    {
        self.visit_instr_mut(stmt);
    }

    fn visit_exception_edge_mut(&mut self, edge: &mut BlockEdge) {
        self.visit_edge_mut(edge);
    }
}

pub(crate) trait VisitFunction<P: BlockPyPass>: VisitBlock<P::Expr> {
    fn visit_fn(&mut self, func: &BlockPyFunction<P>)
    where
        P::Expr: ChildVisitable<P::Expr>,
    {
        walk_fn(self, func);
    }
}

pub(crate) trait VisitMutFunction<P: BlockPyPass>: VisitMutBlock<P::Expr> {
    fn visit_fn_mut(&mut self, func: &mut BlockPyFunction<P>)
    where
        P::Expr: ChildVisitable<P::Expr>,
    {
        walk_fn_mut(self, func);
    }
}

pub(crate) trait VisitModule<P: BlockPyPass>: VisitFunction<P> {
    fn visit_module(&mut self, module: &BlockPyModule<P>)
    where
        P::Expr: ChildVisitable<P::Expr>,
    {
        walk_module(self, module);
    }
}

pub(crate) trait VisitMutModule<P: BlockPyPass>: VisitMutFunction<P> {
    fn visit_module_mut(&mut self, module: &mut BlockPyModule<P>)
    where
        P::Expr: ChildVisitable<P::Expr>,
    {
        walk_module_mut(self, module);
    }
}

pub(crate) fn walk_module<V, P>(visitor: &mut V, module: &BlockPyModule<P>)
where
    V: VisitModule<P> + ?Sized,
    P: BlockPyPass,
    P::Expr: ChildVisitable<P::Expr>,
{
    for func in &module.callable_defs {
        visitor.visit_fn(func);
    }
}

pub(crate) fn walk_module_mut<V, P>(visitor: &mut V, module: &mut BlockPyModule<P>)
where
    V: VisitMutModule<P> + ?Sized,
    P: BlockPyPass,
    P::Expr: ChildVisitable<P::Expr>,
{
    for func in &mut module.callable_defs {
        visitor.visit_fn_mut(func);
    }
}

pub(crate) fn walk_fn<V, P>(visitor: &mut V, func: &BlockPyFunction<P>)
where
    V: VisitFunction<P> + ?Sized,
    P: BlockPyPass,
    P::Expr: ChildVisitable<P::Expr>,
{
    for block in &func.blocks {
        visitor.visit_block(block);
    }
}

pub(crate) fn walk_fn_mut<V, P>(visitor: &mut V, func: &mut BlockPyFunction<P>)
where
    V: VisitMutFunction<P> + ?Sized,
    P: BlockPyPass,
    P::Expr: ChildVisitable<P::Expr>,
{
    for block in &mut func.blocks {
        visitor.visit_block_mut(block);
    }
}

pub(crate) fn walk_block<V, I>(visitor: &mut V, block: &Block<I, I>)
where
    V: VisitBlock<I> + ?Sized,
    I: Instr + ChildVisitable<I>,
{
    for param in &block.params {
        visitor.visit_block_param(param);
    }
    for stmt in &block.body {
        visitor.visit_stmt(stmt);
    }
    if let Some(exc_edge) = &block.exc_edge {
        visitor.visit_exception_edge(exc_edge);
    }
    visitor.visit_term(&block.term);
}

pub(crate) fn walk_block_mut<V, I>(visitor: &mut V, block: &mut Block<I, I>)
where
    V: VisitMutBlock<I> + ?Sized,
    I: Instr + ChildVisitable<I>,
{
    for param in &mut block.params {
        visitor.visit_block_param_mut(param);
    }
    for stmt in &mut block.body {
        visitor.visit_stmt_mut(stmt);
    }
    if let Some(exc_edge) = &mut block.exc_edge {
        visitor.visit_exception_edge_mut(exc_edge);
    }
    visitor.visit_term_mut(&mut block.term);
}

pub(crate) fn walk_stmt<V, I>(visitor: &mut V, stmt: &I)
where
    V: VisitBlock<I> + ?Sized,
    I: Instr + ChildVisitable<I>,
{
    visitor.visit_instr(stmt);
}

pub(crate) fn walk_stmt_mut<V, I>(visitor: &mut V, stmt: &mut I)
where
    V: VisitMutBlock<I> + ?Sized,
    I: Instr + ChildVisitable<I>,
{
    visitor.visit_instr_mut(stmt);
}

pub(crate) fn walk_edge<V, I>(visitor: &mut V, edge: &BlockEdge)
where
    V: VisitTerm<I> + ?Sized,
    I: Instr,
{
    visitor.visit_label(&edge.target);
    for arg in &edge.args {
        visitor.visit_block_arg(arg);
    }
}

pub(crate) fn walk_edge_mut<V, I>(visitor: &mut V, edge: &mut BlockEdge)
where
    V: VisitMutTerm<I> + ?Sized,
    I: Instr,
{
    visitor.visit_label_mut(&mut edge.target);
    for arg in &mut edge.args {
        visitor.visit_block_arg_mut(arg);
    }
}

pub(crate) fn walk_term<V, I>(visitor: &mut V, term: &BlockTerm<I>)
where
    V: VisitTerm<I> + VisitInstr<I> + ?Sized,
    I: Instr + ChildVisitable<I>,
{
    match term {
        BlockTerm::Jump(edge) => visitor.visit_edge(edge),
        BlockTerm::IfTerm(if_term) => visitor.visit_if_term(if_term),
        BlockTerm::BranchTable(branch) => visitor.visit_branch_table_term(branch),
        BlockTerm::Raise(raise_term) => visitor.visit_raise_term(raise_term),
        BlockTerm::Return(value) => visitor.visit_return_term(value),
    }
}

pub(crate) fn walk_term_mut<V, I>(visitor: &mut V, term: &mut BlockTerm<I>)
where
    V: VisitMutTerm<I> + VisitMutInstr<I> + ?Sized,
    I: Instr + ChildVisitable<I>,
{
    match term {
        BlockTerm::Jump(edge) => visitor.visit_edge_mut(edge),
        BlockTerm::IfTerm(if_term) => visitor.visit_if_term_mut(if_term),
        BlockTerm::BranchTable(branch) => visitor.visit_branch_table_term_mut(branch),
        BlockTerm::Raise(raise_term) => visitor.visit_raise_term_mut(raise_term),
        BlockTerm::Return(value) => visitor.visit_return_term_mut(value),
    }
}

pub(crate) fn walk_if_term<V, I>(visitor: &mut V, if_term: &TermIf<I>)
where
    V: VisitTerm<I> + VisitInstr<I> + ?Sized,
    I: Instr + ChildVisitable<I>,
{
    visitor.visit_instr(&if_term.test);
    visitor.visit_label(&if_term.then_label);
    visitor.visit_label(&if_term.else_label);
}

pub(crate) fn walk_if_term_mut<V, I>(visitor: &mut V, if_term: &mut TermIf<I>)
where
    V: VisitMutTerm<I> + VisitMutInstr<I> + ?Sized,
    I: Instr + ChildVisitable<I>,
{
    visitor.visit_instr_mut(&mut if_term.test);
    visitor.visit_label_mut(&mut if_term.then_label);
    visitor.visit_label_mut(&mut if_term.else_label);
}

pub(crate) fn walk_branch_table_term<V, I>(visitor: &mut V, branch: &TermBranchTable<I>)
where
    V: VisitTerm<I> + VisitInstr<I> + ?Sized,
    I: Instr + ChildVisitable<I>,
{
    visitor.visit_instr(&branch.index);
    for target in &branch.targets {
        visitor.visit_label(target);
    }
    visitor.visit_label(&branch.default_label);
}

pub(crate) fn walk_branch_table_term_mut<V, I>(visitor: &mut V, branch: &mut TermBranchTable<I>)
where
    V: VisitMutTerm<I> + VisitMutInstr<I> + ?Sized,
    I: Instr + ChildVisitable<I>,
{
    visitor.visit_instr_mut(&mut branch.index);
    for target in &mut branch.targets {
        visitor.visit_label_mut(target);
    }
    visitor.visit_label_mut(&mut branch.default_label);
}

pub(crate) fn walk_raise_term<V, I>(visitor: &mut V, raise_term: &TermRaise<I>)
where
    V: VisitTerm<I> + VisitInstr<I> + ?Sized,
    I: Instr + ChildVisitable<I>,
{
    if let Some(exc) = &raise_term.exc {
        visitor.visit_instr(exc);
    }
}

pub(crate) fn walk_raise_term_mut<V, I>(visitor: &mut V, raise_term: &mut TermRaise<I>)
where
    V: VisitMutTerm<I> + VisitMutInstr<I> + ?Sized,
    I: Instr + ChildVisitable<I>,
{
    if let Some(exc) = &mut raise_term.exc {
        visitor.visit_instr_mut(exc);
    }
}

pub(crate) fn walk_expr<V, I>(visitor: &mut V, expr: &I)
where
    V: VisitInstr<I> + ?Sized,
    I: Instr + ChildVisitable<I>,
{
    expr.visit_children(visitor);
}

pub(crate) fn walk_expr_mut<V, I>(visitor: &mut V, expr: &mut I)
where
    V: VisitMutInstr<I> + ?Sized,
    I: Instr + ChildVisitable<I>,
{
    expr.visit_children_mut(visitor);
}
