use ruff_python_ast::{self as ast, Expr, Stmt, StmtBody};

use crate::py_stmt;
use crate::transform::context::Context;

fn future_import_insert_index(module: &[Box<Stmt>]) -> usize {
    let mut insert_at = 0;
    if let Some(Stmt::Expr(ast::StmtExpr { value, .. })) = module.get(0).map(|stmt| stmt.as_ref()) {
        if matches!(**value, Expr::StringLiteral(_)) {
            insert_at = 1;
        }
    }
    while insert_at < module.len() {
        if let Stmt::ImportFrom(ast::StmtImportFrom {
            module: Some(module_name),
            ..
        }) = module[insert_at].as_ref()
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

pub fn ensure_imports(context: &Context, module: &mut StmtBody) {
    let mut imports = vec![py_stmt!("__dp__ = __import__(\"__dp__\")")];

    if context.needs_typing_import() {
        imports.push(py_stmt!("_dp_typing = __import__(\"typing\")"));
    }

    if context.needs_templatelib_import() {
        imports.push(py_stmt!(
            "_dp_templatelib = __dp__.import_(\"string.templatelib\", __spec__, __dp__.list((\"templatelib\",)))"
        ));
    }

    let insert_at = future_import_insert_index(&module.body);
    let imports = imports.into_iter().map(Box::new).collect::<Vec<_>>();
    module.body.splice(insert_at..insert_at, imports);
}
