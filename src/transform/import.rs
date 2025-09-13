use ruff_python_ast::visitor::transformer::{walk_stmt, Transformer};
use ruff_python_ast::{self as ast, Expr, Stmt};

const INTRINSICS: &[&str] = &[
    "add",
    "sub",
    "mul",
    "matmul",
    "truediv",
    "floordiv",
    "mod",
    "pow",
    "lshift",
    "rshift",
    "or_",
    "xor",
    "and_",
    "getitem",
    "setitem",
    "delitem",
    "iadd",
    "isub",
    "imul",
    "imatmul",
    "itruediv",
    "imod",
    "ipow",
    "ilshift",
    "irshift",
    "ior",
    "ixor",
    "iand",
    "ifloordiv",
    "pos",
    "neg",
    "invert",
    "not_",
    "truth",
    "eq",
    "ne",
    "lt",
    "le",
    "gt",
    "ge",
    "is_",
    "is_not",
    "contains",
    "next",
    "iter",
    "aiter",
    "anext",
    "isinstance",
    "setattr",
    "resolve_bases",
    "prepare_class",
    "exc_info",
    "current_exception",
    "raise_from",
    "import_",
    "if_expr",
    "or_expr",
    "and_expr",
];

pub fn ensure_import(module: &mut ast::ModModule, name: &str) {
    let mut import_index = None;
    for (idx, stmt) in module.body.iter().enumerate() {
        if let Stmt::Import(ast::StmtImport { names, .. }) = stmt {
            if names.iter().any(|alias| alias.name.id.as_str() == name) {
                import_index = Some(idx);
                break;
            }
        }
    }
    let has_intrinsics = module.body.iter().any(|stmt| {
        if let Stmt::Assign(ast::StmtAssign { targets, .. }) = stmt {
            if let Some(Expr::Name(ast::ExprName { id, .. })) = targets.get(0) {
                id.as_str() == "_dp_add"
            } else {
                false
            }
        } else {
            false
        }
    });

    if import_index.is_none() {
        let import = crate::py_stmt!("\nimport {name:id}", name = name);
        let mut insert_at = 0;
        if let Some(Stmt::Expr(ast::StmtExpr { value, .. })) = module.body.get(0) {
            if matches!(**value, Expr::StringLiteral(_)) {
                insert_at = 1;
            }
        }
        while insert_at < module.body.len() {
            if let Stmt::ImportFrom(ast::StmtImportFrom { module: Some(module_name), .. }) = &module.body[insert_at] {
                if module_name.id.as_str() == "__future__" {
                    insert_at += 1;
                    continue;
                }
            }
            break;
        }
        module.body.insert(insert_at, import);
        import_index = Some(insert_at);
    }

    if name == "__dp__" && !has_intrinsics {
        let insert_pos = import_index.unwrap() + 1;
        for (i, func) in INTRINSICS.iter().enumerate() {
            let alias = format!("_dp_{func}");
            let assign = crate::py_stmt!(
                "\n{alias:id} = __dp__.{func:id}",
                alias = alias.as_str(),
                func = *func,
            );
            module.body.insert(insert_pos + i, assign);
        }
    }
}

pub struct ImportRewriter;

impl ImportRewriter {
    pub fn new() -> Self {
        Self
    }
}

impl Transformer for ImportRewriter {
    fn visit_stmt(&self, stmt: &mut Stmt) {
        walk_stmt(self, stmt);
        match stmt {
            Stmt::Import(ast::StmtImport { names, .. }) => {
                let mut stmts = Vec::new();
                for alias in names {
                    let module_name = alias.name.id.to_string();
                    let binding = alias
                        .asname
                        .as_ref()
                        .map(|n| n.id.as_str())
                        .unwrap_or_else(|| module_name.split('.').next().unwrap());
                    let assign = crate::py_stmt!(
                        "
{name:id} = _dp_import_({module:literal}, __spec__)
",
                        name = binding,
                        module = module_name.as_str(),
                    );
                    stmts.push(assign);
                }
                *stmt = crate::py_stmt!("{body:stmt}", body = stmts);
            }
            Stmt::ImportFrom(ast::StmtImportFrom {
                module,
                names,
                level,
                ..
            }) => {
                if names.iter().any(|alias| alias.name.id.as_str() == "*") {
                    return;
                }
                let module_name = module.as_ref().map(|n| n.id.as_str()).unwrap_or("");
                let level_val = *level;
                let mut stmts = Vec::new();
                for alias in names {
                    let orig = alias.name.id.as_str();
                    let binding = alias.asname.as_ref().map(|n| n.id.as_str()).unwrap_or(orig);
                    let assign = if level_val == 0 {
                        crate::py_stmt!(
                            "
{name:id} = _dp_import_({module:literal}, __spec__, [{orig:literal}]).{attr:id}
",
                            name = binding,
                            module = module_name,
                            orig = orig,
                            attr = orig,
                        )
                    } else {
                        crate::py_stmt!(
                            "
{name:id} = _dp_import_({module:literal}, __spec__, [{orig:literal}], {level:id}).{attr:id}
",
                            name = binding,
                            module = module_name,
                            orig = orig,
                            level = level_val.to_string(),
                            attr = orig,
                        )
                    };
                    stmts.push(assign);
                }
                *stmt = crate::py_stmt!("{body:stmt}", body = stmts);
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assert_flatten_eq;
    use ruff_python_ast::visitor::transformer::walk_body;
    use ruff_python_parser::parse_module;

    fn rewrite(source: &str) -> Vec<Stmt> {
        let parsed = parse_module(source).expect("parse error");
        let mut module = parsed.into_syntax();
        let rewriter = ImportRewriter::new();
        walk_body(&rewriter, &mut module.body);
        module.body
    }

    #[test]
    fn rewrites_basic_import() {
        let output = rewrite(
            r#"
import a
"#,
        );
        let expected = r#"
a = _dp_import_("a", __spec__)
"#;
        assert_flatten_eq!(output, expected);
    }

    #[test]
    fn rewrites_from_import() {
        let output = rewrite(
            r#"
from a.b import c
"#,
        );
        let expected = r#"
c = _dp_import_("a.b", __spec__, ["c"]).c
"#;
        assert_flatten_eq!(output, expected);
    }

    #[test]
    fn rewrites_relative_import() {
        let output = rewrite(
            r#"
from ..a import b
"#,
        );
        let expected = r#"
b = _dp_import_("a", __spec__, ["b"], 2).b
"#;
        assert_flatten_eq!(output, expected);
    }

    #[test]
    fn inserts_after_future_and_docstring() {
        let parsed = parse_module(
            r#"
"doc"
from __future__ import annotations
x = 1
"#,
        )
        .expect("parse error");
        let mut module = parsed.into_syntax();
        ensure_import(&mut module, "__dp__");
        assert_flatten_eq!(
            module.body,
            r#"
"doc"
from __future__ import annotations
import __dp__
_dp_add = __dp__.add
_dp_sub = __dp__.sub
_dp_mul = __dp__.mul
_dp_matmul = __dp__.matmul
_dp_truediv = __dp__.truediv
_dp_floordiv = __dp__.floordiv
_dp_mod = __dp__.mod
_dp_pow = __dp__.pow
_dp_lshift = __dp__.lshift
_dp_rshift = __dp__.rshift
_dp_or_ = __dp__.or_
_dp_xor = __dp__.xor
_dp_and_ = __dp__.and_
_dp_getitem = __dp__.getitem
_dp_setitem = __dp__.setitem
_dp_delitem = __dp__.delitem
_dp_iadd = __dp__.iadd
_dp_isub = __dp__.isub
_dp_imul = __dp__.imul
_dp_imatmul = __dp__.imatmul
_dp_itruediv = __dp__.itruediv
_dp_imod = __dp__.imod
_dp_ipow = __dp__.ipow
_dp_ilshift = __dp__.ilshift
_dp_irshift = __dp__.irshift
_dp_ior = __dp__.ior
_dp_ixor = __dp__.ixor
_dp_iand = __dp__.iand
_dp_ifloordiv = __dp__.ifloordiv
_dp_pos = __dp__.pos
_dp_neg = __dp__.neg
_dp_invert = __dp__.invert
_dp_not_ = __dp__.not_
_dp_truth = __dp__.truth
_dp_eq = __dp__.eq
_dp_ne = __dp__.ne
_dp_lt = __dp__.lt
_dp_le = __dp__.le
_dp_gt = __dp__.gt
_dp_ge = __dp__.ge
_dp_is_ = __dp__.is_
_dp_is_not = __dp__.is_not
_dp_contains = __dp__.contains
_dp_next = __dp__.next
_dp_iter = __dp__.iter
_dp_aiter = __dp__.aiter
_dp_anext = __dp__.anext
_dp_isinstance = __dp__.isinstance
_dp_setattr = __dp__.setattr
_dp_resolve_bases = __dp__.resolve_bases
_dp_prepare_class = __dp__.prepare_class
_dp_exc_info = __dp__.exc_info
_dp_current_exception = __dp__.current_exception
_dp_raise_from = __dp__.raise_from
_dp_import_ = __dp__.import_
_dp_if_expr = __dp__.if_expr
_dp_or_expr = __dp__.or_expr
_dp_and_expr = __dp__.and_expr
x = 1
"#,
        );
    }
}
