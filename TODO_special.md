
BlockPyLabel doesn't need both teh from_u32_index/from_index and the
From<> impl, just have one of them.

===

Remove FunctionId.plan_qualname

===

Just use Debug for pretty-print LocalLocation and CellLocation

===

Move all callers of MapExpr and TryMapExpr to use the corresponding functions on InstrExprNode

===

The trait method:


  pub trait BlockPyExprLike: Clone + fmt::Debug + MapExpr<Self> {
    fn walk_child_exprs<F>(&self, f: &mut F)

should go away because it's users switch to visit_exprs

===

Also should impl InstrExprNode:

impl MapExpr<Expr> for Expr {
    fn map_expr(self, f: &mut impl FnMut(Self) -> Expr) -> Expr {
        struct DirectChildTransformer<'a, F>(&'a mut F);

but why do we need this at all?

===

Do we need this?

  impl<T> BlockPyExprLike for T where T: Clone + fmt::Debug + MapExpr<Self> {}

seems like those constraints are probably already specified.

===

Not sure why we have these impl:

    impl From<LocatedName> for ast::ExprName {
         fn from(value: LocatedName) -> Self {

===

Verify     MakeString(MakeString), isn't being produced anymore, remove it and it's consumers in codegen.

===

CodegenBlockPyLiteral and CoreBlockPyLiteral, why are these different types?

===

What is this trait for?


pub(crate) trait CoreCallLikeExpr: Sized + Instr {
    type Name: BlockPyNameLike + From<ast::ExprName>;

    fn from_name(name: ast::ExprName) -> Self;

    fn from_operation(operation: block_py_operation::OperationDetail<Self>) -> Self;
}

implemetn expr_any in terms of the new walk function
