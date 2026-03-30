use super::*;

// These statements are still expected to be handled by earlier AST passes or by
// the stmt-sequence control-flow lowering. Keeping them together makes the
// boundary explicit without burying the real lowering code in panic-only files.

impl_unreduced_stmt_lowerer!(
    ast::StmtFunctionDef,
    Stmt::FunctionDef,
    "FunctionDef should be extracted before Ruff AST -> BlockPy conversion"
);
impl_unreduced_stmt_lowerer!(
    ast::StmtClassDef,
    Stmt::ClassDef,
    "ClassDef should be lowered before Ruff AST -> BlockPy conversion"
);
impl_unreduced_stmt_lowerer!(
    ast::StmtAnnAssign,
    Stmt::AnnAssign,
    "AnnAssign should be lowered before Ruff AST -> BlockPy conversion"
);
impl_unreduced_stmt_lowerer!(
    ast::StmtWhile,
    Stmt::While,
    "While should be lowered before Ruff AST -> BlockPy stmt-list conversion"
);
impl_unreduced_stmt_lowerer!(
    ast::StmtFor,
    Stmt::For,
    "For should be lowered before Ruff AST -> BlockPy stmt-list conversion"
);
impl_unreduced_stmt_lowerer!(
    ast::StmtIpyEscapeCommand,
    Stmt::IpyEscapeCommand,
    "IpyEscapeCommand should not reach BlockPy conversion"
);
