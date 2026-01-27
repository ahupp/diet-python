use ruff_python_ast::{self as ast, Expr, Stmt};

use crate::py_stmt;

fn future_import_insert_index(module: &Vec<Stmt>) -> usize {
    let mut insert_at = 0;
    if let Some(Stmt::Expr(ast::StmtExpr { value, .. })) = module.get(0) {
        if matches!(**value, Expr::StringLiteral(_)) {
            insert_at = 1;
        }
    }
    while insert_at < module.len() {
        if let Stmt::ImportFrom(ast::StmtImportFrom {
            module: Some(module_name),
            ..
        }) = &module[insert_at]
        {
            if module_name.id.as_str() == "__future__" {
                insert_at += 1;
                continue;
            }
        }
        break;
    }
    insert_at
}

pub fn ensure_import(module: &mut Vec<Stmt>) {
    let import = py_stmt!("__dp__ = __import__(\"__dp__\")");
    let insert_at = future_import_insert_index(module);
    module.splice(insert_at..insert_at, import);
}

