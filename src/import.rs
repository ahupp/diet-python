use ruff_python_ast::{self as ast, Stmt};

pub fn ensure_import(module: &mut ast::ModModule, name: &str) {
    let has_import = module.body.iter().any(|stmt| {
        if let Stmt::Import(ast::StmtImport { names, .. }) = stmt {
            names.iter().any(|alias| alias.name.id.as_str() == name)
        } else {
            false
        }
    });

    if !has_import {
        let import = crate::py_stmt!("import {name:id}", name = name);
        module.body.insert(0, import);
    }
}
