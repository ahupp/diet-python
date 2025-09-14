use ruff_python_ast::str::{Quote, TripleQuotes};
use ruff_python_ast::str_prefix::StringLiteralPrefix;
use ruff_python_ast::{self as ast, Expr, Stmt};
use ruff_text_size::TextRange;

fn string_expr(value: &str) -> Expr {
    let flags = ast::StringLiteralFlags::empty()
        .with_quote_style(Quote::Double)
        .with_triple_quotes(TripleQuotes::No)
        .with_prefix(StringLiteralPrefix::Empty);
    let literal = ast::StringLiteral {
        range: TextRange::default(),
        node_index: ast::AtomicNodeIndex::default(),
        value: value.into(),
        flags,
    };
    Expr::StringLiteral(ast::ExprStringLiteral {
        node_index: ast::AtomicNodeIndex::default(),
        range: TextRange::default(),
        value: ast::StringLiteralValue::single(literal),
    })
}

pub fn rewrite(
    ast::StmtClassDef {
        name,
        body,
        arguments,
        ..
    }: ast::StmtClassDef,
) -> Stmt {
    let class_name = name.id.as_str().to_string();
    let ns_func_name = format!("_dp_ns_{}", class_name);
    let class_func_name = format!("_class_{}", class_name);

    // Build namespace function body
    let mut ns_body = Vec::new();
    ns_body.push(crate::py_stmt!("_dp_temp_ns = {}"));
    ns_body.push(crate::py_stmt!(
        "_dp_temp_ns[\"__module__\"] = _ns[\"__module__\"] = __name__"
    ));
    // TODO: correctly calculate the qualname of the class when nested
    ns_body.push(crate::py_stmt!(
        "_dp_temp_ns[\"__qualname__\"] = _ns[\"__qualname__\"] = {q:expr}",
        q = string_expr(&class_name)
    ));

    let mut original_body = body;
    if let Some(Stmt::Expr(ast::StmtExpr { value, .. })) = original_body.first() {
        if matches!(value.as_ref(), Expr::StringLiteral(_)) {
            if let Expr::StringLiteral(s) = value.as_ref() {
                ns_body.push(crate::py_stmt!(
                    "_dp_temp_ns[\"__doc__\"] = _ns[\"__doc__\"] = {doc:expr}",
                    doc = Expr::StringLiteral(s.clone())
                ));
            }
            original_body.remove(0);
        }
    }

    for stmt in original_body {
        match stmt {
            Stmt::Assign(ast::StmtAssign { targets, value, .. }) => {
                if let [Expr::Name(ast::ExprName { id, .. })] = targets.as_slice() {
                    ns_body.push(crate::py_stmt!(
                        "_dp_temp_ns[{k1:expr}] = _ns[{k2:expr}] = {v:expr}",
                        k1 = string_expr(id.as_str()),
                        k2 = string_expr(id.as_str()),
                        v = value
                    ));
                }
            }
            Stmt::AnnAssign(ast::StmtAnnAssign {
                target,
                value: Some(v),
                ..
            }) => {
                if let Expr::Name(ast::ExprName { id, .. }) = target.as_ref() {
                    ns_body.push(crate::py_stmt!(
                        "_dp_temp_ns[{k1:expr}] = _ns[{k2:expr}] = {v:expr}",
                        k1 = string_expr(id.as_str()),
                        k2 = string_expr(id.as_str()),
                        v = *v
                    ));
                }
            }
            Stmt::FunctionDef(func_def) => {
                let fn_name = func_def.name.id.to_string();
                let mk_name = format!("_mk_{}", fn_name);
                let mk_body = vec![
                    Stmt::FunctionDef(func_def),
                    crate::py_stmt!(
                        "\n{fn_name:id}.__qualname__ = _ns[\"__qualname__\"] + {suffix:expr}",
                        fn_name = fn_name.as_str(),
                        suffix = string_expr(&format!(".{}", fn_name))
                    ),
                    crate::py_stmt!("return {fn_name:id}", fn_name = fn_name.as_str()),
                ];
                ns_body.push(crate::py_stmt!(
                    "def {mk_name:id}():\n    {body:stmt}",
                    mk_name = mk_name.as_str(),
                    body = mk_body
                ));
                ns_body.push(crate::py_stmt!(
                    "_dp_temp_ns[{k1:expr}] = _ns[{k2:expr}] = {mk_name:id}()",
                    k1 = string_expr(&fn_name),
                    k2 = string_expr(&fn_name),
                    mk_name = mk_name.as_str()
                ));
            }
            other => ns_body.push(other),
        }
    }

    let ns_func = crate::py_stmt!(
        "def {name:id}(_ns):\n    {body:stmt}",
        name = ns_func_name.as_str(),
        body = ns_body
    );

    // Build class helper function
    let mut bases = Vec::new();
    let mut kw_keys = Vec::new();
    let mut kw_vals = Vec::new();
    if let Some(args) = arguments {
        bases.extend(args.args.into_vec());
        for kw in args.keywords.into_vec() {
            if let Some(arg) = kw.arg {
                kw_keys.push(string_expr(arg.as_str()));
                kw_vals.push(kw.value);
            }
        }
    }
    let has_kw = !kw_keys.is_empty();

    let bases_tuple = Expr::Tuple(ast::ExprTuple {
        node_index: ast::AtomicNodeIndex::default(),
        range: TextRange::default(),
        elts: bases,
        ctx: ast::ExprContext::Load,
        parenthesized: true,
    });

    let bases_stmt = crate::py_stmt!("bases = __dp__.resolve_bases({b:expr})", b = bases_tuple);

    let prepare_stmt = if has_kw {
        let items: Vec<ast::DictItem> = kw_keys
            .into_iter()
            .zip(kw_vals.into_iter())
            .map(|(k, v)| ast::DictItem {
                key: Some(k),
                value: v,
            })
            .collect();
        let dict_expr = Expr::Dict(ast::ExprDict {
            node_index: ast::AtomicNodeIndex::default(),
            range: TextRange::default(),
            items,
        });
        crate::py_stmt!(
            "meta, ns, kwds = __dp__.prepare_class({n:expr}, bases, {d:expr})",
            n = string_expr(&class_name),
            d = dict_expr
        )
    } else {
        crate::py_stmt!(
            "meta, ns, kwds = __dp__.prepare_class({n:expr}, bases)",
            n = string_expr(&class_name)
        )
    };

    let exec_stmt = crate::py_stmt!("{ns_func:id}(ns)", ns_func = ns_func_name.as_str());

    let cls_stmt = if has_kw {
        crate::py_stmt!(
            "cls = meta({n:expr}, bases, ns, **kwds)",
            n = string_expr(&class_name)
        )
    } else {
        crate::py_stmt!(
            "cls = meta({n:expr}, bases, ns)",
            n = string_expr(&class_name)
        )
    };

    let ret_stmt = crate::py_stmt!("return cls");

    let class_func = crate::py_stmt!(
        "def {name:id}():\n    {body:stmt}",
        name = class_func_name.as_str(),
        body = vec![bases_stmt, prepare_stmt, exec_stmt, cls_stmt, ret_stmt]
    );

    let call_stmt = crate::py_stmt!(
        "{cls_name:id} = {builder:id}()",
        cls_name = class_name.as_str(),
        builder = class_func_name.as_str()
    );

    crate::py_stmt!("{body:stmt}", body = vec![ns_func, class_func, call_stmt])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_util::assert_transform_eq;

    #[test]
    fn lowers_simple_class() {
        let input = r#"
class C:
    x = 1
"#;
        let expected = r#"
def _dp_ns_C(_ns):
    _dp_temp_ns = dict(())
    _dp_tmp_1 = __name__
    getattr(__dp__, "setitem")(_dp_temp_ns, "__module__", _dp_tmp_1)
    getattr(__dp__, "setitem")(_ns, "__module__", _dp_tmp_1)
    _dp_tmp_2 = "C"
    getattr(__dp__, "setitem")(_dp_temp_ns, "__qualname__", _dp_tmp_2)
    getattr(__dp__, "setitem")(_ns, "__qualname__", _dp_tmp_2)
    _dp_tmp_3 = 1
    getattr(__dp__, "setitem")(_dp_temp_ns, "x", _dp_tmp_3)
    getattr(__dp__, "setitem")(_ns, "x", _dp_tmp_3)
def _class_C():
    bases = getattr(__dp__, "resolve_bases")(())
    _dp_tmp_4 = getattr(__dp__, "prepare_class")("C", bases)
    meta = getattr(__dp__, "getitem")(_dp_tmp_4, 0)
    ns = getattr(__dp__, "getitem")(_dp_tmp_4, 1)
    kwds = getattr(__dp__, "getitem")(_dp_tmp_4, 2)
    _dp_ns_C(ns)
    cls = meta("C", bases, ns)
    return cls
C = _class_C()
"#;
        assert_transform_eq(input, expected);
    }

    #[test]
    fn lowers_inherits() {
        let input = r#"
class C(B):
    pass
"#;
        let expected = r#"
def _dp_ns_C(_ns):
    _dp_temp_ns = dict(())
    _dp_tmp_1 = __name__
    getattr(__dp__, "setitem")(_dp_temp_ns, "__module__", _dp_tmp_1)
    getattr(__dp__, "setitem")(_ns, "__module__", _dp_tmp_1)
    _dp_tmp_2 = "C"
    getattr(__dp__, "setitem")(_dp_temp_ns, "__qualname__", _dp_tmp_2)
    getattr(__dp__, "setitem")(_ns, "__qualname__", _dp_tmp_2)
    pass
def _class_C():
    bases = getattr(__dp__, "resolve_bases")((B,))
    _dp_tmp_3 = getattr(__dp__, "prepare_class")("C", bases)
    meta = getattr(__dp__, "getitem")(_dp_tmp_3, 0)
    ns = getattr(__dp__, "getitem")(_dp_tmp_3, 1)
    kwds = getattr(__dp__, "getitem")(_dp_tmp_3, 2)
    _dp_ns_C(ns)
    cls = meta("C", bases, ns)
    return cls
C = _class_C()
"#;
        assert_transform_eq(input, expected);
    }

    #[test]
    fn lowers_with_docstring_and_keywords() {
        let input = r#"
class C(B, metaclass=Meta, kw=1):
    'doc'
    x = 2
"#;
        let expected = r#"
def _dp_ns_C(_ns):
    _dp_temp_ns = dict(())
    _dp_tmp_1 = __name__
    getattr(__dp__, "setitem")(_dp_temp_ns, "__module__", _dp_tmp_1)
    getattr(__dp__, "setitem")(_ns, "__module__", _dp_tmp_1)
    _dp_tmp_2 = "C"
    getattr(__dp__, "setitem")(_dp_temp_ns, "__qualname__", _dp_tmp_2)
    getattr(__dp__, "setitem")(_ns, "__qualname__", _dp_tmp_2)
    _dp_tmp_3 = "doc"
    getattr(__dp__, "setitem")(_dp_temp_ns, "__doc__", _dp_tmp_3)
    getattr(__dp__, "setitem")(_ns, "__doc__", _dp_tmp_3)
    _dp_tmp_4 = 2
    getattr(__dp__, "setitem")(_dp_temp_ns, "x", _dp_tmp_4)
    getattr(__dp__, "setitem")(_ns, "x", _dp_tmp_4)
def _class_C():
    bases = getattr(__dp__, "resolve_bases")((B,))
    _dp_tmp_5 = getattr(__dp__, "prepare_class")("C", bases, dict((("metaclass", Meta), ("kw", 1))))
    meta = getattr(__dp__, "getitem")(_dp_tmp_5, 0)
    ns = getattr(__dp__, "getitem")(_dp_tmp_5, 1)
    kwds = getattr(__dp__, "getitem")(_dp_tmp_5, 2)
    _dp_ns_C(ns)
    cls = meta("C", bases, ns, **kwds)
    return cls
C = _class_C()
"#;
        assert_transform_eq(input, expected);
    }

    #[test]
    fn lowers_method() {
        let input = r#"
class C:
    def m(self):
        return 1
"#;
        let expected = r#"
def _dp_ns_C(_ns):
    _dp_temp_ns = dict(())
    _dp_tmp_1 = __name__
    getattr(__dp__, "setitem")(_dp_temp_ns, "__module__", _dp_tmp_1)
    getattr(__dp__, "setitem")(_ns, "__module__", _dp_tmp_1)
    _dp_tmp_2 = "C"
    getattr(__dp__, "setitem")(_dp_temp_ns, "__qualname__", _dp_tmp_2)
    getattr(__dp__, "setitem")(_ns, "__qualname__", _dp_tmp_2)

    def _mk_m():

        def m(self):
            return 1
        getattr(__dp__, "setattr")(m, "__qualname__", getattr(__dp__, "add")(getattr(__dp__, "getitem")(_ns, "__qualname__"), ".m"))
        return m
    _dp_tmp_3 = _mk_m()
    getattr(__dp__, "setitem")(_dp_temp_ns, "m", _dp_tmp_3)
    getattr(__dp__, "setitem")(_ns, "m", _dp_tmp_3)
def _class_C():
    bases = getattr(__dp__, "resolve_bases")(())
    _dp_tmp_4 = getattr(__dp__, "prepare_class")("C", bases)
    meta = getattr(__dp__, "getitem")(_dp_tmp_4, 0)
    ns = getattr(__dp__, "getitem")(_dp_tmp_4, 1)
    kwds = getattr(__dp__, "getitem")(_dp_tmp_4, 2)
    _dp_ns_C(ns)
    cls = meta("C", bases, ns)
    return cls
C = _class_C()
"#;
        assert_transform_eq(input, expected);
    }
}
