use ruff_python_ast::{self as ast, Expr, Stmt};

use crate::py_stmt;

fn future_import_insert_index(module: &ast::ModModule) -> usize {
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
    insert_at
}

pub fn ensure_future_explicit_scope(module: &mut ast::ModModule) {
    let has_explicit_scope = module.body.iter().any(|stmt| {
        if let Stmt::ImportFrom(ast::StmtImportFrom {
            module: Some(module_name),
            names,
            ..
        }) = stmt
        {
            if module_name.id.as_str() != "__future__" {
                return false;
            }
            return names
                .iter()
                .any(|alias| alias.name.id.as_str() == "explicit_scope");
        }
        false
    });
    if has_explicit_scope {
        return;
    }
    let import = py_stmt!("from __future__ import explicit_scope");
    let insert_at = future_import_insert_index(module);
    module.body.splice(insert_at..insert_at, import);
}

pub fn ensure_module_annotations(module: &mut ast::ModModule) {
    if !module_needs_annotations(&module.body) {
        return;
    }

    let insert_at = future_import_insert_index(module);
    module
        .body
        .splice(insert_at..insert_at, py_stmt!("__annotations__ = __dp__.dict()"));
}

pub fn ensure_import(module: &mut ast::ModModule) {
    let import = py_stmt!("import __dp__");
    let insert_at = future_import_insert_index(module);
    module.body.splice(insert_at..insert_at, import);
}

fn module_needs_annotations(stmts: &[Stmt]) -> bool {
    stmts.iter().any(stmt_needs_annotations)
}

fn stmt_needs_annotations(stmt: &Stmt) -> bool {
    match stmt {
        Stmt::AnnAssign(_) => true,
        Stmt::FunctionDef(_) | Stmt::ClassDef(_) => false,
        Stmt::If(ast::StmtIf {
            body,
            elif_else_clauses,
            ..
        }) => {
            if module_needs_annotations(body) {
                return true;
            }
            elif_else_clauses
                .iter()
                .any(|clause| module_needs_annotations(&clause.body))
        }
        Stmt::For(ast::StmtFor { body, orelse, .. })
        | Stmt::While(ast::StmtWhile { body, orelse, .. }) => {
            module_needs_annotations(body) || module_needs_annotations(orelse)
        }
        Stmt::With(ast::StmtWith { body, .. }) => module_needs_annotations(body),
        Stmt::Try(ast::StmtTry {
            body,
            handlers,
            orelse,
            finalbody,
            ..
        }) => {
            if module_needs_annotations(body)
                || module_needs_annotations(orelse)
                || module_needs_annotations(finalbody)
            {
                return true;
            }
            handlers.iter().any(|handler| match handler {
                ast::ExceptHandler::ExceptHandler(handler) => {
                    module_needs_annotations(&handler.body)
                }
            })
        }
        Stmt::Match(ast::StmtMatch { cases, .. }) => cases
            .iter()
            .any(|case| module_needs_annotations(&case.body)),
        _ => false,
    }
}
