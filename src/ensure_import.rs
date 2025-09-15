use ruff_python_ast::{self as ast, Expr, Stmt};

use crate::py_stmt;

pub fn ensure_import(module: &mut ast::ModModule) {
    let import = py_stmt!("import __dp__");
    let mut insert_at = 0;
    if let Some(Stmt::Expr(ast::StmtExpr { value, .. })) = module.body.get(0) {
        if matches!(**value, Expr::StringLiteral(_)) {
            insert_at = 1;
        }
    }
    while insert_at < module.body.len() {
        if let Stmt::ImportFrom(ast::StmtImportFrom {
            module: Some(module_name),
            ..
        }) = &module.body[insert_at]
        {
            if module_name.id.as_str() == "__future__" {
                insert_at += 1;
                continue;
            }
        }
        break;
    }
    module.body.insert(insert_at, import);
}
