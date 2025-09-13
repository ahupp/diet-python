use ruff_python_ast::{self as ast, Expr, Stmt};

pub fn ensure_import(module: &mut ast::ModModule, name: &str) {
    let import = crate::py_stmt!("\nimport {name:id}", name = name);
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
